package ai.nubase.auth.dto.response.mfa;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/** Response for {@code POST /auth/v1/factors/{id}/challenge}. */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ChallengeResponse {

    private String id;

    private String type;

    /** Unix epoch seconds when the challenge expires. */
    @JsonProperty("expires_at")
    private Long expiresAt;
}
