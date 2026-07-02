package ai.nubase.postgrest.query;

import ai.nubase.postgrest.multidb.SchemaCacheManager;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.springframework.jdbc.core.JdbcTemplate;

import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.Collections;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.mock;

/**
 * Tests for asterisk (*) and whitespace handling in SQL generation.
 *
 * Key requirements:
 * 1. Asterisk (*) should NEVER be quoted - "SELECT *" not "SELECT \"*\""
 * 2. Whitespace in identifiers should be trimmed - "profiles" not "profiles "
 */
@DisplayName("Asterisk and Whitespace Handling Tests")
class AsteriskAndWhitespaceTest {

    private QueryExecutor executor;
    private Method buildSQLMethod;
    private Method quoteMethod;
    private Method quoteQualifiedColumnMethod;

    @BeforeEach
    void setUp() throws Exception {
        JdbcTemplate jdbcTemplate = mock(JdbcTemplate.class);
        ObjectMapper objectMapper = new ObjectMapper();
        SchemaCacheManager schemaCacheManager = mock(SchemaCacheManager.class);
        executor = new QueryExecutor(jdbcTemplate, objectMapper, schemaCacheManager);

        // Access private methods via reflection
        buildSQLMethod = QueryExecutor.class.getDeclaredMethod("buildSQL", QueryPlan.class, String.class);
        buildSQLMethod.setAccessible(true);

        quoteMethod = QueryExecutor.class.getDeclaredMethod("quote", String.class);
        quoteMethod.setAccessible(true);

        quoteQualifiedColumnMethod = QueryExecutor.class.getDeclaredMethod("quoteQualifiedColumn", String.class);
        quoteQualifiedColumnMethod.setAccessible(true);
    }

    private String buildSQL(QueryPlan plan) throws Exception {
        return (String) buildSQLMethod.invoke(executor, plan, null);
    }

    private String quote(String identifier) throws Exception {
        return (String) quoteMethod.invoke(executor, identifier);
    }

    private String quoteQualifiedColumn(String column) throws Exception {
        return (String) quoteQualifiedColumnMethod.invoke(executor, column);
    }

    // ==================== Asterisk Tests ====================

    @Nested
    @DisplayName("Asterisk (*) should not be quoted")
    class AsteriskTests {

        @Test
        @DisplayName("SELECT * - asterisk in select all")
        void testSelectAllAsteriskNotQuoted() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .build();

            String sql = buildSQL(plan);

            // Should be: SELECT "users".* FROM "public"."users"
            // NOT: SELECT "users"."*" or SELECT "*"
            assertTrue(sql.contains("\"users\".*"), "Asterisk should not be quoted: " + sql);
            assertFalse(sql.contains("\"*\""), "Asterisk should not be in quotes: " + sql);
        }

