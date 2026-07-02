package ai.nubase.mem.service;

import ai.nubase.auth.entity.User;
import ai.nubase.auth.exception.ForbiddenException;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.dto.AddMemoryRequest;
import ai.nubase.mem.dto.MemoryEventResponse;
import ai.nubase.mem.dto.SearchMemoryRequest;
import ai.nubase.mem.entity.Memory;
import ai.nubase.mem.entity.MemoryHistory;
import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.repository.EntityRepository;
import ai.nubase.mem.repository.MemoryHistoryRepository;
import ai.nubase.mem.repository.MemoryRepository;
import ai.nubase.mem.repository.SessionMessageRepository;
import ai.nubase.mem.service.FactExtractionService.ExtractedEntity;
import ai.nubase.mem.service.FactExtractionService.FactExtractionResult;
import ai.nubase.mem.service.MemoryDecisionService.Decision;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.ArgumentMatchers.anyList;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

/**
 * Orchestration-level tests for {@link MemoryService}. Repositories and the LLM-facing
 * services are mocked so this runs without a database or network.
 */
class MemoryServiceTest {

    private MemProperties props;
    private MemConfigResolver resolver;
    private EmbeddingService embeddingService;
    private FactExtractionService factExtractionService;
    private MemoryDecisionService memoryDecisionService;
    private QueryEntityExtractionService queryEntityExtractionService;
    private EntityStoreService entityStoreService;
    private MemoryRepository memoryRepository;
    private MemoryHistoryRepository memoryHistoryRepository;
    private SessionMessageRepository sessionMessageRepository;
    private EntityRepository entityRepository;
    private MemoryService svc;

    private static final UUID USER_ID = UUID.fromString("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    private static final UUID MEM_ID_1 = UUID.fromString("11111111-1111-1111-1111-111111111111");
    private static final UUID MEM_ID_2 = UUID.fromString("22222222-2222-2222-2222-222222222222");

    @BeforeEach
    void setUp() {
        props = new MemProperties();
        // Disable session-window writes for most tests to keep mocks minimal.
        props.getSession().setEnabled(false);
        resolver = mock(MemConfigResolver.class);
        when(resolver.sessionEnabled()).thenAnswer(invocation -> props.getSession().isEnabled());
        when(resolver.sessionMaxMessages()).thenAnswer(invocation -> props.getSession().getMaxMessages());
        when(resolver.sessionInjectIntoExtraction()).thenAnswer(invocation -> props.getSession().isInjectIntoExtraction());
        when(resolver.isHistoryEnabled()).thenAnswer(invocation -> props.isHistoryEnabled());
        when(resolver.searchDefaultTopK()).thenAnswer(invocation -> props.getSearch().getDefaultTopK());
        when(resolver.searchDefaultThreshold()).thenAnswer(invocation -> props.getSearch().getDefaultThreshold());
        embeddingService = mock(EmbeddingService.class);
        factExtractionService = mock(FactExtractionService.class);
        memoryDecisionService = mock(MemoryDecisionService.class);
        queryEntityExtractionService = mock(QueryEntityExtractionService.class);
        entityStoreService = mock(EntityStoreService.class);
        memoryRepository = mock(MemoryRepository.class);
        memoryHistoryRepository = mock(MemoryHistoryRepository.class);
        sessionMessageRepository = mock(SessionMessageRepository.class);
        entityRepository = mock(EntityRepository.class);

        // Default: query entity extraction returns empty, entity boosts map empty.
        when(queryEntityExtractionService.extract(org.mockito.ArgumentMatchers.anyString()))
                .thenReturn(List.of());
        when(entityStoreService.computeBoosts(org.mockito.ArgumentMatchers.anyList(),
                org.mockito.ArgumentMatchers.any(), org.mockito.ArgumentMatchers.any(),
                org.mockito.ArgumentMatchers.any()))
                .thenReturn(Map.of());

        svc = new MemoryService(
                props, resolver, embeddingService, factExtractionService, memoryDecisionService,
                queryEntityExtractionService, entityStoreService,
                memoryRepository, memoryHistoryRepository, sessionMessageRepository,
                entityRepository);

        // Default: requests come in as an authenticated user whose JWT sub == USER_ID.
        // Individual tests can override (authenticateAsServiceRole / authenticateAsUser).
        authenticateAsUser(USER_ID);
    }

    @AfterEach
    void tearDown() {
        SecurityContextHolder.clearContext();
        MultiTenancyContext.clear();
    }

    /**
     * Fake the runtime state that {@code MemoryAuthScope.fromContext} reads:
     * a non-service-role tenant context plus a Spring Security principal whose id is the
     * supplied {@code userId}. Mirrors what the production filter chain would do for a
     * Bearer-authenticated request.
     */
    private void authenticateAsUser(UUID userId) {
        SecurityContextHolder.clearContext();
        MultiTenancyContext.clear();

        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder()
                .appCode("test-app")
                .schemaName("public")
                .jwtSecret("test-secret")
                .serviceRole(false)
                .build());

        User user = User.builder().id(userId).email("u@test").build();
        var auth = new UsernamePasswordAuthenticationToken(
                user, null, Collections.singletonList(new SimpleGrantedAuthority("ROLE_AUTHENTICATED")));
        SecurityContextHolder.getContext().setAuthentication(auth);
    }

