package ai.nubase.auth.dto.request;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Email;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;

@Data
@NoArgsConstructor
@AllArgsConstructor
public class SignUpRequest {

    // Not @NotBlank: an empty body signs in anonymously (Supabase parity).
    // Presence of email/password is enforced in AuthService for the normal flow.
    @Email(message = "Invalid email format")
    private String email;

    private String password;

    private String phone;

    @JsonProperty("data")
    private Map<String, Object> data;  // User metadata

    // Optional fields for invite flow
    @JsonProperty("redirect_to")
    private String redirectTo;

    @JsonProperty("captcha_token")
    private String captchaToken;
}
