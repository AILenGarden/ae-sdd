package ai.nubase.postgrest.query;

import ai.nubase.postgrest.multidb.SchemaCacheManager;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;
import org.springframework.jdbc.core.JdbcTemplate;
import com.fasterxml.jackson.databind.ObjectMapper;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.*;

/**
 * Complete tests for all 25+ PostgREST filter operators
 * Covers every operator documented in the official PostgREST documentation
 */
@DisplayName("Filter operators complete tests")
class FilterOperatorsTest {

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

    @Test
    @DisplayName("Basic comparison operator - EQ (equal)")
    void testEqualOperator() {
        WhereClause clause = WhereClause.builder()
            .column("age")
            .operator("=")
            .value("25")
            .operatorType("EQ")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"age\" = '25'", sql);
    }

    @Test
    @DisplayName("Basic comparison operator - NEQ (not equal)")
    void testNotEqualOperator() {
        WhereClause clause = WhereClause.builder()
            .column("status")
            .operator("!=")
            .value("inactive")
            .operatorType("NEQ")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"status\" != 'inactive'", sql);
    }

    @ParameterizedTest
    @CsvSource({
        "GT, >, 18, WHERE \"age\" > '18'",
        "GTE, >=, 18, WHERE \"age\" >= '18'",
        "LT, <, 65, WHERE \"age\" < '65'",
        "LTE, <=, 65, WHERE \"age\" <= '65'"
    })
    @DisplayName("Comparison operators - GT, GTE, LT, LTE")
    void testComparisonOperators(String opType, String operator, String value, String expected) {
        WhereClause clause = WhereClause.builder()
            .column("age")
            .operator(operator)
            .value(value)
            .operatorType(opType)
            .build();

        String sql = buildWhereClause(clause);
        assertEquals(expected, sql);
    }

    @Test
    @DisplayName("LIKE operator - wildcard conversion")
    void testLikeOperator() {
        WhereClause clause = WhereClause.builder()
            .column("name")
            .operator("LIKE")
            .value("*John*")
            .operatorType("LIKE")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"name\" LIKE '%John%'", sql);
    }

    @Test
    @DisplayName("ILIKE operator - case-insensitive")
    void testILikeOperator() {
        WhereClause clause = WhereClause.builder()
            .column("email")
            .operator("ILIKE")
            .value("*@gmail.com")
            .operatorType("ILIKE")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"email\" ILIKE '%@gmail.com'", sql);
    }

    @Test
    @DisplayName("MATCH operator - regular expression")
    void testMatchOperator() {
        WhereClause clause = WhereClause.builder()
            .column("description")
            .operator("~")
            .value("^Project.*")
            .operatorType("MATCH")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"description\" ~ '^Project.*'", sql);
    }

    @Test
    @DisplayName("IMATCH operator - case-insensitive regex")
    void testIMatchOperator() {
        WhereClause clause = WhereClause.builder()
            .column("title")
            .operator("~*")
            .value("(?i)important")
            .operatorType("IMATCH")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"title\" ~* '(?i)important'", sql);
    }

    @Test
    @DisplayName("IN operator - list query")
    void testInOperator() {
        WhereClause clause = WhereClause.builder()
            .column("status")
            .operator("IN")
            .value("(active,pending,approved)")
            .operatorType("IN")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"status\" IN ('active', 'pending', 'approved')", sql);
    }

    @Test
    @DisplayName("IS operator - NULL check")
    void testIsNullOperator() {
        WhereClause clause = WhereClause.builder()
            .column("deleted_at")
            .operator("IS")
            .value("null")
            .operatorType("IS")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"deleted_at\" IS NULL", sql);
    }

    @ParameterizedTest
    @CsvSource({
        "true, WHERE \"is_active\" IS TRUE",
        "false, WHERE \"is_deleted\" IS FALSE",
        "unknown, WHERE \"status\" IS UNKNOWN"
    })
    @DisplayName("IS operator - boolean values")
    void testIsBooleanOperator(String value, String expected) {
        WhereClause clause = WhereClause.builder()
            .column(value.equals("true") ? "is_active" : value.equals("false") ? "is_deleted" : "status")
            .operator("IS")
            .value(value)
            .operatorType("IS")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals(expected, sql);
    }

    @Test
    @DisplayName("FTS operator - full-text search to_tsquery")
    void testFullTextSearchOperator() {
        WhereClause clause = WhereClause.builder()
            .column("content")
            .operator("@@")
            .value("fat & cat")
            .operatorType("FTS")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"content\" @@ to_tsquery('fat & cat')", sql);
    }

    @Test
    @DisplayName("PLFTS operator - plainto_tsquery")
    void testPlainTextSearchOperator() {
        WhereClause clause = WhereClause.builder()
            .column("description")
            .operator("@@")
            .value("The Fat Cats")
            .operatorType("PLFTS")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"description\" @@ plainto_tsquery('The Fat Cats')", sql);
    }

    @Test
    @DisplayName("PHFTS operator - phraseto_tsquery")
    void testPhraseTextSearchOperator() {
        WhereClause clause = WhereClause.builder()
            .column("article")
            .operator("@@")
            .value("quick brown fox")
            .operatorType("PHFTS")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"article\" @@ phraseto_tsquery('quick brown fox')", sql);
    }

    @Test
    @DisplayName("WFTS operator - websearch_to_tsquery")
    void testWebSearchOperator() {
        WhereClause clause = WhereClause.builder()
            .column("search_field")
            .operator("@@")
            .value("cats AND dogs")
            .operatorType("WFTS")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"search_field\" @@ websearch_to_tsquery('cats AND dogs')", sql);
    }

    @Test
    @DisplayName("CS operator - contains (@>)")
    void testContainsOperator() {
        WhereClause clause = WhereClause.builder()
            .column("tags")
            .operator("@>")
            .value("{tech,news}")
            .operatorType("CS")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"tags\" @> ARRAY['tech', 'news']", sql);
    }

    @Test
    @DisplayName("CD operator - contained in (<@)")
    void testContainedInOperator() {
        WhereClause clause = WhereClause.builder()
            .column("selected_items")
            .operator("<@")
            .value("{1,2,3,4,5}")
            .operatorType("CD")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"selected_items\" <@ ARRAY['1', '2', '3', '4', '5']", sql);
    }

    @Test
    @DisplayName("OV operator - overlap (&&)")
    void testOverlapOperator() {
        WhereClause clause = WhereClause.builder()
            .column("available_dates")
            .operator("&&")
            .value("[2024-01-01,2024-12-31]")
            .operatorType("OV")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"available_dates\" && '[2024-01-01,2024-12-31]'", sql);
    }

    @Test
    @DisplayName("SL operator - strictly left (<<)")
    void testStrictlyLeftOperator() {
        WhereClause clause = WhereClause.builder()
            .column("int_range")
            .operator("<<")
            .value("(10,20)")
            .operatorType("SL")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"int_range\" << '(10,20)'", sql);
    }

    @Test
    @DisplayName("SR operator - strictly right (>>)")
    void testStrictlyRightOperator() {
        WhereClause clause = WhereClause.builder()
            .column("int_range")
            .operator(">>")
            .value("(100,200)")
            .operatorType("SR")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"int_range\" >> '(100,200)'", sql);
    }

    @Test
    @DisplayName("NXR operator - not extend right (&<)")
    void testNotExtendRightOperator() {
        WhereClause clause = WhereClause.builder()
            .column("date_range")
            .operator("&<")
            .value("[2024-01-01,2024-06-30]")
            .operatorType("NXR")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"date_range\" &< '[2024-01-01,2024-06-30]'", sql);
    }

    @Test
    @DisplayName("NXL operator - not extend left (&>)")
    void testNotExtendLeftOperator() {
        WhereClause clause = WhereClause.builder()
            .column("date_range")
            .operator("&>")
            .value("[2024-07-01,2024-12-31]")
            .operatorType("NXL")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"date_range\" &> '[2024-07-01,2024-12-31]'", sql);
    }

    @Test
    @DisplayName("ADJ operator - adjacent (-|-)")
    void testAdjacentOperator() {
        WhereClause clause = WhereClause.builder()
            .column("period")
            .operator("-|-")
            .value("[2024-01-01,2024-01-31]")
            .operatorType("ADJ")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE \"period\" -|- '[2024-01-01,2024-01-31]'", sql);
    }

    @Test
    @DisplayName("ISDISTINCT operator - IS DISTINCT FROM")
    void testIsDistinctFromOperator() {
        WhereClause clause = WhereClause.builder()
            .column("value")
            .operator("IS DISTINCT FROM")
            .value("null")
            .operatorType("ISDISTINCT")
            .build();

        String sql = buildWhereClause(clause);
        assertTrue(sql.contains("IS DISTINCT FROM"));
    }

    @Test
    @DisplayName("NOT modifier - negate operator")
    void testNegateOperator() {
        WhereClause clause = WhereClause.builder()
            .column("status")
            .operator("=")
            .value("inactive")
            .negate(true)
            .operatorType("EQ")
            .build();

        String sql = buildWhereClause(clause);
        assertEquals("WHERE NOT (\"status\" = 'inactive')", sql);
    }

    @Test
    @DisplayName("Multiple filter conditions - AND-connected")
    void testMultipleFiltersWithAnd() {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("users")
            .whereClauses(java.util.Arrays.asList(
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
            .build();

        String sql = buildSelectSQL(plan);
        assertTrue(sql.contains("WHERE \"age\" >= '18' AND \"status\" = 'active'"));
    }

    // Helper methods
    private String buildWhereClause(WhereClause clause) {
        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.SELECT)
            .schema("public")
            .table("test_table")
            .whereClauses(java.util.Collections.singletonList(clause))
            .build();

        return extractWhereClause(buildSelectSQL(plan));
    }

    private String buildSelectSQL(QueryPlan plan) {
        // Use reflection to invoke QueryExecutor's private method, or create a test helper
        try {
            java.lang.reflect.Method method = QueryExecutor.class.getDeclaredMethod("buildSQL", QueryPlan.class, String.class);
            method.setAccessible(true);
            return (String) method.invoke(queryExecutor, plan, null);
        } catch (Exception e) {
            throw new RuntimeException("Failed to build SQL", e);
        }
    }

    private String extractWhereClause(String fullSql) {
        int whereIndex = fullSql.indexOf("WHERE");
        if (whereIndex == -1) return "";

        // Extract WHERE up to ORDER BY/LIMIT/end of string
        int endIndex = fullSql.length();
        for (String keyword : new String[]{"ORDER BY", "LIMIT", "OFFSET"}) {
            int idx = fullSql.indexOf(keyword, whereIndex);
            if (idx != -1 && idx < endIndex) {
                endIndex = idx;
            }
        }

        return fullSql.substring(whereIndex, endIndex).strip();
    }
}
