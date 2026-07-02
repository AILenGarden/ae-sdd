package ai.nubase.auth.dto.response.admin;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class PlatformUserResponse {
    private String id;
    private String email;
    private String fullName;
    /** 'super_admin' | 'user' */
    private String role;
    private Boolean isActive;
    private Instant lastSignedInAt;
    private Instant createdAt;
}