    /** Fake a service-role apikey request — no Bearer principal needed. */
    private void authenticateAsServiceRole() {
        SecurityContextHolder.clearContext();
        MultiTenancyContext.clear();
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder()
                .appCode("test-app")
                .schemaName("public")
                .jwtSecret("test-secret")
                .serviceRole(true)
                .build());
    }

    /** Fake a misconfigured request: tenant context set but no user authenticated. */
    private void authenticateAsAnonymous() {
        SecurityContextHolder.clearContext();
        MultiTenancyContext.clear();
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder()
                .appCode("test-app")
                .schemaName("public")
                .jwtSecret("test-secret")
                .serviceRole(false)
                .build());
    }

    private static FactExtractionResult facts(List<String> facts, List<List<ExtractedEntity>> entities) {
        return FactExtractionResult.builder().facts(facts).entities(entities).build();
    }

    private static FactExtractionResult factsOnly(String... facts) {
        List<String> f = List.of(facts);
        List<List<ExtractedEntity>> empty = new java.util.ArrayList<>();
        for (int i = 0; i < f.size(); i++) empty.add(List.of());
        return facts(f, empty);
    }

    // ============================================================================
    //  Authorization tests (P0-#1)
    // ============================================================================

    @Test
    void add_rejectsCrossUserSpoofingFromBody() {
        // Authenticated user is USER_ID; body asks for a different userId.
        UUID attackerTarget = UUID.fromString("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(attackerTarget);
        req.setMessages(List.of(ChatMessage.user("malicious")));

        assertThatThrownBy(() -> svc.add(req))
                .isInstanceOf(ForbiddenException.class)
                .hasMessageContaining("another user");
    }

    @Test
    void add_authenticatedUser_injectsJwtUserIdEvenWhenBodyOmitsIt() {
        // Body has no userId; JWT sub == USER_ID; req.userId should be set before downstream calls.
        AddMemoryRequest req = new AddMemoryRequest();
        req.setInfer(false);
        req.setMessages(List.of(ChatMessage.user("hello")));

        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});
        when(memoryRepository.findByHash(any(), any(), any(), anyString())).thenReturn(Optional.empty());
        when(memoryRepository.insert(any(Memory.class))).thenReturn(MEM_ID_1);

        svc.add(req);

        // The insert should carry the JWT user id, not null.
        ArgumentCaptor<Memory> captor = ArgumentCaptor.forClass(Memory.class);
        verify(memoryRepository).insert(captor.capture());
        assertThat(captor.getValue().getUserId()).isEqualTo(USER_ID);
    }

    @Test
    void search_rejectsCrossUserSpoofingFromBody() {
        UUID attackerTarget = UUID.fromString("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        SearchMemoryRequest req = new SearchMemoryRequest();
        req.setUserId(attackerTarget);
        req.setQuery("hi");
        assertThatThrownBy(() -> svc.search(req))
                .isInstanceOf(ForbiddenException.class);
    }

    @Test
    void deleteAll_rejectsCrossUserSpoofing() {
        UUID otherUser = UUID.fromString("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        assertThatThrownBy(() -> svc.deleteAll(otherUser, null, null))
                .isInstanceOf(ForbiddenException.class);
    }

    @Test
    void get_otherUserMemoryReturnsEmpty() {
        // A memory belonging to a different user — service should refuse via findByIdForOwner.
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID))
                .thenReturn(Optional.empty()); // SQL adds AND user_id = ? — no match
        assertThat(svc.get(MEM_ID_1)).isEmpty();
    }

    @Test
    void update_otherUserMemoryReturnsEmpty() {
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID))
                .thenReturn(Optional.empty());
        assertThat(svc.update(MEM_ID_1, "new")).isEmpty();
        verify(memoryRepository, never()).updateContent(any(), anyString(), any(), anyString());
    }

    @Test
    void delete_otherUserMemoryReturnsFalse() {
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID))
                .thenReturn(Optional.empty());
        assertThat(svc.delete(MEM_ID_1)).isFalse();
        verify(memoryRepository, never()).softDelete(any());
    }

    @Test
    void history_otherUserMemoryReturnsEmpty() {
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID))
                .thenReturn(Optional.empty());
        assertThat(svc.history(MEM_ID_1)).isEmpty();
        verify(memoryHistoryRepository, never()).findByMemoryId(any());
    }

    @Test
    void serviceRole_canReachAnyOwnerWithExplicitUserId() {
        // Admin pinning a specific userId — should be allowed, scope.userId == requested.
        authenticateAsServiceRole();
        UUID target = UUID.fromString("ccccccc1-cccc-cccc-cccc-cccccccccccc");

        when(memoryRepository.findByIdForOwner(MEM_ID_1, target))
                .thenReturn(Optional.of(Memory.builder().id(MEM_ID_1).userId(target).memory("x").build()));
        // Even though scope.userId is `target` and not null, the lookup uses it as the filter.

        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(target);
        req.setInfer(false);
        req.setMessages(List.of(ChatMessage.user("admin write")));
        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});
        when(memoryRepository.findByHash(any(), any(), any(), anyString())).thenReturn(Optional.empty());
        when(memoryRepository.insert(any(Memory.class))).thenReturn(MEM_ID_1);

        svc.add(req);

        ArgumentCaptor<Memory> captor = ArgumentCaptor.forClass(Memory.class);
        verify(memoryRepository).insert(captor.capture());
        assertThat(captor.getValue().getUserId()).isEqualTo(target);
    }

    @Test
    void serviceRole_unrestricted_skipsOwnerFilterInLoad() {
        // Admin without specifying userId — service.get(id) calls findByIdForOwner(id, null)
        // which delegates to plain findById internally.
        authenticateAsServiceRole();
        when(memoryRepository.findByIdForOwner(MEM_ID_1, null))
                .thenReturn(Optional.of(Memory.builder().id(MEM_ID_1).userId(USER_ID).memory("x").build()));

        var res = svc.get(MEM_ID_1);

        assertThat(res).isPresent();
        verify(memoryRepository).findByIdForOwner(MEM_ID_1, null);
    }

    // ============================================================================
    //  #5 session window injection
    // ============================================================================

    @Test
    void add_injectsSessionHistoryIntoExtractionWhenEnabled() {
        // Enable the inject path. Session "enabled" must also be true so the read/write
        // codepaths don't short-circuit.
        props.getSession().setEnabled(true);
        props.getSession().setInjectIntoExtraction(true);

        ai.nubase.mem.entity.SessionMessage hist = ai.nubase.mem.entity.SessionMessage.builder()
                .sessionScope("user:" + USER_ID)
                .role("user").content("earlier turn").build();
        when(sessionMessageRepository.findRecent(anyString(), anyInt()))
                .thenReturn(List.of(hist));
        when(factExtractionService.extract(anyList()))
                .thenReturn(FactExtractionResult.empty());

        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("current turn")));
        svc.add(req);

        // Extractor should see history prepended to the current message.
        ArgumentCaptor<List<ChatMessage>> captor = ArgumentCaptor.forClass(List.class);
        verify(factExtractionService).extract(captor.capture());
        List<ChatMessage> sent = captor.getValue();
        assertThat(sent).hasSize(2);
        assertThat(sent.get(0).getContent()).isEqualTo("earlier turn");
        assertThat(sent.get(1).getContent()).isEqualTo("current turn");
    }

    @Test
    void add_doesNotInjectSessionHistoryWhenFlagOff() {
        props.getSession().setEnabled(true);
        props.getSession().setInjectIntoExtraction(false); // explicit

        when(factExtractionService.extract(anyList()))
                .thenReturn(FactExtractionResult.empty());

        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("current turn")));
        svc.add(req);

        // findRecent should not even be called when injection is off — saves a DB round-trip.
        verify(sessionMessageRepository, never()).findRecent(anyString(), anyInt());
    }

    @Test
    void add_persistsSessionAfterExtraction_toAvoidDoubleCount() {
        // When injection is on, persist must run AFTER extract or the current turn would
        // appear in findRecent for itself.
        props.getSession().setEnabled(true);
        props.getSession().setInjectIntoExtraction(true);
        when(sessionMessageRepository.findRecent(anyString(), anyInt())).thenReturn(List.of());
        when(factExtractionService.extract(anyList()))
                .thenReturn(FactExtractionResult.empty());

        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("hi")));
        svc.add(req);

        var inOrder = org.mockito.Mockito.inOrder(factExtractionService, sessionMessageRepository);
        inOrder.verify(factExtractionService).extract(anyList());
        inOrder.verify(sessionMessageRepository).insertWithEviction(any(), anyInt());
    }

    @Test
    void add_sessionHistoryReadFailure_degradesGracefully() {
        props.getSession().setEnabled(true);
        props.getSession().setInjectIntoExtraction(true);
        when(sessionMessageRepository.findRecent(anyString(), anyInt()))
                .thenThrow(new RuntimeException("db down"));
        when(factExtractionService.extract(anyList()))
                .thenReturn(FactExtractionResult.empty());

        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("hi")));

        // Must not throw — falls back to current-messages-only extraction.
        svc.add(req);

        ArgumentCaptor<List<ChatMessage>> captor = ArgumentCaptor.forClass(List.class);
        verify(factExtractionService).extract(captor.capture());
        assertThat(captor.getValue()).hasSize(1);
        assertThat(captor.getValue().get(0).getContent()).isEqualTo("hi");
    }

    // ============================================================================
    //  #7 SKIPPED event (no null elements in response)
    // ============================================================================

    @Test
    void add_unknownDecisionEvent_returnsSkippedNotNull() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("hi")));

        when(factExtractionService.extract(any())).thenReturn(factsOnly("f1"));
        when(embeddingService.embedBatch(any())).thenReturn(List.of(new float[]{1f}));
        when(memoryRepository.searchByVector(any(), any(), any(), any(), anyInt(), any()))
                .thenReturn(List.of());
        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("MAYBE").id("new_0").text("f1").build()
        ));

        var events = svc.add(req);

        assertThat(events).hasSize(1).doesNotContainNull();
        assertThat(events.get(0).getEvent()).isEqualTo("SKIPPED");
        assertThat(events.get(0).getPreviousMemory()).contains("unknown event");
    }

    @Test
    void add_malformedAddDecision_returnsSkippedInsteadOfNull() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("hi")));

        when(factExtractionService.extract(any())).thenReturn(factsOnly("f1"));
        when(embeddingService.embedBatch(any())).thenReturn(List.of(new float[]{1f}));
        when(memoryRepository.searchByVector(any(), any(), any(), any(), anyInt(), any()))
                .thenReturn(List.of());
        // ADD decision with no text → applyAdd returns null → service must wrap as SKIPPED.
        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("ADD").id("new_0").text(null).build()
        ));

        var events = svc.add(req);

        assertThat(events).hasSize(1).doesNotContainNull();
        assertThat(events.get(0).getEvent()).isEqualTo("SKIPPED");
        verify(memoryRepository, never()).insert(any(Memory.class));
    }

    @Test
    void add_updateDecisionTargetingUnauthorizedMemory_returnsSkipped() {
        // LLM hallucinates a UUID owned by a different user. The owner-scoped lookup returns
        // empty → applyUpdate falls back to applyAdd with no text (null text in decision)
        // → returns null → wrapped as SKIPPED.
        UUID hallucinated = UUID.fromString("ffffffff-ffff-ffff-ffff-ffffffffffff");
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("hi")));

        when(factExtractionService.extract(any())).thenReturn(factsOnly("f1"));
        when(embeddingService.embedBatch(any())).thenReturn(List.of(new float[]{1f}));
        when(memoryRepository.searchByVector(any(), any(), any(), any(), anyInt(), any()))
                .thenReturn(List.of());
        when(memoryRepository.findByIdForOwner(hallucinated, USER_ID))
                .thenReturn(Optional.empty()); // not ours
        // applyAdd fallback path: fact text IS available, so the ADD would succeed normally.
        // To exercise the SKIPPED path, give a decision whose text is also null.
        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("UPDATE").id(hallucinated.toString()).text(null).build()
        ));

        var events = svc.add(req);

        assertThat(events).hasSize(1).doesNotContainNull();
        assertThat(events.get(0).getEvent()).isEqualTo("SKIPPED");
        verify(memoryRepository, never()).updateContent(any(), anyString(), any(), anyString());
    }

    // ============================================================================
    //  Paginated list + stats (admin UI)
    // ============================================================================

    @Test
    void listPaged_clampsPageSizeAndPageBounds() {
        // pageSize > 500 is clamped to 500; page < 1 is clamped to 1.
        when(memoryRepository.listByOwnerPaged(eq(USER_ID), any(), any(), any(),
                eq(1), eq(500)))
                .thenReturn(List.of());
        when(memoryRepository.countByOwner(eq(USER_ID), any(), any(), any())).thenReturn(0L);

        var res = svc.listPaged(USER_ID, null, null, null, /*page*/ -3, /*pageSize*/ 9999);

        assertThat(res.getPage()).isEqualTo(1);
        assertThat(res.getPageSize()).isEqualTo(500);
        verify(memoryRepository).listByOwnerPaged(eq(USER_ID), any(), any(), any(),
                eq(1), eq(500));
    }

    @Test
    void listPaged_serviceRoleWithoutOwner_listsWholeTenant() {
        // Admin with all filters null — fromContext (not Require) allows this.
        authenticateAsServiceRole();
        when(memoryRepository.listByOwnerPaged(org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                eq(1), eq(25))).thenReturn(List.of());
        when(memoryRepository.countByOwner(
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull())).thenReturn(0L);

        svc.listPaged(null, null, null, null, 1, 25);

        verify(memoryRepository).listByOwnerPaged(
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                eq(1), eq(25));
    }

    @Test
    void listPaged_authenticatedUser_alwaysScopedToJwtSub() {
        // Authenticated user with null params — JWT user id gets injected.
        when(memoryRepository.listByOwnerPaged(eq(USER_ID), any(), any(), any(),
                eq(1), eq(25))).thenReturn(List.of());
        when(memoryRepository.countByOwner(eq(USER_ID), any(), any(), any())).thenReturn(7L);

        var res = svc.listPaged(null, null, null, null, 1, 25);

        assertThat(res.getTotal()).isEqualTo(7L);
        verify(memoryRepository).listByOwnerPaged(eq(USER_ID), any(), any(), any(),
                eq(1), eq(25));
    }

    @Test
    void stats_admin_returnsTopUsersAndTenantWideCounts() {
        authenticateAsServiceRole();

        when(memoryRepository.countByOwner(org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull())).thenReturn(1000L);
        when(entityRepository.countByOwner(org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull(),
                org.mockito.ArgumentMatchers.isNull())).thenReturn(300L);
        when(memoryHistoryRepository.countEventsSince(eq(1440),
                org.mockito.ArgumentMatchers.isNull()))
                .thenReturn(Map.of("ADD", 12L, "UPDATE", 5L, "DELETE", 2L));
        java.util.Map<String, Object> row = new java.util.HashMap<>();
        row.put("user_id", USER_ID);
        row.put("count", 42L);
        when(memoryRepository.topUsersByMemoryCount(5)).thenReturn(List.of(row));

        var stats = svc.stats(null, null, null);

        assertThat(stats.getTotalMemories()).isEqualTo(1000L);
        assertThat(stats.getTotalEntities()).isEqualTo(300L);
        assertThat(stats.getLast24h().getAdd()).isEqualTo(12L);
        assertThat(stats.getLast24h().getUpdate()).isEqualTo(5L);
        assertThat(stats.getLast24h().getDelete()).isEqualTo(2L);
        assertThat(stats.getTopUsers()).hasSize(1);
        assertThat(stats.getTopUsers().get(0).getUserId()).isEqualTo(USER_ID);
        assertThat(stats.getTopUsers().get(0).getCount()).isEqualTo(42L);
    }

    @Test
    void stats_nonAdmin_omitsTopUsersAndScopesToSelf() {
        // Authenticated USER_ID — should NOT see other users' counts.
        when(memoryRepository.countByOwner(eq(USER_ID), any(), any(), any())).thenReturn(7L);
        when(entityRepository.countByOwner(eq(USER_ID), any(), any())).thenReturn(3L);
        when(memoryHistoryRepository.countEventsSince(eq(1440), eq(USER_ID)))
                .thenReturn(Map.of());

        var stats = svc.stats(null, null, null);

        assertThat(stats.getTotalMemories()).isEqualTo(7L);
        assertThat(stats.getTotalEntities()).isEqualTo(3L);
        // missing event types default to 0
        assertThat(stats.getLast24h().getAdd()).isEqualTo(0L);
        assertThat(stats.getTopUsers()).isEmpty();
        // Admin-only query must not have been called.
        verify(memoryRepository, never()).topUsersByMemoryCount(anyInt());
    }

    @Test
    void linkedEntities_ownerCheckedViaLoadByIdForScope() {
        Memory ownedMem = Memory.builder()
                .id(MEM_ID_1).userId(USER_ID).memory("x").build();
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID))
                .thenReturn(Optional.of(ownedMem));
        var ent = ai.nubase.mem.entity.Entity.builder()
                .id(UUID.randomUUID()).text("John").build();
        // entityStoreService is mocked here — stub its facade method directly.
        when(entityStoreService.findLinkedTo(eq(MEM_ID_1), eq(Optional.of(ownedMem))))
                .thenReturn(List.of(ent));

        var res = svc.linkedEntities(MEM_ID_1);

        assertThat(res).hasSize(1);
        assertThat(res.get(0).getText()).isEqualTo("John");
    }

    @Test
    void linkedEntities_unauthorizedMemoryReturnsEmpty() {
        // Memory not owned by USER_ID — findByIdForOwner returns empty, service passes
        // Optional.empty() to findLinkedTo which returns empty per its contract.
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID))
                .thenReturn(Optional.empty());
        when(entityStoreService.findLinkedTo(eq(MEM_ID_1), eq(Optional.empty())))
                .thenReturn(List.of());

        var res = svc.linkedEntities(MEM_ID_1);

        assertThat(res).isEmpty();
    }

    @Test
    void anonymousCall_rejected() {
        // Tenant context present but no JWT principal — must refuse to operate on any
        // userId from the body.
        authenticateAsAnonymous();
        SearchMemoryRequest req = new SearchMemoryRequest();
        req.setQuery("hi");
        assertThatThrownBy(() -> svc.search(req))
                .isInstanceOf(ForbiddenException.class)
                .hasMessageContaining("authenticated user");
    }

    @Test
    void requiresAtLeastOneOwnerId_underServiceRole() {
        // Regular users get userId auto-injected from JWT, so this case only triggers for
        // service_role callers who didn't pin any owner field.
        authenticateAsServiceRole();
        AddMemoryRequest req = new AddMemoryRequest();
        req.setMessages(List.of(ChatMessage.user("hello")));
        assertThatThrownBy(() -> svc.add(req))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("userId");
    }

    @Test
    void add_inferFalse_storesEachMessageVerbatim() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setInfer(false);
        req.setMessages(List.of(
                ChatMessage.system("be polite"),
                ChatMessage.user("I like ramen"),
                ChatMessage.assistant("OK")));

        when(embeddingService.embed(anyString())).thenReturn(new float[]{0.1f, 0.2f});
        when(memoryRepository.findByHash(any(), any(), any(), anyString())).thenReturn(Optional.empty());
        when(memoryRepository.insert(any(Memory.class))).thenReturn(UUID.randomUUID());

        List<MemoryEventResponse> events = svc.add(req);

        // system message excluded; user + assistant -> 2 events
        assertThat(events).hasSize(2);
        assertThat(events).allSatisfy(e -> assertThat(e.getEvent()).isEqualTo("ADD"));
        verify(memoryRepository, times(2)).insert(any(Memory.class));
        verify(factExtractionService, never()).extract(any());
    }

    @Test
    void add_inferTrue_dispatchesAddAndUpdateDecisions() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("My name is John, I love sushi")));

        when(factExtractionService.extract(any()))
                .thenReturn(factsOnly("Name is John", "Loves sushi"));
        when(embeddingService.embedBatch(any()))
                .thenReturn(List.of(new float[]{1f}, new float[]{2f}));
        when(embeddingService.embed(anyString())).thenReturn(new float[]{3f});

        Memory existing = Memory.builder()
                .id(MEM_ID_1)
                .userId(USER_ID)
                .memory("Likes seafood")
                .build();
        when(memoryRepository.searchByVector(any(), eq(USER_ID), any(), any(), anyInt(), any()))
                .thenReturn(List.of(existing));
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID)).thenReturn(Optional.of(existing));
        when(memoryRepository.findByHash(any(), any(), any(), anyString())).thenReturn(Optional.empty());
        when(memoryRepository.insert(any(Memory.class))).thenReturn(UUID.randomUUID());

        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("ADD").id("new_0").text("Name is John").build(),
                Decision.builder().event("UPDATE").id(MEM_ID_1.toString())
                        .text("Loves sushi").oldMemory("Likes seafood").build()
        ));

        List<MemoryEventResponse> events = svc.add(req);

        assertThat(events).hasSize(2);
        assertThat(events).anyMatch(e -> "ADD".equals(e.getEvent()));
        assertThat(events).anyMatch(e -> "UPDATE".equals(e.getEvent())
                && "Loves sushi".equals(e.getMemory())
                && "Likes seafood".equals(e.getPreviousMemory()));

        verify(memoryRepository).insert(any(Memory.class));
        verify(memoryRepository).updateContent(eq(MEM_ID_1), eq("Loves sushi"), any(), anyString());
        verify(memoryHistoryRepository, times(2)).insert(any(MemoryHistory.class));
    }

    @Test
    void add_inferTrue_handlesDeleteAndNone() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("anything")));

        when(factExtractionService.extract(any())).thenReturn(factsOnly("f1"));
        when(embeddingService.embedBatch(any())).thenReturn(List.of(new float[]{1f}));

        Memory toDelete = Memory.builder().id(MEM_ID_1).userId(USER_ID).memory("stale").build();
        Memory toKeep = Memory.builder().id(MEM_ID_2).userId(USER_ID).memory("kept").build();
        when(memoryRepository.searchByVector(any(), any(), any(), any(), anyInt(), any()))
                .thenReturn(List.of(toDelete, toKeep));
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID)).thenReturn(Optional.of(toDelete));
        when(memoryRepository.findByIdForOwner(MEM_ID_2, USER_ID)).thenReturn(Optional.of(toKeep));

        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("DELETE").id(MEM_ID_1.toString()).build(),
                Decision.builder().event("NONE").id(MEM_ID_2.toString()).build()
        ));

        List<MemoryEventResponse> events = svc.add(req);

        assertThat(events).hasSize(2);
        verify(memoryRepository).softDelete(MEM_ID_1);
        verify(memoryRepository, never()).softDelete(MEM_ID_2);
        // history should have one DELETE entry
        ArgumentCaptor<MemoryHistory> captor = ArgumentCaptor.forClass(MemoryHistory.class);
        verify(memoryHistoryRepository).insert(captor.capture());
        assertThat(captor.getValue().getEvent()).isEqualTo("DELETE");
    }

    @Test
    void add_inferTrue_dedupeByHashCollapsesToNone() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("hello")));

        when(factExtractionService.extract(any())).thenReturn(factsOnly("Already-known fact"));
        when(embeddingService.embedBatch(any())).thenReturn(List.of(new float[]{1f}));
        when(memoryRepository.searchByVector(any(), any(), any(), any(), anyInt(), any()))
                .thenReturn(List.of());

        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("ADD").id("new_0").text("Already-known fact").build()
        ));

        Memory dup = Memory.builder().id(MEM_ID_1).userId(USER_ID).memory("Already-known fact").build();
        when(memoryRepository.findByHash(any(), any(), any(), anyString())).thenReturn(Optional.of(dup));

        List<MemoryEventResponse> events = svc.add(req);

        assertThat(events).hasSize(1);
        assertThat(events.get(0).getEvent()).isEqualTo("NONE");
        assertThat(events.get(0).getId()).isEqualTo(MEM_ID_1);
        verify(memoryRepository, never()).insert(any(Memory.class));
    }

    @Test
    void search_fusesVectorAndBm25WithOverFetch() {
        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});

        // Vector returns A (high), B (low). BM25 returns A (high) and a new C.
        Memory semA = Memory.builder().id(MEM_ID_1).userId(USER_ID).memory("vec-A").score(0.1).build();
        Memory semB = Memory.builder().id(MEM_ID_2).userId(USER_ID).memory("vec-B").score(0.2).build();
        when(memoryRepository.searchByVector(any(), eq(USER_ID), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                anyInt(), any()))
                .thenReturn(List.of(semA, semB));

        UUID memC = UUID.fromString("33333333-3333-3333-3333-333333333333");
        Memory bmA = Memory.builder().id(MEM_ID_1).userId(USER_ID).memory("vec-A").score(2.0).build();
        Memory bmC = Memory.builder().id(memC).userId(USER_ID).memory("bm-only").score(0.5).build();
        when(memoryRepository.searchByText(anyString(), eq(USER_ID), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                anyInt()))
                .thenReturn(List.of(bmA, bmC));

        SearchMemoryRequest req = new SearchMemoryRequest();
        req.setQuery("hello");
        req.setUserId(USER_ID);
        req.setTopK(2);
        req.setThreshold(0.5);

        var res = svc.search(req);

        // A (hybrid) should beat B (semantic-only). C is bm25-only and may or may not
        // pass the similarity floor (similarityFloor = 1 - 0.5 = 0.5; C has sim=0) — drops.
        assertThat(res).hasSize(2);
        assertThat(res.get(0).getId()).isEqualTo(MEM_ID_1);
        assertThat(res.get(1).getId()).isEqualTo(MEM_ID_2);

        // Confirm over-fetch was applied (internalLimit = max(topK*4, 60) = 60)
        verify(memoryRepository).searchByVector(any(), eq(USER_ID), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                eq(60), eq(0.5));
        verify(memoryRepository).searchByText(eq("hello"), eq(USER_ID), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                eq(60));
    }

    @Test
    void search_returnsEmptyWhenBothChannelsEmpty() {
        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});
        when(memoryRepository.searchByVector(any(), any(), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                anyInt(), any()))
                .thenReturn(List.of());
        when(memoryRepository.searchByText(anyString(), any(), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                anyInt()))
                .thenReturn(List.of());

        SearchMemoryRequest req = new SearchMemoryRequest();
        req.setQuery("hello");
        req.setUserId(USER_ID);
        assertThat(svc.search(req)).isEmpty();
    }

    @Test
    void search_appliesConfiguredDefaultsWhenUnset() {
        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});
        when(memoryRepository.searchByVector(any(), eq(USER_ID), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                anyInt(), eq(props.getSearch().getDefaultThreshold())))
                .thenReturn(List.of());
        when(memoryRepository.searchByText(anyString(), eq(USER_ID), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                anyInt()))
                .thenReturn(List.of());

        SearchMemoryRequest req = new SearchMemoryRequest();
        req.setQuery("hello");
        req.setUserId(USER_ID);
        svc.search(req);

        // Default topK=5 → internalLimit = max(20, 60) = 60
        verify(memoryRepository).searchByVector(any(), eq(USER_ID), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                eq(60), eq(props.getSearch().getDefaultThreshold()));
        verify(memoryRepository).searchByText(eq("hello"), eq(USER_ID), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                eq(60));
    }

    @Test
    void search_passesMetadataFiltersAndEntityBoostsToFusion() {
        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});

        Memory semA = Memory.builder().id(MEM_ID_1).userId(USER_ID).memory("vec-A").score(0.1).build();
        Map<String, Object> filters = Map.of("category", "food");

        when(memoryRepository.searchByVector(any(), eq(USER_ID), any(), any(),
                eq(filters), anyInt(), any()))
                .thenReturn(List.of(semA));
        when(memoryRepository.searchByText(anyString(), eq(USER_ID), any(), any(),
                eq(filters), anyInt()))
                .thenReturn(List.of());

        // Entity extraction returns one entity; entity store returns a boost for MEM_ID_1.
        when(queryEntityExtractionService.extract(anyString())).thenReturn(List.of(
                ExtractedEntity.builder().text("pizza").type("food").build()
        ));
        when(entityStoreService.computeBoosts(anyList(), eq(USER_ID), any(), any()))
                .thenReturn(Map.of(MEM_ID_1, 0.5));

        SearchMemoryRequest req = new SearchMemoryRequest();
        req.setQuery("pizza");
        req.setUserId(USER_ID);
        req.setMetadataFilters(filters);

        var res = svc.search(req);

        assertThat(res).hasSize(1);
        assertThat(res.get(0).getId()).isEqualTo(MEM_ID_1);
        verify(memoryRepository).searchByVector(any(), eq(USER_ID), any(), any(),
                eq(filters), anyInt(), any());
        verify(entityStoreService).computeBoosts(anyList(), eq(USER_ID), any(), any());
    }

    @Test
    void search_entityBoostFailureDegradesGracefully() {
        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});
        Memory semA = Memory.builder().id(MEM_ID_1).userId(USER_ID).memory("vec-A").score(0.1).build();
        when(memoryRepository.searchByVector(any(), any(), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                anyInt(), any()))
                .thenReturn(List.of(semA));
        when(memoryRepository.searchByText(anyString(), any(), any(), any(),
                org.mockito.ArgumentMatchers.<Map<String, Object>>any(),
                anyInt()))
                .thenReturn(List.of());

        when(queryEntityExtractionService.extract(anyString()))
                .thenThrow(new RuntimeException("boom"));

        SearchMemoryRequest req = new SearchMemoryRequest();
        req.setQuery("hello");
        req.setUserId(USER_ID);
        var res = svc.search(req);

        assertThat(res).hasSize(1); // search still returns
    }

    @Test
    void search_emptyQueryReturnsEmptyWithoutEmbedding() {
        SearchMemoryRequest req = new SearchMemoryRequest();
        req.setUserId(USER_ID);
        req.setQuery(" ");
        var res = svc.search(req);
        assertThat(res).isEmpty();
        verify(embeddingService, never()).embed(anyString());
    }

    @Test
    void update_writesHistoryEntry() {
        Memory existing = Memory.builder()
                .id(MEM_ID_1).userId(USER_ID).agentId("agentA").memory("old").build();
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID)).thenReturn(Optional.of(existing));
        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});

        var res = svc.update(MEM_ID_1, "new");

        assertThat(res).isPresent();
        assertThat(res.get().getEvent()).isEqualTo("UPDATE");
        verify(memoryRepository).updateContent(eq(MEM_ID_1), eq("new"), any(), anyString());
        // P2-12: update() strips stale entity links since the new text may no longer
        // mention the old entities. The caller didn't supply fresh entities, so we
        // just unlink and leave the memory un-boosted.
        verify(entityStoreService).unlinkMemory(MEM_ID_1, USER_ID, "agentA", null);
        ArgumentCaptor<MemoryHistory> captor = ArgumentCaptor.forClass(MemoryHistory.class);
        verify(memoryHistoryRepository).insert(captor.capture());
        assertThat(captor.getValue().getEvent()).isEqualTo("UPDATE");
        assertThat(captor.getValue().getOldValue()).isEqualTo("old");
        assertThat(captor.getValue().getNewValue()).isEqualTo("new");
    }

    @Test
    void add_updateDecision_unlinksOldEntitiesBeforeRelinking() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("I prefer steak now")));

        // One fact, with new entity "steak".
        List<List<ExtractedEntity>> entities = new java.util.ArrayList<>();
        entities.add(List.of(ExtractedEntity.builder().text("steak").type("food").build()));
        when(factExtractionService.extract(any()))
                .thenReturn(facts(List.of("Prefers steak"), entities));
        when(embeddingService.embedBatch(any())).thenReturn(List.of(new float[]{1f}));

        Memory existing = Memory.builder()
                .id(MEM_ID_1).userId(USER_ID).memory("Likes sushi").build();
        when(memoryRepository.searchByVector(any(), eq(USER_ID), any(), any(),
                anyInt(), any())).thenReturn(List.of(existing));
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID)).thenReturn(Optional.of(existing));

        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("UPDATE").id(MEM_ID_1.toString())
                        .text("Prefers steak").oldMemory("Likes sushi").build()
        ));
        when(embeddingService.embed(anyString())).thenReturn(new float[]{2f});

        svc.add(req);

        // The order matters: unlink old, then link new. inOrder() captures this.
        var inOrder = org.mockito.Mockito.inOrder(entityStoreService);
        inOrder.verify(entityStoreService).unlinkMemory(MEM_ID_1, USER_ID, null, null);
        inOrder.verify(entityStoreService).linkEntities(eq(MEM_ID_1), anyList(),
                eq(USER_ID), any(), any());
    }

    @Test
    void add_noneDecision_linksNewEntitiesToExistingMemory() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("I like pizza")));

        // Fact "Likes pizza" with a new entity the existing memory didn't have.
        List<List<ExtractedEntity>> entities = new java.util.ArrayList<>();
        entities.add(List.of(ExtractedEntity.builder().text("pizza").type("food").build()));
        when(factExtractionService.extract(any()))
                .thenReturn(facts(List.of("Likes pizza"), entities));
        when(embeddingService.embedBatch(any())).thenReturn(List.of(new float[]{1f}));

        Memory existing = Memory.builder()
                .id(MEM_ID_1).userId(USER_ID).memory("Likes pizza").build();
        when(memoryRepository.searchByVector(any(), eq(USER_ID), any(), any(),
                anyInt(), any())).thenReturn(List.of(existing));
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID)).thenReturn(Optional.of(existing));

        // LLM echoes the fact text back so the caller can correlate.
        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("NONE").id(MEM_ID_1.toString())
                        .text("Likes pizza").build()
        ));

        svc.add(req);

        // P2-12 #16: NONE branch should now re-link entities (idempotent at the
        // repository layer) — captures any new entities the LLM surfaces on re-ingestion.
        verify(entityStoreService).linkEntities(eq(MEM_ID_1), anyList(),
                eq(USER_ID), any(), any());
    }

    @Test
    void add_noneDecisionWithoutText_skipsEntityRelink() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("anything")));

        when(factExtractionService.extract(any()))
                .thenReturn(facts(List.of("X"), List.of(List.of(
                        ExtractedEntity.builder().text("X").type("other").build()))));
        when(embeddingService.embedBatch(any())).thenReturn(List.of(new float[]{1f}));

        Memory existing = Memory.builder()
                .id(MEM_ID_1).userId(USER_ID).memory("X").build();
        when(memoryRepository.searchByVector(any(), any(), any(), any(),
                anyInt(), any())).thenReturn(List.of(existing));
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID)).thenReturn(Optional.of(existing));

        // Legacy prompt returned NONE without text — we can't correlate, so skip.
        when(memoryDecisionService.decide(any(), any())).thenReturn(List.of(
                Decision.builder().event("NONE").id(MEM_ID_1.toString()).build()
        ));

        svc.add(req);

        verify(entityStoreService, never()).linkEntities(any(), anyList(), any(), any(), any());
    }

    @Test
    void delete_emitsHistoryEntry() {
        Memory existing = Memory.builder().id(MEM_ID_1).userId(USER_ID).memory("stale").build();
        when(memoryRepository.findByIdForOwner(MEM_ID_1, USER_ID)).thenReturn(Optional.of(existing));

        boolean deleted = svc.delete(MEM_ID_1);

        assertThat(deleted).isTrue();
        verify(memoryRepository).softDelete(MEM_ID_1);
        ArgumentCaptor<MemoryHistory> captor = ArgumentCaptor.forClass(MemoryHistory.class);
        verify(memoryHistoryRepository).insert(captor.capture());
        assertThat(captor.getValue().getEvent()).isEqualTo("DELETE");
        assertThat(captor.getValue().getOldValue()).isEqualTo("stale");
    }

    @Test
    void sessionScope_combinesAllPresentFields() {
        String scope = MemoryService.sessionScope(USER_ID, "agentA", "runZ");
        assertThat(scope).isEqualTo("user:" + USER_ID + "|agent:agentA|run:runZ");
    }

    @Test
    void sessionScope_returnsNullWhenAllAbsent() {
        assertThat(MemoryService.sessionScope(null, null, null)).isNull();
    }

    @Test
    void sha256_isDeterministicAndHex() {
        String h1 = MemoryService.sha256("hello");
        String h2 = MemoryService.sha256("hello");
        assertThat(h1).hasSize(64).isEqualTo(h2).matches("^[0-9a-f]+$");
        assertThat(MemoryService.sha256("world")).isNotEqualTo(h1);
    }

    @Test
    void deleteAll_requiresAtLeastOneOwner_underServiceRole() {
        // Same justification as add(): authenticated users always have userId from JWT.
        authenticateAsServiceRole();
        assertThatThrownBy(() -> svc.deleteAll(null, null, null))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("userId");
    }

    @Test
    void deleteAll_cleansEntityLinksThenSoftDeletes() {
        // 1. Select victims first.
        List<UUID> victims = List.of(MEM_ID_1, MEM_ID_2);
        when(memoryRepository.selectIdsByOwner(eq(USER_ID), eq("agentA"),
                org.mockito.ArgumentMatchers.isNull())).thenReturn(victims);
        // 2. Bulk unlink (return value unused).
        // 3. Soft-delete returns the count.
        when(memoryRepository.softDeleteByOwner(eq(USER_ID), eq("agentA"),
                org.mockito.ArgumentMatchers.isNull())).thenReturn(17);

        int n = svc.deleteAll(USER_ID, "agentA", null);

        assertThat(n).isEqualTo(17);
        // Order: select → unlink → softDelete.
        var inOrder = org.mockito.Mockito.inOrder(memoryRepository, entityStoreService);
        inOrder.verify(memoryRepository).selectIdsByOwner(USER_ID, "agentA", null);
        inOrder.verify(entityStoreService).unlinkMemoriesBulk(victims, USER_ID, "agentA", null);
        inOrder.verify(memoryRepository).softDeleteByOwner(USER_ID, "agentA", null);
    }

    @Test
    void deleteAll_zeroVictimsSkipsUnlinkAndSoftDelete() {
        when(memoryRepository.selectIdsByOwner(any(), any(), any())).thenReturn(List.of());

        int n = svc.deleteAll(USER_ID, null, null);

        assertThat(n).isEqualTo(0);
        verify(entityStoreService, never()).unlinkMemoriesBulk(any(), any(), any(), any());
        verify(memoryRepository, never()).softDeleteByOwner(any(), any(), any());
    }

    @Test
    void reset_truncatesAllFourTables() {
        svc.reset();
        verify(memoryRepository).truncate();
        verify(memoryHistoryRepository).truncate();
        verify(sessionMessageRepository).truncate();
        verify(entityRepository).truncate();
    }

    @Test
    void add_inferTrueButNoFactsExtracted_returnsEmpty() {
        AddMemoryRequest req = new AddMemoryRequest();
        req.setUserId(USER_ID);
        req.setMessages(List.of(ChatMessage.user("Hi.")));

        when(factExtractionService.extract(any())).thenReturn(FactExtractionResult.empty());

        var events = svc.add(req);

        assertThat(events).isEmpty();
        verify(memoryDecisionService, never()).decide(any(), any());
        verify(memoryRepository, never()).insert(any(Memory.class));
    }
}
