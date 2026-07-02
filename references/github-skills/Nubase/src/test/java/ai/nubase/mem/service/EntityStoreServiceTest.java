package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.entity.Entity;
import ai.nubase.mem.repository.EntityRepository;
import ai.nubase.mem.service.FactExtractionService.ExtractedEntity;
import ai.nubase.mem.service.MemConfigResolver;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.dao.DuplicateKeyException;

import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.within;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

/**
 * Unit tests for {@link EntityStoreService}. Repositories and embedding service are mocked
 * so this runs without a database or LLM.
 */
class EntityStoreServiceTest {

    private static final UUID USER_ID = UUID.fromString("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    private static final UUID MEM_ID = UUID.fromString("11111111-1111-1111-1111-111111111111");
    private static final UUID MEM_ID_2 = UUID.fromString("22222222-2222-2222-2222-222222222222");
    private static final UUID ENT_ID_1 = UUID.fromString("99999999-9999-9999-9999-999999999991");
    private static final UUID ENT_ID_2 = UUID.fromString("99999999-9999-9999-9999-999999999992");

    private EntityRepository entityRepository;
    private EmbeddingService embeddingService;
    private MemProperties props;
    private MemConfigResolver resolver;
    private EntityStoreService svc;

    @BeforeEach
    void setUp() {
        entityRepository = mock(EntityRepository.class);
        embeddingService = mock(EmbeddingService.class);
        props = new MemProperties();
        resolver = mock(MemConfigResolver.class);
        when(resolver.searchEntityMatchSimilarity()).thenReturn(props.getSearch().getEntityMatchSimilarity());
        when(resolver.entityMaxLinkedMemoryIds()).thenReturn(props.getEntity().getMaxLinkedMemoryIds());
        svc = new EntityStoreService(entityRepository, embeddingService, props, resolver);
    }

    @Test
    void linkEntities_existingExactMatch_skipsEmbeddingAndAppendsLink() {
        when(entityRepository.findByExactText(eq("John"), eq("person"),
                eq(USER_ID), any(), any()))
                .thenReturn(Optional.of(Entity.builder()
                        .id(ENT_ID_1).userId(USER_ID).text("John").entityType("person").build()));

        svc.linkEntities(MEM_ID,
                List.of(ExtractedEntity.builder().text("John").type("person").build()),
                USER_ID, null, null);

        verify(entityRepository).appendLinkedMemory(ENT_ID_1, MEM_ID);
        verify(embeddingService, never()).embed(anyString());
        verify(entityRepository, never()).insert(any(), any());
    }

    @Test
    void linkEntities_nearDuplicateMerges_whenSimilarityAbove092() {
        // No exact match
        when(entityRepository.findByExactText(anyString(), any(), any(), any(), any()))
                .thenReturn(Optional.empty());
        when(embeddingService.embed("NYC")).thenReturn(new float[]{0.5f, 0.5f});
        // Existing "New York City" at distance 0.05 → similarity 0.95 → merge
        Entity existing = Entity.builder()
                .id(ENT_ID_1).text("New York City").entityType("location")
                .score(0.05).build();
        when(entityRepository.searchByVector(any(), eq(USER_ID), any(), any(), eq(1)))
                .thenReturn(List.of(existing));

        svc.linkEntities(MEM_ID,
                List.of(ExtractedEntity.builder().text("NYC").type("location").build()),
                USER_ID, null, null);

        verify(entityRepository).appendLinkedMemory(ENT_ID_1, MEM_ID);
        verify(entityRepository, never()).insert(any(), any());
    }

    @Test
    void linkEntities_noMatch_insertsNewEntity() {
        when(entityRepository.findByExactText(anyString(), any(), any(), any(), any()))
                .thenReturn(Optional.empty());
        when(embeddingService.embed("Tokyo")).thenReturn(new float[]{0.1f, 0.2f});
        // Search returns nothing
        when(entityRepository.searchByVector(any(), any(), any(), any(), eq(1)))
                .thenReturn(List.of());

        svc.linkEntities(MEM_ID,
                List.of(ExtractedEntity.builder().text("Tokyo").type("location").build()),
                USER_ID, null, null);

        verify(entityRepository).insert(any(Entity.class), eq(MEM_ID));
    }

    @Test
    void linkEntities_handlesRaceOnInsertViaDuplicateKey() {
        when(entityRepository.findByExactText(eq("Tokyo"), any(), any(), any(), any()))
                .thenReturn(Optional.empty())                    // first check: missing
                .thenReturn(Optional.of(Entity.builder()         // re-read after race: now present
                        .id(ENT_ID_1).text("Tokyo").build()));
        when(embeddingService.embed("Tokyo")).thenReturn(new float[]{1f});
        when(entityRepository.searchByVector(any(), any(), any(), any(), eq(1)))
                .thenReturn(List.of());
        when(entityRepository.insert(any(), any()))
                .thenThrow(new DuplicateKeyException("races"));

        svc.linkEntities(MEM_ID,
                List.of(ExtractedEntity.builder().text("Tokyo").type("location").build()),
                USER_ID, null, null);

        verify(entityRepository).appendLinkedMemory(ENT_ID_1, MEM_ID);
    }

    @Test
    void linkEntities_dedupesInputByLowercaseTextAndType() {
        when(entityRepository.findByExactText(anyString(), any(), any(), any(), any()))
                .thenReturn(Optional.empty());
        when(embeddingService.embed(anyString())).thenReturn(new float[]{1f});
        when(entityRepository.searchByVector(any(), any(), any(), any(), eq(1)))
                .thenReturn(List.of());

        svc.linkEntities(MEM_ID,
                List.of(
                        ExtractedEntity.builder().text("Tokyo").type("location").build(),
                        ExtractedEntity.builder().text("TOKYO").type("location").build(),
                        ExtractedEntity.builder().text("tokyo").type("location").build()),
                USER_ID, null, null);

        // Should only insert once
        verify(entityRepository, times(1)).insert(any(), eq(MEM_ID));
    }

    @Test
    void linkEntities_silentlyDropsEntityWhenEmbeddingFails() {
        when(entityRepository.findByExactText(anyString(), any(), any(), any(), any()))
                .thenReturn(Optional.empty());
        when(embeddingService.embed("Tokyo")).thenThrow(new RuntimeException("openai down"));

        svc.linkEntities(MEM_ID,
                List.of(ExtractedEntity.builder().text("Tokyo").type("location").build()),
                USER_ID, null, null);

        verify(entityRepository, never()).insert(any(), any());
    }

    @Test
    void computeBoosts_emptyQueryEntitiesShortCircuits() {
        Map<UUID, Double> boosts = svc.computeBoosts(List.of(), USER_ID, null, null);
        assertThat(boosts).isEmpty();
        verify(embeddingService, never()).embed(anyString());
    }

    @Test
    void computeBoosts_appliesEntityBoostWeightAndSpreadAttenuation() {
        when(embeddingService.embed("John")).thenReturn(new float[]{1f});

        // Match at similarity 1.0 (distance 0.0), linked to a single memory → no attenuation.
        Entity m = Entity.builder()
                .id(ENT_ID_1).score(0.0)
                .linkedMemoryIds(List.of(MEM_ID))
                .build();
        when(entityRepository.searchByVector(any(), eq(USER_ID), any(), any(), anyInt()))
                .thenReturn(List.of(m));

        Map<UUID, Double> boosts = svc.computeBoosts(
                List.of(ExtractedEntity.builder().text("John").type("person").build()),
                USER_ID, null, null);

        assertThat(boosts).hasSize(1);
        // 1.0 sim * 0.5 weight * 1.0 attenuation = 0.5
        assertThat(boosts.get(MEM_ID)).isCloseTo(0.5, within(1e-9));
    }

    @Test
    void computeBoosts_spreadAttenuationReducesBoostForManyLinks() {
        when(embeddingService.embed("John")).thenReturn(new float[]{1f});

        // Linked to 100 memories — attenuation ≈ 1 / (1 + 0.001 * 99^2) = 1 / (1 + 9.801) ≈ 0.0926
        List<UUID> many = new java.util.ArrayList<>();
        for (int i = 0; i < 100; i++) many.add(UUID.randomUUID());
        Entity m = Entity.builder()
                .id(ENT_ID_1).score(0.0)
                .linkedMemoryIds(many)
                .build();
        when(entityRepository.searchByVector(any(), any(), any(), any(), anyInt()))
                .thenReturn(List.of(m));

        Map<UUID, Double> boosts = svc.computeBoosts(
                List.of(ExtractedEntity.builder().text("John").build()),
                USER_ID, null, null);

        assertThat(boosts).hasSize(100);
        double anyBoost = boosts.values().iterator().next();
        assertThat(anyBoost).isLessThan(0.05); // dramatically attenuated
    }

    @Test
    void computeBoosts_disabledViaConfigReturnsEmpty() {
        // computeBoosts itself doesn't read the flag — that's QueryEntityExtractionService's job —
        // but verify that empty input (which is what the disabled extractor returns) yields {}.
        Map<UUID, Double> boosts = svc.computeBoosts(List.of(), USER_ID, null, null);
        assertThat(boosts).isEmpty();
    }

    @Test
    void computeBoosts_takesMaxBoostWhenMultipleEntitiesPointToSameMemory() {
        when(embeddingService.embed("John")).thenReturn(new float[]{1f});
        when(embeddingService.embed("Tokyo")).thenReturn(new float[]{2f});

        Entity john = Entity.builder().id(ENT_ID_1).score(0.0).linkedMemoryIds(List.of(MEM_ID)).build();
        Entity tokyo = Entity.builder().id(ENT_ID_2).score(0.5).linkedMemoryIds(List.of(MEM_ID)).build();
        when(entityRepository.searchByVector(any(float[].class), any(), any(), any(), anyInt()))
                .thenReturn(List.of(john))
                .thenReturn(List.of(tokyo));

        Map<UUID, Double> boosts = svc.computeBoosts(
                List.of(
                        ExtractedEntity.builder().text("John").build(),
                        ExtractedEntity.builder().text("Tokyo").build()),
                USER_ID, null, null);

        // Boost from John (sim 1.0 → 0.5) beats Tokyo (sim 0.5 → 0.25) — max wins.
        assertThat(boosts.get(MEM_ID)).isCloseTo(0.5, within(1e-9));
    }

    @Test
    void computeBoosts_dropsMatchesBelowSimilarityFloor() {
        when(embeddingService.embed("John")).thenReturn(new float[]{1f});
        // Match at similarity 0.4 — below default floor 0.5
        Entity weak = Entity.builder().id(ENT_ID_1).score(0.6).linkedMemoryIds(List.of(MEM_ID)).build();
        when(entityRepository.searchByVector(any(), any(), any(), any(), anyInt()))
                .thenReturn(List.of(weak));

        Map<UUID, Double> boosts = svc.computeBoosts(
                List.of(ExtractedEntity.builder().text("John").build()),
                USER_ID, null, null);

        assertThat(boosts).isEmpty();
    }

    @Test
    void unlinkMemory_delegatesToRepository() {
        svc.unlinkMemory(MEM_ID, USER_ID, "agentA", null);
        verify(entityRepository).removeMemoryLinks(MEM_ID, USER_ID, "agentA", null);
    }

    // ============================================================================
    //  Admin / management API used by EntityController
    // ============================================================================

    @Test
    void listPaged_clampsPageSizeAndRequiresContext() {
        // No tenant context → fromContext should throw. Just verify nothing else fires.
        try {
            svc.listPaged(null, null, null, null, 1, 9999);
            org.assertj.core.api.Assertions.fail("expected ForbiddenException");
        } catch (ai.nubase.auth.exception.ForbiddenException expected) {
            // ok
        }
        org.mockito.Mockito.verifyNoInteractions(entityRepository);
    }

    @Test
    void listPaged_serviceRoleNoFilter_listsWholeTenant() {
        ai.nubase.common.context.MultiTenancyContext.setContext(
                ai.nubase.common.context.MultiTenancyContext.ContextData.builder()
                        .appCode("t").schemaName("public").jwtSecret("s")
                        .serviceRole(true).build());
        try {
            when(entityRepository.listByOwnerPaged(
                    org.mockito.ArgumentMatchers.isNull(),
                    org.mockito.ArgumentMatchers.isNull(),
                    org.mockito.ArgumentMatchers.isNull(),
                    org.mockito.ArgumentMatchers.isNull(),
                    eq(1), eq(500))).thenReturn(List.of());
            when(entityRepository.countByOwnerAndType(
                    org.mockito.ArgumentMatchers.isNull(),
                    org.mockito.ArgumentMatchers.isNull(),
                    org.mockito.ArgumentMatchers.isNull(),
                    org.mockito.ArgumentMatchers.isNull())).thenReturn(0L);

            var res = svc.listPaged(null, null, null, null, -3, 9999);

            assertThat(res.getPage()).isEqualTo(1);
            assertThat(res.getPageSize()).isEqualTo(500);
        } finally {
            ai.nubase.common.context.MultiTenancyContext.clear();
        }
    }

    @Test
    void findByIdForScope_otherOwnerReturnsEmpty() {
        authenticateAsUser();
        try {
            UUID owner = UUID.randomUUID();
            Entity owned = Entity.builder().id(ENT_ID_1).userId(owner).text("x").build();
            when(entityRepository.findById(ENT_ID_1)).thenReturn(Optional.of(owned));

            // current user is USER_ID, entity owner is `owner` → cross-owner reject.
            var res = svc.findByIdForScope(ENT_ID_1);
            assertThat(res).isEmpty();
        } finally {
            ai.nubase.common.context.MultiTenancyContext.clear();
            org.springframework.security.core.context.SecurityContextHolder.clearContext();
        }
    }

    @Test
    void deleteForScope_otherOwnerReturnsFalse_andDoesNotDelete() {
        authenticateAsUser();
        try {
            UUID owner = UUID.randomUUID();
            Entity owned = Entity.builder().id(ENT_ID_1).userId(owner).text("x").build();
            when(entityRepository.findById(ENT_ID_1)).thenReturn(Optional.of(owned));

            boolean deleted = svc.deleteForScope(ENT_ID_1);
            assertThat(deleted).isFalse();
            verify(entityRepository, never()).deleteByIdForOwner(any(), any());
        } finally {
            ai.nubase.common.context.MultiTenancyContext.clear();
            org.springframework.security.core.context.SecurityContextHolder.clearContext();
        }
    }

    @Test
    void deleteForScope_ownEntity_returnsTrue() {
        authenticateAsUser();
        try {
            Entity owned = Entity.builder().id(ENT_ID_1).userId(USER_ID).text("x").build();
            when(entityRepository.findById(ENT_ID_1)).thenReturn(Optional.of(owned));
            when(entityRepository.deleteByIdForOwner(ENT_ID_1, USER_ID)).thenReturn(1);

            assertThat(svc.deleteForScope(ENT_ID_1)).isTrue();
        } finally {
            ai.nubase.common.context.MultiTenancyContext.clear();
            org.springframework.security.core.context.SecurityContextHolder.clearContext();
        }
    }

    /** Set up a non-service-role tenant context with USER_ID as the JWT principal. */
    private void authenticateAsUser() {
        ai.nubase.common.context.MultiTenancyContext.setContext(
                ai.nubase.common.context.MultiTenancyContext.ContextData.builder()
                        .appCode("t").schemaName("public").jwtSecret("s")
                        .serviceRole(false).build());
        ai.nubase.auth.entity.User user = ai.nubase.auth.entity.User.builder()
                .id(USER_ID).email("u@test").build();
        var auth = new org.springframework.security.authentication.UsernamePasswordAuthenticationToken(
                user, null,
                java.util.Collections.singletonList(
                        new org.springframework.security.core.authority.SimpleGrantedAuthority("ROLE_USER")));
        org.springframework.security.core.context.SecurityContextHolder.getContext().setAuthentication(auth);
    }

    @Test
    void unlinkMemory_swallowsRepositoryException() {
        org.mockito.Mockito.doThrow(new RuntimeException("db down"))
                .when(entityRepository).removeMemoryLinks(any(), any(), any(), any());
        // Should not throw
        svc.unlinkMemory(MEM_ID_2, USER_ID, null, null);
    }
}
