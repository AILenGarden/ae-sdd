package ai.nubase.auth.dto.storage;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Update-bucket request matching the Supabase Storage API format.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class UpdateBucketRequest {

    @JsonProperty("public")
    private Boolean isPublic;

    @JsonProperty("avif_autodetection")
    private Boolean avifAutodetection;

    @JsonProperty("file_size_limit")
    private Long fileSizeLimit;

    @JsonProperty("allowed_mime_types")
    private String[] allowedMimeTypes;
}
