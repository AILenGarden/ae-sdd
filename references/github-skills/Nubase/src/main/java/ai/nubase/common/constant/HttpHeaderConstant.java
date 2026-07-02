package ai.nubase.common.constant;

/**
 * HTTP Header constants.
 * <p>
 * Centralizes constants for all HTTP headers, avoiding magic strings scattered throughout the code.
 * <p>
 *
 * @author nubase
 * @since 2026-03-03
 */
public class HttpHeaderConstant {

    // ==================== Standard HTTP Headers ====================

    /**
     * Authorization header
     */
    public static final String AUTHORIZATION = "Authorization";

    /**
     * Content-Type header
     */
    public static final String CONTENT_TYPE = "Content-Type";

    /**
     * Cache-Control header
     */
    public static final String CACHE_CONTROL = "cache-control";

    /**
     * ETag header
     */
    public static final String ETAG = "ETag";

    /**
     * Last-Modified header
     */
    public static final String LAST_MODIFIED = "last-modified";

    /**
     * Content-Disposition header
     */
    public static final String CONTENT_DISPOSITION = "Content-Disposition";

    // ==================== Custom Headers ====================

    /**
     * X-Upsert header - indicates whether upsert mode is used
     */
    public static final String X_UPSERT = "x-upsert";

    /**
     * X-Robots-Tag header - robot access tag
     */
    public static final String X_ROBOTS_TAG = "x-robots-tag";

    /**
     * X-Metadata header - custom metadata
     */
    public static final String X_METADATA = "x-metadata";

    /**
     * X-Forwarded-For header - original client IP
     */
    public static final String X_FORWARDED_FOR = "X-Forwarded-For";

    /**
     * X-Real-IP header - real client IP
     */
    public static final String X_REAL_IP = "X-Real-IP";

    // ==================== Token Headers ====================

    /**
     * API Key header
     */
    public static final String APIKEY = "apikey";

    /**
     * Bearer token prefix
     */
    public static final String BEARER_PREFIX = "Bearer ";

    // ==================== Private Constructor ====================

    private HttpHeaderConstant() {
        throw new AssertionError("Cannot instantiate constant class");
    }
}
