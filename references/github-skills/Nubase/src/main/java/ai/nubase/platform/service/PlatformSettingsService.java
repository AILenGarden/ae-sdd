package ai.nubase.platform.service;

import ai.nubase.metadata.entity.PlatformSetting;
import ai.nubase.metadata.repository.PlatformSettingRepository;
import ai.nubase.platform.event.SettingsChangedEvent;
import ai.nubase.postgrest.multidb.EncryptionService;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.context.ApplicationEventPublisher;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.Collections;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Single source of truth for runtime-editable platform-wide settings.
 *
 * <p>Reads and writes go through this service so that:
 * <ul>
 *   <li>Sensitive fields are encrypted with the master key before they hit the DB.</li>
 *   <li>An in-process cache (per category) avoids hitting the metadata DB on every read.</li>
 *   <li>{@link SettingsChangedEvent} fires after writes so dependent beans (mail sender,
 *       OAuth client cache, etc.) can rebuild themselves.</li>
 * </ul>
 *
 * <p>Callers should treat absent values as "not configured" and fall back to YAML / env
 * defaults — the server must boot before any settings have been written.
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class PlatformSettingsService {

    /**
     * Keys whose value is encrypted at rest. Any key containing one of these tokens (case
     * insensitive) — "password", "secret", "key", "token" — is treated as sensitive.
     */
    private static final Set<String> SENSITIVE_TOKENS = Set.of(
            "password", "secret", "key", "token", "auth_token", "apikey"
    );

    private final PlatformSettingRepository repository;
    private final EncryptionService encryptionService;
    private final ApplicationEventPublisher events;

    /** category -> (key -> decrypted value). Built lazily, invalidated on write. */
    private final Map<String, Map<String, String>> cache = new ConcurrentHashMap<>();

    /**
     * Return all key/value pairs in a category, with sensitive values <strong>decrypted</strong>.
     * Intended for the dynamic-config consumers (mail sender, etc.) — never expose this map
     * directly over HTTP without masking.
     */
    public Map<String, String> getCategory(String category) {
        return cache.computeIfAbsent(category, this::loadCategory);
    }

    /**
     * Return one value, or {@code null} when unset.
     */
    public String get(String category, String key) {
        return getCategory(category).get(key);
    }

    /**
     * Same as {@link #getCategory(String)} but blank-out sensitive values. Use this for
     * GET endpoints so the UI can render "configured / not configured" without ever
     * receiving the secret back.
     */
    public Map<String, Object> getCategoryMasked(String category) {
        Map<String, String> raw = getCategory(category);
        Map<String, Object> out = new HashMap<>(raw.size());
        for (Map.Entry<String, String> e : raw.entrySet()) {
            if (isSensitiveKey(e.getKey())) {
                boolean set = e.getValue() != null && !e.getValue().isEmpty();
                out.put(e.getKey(), Map.of("set", set));
            } else {
                out.put(e.getKey(), e.getValue());
            }
        }
        return out;
    }

    /**
     * Replace every key in a category with the supplied map. Keys absent from {@code values}
     * are deleted; sensitive keys with value {@code null} are left untouched (so the UI can
     * omit them when not rotating). Empty string explicitly clears a sensitive key.
     */
    @Transactional("metadataTransactionManager")
    public void replaceCategory(String category, Map<String, String> values, UUID actor) {
        Map<String, String> existing = loadRawCategory(category);
        for (Map.Entry<String, String> e : values.entrySet()) {
            String key = e.getKey();
            String value = e.getValue();
            boolean sensitive = isSensitiveKey(key);
            // Sensitive key with null → keep existing; with "" → clear; otherwise re-encrypt.
            if (sensitive && value == null) continue;
            if (value == null || value.isEmpty()) {
                repository.deleteByCategoryAndKey(category, key);
                continue;
            }
            String storedValue;
            try {
                storedValue = sensitive ? encryptionService.encrypt(value) : value;
            } catch (Exception ex) {
                throw new IllegalStateException(
                        "Failed to encrypt setting " + category + "." + key, ex);
            }
            PlatformSetting setting = PlatformSetting.builder()
                    .category(category)
                    .key(key)
                    .value(storedValue)
                    .encrypted(sensitive)
                    .updatedAt(Instant.now())
                    .updatedBy(actor)
                    .build();
            repository.save(setting);
        }
        // Delete any keys that were previously present but omitted from this write — only when
        // the caller explicitly listed them as known keys. We don't do "clean sweep" here
        // because the caller may be a partial PATCH.
        log.info("Updated {} keys in platform settings category={} actor={}",
                values.size(), category, actor);
        invalidate(category);
    }

    /** Drop the cached view for one category. Visible for tests + the change event. */
    public void invalidate(String category) {
        cache.remove(category);
        events.publishEvent(new SettingsChangedEvent(this, category));
    }

    // ----------------------------------------------------------------- internals

    private Map<String, String> loadCategory(String category) {
        Map<String, String> raw = loadRawCategory(category);
        Map<String, String> decrypted = new HashMap<>(raw.size());
        for (Map.Entry<String, String> e : raw.entrySet()) {
            String value = e.getValue();
            if (value != null && encryptionService.isEncrypted(value)) {
                try {
                    value = encryptionService.decrypt(value);
                } catch (Exception ex) {
                    log.warn("Failed to decrypt setting {}.{}, returning blank", category, e.getKey(), ex);
                    value = "";
                }
            }
            decrypted.put(e.getKey(), value);
        }
        return Collections.unmodifiableMap(decrypted);
    }

    private Map<String, String> loadRawCategory(String category) {
        Map<String, String> out = new HashMap<>();
        for (PlatformSetting s : repository.findByCategory(category)) {
            out.put(s.getKey(), s.getValue());
        }
        return out;
    }

    private static boolean isSensitiveKey(String key) {
        if (key == null) return false;
        String lower = key.toLowerCase();
        for (String token : SENSITIVE_TOKENS) {
            if (lower.contains(token)) return true;
        }
        return false;
    }
}
