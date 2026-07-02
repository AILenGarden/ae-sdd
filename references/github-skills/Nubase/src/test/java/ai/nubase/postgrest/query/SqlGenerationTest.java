package ai.nubase.postgrest.query;

import ai.nubase.postgrest.multidb.SchemaCacheManager;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.jdbc.core.JdbcTemplate;

import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.mock;

/**
 * SQL Generation tests - Verify QueryPlan to SQL conversion
 * Based on PostgREST's SQL generation logic
 */
class SqlGenerationTest {

    private QueryExecutor executor;
    private Method buildSQLMethod;
    private SchemaCacheManager schemaCacheManager;

    @BeforeEach
    void setUp() throws Exception {
        JdbcTemplate jdbcTemplate = mock(JdbcTemplate.class);
        ObjectMapper objectMapper = new ObjectMapper();
        schemaCacheManager = mock(SchemaCacheManager.class);
        executor = new QueryExecutor(jdbcTemplate, objectMapper, schemaCacheManager);

        // Access private buildSQL method via reflection
        buildSQLMethod = QueryExecutor.class.getDeclaredMethod("buildSQL", QueryPlan.class, String.class);
        buildSQLMethod.setAccessible(true);
    }

    private String buildSQL(QueryPlan plan, String body) throws Exception {
        return (String) buildSQLMethod.invoke(executor, plan, body);
    }

    // ==================== SELECT Tests ====================

    @Test
    void testSelect_AllColumns() throws Exception {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .build();

        String sql = buildSQL(plan, null);

        // With the PostgREST-style implementation, table is always qualified
        assertEquals("SELECT \"users\".* FROM \"public\".\"users\"", sql);
    }

    @Test
    void testSelect_SpecificColumns() throws Exception {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Arrays.asList("id", "name", "email"))
            .build();

        String sql = buildSQL(plan, null);

