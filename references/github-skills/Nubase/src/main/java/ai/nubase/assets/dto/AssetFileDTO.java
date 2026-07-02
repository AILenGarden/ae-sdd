package ai.nubase.assets.dto;

import lombok.Builder;

import java.time.Instant;

/**
 * Public view of a static asset, including the resolved public URL.
 */
@Builder
public record AssetFileDTO(
        String path,
        String contentType,
        long sizeBytes,
        String etag,
        String cacheControl,
        Instant createdAt,
        Instant updatedAt,
        String publicUrl
) {
}
