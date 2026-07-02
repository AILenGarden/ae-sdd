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
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.mock;

/**
 * Tests for PostgREST-style embedded resource SQL generation.
 *
 * Verifies that:
 * 1. Embedded resources use subqueries instead of JOINs
 * 2. SQL uses row_to_json for proper JSON nesting
 * 3. COALESCE handles NULL values (LEFT JOIN semantics)
 * 4. Column name conflicts are avoided via subquery isolation
 */
@DisplayName("Embedded Resource SQL Generation (PostgREST Style)")
class EmbeddedResourceSqlGenerationTest {

    private QueryExecutor executor;
    private Method buildSQLMethod;
    private Method convertToNestedStructureMethod;
    private Method parseEmbeddedValueMethod;
    private ObjectMapper objectMapper;
    private SchemaCacheManager schemaCacheManager;

    @BeforeEach
    void setUp() throws Exception {
        JdbcTemplate jdbcTemplate = mock(JdbcTemplate.class);
        objectMapper = new ObjectMapper();
        schemaCacheManager = mock(SchemaCacheManager.class);
        executor = new QueryExecutor(jdbcTemplate, objectMapper, schemaCacheManager);

        // Access private methods via reflection
        buildSQLMethod = QueryExecutor.class.getDeclaredMethod("buildSQL", QueryPlan.class, String.class);
        buildSQLMethod.setAccessible(true);

        convertToNestedStructureMethod = QueryExecutor.class.getDeclaredMethod(
            "convertToNestedStructure", List.class, QueryPlan.class);
        convertToNestedStructureMethod.setAccessible(true);

        parseEmbeddedValueMethod = QueryExecutor.class.getDeclaredMethod(
            "parseEmbeddedValue", Object.class, String.class);
        parseEmbeddedValueMethod.setAccessible(true);
    }

    private String buildSQL(QueryPlan plan) throws Exception {
        return (String) buildSQLMethod.invoke(executor, plan, null);
    }

    @SuppressWarnings("unchecked")
    private List<Map<String, Object>> convertToNestedStructure(
            List<Map<String, Object>> rows, QueryPlan plan) throws Exception {
        return (List<Map<String, Object>>) convertToNestedStructureMethod.invoke(executor, rows, plan);
    }

    private Object parseEmbeddedValue(Object value, String embeddingName) throws Exception {
        return parseEmbeddedValueMethod.invoke(executor, value, embeddingName);
    }

    // ==================== SQL Generation Tests ====================

    @Nested
    @DisplayName("SQL Generation for Many-to-One Relationships")
    class ManyToOneSqlGeneration {

        @Test
        @DisplayName("Should generate subquery with row_to_json for embedded resource")
        void testSubqueryGeneration() throws Exception {
            // student_courses -> course (many-to-one)
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("student_courses.course_id")
                        .rightColumn("course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .selectColumns(Arrays.asList("student_courses.id", "student_courses.student_id"))
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // Verify subquery structure
            assertTrue(sql.contains("COALESCE("), "Should use COALESCE for NULL handling");
            assertTrue(sql.contains("row_to_json("), "Should use row_to_json for JSON conversion");
            assertTrue(sql.contains("AS \"course\""), "Should alias result as embedding name");

            // Verify it's a subquery, not a JOIN
            assertFalse(sql.contains("LEFT JOIN \"public\".\"courses\""),
                "Should NOT use direct JOIN, but subquery instead");
            assertTrue(sql.contains("FROM \"public\".\"courses\""),
                "Should have courses in subquery FROM clause");
        }

        @Test
        @DisplayName("Should generate correct WHERE condition in subquery")
        void testSubqueryWhereCondition() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("student_courses.course_id")
                        .rightColumn("course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // The subquery WHERE should link embedded table to parent table
            assertTrue(sql.contains("WHERE \"courses\".\"id\" = \"student_courses\".\"course_id\""),
                "Subquery should have correct WHERE condition linking tables");
        }

        @Test
        @DisplayName("Should select specific columns when specified")
        void testSpecificColumnsSelection() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .selectColumns(Arrays.asList("course.id", "course.name", "course.code"))
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("student_courses.course_id")
                        .rightColumn("course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // Should select only specified columns
            assertTrue(sql.contains("SELECT \"id\", \"name\", \"code\""),
                "Should select only specified columns in subquery");
        }

        @Test
        @DisplayName("Should select all columns when none specified")
        void testAllColumnsSelection() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .selectColumns(null) // No specific columns
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("student_courses.course_id")
                        .rightColumn("course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // Should use * for all columns
            assertTrue(sql.contains("SELECT *") || sql.contains("SELECT * FROM"),
                "Should select all columns when none specified");
        }
    }

    @Nested
    @DisplayName("SQL Generation with Aliases")
    class AliasHandling {

        @Test
        @DisplayName("Should use alias as embedding name")
        void testAliasAsEmbeddingName() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .alias("enrolled_course")
                .embeddingName("enrolled_course")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("student_courses.course_id")
                        .rightColumn("enrolled_course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // Should use alias in the result
            assertTrue(sql.contains("AS \"enrolled_course\""),
                "Should use alias as the embedded resource name");
        }
    }

