package ai.nubase.postgrest.query;

import ai.nubase.postgrest.multidb.SchemaCacheManager;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;
import org.springframework.jdbc.core.JdbcTemplate;

import java.util.Arrays;
import java.util.Collections;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.*;

/**
 * SQL generation complete tests
 * Covers all CRUD operations and special SQL scenarios
 */
@DisplayName("SQL generation complete tests")
class SqlGenerationCompleteTest {

    private QueryExecutor queryExecutor;
    private JdbcTemplate jdbcTemplate;
    private ObjectMapper objectMapper;
    private SchemaCacheManager schemaCacheManager;

    @BeforeEach
    void setUp() {
        jdbcTemplate = mock(JdbcTemplate.class);
        objectMapper = new ObjectMapper();
        schemaCacheManager = mock(SchemaCacheManager.class);
        queryExecutor = new QueryExecutor(jdbcTemplate, objectMapper, schemaCacheManager);
    }

    // ==================== SELECT query tests ====================

    @Test
    @DisplayName("Generate simple SELECT query")
    void testGenerateSimpleSelect() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Arrays.asList("id", "name", "email"))
            .build();

        String sql = buildSql(plan);

        // SQL uses quoted identifiers
        assertTrue(sql.contains("\"id\"") && sql.contains("\"name\"") && sql.contains("\"email\""));
        assertTrue(sql.contains("FROM \"public\".\"users\""));
    }

    @Test
    @DisplayName("Generate SELECT * query")
    void testGenerateSelectAll() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .build();

        String sql = buildSql(plan);

        // SELECT with table qualified wildcard
        assertTrue(sql.contains("\"users\".*"));
        assertTrue(sql.contains("FROM \"public\".\"users\""));
    }

    @Test
    @DisplayName("Generate SELECT with WHERE query")
    void testGenerateSelectWithWhere() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Arrays.asList("id", "name"))
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("age")
                    .operator(">=")
                    .value("18")
                    .operatorType("GTE")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"age\" >= '18'"));
    }

    @Test
    @DisplayName("Generate SELECT with ORDER BY query")
    void testGenerateSelectWithOrderBy() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("posts")
            .orderBy(Arrays.asList("created_at DESC", "id ASC"))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("ORDER BY created_at DESC, id ASC"));
    }

    @Test
    @DisplayName("Generate SELECT with LIMIT query")
    void testGenerateSelectWithLimit() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .limit(10L)
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("LIMIT 10"));
    }

    @Test
    @DisplayName("Generate SELECT with OFFSET query")
    void testGenerateSelectWithOffset() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .limit(10L)
            .offset(20L)
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("LIMIT 10"));
        assertTrue(sql.contains("OFFSET 20"));
    }

    @Test
    @DisplayName("Generate SELECT with JOIN query")
    void testGenerateSelectWithJoin() {
        JoinClause join = JoinClause.builder()
            .type(JoinClause.JoinType.LEFT)
            .targetSchema("public")
            .targetTable("posts")
            .conditions(Arrays.asList(
                JoinClause.JoinCondition.builder()
                    .leftColumn("id")
                    .operator("=")
                    .rightColumn("user_id")
                    .build()
            ))
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .joins(Collections.singletonList(join))
            .build();

        String sql = buildSql(plan);

        // Joins are now implemented as embedded subqueries (PostgREST style)
        assertTrue(sql.contains("COALESCE((SELECT row_to_json"));
        assertTrue(sql.contains("\"public\".\"posts\""));
    }

    // ==================== INSERT query tests ====================

    @Test
    @DisplayName("Generate single-row INSERT query")
    void testGenerateSingleInsert() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.INSERT)
            .schema("public")
            .table("users")
            .build();

        String body = "{\"name\":\"John\",\"email\":\"john@example.com\"}";
        String sql = buildSql(plan, body);

        assertTrue(sql.contains("INSERT INTO \"public\".\"users\""));
        // Column names are quoted
        assertTrue(sql.contains("\"name\"") && sql.contains("\"email\""));
        assertTrue(sql.contains("VALUES"));
        assertTrue(sql.contains("'John'"));
        assertTrue(sql.contains("'john@example.com'"));
    }

    @Test
    @DisplayName("Generate bulk INSERT query")
    void testGenerateBulkInsert() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.INSERT)
            .schema("public")
            .table("users")
            .build();

        String body = "[{\"name\":\"John\"},{\"name\":\"Jane\"}]";
        String sql = buildSql(plan, body);

        assertTrue(sql.contains("INSERT INTO"));
        assertTrue(sql.contains("VALUES"));
        assertTrue(sql.contains("'John'"));
        assertTrue(sql.contains("'Jane'"));
    }

    @Test
    @DisplayName("Generate INSERT with RETURNING query")
    void testGenerateInsertWithReturning() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.INSERT)
            .schema("public")
            .table("users")
            .returningAll(true)
            .build();

        String body = "{\"name\":\"John\"}";
        String sql = buildSql(plan, body);

        assertTrue(sql.contains("RETURNING *"));
    }

    // ==================== UPDATE query tests ====================

    @Test
    @DisplayName("Generate UPDATE query")
    void testGenerateUpdate() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("1")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String body = "{\"status\":\"active\"}";
        String sql = buildSql(plan, body);

        assertTrue(sql.contains("UPDATE \"public\".\"users\""));
        assertTrue(sql.contains("SET \"status\" = 'active'"));
        assertTrue(sql.contains("WHERE \"id\" = '1'"));
    }

    @Test
    @DisplayName("Generate UPDATE with RETURNING query")
    void testGenerateUpdateWithReturning() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .returningAll(true)
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("1")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String body = "{\"name\":\"Updated\"}";
        String sql = buildSql(plan, body);

        assertTrue(sql.contains("RETURNING *"));
    }

    // ==================== DELETE query tests ====================

    @Test
    @DisplayName("Generate DELETE query")
    void testGenerateDelete() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("users")
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("1")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("DELETE FROM \"public\".\"users\""));
        assertTrue(sql.contains("WHERE \"id\" = '1'"));
    }

    @Test
    @DisplayName("Generate DELETE with RETURNING query")
    void testGenerateDeleteWithReturning() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("users")
            .returningAll(true)
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("status")
                    .operator("=")
                    .value("inactive")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("RETURNING *"));
    }

    // ==================== UPSERT query tests ====================

    @Test
    @DisplayName("Generate UPSERT with DO NOTHING query")
    void testGenerateUpsertDoNothing() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPSERT)
            .schema("public")
            .table("users")
            .conflictColumns(Arrays.asList("email"))
            .ignoreConflict(true)
            .build();

        String body = "{\"email\":\"john@example.com\",\"name\":\"John\"}";
        String sql = buildSql(plan, body);

        assertTrue(sql.contains("INSERT INTO"));
        assertTrue(sql.contains("ON CONFLICT (\"email\")"));
        assertTrue(sql.contains("DO NOTHING"));
    }

    @Test
    @DisplayName("Generate UPSERT with DO UPDATE query")
    void testGenerateUpsertDoUpdate() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPSERT)
            .schema("public")
            .table("users")
            .conflictColumns(Arrays.asList("id"))
            .ignoreConflict(false)
            .build();

        String body = "{\"id\":1,\"name\":\"Updated\"}";
        String sql = buildSql(plan, body);

        assertTrue(sql.contains("ON CONFLICT (\"id\")"));
        assertTrue(sql.contains("DO UPDATE SET"));
        assertTrue(sql.contains("EXCLUDED"));
    }

    // ==================== SQL injection protection tests ====================

    @Test
    @DisplayName("SQL injection protection - table name quoting")
    void testSqlInjectionTableName() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users; DROP TABLE users;--")
            .build();

        String sql = buildSql(plan);

        // Table name should be wrapped in double quotes; the injected DROP TABLE is safely contained within the quotes
        assertTrue(sql.contains("\"users; DROP TABLE users;--\""));
        // Verify SQL formatting: the table name in the FROM clause is wrapped in quotes
        assertTrue(sql.contains("FROM \"public\".\"users; DROP TABLE users;--\""));
    }

    @Test
    @DisplayName("SQL injection protection - column name quoting")
    void testSqlInjectionColumnName() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Arrays.asList("id; DROP TABLE users;--"))
            .build();

        String sql = buildSql(plan);

        // Column name should be quoted
        assertTrue(sql.contains("\"id; DROP TABLE users;--\""));
    }

    @Test
    @DisplayName("SQL injection protection - value escaping")
    void testSqlInjectionValueEscape() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("name")
                    .operator("=")
                    .value("'; DROP TABLE users; --")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        // Single quotes should be escaped
        assertTrue(sql.contains("''; DROP TABLE users; --'"));
    }

    @Test
    @DisplayName("Protect against NULL character injection")
    void testNullCharacterInjection() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users\0malicious")
            .build();

        String sql = buildSql(plan);

        // Content after the NULL character should be truncated
        assertFalse(sql.contains("malicious"));
        assertTrue(sql.contains("\"users\""));
    }

    // ==================== Special character handling tests ====================

    @ParameterizedTest
    @CsvSource({
        "user name, \"user name\"",
        "user@email, \"user@email\"",
        "user-id, \"user-id\"",
        "user_name, \"user_name\"",
        "user.name, \"user.name\""
    })
    @DisplayName("Handle special characters - identifier quoting")
    void testSpecialCharactersInIdentifiers(String identifier, String expected) {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table(identifier)
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains(expected));
    }

    @Test
    @DisplayName("Handle double quotes - identifier escaping")
    void testDoubleQuoteEscape() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("My \"Table\"")
            .build();

        String sql = buildSql(plan);

        // Double quotes should be doubled
        assertTrue(sql.contains("\"My \"\"Table\"\"\""));
    }

    @Test
    @DisplayName("Handle single quotes - value escaping")
    void testSingleQuoteEscape() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("name")
                    .operator("=")
                    .value("O'Reilly")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        // Single quotes should be doubled
        assertTrue(sql.contains("'O''Reilly'"));
    }

    // ==================== Complex query tests ====================

    @Test
    @DisplayName("Generate complex query - all clauses")
    void testComplexQueryAllClauses() {
        JoinClause join = JoinClause.builder()
            .type(JoinClause.JoinType.LEFT)
            .targetSchema("public")
            .targetTable("posts")
            .conditions(Arrays.asList(
                JoinClause.JoinCondition.builder()
                    .leftColumn("id")
                    .operator("=")
                    .rightColumn("user_id")
                    .build()
            ))
            .build();

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Arrays.asList("id", "name", "email"))
            .joins(Collections.singletonList(join))
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("age")
                    .operator(">=")
                    .value("18")
                    .operatorType("GTE")
                    .build(),
                WhereClause.builder()
                    .column("status")
                    .operator("=")
                    .value("active")
                    .operatorType("EQ")
                    .build()
            ))
            .orderBy(Arrays.asList("created_at DESC"))
            .limit(10L)
            .offset(20L)
            .build();

        String sql = buildSql(plan);

        // Column names are quoted
        assertTrue(sql.contains("\"id\"") && sql.contains("\"name\"") && sql.contains("\"email\""));
        assertTrue(sql.contains("FROM \"public\".\"users\""));
        // Joins are implemented as embedded subqueries
        assertTrue(sql.contains("COALESCE((SELECT row_to_json"));
        assertTrue(sql.contains("WHERE"));
        assertTrue(sql.contains("AND"));
        assertTrue(sql.contains("ORDER BY"));
        assertTrue(sql.contains("LIMIT 10"));
        assertTrue(sql.contains("OFFSET 20"));
    }

    // ==================== Helper Methods ====================

    private String buildSql(QueryPlan plan) {
        return buildSql(plan, null);
    }

    private String buildSql(QueryPlan plan, String body) {
        try {
            java.lang.reflect.Method method = QueryExecutor.class.getDeclaredMethod(
                "buildSQL", QueryPlan.class, String.class);
            method.setAccessible(true);
            return (String) method.invoke(queryExecutor, plan, body);
        } catch (Exception e) {
            throw new RuntimeException("Failed to build SQL", e);
        }
    }
}
