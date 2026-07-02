package ai.nubase.auth.dto.request.mfa;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

/** Request body for {@code POST /auth/v1/factors} (MFA enrollment). */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class EnrollFactorRequest {

    /** "totp" (default) or "phone". */
    @JsonProperty("factor_type")
    private String factorType;

    @JsonProperty("friendly_name")
    private String friendlyName;

    /** Custom issuer label for the otpauth URI (TOTP only). */
    private String issuer;

    /** E.164 phone number (phone factor only). */
    private String phone;
}
