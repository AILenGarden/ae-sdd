package ai.nubase.auth.dto.storage;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * Object DTO matching the Supabase Storage API format.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ObjectDTO {

    private String name;

    @JsonProperty("bucket_id")
    private String bucketId;

    private UUID owner;

    private UUID id;

    @JsonProperty("updated_at")
    private Instant updatedAt;

    @JsonProperty("created_at")
    private Instant createdAt;

    @JsonProperty("last_accessed_at")
    private Instant lastAccessedAt;

    private Map<String, Object> metadata;

    @JsonProperty("buckets")
    private BucketDTO bucket;
}
