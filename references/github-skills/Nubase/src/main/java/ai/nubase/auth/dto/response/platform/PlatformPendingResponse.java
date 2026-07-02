package ai.nubase.auth.dto.response.platform;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Returned when a platform flow cannot complete yet because an emailed verification code is required.
 * The frontend switches to a code-entry step and posts to {@code /verify-email}.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class PlatformPendingResponse {

    @JsonProperty("verification_required")
    @Builder.Default
    private boolean verificationRequired = true;

    private String email;
}
