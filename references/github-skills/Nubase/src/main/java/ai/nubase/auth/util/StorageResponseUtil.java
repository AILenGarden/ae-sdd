package ai.nubase.auth.util;

import ai.nubase.auth.dto.storage.DownloadResult;
import ai.nubase.auth.dto.storage.ObjectDTO;
import ai.nubase.common.constant.HttpHeaderConstant;
import org.springframework.core.io.Resource;
import org.springframework.http.HttpHeaders;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;

import java.time.Instant;
import java.time.ZoneOffset;
import java.time.format.DateTimeFormatter;
import java.util.Map;

/**
 * Utility for building storage responses.
 */
public final class StorageResponseUtil {

    private StorageResponseUtil() {
    }

    /**
     * Build a file download response.
     */
    public static ResponseEntity<Resource> buildDownloadResponse(DownloadResult result, String download) {
        ObjectDTO metadata = result.metadata();
        HttpHeaders headers = buildObjectHeaders(metadata);

        if (download != null) {
            if (download.isBlank()) {
                headers.set(HttpHeaderConstant.CONTENT_DISPOSITION, "attachment;");
            } else {
                headers.set(HttpHeaderConstant.CONTENT_DISPOSITION,
                        "attachment; filename=" + download + "; filename*=UTF-8''" + download);
            }
        }

        String contentType = resolveContentType(metadata);

        return ResponseEntity.ok()
                .headers(headers)
                .contentType(MediaType.parseMediaType(contentType))
                .body(result.resource());
    }

    /**
     * Build a `HEAD` response (headers only, no body).
     */
    public static ResponseEntity<Void> buildHeadResponse(ObjectDTO metadata) {
        HttpHeaders headers = buildObjectHeaders(metadata);
        return ResponseEntity.ok().headers(headers).build();
    }

    /**
     * Build common response headers from the metadata of an `ObjectDTO`.
     */
    public static HttpHeaders buildObjectHeaders(ObjectDTO metadata) {
        HttpHeaders headers = new HttpHeaders();

        Map<String, Object> meta = metadata.getMetadata();
        if (meta == null) {
            return headers;
        }

        Object size = meta.get("size");
        if (size != null) {
            headers.setContentLength(Long.parseLong(size.toString()));
        }

        Object mimetype = meta.get("mimetype");
        if (mimetype != null) {
            headers.setContentType(MediaType.parseMediaType(mimetype.toString()));
        }

        Object cacheControl = meta.get("cacheControl");
        if (cacheControl != null) {
            headers.setCacheControl(cacheControl.toString());
        }

        Object etag = meta.get("eTag");
        if (etag != null) {
            headers.setETag(etag.toString());
        }

        Object xRobotsTag = meta.get("xRobotsTag");
        if (xRobotsTag != null) {
            headers.set(HttpHeaderConstant.X_ROBOTS_TAG, xRobotsTag.toString());
        }

        Object lastModified = meta.get("lastModified");
        if (lastModified != null) {
            try {
                Instant instant = Instant.parse(lastModified.toString());
                headers.set(HttpHeaderConstant.LAST_MODIFIED, DateTimeFormatter.RFC_1123_DATE_TIME
                        .withZone(ZoneOffset.UTC)
                        .format(instant));
            } catch (Exception ignored) {
            }
        }

        return headers;
    }

    private static String resolveContentType(ObjectDTO metadata) {
        if (metadata.getMetadata() != null && metadata.getMetadata().get("mimetype") != null) {
            return metadata.getMetadata().get("mimetype").toString();
        }
        return MediaType.APPLICATION_OCTET_STREAM_VALUE;
    }
}
