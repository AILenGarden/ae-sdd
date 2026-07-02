package ai.nubase.postgrest.query;

import lombok.Builder;
import lombok.Data;

import java.util.List;
import java.util.Map;

/**
 * Query execution result
 */
@Data
@Builder
public class QueryResult {
    private List<Map<String, Object>> data;
    private Integer totalCount;
}
