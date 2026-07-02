package ai.nubase.auth.controller.storage;

import ai.nubase.auth.dto.storage.DownloadResult;
import ai.nubase.auth.dto.storage.FileListRequest;
import ai.nubase.auth.dto.storage.ObjectDTO;
import ai.nubase.auth.dto.storage.SignedUrlResponse;
import ai.nubase.auth.service.StorageService;
import ai.nubase.auth.util.SecurityUtil;
import ai.nubase.auth.util.StorageDownloadHelper;
import ai.nubase.auth.util.StorageResponseUtil;
import ai.nubase.common.util.RequestUtil;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.core.io.Resource;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestMethod;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;
import java.util.Map;

/**
 * Storage object read controller.
 * Handles object download, info query, metadata, listing and signed URL operations.
 * Base path: /storage/v1
 */
@Slf4j
@RestController
@RequiredArgsConstructor
@RequestMapping("/storage/v1")
public class StorageObjectReadController {

    private final StorageService storageService;
    private final StorageDownloadHelper storageDownloadHelper;
    private final SecurityUtil securityUtil;

    // ==================== Download ====================

    /**
     * Download an object via a signed URL token.
     * GET /storage/v1/object/sign/{bucketId}/{path}?token=...
     */
    @GetMapping("/object/sign/{bucketId}/**")
    public ResponseEntity<Resource> downloadSignedObject(
            @PathVariable String bucketId,
            @RequestParam String token,
            @RequestParam(required = false) String download,
            HttpServletRequest request
    ) {
        String path = RequestUtil.extractPathVariable(request);
        storageService.verifySignedObjectToken(token, bucketId, path);

        DownloadResult result = storageService.downloadFile(bucketId, path);
        return StorageResponseUtil.buildDownloadResponse(result, download);
    }

    /**
     * Download an object (default route).
     * GET /storage/v1/object/{bucketId}/{path}
     */
    @GetMapping("/object/{bucketId}/**")
    public ResponseEntity<Resource> downloadFile(
            @PathVariable String bucketId,
            @RequestParam(required = false) String download,
            HttpServletRequest request
    ) {
        return storageDownloadHelper.downloadByPrefix(bucketId, request, download);
    }

    /**
     * Download an object (public route).
     * GET /storage/v1/object/public/{bucketId}/{path}
     */
    @GetMapping("/object/public/{bucketId}/**")
    public ResponseEntity<Resource> downloadPublicFile(
            @PathVariable String bucketId,
            @RequestParam(required = false) String download,
            HttpServletRequest request
    ) {
        return storageDownloadHelper.downloadByPrefix(bucketId, request, download);
    }

    /**
     * Download an object (authenticated route).
     * GET /storage/v1/object/authenticated/{bucketId}/**")
     */
    @GetMapping("/object/authenticated/{bucketId}/**")
    public ResponseEntity<Resource> downloadAuthenticatedFile(
            @PathVariable String bucketId,
            @RequestParam(required = false) String download,
            HttpServletRequest request
    ) {
        securityUtil.requireAuthenticatedOrServiceRole();
        return storageDownloadHelper.downloadByPrefix(bucketId, request, download);
    }

    // ==================== Object info (JSON) ====================

    /**
     * Get object info (JSON).
     * GET /storage/v1/object/info/{bucketId}/{path}
     */
    @GetMapping("/object/info/{bucketId}/**")
    public ResponseEntity<ObjectDTO> getObjectInfo(
            @PathVariable String bucketId,
            HttpServletRequest request
    ) {
        String path = RequestUtil.extractPathVariable(request);
        return ResponseEntity.ok(storageService.getObjectMetadata(bucketId, path));
    }

    /**
     * Get object info (JSON, public route).
     * GET /storage/v1/object/info/public/{bucketId}/{path}
     */
    @GetMapping("/object/info/public/{bucketId}/**")
    public ResponseEntity<ObjectDTO> getPublicObjectInfo(
            @PathVariable String bucketId,
            HttpServletRequest request
    ) {
        String path = RequestUtil.extractPathVariable(request);
        return ResponseEntity.ok(storageService.getObjectMetadata(bucketId, path));
    }

    /**
     * Get object info (JSON, authenticated route).
     * GET /storage/v1/object/info/authenticated/{bucketId}/{path}
     */
    @GetMapping("/object/info/authenticated/{bucketId}/**")
    public ResponseEntity<ObjectDTO> getAuthenticatedObjectInfo(
            @PathVariable String bucketId,
            HttpServletRequest request
    ) {
        securityUtil.requireAuthenticatedOrServiceRole();
        String path = RequestUtil.extractPathVariable(request);
        return ResponseEntity.ok(storageService.getObjectMetadata(bucketId, path));
    }

    // ==================== Object metadata (HEAD) ====================

    /**
     * Get object metadata response headers.
     * HEAD /storage/v1/object/{bucketId}/{path}
     */
    @RequestMapping(value = "/object/{bucketId}/**", method = RequestMethod.HEAD)
    public ResponseEntity<Void> getObjectMetadata(
            @PathVariable String bucketId,
            HttpServletRequest request
    ) {
        String path = RequestUtil.extractPathVariable(request);
        return StorageResponseUtil.buildHeadResponse(storageService.getObjectMetadata(bucketId, path));
    }

