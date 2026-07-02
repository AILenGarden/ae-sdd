package ai.nubase.auth.dto.storage;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

/**
 * Bucket DTO matching the Supabase Storage API format.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class BucketDTO {

    private String id;

    private String type;

    private String name;

    private String owner;

    @JsonProperty("public")
    private Boolean isPublic;

    @JsonProperty("avif_autodetection")
    private Boolean avifAutodetection;

    @JsonProperty("file_size_limit")
    private Long fileSizeLimit;

    @JsonProperty("allowed_mime_types")
    private String[] allowedMimeTypes;

    @JsonProperty("created_at")
    private Instant createdAt;

    @JsonProperty("updated_at")
    private Instant updatedAt;
}
