package ai.nubase.auth.dto.request.admin;

import lombok.Data;

@Data
public class UpdatePlatformUserRequest {
    /** 'super_admin' | 'user' — only set if changing. */
    private String role;
    /** Disable / re-enable an account. */
    private Boolean isActive;
}
