package ai.nubase.auth.dto.request;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;

/** Request body for {@code POST /auth/v1/otp} (passwordless magic link / OTP). */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class OtpRequest {

    private String email;

    private String phone;

    /** Auto-create the user if they don't exist yet (default true). */
    @JsonProperty("create_user")
    private Boolean createUser;

    @JsonProperty("data")
    private Map<String, Object> data;

    @JsonProperty("redirect_to")
    private String redirectTo;

    @JsonProperty("captcha_token")
    private String captchaToken;

    /** PKCE: when set, verifying the magic link yields an auth code instead of a session. */
    @JsonProperty("code_challenge")
    private String codeChallenge;

    @JsonProperty("code_challenge_method")
    private String codeChallengeMethod;
}
