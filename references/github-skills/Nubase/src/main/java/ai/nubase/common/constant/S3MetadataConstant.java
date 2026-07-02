package ai.nubase.common.constant;

/**
 * S3/R2 object metadata key constants.
 */
public final class S3MetadataConstant {

    private S3MetadataConstant() {}

    // S3 user-metadata keys (stored in object metadata on R2/S3)
    public static final String ORIGINAL_FILENAME = "original-filename";
    public static final String CONTENT_TYPE = "content-type";
    public static final String SIZE = "size";

    // StorageObject metadata keys (stored in PostgreSQL)
    public static final String MIMETYPE = "mimetype";
    public static final String CACHE_CONTROL = "cacheControl";
    public static final String ETAG = "eTag";
    public static final String LAST_MODIFIED = "lastModified";
}
