package ai.nubase.auth.dto.response.admin;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;
import java.util.Map;

/**
 * Response DTO for schema DDL export
 * <p>
 * Contains complete DDL statements for all tables in the specified schema
 *
 * @author nubase
 * @since 2025-01-03
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ExportSchemaDdlResponse {

    /**
     * Operation success status
     */
    private boolean success;

    /**
     * Schema name that was exported
     */
    private String schemaName;

    /**
     * Map of table name to DDL statement
     * Key: table name
     * Value: complete DDL statement including structure, comments, constraints, and indexes
     */
    private Map<String, String> tableDdls;

    /**
     * List of table names in export order
     * Ordered by foreign key dependencies (tables without foreign keys first)
     */
    private List<String> tableOrder;

    /**
     * Total number of tables exported
     */
    private Integer tableCount;

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
}
