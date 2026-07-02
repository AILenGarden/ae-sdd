package ai.nubase.auth.dto.response.admin;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

/**
 * Lightweight projection of a tenant project for the Studio dashboard.
 * Returned by GET /auth/v1/admin/projects. Only carries fields safe to expose
 * to a Studio session — no encrypted secrets, no decrypted passwords.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ProjectSummaryResponse {

    private String ref;
    private String name;
    private String description;
    private String schemaName;
    private String initStatus;
    private String healthStatus;
    private Boolean enabled;
    private Instant createdAt;
    private Instant updatedAt;

    /**
     * Service-role API key (JWT signed with the tenant's secret).
     * Studio uses this as the `apikey` header when calling tenant-scoped endpoints.
     */
    private String apikey;
}
