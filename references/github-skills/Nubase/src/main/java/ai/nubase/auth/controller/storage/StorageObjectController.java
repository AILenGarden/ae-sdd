package ai.nubase.auth.controller.storage;

import ai.nubase.auth.dto.storage.ObjectCopyRequest;
import ai.nubase.auth.dto.storage.ObjectDTO;
import ai.nubase.auth.dto.storage.ObjectMoveRequest;
import ai.nubase.auth.dto.storage.SignedUploadToken;
import ai.nubase.auth.dto.storage.SignedUploadUrlRequest;
import ai.nubase.auth.dto.storage.UploadRequest;
import ai.nubase.auth.dto.storage.UploadResponse;
import ai.nubase.auth.service.StorageService;
import ai.nubase.auth.service.StorageUploadLimitService;
import ai.nubase.auth.util.SecurityUtil;
import ai.nubase.common.constant.HttpHeaderConstant;
import ai.nubase.common.enums.TrueOrFalseEnum;
import ai.nubase.common.util.RequestUtil;
import ai.nubase.common.util.RequestUtil.ParsedUpload;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestMethod;
import org.springframework.web.bind.annotation.RestController;

import java.io.IOException;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Storage object write controller.
 * Handles object upload, update, delete, move, and copy operations.
 * Base path: /storage/v1
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/storage/v1")
public class StorageObjectController {

    private final StorageService storageService;
    private final SecurityUtil securityUtil;
    private final StorageUploadLimitService storageUploadLimitService;

    // ==================== Upload ====================

    /**
     * Upload or update a file.
     * POST /storage/v1/object/{bucketId}/{path} - insert or update (controlled by the x-upsert header)
     * PUT  /storage/v1/object/{bucketId}/{path} - always performs an update (upsert)
     */
    @RequestMapping(value = "/object/{bucketId}/**", method = {RequestMethod.POST, RequestMethod.PUT})
    public ResponseEntity<UploadResponse> uploadFile(
            @PathVariable String bucketId,
            @RequestHeader(value = HttpHeaderConstant.X_UPSERT, required = false) String xUpsert,
            @RequestHeader(value = HttpHeaderConstant.X_METADATA, required = false) String xMetadata,
            HttpServletRequest request
    ) throws IOException {
        byte[] rawBody = RequestUtil.readRawRequestBody(request, storageUploadLimitService.getMaxUploadBytesForCurrentUser());
        boolean isUpsert = RequestMethod.PUT.name().equalsIgnoreCase(request.getMethod())
                || TrueOrFalseEnum.TRUE.getValue().equalsIgnoreCase(xUpsert);
        String path = RequestUtil.extractPathVariable(request);

        ParsedUpload parsed = RequestUtil.parseUploadParts(request, rawBody);

        UploadRequest uploadRequest = UploadRequest.builder()
                .bucketId(bucketId)
                .path(path)
                .fileBytes(parsed.fileBytes())
                .contentType(parsed.contentType())
                .fileSize(parsed.fileBytes().length)
                .cacheControl(parsed.cacheControl())
                .xMetadata(xMetadata)
                .ownerOverride(securityUtil.getCurrentUserId())
                .build();

        UploadResponse response = storageService.doUpload(uploadRequest, isUpsert);
        return ResponseEntity.ok(response);
    }

    /**
     * Upload via a signed URL.
     * PUT /storage/v1/object/upload/sign/{bucketId}/{path}?token=...
     */
    @PutMapping("/object/upload/sign/{bucketId}/**")
    public ResponseEntity<UploadResponse> uploadSignedFile(
            @PathVariable String bucketId,
            @RequestHeader(value = HttpHeaderConstant.X_METADATA, required = false) String xMetadata,
            HttpServletRequest request
    ) throws IOException {
        String path = RequestUtil.extractPathVariable(request);
        String token = extractTokenFromQueryString(request.getQueryString());
        if (token == null || token.isBlank()) {
            throw new IllegalArgumentException("Missing signed upload token");
        }
        SignedUploadToken signedUploadToken = storageService.verifySignedUploadToken(token, bucketId, path);

        byte[] rawBody = RequestUtil.readRawRequestBody(request, storageUploadLimitService.getMaxUploadBytesForCurrentUser());
        ParsedUpload parsed = RequestUtil.parseUploadParts(request, rawBody);

        UploadRequest uploadRequest = UploadRequest.builder()
                .bucketId(bucketId)
                .path(path)
                .fileBytes(parsed.fileBytes())
                .contentType(parsed.contentType())
                .fileSize(parsed.fileBytes().length)
                .cacheControl(parsed.cacheControl())
                .xMetadata(xMetadata)
                .ownerOverride(signedUploadToken.owner())
                .build();

        UploadResponse response = storageService.doUpload(uploadRequest, signedUploadToken.upsert());
        return ResponseEntity.ok(response);
    }

