package ai.nubase.auth.controller;

import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.auth.service.EmailTemplateService;
import ai.nubase.auth.service.EffectiveAuthConfig;
import ai.nubase.common.config.EmailTemplate;
import ai.nubase.common.config.TenantAuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.postgrest.multidb.DatabaseConfig;
import ai.nubase.postgrest.multidb.DatabaseConfigRepository;
import cn.hutool.json.JSONUtil;
import lombok.Data;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Per-tenant email template editor (service_role only).
 *
 * <ul>
 *   <li>{@code GET  /auth/v1/admin/settings/email-templates} — effective templates (tenant
 *       override or built-in default) + the variables available per type.</li>
 *   <li>{@code PUT  /auth/v1/admin/settings/email-templates} — store template overrides
 *       (read-modify-write on auth_config, preserving the other auth settings).</li>
 * </ul>
 *
 * Templates live in {@code TenantAuthConfig.emailTemplates}; an absent/blank type falls back
 * to the built-in default in {@link EmailTemplateService}.
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1/admin/settings/email-templates")
@RequireServiceRole
@Slf4j
public class EmailTemplateAdminController {

    private final EmailTemplateService emailTemplateService;
    private final EffectiveAuthConfig effectiveAuthConfig;
    private final DatabaseConfigRepository databaseConfigRepository;

    @Data
    public static class EmailTemplatesRequest {
        private Map<String, EmailTemplate> templates;
    }

    @GetMapping
    public ResponseEntity<Map<String, Object>> get() {
        Map<String, EmailTemplate> templates = new LinkedHashMap<>();
        Map<String, Object> variables = new LinkedHashMap<>();
        Map<String, Boolean> customized = new LinkedHashMap<>();
        for (String type : EmailTemplateService.TYPES) {
            templates.put(type, emailTemplateService.effective(type));
            variables.put(type, emailTemplateService.variablesFor(type));
            customized.put(type, effectiveAuthConfig.emailTemplateOverride(type) != null);
        }
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("types", EmailTemplateService.TYPES);
        body.put("templates", templates);
        body.put("variables", variables);
        body.put("customized", customized);
        return ResponseEntity.ok(body);
    }

    @PutMapping
    public ResponseEntity<Map<String, Object>> put(@RequestBody EmailTemplatesRequest request) {
        String dbKey = MultiTenancyContext.getDatabaseKey();
        if (dbKey == null) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST).body(Map.of("error", "tenant not resolved"));
        }

        // Read-modify-write: preserve the other auth_config groups.
        TenantAuthConfig current = loadStored(dbKey);

        Map<String, EmailTemplate> overrides = new HashMap<>();
        if (request.getTemplates() != null) {
            request.getTemplates().forEach((type, tpl) -> {
                if (tpl != null
                        && StringUtils.isNotBlank(tpl.getSubject())
                        && StringUtils.isNotBlank(tpl.getContent())) {
                    overrides.put(type, tpl);
                }
                // blank subject/content → omit → reverts that type to the built-in default
            });
        }
        current.setEmailTemplates(overrides.isEmpty() ? null : overrides);

        databaseConfigRepository.updateAuthConfig(dbKey, JSONUtil.toJsonStr(current));
        log.info("Updated email templates for dbKey={} ({} custom)", dbKey, overrides.size());
        return ResponseEntity.ok(Map.of("success", true, "db_key", dbKey, "customized", overrides.size()));
    }

    /** Load the tenant's stored override document (not the effective merge), or an empty one. */
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
}
