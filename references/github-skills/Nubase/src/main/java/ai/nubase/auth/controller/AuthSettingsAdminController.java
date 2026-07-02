package ai.nubase.auth.controller;

import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.auth.service.EffectiveAuthConfig;
import ai.nubase.common.config.TenantAuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.postgrest.multidb.DatabaseConfig;
import ai.nubase.postgrest.multidb.DatabaseConfigRepository;
import cn.hutool.json.JSONUtil;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.net.URI;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

/**
 * Per-tenant authentication settings admin API (service_role only).
 *
 * <ul>
 *   <li>{@code GET    /auth/v1/admin/settings/auth} — the EFFECTIVE settings for the tenant
 *       (per-tenant override merged over global defaults)</li>
 *   <li>{@code PUT    /auth/v1/admin/settings/auth} — store a per-tenant override
 *       (TenantAuthConfig JSON; any null group inherits the global default)</li>
 *   <li>{@code DELETE /auth/v1/admin/settings/auth} — clear the override (revert to global)</li>
 * </ul>
 *
 * <p>Persisted in {@code database_configs.auth_config} and picked up on the next request by
 * {@code UnifiedMultiTenancyFilter} → {@link EffectiveAuthConfig}.
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1/admin/settings/auth")
@RequireServiceRole
@Slf4j
public class AuthSettingsAdminController {

    private final EffectiveAuthConfig effectiveAuthConfig;
    private final DatabaseConfigRepository databaseConfigRepository;

    public record RedirectAllowListRequest(List<String> urls) {}

    /** Return the effective auth settings for the current tenant. */
    @GetMapping
    public ResponseEntity<TenantAuthConfig> get() {
        TenantAuthConfig effective = new TenantAuthConfig();
        effective.setMfa(effectiveAuthConfig.mfa());
        effective.setOtp(effectiveAuthConfig.otp());
        effective.setSms(effectiveAuthConfig.sms());
        effective.setCaptcha(effectiveAuthConfig.captcha());
        effective.setRateLimit(effectiveAuthConfig.rateLimit());
        effective.setRedirect(effectiveAuthConfig.redirect());
        effective.setPassword(effectiveAuthConfig.password());
        effective.setEmailConfirmationRequired(effectiveAuthConfig.emailConfirmationRequired());
        effective.setDisableSignup(effectiveAuthConfig.signupDisabled());
        return ResponseEntity.ok(effective);
    }

    /** Replace the per-tenant override with the supplied settings. */
    @PutMapping
    public ResponseEntity<Map<String, Object>> put(@RequestBody TenantAuthConfig request) {
        String dbKey = MultiTenancyContext.getDatabaseKey();
        if (dbKey == null) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                    .body(Map.of("error", "tenant not resolved"));
        }
        databaseConfigRepository.updateAuthConfig(dbKey, JSONUtil.toJsonStr(request));
        log.info("Updated per-tenant auth settings for dbKey={}", dbKey);
        // Note: takes effect on the NEXT request (config is loaded by the tenant filter).
        return ResponseEntity.ok(Map.of("success", true, "db_key", dbKey));
    }

    /**
     * Replace one platform-managed redirect allow-list namespace without touching manual
     * redirect settings or other platform namespaces.
     */
    @PutMapping("/redirect-allowlist/{namespace}")
    public ResponseEntity<Map<String, Object>> putManagedRedirectAllowList(
            @PathVariable String namespace,
            @RequestBody RedirectAllowListRequest request) {
        String dbKey = MultiTenancyContext.getDatabaseKey();
        if (dbKey == null) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                    .body(Map.of("error", "tenant not resolved"));
        }
        String normalizedNamespace;
        List<String> urls;
        try {
            normalizedNamespace = normalizeNamespace(namespace);
            urls = normalizeRedirectUrls(request == null ? null : request.urls());
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body(Map.of("error", e.getMessage()));
        }
        TenantAuthConfig stored = loadStored(dbKey);
        Map<String, List<String>> managed = stored.getManagedRedirectAllowLists() == null
                ? new LinkedHashMap<>()
                : new LinkedHashMap<>(stored.getManagedRedirectAllowLists());
        managed.put(normalizedNamespace, urls);
        stored.setManagedRedirectAllowLists(managed);
        databaseConfigRepository.updateAuthConfig(dbKey, JSONUtil.toJsonStr(stored));
        log.info("Updated managed redirect allow-list namespace={} for dbKey={} ({} urls)",
                normalizedNamespace, dbKey, urls.size());
        return ResponseEntity.ok(Map.of(
                "success", true,
                "db_key", dbKey,
                "namespace", normalizedNamespace,
                "urls", urls));
    }

    @GetMapping("/redirect-allowlist/{namespace}")
    public ResponseEntity<Map<String, Object>> getManagedRedirectAllowList(@PathVariable String namespace) {
        String dbKey = MultiTenancyContext.getDatabaseKey();
        if (dbKey == null) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                    .body(Map.of("error", "tenant not resolved"));
        }
        String normalizedNamespace;
        try {
            normalizedNamespace = normalizeNamespace(namespace);
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body(Map.of("error", e.getMessage()));
        }
        TenantAuthConfig stored = loadStored(dbKey);
        List<String> urls = stored.getManagedRedirectAllowLists() == null
                ? List.of()
                : stored.getManagedRedirectAllowLists().getOrDefault(normalizedNamespace, List.of());
        return ResponseEntity.ok(Map.of(
                "db_key", dbKey,
                "namespace", normalizedNamespace,
                "urls", urls));
    }

    @DeleteMapping("/redirect-allowlist/{namespace}")
    public ResponseEntity<Map<String, Object>> deleteManagedRedirectAllowList(@PathVariable String namespace) {
        String dbKey = MultiTenancyContext.getDatabaseKey();
        if (dbKey == null) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                    .body(Map.of("error", "tenant not resolved"));
        }
        String normalizedNamespace;
        try {
            normalizedNamespace = normalizeNamespace(namespace);
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body(Map.of("error", e.getMessage()));
        }
        TenantAuthConfig stored = loadStored(dbKey);
        if (stored.getManagedRedirectAllowLists() != null) {
            Map<String, List<String>> managed = new LinkedHashMap<>(stored.getManagedRedirectAllowLists());
            managed.remove(normalizedNamespace);
            stored.setManagedRedirectAllowLists(managed.isEmpty() ? null : managed);
            databaseConfigRepository.updateAuthConfig(dbKey, JSONUtil.toJsonStr(stored));
        }
        log.info("Deleted managed redirect allow-list namespace={} for dbKey={}", normalizedNamespace, dbKey);
        return ResponseEntity.ok(Map.of(
                "success", true,
                "db_key", dbKey,
                "namespace", normalizedNamespace));
    }

    /** Clear the per-tenant override; the tenant reverts to global defaults. */
    @DeleteMapping
    public ResponseEntity<Map<String, Object>> clear() {
        String dbKey = MultiTenancyContext.getDatabaseKey();
        if (dbKey == null) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                    .body(Map.of("error", "tenant not resolved"));
        }
        databaseConfigRepository.updateAuthConfig(dbKey, null);
        log.info("Cleared per-tenant auth settings for dbKey={}", dbKey);
        return ResponseEntity.ok(Map.of("success", true, "db_key", dbKey));
    }

    private TenantAuthConfig loadStored(String dbKey) {
        DatabaseConfig cfg = databaseConfigRepository.findByDbKey(dbKey);
        String json = cfg != null ? cfg.getAuthConfigJson() : null;
        if (StringUtils.isBlank(json)) {
            return new TenantAuthConfig();
        }
        try {
            return JSONUtil.parseObj(json).toBean(TenantAuthConfig.class);
        } catch (Exception e) {
            log.warn("Failed to parse stored auth_config for dbKey={}, starting fresh: {}", dbKey, e.getMessage());
            return new TenantAuthConfig();
        }
    }

    private String normalizeNamespace(String namespace) {
        if (StringUtils.isBlank(namespace) || !namespace.matches("[A-Za-z0-9_-]{1,64}")) {
            throw new IllegalArgumentException("Invalid redirect allow-list namespace");
        }
        return namespace;
    }

    private List<String> normalizeRedirectUrls(List<String> urls) {
        if (urls == null) {
            return List.of();
        }
        LinkedHashSet<String> normalized = new LinkedHashSet<>();
        for (String value : urls) {
            if (StringUtils.isBlank(value)) {
                continue;
            }
            URI uri;
            try {
                uri = URI.create(value.trim());
            } catch (Exception e) {
                throw new IllegalArgumentException("Invalid redirect URL: " + value);
            }
            String scheme = uri.getScheme();
            String host = uri.getHost();
            if (StringUtils.isBlank(scheme)
                    || StringUtils.isBlank(host)
                    || (!"http".equalsIgnoreCase(scheme) && !"https".equalsIgnoreCase(scheme))) {
                throw new IllegalArgumentException("Redirect URL must be absolute http/https: " + value);
            }
            if (!"/auth/callback".equals(uri.getPath())) {
                throw new IllegalArgumentException("Redirect URL path must be /auth/callback: " + value);
            }
            URI clean = URI.create(uri.getScheme().toLowerCase() + "://" + uri.getRawAuthority()
                    + "/auth/callback");
            normalized.add(clean.toString());
        }
        return new ArrayList<>(normalized);
    }
}