    private static String extractTokenFromQueryString(String queryString) {
        if (queryString == null || queryString.isBlank()) {
            return null;
        }
        for (String pair : queryString.split("&")) {
            if (pair.isEmpty()) {
                continue;
            }
            int idx = pair.indexOf('=');
            String rawKey = idx >= 0 ? pair.substring(0, idx) : pair;
            if (!"token".equals(urlDecode(rawKey))) {
                continue;
            }
            String rawValue = idx >= 0 ? pair.substring(idx + 1) : "";
            return urlDecode(rawValue);
        }
        return null;
    }

    private static String urlDecode(String value) {
        return URLDecoder.decode(value, StandardCharsets.UTF_8);
    }

    /**
     * Create a signed upload URL.
     * POST /storage/v1/object/upload/sign/{bucketId}/{path}
     */
    @PostMapping("/object/upload/sign/{bucketId}/**")
    public ResponseEntity<Map<String, String>> createSignedUploadUrl(
            @PathVariable String bucketId,
            @RequestHeader(value = HttpHeaderConstant.X_UPSERT, required = false) String xUpsert,
            HttpServletRequest request
    ) {
        String path = RequestUtil.extractPathVariable(request);
        boolean upsert = TrueOrFalseEnum.TRUE.getValue().equalsIgnoreCase(xUpsert);
        UUID owner = securityUtil.getCurrentUserId();

        Map<String, String> response = storageService.createSignedUploadUrl(SignedUploadUrlRequest.builder()
                .bucketId(bucketId)
                .path(path)
                .upsert(upsert)
                .owner(owner)
                .build());
        return ResponseEntity.ok(response);
    }

    // ==================== Delete ====================

    /**
     * Delete an object.
     * DELETE /storage/v1/object/{bucketId}/{path}
     */
    @DeleteMapping("/object/{bucketId}/**")
    public ResponseEntity<Map<String, String>> deleteFile(
            @PathVariable String bucketId,
            HttpServletRequest request
    ) {
        String path = RequestUtil.extractPathVariable(request);
        storageService.deleteFile(bucketId, path);
        return ResponseEntity.ok(Map.of("message", "Successfully deleted"));
    }

    /**
     * Delete objects in bulk.
     * DELETE /storage/v1/object/{bucketId}
     */
    @DeleteMapping("/object/{bucketId}")
    public ResponseEntity<List<ObjectDTO>> deleteFiles(
            @PathVariable String bucketId,
            @RequestBody Map<String, Object> request
    ) {
        List<String> prefixes = request == null ? null : RequestUtil.toStringList(request.get("prefixes"));
        List<ObjectDTO> response = storageService.deleteFiles(bucketId, prefixes);
        return ResponseEntity.ok(response);
    }

    // ==================== Move / Copy ====================

    /**
     * Move an object.
     * POST /storage/v1/object/move
     */
    @PostMapping("/object/move")
    public ResponseEntity<Map<String, String>> moveObject(
            @RequestBody Map<String, Object> request
    ) {
        String bucketId = RequestUtil.stringValue(request.get("bucketId"));
        String sourceKey = RequestUtil.stringValue(request.get("sourceKey"));
        String destinationBucket = RequestUtil.stringValue(request.get("destinationBucket"));
        String destinationKey = RequestUtil.stringValue(request.get("destinationKey"));

        UploadResponse moved = storageService.moveObject(ObjectMoveRequest.builder()
                .bucketId(bucketId)
                .sourceKey(sourceKey)
                .destinationBucket(destinationBucket)
                .destinationKey(destinationKey)
                .owner(securityUtil.getCurrentUserId())
                .build());

        return ResponseEntity.ok(Map.of(
                "message", "Successfully moved",
                "Id", moved.getId(),
                "Key", moved.getKey()
        ));
    }

    /**
     * Copy an object.
     * POST /storage/v1/object/copy
     */
    @PostMapping("/object/copy")
    public ResponseEntity<Map<String, String>> copyObject(
            @RequestBody Map<String, Object> request,
            @RequestHeader(value = HttpHeaderConstant.X_UPSERT, required = false) String xUpsert
    ) {
        String bucketId = RequestUtil.stringValue(request.get("bucketId"));
        String sourceKey = RequestUtil.stringValue(request.get("sourceKey"));
        String destinationBucket = RequestUtil.stringValue(request.get("destinationBucket"));
        String destinationKey = RequestUtil.stringValue(request.get("destinationKey"));
        Map<String, Object> metadata = request.get("metadata") instanceof Map<?, ?> m
                ? (Map<String, Object>) m
                : null;

        boolean upsert = TrueOrFalseEnum.TRUE.getValue().equalsIgnoreCase(xUpsert);

        UploadResponse copied = storageService.copyObject(ObjectCopyRequest.builder()
                .bucketId(bucketId)
                .sourceKey(sourceKey)
                .destinationBucket(destinationBucket)
                .destinationKey(destinationKey)
                .owner(securityUtil.getCurrentUserId())
                .upsert(upsert)
                .metadataOverrides(metadata)
                .build());

        return ResponseEntity.ok(Map.of(
                "Id", copied.getId(),
                "Key", copied.getKey()
        ));
    }

}
