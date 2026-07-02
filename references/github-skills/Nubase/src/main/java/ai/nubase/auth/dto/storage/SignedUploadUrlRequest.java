package ai.nubase.auth.dto.storage;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.UUID;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class SignedUploadUrlRequest {

    private String bucketId;
    private String path;
    private boolean upsert;
    private UUID owner;
}
