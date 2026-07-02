package ai.nubase.mem.service;

import ai.nubase.mem.entity.Memory;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.within;

/**
 * Pure-unit tests for the score-fusion math. No I/O, no Spring.
 */
class ScoreFusionTest {

    private static final UUID A = UUID.fromString("11111111-1111-1111-1111-111111111111");
    private static final UUID B = UUID.fromString("22222222-2222-2222-2222-222222222222");
    private static final UUID C = UUID.fromString("33333333-3333-3333-3333-333333333333");

    private static Memory semantic(UUID id, double distance) {
        return Memory.builder().id(id).memory("m-" + id).score(distance).build();
    }

    private static Memory bm25(UUID id, double rank) {
        return Memory.builder().id(id).memory("m-" + id).score(rank).build();
    }

    @Test
    void distanceToSimilarity_clampsAndInverts() {
        assertThat(ScoreFusion.distanceToSimilarity(0.0)).isEqualTo(1.0);
        assertThat(ScoreFusion.distanceToSimilarity(0.3)).isCloseTo(0.7, within(1e-9));
        assertThat(ScoreFusion.distanceToSimilarity(1.0)).isEqualTo(0.0);
        assertThat(ScoreFusion.distanceToSimilarity(1.5)).isEqualTo(0.0);
        assertThat(ScoreFusion.distanceToSimilarity(null)).isEqualTo(0.0);
    }

    @Test
    void normalizeBm25_minMaxScales() {
        Map<UUID, Double> norm = ScoreFusion.normalizeBm25(List.of(
                bm25(A, 0.0),
                bm25(B, 0.5),
                bm25(C, 2.0)
        ));
        assertThat(norm).hasSize(2); // A's zero is dropped
        assertThat(norm.get(B)).isCloseTo(0.25, within(1e-9));
        assertThat(norm.get(C)).isEqualTo(1.0);
    }

    @Test
    void normalizeBm25_emptyOrAllZerosReturnsEmpty() {
        assertThat(ScoreFusion.normalizeBm25(List.of())).isEmpty();
        assertThat(ScoreFusion.normalizeBm25(List.of(bm25(A, 0.0), bm25(B, 0.0)))).isEmpty();
    }

    @Test
    void fuse_semanticOnly_returnsSemanticOrder() {
        List<Memory> sem = List.of(
                semantic(A, 0.1), // sim 0.9
                semantic(B, 0.4)  // sim 0.6
        );
        List<Memory> result = ScoreFusion.fuse(sem, List.of(), Map.of(), 0.0, 10);

        assertThat(result).extracting(Memory::getId).containsExactly(A, B);
        // max_possible = 1.0 (semantic only), so combined == semanticSim
        assertThat(result.get(0).getScore()).isCloseTo(0.9, within(1e-9));
        assertThat(result.get(1).getScore()).isCloseTo(0.6, within(1e-9));
    }

    @Test
    void fuse_semanticPlusBm25_increasesScoreOfHybridHits() {
        // A has strong semantic AND BM25; B has semantic only; C has BM25 only (and no semantic match).
        List<Memory> sem = List.of(
                semantic(A, 0.1), // sim 0.9
                semantic(B, 0.2)  // sim 0.8
        );
        List<Memory> bm = List.of(
                bm25(A, 2.0),   // normalized → 1.0
                bm25(C, 1.0)    // normalized → 0.5
        );

        List<Memory> result = ScoreFusion.fuse(sem, bm, Map.of(), 0.0, 10);

        // max_possible = 2.0
        // A: (0.9 + 1.0) / 2 = 0.95
        // B: (0.8 + 0.0) / 2 = 0.40
        // C: (0.0 + 0.5) / 2 = 0.25 (passes similarity floor 0.0)
        assertThat(result).extracting(Memory::getId).containsExactly(A, B, C);
        assertThat(result.get(0).getScore()).isCloseTo(0.95, within(1e-9));
        assertThat(result.get(1).getScore()).isCloseTo(0.40, within(1e-9));
        assertThat(result.get(2).getScore()).isCloseTo(0.25, within(1e-9));
    }

    @Test
    void fuse_similarityThresholdGatesSemanticBeforeCombining() {
        // B has similarity 0.4 — should be dropped by threshold 0.5 even though BM25 is high.
        List<Memory> sem = List.of(
                semantic(A, 0.1),  // sim 0.9 → passes
                semantic(B, 0.6)   // sim 0.4 → dropped
        );
        List<Memory> bm = List.of(
                bm25(A, 1.0),
                bm25(B, 10.0)
        );

        List<Memory> result = ScoreFusion.fuse(sem, bm, Map.of(), 0.5, 10);

        assertThat(result).extracting(Memory::getId).containsExactly(A);
    }

    @Test
    void fuse_topKBoundsOutput() {
        List<Memory> sem = List.of(
                semantic(A, 0.1),
                semantic(B, 0.2),
                semantic(C, 0.3)
        );
        List<Memory> result = ScoreFusion.fuse(sem, List.of(), Map.of(), 0.0, 2);
        assertThat(result).hasSize(2);
        assertThat(result).extracting(Memory::getId).containsExactly(A, B);
    }

    @Test
    void fuse_entityBoostContributes() {
        List<Memory> sem = List.of(semantic(A, 0.5)); // sim 0.5
        Map<UUID, Double> boosts = Map.of(A, 0.5);

        List<Memory> result = ScoreFusion.fuse(sem, List.of(), boosts, 0.0, 10);

        // max_possible = 1.0 (semantic) + 0.5 (entity weight) = 1.5
        // combined = (0.5 + 0.5) / 1.5 ≈ 0.6667
        assertThat(result).hasSize(1);
        assertThat(result.get(0).getScore()).isCloseTo(1.0 / 1.5, within(1e-9));
    }

    @Test
    void fuse_emptyInputsReturnEmpty() {
        assertThat(ScoreFusion.fuse(null, null, null, 0.0, 5)).isEmpty();
        assertThat(ScoreFusion.fuse(List.of(), List.of(), Map.of(), 0.0, 5)).isEmpty();
    }

    @Test
    void fuse_preservesMemoryPayloadFields() {
        Memory withMeta = Memory.builder()
                .id(A)
                .memory("hello")
                .userId(UUID.randomUUID())
                .agentId("agentX")
                .score(0.1)
                .build();

        List<Memory> result = ScoreFusion.fuse(List.of(withMeta), List.of(), Map.of(), 0.0, 5);

        assertThat(result.get(0).getMemory()).isEqualTo("hello");
        assertThat(result.get(0).getAgentId()).isEqualTo("agentX");
        assertThat(result.get(0).getUserId()).isEqualTo(withMeta.getUserId());
    }
}
