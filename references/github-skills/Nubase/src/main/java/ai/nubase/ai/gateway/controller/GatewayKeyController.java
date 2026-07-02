package ai.nubase.ai.gateway.controller;

import ai.nubase.ai.gateway.entity.ApiKey;
import ai.nubase.ai.gateway.repository.ApiKeyRepository;
import ai.nubase.ai.gateway.util.GatewayKeyUtil;
import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.common.context.MultiTenancyContext;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 项目级网关密钥管理（控制面）。
 * <p>签发自路由密钥 {@code nbk_<appCode>_<secret>}：完整密钥只在创建时返回一次，库内仅存其哈希。
 */
@Slf4j
@RestController
@RequestMapping("/ai-gateway/admin/v1/keys")
@RequiredArgsConstructor
@RequireServiceRole
public class GatewayKeyController {

    private static final int SECRET_LENGTH = 48;

    private final ApiKeyRepository apiKeyRepository;

    /** 列出当前项目的网关密钥（不含明文与哈希）。 */
    @GetMapping
    public ResponseEntity<Map<String, Object>> list() {
        List<Map<String, Object>> data = new ArrayList<>();
        for (ApiKey k : apiKeyRepository.findAll()) {
            data.add(toDto(k));
        }
        return ResponseEntity.ok(Map.of("data", data));
    }

    /**
     * 签发一个新的网关密钥，完整密钥仅此一次返回。
     * Body: {"name": "...", "description": "...", "expiresAt": "2026-12-31T00:00:00"(可选)}
     */
    @PostMapping
    public ResponseEntity<Map<String, Object>> issue(@RequestBody(required = false) Map<String, Object> body) {
        String appCode = MultiTenancyContext.getAppCode();
        if (appCode == null || appCode.isBlank()) {
            return ResponseEntity.status(400).body(Map.of("error", "no project context"));
        }
        Map<String, Object> b = body == null ? Map.of() : body;
        String name = b.get("name") == null ? "Untitled key" : String.valueOf(b.get("name"));
        String description = b.get("description") == null ? null : String.valueOf(b.get("description"));

        String fullKey = GatewayKeyUtil.generate(appCode, SECRET_LENGTH);
        ApiKey k = ApiKey.builder()
                .keyHash(GatewayKeyUtil.sha256Hex(fullKey))
                .keyPrefix(GatewayKeyUtil.displayPrefix(fullKey))
                .name(name)
                .description(description)
                .scope("all")
                .isActive(true)
                .build();
        if (b.get("expiresAt") != null) {
            try {
                k.setExpiresAt(LocalDateTime.parse(String.valueOf(b.get("expiresAt"))));
            } catch (Exception ignored) {
                // ignore malformed expiry; key has no expiry
            }
        }
        apiKeyRepository.save(k);

        Map<String, Object> dto = toDto(k);
        // 完整密钥仅返回这一次
        dto.put("apiKey", fullKey);
        return ResponseEntity.ok(dto);
    }

    /** 切换启用状态（禁用而非删除，历史用量保留）。 Body: {"is_active": true|false} */
    @PatchMapping("/{id}")
    public ResponseEntity<Map<String, Object>> toggle(@PathVariable Long id, @RequestBody Map<String, Object> body) {
        return apiKeyRepository.findById(id).map(k -> {
            Object isActive = body.get("is_active");
            if (isActive instanceof Boolean active) {
                k.setIsActive(active);
                apiKeyRepository.save(k);
            }
            return ResponseEntity.ok(toDto(k));
        }).orElseGet(() -> ResponseEntity.status(404).body(Map.of("error", "not found")));
    }

    /** 吊销密钥（标记 revoked 并禁用）。 */
    @DeleteMapping("/{id}")
    public ResponseEntity<Map<String, Object>> revoke(@PathVariable Long id) {
        return apiKeyRepository.findById(id).map(k -> {
            k.setIsActive(false);
            k.setRevokedAt(LocalDateTime.now());
            apiKeyRepository.save(k);
            return ResponseEntity.ok(Map.<String, Object>of("ok", true));
        }).orElseGet(() -> ResponseEntity.status(404).body(Map.of("error", "not found")));
    }

    private Map<String, Object> toDto(ApiKey k) {
        Map<String, Object> dto = new LinkedHashMap<>();
        dto.put("id", k.getId());
        dto.put("name", k.getName());
        dto.put("prefix", k.getKeyPrefix());
        dto.put("isActive", Boolean.TRUE.equals(k.getIsActive()));
        dto.put("createdAt", k.getCreatedAt());
        dto.put("lastUsedAt", k.getLastUsedAt());
        dto.put("expiresAt", k.getExpiresAt());
        dto.put("revokedAt", k.getRevokedAt());
        return dto;
    }
}
