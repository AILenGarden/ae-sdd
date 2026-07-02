package ai.nubase.auth.util;

import ai.nubase.auth.dto.storage.DownloadResult;
import ai.nubase.auth.service.StorageService;
import ai.nubase.common.util.RequestUtil;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.core.io.Resource;
import org.springframework.http.ResponseEntity;
import org.springframework.stereotype.Component;

@Component
@RequiredArgsConstructor
public class StorageDownloadHelper {

    private final StorageService storageService;

    public ResponseEntity<Resource> downloadByPrefix(
            String bucketId,
            HttpServletRequest request,
            String download
    ) {
        String path = RequestUtil.extractPathVariable(request);
        DownloadResult result = storageService.downloadFile(bucketId, path);
        return StorageResponseUtil.buildDownloadResponse(result, download);
    }
}
