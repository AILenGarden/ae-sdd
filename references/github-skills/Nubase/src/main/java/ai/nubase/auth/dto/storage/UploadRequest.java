package ai.nubase.auth.dto.storage;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.UUID;

/**
 * Upload request DTO.
 * <p>
 * Encapsulates all parameters used when uploading a file.
 * <p>
 *
 * @author nubase
 * @since 2026-03-03
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class UploadRequest {

    /**
     * Bucket ID
     */
    private String bucketId;

    /**
     * Resolved object path (e.g., "folder/file.jpg")
     */
    private String path;

    /**
     * Raw file bytes
     */
    private byte[] fileBytes;

    /**
     * Content type of the file (e.g., "image/png")
     */
    private String contentType;

    /**
     * File size in bytes
     */
    private long fileSize;

    /**
     * Cache-Control value (from a form field or request header)
     */
    private String cacheControl;

    /**
     * x-metadata request header
     */
    private String xMetadata;

    /**
     * Owner ID override
     */
    private UUID ownerOverride;

}
