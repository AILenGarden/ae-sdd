package ai.nubase.auth.dto.request.platform;

import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;
import lombok.Data;

@Data
public class PlatformVerifyEmailRequest {

    @NotBlank
    @Email
    private String email;

    @NotBlank
    private String code;
}
