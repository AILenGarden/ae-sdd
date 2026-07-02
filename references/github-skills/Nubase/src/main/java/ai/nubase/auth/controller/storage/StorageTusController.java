package ai.nubase.auth.controller.storage;

import ai.nubase.auth.service.TusUploadService;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestMethod;
import org.springframework.web.bind.annotation.RestController;

import java.io.IOException;

@RestController
@RequiredArgsConstructor
@RequestMapping("/storage/v1/upload/resumable")
public class StorageTusController {

    private static final String TUS_VERSION = "1.0.0";

    private final TusUploadService tusUploadService;

    @RequestMapping(value = {"", "/**"}, method = RequestMethod.OPTIONS)
    public ResponseEntity<Void> options() {
        HttpHeaders headers = new HttpHeaders();
        headers.set("Tus-Resumable", TUS_VERSION);
        headers.set("Tus-Version", TUS_VERSION);
        headers.set("Tus-Extension", "creation,creation-with-upload,termination");
        headers.set("Tus-Max-Size", "52428800");
        return ResponseEntity.status(HttpStatus.NO_CONTENT).headers(headers).build();
    }

    @PostMapping({"", "/**"})
    public ResponseEntity<Void> createUpload(
            @RequestHeader(value = "Upload-Length", required = false) Long uploadLength,
            @RequestHeader(value = "Upload-Metadata", required = false) String uploadMetadata,
            HttpServletRequest request
    ) {
        tusUploadService.createSession(uploadLength, uploadMetadata, extractSuffix(request), null, false);
        return ResponseEntity.status(HttpStatus.CREATED).build();
    }

    @PutMapping("/**")
    public ResponseEntity<Void> putUpload(
            @RequestHeader(value = "Upload-Offset", required = false) Long uploadOffset,
            HttpServletRequest request
    ) throws IOException {
        tusUploadService.append(extractUploadId(request), request.getInputStream().readAllBytes(), uploadOffset);
        return ResponseEntity.status(HttpStatus.NO_CONTENT).build();
    }

    @PatchMapping("/**")
    public ResponseEntity<Void> patchUpload(
            @RequestHeader(value = "Upload-Offset", required = false) Long uploadOffset,
            HttpServletRequest request
    ) throws IOException {
        tusUploadService.append(extractUploadId(request), request.getInputStream().readAllBytes(), uploadOffset);
        return ResponseEntity.status(HttpStatus.NO_CONTENT).build();
    }

    @RequestMapping(value = "/**", method = RequestMethod.HEAD)
    public ResponseEntity<Void> headUpload(HttpServletRequest request) {
        tusUploadService.getSession(extractUploadId(request));
        return ResponseEntity.ok().build();
    }

    @DeleteMapping("/**")
    public ResponseEntity<Void> deleteUpload(HttpServletRequest request) {
        tusUploadService.deleteSession(extractUploadId(request));
        return ResponseEntity.status(HttpStatus.NO_CONTENT).build();
    }

    private String extractUploadId(HttpServletRequest request) {
        String suffix = extractSuffix(request);
        if (suffix == null || suffix.isBlank()) {
            throw new IllegalArgumentException("Invalid upload path");
        }
        int slash = suffix.indexOf('/');
        return slash == -1 ? suffix : suffix.substring(0, slash);
    }

    private String extractSuffix(HttpServletRequest request) {
        String prefix = "/storage/v1/upload/resumable";
        String uri = request.getRequestURI();
        if (!uri.startsWith(prefix)) {
            throw new IllegalArgumentException("Invalid upload path");
        }
        String suffix = uri.substring(prefix.length());
        if (suffix.startsWith("/")) {
            suffix = suffix.substring(1);
        }
        return suffix;
    }
}
