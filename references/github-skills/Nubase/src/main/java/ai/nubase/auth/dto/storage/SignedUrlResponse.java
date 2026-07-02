package ai.nubase.auth.dto.storage;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Signed URL response matching the Supabase Storage API format.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class SignedUrlResponse {

    @JsonProperty("signedURL")
    private String signedUrl;
}
