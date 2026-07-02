package ai.nubase.auth.dto.response;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.Map;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class IdentityResponse {

    private String id;

    @JsonProperty("user_id")
    private String userId;

    @JsonProperty("identity_data")
    private Map<String, Object> identityData;

    private String provider;

    @JsonProperty("last_sign_in_at")
    private Instant lastSignInAt;

    @JsonProperty("created_at")
    private Instant createdAt;

    @JsonProperty("updated_at")
    private Instant updatedAt;
}
