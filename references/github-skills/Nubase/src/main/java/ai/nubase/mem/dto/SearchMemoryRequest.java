package ai.nubase.mem.dto;

import lombok.Data;

import java.util.Map;
import java.util.UUID;

/**
 * Request body for {@code POST /mem/v1/search}.
 */
@Data
public class SearchMemoryRequest {

    private String query;

    private UUID userId;
    private String agentId;
    private String runId;

    private Integer topK;

    /** Cosine distance threshold (0..2). Results with distance > threshold are excluded. */
    private Double threshold;

    /**
     * Optional advanced metadata filters.
     *
     * <p>See {@code MetadataFilterParser} for the supported grammar. Examples:
     * <pre>
     *   {"category": "food"}
     *   {"category": {"eq": "food"}, "year": {"gte": 2025}}
     *   {"AND": [{"tags": {"in": ["urgent", "work"]}}, {"NOT": {"archived": "true"}}]}
     * </pre>
     */
    private Map<String, Object> metadataFilters;
}
