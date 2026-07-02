package ai.nubase.auth.dto.storage;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Create-bucket request matching the Supabase Storage API format.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class CreateBucketRequest {

    @NotBlank(message = "Bucket name is required")
    @Pattern(regexp = "^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$",
             message = "Bucket name must be 3-63 characters, lowercase alphanumeric with hyphens")
    private String name;

    private String id;

    private String type;

    @JsonProperty("public")
    private Boolean isPublic = false;

    @JsonProperty("avif_autodetection")
    private Boolean avifAutodetection = false;

    @JsonProperty("file_size_limit")
    private Long fileSizeLimit;

    @JsonProperty("allowed_mime_types")
    private String[] allowedMimeTypes;
}
