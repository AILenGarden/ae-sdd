package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.EmbeddingProvider;
import ai.nubase.mem.llm.LLMException;
import ai.nubase.mem.llm.LLMProviderRegistry;
import com.github.benmanes.caffeine.cache.Cache;
import com.github.benmanes.caffeine.cache.Caffeine;
import jakarta.annotation.PostConstruct;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Wraps the configured {@link EmbeddingProvider} with dimension validation and an
 * in-process Caffeine cache keyed by {@code SHA-256(model + ":" + text)}.
 *
 * <p>The cache is content-addressed — same text under same model always yields the same
 * vector — so it is safe to share across tenants. Stored vectors are never tenant-tagged.
 */
@Slf4j
@Service
public class EmbeddingService {

    private final LLMProviderRegistry providers;
    private final MemProperties memProperties;

    /** Null when caching is disabled. */
    private Cache<String, float[]> cache;

    public EmbeddingService(LLMProviderRegistry providers, MemProperties memProperties) {
        this.providers = providers;
        this.memProperties = memProperties;
    }

    @PostConstruct
    void initCache() {
        MemProperties.Cache cfg = memProperties.getEmbedding().getCache();
        if (cfg.isEnabled()) {
            this.cache = Caffeine.newBuilder()
                    .maximumSize(cfg.getMaximumSize())
                    .expireAfterWrite(Duration.ofMinutes(cfg.getTtlMinutes()))
                    .recordStats()
                    .build();
            log.info("Embedding cache enabled: maxSize={}, ttlMinutes={}",
                    cfg.getMaximumSize(), cfg.getTtlMinutes());
        } else {
            log.info("Embedding cache disabled");
        }
    }

    public int dimensions() {
        return providers.embedding().dimensions();
    }

    public float[] embed(String text) {
        EmbeddingProvider p = providers.embedding();
        ensureAvailable(p);
        if (cache == null) {
            return validateDims(p, p.embed(text));
        }
        String key = cacheKey(p, text);
        float[] cached = cache.getIfPresent(key);
        if (cached != null) {
            return cached;
        }
        float[] vec = validateDims(p, p.embed(text));
        cache.put(key, vec);
        return vec;
    }

    public List<float[]> embedBatch(List<String> texts) {
        EmbeddingProvider p = providers.embedding();
        if (texts == null || texts.isEmpty()) {
            return List.of();
        }
        ensureAvailable(p);
        if (cache == null) {
            List<float[]> out = p.embedBatch(texts);
            for (float[] v : out) validateDims(p, v);
            return out;
        }

        // Look up each text in cache; collect only the misses for a batch upstream call.
        float[][] results = new float[texts.size()][];
        List<Integer> missIndices = new ArrayList<>();
        List<String> missTexts = new ArrayList<>();
        Map<String, Integer> firstOccurrence = new HashMap<>();
        for (int i = 0; i < texts.size(); i++) {
            String t = texts.get(i);
            String key = cacheKey(p, t);
            float[] hit = cache.getIfPresent(key);
            if (hit != null) {
                results[i] = hit;
            } else if (firstOccurrence.containsKey(key)) {
                // Same text appears earlier in the batch — reuse that slot once it's filled.
                missIndices.add(i);
                missTexts.add(null); // placeholder so misses align with missTexts
            } else {
                firstOccurrence.put(key, i);
                missIndices.add(i);
                missTexts.add(t);
            }
        }

        // Strip the placeholder nulls before calling upstream.
        List<String> upstreamTexts = new ArrayList<>();
        List<Integer> upstreamSourceIdx = new ArrayList<>();
        for (int j = 0; j < missTexts.size(); j++) {
            String t = missTexts.get(j);
            if (t != null) {
                upstreamTexts.add(t);
                upstreamSourceIdx.add(missIndices.get(j));
            }
        }

        if (!upstreamTexts.isEmpty()) {
            List<float[]> fresh = p.embedBatch(upstreamTexts);
            if (fresh.size() != upstreamTexts.size()) {
                throw new LLMException("Embedding batch returned " + fresh.size()
                        + " vectors for " + upstreamTexts.size() + " inputs");
            }
            for (int j = 0; j < upstreamTexts.size(); j++) {
                int origIdx = upstreamSourceIdx.get(j);
                float[] v = validateDims(p, fresh.get(j));
                results[origIdx] = v;
                cache.put(cacheKey(p, upstreamTexts.get(j)), v);
            }
        }

        // Fill duplicate-within-batch slots from cache (now populated for those texts).
        for (int i = 0; i < texts.size(); i++) {
            if (results[i] == null) {
                results[i] = cache.getIfPresent(cacheKey(p, texts.get(i)));
                if (results[i] == null) {
                    throw new LLMException("Embedding cache miss after population — should never happen");
                }
            }
        }
        return new ArrayList<>(java.util.Arrays.asList(results));
    }

    /** Drop every cached entry. Intended for tests and admin reset. */
    public void invalidateCache() {
        if (cache != null) {
            cache.invalidateAll();
        }
    }

    /** Visible to tests; null if caching is disabled. */
    Cache<String, float[]> rawCache() {
        return cache;
    }

    private static float[] validateDims(EmbeddingProvider provider, float[] vec) {
        if (vec.length != provider.dimensions()) {
            throw new LLMException("Embedding dimension mismatch: provider returned " + vec.length
                    + " but configuration declares " + provider.dimensions()
                    + ". Likely causes: (a) wrong embedding model selected, or (b) "
                    + "nubase.mem.embedding.dimensions does not match the model's native size. "
                    + "Note: the DB column dimension is baked in at tenant init time — "
                    + "changing it requires re-init or a manual ALTER TABLE.");
        }
        return vec;
    }

    /**
     * Pre-flight: surface a clear "provider not configured" error instead of letting the
     * upstream call fail with an opaque 401. Memory ingestion and search both require
     * embeddings, so we cannot degrade gracefully here — fail fast.
     */
    private static void ensureAvailable(EmbeddingProvider provider) {
        if (!provider.isAvailable()) {
            throw new LLMException("Embedding provider '" + provider.name()
                    + "' is not configured. Set the corresponding API key "
                    + "(e.g. OPENAI_API_KEY) before calling memory APIs.");
        }
    }

    private static String cacheKey(EmbeddingProvider provider, String text) {
        return sha256(provider.name() + "::" + provider.dimensions() + "::" + (text == null ? "" : text));
    }

    private static String sha256(String s) {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] d = md.digest(s.getBytes(StandardCharsets.UTF_8));
            StringBuilder sb = new StringBuilder(d.length * 2);
            for (byte b : d) {
                sb.append(String.format("%02x", b));
            }
            return sb.toString();
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256 unavailable", e);
        }
    }
}
