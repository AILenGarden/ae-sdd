package ai.nubase.auth.dto.response.mfa;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

/** A single enrolled MFA factor as returned by {@code GET /auth/v1/factors}. */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class FactorResponse {

    private String id;

    @JsonProperty("friendly_name")
    private String friendlyName;

    @JsonProperty("factor_type")
    private String factorType;

    private String status;

    private String phone;

    @JsonProperty("created_at")
    private Instant createdAt;

    @JsonProperty("updated_at")
    private Instant updatedAt;
}
