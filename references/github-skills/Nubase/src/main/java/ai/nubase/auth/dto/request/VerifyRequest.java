package ai.nubase.auth.dto.request;

import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

@Data
@NoArgsConstructor
@AllArgsConstructor
public class VerifyRequest {

    @NotBlank(message = "Type is required")
    private String type;  // "signup", "recovery", "email_change", etc.

    @NotBlank(message = "Token is required")
    private String token;

    @Email(message = "Invalid email format")
    private String email;

    private String phone;

    private String redirect_to;
}
