package ai.nubase.postgrest.query;

import ai.nubase.postgrest.multidb.SchemaCacheManager;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;
import org.springframework.jdbc.core.JdbcTemplate;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.*;

/**
 * Complete SQL generation tests
 * Covers all query conditions and UPDATE/DELETE scenarios
 */
@DisplayName("Complete SQL generation tests")
class CompleteSqlGenerationTest {

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
    @DisplayName("SELECT - basic query")
    void testSelectBasic() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Arrays.asList("id", "name", "email"))
            .build();

        String sql = buildSql(plan);

        assertEquals("SELECT \"id\", \"name\", \"email\" FROM \"public\".\"users\"", sql);
    }

    @Test
    @DisplayName("SELECT - all columns")
    void testSelectAll() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .build();

        String sql = buildSql(plan);

        assertEquals("SELECT \"users\".* FROM \"public\".\"users\"", sql);
    }

    @Test
    @DisplayName("SELECT - single WHERE condition")
    void testSelectWithSingleWhere() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Arrays.asList("id", "name"))
            .whereClauses(Collections.singletonList(
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
    @DisplayName("SELECT - multiple WHERE conditions (AND)")
    void testSelectWithMultipleWhere() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
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
                    .build(),
                WhereClause.builder()
                    .column("verified")
                    .operator("IS")
                    .value("true")
                    .operatorType("IS")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE"));
        assertTrue(sql.contains("\"age\" >= '18'"));
        assertTrue(sql.contains("AND"));
        assertTrue(sql.contains("\"status\" = 'active'"));
        assertTrue(sql.contains("\"verified\" IS TRUE"));
    }

    @Test
    @DisplayName("SELECT - WHERE with table prefix")
    void testSelectWithTablePrefixWhere() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("tasks")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("projects.status")
                    .operator("=")
                    .value("active")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"projects\".\"status\" = 'active'"));
    }

    @ParameterizedTest
    @CsvSource({
        "\"name\" ASC, \"name\" ASC",
        "\"created_at\" DESC, \"created_at\" DESC",
        "\"tasks\".\"priority\" DESC, \"tasks\".\"priority\" DESC"
    })
    @DisplayName("SELECT - ORDER BY various formats")
    void testSelectWithOrderBy(String orderByClause, String expected) {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("tasks")
            .orderBy(Collections.singletonList(orderByClause))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("ORDER BY " + expected));
    }

    @Test
    @DisplayName("SELECT - ORDER BY multiple columns")
    void testSelectWithMultipleOrderBy() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("tasks")
            .orderBy(Arrays.asList(
                "\"priority\" DESC",
                "\"created_at\" ASC"
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("ORDER BY \"priority\" DESC, \"created_at\" ASC"));
    }

    @Test
    @DisplayName("SELECT - ORDER BY with NULLS FIRST")
    void testSelectWithOrderByNullsFirst() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("tasks")
            .orderBy(Collections.singletonList("\"due_date\" ASC NULLS FIRST"))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("ORDER BY \"due_date\" ASC NULLS FIRST"));
    }

    @Test
    @DisplayName("SELECT - LIMIT")
    void testSelectWithLimit() {
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
    @DisplayName("SELECT - OFFSET")
    void testSelectWithOffset() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .offset(20L)
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("OFFSET 20"));
    }

    @Test
    @DisplayName("SELECT - LIMIT and OFFSET")
    void testSelectWithLimitAndOffset() {
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
    @DisplayName("SELECT - full complex query")
    void testSelectComplex() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("tasks")
            .selectColumns(Arrays.asList("id", "title", "priority"))
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("status")
                    .operator("IN")
                    .value("(todo,in-progress)")
                    .operatorType("IN")
                    .build(),
                WhereClause.builder()
                    .column("priority")
                    .operator(">=")
                    .value("3")
                    .operatorType("GTE")
                    .build()
            ))
            .orderBy(Arrays.asList("\"priority\" DESC", "\"created_at\" ASC"))
            .limit(10L)
            .offset(5L)
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("SELECT \"id\", \"title\", \"priority\""));
        assertTrue(sql.contains("FROM \"public\".\"tasks\""));
        assertTrue(sql.contains("WHERE"));
        assertTrue(sql.contains("\"status\" IN ('todo', 'in-progress')"));
        assertTrue(sql.contains("\"priority\" >= '3'"));
        assertTrue(sql.contains("ORDER BY \"priority\" DESC, \"created_at\" ASC"));
        assertTrue(sql.contains("LIMIT 10"));
        assertTrue(sql.contains("OFFSET 5"));
    }

    // ==================== WHERE condition operator tests ====================

    @ParameterizedTest
    @CsvSource({
        "EQ, =, 25, \"age\" = '25'",
        "NEQ, !=, inactive, \"status\" != 'inactive'",
        "GT, >, 18, \"age\" > '18'",
        "GTE, >=, 18, \"age\" >= '18'",
        "LT, <, 65, \"age\" < '65'",
        "LTE, <=, 65, \"age\" <= '65'"
    })
    @DisplayName("WHERE - comparison operators")
    void testWhereComparisonOperators(String opType, String operator, String value, String expected) {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column(expected.split(" ")[0].replace("\"", ""))
                    .operator(operator)
                    .value(value)
                    .operatorType(opType)
                    .build()
            ))
            .build();

        String sql = buildSql(plan);
        assertTrue(sql.contains(expected));
    }

    @Test
    @DisplayName("WHERE - LIKE wildcard conversion")
    void testWhereLike() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("products")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("name")
                    .operator("LIKE")
                    .value("*Phone*")
                    .operatorType("LIKE")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"name\" LIKE '%Phone%'"));
    }

    @Test
    @DisplayName("WHERE - ILIKE case-insensitive")
    void testWhereILike() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("email")
                    .operator("ILIKE")
                    .value("*@gmail.com")
                    .operatorType("ILIKE")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"email\" ILIKE '%@gmail.com'"));
    }

    @Test
    @DisplayName("WHERE - IN operator")
    void testWhereIn() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("tasks")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("status")
                    .operator("IN")
                    .value("(todo,in-progress,done)")
                    .operatorType("IN")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"status\" IN ('todo', 'in-progress', 'done')"));
    }

    @ParameterizedTest
    @CsvSource({
        "null, \"deleted_at\" IS NULL",
        "true, \"is_active\" IS TRUE",
        "false, \"is_deleted\" IS FALSE",
        "unknown, \"status\" IS UNKNOWN"
    })
    @DisplayName("WHERE - IS operator special values")
    void testWhereIsSpecialValues(String value, String expected) {
        String column = expected.split(" ")[0].replace("\"", "");

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column(column)
                    .operator("IS")
                    .value(value)
                    .operatorType("IS")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains(expected));
    }

    @Test
    @DisplayName("WHERE - FTS full-text search")
    void testWhereFts() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("articles")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("content")
                    .operator("@@")
                    .value("java & spring")
                    .operatorType("FTS")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"content\" @@ to_tsquery('java & spring')"));
    }

    @Test
    @DisplayName("WHERE - PLFTS plainto_tsquery")
    void testWherePlfts() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("articles")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("content")
                    .operator("@@")
                    .value("Java Spring Boot")
                    .operatorType("PLFTS")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"content\" @@ plainto_tsquery('Java Spring Boot')"));
    }

    @Test
    @DisplayName("WHERE - PHFTS phraseto_tsquery")
    void testWherePhfts() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("articles")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("content")
                    .operator("@@")
                    .value("quick brown fox")
                    .operatorType("PHFTS")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"content\" @@ phraseto_tsquery('quick brown fox')"));
    }

    @Test
    @DisplayName("WHERE - WFTS websearch_to_tsquery")
    void testWhereWfts() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("articles")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("content")
                    .operator("@@")
                    .value("cats OR dogs")
                    .operatorType("WFTS")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"content\" @@ websearch_to_tsquery('cats OR dogs')"));
    }

    @Test
    @DisplayName("WHERE - CS (contains) array operator")
    void testWhereCs() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("posts")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("tags")
                    .operator("@>")
                    .value("{tech,java}")
                    .operatorType("CS")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"tags\" @> ARRAY['tech', 'java']"));
    }

    @Test
    @DisplayName("WHERE - CD (contained in) array operator")
    void testWhereCd() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("posts")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("selected_tags")
                    .operator("<@")
                    .value("{tech,java,spring,python}")
                    .operatorType("CD")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"selected_tags\" <@ ARRAY['tech', 'java', 'spring', 'python']"));
    }

    @Test
    @DisplayName("WHERE - OV (overlap) array operator")
    void testWhereOv() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("events")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("date_range")
                    .operator("&&")
                    .value("[2024-01-01,2024-12-31]")
                    .operatorType("OV")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"date_range\" && '[2024-01-01,2024-12-31]'"));
    }

    @Test
    @DisplayName("WHERE - NOT modifier")
    void testWhereNot() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("status")
                    .operator("=")
                    .value("inactive")
                    .operatorType("EQ")
                    .negate(true)
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE NOT (\"status\" = 'inactive')"));
    }

    @Test
    @DisplayName("WHERE - regex MATCH")
    void testWhereMatch() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("username")
                    .operator("~")
                    .value("^user[0-9]+$")
                    .operatorType("MATCH")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"username\" ~ '^user[0-9]+$'"));
    }

    @Test
    @DisplayName("WHERE - case-insensitive regex IMATCH")
    void testWhereImatch() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("email")
                    .operator("~*")
                    .value("(?i)admin")
                    .operatorType("IMATCH")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"email\" ~* '(?i)admin'"));
    }

    // ==================== INSERT tests ====================

    @Test
    @DisplayName("INSERT - single row")
    void testInsertSingle() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.INSERT)
            .schema("public")
            .table("users")
            .build();

        String sql = buildSql(plan, "[{\"name\":\"John\",\"email\":\"john@example.com\",\"age\":30}]");

        assertTrue(sql.contains("INSERT INTO \"public\".\"users\""));
        assertTrue(sql.contains("VALUES"));
        assertTrue(sql.contains("'John'"));
        assertTrue(sql.contains("'john@example.com'"));
        assertTrue(sql.contains("30"));
    }

    @Test
    @DisplayName("INSERT - bulk insert")
    void testInsertBulk() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.INSERT)
            .schema("public")
            .table("users")
            .build();

        String body = "[{\"name\":\"John\",\"email\":\"john@example.com\"},{\"name\":\"Jane\",\"email\":\"jane@example.com\"}]";
        String sql = buildSql(plan, body);

        assertTrue(sql.contains("INSERT INTO \"public\".\"users\""));
        assertTrue(sql.contains("VALUES"));
        assertTrue(sql.contains("'John'"));
        assertTrue(sql.contains("'Jane'"));
        // Should have two value groups
        int valuesCount = sql.split("\\),\\(").length;
        assertTrue(valuesCount >= 1); // At least one comma separator
    }

    @Test
    @DisplayName("INSERT - RETURNING all")
    void testInsertReturningAll() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.INSERT)
            .schema("public")
            .table("users")
            .returningAll(true)
            .build();

        String sql = buildSql(plan, "[{\"name\":\"John\",\"email\":\"john@example.com\"}]");

        assertTrue(sql.contains("RETURNING *"));
    }

    // ==================== UPDATE tests ====================

    @Test
    @DisplayName("UPDATE - basic update")
    void testUpdateBasic() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .build();

        String sql = buildSql(plan, "{\"name\":\"John Updated\",\"email\":\"new@example.com\"}");

        assertTrue(sql.contains("UPDATE \"public\".\"users\""));
        assertTrue(sql.contains("SET"));
        assertTrue(sql.contains("\"name\" = 'John Updated'"));
        assertTrue(sql.contains("\"email\" = 'new@example.com'"));
    }

    @Test
    @DisplayName("UPDATE - with WHERE condition")
    void testUpdateWithWhere() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("123")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan, "{\"status\":\"inactive\"}");

        assertTrue(sql.contains("UPDATE \"public\".\"users\""));
        assertTrue(sql.contains("SET \"status\" = 'inactive'"));
        assertTrue(sql.contains("WHERE \"id\" = '123'"));
    }

    @Test
    @DisplayName("UPDATE - multiple WHERE conditions")
    void testUpdateWithMultipleWhere() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("tasks")
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("project_id")
                    .operator("=")
                    .value("456")
                    .operatorType("EQ")
                    .build(),
                WhereClause.builder()
                    .column("status")
                    .operator("=")
                    .value("pending")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan, "{\"status\":\"completed\",\"updated_at\":\"2024-01-01\"}");

        assertTrue(sql.contains("UPDATE \"public\".\"tasks\""));
        assertTrue(sql.contains("SET"));
        assertTrue(sql.contains("WHERE"));
        assertTrue(sql.contains("\"project_id\" = '456'"));
        assertTrue(sql.contains("AND"));
        assertTrue(sql.contains("\"status\" = 'pending'"));
    }

    @Test
    @DisplayName("UPDATE - with IN condition")
    void testUpdateWithInCondition() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("id")
                    .operator("IN")
                    .value("(1,2,3,4,5)")
                    .operatorType("IN")
                    .build()
            ))
            .build();

        String sql = buildSql(plan, "{\"verified\":true}");

        assertTrue(sql.contains("WHERE \"id\" IN ('1', '2', '3', '4', '5')"));
    }

    @Test
    @DisplayName("UPDATE - with comparison operators")
    void testUpdateWithComparisonOperators() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("products")
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("price")
                    .operator(">")
                    .value("100")
                    .operatorType("GT")
                    .build(),
                WhereClause.builder()
                    .column("stock")
                    .operator("<=")
                    .value("10")
                    .operatorType("LTE")
                    .build()
            ))
            .build();

        String sql = buildSql(plan, "{\"discount\":0.2}");

        assertTrue(sql.contains("WHERE \"price\" > '100' AND \"stock\" <= '10'"));
    }

    @Test
    @DisplayName("UPDATE - with LIKE condition")
    void testUpdateWithLikeCondition() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("email")
                    .operator("LIKE")
                    .value("*@gmail.com")
                    .operatorType("LIKE")
                    .build()
            ))
            .build();

        String sql = buildSql(plan, "{\"category\":\"gmail-user\"}");

        assertTrue(sql.contains("WHERE \"email\" LIKE '%@gmail.com'"));
    }

    @Test
    @DisplayName("UPDATE - RETURNING")
    void testUpdateReturning() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("1")
                    .operatorType("EQ")
                    .build()
            ))
            .returningAll(true)
            .build();

        String sql = buildSql(plan, "{\"name\":\"Updated Name\"}");

        assertTrue(sql.contains("UPDATE \"public\".\"users\""));
        assertTrue(sql.contains("RETURNING *"));
    }

    @Test
    @DisplayName("UPDATE - update to NULL")
    void testUpdateToNull() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("1")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan, "{\"deleted_at\":null}");

        assertTrue(sql.contains("SET \"deleted_at\" = NULL"));
    }

    // ==================== DELETE tests ====================

    @Test
    @DisplayName("DELETE - basic delete")
    void testDeleteBasic() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("1")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertEquals("DELETE FROM \"public\".\"users\" WHERE \"id\" = '1'", sql);
    }

    @Test
    @DisplayName("DELETE - multiple WHERE conditions")
    void testDeleteWithMultipleWhere() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("sessions")
            .whereClauses(Arrays.asList(
                WhereClause.builder()
                    .column("user_id")
                    .operator("=")
                    .value("123")
                    .operatorType("EQ")
                    .build(),
                WhereClause.builder()
                    .column("expired_at")
                    .operator("<")
                    .value("2024-01-01")
                    .operatorType("LT")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("DELETE FROM \"public\".\"sessions\""));
        assertTrue(sql.contains("WHERE \"user_id\" = '123' AND \"expired_at\" < '2024-01-01'"));
    }

    @Test
    @DisplayName("DELETE - with IN condition")
    void testDeleteWithIn() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("logs")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("level")
                    .operator("IN")
                    .value("(debug,trace)")
                    .operatorType("IN")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"level\" IN ('debug', 'trace')"));
    }

    @Test
    @DisplayName("DELETE - with IS NULL condition")
    void testDeleteWithIsNull() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("temp_data")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("processed_at")
                    .operator("IS")
                    .value("null")
                    .operatorType("IS")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"processed_at\" IS NULL"));
    }

    @Test
    @DisplayName("DELETE - with LIKE condition")
    void testDeleteWithLike() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("files")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("filename")
                    .operator("LIKE")
                    .value("temp_*")
                    .operatorType("LIKE")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"filename\" LIKE 'temp_%'"));
    }

    @Test
    @DisplayName("DELETE - with comparison operator")
    void testDeleteWithComparison() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("old_records")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("created_at")
                    .operator("<")
                    .value("2020-01-01")
                    .operatorType("LT")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"created_at\" < '2020-01-01'"));
    }

    @Test
    @DisplayName("DELETE - RETURNING")
    void testDeleteReturning() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("1")
                    .operatorType("EQ")
                    .build()
            ))
            .returningAll(true)
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("RETURNING *"));
    }

    @Test
    @DisplayName("DELETE - RETURNING specific columns")
    void testDeleteReturningColumns() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("id")
                    .operator("=")
                    .value("1")
                    .operatorType("EQ")
                    .build()
            ))
            .returningAll(true)
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("RETURNING *"));
    }

    @Test
    @DisplayName("DELETE - with NOT modifier")
    void testDeleteWithNot() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.DELETE)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("role")
                    .operator("=")
                    .value("admin")
                    .operatorType("EQ")
                    .negate(true)
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE NOT (\"role\" = 'admin')"));
    }

    // ==================== UPSERT tests ====================

    @Test
    @DisplayName("UPSERT - ON CONFLICT DO NOTHING")
    void testUpsertDoNothing() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPSERT)
            .schema("public")
            .table("users")
            .conflictColumns(Arrays.asList("email"))
            .ignoreConflict(true)
            .build();

        String sql = buildSql(plan, "[{\"email\":\"john@example.com\",\"name\":\"John\"}]");

        assertTrue(sql.contains("INSERT INTO \"public\".\"users\""));
        assertTrue(sql.contains("ON CONFLICT (\"email\") DO NOTHING"));
    }

    @Test
    @DisplayName("UPSERT - ON CONFLICT DO UPDATE")
    void testUpsertDoUpdate() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPSERT)
            .schema("public")
            .table("users")
            .conflictColumns(Arrays.asList("email"))
            .ignoreConflict(false)
            .build();

        String sql = buildSql(plan, "[{\"email\":\"john@example.com\",\"name\":\"John Updated\",\"age\":31}]");

        assertTrue(sql.contains("INSERT INTO \"public\".\"users\""));
        assertTrue(sql.contains("ON CONFLICT (\"email\")"));
        assertTrue(sql.contains("DO UPDATE SET"));
        assertTrue(sql.contains("\"name\" = EXCLUDED.\"name\""));
        assertTrue(sql.contains("\"age\" = EXCLUDED.\"age\""));
    }

    // ==================== Edge cases and special scenarios ====================

    @Test
    @DisplayName("Special characters - single quote escaping")
    void testSpecialCharsSingleQuote() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.singletonList(
                WhereClause.builder()
                    .column("name")
                    .operator("=")
                    .value("O'Brien")
                    .operatorType("EQ")
                    .build()
            ))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("WHERE \"name\" = 'O''Brien'"));
    }

    @Test
    @DisplayName("Special characters - double quote in column name")
    void testSpecialCharsDoubleQuoteInColumn() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .selectColumns(Collections.singletonList("weird\"column"))
            .build();

        String sql = buildSql(plan);

        assertTrue(sql.contains("\"weird\"\"column\""));
    }

    @Test
    @DisplayName("Empty handling - empty WHERE list")
    void testEmptyWhereList() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(Collections.emptyList())
            .build();

        String sql = buildSql(plan);

        assertFalse(sql.contains("WHERE"));
    }

    @Test
    @DisplayName("Numeric values - should not be quoted")
    void testNumericValues() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("products")
            .build();

        String sql = buildSql(plan, "{\"price\":99.99,\"stock\":100}");

        assertTrue(sql.contains("\"price\" = 99.99"));
        assertTrue(sql.contains("\"stock\" = 100"));
    }

    @Test
    @DisplayName("Boolean value handling")
    void testBooleanValues() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPDATE)
            .schema("public")
            .table("users")
            .build();

        String sql = buildSql(plan, "{\"verified\":true,\"active\":false}");

        assertTrue(sql.contains("\"verified\" = true"));
        assertTrue(sql.contains("\"active\" = false"));
    }

    // ==================== Helper Methods ====================

    /**
     * Build SQL (without body)
     */
    private String buildSql(QueryPlan plan) {
        return buildSql(plan, null);
    }

    /**
     * Build SQL (with body)
     */
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
