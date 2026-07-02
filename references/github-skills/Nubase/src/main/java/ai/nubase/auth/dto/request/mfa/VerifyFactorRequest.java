package ai.nubase.auth.dto.request.mfa;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

/** Request body for {@code POST /auth/v1/factors/{id}/verify}. */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class VerifyFactorRequest {

    @JsonProperty("challenge_id")
    private String challengeId;

    @NotBlank(message = "code is required")
    private String code;
}
