package ai.nubase.auth.dto.response.platform;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class PlatformUserPayload {
    private String id;
    private String email;

    @JsonProperty("full_name")
    private String fullName;

    /** Platform role: 'super_admin' or 'user'. */
    private String role;

    @JsonProperty("created_at")
    private Instant createdAt;
}
