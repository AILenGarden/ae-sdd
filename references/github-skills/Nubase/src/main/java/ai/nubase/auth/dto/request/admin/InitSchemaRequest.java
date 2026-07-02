package ai.nubase.auth.dto.request.admin;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Request DTO for initializing a new tenant schema.
 * Creates schema, roles, and auth tables from SQL templates.
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class InitSchemaRequest {

    /**
     * Schema name (must be valid PostgreSQL identifier)
     * Example: "tenant1", "company_abc"
     */
    @NotBlank(message = "Schema name is required")
    @Pattern(regexp = "^[a-z][a-z0-9_]*$", message = "Schema name must start with lowercase letter and contain only lowercase letters, numbers, and underscores")
    private String schema;

    /**
     * Optional admin role name (defaults to admin)
     * Example: "tenant1_admin"
     */
    private String adminRole;

    /**
     * Optional user role name (defaults to authenticated)
     * Example: "authenticated"
     */
    private String userRole;

//    /**
//     * Optional database connection user to grant roles to
//     * If not provided, roles won't be granted to any user
//     */
//    private String dbConnectionUser;
}
