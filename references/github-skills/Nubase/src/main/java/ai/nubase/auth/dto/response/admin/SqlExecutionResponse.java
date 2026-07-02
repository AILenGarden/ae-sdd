package ai.nubase.auth.dto.response.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;
import java.util.Map;

/**
 * Response DTO for SQL execution results.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class SqlExecutionResponse {

    /**
     * Execution status (success or error)
     */
    private boolean success;

//    /**
//     * Query results (for SELECT statements)
//     * Each map represents a row with column name -> value
//     */
//    private List<Map<String, Object>> rows;

    /**
     * Number of rows affected (for INSERT/UPDATE/DELETE)
     */
    @JsonProperty("rows_affected")
    private Integer rowsAffected;

    /**
     * Multiple SQL execution results
     * Each result contains information about one SQL statement
     */
    private List<SqlStatementResult> results;

    /**
     * Execution time in milliseconds
     */
    @JsonProperty("execution_time_ms")
    private Long executionTimeMs;

    /**
     * Error message (if execution failed)
     */
    private String error;

    /**
     * Additional metadata about the execution
     */
    private Map<String, Object> metadata;

//    /**
//     * Create a success response for query results
//     */
//    public static SqlExecutionResponse success(List<Map<String, Object>> rows, long executionTimeMs) {
//        return SqlExecutionResponse.builder()
//                .success(true)
//                .rows(rows)
//                .executionTimeMs(executionTimeMs)
//                .build();
//    }

//    /**
//     * Create a success response for DML operations
//     */
//    public static SqlExecutionResponse success(int rowsAffected, long executionTimeMs) {
//        return SqlExecutionResponse.builder()
//                .success(true)
//                .rowsAffected(rowsAffected)
//                .executionTimeMs(executionTimeMs)
//                .build();
//    }

    /**
     * Create a success response with multiple statement results
     */
    public static SqlExecutionResponse successWithResults(List<SqlStatementResult> results, long executionTimeMs) {
        return SqlExecutionResponse.builder()
                .success(true)
                .results(results)
                .executionTimeMs(executionTimeMs)
                .build();
    }

    /**
     * Create an error response
     */
    public static SqlExecutionResponse error(String errorMessage, long executionTimeMs) {
        return SqlExecutionResponse.builder()
                .success(false)
                .error(errorMessage)
                .executionTimeMs(executionTimeMs)
                .build();
    }

    /**
     * Represents the result of a single SQL statement
     */
    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class SqlStatementResult {
        /**
         * Statement index (0-based)
         */
        private int index;

        /**
         * Type of statement: "query" (SELECT) or "update" (INSERT/UPDATE/DELETE/DDL)
         */
        private String type;

        /**
         * Query results (for SELECT statements)
         */
        private List<Map<String, Object>> rows;

        /**
         * Number of rows affected (for DML/DDL statements)
         */
        @JsonProperty("rows_affected")
        private Integer rowsAffected;

        /**
         * Error message if this statement failed
         */
        private String error;
    }
}