    /**
     * Get object metadata response headers (public route).
     * HEAD /storage/v1/object/public/{bucketId}/{path}
     */
    @RequestMapping(value = "/object/public/{bucketId}/**", method = RequestMethod.HEAD)
    public ResponseEntity<Void> getPublicObjectMetadata(
            @PathVariable String bucketId,
            HttpServletRequest request
    ) {
        String path = RequestUtil.extractPathVariable(request);
        return StorageResponseUtil.buildHeadResponse(storageService.getObjectMetadata(bucketId, path));
    }

    /**
     * Get object metadata response headers (authenticated route).
     * HEAD /storage/v1/object/authenticated/{bucketId}/{path}
     */
    @RequestMapping(value = "/object/authenticated/{bucketId}/**", method = RequestMethod.HEAD)
    public ResponseEntity<Void> getAuthenticatedObjectMetadata(
            @PathVariable String bucketId,
            HttpServletRequest request
    ) {
        securityUtil.requireAuthenticatedOrServiceRole();
        String path = RequestUtil.extractPathVariable(request);
        return StorageResponseUtil.buildHeadResponse(storageService.getObjectMetadata(bucketId, path));
    }

    // ==================== Listing ====================

    /**
     * List files in a bucket.
     * POST /storage/v1/object/list/{bucketId}
     */
    @PostMapping("/object/list/{bucketId}")
    public ResponseEntity<List<ObjectDTO>> listFiles(
            @PathVariable String bucketId,
            @RequestBody(required = false) Map<String, Object> request
    ) {
        String prefix = request != null ? RequestUtil.stringValue(request.get("prefix")) : null;
        Integer limit = request != null ? RequestUtil.intValue(request.get("limit")) : null;
        Integer offset = request != null ? RequestUtil.intValue(request.get("offset")) : null;
        String search = request != null ? RequestUtil.stringValue(request.get("search")) : null;

        String sortColumn = null;
        String sortOrder = null;
        if (request != null && request.get("sortBy") instanceof Map<?, ?> sortBy) {
            sortColumn = RequestUtil.stringValue(sortBy.get("column"));
            sortOrder = RequestUtil.stringValue(sortBy.get("order"));
        }

        List<ObjectDTO> files = storageService.listFiles(FileListRequest.builder()
                .bucketId(bucketId)
                .prefix(prefix)
                .limit(limit)
                .offset(offset)
                .sortColumn(sortColumn)
                .sortOrder(sortOrder)
                .search(search)
                .build());
        return ResponseEntity.ok(files);
    }

    /**
     * List files in a bucket (v2 compatibility route).
     * POST /storage/v1/object/list-v2/{bucketId}
     */
    @PostMapping("/object/list-v2/{bucketId}")
    public ResponseEntity<Map<String, Object>> listFilesV2(
            @PathVariable String bucketId,
            @RequestBody(required = false) Map<String, Object> request
    ) {
        String prefix = request != null ? RequestUtil.stringValue(request.get("prefix")) : null;
        Integer limit = request != null ? RequestUtil.intValue(request.get("limit")) : null;
        List<ObjectDTO> files = storageService.listFiles(FileListRequest.builder()
                .bucketId(bucketId)
                .prefix(prefix)
                .limit(limit)
                .sortColumn("name")
                .sortOrder("asc")
                .build());

        return ResponseEntity.ok(Map.of(
                "data", files,
                "cursor", ""
        ));
    }

    // ==================== Signed URLs ====================

    /**
     * Create a signed URL for a single file.
     * POST /storage/v1/object/sign/{bucketId}/{path}
     */
    @PostMapping("/object/sign/{bucketId}/**")
    public ResponseEntity<SignedUrlResponse> createSignedUrl(
            @PathVariable String bucketId,
            @RequestBody(required = false) Map<String, Object> request,
            HttpServletRequest httpRequest
    ) {
        String path = RequestUtil.extractPathVariable(httpRequest);
        Integer expiresIn = request != null ? RequestUtil.intValue(request.get("expiresIn")) : null;

        SignedUrlResponse response = storageService.createSignedUrl(bucketId, path, expiresIn);
        return ResponseEntity.ok(response);
    }

    /**
     * Create signed URLs for multiple files.
     * POST /storage/v1/object/sign/{bucketId}
     */
    @PostMapping("/object/sign/{bucketId}")
    public ResponseEntity<List<Map<String, Object>>> createSignedUrls(
            @PathVariable String bucketId,
            @RequestBody Map<String, Object> request
    ) {
        Integer expiresIn = RequestUtil.intValue(request.get("expiresIn"));
        List<String> paths = RequestUtil.toStringList(request.get("paths"));

        List<Map<String, Object>> response = storageService.createSignedUrls(bucketId, paths, expiresIn);
        return ResponseEntity.ok(response);
    }

}
