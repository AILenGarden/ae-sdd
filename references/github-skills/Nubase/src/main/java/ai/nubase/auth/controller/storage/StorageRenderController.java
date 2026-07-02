package ai.nubase.auth.controller.storage;

/**
 * Render-image path compatibility controller.
 *
 * TODO: Currently only matches the Supabase `/render/image` path shape; the actual logic
 * still reuses raw object download and does not yet implement true image transformation
 * (width, height, resize, format, quality, etc.). This should not be treated as a complete
 * render endpoint. Filling this in later requires a separate evaluation of a Java-side
 * image processing solution, especially for AVIF output.
 */

import ai.nubase.auth.dto.storage.DownloadResult;
import ai.nubase.auth.service.StorageService;
import ai.nubase.auth.util.SecurityUtil;
import ai.nubase.auth.util.StorageResponseUtil;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.core.io.Resource;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

@Slf4j
@RestController
@RequiredArgsConstructor
@RequestMapping("/storage/v1/render/image")
public class StorageRenderController {

    private final StorageService storageService;
    private final SecurityUtil securityUtil;

    @GetMapping("/public/{bucketId}/**")
    public ResponseEntity<Resource> renderPublicImage(
            @PathVariable String bucketId,
            @RequestParam(required = false) String download,
            HttpServletRequest request
    ) {
        String path = extractPathFromRequest(request, "/storage/v1/render/image/public/" + bucketId + "/");
        return renderByPath(bucketId, path, download);
    }

    @GetMapping("/authenticated/{bucketId}/**")
    public ResponseEntity<Resource> renderAuthenticatedImage(
            @PathVariable String bucketId,
            @RequestParam(required = false) String download,
            HttpServletRequest request
    ) {
        securityUtil.requireAuthenticatedOrServiceRole();
        String path = extractPathFromRequest(request, "/storage/v1/render/image/authenticated/" + bucketId + "/");
        return renderByPath(bucketId, path, download);
    }

    @GetMapping("/sign/{bucketId}/**")
    public ResponseEntity<Resource> renderSignedImage(
            @PathVariable String bucketId,
            @RequestParam String token,
            @RequestParam(required = false) String download,
            HttpServletRequest request
    ) {
        String path = extractPathFromRequest(request, "/storage/v1/render/image/sign/" + bucketId + "/");
        storageService.verifySignedObjectToken(token, bucketId, path);
        return renderByPath(bucketId, path, download);
    }

    // TODO: this currently returns the raw object content as a placeholder compatibility
    // implementation for the render path. When the render capability is filled in, the
    // actual image transformation pipeline should be wired in here instead of downloading
    // the original image directly.
    private ResponseEntity<Resource> renderByPath(String bucketId, String path, String download) {
        DownloadResult result = storageService.downloadFile(bucketId, path);
        return StorageResponseUtil.buildDownloadResponse(result, download);
    }

    private String extractPathFromRequest(HttpServletRequest request, String basePath) {
        String requestURI = request.getRequestURI();
        int basePathIndex = requestURI.indexOf(basePath);
        if (basePathIndex != -1) {
            return requestURI.substring(basePathIndex + basePath.length());
        }
        throw new IllegalArgumentException("Invalid request path");
    }
}
