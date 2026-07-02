package ai.nubase.postgrest.schema;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mock;
import org.mockito.MockitoAnnotations;
import ai.nubase.postgrest.config.PostgRESTConfig;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.jdbc.core.RowCallbackHandler;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;

class SchemaCacheTest {

    @Mock
    private JdbcTemplate jdbcTemplate;

    @Mock
    private PostgRESTConfig config;

    private SchemaCache schemaCache;

    @BeforeEach
    void setUp() {
        MockitoAnnotations.openMocks(this);
        when(config.getDbSchemas()).thenReturn(List.of("public"));

        // Mock the query methods
        doAnswer(invocation -> {
            RowCallbackHandler handler = invocation.getArgument(1);
            // Simulate empty result for now
            return null;
        }).when(jdbcTemplate).query(anyString(), any(RowCallbackHandler.class), any(Object.class));

        schemaCache = new SchemaCache(jdbcTemplate, config, "test_db");
    }

    @Test
    void testSchemaCacheInitialization() {
        assertNotNull(schemaCache);
        assertNotNull(schemaCache.getTables());
        assertNotNull(schemaCache.getRoutines());
        assertNotNull(schemaCache.getRelationships());
    }

    @Test
    void testGetTableReturnsNullForNonExistentTable() {
        Table table = schemaCache.getTable("public", "nonexistent");
        assertNull(table);
    }

    @Test
    void testGetRoutineReturnsNullForNonExistentRoutine() {
        Routine routine = schemaCache.getRoutine("public", "nonexistent");
        assertNull(routine);
    }

    @Test
    void testGetRelationshipsReturnsEmptyListForNonExistentTable() {
        List<ForeignKey> relationships = schemaCache.getRelationships("public", "nonexistent");
        assertNotNull(relationships);
        assertTrue(relationships.isEmpty());
    }

    @Test
    void testReloadClearsCache() {
        // The cache is loaded during initialization
        // Calling reload should trigger the load queries again

        // Reset the mock to track new invocations
        reset(jdbcTemplate);

        // Set up mock again for reload
        doAnswer(invocation -> {
            RowCallbackHandler handler = invocation.getArgument(1);
            return null;
        }).when(jdbcTemplate).query(anyString(), any(RowCallbackHandler.class), any(Object.class));

        // Reload
        schemaCache.reload();

        // Verify query methods were called during reload
        verify(jdbcTemplate, atLeast(1)).query(anyString(), any(RowCallbackHandler.class), any(Object.class));
    }
}
