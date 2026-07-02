package ai.nubase.auth.dto.response;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.List;
import java.util.Map;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class UserResponse {

    private String id;

    private String aud;

    private String role;

    private String email;

    @JsonProperty("email_confirmed_at")
    private Instant emailConfirmedAt;

    private String phone;

    @JsonProperty("phone_confirmed_at")
    private Instant phoneConfirmedAt;

    @JsonProperty("confirmed_at")
    private Instant confirmedAt;

    @JsonProperty("last_sign_in_at")
    private Instant lastSignInAt;

    @JsonProperty("banned_until")
    private Instant bannedUntil;

    @JsonProperty("app_metadata")
    private Map<String, Object> appMetadata;

    @JsonProperty("user_metadata")
    private Map<String, Object> userMetadata;

    private List<IdentityResponse> identities;

    @JsonProperty("created_at")
    private Instant createdAt;

    @JsonProperty("updated_at")
    private Instant updatedAt;
}
