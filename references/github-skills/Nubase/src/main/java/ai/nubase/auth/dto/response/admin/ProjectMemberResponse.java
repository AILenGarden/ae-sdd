package ai.nubase.auth.dto.response.admin;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

/**
 * One row in the project members table — a platform user with their role on the project.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ProjectMemberResponse {
    private String userId;
    private String email;
    private String fullName;
    /** 'owner' | 'member' */
    private String role;
    private Instant addedAt;
}
