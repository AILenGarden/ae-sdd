package ai.nubase.common.config;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.context.annotation.Configuration;
import org.springframework.validation.annotation.Validated;

/**
 * Storage configuration properties for Cloudflare R2.
 */
@Configuration
@ConfigurationProperties(prefix = "nubase.storage.r2")
@Validated
@Data
public class R2StorageProperties {

    /**
     * R2 account ID.
     */
    private String accountId;

    /**
     * R2 access key ID.
     */
    private String accessKeyId;

    /**
     * R2 secret access key value.
     */
    private String secretAccessKey;

    /**
     * R2 endpoint URL (format: https://<account_id>.r2.cloudflarestorage.com).
     */
    private String endpoint;

    /**
     * R2 region (usually "auto").
     */
    private String region;

    /**
     * Public URL domain for file access (optional).
     */
    private String publicUrl;

    /**
     * Maximum file size (bytes, default: 50MB).
     */
    @NotNull
    @Min(1)
    private Long maxFileSize;

    /**
     * Allowed MIME types (empty means all types are allowed).
     */
    private String[] allowedMimeTypes = new String[0];

    /**
     * Global shared S3 bucket name (shared by all tenants, isolated via key prefixes).
     */
    private String globalBucket;
}
