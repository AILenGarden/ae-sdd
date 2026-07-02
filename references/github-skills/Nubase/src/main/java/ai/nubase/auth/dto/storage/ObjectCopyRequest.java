package ai.nubase.auth.dto.storage;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;
import java.util.UUID;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ObjectCopyRequest {

    private String bucketId;
    private String sourceKey;
    private String destinationBucket;
    private String destinationKey;
    private UUID owner;
    private boolean upsert;
    private Map<String, Object> metadataOverrides;
}
