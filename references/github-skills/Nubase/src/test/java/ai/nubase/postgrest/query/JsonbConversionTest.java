package ai.nubase.postgrest.query;

import ai.nubase.postgrest.multidb.SchemaCacheManager;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.jdbc.core.JdbcTemplate;

import java.lang.reflect.Method;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.mock;

/**
 * Test JSONB conversion to ensure PostgREST-compatible JSON output.
 * JSONB columns should be returned as native JSON objects/arrays, not as strings or PGobject.
 */
@DisplayName("JSONB Conversion Tests")
class JsonbConversionTest {

    private QueryExecutor executor;
    private Method convertPgValueMethod;
    private ObjectMapper objectMapper;

    @BeforeEach
    void setUp() throws Exception {
        JdbcTemplate jdbcTemplate = mock(JdbcTemplate.class);
        objectMapper = new ObjectMapper();
        SchemaCacheManager schemaCacheManager = mock(SchemaCacheManager.class);
        executor = new QueryExecutor(jdbcTemplate, objectMapper, schemaCacheManager);

        // Access private convertPgValue method via reflection
        convertPgValueMethod = QueryExecutor.class.getDeclaredMethod("convertPgValue", Object.class);
        convertPgValueMethod.setAccessible(true);
    }

    private Object convertPgValue(Object value) throws Exception {
        return convertPgValueMethod.invoke(executor, value);
    }

    @Test
    @DisplayName("Mock PGobject with JSONB array should convert to List")
    void testPGobjectJsonbArray() throws Exception {
        // Create a mock PGobject-like object
        MockPGobject pgObject = new MockPGobject("jsonb", "[\"1\", \"2\", \"\", \"\", \"\"]");

        Object result = convertPgValue(pgObject);

        // Should be converted to a List
        assertInstanceOf(List.class, result);
        List<?> list = (List<?>) result;
        assertEquals(5, list.size());
        assertEquals("1", list.get(0));
        assertEquals("2", list.get(1));
        assertEquals("", list.get(2));
    }

    @Test
    @DisplayName("Mock PGobject with JSONB object should convert to Map")
    void testPGobjectJsonbObject() throws Exception {
        MockPGobject pgObject = new MockPGobject("jsonb", "{\"key\": \"value\", \"nested\": {\"a\": 1}}");

        Object result = convertPgValue(pgObject);

        // Should be converted to a Map
        assertInstanceOf(Map.class, result);
        @SuppressWarnings("unchecked")
        Map<String, Object> map = (Map<String, Object>) result;
        assertEquals("value", map.get("key"));
        assertInstanceOf(Map.class, map.get("nested"));
    }

    @Test
    @DisplayName("Mock PGobject with JSON null should convert to null")
    void testPGobjectJsonbNull() throws Exception {
        MockPGobject pgObject = new MockPGobject("jsonb", "null");

        Object result = convertPgValue(pgObject);

        assertNull(result);
    }

    @Test
    @DisplayName("Mock PGobject with empty string should convert to null")
    void testPGobjectJsonbEmpty() throws Exception {
        MockPGobject pgObject = new MockPGobject("jsonb", "");

        Object result = convertPgValue(pgObject);

        assertNull(result);
    }

    @Test
    @DisplayName("Regular String should pass through unchanged")
    void testRegularString() throws Exception {
        String value = "hello world";

        Object result = convertPgValue(value);

        assertEquals("hello world", result);
    }

    @Test
    @DisplayName("Null value should return null")
    void testNullValue() throws Exception {
        Object result = convertPgValue(null);

        assertNull(result);
    }

    @Test
    @DisplayName("Map with nested PGobject should be recursively converted")
    void testNestedPGobject() throws Exception {
        Map<String, Object> row = new HashMap<>();
        row.put("id", 1);
        row.put("name", "test");
        row.put("metadata", new MockPGobject("jsonb", "{\"key\": \"value\"}"));

        Object result = convertPgValue(row);

        assertInstanceOf(Map.class, result);
        @SuppressWarnings("unchecked")
        Map<String, Object> convertedRow = (Map<String, Object>) result;
        assertEquals(1, convertedRow.get("id"));
        assertEquals("test", convertedRow.get("name"));
        assertInstanceOf(Map.class, convertedRow.get("metadata"));
    }

    @Test
    @DisplayName("JSONB array serializes correctly to JSON string")
    void testJsonbArraySerialization() throws Exception {
        MockPGobject pgObject = new MockPGobject("jsonb", "[\"1\",\"2\",\"\",\"\",\"\"]");

        Object result = convertPgValue(pgObject);
        String json = objectMapper.writeValueAsString(result);

        // Should be a native JSON array, not escaped
        assertEquals("[\"1\",\"2\",\"\",\"\",\"\"]", json);
    }

    @Test
    @DisplayName("Row with JSONB column serializes correctly")
    void testRowWithJsonbSerialization() throws Exception {
        Map<String, Object> row = new HashMap<>();
        row.put("id", 1);
        row.put("driver_group_1_entries", new MockPGobject("jsonb", "[\"1\",\"2\"]"));

        Object convertedRow = convertPgValue(row);
        String json = objectMapper.writeValueAsString(convertedRow);

        // The JSONB array should be native, not a string
        assertTrue(json.contains("\"driver_group_1_entries\":[\"1\",\"2\"]") ||
                   json.contains("\"driver_group_1_entries\": [\"1\", \"2\"]"));
        // Should NOT contain escaped quotes or type/value wrapper
        assertFalse(json.contains("\\\"1\\\""));
        assertFalse(json.contains("\"type\":\"jsonb\""));
    }

    /**
     * Mock class to simulate PostgreSQL's PGobject behavior.
     * The real PGobject class is from org.postgresql.util.PGobject.
     */
    public static class MockPGobject {
        private String type;
        private String value;

        public MockPGobject(String type, String value) {
            this.type = type;
            this.value = value;
        }

        public String getType() {
            return type;
        }

        public String getValue() {
            return value;
        }

        public boolean isNull() {
            return value == null;
        }

        @Override
        public String toString() {
            return value;
        }
    }
}
