package ai.nubase.auth.dto.storage;

import java.util.UUID;

public record SignedUploadToken(UUID owner, boolean upsert) {
}
