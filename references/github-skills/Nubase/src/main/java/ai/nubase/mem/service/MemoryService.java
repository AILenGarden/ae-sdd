package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.dto.AddMemoryRequest;
import ai.nubase.mem.dto.MemoryEventResponse;
import ai.nubase.mem.dto.MemoryResponse;
import ai.nubase.mem.dto.SearchMemoryRequest;
import ai.nubase.mem.entity.Memory;
import ai.nubase.mem.entity.MemoryHistory;
import ai.nubase.mem.entity.SessionMessage;
import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.repository.EntityRepository;
import ai.nubase.mem.repository.MemoryHistoryRepository;
import ai.nubase.mem.repository.MemoryRepository;
import ai.nubase.mem.repository.SessionMessageRepository;
import ai.nubase.mem.service.FactExtractionService.ExtractedEntity;
import ai.nubase.mem.service.FactExtractionService.FactExtractionResult;
import ai.nubase.mem.service.MemoryDecisionService.Decision;
import ai.nubase.mem.service.MemoryDecisionService.ExistingMemory;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * Top-level memory orchestration service.
 *
 * <p>Implements the mem0-style flow:
 * <ol>
 *   <li>Extract candidate facts from a conversation (optional, controlled by {@code infer}).</li>
 *   <li>Embed each fact.</li>
 *   <li>For each fact, find the top-K nearest existing memories.</li>
 *   <li>Ask the LLM to choose ADD / UPDATE / DELETE / NONE per fact.</li>
 *   <li>Apply the decisions inside a transaction; write {@code mem.memory_history} entries.</li>
 * </ol>
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class MemoryService {

    /**
     * Bean name of the JDBC-bound transaction manager used by every {@code @Transactional}
     * method in this class. Defined in {@code MultiDatabaseConfiguration}. Using the named
     * manager (instead of the default JPA one) guarantees that our JdbcTemplate writes
     * actually share a connection and roll back together on failure.
     */
    public static final String MEM_TX_MANAGER = "memJdbcTransactionManager";

    private static final int DECISION_TOP_K = 5;

    private final MemProperties memProperties;
    private final MemConfigResolver memConfig;
    private final EmbeddingService embeddingService;
    private final FactExtractionService factExtractionService;
    private final MemoryDecisionService memoryDecisionService;
    private final QueryEntityExtractionService queryEntityExtractionService;
    private final EntityStoreService entityStoreService;

    private final MemoryRepository memoryRepository;
    private final MemoryHistoryRepository memoryHistoryRepository;
    private final SessionMessageRepository sessionMessageRepository;
    private final EntityRepository entityRepository;

    @Transactional(transactionManager = MEM_TX_MANAGER)
    public List<MemoryEventResponse> add(AddMemoryRequest req) {
        // Authorization gate: for non-admin callers, force userId = JWT sub regardless of body input.
        // Mutate the request so every downstream call sees the trusted owner.
        MemoryAuthScope scope = MemoryAuthScope.fromContextRequireOwner(
                req.getUserId(), req.getAgentId(), req.getRunId());
        req.setUserId(scope.getUserId());

        List<ChatMessage> messages = req.getMessages() != null ? req.getMessages() : List.of();
        boolean infer = req.getInfer() == null || req.getInfer();

        if (!infer) {
            // Verbatim path: write session window first (no extraction → no double-count concern),
            // then persist each message as its own memory.
            if (memConfig.sessionEnabled()) {
                persistSessionMessages(messages, req);
            }
            return addVerbatim(messages, req);
        }

        // Infer path order matters:
        //   1. Read session-window history (if enabled) BEFORE writing the current messages,
        //      otherwise the LLM would see the current turn twice.
        //   2. Extract facts using history + current.
        //   3. Persist the current messages to the session window for the NEXT call.
        //   4. Apply decisions.
        List<ChatMessage> extractionInput = buildExtractionInput(messages, req);

        FactExtractionResult extraction = factExtractionService.extract(extractionInput);

        if (memConfig.sessionEnabled()) {
            persistSessionMessages(messages, req);
        }

        if (extraction.isEmpty()) {
            log.info("No facts extracted from {} messages ({} including session history)",
                    messages.size(), extractionInput.size());
            return List.of();
        }

        return applyFactsWithDecisions(extraction, req);
    }

    /**
     * Build the message list sent to the fact-extractor. When
     * {@code nubase.mem.session.injectIntoExtraction} is true and a session scope is
     * resolvable, prepends the recent N messages from {@code mem.session_messages} so the
     * LLM can see continuing conversation context.
     *
     * <p>Returns the original list unchanged when injection is disabled or the lookup fails
     * (we never let session-history reads block the main flow).
     */
    private List<ChatMessage> buildExtractionInput(List<ChatMessage> currentMessages,
                                                   AddMemoryRequest req) {
        if (!memConfig.sessionEnabled() || !memConfig.sessionInjectIntoExtraction()) {
            return currentMessages;
        }
        String scope = sessionScope(req.getUserId(), req.getAgentId(), req.getRunId());
        if (scope == null) {
            return currentMessages;
        }
        try {
            List<SessionMessage> history = sessionMessageRepository.findRecent(
                    scope, memConfig.sessionMaxMessages());
            if (history.isEmpty()) {
                return currentMessages;
            }
            List<ChatMessage> merged = new ArrayList<>(history.size() + currentMessages.size());
            for (SessionMessage h : history) {
                merged.add(ChatMessage.builder()
                        .role(h.getRole())
                        .content(h.getContent())
                        .name(h.getName())
                        .build());
            }
            merged.addAll(currentMessages);
            log.debug("Prepended {} session-history messages to {} current messages",
                    history.size(), currentMessages.size());
            return merged;
        } catch (Exception e) {
            log.warn("Failed to load session history for scope '{}', extracting without context: {}",
                    scope, e.getMessage());
            return currentMessages;
        }
    }

    private List<MemoryEventResponse> applyFactsWithDecisions(FactExtractionResult extraction,
                                                              AddMemoryRequest req) {
        List<String> facts = extraction.getFacts();
        List<List<ExtractedEntity>> entitiesPerFact = extraction.getEntities();
        List<float[]> factEmbeddings = embeddingService.embedBatch(facts);

        // Gather union of nearest existing memories across all facts.
        Map<UUID, Memory> candidates = new HashMap<>();
        for (float[] e : factEmbeddings) {
            List<Memory> near = memoryRepository.searchByVector(
                    e, req.getUserId(), req.getAgentId(), req.getRunId(), DECISION_TOP_K, null);
            for (Memory m : near) {
                candidates.putIfAbsent(m.getId(), m);
            }
        }
        List<ExistingMemory> existing = candidates.values().stream()
                .map(m -> ExistingMemory.builder().id(m.getId().toString()).text(m.getMemory()).build())
                .toList();

        List<Decision> decisions = memoryDecisionService.decide(existing, facts);

        // Index facts by text for ADD operations and to look up per-fact entities.
        Map<String, float[]> embByText = new HashMap<>();
        Map<String, List<ExtractedEntity>> entitiesByText = new HashMap<>();
        for (int i = 0; i < facts.size(); i++) {
            embByText.put(facts.get(i), factEmbeddings.get(i));
            List<ExtractedEntity> entitiesForFact = entitiesPerFact != null && i < entitiesPerFact.size()
                    ? entitiesPerFact.get(i)
                    : List.of();
            entitiesByText.put(facts.get(i), entitiesForFact);
        }

        List<MemoryEventResponse> out = new ArrayList<>();
        for (Decision d : decisions) {
            MemoryEventResponse ev = switch (d.getEvent() == null ? "" : d.getEvent()) {
                case "ADD" -> applyAdd(d, embByText, entitiesByText, req);
                case "UPDATE" -> applyUpdate(d, entitiesByText, req);
                case "DELETE" -> applyDelete(d, req);
                case "NONE" -> applyNone(d, entitiesByText, req);
                default -> {
                    log.warn("Unknown decision event: {}", d.getEvent());
                    yield skipped(d, "unknown event '" + d.getEvent() + "'");
                }
            };
            // apply* methods return null for malformed decisions (missing text/id, memory not
            // found by owner). Replace with an explicit SKIPPED event so clients never see
            // null array elements — they can still correlate the skip to a decision via the id.
            if (ev == null) {
                ev = skipped(d, "decision could not be applied (missing id/text or unauthorized memory)");
            }
            out.add(ev);
        }
        return out;
    }

    /** Build a SKIPPED event so we always return one row per decision (no null gaps). */
    private static MemoryEventResponse skipped(Decision d, String reason) {
        return MemoryEventResponse.builder()
                .id(parseUuidQuiet(d.getId()))
                .memory(d.getText())
                .event("SKIPPED")
                .previousMemory(reason)
                .build();
    }

    /**
     * The fact is already represented by an existing memory — no insert needed, but we still
     * link any freshly-extracted entities. LLM output is non-deterministic, so a second pass
     * over the "same" fact can surface entities that weren't captured the first time.
     * {@code appendLinkedMemory} is idempotent, so re-linking known entities is cheap and safe.
     *
     * <p>Correlation relies on the prompt echoing the original fact text back in
     * {@code decision.text}; if the LLM omits it or rewrites it beyond exact match, we
     * skip the relink (no false positives).
     */
    private MemoryEventResponse applyNone(Decision d,
                                          Map<String, List<ExtractedEntity>> entitiesByText,
                                          AddMemoryRequest req) {
        UUID id = parseUuidQuiet(d.getId());
        // Owner-scoped lookup — refuses to leak across users even if the LLM picks an id
        // outside the current owner's scope.
        Memory existingMem = id != null
                ? memoryRepository.findByIdForOwner(id, req.getUserId()).orElse(null)
                : null;

        if (id != null && existingMem != null && d.getText() != null) {
            List<ExtractedEntity> entities = entitiesByText.get(d.getText());
            if (entities != null && !entities.isEmpty()) {
                entityStoreService.linkEntities(id, entities,
                        req.getUserId(), req.getAgentId(), req.getRunId());
            }
        }

        return MemoryEventResponse.builder()
                .id(id)
                .memory(existingMem != null ? existingMem.getMemory() : null)
                .event("NONE")
                .build();
    }

    private MemoryEventResponse applyAdd(Decision d,
                                         Map<String, float[]> embByText,
                                         Map<String, List<ExtractedEntity>> entitiesByText,
                                         AddMemoryRequest req) {
        String text = d.getText();
        if (text == null || text.isBlank()) {
            return null;
        }
        float[] vec = embByText.get(text);
        if (vec == null) {
            vec = embeddingService.embed(text);
        }
        MemoryEventResponse ev = createMemory(text, vec, req, null);
        if (ev != null && "ADD".equals(ev.getEvent()) && ev.getId() != null) {
            List<ExtractedEntity> entities = entitiesByText.getOrDefault(text, List.of());
            entityStoreService.linkEntities(ev.getId(), entities,
                    req.getUserId(), req.getAgentId(), req.getRunId());
        }
        return ev;
    }

    private MemoryEventResponse applyUpdate(Decision d,
                                            Map<String, List<ExtractedEntity>> entitiesByText,
                                            AddMemoryRequest req) {
        UUID id = parseUuidQuiet(d.getId());
        if (id == null) {
            log.warn("UPDATE decision missing valid existing id: {}", d);
            return null;
        }
        // Owner-scoped lookup: even though the LLM picks the id from a list we already
        // scoped to the caller, hallucinated cross-owner UUIDs would otherwise sneak through.
        Optional<Memory> existing = memoryRepository.findByIdForOwner(id, req.getUserId());
        if (existing.isEmpty()) {
            log.warn("UPDATE decision references unknown or unauthorized memory id {}; "
                    + "treating as ADD", id);
            return applyAdd(d, Map.of(), entitiesByText, req);
        }
        String newText = d.getText();
        if (newText == null || newText.isBlank()) {
            return null;
        }
        float[] vec = embeddingService.embed(newText);
        String hash = sha256(newText);
        memoryRepository.updateContent(id, newText, vec, hash);
        if (memConfig.isHistoryEnabled()) {
            memoryHistoryRepository.insert(MemoryHistory.builder()
                    .memoryId(id)
                    .oldValue(existing.get().getMemory())
                    .newValue(newText)
                    .event("UPDATE")
                    .build());
        }
        // Strip every existing entity link first — UPDATE means the memory text changed, so
        // entities that used to apply may no longer be accurate. Then link fresh entities for
        // the new text. Without this, "I like sushi" → "I prefer steak now" would still get
        // boosted on a "sushi" query.
        entityStoreService.unlinkMemory(id,
                req.getUserId(), req.getAgentId(), req.getRunId());
        List<ExtractedEntity> entities = entitiesByText.getOrDefault(newText, List.of());
        if (!entities.isEmpty()) {
            entityStoreService.linkEntities(id, entities,
                    req.getUserId(), req.getAgentId(), req.getRunId());
        }
        return MemoryEventResponse.builder()
                .id(id)
                .memory(newText)
                .previousMemory(existing.get().getMemory())
                .event("UPDATE")
                .build();
    }

    private MemoryEventResponse applyDelete(Decision d, AddMemoryRequest req) {
        UUID id = parseUuidQuiet(d.getId());
        if (id == null) {
            return null;
        }
        // Owner-scoped lookup — see applyUpdate comment.
        Optional<Memory> existing = memoryRepository.findByIdForOwner(id, req.getUserId());
        if (existing.isEmpty()) {
            return null;
        }
        memoryRepository.softDelete(id);
        entityStoreService.unlinkMemory(id, req.getUserId(), req.getAgentId(), req.getRunId());
        if (memConfig.isHistoryEnabled()) {
            memoryHistoryRepository.insert(MemoryHistory.builder()
                    .memoryId(id)
                    .oldValue(existing.get().getMemory())
                    .event("DELETE")
                    .build());
        }
        return MemoryEventResponse.builder()
                .id(id)
                .memory(existing.get().getMemory())
                .event("DELETE")
                .build();
    }

    private List<MemoryEventResponse> addVerbatim(List<ChatMessage> messages, AddMemoryRequest req) {
        List<MemoryEventResponse> out = new ArrayList<>();
        for (ChatMessage m : messages) {
            if ("system".equalsIgnoreCase(m.getRole())) {
                continue;
            }
            String content = m.getContent();
            if (content == null || content.isBlank()) {
                continue;
            }
            float[] vec = embeddingService.embed(content);
            MemoryEventResponse ev = createMemory(content, vec, req, m);
            out.add(ev);
        }
        return out;
    }

    private MemoryEventResponse createMemory(String text, float[] embedding, AddMemoryRequest req,
                                             ChatMessage source) {
        String hash = sha256(text);
        // Hash-based dedupe within the same owner triple
        Optional<Memory> dup = memoryRepository.findByHash(
                req.getUserId(), req.getAgentId(), req.getRunId(), hash);
        if (dup.isPresent()) {
            return MemoryEventResponse.builder()
                    .id(dup.get().getId())
                    .memory(dup.get().getMemory())
                    .event("NONE")
                    .build();
        }

        Memory m = Memory.builder()
                .userId(req.getUserId())
                .agentId(req.getAgentId())
                .runId(req.getRunId())
                .memory(text)
                .embedding(embedding)
                .metadata(req.getMetadata() != null ? req.getMetadata() : new HashMap<>())
                .actorId(source != null ? source.getName() : null)
                .role(source != null ? source.getRole() : null)
                .hash(hash)
                .build();
        UUID id = memoryRepository.insert(m);
        if (memConfig.isHistoryEnabled()) {
            memoryHistoryRepository.insert(MemoryHistory.builder()
                    .memoryId(id)
                    .newValue(text)
                    .event("ADD")
                    .build());
        }
        return MemoryEventResponse.builder()
                .id(id)
                .memory(text)
                .event("ADD")
                .build();
    }

    private void persistSessionMessages(List<ChatMessage> messages, AddMemoryRequest req) {
        String scope = sessionScope(req.getUserId(), req.getAgentId(), req.getRunId());
        if (scope == null) {
            return;
        }
        int max = memConfig.sessionMaxMessages();
        for (ChatMessage m : messages) {
            if ("system".equalsIgnoreCase(m.getRole())) {
                continue;
            }
            SessionMessage sm = SessionMessage.builder()
                    .sessionScope(scope)
                    .userId(req.getUserId())
                    .role(m.getRole())
                    .content(m.getContent())
                    .name(m.getName())
                    .build();
            sessionMessageRepository.insertWithEviction(sm, max);
        }
    }

    // ---------- read APIs ----------

    public List<MemoryResponse> list(UUID userId, String agentId, String runId, int limit) {
        return list(userId, agentId, runId, null, limit);
    }

    public List<MemoryResponse> list(UUID userId, String agentId, String runId,
                                     Map<String, Object> metadataFilters, int limit) {
        MemoryAuthScope scope = MemoryAuthScope.fromContextRequireOwner(userId, agentId, runId);
        return memoryRepository.listByOwner(
                        scope.getUserId(), scope.getAgentId(), scope.getRunId(),
                        metadataFilters, limit)
                .stream()
                .map(MemoryResponse::from)
                .toList();
    }

    /**
     * Paginated list for the admin UI table.
     *
     * <p>Owner triple is optional — service_role callers can pass nulls to enumerate every
     * memory in the tenant. Non-admin callers still get their {@code userId} forced from JWT.
     */
    public ai.nubase.mem.dto.PagedResponse<MemoryResponse> listPaged(
            UUID userId, String agentId, String runId,
            Map<String, Object> metadataFilters,
            int page, int pageSize) {
        // fromContext (not Require) so admin with empty filter can list the whole tenant.
        MemoryAuthScope scope = MemoryAuthScope.fromContext(userId, agentId, runId);
        int safePage = Math.max(1, page);
        int safeSize = Math.max(1, Math.min(500, pageSize));

        List<MemoryResponse> items = memoryRepository.listByOwnerPaged(
                        scope.getUserId(), scope.getAgentId(), scope.getRunId(),
                        metadataFilters, safePage, safeSize)
                .stream()
                .map(MemoryResponse::from)
                .toList();
        long total = memoryRepository.countByOwner(
                scope.getUserId(), scope.getAgentId(), scope.getRunId(), metadataFilters);
        return ai.nubase.mem.dto.PagedResponse.of(items, total, safePage, safeSize);
    }

    /**
     * Fetch a single memory by id with owner-based authorization.
     *
     * <p>Returns {@link Optional#empty()} both for "not found" and for "exists but the caller
     * doesn't own it" — collapsing the two cases prevents existence-leaking via id enumeration.
     */
    public Optional<MemoryResponse> get(UUID id) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(null, null, null);
        return loadByIdForScope(id, scope).map(MemoryResponse::from);
    }

    /**
     * Load a memory and verify the current caller is allowed to see it.
     *
     * <p>This is the single chokepoint for id-based authorization. Every method that takes
     * an id from the client ({@code get}, {@code update}, {@code delete}, {@code history})
     * goes through here first.
     *
     * <p>Defense-in-depth: pushes the owner check into SQL via
     * {@link MemoryRepository#findByIdForOwner}, AND re-verifies in Java in case a future
     * repo refactor breaks the SQL. Service-role callers with {@link MemoryAuthScope#isUnrestricted}
     * skip the owner WHERE clause but the Java check is a no-op for them (canAccess returns true).
     */
    private Optional<Memory> loadByIdForScope(UUID id, MemoryAuthScope scope) {
        UUID ownerFilter = scope.isUnrestricted() ? null : scope.getUserId();
        Optional<Memory> mem = memoryRepository.findByIdForOwner(id, ownerFilter);
        if (mem.isEmpty()) {
            return Optional.empty();
        }
        if (!scope.canAccess(mem.get().getUserId())) {
            // Belt + suspenders: should be unreachable given findByIdForOwner above, but if
            // the repo SQL ever drifts we still refuse here.
            log.warn("Cross-owner access denied: caller userId={} tried to access memory {} "
                    + "owned by userId={}",
                    scope.getUserId(), id, mem.get().getUserId());
            return Optional.empty();
        }
        return mem;
    }

    /**
     * Multi-signal search: vector + BM25 + (optional) entity boost, fused via {@link ScoreFusion}.
     *
     * <p>{@code threshold} in the request is preserved as a cosine-<em>distance</em> cap on the
     * raw vector results (back-compat). The same threshold is converted to a similarity floor
     * for the fusion gate so BM25/entity-only hits with no vector match can't sneak past it.
     *
     * <p>{@code metadataFilters} are applied uniformly to both retrieval channels.
     */
    public List<MemoryResponse> search(SearchMemoryRequest req) {
        MemoryAuthScope scope = MemoryAuthScope.fromContextRequireOwner(
                req.getUserId(), req.getAgentId(), req.getRunId());
        // Replace whatever the body claimed with the trusted owner; downstream queries use req.
        req.setUserId(scope.getUserId());

        if (req.getQuery() == null || req.getQuery().isBlank()) {
            return List.of();
        }
        int topK = req.getTopK() != null ? req.getTopK() : memConfig.searchDefaultTopK();
        Double distanceThreshold = req.getThreshold() != null
                ? req.getThreshold()
                : memConfig.searchDefaultThreshold();

        Map<String, Object> filters = req.getMetadataFilters();

        // Over-fetch like mem0 to give the fusion stage room to work with.
        int internalLimit = Math.max(topK * 4, 60);

        float[] qVec = embeddingService.embed(req.getQuery());

        List<Memory> semantic = memoryRepository.searchByVector(
                qVec, req.getUserId(), req.getAgentId(), req.getRunId(),
                filters, internalLimit, distanceThreshold);

        List<Memory> bm25 = memoryRepository.searchByText(
                req.getQuery(), req.getUserId(), req.getAgentId(), req.getRunId(),
                filters, internalLimit);

        // Compute entity boosts from query — best-effort, never fails the search.
        Map<UUID, Double> entityBoosts;
        try {
            List<ExtractedEntity> queryEntities = queryEntityExtractionService.extract(req.getQuery());
            entityBoosts = entityStoreService.computeBoosts(
                    queryEntities, req.getUserId(), req.getAgentId(), req.getRunId());
        } catch (Exception e) {
            log.warn("Entity-boost computation failed, continuing without it: {}", e.getMessage());
            entityBoosts = Map.of();
        }

        // similarityThreshold = 1 - distanceThreshold (only used to gate non-vector hits;
        // vector hits are already gated inside the repository).
        double similarityFloor = Math.max(0.0, 1.0 - distanceThreshold);

        List<Memory> fused = ScoreFusion.fuse(semantic, bm25, entityBoosts, similarityFloor, topK);

        return fused.stream().map(MemoryResponse::from).toList();
    }

    @Transactional(transactionManager = MEM_TX_MANAGER)
    public Optional<MemoryEventResponse> update(UUID id, String newText) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(null, null, null);
        Optional<Memory> existing = loadByIdForScope(id, scope);
        if (existing.isEmpty()) {
            return Optional.empty();
        }
        Memory mem = existing.get();
        float[] vec = embeddingService.embed(newText);
        memoryRepository.updateContent(id, newText, vec, sha256(newText));
        // Strip entity links for the old text — this endpoint doesn't get fresh entity
        // extraction (the caller passes just the new text), so we drop all and leave the
        // memory un-boosted by entities until the next infer=true add reaches it.
        entityStoreService.unlinkMemory(id,
                mem.getUserId(), mem.getAgentId(), mem.getRunId());
        if (memConfig.isHistoryEnabled()) {
            memoryHistoryRepository.insert(MemoryHistory.builder()
                    .memoryId(id)
                    .oldValue(mem.getMemory())
                    .newValue(newText)
                    .event("UPDATE")
                    .build());
        }
        return Optional.of(MemoryEventResponse.builder()
                .id(id)
                .memory(newText)
                .previousMemory(mem.getMemory())
                .event("UPDATE")
                .build());
    }

    @Transactional(transactionManager = MEM_TX_MANAGER)
    public boolean delete(UUID id) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(null, null, null);
        Optional<Memory> existing = loadByIdForScope(id, scope);
        if (existing.isEmpty()) {
            return false;
        }
        memoryRepository.softDelete(id);
        entityStoreService.unlinkMemory(id,
                existing.get().getUserId(),
                existing.get().getAgentId(),
                existing.get().getRunId());
        if (memConfig.isHistoryEnabled()) {
            memoryHistoryRepository.insert(MemoryHistory.builder()
                    .memoryId(id)
                    .oldValue(existing.get().getMemory())
                    .event("DELETE")
                    .build());
        }
        return true;
    }

    /**
     * Fetch the audit trail for a single memory, gated by the same owner check as {@link #get}.
     * Returns an empty list if the memory doesn't exist or the caller doesn't own it — never
     * leaks history of other users' memories.
     */
    public List<MemoryHistory> history(UUID id) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(null, null, null);
        if (loadByIdForScope(id, scope).isEmpty()) {
            return List.of();
        }
        return memoryHistoryRepository.findByMemoryId(id);
    }

    /**
     * List entities linked to a single memory. Gated by memory ownership — same as
     * {@link #history}.
     */
    public List<ai.nubase.mem.entity.Entity> linkedEntities(UUID id) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(null, null, null);
        var mem = loadByIdForScope(id, scope);
        return entityStoreService.findLinkedTo(id, mem);
    }

    /**
     * Batch soft-delete all non-deleted memories matching the owner triple.
     *
     * <p>Before the soft-delete UPDATE, every affected memory id is stripped from
     * {@code mem.entities.linked_memory_ids} so the entity store doesn't accumulate
     * dangling pointers. The cleanup runs in the same transaction as the soft-delete,
     * so a partial failure rolls back both.
     *
     * @return number of memories soft-deleted
     */
    @Transactional(transactionManager = MEM_TX_MANAGER)
    public int deleteAll(UUID userId, String agentId, String runId) {
        MemoryAuthScope scope = MemoryAuthScope.fromContextRequireOwner(userId, agentId, runId);
        UUID effUserId = scope.getUserId();
        String effAgentId = scope.getAgentId();
        String effRunId = scope.getRunId();

        // 1. Collect the ids we're about to delete so we can clean entity links.
        List<UUID> victimIds = memoryRepository.selectIdsByOwner(effUserId, effAgentId, effRunId);
        if (victimIds.isEmpty()) {
            return 0;
        }

        // 2. Strip the victims from every entity in the owner scope (single SQL UPDATE).
        entityStoreService.unlinkMemoriesBulk(victimIds, effUserId, effAgentId, effRunId);

        // 3. Soft-delete the memories.
        int count = memoryRepository.softDeleteByOwner(effUserId, effAgentId, effRunId);
        log.info("Batch soft-deleted {} memories for owner userId={} agentId={} runId={} "
                        + "(entity links cleaned for {} ids)",
                count, effUserId, effAgentId, effRunId, victimIds.size());
        return count;
    }

    /**
     * Aggregate stats for the admin dashboard.
     *
     * <p>Scope behavior matches the rest of the API: service_role with no owner filter sees
     * the entire tenant; authenticated users see only their own JWT-scoped data.
     */
    public ai.nubase.mem.dto.MemoryStatsResponse stats(UUID userId, String agentId, String runId) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(userId, agentId, runId);

        long totalMem = memoryRepository.countByOwner(
                scope.getUserId(), scope.getAgentId(), scope.getRunId(), null);
        long totalEnt = entityRepository.countByOwner(
                scope.getUserId(), scope.getAgentId(), scope.getRunId());

        Map<String, Long> eventCounts = memoryHistoryRepository.countEventsSince(
                24 * 60, scope.getUserId());
        var activity = ai.nubase.mem.dto.MemoryStatsResponse.RecentActivity.builder()
                .add(eventCounts.getOrDefault("ADD", 0L))
                .update(eventCounts.getOrDefault("UPDATE", 0L))
                .delete(eventCounts.getOrDefault("DELETE", 0L))
                .build();

        // Top users only when caller is admin AND unrestricted — otherwise leaks user ids.
        List<ai.nubase.mem.dto.MemoryStatsResponse.UserCount> topUsers;
        if (scope.isUnrestricted()) {
            topUsers = memoryRepository.topUsersByMemoryCount(5).stream()
                    .map(row -> ai.nubase.mem.dto.MemoryStatsResponse.UserCount.builder()
                            .userId((UUID) row.get("user_id"))
                            .count((Long) row.get("count"))
                            .build())
                    .toList();
        } else {
            topUsers = List.of();
        }

        return ai.nubase.mem.dto.MemoryStatsResponse.builder()
                .totalMemories(totalMem)
                .totalEntities(totalEnt)
                .last24h(activity)
                .topUsers(topUsers)
                .build();
    }

    /**
     * Wipe every memory, history entry, entity, and session message for the current tenant.
     *
     * <p>Destructive and irreversible — callers must enforce service-role authorization at the
     * boundary (e.g. {@code @RequireServiceRole} on the controller).
     */
    @Transactional(transactionManager = MEM_TX_MANAGER)
    public void reset() {
        memoryRepository.truncate();
        memoryHistoryRepository.truncate();
        sessionMessageRepository.truncate();
        entityRepository.truncate();
        log.warn("All memory data (including entities) truncated for current tenant");
    }

    // ---------- helpers ----------

    static String sessionScope(UUID userId, String agentId, String runId) {
        StringBuilder sb = new StringBuilder();
        if (userId != null) {
            sb.append("user:").append(userId);
        }
        if (agentId != null) {
            if (sb.length() > 0) sb.append('|');
            sb.append("agent:").append(agentId);
        }
        if (runId != null) {
            if (sb.length() > 0) sb.append('|');
            sb.append("run:").append(runId);
        }
        return sb.length() == 0 ? null : sb.toString();
    }

    static void validateOwner(UUID userId, String agentId, String runId) {
        if (userId == null && (agentId == null || agentId.isBlank()) && (runId == null || runId.isBlank())) {
            throw new IllegalArgumentException(
                    "At least one of userId / agentId / runId must be provided");
        }
    }

    static UUID parseUuidQuiet(String s) {
        if (s == null || s.isBlank()) {
            return null;
        }
        try {
            return UUID.fromString(s);
        } catch (IllegalArgumentException e) {
            return null;
        }
    }

    static String sha256(String text) {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] digest = md.digest(text.getBytes(StandardCharsets.UTF_8));
            StringBuilder sb = new StringBuilder(digest.length * 2);
            for (byte b : digest) {
                sb.append(String.format("%02x", b));
            }
            return sb.toString();
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256 not available", e);
        }
    }
}
