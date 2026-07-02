package ai.nubase.auth.dto.request.admin;

import lombok.Data;

/**
 * Body for PATCH /auth/v1/admin/projects/{ref}. All fields are optional —
 * only those set are applied. Used for renaming a project, editing its
 * description and pausing/resuming traffic.
 */
@Data
public class UpdateProjectRequest {
    private String appName;
    private String description;
    /** false → pause (drops from active list); true → resume. */
    private Boolean enabled;
}
