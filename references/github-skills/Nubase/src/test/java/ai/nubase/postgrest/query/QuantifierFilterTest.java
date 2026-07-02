package ai.nubase.postgrest.query;

import ai.nubase.postgrest.api.Filter;
import ai.nubase.postgrest.multidb.SchemaCacheManager;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.springframework.jdbc.core.JdbcTemplate;

import java.lang.reflect.Method;
import java.util.Collections;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.mock;

/**
 * Tests for any() and all() quantifier support in PostgREST-compatible API.
 *
 * PostgREST syntax:
 *   - column=eq(any).{1,2,3}  -> column = ANY(ARRAY[1, 2, 3])
 *   - column=lt(all).{10,20}  -> column < ALL(ARRAY[10, 20])
 *   - column=like(any).{%a%,%b%} -> column LIKE ANY(ARRAY['%a%', '%b%'])
 *
 * Supported operators with quantifiers: eq, neq, gt, gte, lt, lte, like, ilike, match, imatch
 */
@DisplayName("Quantifier Filter Tests (any/all)")
class QuantifierFilterTest {

    private QueryExecutor executor;
    private Method buildSQLMethod;

    @BeforeEach
    void setUp() throws Exception {
        JdbcTemplate jdbcTemplate = mock(JdbcTemplate.class);
        ObjectMapper objectMapper = new ObjectMapper();
        SchemaCacheManager schemaCacheManager = mock(SchemaCacheManager.class);
        executor = new QueryExecutor(jdbcTemplate, objectMapper, schemaCacheManager);

        // Access private buildSQL method via reflection
        buildSQLMethod = QueryExecutor.class.getDeclaredMethod("buildSQL", QueryPlan.class, String.class);
        buildSQLMethod.setAccessible(true);
    }

    private String buildSQL(QueryPlan plan) throws Exception {
        return (String) buildSQLMethod.invoke(executor, plan, null);
    }

    // ==================== ANY Quantifier Tests ====================

    @Nested
    @DisplayName("ANY quantifier tests")
    class AnyQuantifierTests {

        @Test
        @DisplayName("eq(any) - equals any value in array")
        void testEqAny() throws Exception {
            // PostgREST: ?id=eq(any).{3,4,5}
            // SQL: "id" = ANY(ARRAY['3', '4', '5'])
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("projects")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("id")
                        .operator("=")
                        .value("{3,4,5}")
                        .operatorType("EQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("= ANY(ARRAY["), "Should use ANY with ARRAY: " + sql);
            assertTrue(sql.contains("'3'") && sql.contains("'4'") && sql.contains("'5'"),
                "Should contain all values: " + sql);
        }

        @Test
        @DisplayName("neq(any) - not equals any value")
        void testNeqAny() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("status")
                        .operator("!=")
                        .value("{active,pending}")
                        .operatorType("NEQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("!= ANY(ARRAY["), "Should use != ANY: " + sql);
        }

        @Test
        @DisplayName("like(any) - pattern match any")
        void testLikeAny() throws Exception {
            // PostgREST: ?body=like(any).{%plan%,%brain%}
            // SQL: "body" LIKE ANY(ARRAY['%plan%', '%brain%'])
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("articles")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("body")
                        .operator("LIKE")
                        .value("{%plan%,%brain%}")
                        .operatorType("LIKE")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("LIKE ANY(ARRAY["), "Should use LIKE ANY: " + sql);
            assertTrue(sql.contains("'%plan%'") && sql.contains("'%brain%'"),
                "Should contain patterns: " + sql);
        }

        @Test
        @DisplayName("like(any) with asterisk wildcard")
        void testLikeAnyWithAsterisk() throws Exception {
            // PostgREST: ?name=like(any).{O*,P*}
            // SQL: "name" LIKE ANY(ARRAY['O%', 'P%'])
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("name")
                        .operator("LIKE")
                        .value("{O*,P*}")
                        .operatorType("LIKE")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("LIKE ANY(ARRAY["), "Should use LIKE ANY: " + sql);
            assertTrue(sql.contains("'O%'") && sql.contains("'P%'"),
                "Asterisks should be converted to %: " + sql);
        }

        @Test
        @DisplayName("ilike(any) - case-insensitive pattern match")
        void testIlikeAny() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("products")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("name")
                        .operator("ILIKE")
                        .value("{%apple%,%orange%}")
                        .operatorType("ILIKE")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("ILIKE ANY(ARRAY["), "Should use ILIKE ANY: " + sql);
        }

        @Test
        @DisplayName("match(any) - regex match any pattern")
        void testMatchAny() throws Exception {
            // PostgREST: ?body=match(any).{stop,thing}
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("articles")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("body")
                        .operator("~")
                        .value("{stop,thing}")
                        .operatorType("MATCH")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("~ ANY(ARRAY["), "Should use ~ ANY: " + sql);
            assertTrue(sql.contains("'stop'") && sql.contains("'thing'"),
                "Should contain regex patterns: " + sql);
        }