        @Test
        @DisplayName("SELECT * with explicit column list containing *")
        void testSelectColumnsWithAsterisk() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .selectColumns(Collections.singletonList("*"))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("\"users\".*"), "Asterisk should not be quoted: " + sql);
            assertFalse(sql.contains("\"*\""), "Asterisk should not be in quotes: " + sql);
        }

        @Test
        @DisplayName("quoteQualifiedColumn with standalone *")
        void testQuoteQualifiedColumnStandaloneAsterisk() throws Exception {
            String result = quoteQualifiedColumn("*");
            assertEquals("*", result, "Standalone asterisk should not be quoted");
        }

        @Test
        @DisplayName("quoteQualifiedColumn with table.*")
        void testQuoteQualifiedColumnTableAsterisk() throws Exception {
            String result = quoteQualifiedColumn("users.*");
            assertEquals("\"users\".*", result, "Table asterisk format should be correct");
        }

        @Test
        @DisplayName("quoteQualifiedColumn with * with spaces")
        void testQuoteQualifiedColumnAsteriskWithSpaces() throws Exception {
            String result = quoteQualifiedColumn(" * ");
            assertEquals("*", result, "Asterisk with spaces should be trimmed and not quoted");
        }

        @Test
        @DisplayName("quoteQualifiedColumn with table.* with spaces")
        void testQuoteQualifiedColumnTableAsteriskWithSpaces() throws Exception {
            String result = quoteQualifiedColumn(" users . * ");
            assertEquals("\"users\".*", result, "Table asterisk with spaces should be trimmed correctly");
        }

        @Test
        @DisplayName("SELECT with mixed columns including *")
        void testSelectMixedColumnsWithAsterisk() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .selectColumns(Arrays.asList("id", "*", "name"))
                .build();

            String sql = buildSQL(plan);

            // Should contain unquoted asterisk
            assertFalse(sql.contains("\"*\""), "Asterisk should not be in quotes: " + sql);
            // Should contain quoted regular columns
            assertTrue(sql.contains("\"id\""), "Regular columns should be quoted: " + sql);
            assertTrue(sql.contains("\"name\""), "Regular columns should be quoted: " + sql);
        }
    }

    // ==================== Whitespace Tests ====================

    @Nested
    @DisplayName("Whitespace should be trimmed")
    class WhitespaceTests {

        @Test
        @DisplayName("quote() should trim whitespace")
        void testQuoteTrimsWhitespace() throws Exception {
            assertEquals("\"users\"", quote(" users "), "Leading/trailing spaces should be trimmed");
            assertEquals("\"users\"", quote("users "), "Trailing space should be trimmed");
            assertEquals("\"users\"", quote(" users"), "Leading space should be trimmed");
            assertEquals("\"users\"", quote("\tusers\t"), "Tabs should be trimmed");
        }

        @Test
        @DisplayName("quoteQualifiedColumn() should trim whitespace")
        void testQuoteQualifiedColumnTrimsWhitespace() throws Exception {
            assertEquals("\"id\"", quoteQualifiedColumn(" id "));
            assertEquals("\"users\".\"id\"", quoteQualifiedColumn(" users . id "));
            assertEquals("\"users\".\"id\"", quoteQualifiedColumn("users.id "));
        }

        @Test
        @DisplayName("SELECT with table name having trailing space")
        void testSelectWithTableNameSpace() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("profiles ")  // Note: trailing space
                .build();

            String sql = buildSQL(plan);

            // Should be trimmed
            assertTrue(sql.contains("\"profiles\""), "Table name should be trimmed: " + sql);
            assertFalse(sql.contains("\"profiles \""), "Table name should not have trailing space: " + sql);
        }

        @Test
        @DisplayName("SELECT with schema name having spaces")
        void testSelectWithSchemaNameSpace() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema(" public ")  // Note: spaces
                .table("users")
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("\"public\""), "Schema name should be trimmed: " + sql);
            assertFalse(sql.contains("\" public \""), "Schema name should not have spaces: " + sql);
        }

        @Test
        @DisplayName("SELECT with column names having spaces")
        void testSelectWithColumnNameSpaces() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .selectColumns(Arrays.asList(" id ", " name ", " email "))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("\"id\""), "Column 'id' should be trimmed: " + sql);
            assertTrue(sql.contains("\"name\""), "Column 'name' should be trimmed: " + sql);
            assertTrue(sql.contains("\"email\""), "Column 'email' should be trimmed: " + sql);
            assertFalse(sql.contains("\" id \""), "Column should not have spaces: " + sql);
        }

        @Test
        @DisplayName("WHERE clause with column name having spaces")
        void testWhereWithColumnNameSpaces() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column(" id ")  // Note: spaces
                        .operator("=")
                        .value("123")
                        .operatorType("EQ")
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("\"id\""), "Column in WHERE should be trimmed: " + sql);
            assertFalse(sql.contains("\" id \""), "Column in WHERE should not have spaces: " + sql);
        }

        @Test
        @DisplayName("quote() should trim non-breaking space (U+00A0)")
        void testQuoteTrimsNonBreakingSpace() throws Exception {
            // U+00A0 is non-breaking space - Java's strip() does NOT remove it!
            String nbsp = "\u00A0";
            assertEquals("\"users\"", quote(nbsp + "users" + nbsp), "Non-breaking spaces should be trimmed");
            assertEquals("\"users\"", quote("users" + nbsp), "Trailing non-breaking space should be trimmed");
            assertEquals("\"users\"", quote(nbsp + "users"), "Leading non-breaking space should be trimmed");
        }

        @Test
        @DisplayName("quote() should trim figure space (U+2007)")
        void testQuoteTrimsFigureSpace() throws Exception {
            // U+2007 is figure space - Java's strip() does NOT remove it!
            String figureSpace = "\u2007";
            assertEquals("\"users\"", quote(figureSpace + "users" + figureSpace), "Figure spaces should be trimmed");
        }

        @Test
        @DisplayName("quote() should trim narrow no-break space (U+202F)")
        void testQuoteTrimsNarrowNoBreakSpace() throws Exception {
            // U+202F is narrow no-break space - Java's strip() does NOT remove it!
            String narrowNbsp = "\u202F";
            assertEquals("\"users\"", quote(narrowNbsp + "users" + narrowNbsp), "Narrow no-break spaces should be trimmed");
        }

        @Test
        @DisplayName("quote() should trim mixed special whitespace")
        void testQuoteTrimsMixedSpecialWhitespace() throws Exception {
            // Mix of regular space, non-breaking space, and tab
            String mixed = " \u00A0\tusers\t\u00A0 ";
            assertEquals("\"users\"", quote(mixed), "Mixed whitespace including non-breaking space should be trimmed");
        }
    }

    // ==================== Combined Tests ====================

    @Nested
    @DisplayName("Combined asterisk and whitespace scenarios")
    class CombinedTests {

        @Test
        @DisplayName("SELECT * with spaces around asterisk")
        void testSelectAsteriskWithSpaces() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .selectColumns(Collections.singletonList(" * "))  // Spaces around asterisk
                .build();

            String sql = buildSQL(plan);

            assertTrue(sql.contains("\"users\".*"), "Asterisk should be handled correctly: " + sql);
            assertFalse(sql.contains("\"*\""), "Asterisk should not be quoted: " + sql);
            assertFalse(sql.contains("\" * \""), "Spaces should be trimmed: " + sql);
        }

        @Test
        @DisplayName("Full query with spaces and asterisk")
        void testFullQueryWithSpacesAndAsterisk() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema(" public ")
                .table(" profiles ")
                .selectColumns(Collections.singletonList(" * "))
                .whereClauses(Collections.singletonList(
                    WhereClause.builder()
                        .column(" id ")
                        .operator("=")
                        .value("123")
                        .operatorType("EQ")
                        .build()
                ))
                .build();

            String sql = buildSQL(plan);

            // Verify correct SQL structure
            assertTrue(sql.contains("\"profiles\".*"), "Should have correct SELECT: " + sql);
            assertTrue(sql.contains("FROM \"public\".\"profiles\""), "Should have correct FROM: " + sql);
            assertTrue(sql.contains("\"id\""), "Should have correct WHERE column: " + sql);

            // Verify no incorrect patterns
            assertFalse(sql.contains("\"*\""), "Asterisk should not be quoted: " + sql);
            // Check for spaces inside quotes (e.g., "\" id\"" or "\"id \"")
            assertFalse(sql.contains("\" public\""), "Schema should not have leading space: " + sql);
            assertFalse(sql.contains("\"public \""), "Schema should not have trailing space: " + sql);
            assertFalse(sql.contains("\" profiles\""), "Table should not have leading space: " + sql);
            assertFalse(sql.contains("\"profiles \""), "Table should not have trailing space: " + sql);
            assertFalse(sql.contains("\" id\""), "Column should not have leading space: " + sql);
            assertFalse(sql.contains("\"id \""), "Column should not have trailing space: " + sql);
        }
    }

    // ==================== Embedded Resource (JOIN) Tests ====================

    @Nested
    @DisplayName("Embedded resource (JOIN) asterisk handling")
    class EmbeddedResourceTests {

        @Test
        @DisplayName("Embedded resource SELECT * should not be quoted")
        void testEmbeddedResourceSelectAll() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("posts")
                .embeddingName("posts")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("id")
                        .operator("=")
                        .rightColumn("user_id")
                        .build()
                ))
                // No selectColumns means SELECT *
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // The embedded subquery should have SELECT * not SELECT "*"
            // Pattern: COALESCE((SELECT row_to_json(...) FROM (SELECT * FROM ...
            assertTrue(sql.contains("SELECT *"), "Embedded resource should have unquoted SELECT *: " + sql);
            assertFalse(sql.contains("SELECT \"*\""), "Embedded resource should not have quoted asterisk: " + sql);
        }

        @Test
        @DisplayName("Embedded resource with explicit * column should not be quoted")
        void testEmbeddedResourceExplicitAsterisk() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("posts")
                .embeddingName("posts")
                .selectColumns(Collections.singletonList("*"))  // Explicit asterisk
                .conditions(Collections.singletonList(
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

            String sql = buildSQL(plan);

            // Check the subquery has unquoted asterisk
            assertFalse(sql.contains("\"*\""), "Explicit asterisk in embedded resource should not be quoted: " + sql);
        }

        @Test
        @DisplayName("Embedded resource with mixed columns including *")
        void testEmbeddedResourceMixedColumnsWithAsterisk() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("posts")
                .embeddingName("posts")
                .selectColumns(Arrays.asList("id", "*", "title"))
                .conditions(Collections.singletonList(
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

            String sql = buildSQL(plan);

            // Asterisk should not be quoted, but other columns should be
            assertFalse(sql.contains("\"*\""), "Asterisk should not be quoted: " + sql);
        }
    }
}
