package ai.nubase.auth.dto.request;

import jakarta.validation.constraints.NotBlank;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

/** Request body for {@code POST /auth/v1/resend}. */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class ResendRequest {

    /** signup | email_change | sms | phone_change */
    @NotBlank(message = "type is required")
    private String type;

    private String email;

    private String phone;
}
