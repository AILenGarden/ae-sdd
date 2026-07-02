package ai.nubase.auth.dto.response.admin;

import ai.nubase.auth.dto.response.UserResponse;
import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/** Response for {@code POST /auth/v1/admin/generate_link}. */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class GenerateLinkResponse {

    /** The full verification action link to hand to the user. */
    @JsonProperty("action_link")
    private String actionLink;

    /** The raw token embedded in the link (the GoTrue {@code token}). */
    private String token;

    /** Numeric OTP code (magic-link flow only). */
    @JsonProperty("email_otp")
    private String emailOtp;

    @JsonProperty("verification_type")
    private String verificationType;

    @JsonProperty("redirect_to")
    private String redirectTo;

    private UserResponse user;
}