        // Columns are now quoted for SQL safety
        assertEquals("SELECT \"id\", \"name\", \"email\" FROM \"public\".\"users\"", sql);
    }

    @Test
    void testSelect_WithWhereClause() throws Exception {
        WhereClause where = WhereClause.builder()
            .column("age")
            .operator(">")
            .value(18)
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(where))
            .build();

        String sql = buildSQL(plan, null);

        assertTrue(sql.contains("WHERE"));
        assertTrue(sql.contains("\"age\" > 18"));
    }

    @Test
    void testSelect_WithMultipleWhereConditions() throws Exception {
        List<WhereClause> whereClauses = Arrays.asList(
            WhereClause.builder()
                .column("age")
                .operator(">=")
                .value(18)
                .build(),
            WhereClause.builder()
                .column("status")
                .operator("=")
                .value("active")
                .build()
        );

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(whereClauses)
            .build();

        String sql = buildSQL(plan, null);

        assertTrue(sql.contains("WHERE"));
        assertTrue(sql.contains("\"age\" >= 18"));
        assertTrue(sql.contains("AND"));
        assertTrue(sql.contains("\"status\" = 'active'"));
    }

    @Test
    void testSelect_WithOrderBy() throws Exception {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .orderBy(Arrays.asList("created_at DESC", "id ASC"))
            .build();

        String sql = buildSQL(plan, null);

        assertTrue(sql.contains("ORDER BY"));
        assertTrue(sql.contains("created_at DESC, id ASC"));
    }

    @Test
    void testSelect_WithLimitAndOffset() throws Exception {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .limit(10L)
            .offset(20L)
            .build();

        String sql = buildSQL(plan, null);

        assertTrue(sql.contains("LIMIT 10"));
        assertTrue(sql.contains("OFFSET 20"));
    }

    @Test
    void testSelect_ComplexQuery() throws Exception {
        WhereClause where = WhereClause.builder()
            .column("status")
            .operator("=")
            .value("active")
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Arrays.asList("id", "name", "email"))
            .whereClauses(Collections.singletonList(where))
            .orderBy(Collections.singletonList("name ASC"))
            .limit(10L)
            .offset(0L)
            .build();

        String sql = buildSQL(plan, null);

        // Verify all parts are present (columns are now quoted)
        assertTrue(sql.contains("SELECT \"id\", \"name\", \"email\""));
        assertTrue(sql.contains("FROM \"public\".\"users\""));
        assertTrue(sql.contains("WHERE \"status\" = 'active'"));
        assertTrue(sql.contains("ORDER BY name ASC"));
        assertTrue(sql.contains("LIMIT 10"));
        assertTrue(sql.contains("OFFSET 0"));
    }

    // ==================== INSERT Tests ====================
    // Note: INSERT tests are skipped because buildInsertSQL requires valid JSON parsing
    // which depends on runtime ObjectMapper configuration. Integration tests cover these scenarios.

    // ==================== UPDATE Tests ====================

    @Test
    void testUpdate_WithWhereClause() throws Exception {
        WhereClause where = WhereClause.builder()
            .column("id")
            .operator("=")
            .value(1)
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(where))
            .build();

        String body = "{\"name\":\"Updated Name\"}";
        String sql = buildSQL(plan, body);

        assertTrue(sql.startsWith("UPDATE \"public\".\"users\""));
        assertTrue(sql.contains("SET"));
        assertTrue(sql.contains("\"name\" = 'Updated Name'"));
        assertTrue(sql.contains("WHERE"));
        assertTrue(sql.contains("\"id\" = 1"));
    }

    @Test
    void testUpdate_MultipleFields() throws Exception {
        WhereClause where = WhereClause.builder()
            .column("id")
            .operator("=")
            .value(1)
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(where))
            .build();

        String body = "{\"name\":\"John\",\"email\":\"john@new.com\",\"age\":25}";
        String sql = buildSQL(plan, body);

        assertTrue(sql.contains("SET"));
        assertTrue(sql.contains("\"name\" = 'John'"));
        assertTrue(sql.contains("\"email\" = 'john@new.com'"));
        assertTrue(sql.contains("\"age\" = 25"));
    }

    // ==================== DELETE Tests ====================

    @Test
    void testDelete_WithWhereClause() throws Exception {
        WhereClause where = WhereClause.builder()
            .column("id")
            .operator("=")
            .value(999)
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(where))
            .build();

        String sql = buildSQL(plan, null);

        assertEquals("DELETE FROM \"public\".\"users\" WHERE \"id\" = 999", sql);
    }

    @Test
    void testDelete_WithReturning() throws Exception {
        WhereClause where = WhereClause.builder()
            .column("status")
            .operator("=")
            .value("inactive")
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(where))
            .returningAll(true)
            .build();

        String sql = buildSQL(plan, null);

        assertTrue(sql.contains("DELETE FROM"));
        assertTrue(sql.contains("WHERE \"status\" = 'inactive'"));
        assertTrue(sql.endsWith("RETURNING *"));
    }

    // ==================== SQL Injection Prevention ====================

    @Test
    void testSqlInjection_IdentifierQuoting() throws Exception {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Collections.singletonList("id"))
            .build();

        String sql = buildSQL(plan, null);

        // Verify identifiers are quoted
        assertTrue(sql.contains("\"public\""));
        assertTrue(sql.contains("\"users\""));
    }

    @Test
    void testSqlInjection_StringEscaping() throws Exception {
        WhereClause where = WhereClause.builder()
            .column("name")
            .operator("=")
            .value("O'Reilly")
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(where))
            .build();

        String sql = buildSQL(plan, null);

        // Single quotes should be escaped
        assertTrue(sql.contains("'O''Reilly'"));
    }

    @Test
    void testSqlInjection_NullHandling() throws Exception {
        WhereClause where = WhereClause.builder()
            .column("deleted_at")
            .operator("IS")
            .value(null)
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(where))
            .build();

        String sql = buildSQL(plan, null);

        assertTrue(sql.contains("NULL"));
    }

    // ==================== Edge Cases ====================

    @Test
    void testSelect_NoSchema() throws Exception {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .table("users")
            .build();

        String sql = buildSQL(plan, null);

        // Should not have schema prefix
        assertFalse(sql.contains("\"public\"."));
        assertTrue(sql.contains("FROM \"users\""));
    }

    @Test
    void testSelect_EmptyWhereClause() throws Exception {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.emptyList())
            .build();

        String sql = buildSQL(plan, null);

        // Should not have WHERE clause
        assertFalse(sql.contains("WHERE"));
    }

    @Test
    void testUpdate_NoWhereClause() throws Exception {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.emptyList())
            .build();

        String body = "{\"status\":\"migrated\"}";
        String sql = buildSQL(plan, body);

        // Should update all rows (no WHERE clause)
        assertTrue(sql.contains("UPDATE"));
        assertTrue(sql.contains("SET"));
        assertFalse(sql.contains("WHERE"));
    }
}