    @Nested
    @DisplayName("Multiple Embedded Resources")
    class MultipleEmbeddedResources {

        @Test
        @DisplayName("Should handle multiple embedded resources")
        void testMultipleEmbeds() throws Exception {
            JoinClause courseJoin = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("enrollments.course_id")
                        .rightColumn("course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            JoinClause studentJoin = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("students")
                .embeddingName("student")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("enrollments.student_id")
                        .rightColumn("student.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("enrollments")
                .joins(Arrays.asList(courseJoin, studentJoin))
                .build();

            String sql = buildSQL(plan);

            // Should have subqueries for both embedded resources
            assertTrue(sql.contains("AS \"course\""), "Should have course embedding");
            assertTrue(sql.contains("AS \"student\""), "Should have student embedding");

            // Count occurrences of COALESCE (one per embedding)
            int coalesceCount = countOccurrences(sql, "COALESCE(");
            assertEquals(2, coalesceCount, "Should have 2 COALESCE expressions for 2 embeddings");
        }
    }

    // ==================== JSON Parsing Tests ====================

    @Nested
    @DisplayName("JSON Result Parsing")
    class JsonParsing {

        @Test
        @DisplayName("Should convert rows with JSON embedded resources to nested structure")
        void testNestedStructureConversion() throws Exception {
            // Simulate PostgreSQL result with row_to_json output
            Map<String, Object> row = new HashMap<>();
            row.put("id", "abc-123");
            row.put("student_id", "student-456");
            row.put("course_id", "course-789");
            row.put("course", "{\"id\":\"course-789\",\"name\":\"Mathematics\",\"code\":\"MATH101\"}");

            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetTable("courses")
                .embeddingName("course")
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            List<Map<String, Object>> result = convertToNestedStructure(
                Collections.singletonList(row), plan);

            assertEquals(1, result.size());
            Map<String, Object> nestedRow = result.get(0);

            // Verify regular columns preserved
            assertEquals("abc-123", nestedRow.get("id"));
            assertEquals("student-456", nestedRow.get("student_id"));

            // Verify embedded resource is parsed as Map
            Object course = nestedRow.get("course");
            assertInstanceOf(Map.class, course, "Embedded resource should be parsed as Map");

            @SuppressWarnings("unchecked")
            Map<String, Object> courseMap = (Map<String, Object>) course;
            assertEquals("course-789", courseMap.get("id"));
            assertEquals("Mathematics", courseMap.get("name"));
            assertEquals("MATH101", courseMap.get("code"));
        }

        @Test
        @DisplayName("Should handle null embedded resource (LEFT JOIN no match)")
        void testNullEmbeddedResource() throws Exception {
            Map<String, Object> row = new HashMap<>();
            row.put("id", "abc-123");
            row.put("course", null);

            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetTable("courses")
                .embeddingName("course")
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            List<Map<String, Object>> result = convertToNestedStructure(
                Collections.singletonList(row), plan);

            assertEquals(1, result.size());
            assertNull(result.get(0).get("course"), "NULL embedded resource should remain null");
        }

        @Test
        @DisplayName("Should parse 'null' string as null")
        void testNullStringParsing() throws Exception {
            Object result = parseEmbeddedValue("null", "course");
            assertNull(result, "'null' string should be parsed as null");
        }

        @Test
        @DisplayName("Should parse empty string as null")
        void testEmptyStringParsing() throws Exception {
            Object result = parseEmbeddedValue("", "course");
            assertNull(result, "Empty string should be parsed as null");
        }

        @Test
        @DisplayName("Should preserve Map values without re-parsing")
        void testMapValuePreserved() throws Exception {
            Map<String, Object> mapValue = new HashMap<>();
            mapValue.put("id", "123");
            mapValue.put("name", "Test");

            Object result = parseEmbeddedValue(mapValue, "course");

            assertSame(mapValue, result, "Map values should be returned as-is");
        }

        @Test
        @DisplayName("Should handle multiple embedded resources in same row")
        void testMultipleEmbeddedResourcesParsing() throws Exception {
            Map<String, Object> row = new HashMap<>();
            row.put("id", "enrollment-1");
            row.put("course", "{\"id\":\"c1\",\"name\":\"Math\"}");
            row.put("student", "{\"id\":\"s1\",\"name\":\"John Doe\"}");

            JoinClause courseJoin = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetTable("courses")
                .embeddingName("course")
                .build();

            JoinClause studentJoin = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetTable("students")
                .embeddingName("student")
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .table("enrollments")
                .joins(Arrays.asList(courseJoin, studentJoin))
                .build();

            List<Map<String, Object>> result = convertToNestedStructure(
                Collections.singletonList(row), plan);

            Map<String, Object> nestedRow = result.get(0);

            @SuppressWarnings("unchecked")
            Map<String, Object> course = (Map<String, Object>) nestedRow.get("course");
            @SuppressWarnings("unchecked")
            Map<String, Object> student = (Map<String, Object>) nestedRow.get("student");

            assertEquals("c1", course.get("id"));
            assertEquals("Math", course.get("name"));
            assertEquals("s1", student.get("id"));
            assertEquals("John Doe", student.get("name"));
        }
    }

    // ==================== SQL Structure Verification ====================

    @Nested
    @DisplayName("SQL Structure Verification")
    class SqlStructure {

        @Test
        @DisplayName("Should NOT generate traditional JOIN clause")
        void testNoTraditionalJoin() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("student_courses.course_id")
                        .rightColumn("course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // Should NOT have JOIN in the main FROM clause
            assertFalse(sql.matches(".*FROM.*JOIN.*ON.*"),
                "Should not have traditional JOIN syntax");

            // Main FROM should only reference the base table
            assertTrue(sql.contains("FROM \"public\".\"student_courses\""),
                "Main FROM should reference base table");
        }

        @Test
        @DisplayName("Should properly quote identifiers")
        void testIdentifierQuoting() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("student_courses.course_id")
                        .rightColumn("course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // Verify all identifiers are quoted
            assertTrue(sql.contains("\"public\""), "Schema should be quoted");
            assertTrue(sql.contains("\"student_courses\""), "Table should be quoted");
            assertTrue(sql.contains("\"courses\""), "Embedded table should be quoted");
            assertTrue(sql.contains("\"course\""), "Embedding name should be quoted");
        }

        @Test
        @DisplayName("Should include base table columns correctly")
        void testBaseTableColumns() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .conditions(Collections.singletonList(
                    JoinClause.JoinCondition.builder()
                        .leftColumn("student_courses.course_id")
                        .rightColumn("course.id")
                        .operator("=")
                        .build()
                ))
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .selectColumns(Arrays.asList(
                    "student_courses.id",
                    "student_courses.student_id",
                    "student_courses.course_id"
                ))
                .joins(Collections.singletonList(join))
                .build();

            String sql = buildSQL(plan);

            // Base table columns should be at the start
            assertTrue(sql.startsWith("SELECT \"student_courses\".\"id\""),
                "Should start with base table columns");
        }
    }

