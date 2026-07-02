package ai.nubase.auth.dto.response.admin;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;
import java.util.Map;

/**
 * Response DTO for RLS policies export
 * <p>
 * Contains complete RLS policy statements for tables in the database
 *
 * @author nubase
 * @since 2025-01-03
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ExportRlsPoliciesResponse {

    /**
     * Operation success status
     */
    private boolean success;

    /**
     * Schema name(s) that were queried
     */
    private String schemaName;

    /**
     * Complete RLS policy SQL script
     * Includes ALTER TABLE ENABLE RLS and CREATE POLICY statements
     */
    private String rlsPolicySql;

    /**
     * Map of schema to RLS SQL statements (when groupBySchema is true)
     * Key: schema name
     * Value: RLS SQL for that schema
     */
    private Map<String, String> rlsPoliciesBySchema;

    /**
     * List of table information with RLS enabled
     */
    private List<RlsTableInfo> tablesWithRls;

    /**
     * Total number of tables with RLS enabled
     */
    private Integer tableCount;

    /**
     * Total number of policies exported
     */
    private Integer policyCount;

    /**
     * Execution time in milliseconds
     */
    private Long executionTimeMs;

    /**
     * Error message if success is false
     */
    private String error;

    /**
     * Detailed error information
     */
    private String errorDetails;

    /**
     * Information about a table with RLS enabled
     */
    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class RlsTableInfo {
        /**
         * Schema name
         */
        private String schemaName;

        /**
         * Table name
         */
        private String tableName;

        /**
         * Number of policies on this table
         */
        private Integer policyCount;

        /**
         * Whether RLS is enabled
         */
        private Boolean rlsEnabled;

        /**
         * Whether RLS is forced (applies to table owner too)
         */
        private Boolean rlsForced;

        /**
         * List of policy names
         */
        private List<String> policyNames;
    }
}
