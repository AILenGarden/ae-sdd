package ai.nubase.auth.dto.request.admin;

import jakarta.validation.constraints.Pattern;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Request DTO for exporting RLS (Row Level Security) policies
 * <p>
 * Exports all RLS policies for tables in specified schema(s), including:
 * - ALTER TABLE ... ENABLE ROW LEVEL SECURITY statements
 * - CREATE POLICY statements with USING and WITH CHECK clauses
 * - Role assignments
 *
 * @author nubase
 * @since 2025-01-03
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class ExportRlsPoliciesRequest {

    /**
     * Schema name to export RLS policies from
     * If null or empty, export from all schemas in the database
     * Example: "auth", "storage", "public"
     */
    @Pattern(regexp = "^[a-z][a-z0-9_]*$",
             message = "Schema name must start with lowercase letter and contain only lowercase letters, numbers, and underscores")
    private String schemaName;

    /**
     * Export specific tables only (comma-separated)
     * If null or empty, export all tables with RLS policies in the schema
     * Example: "users,sessions,refresh_tokens"
     */
    private String tableNames;

    /**
     * Include DROP POLICY statements before CREATE
     * Default: false
     */
    private Boolean includeDropStatements;

    /**
     * Group policies by schema
     * When true, policies are organized by schema in the response
     * Default: true
     */
    private Boolean groupBySchema;
}
