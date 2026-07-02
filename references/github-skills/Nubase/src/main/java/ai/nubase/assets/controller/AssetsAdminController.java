package ai.nubase.assets.controller;

import ai.nubase.assets.dto.AssetFileDTO;
import ai.nubase.assets.dto.AssetSettingsDTO;
import ai.nubase.assets.dto.UpdateAssetSettingsRequest;
import ai.nubase.assets.service.AssetsExceptions;
import ai.nubase.assets.service.AssetsService;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.common.util.RequestUtil;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.io.IOException;
import java.util.List;

/**
 * Assets control plane: per-project static asset management.
 * Requires the project's service_role apikey (tenant context comes from
 * UnifiedMultiTenancyFilter).
 * Base path: /assets/admin/v1
 */
@Slf4j
@RestController
@RequiredArgsConstructor
@RequestMapping("/assets/admin/v1")
@ConditionalOnProperty(value = "nubase.assets.enabled", havingValue = "true", matchIfMissing = true)
public class AssetsAdminController {

    private final AssetsService assetsService;

    @Value("${nubase.assets.max-file-size:26214400}")
    private long maxFileSize;

    // ==================== Files ====================

    /**
     * List assets.
     * GET /assets/admin/v1/files?prefix=&search=&limit=&offset=
     */
    @GetMapping("/files")
    public ResponseEntity<List<AssetFileDTO>> listFiles(
            @RequestParam(required = false) String prefix,
            @RequestParam(required = false) String search,
            @RequestParam(required = false) Integer limit,
            @RequestParam(required = false) Integer offset
    ) {
        requireServiceRole();
        return ResponseEntity.ok(assetsService.list(prefix, search, limit, offset));
    }

    /**
     * Upload an asset (create; 409 when the path already exists).
     * POST /assets/admin/v1/files/{path}  — raw request body, Content-Type header honored.
     */
    @PostMapping("/files/**")
    public ResponseEntity<AssetFileDTO> createFile(
            @RequestParam(required = false) String cacheControl,
            HttpServletRequest request
    ) throws IOException {
        return upload(request, cacheControl, false);
    }

    /**
     * Upload an asset (upsert).
     * PUT /assets/admin/v1/files/{path}  — raw request body, Content-Type header honored.
     */
    @PutMapping("/files/**")
    public ResponseEntity<AssetFileDTO> upsertFile(
            @RequestParam(required = false) String cacheControl,
            HttpServletRequest request
    ) throws IOException {
        return upload(request, cacheControl, true);
    }

    /**
     * Delete an asset.
     * DELETE /assets/admin/v1/files/{path}
     */
    @DeleteMapping("/files/**")
    public ResponseEntity<Void> deleteFile(HttpServletRequest request) {
        requireServiceRole();
        assetsService.delete(RequestUtil.extractPathVariable(request));
        return ResponseEntity.noContent().build();
    }

    // ==================== Settings ====================

    /**
     * Get the project's asset delivery settings.
     * GET /assets/admin/v1/settings
     */
    @GetMapping("/settings")
    public ResponseEntity<AssetSettingsDTO> getSettings() {
        requireServiceRole();
        return ResponseEntity.ok(assetsService.getSettings());
    }

    /**
     * Update the project's asset delivery settings.
     * PATCH /assets/admin/v1/settings
     */
    @PatchMapping("/settings")
    public ResponseEntity<AssetSettingsDTO> updateSettings(@RequestBody UpdateAssetSettingsRequest body) {
        requireServiceRole();
        return ResponseEntity.ok(assetsService.updateSettings(body));
    }

    // ==================== Helpers ====================

    private ResponseEntity<AssetFileDTO> upload(HttpServletRequest request, String cacheControl,
                                                boolean upsert) throws IOException {
        requireServiceRole();
        String path = RequestUtil.extractPathVariable(request);
        byte[] body;
        try {
            // Streams with an early size-limit abort instead of buffering an
            // arbitrarily large body before checking it.
            body = RequestUtil.readRawRequestBody(request, maxFileSize);
        } catch (IllegalArgumentException e) {
            throw AssetsExceptions.tooLarge(maxFileSize);
        }
        AssetFileDTO dto = assetsService.upload(path, body, request.getContentType(), cacheControl, upsert);
        return ResponseEntity.ok(dto);
    }

    private void requireServiceRole() {
        if (!MultiTenancyContext.isServiceRole()) {
            throw AssetsExceptions.forbidden();
        }
    }
}
