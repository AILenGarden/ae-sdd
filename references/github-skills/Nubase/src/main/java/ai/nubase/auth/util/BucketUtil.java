package ai.nubase.auth.util;

import ai.nubase.auth.entity.Bucket;

import java.util.Locale;
import java.util.Objects;

public final class BucketUtil {

    private BucketUtil() {
    }

    /**
     * Validate that the file size and MIME type comply with the bucket's limits
     */
    public static void validateFileUpload(Bucket bucket, long fileSize, String contentType) {
        if (Objects.nonNull(bucket.getFileSizeLimit()) && fileSize > bucket.getFileSizeLimit()) {
            throw new IllegalArgumentException("File size exceeds bucket limit");
        }

        if (Objects.nonNull(bucket.getAllowedMimeTypes()) && bucket.getAllowedMimeTypes().length > 0) {
            if (!isMimeTypeAllowed(contentType, bucket.getAllowedMimeTypes())) {
                throw new IllegalArgumentException("File MIME type not allowed: " + contentType);
            }
        }
    }

    private static boolean isMimeTypeAllowed(String mimeType, String[] allowedMimeTypes) {
        if (mimeType == null || mimeType.isBlank()) {
            return false;
        }

        for (String allowedMimeType : allowedMimeTypes) {
            if (allowedMimeType == null || allowedMimeType.isBlank()) {
                continue;
            }
            if (allowedMimeType.equalsIgnoreCase(mimeType)) {
                return true;
            }
            if (allowedMimeType.endsWith("/*")) {
                String prefix = allowedMimeType.substring(0, allowedMimeType.length() - 1);
                if (mimeType.toLowerCase(Locale.ROOT).startsWith(prefix.toLowerCase(Locale.ROOT))) {
                    return true;
                }
            }
        }

        return false;
    }
}
