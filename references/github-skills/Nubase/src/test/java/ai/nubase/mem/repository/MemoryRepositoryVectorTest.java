package ai.nubase.mem.repository;

import org.junit.jupiter.api.Test;

import java.lang.reflect.Method;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * Tests the static pgvector serialization helpers in {@link MemoryRepository}.
 *
 * <p>These methods carry the highest blast-radius of any bug — a malformed vector
 * string corrupts every insert and search — so we lock the contract here.
 */
class MemoryRepositoryVectorTest {

    private String serialize(float[] v) throws Exception {
        Method m = MemoryRepository.class.getDeclaredMethod("serializeVector", float[].class);
        m.setAccessible(true);
        return (String) m.invoke(null, (Object) v);
    }

    private float[] parse(String s) throws Exception {
        Method m = MemoryRepository.class.getDeclaredMethod("parseVector", String.class);
        m.setAccessible(true);
        return (float[]) m.invoke(null, s);
    }

    @Test
    void roundTrip_threeValues() throws Exception {
        float[] in = {0.1f, -0.2f, 3.14159f};
        String s = serialize(in);
        assertEquals("[0.1,-0.2,3.14159]", s);
        float[] out = parse(s);
        assertArrayEquals(in, out, 1e-6f);
    }

    @Test
    void roundTrip_emptyVector() throws Exception {
        float[] in = {};
        String s = serialize(in);
        assertEquals("[]", s);
        float[] out = parse(s);
        assertEquals(0, out.length);
    }

    @Test
    void serializeNull_returnsNull() throws Exception {
        assertNull(serialize(null));
    }

    @Test
    void parseNull_returnsNull() throws Exception {
        assertNull(parse(null));
    }

    @Test
    void roundTrip_1536DimVector() throws Exception {
        float[] in = new float[1536];
        for (int i = 0; i < in.length; i++) {
            in[i] = (float) Math.sin(i * 0.001);
        }
        String s = serialize(in);
        float[] out = parse(s);
        assertArrayEquals(in, out, 1e-5f);
    }
}
