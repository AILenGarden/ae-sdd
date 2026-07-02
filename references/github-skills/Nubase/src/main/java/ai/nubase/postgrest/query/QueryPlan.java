package ai.nubase.postgrest.query;

import ai.nubase.postgrest.api.LogicalCondition;
import lombok.Builder;
import lombok.Data;

import java.util.List;
import java.util.Map;

/**
 * Represents a query execution plan
 * Equivalent to PostgREST's Plan types
 */
@Data
@Builder
public class QueryPlan {
    private QueryType type;
    private String schema;
    private String table;
    private List<String> selectColumns;
    private List<SelectColumnInfo> selectColumnsWithInfo; // Detailed column info with aggregates
    private List<JoinClause> joins;
    private List<WhereClause> whereClauses;          // Simple AND-combined filters
    private List<LogicalCondition> logicalConditions; // Complex OR/AND conditions
    private List<String> orderBy;
    private Long limit;
    private Long offset;
    private Map<String, Object> payload;
    private boolean returningAll;
    private List<String> conflictColumns; // For UPSERT
    private boolean ignoreConflict;       // DO NOTHING vs DO UPDATE
    private boolean hasAggregates;        // True if query has aggregate functions
    private boolean missingAsDefault;     // True if missing columns should use DEFAULT values
    private List<String> specifiedColumns; // Columns explicitly specified in ?columns= param

    public enum QueryType {
        SELECT,
        INSERT,
        UPDATE,
        DELETE,
        UPSERT,         // INSERT ... ON CONFLICT
        CALL_FUNCTION
    }

    /**
     * Detailed select column info including aggregate functions
     */
    @Data
    @Builder
    public static class SelectColumnInfo {
        private String name;           // Column name
        private String alias;          // Output alias
        private boolean isAggregate;   // True if aggregate function
        private String aggregateFunction; // sum, count, avg, min, max
        private String qualified;      // Fully qualified name for SQL
    }
}
