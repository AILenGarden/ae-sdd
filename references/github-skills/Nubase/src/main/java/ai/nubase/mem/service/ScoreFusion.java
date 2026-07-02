package ai.nubase.mem.service;

import ai.nubase.mem.entity.Memory;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Multi-signal score fusion for memory retrieval.
 *
 * <p>Mirrors the contract of mem0's {@code mem0/utils/scoring.py#score_and_rank}:
 * <pre>
 *   combined = (semantic_sim + bm25_norm + entity_boost) / max_possible
 * </pre>
 *
 * <ul>
 *   <li>{@code semantic_sim} ∈ [0, 1] — converted from pgvector cosine distance via
 *       {@code 1 - distance} (clamped to {@code [0, 1]}).</li>
 *   <li>{@code bm25_norm} ∈ [0, 1] — min-max normalized within the result set.</li>
 *   <li>{@code entity_boost} — reserved for a future entity-link signal (Batch B).</li>
 * </ul>
 *
 * <p>The semantic threshold gates candidates <em>before</em> combining — a row with a poor
 * semantic match cannot be rescued by BM25 alone.
 */
public final class ScoreFusion {

    /** Weight of the entity-boost contribution; matches mem0's {@code ENTITY_BOOST_WEIGHT = 0.5}. */
    public static final double ENTITY_BOOST_WEIGHT = 0.5;

    private ScoreFusion() {}

    /**
     * Fuse semantic results with BM25 scores and (optional) entity boosts; return top-K.
     *
     * @param semanticResults memories from {@code searchByVector}; their {@link Memory#getScore()}
     *                        holds the raw cosine <em>distance</em> from pgvector
     * @param bm25Results memories from {@code searchByText}; their {@link Memory#getScore()} holds
     *                    the raw {@code ts_rank_cd} score
     * @param entityBoosts optional per-memory boost in {@code [0, 0.5]}, keyed by memory id;
     *                     pass an empty map when entity linking is unavailable
     * @param similarityThreshold candidates with semantic similarity below this are dropped
     * @param topK upper bound on returned rows
     * @return memories ordered by fused score descending, with {@link Memory#getScore()} replaced
     *         by the fused score
     */
    public static List<Memory> fuse(List<Memory> semanticResults,
                                    List<Memory> bm25Results,
                                    Map<UUID, Double> entityBoosts,
                                    double similarityThreshold,
                                    int topK) {
        if (semanticResults == null) semanticResults = List.of();
        if (bm25Results == null) bm25Results = List.of();
        if (entityBoosts == null) entityBoosts = Map.of();

        Map<UUID, Double> bm25Norm = normalizeBm25(bm25Results);

        boolean hasBm25 = !bm25Norm.isEmpty();
        boolean hasEntity = !entityBoosts.isEmpty();
        double maxPossible = 1.0
                + (hasBm25 ? 1.0 : 0.0)
                + (hasEntity ? ENTITY_BOOST_WEIGHT : 0.0);

        // Index semantic results to preserve their payload fields (memory text, owner ids, …).
        Map<UUID, Memory> bySemantic = new LinkedHashMap<>();
        for (Memory m : semanticResults) {
            if (m.getId() == null) continue;
            bySemantic.putIfAbsent(m.getId(), m);
        }

        // Promote BM25-only hits: if a candidate is only in BM25, we still want it scorable.
        // Mem0 gates on semantic_sim before combining, so we'll treat semantic_sim=0 for these
        // — they won't pass any positive threshold, but they will appear if threshold ≤ 0.
        for (Memory m : bm25Results) {
            if (m.getId() != null) {
                bySemantic.putIfAbsent(m.getId(), m);
            }
        }

        List<Memory> scored = new ArrayList<>();
        for (Memory candidate : bySemantic.values()) {
            UUID id = candidate.getId();
            double semanticSim;
            if (semanticResults.stream().anyMatch(s -> id.equals(s.getId()))) {
                // pgvector cosine distance ∈ [0, 2]; for text embeddings typically [0, 1].
                Double distance = candidate.getScore();
                semanticSim = distanceToSimilarity(distance);
            } else {
                semanticSim = 0.0;
            }
            if (semanticSim < similarityThreshold) {
                continue;
            }
            double bm25 = bm25Norm.getOrDefault(id, 0.0);
            double entity = entityBoosts.getOrDefault(id, 0.0);
            double raw = semanticSim + bm25 + entity;
            double combined = Math.min(raw / maxPossible, 1.0);

            Memory copy = cloneWithScore(candidate, combined);
            scored.add(copy);
        }

        scored.sort(Comparator.comparingDouble(Memory::getScore).reversed());
        if (scored.size() > topK) {
            return new ArrayList<>(scored.subList(0, topK));
        }
        return scored;
    }

    /**
     * Convert pgvector cosine distance to similarity in {@code [0, 1]}.
     *
     * <p>For text embeddings (positive cosine), distance is in {@code [0, 1]} and the formula
     * {@code 1 - distance} gives the cosine similarity directly. For the (rare) case of
     * anti-similar vectors where distance > 1, we clamp to 0.
     */
    static double distanceToSimilarity(Double distance) {
        if (distance == null) return 0.0;
        double sim = 1.0 - distance;
        if (sim < 0.0) return 0.0;
        if (sim > 1.0) return 1.0;
        return sim;
    }

    /**
     * Min-max normalize raw BM25 ranks into {@code [0, 1]}.
     *
     * <p>{@code ts_rank_cd} is unbounded above and depends on document length; normalizing
     * per-result-set is robust against query/corpus drift. Empty input → empty map.
     */
    static Map<UUID, Double> normalizeBm25(List<Memory> bm25Results) {
        if (bm25Results == null || bm25Results.isEmpty()) {
            return Map.of();
        }
        double max = 0.0;
        for (Memory m : bm25Results) {
            Double s = m.getScore();
            if (s != null && s > max) {
                max = s;
            }
        }
        if (max <= 0.0) {
            return Map.of();
        }
        Map<UUID, Double> out = new HashMap<>();
        for (Memory m : bm25Results) {
            if (m.getId() == null) continue;
            Double s = m.getScore();
            if (s != null && s > 0.0) {
                out.put(m.getId(), s / max);
            }
        }
        return out;
    }

    private static Memory cloneWithScore(Memory src, double score) {
        return Memory.builder()
                .id(src.getId())
                .userId(src.getUserId())
                .agentId(src.getAgentId())
                .runId(src.getRunId())
                .memory(src.getMemory())
                .embedding(src.getEmbedding())
                .metadata(src.getMetadata())
                .actorId(src.getActorId())
                .role(src.getRole())
                .hash(src.getHash())
                .createdAt(src.getCreatedAt())
                .updatedAt(src.getUpdatedAt())
                .deletedAt(src.getDeletedAt())
                .score(score)
                .build();
    }
}
