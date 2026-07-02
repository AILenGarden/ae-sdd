package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.EmbeddingProvider;
import ai.nubase.mem.llm.LLMException;
import ai.nubase.mem.llm.LLMProviderRegistry;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

/**
 * Behaviour tests for {@link EmbeddingService}'s Caffeine cache:
 * <ul>
 *   <li>Hits avoid upstream calls.</li>
 *   <li>Batches deduplicate within a single call and reuse populated entries afterward.</li>
 *   <li>Dimension mismatches are rejected.</li>
 *   <li>Disabling the cache makes every call upstream.</li>
 * </ul>
 */
class EmbeddingServiceCacheTest {

    /** Counts upstream invocations so we can assert hits vs. misses. */
    static class CountingProvider implements EmbeddingProvider {
        final AtomicInteger embedCalls = new AtomicInteger();
        final AtomicInteger batchCalls = new AtomicInteger();
        final AtomicInteger batchTotalTexts = new AtomicInteger();
        final int dims;
        volatile boolean available = true;

        CountingProvider(int dims) {
            this.dims = dims;
        }

        @Override public String name() { return "counting"; }
        @Override public boolean isAvailable() { return available; }
        @Override public int dimensions() { return dims; }

        @Override
        public float[] embed(String text) {
            embedCalls.incrementAndGet();
            return vectorFor(text);
        }

        @Override
        public List<float[]> embedBatch(List<String> texts) {
            batchCalls.incrementAndGet();
            batchTotalTexts.addAndGet(texts.size());
            List<float[]> out = new ArrayList<>(texts.size());
            for (String t : texts) out.add(vectorFor(t));
            return out;
        }

        private float[] vectorFor(String text) {
            float[] v = new float[dims];
            // Deterministic but text-dependent vector: each dim = hashCode + index, no leading zeros surprise.
            for (int i = 0; i < dims; i++) {
                v[i] = (float) (text.hashCode() + i);
            }
            return v;
        }
    }

    private MemProperties props;
    private CountingProvider provider;
    private LLMProviderRegistry registry;

    @BeforeEach
    void setUp() {
        props = new MemProperties();
        props.getEmbedding().setDimensions(4);
        provider = new CountingProvider(4);
        registry = mock(LLMProviderRegistry.class);
        when(registry.embedding()).thenReturn(provider);
    }

    private EmbeddingService newService(boolean cacheEnabled) {
        props.getEmbedding().getCache().setEnabled(cacheEnabled);
        props.getEmbedding().getCache().setMaximumSize(1024);
        props.getEmbedding().getCache().setTtlMinutes(60);
        EmbeddingService svc = new EmbeddingService(registry, props);
        svc.initCache();
        return svc;
    }

    @Test
    void singleEmbed_secondCallHitsCache() {
        EmbeddingService svc = newService(true);

        float[] v1 = svc.embed("hello");
        float[] v2 = svc.embed("hello");

        assertThat(v1).containsExactly(v2);
        assertThat(provider.embedCalls.get()).isEqualTo(1);
    }

    @Test
    void singleEmbed_differentTextsHitProviderEach() {
        EmbeddingService svc = newService(true);
        svc.embed("a");
        svc.embed("b");
        assertThat(provider.embedCalls.get()).isEqualTo(2);
    }

    @Test
    void batch_onlyMissesGoUpstream() {
        EmbeddingService svc = newService(true);
        svc.embed("a"); // pre-warm
        provider.batchCalls.set(0);
        provider.batchTotalTexts.set(0);

        List<float[]> result = svc.embedBatch(List.of("a", "b", "c"));

        assertThat(result).hasSize(3);
        assertThat(provider.batchCalls.get()).isEqualTo(1);
        // Only "b" and "c" should hit upstream; "a" was cached
        assertThat(provider.batchTotalTexts.get()).isEqualTo(2);
    }

    @Test
    void batch_dedupesWithinSingleCall() {
        EmbeddingService svc = newService(true);

        List<float[]> result = svc.embedBatch(List.of("x", "y", "x", "z", "y"));

        assertThat(result).hasSize(5);
        // Upstream should see exactly 3 distinct texts
        assertThat(provider.batchCalls.get()).isEqualTo(1);
        assertThat(provider.batchTotalTexts.get()).isEqualTo(3);
        // Duplicated slots return identical arrays
        assertThat(result.get(0)).containsExactly(result.get(2));
        assertThat(result.get(1)).containsExactly(result.get(4));
    }

    @Test
    void batch_allCachedSkipsUpstreamEntirely() {
        EmbeddingService svc = newService(true);
        svc.embedBatch(List.of("a", "b")); // populate
        provider.batchCalls.set(0);
        provider.batchTotalTexts.set(0);

        List<float[]> result = svc.embedBatch(List.of("a", "b", "a"));

        assertThat(result).hasSize(3);
        assertThat(provider.batchCalls.get()).isEqualTo(0); // no upstream call at all
    }

    @Test
    void cacheDisabled_everyCallGoesUpstream() {
        EmbeddingService svc = newService(false);

        svc.embed("a");
        svc.embed("a");

        assertThat(provider.embedCalls.get()).isEqualTo(2);
        assertThat(svc.rawCache()).isNull();
    }

    @Test
    void rejectsDimensionMismatchFromProvider() {
        EmbeddingProvider wrong = new EmbeddingProvider() {
            @Override public String name() { return "wrong"; }
            @Override public boolean isAvailable() { return true; }
            @Override public int dimensions() { return 4; }
            @Override public float[] embed(String text) { return new float[3]; }
        };
        when(registry.embedding()).thenReturn(wrong);
        EmbeddingService svc = newService(true);

        assertThatThrownBy(() -> svc.embed("hi"))
                .isInstanceOf(LLMException.class)
                .hasMessageContaining("dimension mismatch");
    }

    @Test
    void invalidateCache_dropsAll() {
        EmbeddingService svc = newService(true);
        svc.embed("a");
        svc.invalidateCache();
        svc.embed("a");
        assertThat(provider.embedCalls.get()).isEqualTo(2);
    }

    @Test
    void emptyBatchReturnsEmpty() {
        EmbeddingService svc = newService(true);
        assertThat(svc.embedBatch(List.of())).isEmpty();
        assertThat(provider.batchCalls.get()).isEqualTo(0);
    }

    @Test
    void providerUnavailable_embedSingleThrowsClearMessage() {
        provider.available = false;
        EmbeddingService svc = newService(true);

        assertThatThrownBy(() -> svc.embed("anything"))
                .isInstanceOf(LLMException.class)
                .hasMessageContaining("not configured")
                .hasMessageContaining("counting");
    }

    @Test
    void providerUnavailable_embedBatchThrowsClearMessage() {
        provider.available = false;
        EmbeddingService svc = newService(true);

        assertThatThrownBy(() -> svc.embedBatch(List.of("a", "b")))
                .isInstanceOf(LLMException.class)
                .hasMessageContaining("not configured");
    }
}
