package ai.nubase.auth.dto.request.admin;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Request DTO for exporting schema DDL
 * <p>
 * Exports complete DDL statements for all tables in a schema, including:
 * - Table structure
 * - Column definitions with types
 * - Column comments
 * - Primary keys
 * - Foreign keys
 * - Unique constraints
 * - Check constraints
 * - Indexes
 *
 * @author nubase
 * @since 2025-01-03
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class ExportSchemaDdlRequest {

    /**
     * Schema name to export
     * Example: "auth", "storage", "public"
     */
    @NotBlank(message = "Schema name is required")
    @Pattern(regexp = "^[a-z][a-z0-9_]*$",
             message = "Schema name must start with lowercase letter and contain only lowercase letters, numbers, and underscores")
    private String schemaName;

    /**
     * Include DROP statements before CREATE
     * Default: false
     */
    private Boolean includeDropStatements;

    /**
     * Include IF NOT EXISTS clause in CREATE statements
     * Default: true
     */
    private Boolean includeIfNotExists;

    /**
     * Export specific tables only (comma-separated)
     * If null or empty, export all tables in the schema
     * Example: "users,sessions,refresh_tokens"
     */
    private String tableNames;
}
