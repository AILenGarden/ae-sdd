package ai.nubase.auth.util;

import ai.nubase.common.context.MultiTenancyContext;
import org.apache.commons.lang3.StringUtils;

/**
 * Builds S3 object keys, isolating tenant data via the {appCode}/{bucketName}/{path} prefix.
 */
public final class StorageKeyResolver {

    private StorageKeyResolver() {}

    /**
     * Build an S3 key: {appCode}/{bucketName}/{path}
     */
    public static String resolveKey(String bucketName, String path) {
        String appCode = getAppCodeOrThrow();
        return appCode + "/" + bucketName + "/" + path;
    }

    /**
     * Build the bucket prefix: {appCode}/{bucketName}/ (used for list/delete)
     */
    public static String resolveBucketPrefix(String bucketName) {
        String appCode = getAppCodeOrThrow();
        return appCode + "/" + bucketName + "/";
    }

    private static String getAppCodeOrThrow() {
        String appCode = MultiTenancyContext.getAppCode();
        if (StringUtils.isBlank(appCode)) {
            throw new IllegalStateException("appCode is not set in MultiTenancyContext");
        }
        return appCode;
    }
}
