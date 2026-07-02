package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.entity.Entity;
import ai.nubase.mem.repository.EntityRepository;
import ai.nubase.mem.service.FactExtractionService.ExtractedEntity;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.dao.DuplicateKeyException;
import org.springframework.stereotype.Service;

import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * Owns reads/writes against {@code mem.entities} and the search-time entity-boost computation.
 *
 * <p>Write path (called by {@code MemoryService.add}):
 * <ol>
 *   <li>For each extracted entity, look for a near-duplicate in the entity store
 *       (case-insensitive text match first, then vector similarity).</li>
 *   <li>If found, append {@code memoryId} to its {@code linked_memory_ids}.</li>
 *   <li>If not, insert a fresh entity row with the embedding and initial link.</li>
 * </ol>
 *
 * <p>Read path (called by {@code MemoryService.search}):
 * <ol>
 *   <li>For each query entity, vector-search the store within the owner scope.</li>
 *   <li>For every match above the similarity threshold, distribute a spread-attenuated
 *       boost to each linked memory (see {@link #SPREAD_DECAY}).</li>
 *   <li>Return the per-memory maximum boost, capped at {@link ScoreFusion#ENTITY_BOOST_WEIGHT}.</li>
 * </ol>
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class EntityStoreService {

    /**
     * Spread-attenuation factor — entities linking to many memories give each memory a smaller
     * share of the boost. Matches mem0's {@code 1 / (1 + 0.001 * (n-1)^2)} curve.
     */
    static final double SPREAD_DECAY = 0.001;

    /** Number of entity-store neighbors examined per query entity. */
    static final int ENTITY_NEIGHBORS = 50;

    /** Hard cap on the number of query entities considered (mem0 uses 8). */
    static final int MAX_QUERY_ENTITIES = 8;

    private final EntityRepository entityRepository;
    private final EmbeddingService embeddingService;
    private final MemProperties memProperties;
    private final MemConfigResolver memConfig;

    /**
     * Link every entity extracted from a fact to {@code memoryId}. Best-effort —
     * individual failures are logged and swallowed so they cannot break memory ingestion.
     */
    public void linkEntities(UUID memoryId,
                             List<ExtractedEntity> entities,
                             UUID userId, String agentId, String runId) {
        if (memoryId == null || entities == null || entities.isEmpty()) {
            return;
        }
        // Deduplicate within the input by (lower(text), type) so we don't link the same memory twice.
        Map<String, ExtractedEntity> dedup = new LinkedHashMap<>();
        for (ExtractedEntity e : entities) {
            if (e.getText() == null || e.getText().isBlank()) continue;
            String key = e.getText().toLowerCase() + "::" + (e.getType() == null ? "" : e.getType());
            dedup.putIfAbsent(key, e);
        }

        for (ExtractedEntity e : dedup.values()) {
            try {
                upsertAndLink(e, memoryId, userId, agentId, runId);
            } catch (Exception ex) {
                log.warn("Failed to link entity '{}' to memory {}: {}",
                        e.getText(), memoryId, ex.getMessage());
            }
        }
    }

    /**
     * Idempotent: link {@code memoryId} to the matching entity, creating it if necessary.
     */
    private void upsertAndLink(ExtractedEntity e, UUID memoryId,
                               UUID userId, String agentId, String runId) {
        // 1. Fast path — exact (case-insensitive) match within owner scope.
        Optional<Entity> exact = entityRepository.findByExactText(
                e.getText(), e.getType(), userId, agentId, runId);
        if (exact.isPresent()) {
            entityRepository.appendLinkedMemory(exact.get().getId(), memoryId);
            return;
        }

        // 2. Embed (if upstream is unavailable, fall back to literal-only — no entity row).
        float[] embedding;
        try {
            embedding = embeddingService.embed(e.getText());
        } catch (Exception ex) {
            log.warn("Skipping entity '{}' — embedding unavailable: {}", e.getText(), ex.getMessage());
            return;
        }

        // 3. Vector dedupe — same-meaning entity ("NYC" / "New York City") merges.
        List<Entity> near = entityRepository.searchByVector(
                embedding, userId, agentId, runId, 1);
        if (!near.isEmpty()) {
            Entity n = near.get(0);
            double sim = ScoreFusion.distanceToSimilarity(n.getScore());
            if (sim >= 0.92) {
                entityRepository.appendLinkedMemory(n.getId(), memoryId);
                return;
            }
        }

        // 4. New entity — insert. Race-safe via the unique index on (text, type, owner).
        try {
            entityRepository.insert(
                    Entity.builder()
                            .userId(userId)
                            .agentId(agentId)
                            .runId(runId)
                            .text(e.getText())
                            .entityType(e.getType())
                            .embedding(embedding)
                            .build(),
                    memoryId);
        } catch (DuplicateKeyException duplicate) {
            // Someone else inserted the same row between our check and insert — link to it.
            Optional<Entity> reread = entityRepository.findByExactText(
                    e.getText(), e.getType(), userId, agentId, runId);
            reread.ifPresent(entity -> entityRepository.appendLinkedMemory(entity.getId(), memoryId));
        }
    }

    /**
     * Compute per-memory boosts for the query's entities.
     *
     * <p>Empty input or disabled extraction → empty map (no boost applied).
     *
     * @return {@code Map<memoryId, boost>} where {@code boost ∈ (0, ENTITY_BOOST_WEIGHT]}.
     */
    public Map<UUID, Double> computeBoosts(List<ExtractedEntity> queryEntities,
                                           UUID userId, String agentId, String runId) {
        if (queryEntities == null || queryEntities.isEmpty()) {
            return Map.of();
        }
        // Deduplicate and truncate to MAX_QUERY_ENTITIES (text-keyed, case-insensitive).
        Map<String, ExtractedEntity> dedup = new LinkedHashMap<>();
        for (ExtractedEntity e : queryEntities) {
            if (e.getText() == null || e.getText().isBlank()) continue;
            String key = e.getText().toLowerCase();
            dedup.putIfAbsent(key, e);
            if (dedup.size() >= MAX_QUERY_ENTITIES) break;
        }
        if (dedup.isEmpty()) {
            return Map.of();
        }

        double matchFloor = memConfig.searchEntityMatchSimilarity();
        Map<UUID, Double> boosts = new HashMap<>();

        for (ExtractedEntity qe : dedup.values()) {
            float[] qVec;
            try {
                qVec = embeddingService.embed(qe.getText());
            } catch (Exception ex) {
                log.warn("Skipping boost for query entity '{}': {}", qe.getText(), ex.getMessage());
                continue;
            }
            List<Entity> matches;
            try {
                matches = entityRepository.searchByVector(qVec, userId, agentId, runId, ENTITY_NEIGHBORS);
            } catch (Exception ex) {
                log.warn("Entity store search failed for '{}': {}", qe.getText(), ex.getMessage());
                continue;
            }

            for (Entity m : matches) {
                double sim = ScoreFusion.distanceToSimilarity(m.getScore());
                if (sim < matchFloor) continue;

                List<UUID> linked = m.getLinkedMemoryIds();
                if (linked == null || linked.isEmpty()) continue;

                int n = linked.size();
                double attenuation = 1.0 / (1.0 + SPREAD_DECAY * Math.pow(n - 1, 2));
                double boost = sim * ScoreFusion.ENTITY_BOOST_WEIGHT * attenuation;

                for (UUID memId : linked) {
                    if (memId == null) continue;
                    boosts.merge(memId, boost, Math::max);
                }
            }
        }
        return boosts;
    }

    /** Remove a memory id from all entity links (called on single-memory delete). */
    public void unlinkMemory(UUID memoryId, UUID userId, String agentId, String runId) {
        if (memoryId == null) return;
        try {
            entityRepository.removeMemoryLinks(memoryId, userId, agentId, runId);
        } catch (Exception ex) {
            log.warn("Failed to remove entity links for memory {}: {}", memoryId, ex.getMessage());
        }
    }

    /**
     * Bulk variant — strip every reference to the supplied memory ids from every entity
     * in the owner scope, in one round-trip. Used by {@code deleteAll}.
     */
    public void unlinkMemoriesBulk(java.util.List<UUID> memoryIds,
                                   UUID userId, String agentId, String runId) {
        if (memoryIds == null || memoryIds.isEmpty()) return;
        try {
            int touched = entityRepository.removeMemoryLinksBulk(memoryIds, userId, agentId, runId);
            log.info("Bulk-unlinked {} memory ids across {} entity rows", memoryIds.size(), touched);
        } catch (Exception ex) {
            log.warn("Bulk entity unlink failed for {} ids: {}", memoryIds.size(), ex.getMessage());
        }
    }

    // ---------- admin / management API (used by /mem/v1/entities/*) ----------

    /**
     * Paginated list of entities for the management UI.
     *
     * <p>Scope behavior matches the rest of the module: service_role with no owner filter
     * sees everything; authenticated users only see entities they own.
     */
    public ai.nubase.mem.dto.PagedResponse<ai.nubase.mem.entity.Entity> listPaged(
            UUID userId, String agentId, String runId, String entityType,
            int page, int pageSize) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(userId, agentId, runId);
        int safePage = Math.max(1, page);
        int safeSize = Math.max(1, Math.min(500, pageSize));

        var items = entityRepository.listByOwnerPaged(
                scope.getUserId(), scope.getAgentId(), scope.getRunId(),
                entityType, safePage, safeSize);
        long total = entityRepository.countByOwnerAndType(
                scope.getUserId(), scope.getAgentId(), scope.getRunId(), entityType);
        return ai.nubase.mem.dto.PagedResponse.of(items, total, safePage, safeSize);
    }

    /**
     * Find a single entity by id, gated by the current caller's scope.
     * Returns empty for "not found" OR "exists but the caller doesn't own it" — same
     * existence-non-leak pattern as memories.
     */
    public java.util.Optional<ai.nubase.mem.entity.Entity> findByIdForScope(UUID id) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(null, null, null);
        var ent = entityRepository.findById(id);
        if (ent.isEmpty()) return java.util.Optional.empty();
        if (!scope.canAccess(ent.get().getUserId())) {
            log.warn("Cross-owner entity access denied: scope={} entity_owner={}",
                    scope.getUserId(), ent.get().getUserId());
            return java.util.Optional.empty();
        }
        return ent;
    }

    /**
     * Hard-delete an entity. Returns true iff a row was actually removed (so the controller
     * can map "wrong owner" → 404 without leaking existence).
     */
    public boolean deleteForScope(UUID id) {
        MemoryAuthScope scope = MemoryAuthScope.fromContext(null, null, null);
        // First lookup so we can refuse cross-owner without leaking via row count.
        var ent = entityRepository.findById(id);
        if (ent.isEmpty() || !scope.canAccess(ent.get().getUserId())) {
            return false;
        }
        UUID ownerFilter = scope.isUnrestricted() ? null : scope.getUserId();
        int n = entityRepository.deleteByIdForOwner(id, ownerFilter);
        return n > 0;
    }

    /**
     * List entities mentioning {@code memoryId}. The memory must first be visible to the
     * caller — otherwise we return empty (don't leak the memory's existence).
     */
    public java.util.List<ai.nubase.mem.entity.Entity> findLinkedTo(
            UUID memoryId, java.util.Optional<ai.nubase.mem.entity.Memory> ownedMemory) {
        if (ownedMemory.isEmpty()) return java.util.List.of();
        MemoryAuthScope scope = MemoryAuthScope.fromContext(null, null, null);
        UUID ownerFilter = scope.isUnrestricted() ? null : scope.getUserId();
        return entityRepository.findByLinkedMemoryId(memoryId, ownerFilter);
    }
}
