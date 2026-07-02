package ai.nubase.auth.dto.request.platform;

import jakarta.validation.constraints.NotBlank;
import lombok.Data;

/** Step 1 of an authenticated password change: prove the current password, get an emailed code. */
@Data
public class PlatformPasswordOtpRequest {

    @NotBlank
    private String currentPassword;
}