    // ==================== Edge Cases ====================

    @Nested
    @DisplayName("Edge Cases")
    class EdgeCases {

        @Test
        @DisplayName("Should handle query without embedded resources")
        void testNoEmbeddedResources() throws Exception {
            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("users")
                .selectColumns(Arrays.asList("id", "name"))
                .build();

            String sql = buildSQL(plan);

            // Should be a simple SELECT without subqueries
            assertFalse(sql.contains("COALESCE("), "Should not have COALESCE");
            assertFalse(sql.contains("row_to_json("), "Should not have row_to_json");
            assertEquals("SELECT \"id\", \"name\" FROM \"public\".\"users\"", sql);
        }

        @Test
        @DisplayName("Should handle empty join conditions list")
        void testEmptyJoinConditions() throws Exception {
            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetSchema("public")
                .targetTable("courses")
                .embeddingName("course")
                .conditions(Collections.emptyList())
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .schema("public")
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            // Should not throw exception
            String sql = buildSQL(plan);
            assertNotNull(sql);
        }

        @Test
        @DisplayName("convertToNestedStructure should return original rows when no joins")
        void testConvertWithoutJoins() throws Exception {
            Map<String, Object> row = new HashMap<>();
            row.put("id", "123");
            row.put("name", "Test");

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .table("users")
                .build();

            List<Map<String, Object>> result = convertToNestedStructure(
                Collections.singletonList(row), plan);

            // Should return the same list
            assertEquals(1, result.size());
            assertEquals("123", result.get(0).get("id"));
            assertEquals("Test", result.get(0).get("name"));
        }

        @Test
        @DisplayName("Should handle invalid JSON gracefully")
        void testInvalidJsonHandling() throws Exception {
            Map<String, Object> row = new HashMap<>();
            row.put("id", "123");
            row.put("course", "invalid json {{{");

            JoinClause join = JoinClause.builder()
                .type(JoinClause.JoinType.LEFT)
                .targetTable("courses")
                .embeddingName("course")
                .build();

            QueryPlan plan = QueryPlan.builder()
                .type(QueryPlan.QueryType.SELECT)
                .table("student_courses")
                .joins(Collections.singletonList(join))
                .build();

            // Should not throw, should return original value
            List<Map<String, Object>> result = convertToNestedStructure(
                Collections.singletonList(row), plan);

            assertEquals(1, result.size());
            // Invalid JSON should be preserved as original string
            assertEquals("invalid json {{{", result.get(0).get("course"));
        }
    }

    // ==================== Helper Methods ====================

    private int countOccurrences(String str, String sub) {
        int count = 0;
        int idx = 0;
        while ((idx = str.indexOf(sub, idx)) != -1) {
            count++;
            idx += sub.length();
        }
        return count;
    }
}
