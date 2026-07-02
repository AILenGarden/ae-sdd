package ai.nubase.auth.dto.response.mfa;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/** Response for {@code POST /auth/v1/factors}. */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class EnrollFactorResponse {

    private String id;

    private String type;

    @JsonProperty("friendly_name")
    private String friendlyName;

    /** Present for TOTP factors. */
    private Totp totp;

    /** Present for phone factors. */
    private String phone;

    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    @JsonInclude(JsonInclude.Include.NON_NULL)
    public static class Totp {
        /** otpauth:// provisioning URI (render as a QR code client-side). */
        @JsonProperty("qr_code")
        private String qrCode;
        private String secret;
        private String uri;
    }
}
