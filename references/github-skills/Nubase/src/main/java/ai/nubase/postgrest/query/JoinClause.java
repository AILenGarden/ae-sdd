package ai.nubase.postgrest.query;

import lombok.Builder;
import lombok.Data;

import java.util.List;

/**
 * Represents a JOIN clause with embedded resource column selection
 */
@Data
@Builder
public class JoinClause {
    private JoinType type;
    private String targetSchema;
    private String targetTable;
    private String alias;
    private List<JoinCondition> conditions;

    /**
     * Columns to select from the joined table.
     * If null or empty, all columns (*) will be selected.
     * Used for embedded resource queries like: table!inner(col1, col2)
     */
    private List<String> selectColumns;

    /**
     * The original embedding name used in the query (for nested JSON response)
     * e.g., "seo_schemas" in seo_schemas!inner(schema_name, schema_type)
     */
    private String embeddingName;

    public enum JoinType {
        INNER,
        LEFT,
        RIGHT,
        FULL
    }

    @Data
    @Builder
    public static class JoinCondition {
        private String leftColumn;
        private String rightColumn;
        private String operator; // Usually "="
    }
}