        @Test
        @DisplayName("imatch(any) - case-insensitive regex")
        void testImatchAny() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("articles")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("body")
                        .operator("~*")
                        .value("{STOP,THING}")
                        .operatorType("IMATCH")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("~* ANY(ARRAY["), "Should use ~* ANY: " + sql);
        }
    }

    // ==================== ALL Quantifier Tests ====================

    @Nested
    @DisplayName("ALL quantifier tests")
    class AllQuantifierTests {

        @Test
        @DisplayName("gt(all) - greater than all values")
        void testGtAll() throws Exception {
            // PostgREST: ?id=gt(all).{4,3}
            // Returns rows where id > 4 AND id > 3 (effectively id > 4)
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("projects")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("id")
                        .operator(">")
                        .value("{4,3}")
                        .operatorType("GT")
                        .quantifier(Filter.Quantifier.ALL)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("> ALL(ARRAY["), "Should use > ALL: " + sql);
            assertTrue(sql.contains("'4'") && sql.contains("'3'"),
                "Should contain all values: " + sql);
        }

        @Test
        @DisplayName("gte(all) - greater than or equal to all values")
        void testGteAll() throws Exception {
            // PostgREST: ?id=gte(all).{4,3}
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("projects")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("id")
                        .operator(">=")
                        .value("{4,3}")
                        .operatorType("GTE")
                        .quantifier(Filter.Quantifier.ALL)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains(">= ALL(ARRAY["), "Should use >= ALL: " + sql);
        }

        @Test
        @DisplayName("lt(all) - less than all values")
        void testLtAll() throws Exception {
            // PostgREST: ?id=lt(all).{4,3}
            // Returns rows where id < 4 AND id < 3 (effectively id < 3)
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("projects")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("id")
                        .operator("<")
                        .value("{4,3}")
                        .operatorType("LT")
                        .quantifier(Filter.Quantifier.ALL)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("< ALL(ARRAY["), "Should use < ALL: " + sql);
        }

        @Test
        @DisplayName("lte(all) - less than or equal to all values")
        void testLteAll() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("projects")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("id")
                        .operator("<=")
                        .value("{4,3}")
                        .operatorType("LTE")
                        .quantifier(Filter.Quantifier.ALL)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("<= ALL(ARRAY["), "Should use <= ALL: " + sql);
        }

        @Test
        @DisplayName("ilike(all) - must match all patterns")
        void testIlikeAll() throws Exception {
            // PostgREST: ?body=ilike(all).{%plan%,%greatness%}
            // Returns rows where body matches BOTH patterns
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("articles")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("body")
                        .operator("ILIKE")
                        .value("{%plan%,%greatness%}")
                        .operatorType("ILIKE")
                        .quantifier(Filter.Quantifier.ALL)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("ILIKE ALL(ARRAY["), "Should use ILIKE ALL: " + sql);
            assertTrue(sql.contains("'%plan%'") && sql.contains("'%greatness%'"),
                "Should contain all patterns: " + sql);
        }

        @Test
        @DisplayName("like(all) with asterisk wildcards")
        void testLikeAllWithAsterisk() throws Exception {
            // PostgREST: ?name=like(all).{*son,J*}
            // Must match both patterns
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("name")
                        .operator("LIKE")
                        .value("{*son,J*}")
                        .operatorType("LIKE")
                        .quantifier(Filter.Quantifier.ALL)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("LIKE ALL(ARRAY["), "Should use LIKE ALL: " + sql);
            assertTrue(sql.contains("'%son'") && sql.contains("'J%'"),
                "Asterisks should be converted to %: " + sql);
        }
    }

    // ==================== Negation Tests ====================

    @Nested
    @DisplayName("Negation with quantifiers")
    class NegationTests {

        @Test
        @DisplayName("not.eq(any) - negate any match")
        void testNotEqAny() throws Exception {
            // PostgREST: ?id=not.eq(any).{1,2,3}
            // SQL: NOT ("id" = ANY(ARRAY['1', '2', '3']))
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("projects")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("id")
                        .operator("=")
                        .value("{1,2,3}")
                        .operatorType("EQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .negate(true)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("NOT ("), "Should have NOT wrapper: " + sql);
            assertTrue(sql.contains("= ANY(ARRAY["), "Should use = ANY: " + sql);
        }

        @Test
        @DisplayName("not.gt(all) - negate all comparison")
        void testNotGtAll() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("projects")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("id")
                        .operator(">")
                        .value("{5,10}")
                        .operatorType("GT")
                        .quantifier(Filter.Quantifier.ALL)
                        .negate(true)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("NOT ("), "Should have NOT wrapper: " + sql);
            assertTrue(sql.contains("> ALL(ARRAY["), "Should use > ALL: " + sql);
        }
    }

    // ==================== Edge Cases ====================

    @Nested
    @DisplayName("Edge cases")
    class EdgeCaseTests {

        @Test
        @DisplayName("Single value in array")
        void testSingleValueArray() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("id")
                        .operator("=")
                        .value("{42}")
                        .operatorType("EQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("= ANY(ARRAY['42'])"), "Should work with single value: " + sql);
        }

        @Test
        @DisplayName("Values with special characters")
        void testValuesWithSpecialChars() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("email")
                        .operator("=")
                        .value("{test@example.com,user+tag@domain.org}")
                        .operatorType("EQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("= ANY(ARRAY["), "Should handle special characters: " + sql);
            assertTrue(sql.contains("test@example.com"), "Should preserve @ symbol: " + sql);
        }

        @Test
        @DisplayName("Numeric values")
        void testNumericValues() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("products")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("price")
                        .operator(">=")
                        .value("{10.99,20.50,30.00}")
                        .operatorType("GTE")
                        .quantifier(Filter.Quantifier.ALL)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains(">= ALL(ARRAY["), "Should handle numeric values: " + sql);
        }

        @Test
        @DisplayName("Empty strings in array")
        void testEmptyStringsInArray() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("items")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("tag")
                        .operator("=")
                        .value("{a,,b}")
                        .operatorType("EQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("= ANY(ARRAY["), "Should handle empty strings: " + sql);
        }

        @Test
        @DisplayName("Value without curly braces (fallback)")
        void testValueWithoutBraces() throws Exception {
            // Should still work even without braces (single value case)
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("name")
                        .operator("LIKE")
                        .value("John*")
                        .operatorType("LIKE")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("LIKE ANY(ARRAY["), "Should work without braces: " + sql);
            assertTrue(sql.contains("'John%'"), "Should convert asterisk: " + sql);
        }
    }

    // ==================== SQL Syntax Verification ====================

    @Nested
    @DisplayName("SQL syntax verification")
    class SqlSyntaxTests {

        @Test
        @DisplayName("Correct ARRAY syntax generated")
        void testCorrectArraySyntax() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("test")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("col")
                        .operator("=")
                        .value("{a,b,c}")
                        .operatorType("EQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            // Verify proper SQL syntax
            assertTrue(sql.matches(".*= ANY\\(ARRAY\\[.*\\]\\).*"),
                "Should have proper ANY(ARRAY[...]) syntax: " + sql);
        }

        @Test
        @DisplayName("Column properly quoted")
        void testColumnQuoted() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("user_id")
                        .operator("=")
                        .value("{1,2}")
                        .operatorType("EQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("\"user_id\""), "Column should be quoted: " + sql);
        }

        @Test
        @DisplayName("Values properly quoted")
        void testValuesQuoted() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("test")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column("name")
                        .operator("=")
                        .value("{hello,world}")
                        .operatorType("EQ")
                        .quantifier(Filter.Quantifier.ANY)
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("'hello'") && sql.contains("'world'"),
                "Values should be quoted: " + sql);
        }
    }
}
