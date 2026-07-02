package ai.nubase.functions.service;

import ai.nubase.metadata.edge.entity.EdgeFunction;
import ai.nubase.metadata.edge.entity.EdgeFunctionSecret;
import ai.nubase.metadata.edge.repository.EdgeFunctionSecretRepository;
import ai.nubase.postgrest.multidb.EncryptionService;
import com.github.benmanes.caffeine.cache.Cache;
import com.github.benmanes.caffeine.cache.Caffeine;
import lombok.RequiredArgsConstructor;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Component;

import java.time.Duration;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

import static ai.nubase.functions.service.EdgeFunctionExceptions.EdgeFunctionException;

/**
 * Loads and decrypts a function's secrets into an env map. Shared by the admin
 * (deploy/sync) and invocation (local executor env injection) paths.
 *
 * The invocation path calls this on every local-executor request; without a
 * cache that is one metadata query plus an AES decrypt per secret per request
 * for values that change ~never. setSecrets evicts on write, and the short TTL
 * bounds staleness across instances.
 */
@Component
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.functions.enabled", havingValue = "true", matchIfMissing = true)
public class EdgeFunctionSecretEnv {

    private final EdgeFunctionSecretRepository secretRepository;
    private final EncryptionService encryptionService;

    private final Cache<UUID, Map<String, String>> cache = Caffeine.newBuilder()
            .expireAfterWrite(Duration.ofSeconds(30))
            .maximumSize(10_000)
            .build();

    public Map<String, String> decryptedEnv(EdgeFunction fn) {
        if (fn.getId() == null) {
            return loadAndDecrypt(fn);
        }
        return cache.get(fn.getId(), ignored -> loadAndDecrypt(fn));
    }

    /** Call after any secret write so same-instance readers see it immediately. */
    public void evict(UUID functionId) {
        if (functionId != null) {
            cache.invalidate(functionId);
        }
    }

    private Map<String, String> loadAndDecrypt(EdgeFunction fn) {
        Map<String, String> env = new LinkedHashMap<>();
        for (EdgeFunctionSecret secret : secretRepository.findByFunctionOrderByNameAsc(fn)) {
            try {
                env.put(secret.getName(), encryptionService.decrypt(secret.getEncryptedValue()));
            } catch (Exception e) {
                throw new EdgeFunctionException(HttpStatus.INTERNAL_SERVER_ERROR, "SECRET_DECRYPTION_FAILED", "Failed to decrypt function secret");
            }
        }
        return Collections.unmodifiableMap(env);
    }
}
