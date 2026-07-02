package ai.nubase.auth.dto.storage;

import org.springframework.core.io.Resource;

/**
 * Download result containing the file resource and its metadata.
 */
public record DownloadResult(Resource resource, ObjectDTO metadata) {
}
