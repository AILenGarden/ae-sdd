package ai.nubase.auth.dto.request.platform;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Size;
import lombok.Data;

/** Step 2 of an authenticated password change: current password + new password + emailed code. */
@Data
public class PlatformPasswordChangeRequest {

    @NotBlank
    private String currentPassword;

    @NotBlank
    @Size(min = 8, max = 128)
    private String newPassword;

    @NotBlank
    private String code;
}
