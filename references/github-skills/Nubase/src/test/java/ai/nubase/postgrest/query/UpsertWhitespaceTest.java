package ai.nubase.postgrest.query;

import ai.nubase.postgrest.multidb.SchemaCacheManager;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.jdbc.core.JdbcTemplate;

import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.mock;

/**
 * Tests for UPSERT SQL generation with potential whitespace issues.
 *
 * This test simulates the exact request that was causing issues:
 * - POST /rest/v1/form_responses?on_conflict=stage_id,section_name,field_name
 * - JSON body with fields: stage_id, section_name, field_name, field_value, responded_by, updated_at
 */
@DisplayName("UPSERT Whitespace Handling Tests")
class UpsertWhitespaceTest {

    private QueryExecutor executor;
    private Method buildSQLMethod;
    private ObjectMapper objectMapper;

    @BeforeEach
    void setUp() throws Exception {
        JdbcTemplate jdbcTemplate = mock(JdbcTemplate.class);
        objectMapper = new ObjectMapper();
        SchemaCacheManager schemaCacheManager = mock(SchemaCacheManager.class);
        executor = new QueryExecutor(jdbcTemplate, objectMapper, schemaCacheManager);

        // Access private buildSQL method via reflection
        buildSQLMethod = QueryExecutor.class.getDeclaredMethod("buildSQL", QueryPlan.class, String.class);
        buildSQLMethod.setAccessible(true);
    }

    private String buildSQL(QueryPlan plan, String body) throws Exception {
        return (String) buildSQLMethod.invoke(executor, plan, body);
    }

    @Test
    @DisplayName("UPSERT with normal JSON body should not have trailing spaces in column names")
    void testUpsertNoTrailingSpaces() throws Exception {
        // Simulate the exact request that was causing issues
        String jsonBody = """
            {
                "stage_id": "3a54dd5e-abfe-4875-ae6c-921bd6060599",
                "section_name": "route_familiarisation",
                "field_name": "driver_section_signature",
                "field_value": "test_value",
                "responded_by": "25e50808-3e2f-4e10-8d9f-65d9d9dc4693",
                "updated_at": "2026-02-21T01:02:23.767Z"
            }
            """;

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPSERT)
            .schema("public")
            .table("form_responses")
            .conflictColumns(Arrays.asList("stage_id", "section_name", "field_name"))
            .returningAll(true)
            .build();

        String sql = buildSQL(plan, jsonBody);

        System.out.println("=== Generated SQL ===");
        System.out.println(sql);
        System.out.println("=====================");

        // Check for trailing spaces inside quoted identifiers
        // Pattern to find quoted identifiers with trailing space: "word_word " (not just ", ")
        // Must be an actual identifier (contains word characters), not just punctuation
        Pattern trailingSpacePattern = Pattern.compile("\"(\\w[^\"]*[\\s\\u00A0])\"");
        Matcher matcher = trailingSpacePattern.matcher(sql);

        StringBuilder issues = new StringBuilder();
        while (matcher.find()) {
            issues.append("Found trailing whitespace in identifier: \"").append(matcher.group(1)).append("\"\n");
        }

        if (issues.length() > 0) {
            System.out.println("=== ISSUES FOUND ===");
            System.out.println(issues);
            System.out.println("====================");
            fail("SQL contains quoted identifiers with trailing spaces:\n" + issues);
        }

        // Verify the SQL is correctly formed
        assertTrue(sql.contains("\"stage_id\""), "Should contain properly quoted stage_id");
        assertTrue(sql.contains("\"updated_at\""), "Should contain properly quoted updated_at");
    }

    @Test
    @DisplayName("UPSERT with non-breaking space (U+00A0) in JSON keys should be trimmed")
    void testUpsertWithNonBreakingSpace() throws Exception {
        // Simulate JSON with non-breaking spaces (U+00A0) in keys
        // This is what was likely causing the original issue
        String nbsp = "\u00A0";
        String jsonBody = """
            {
                "stage_id%s": "3a54dd5e-abfe-4875-ae6c-921bd6060599",
                "section_name%s": "route_familiarisation",
                "field_name%s": "driver_section_signature",
                "field_value%s": "test_value",
                "responded_by%s": "25e50808-3e2f-4e10-8d9f-65d9d9dc4693",
                "updated_at%s": "2026-02-21T01:02:23.767Z"
            }
            """.formatted(nbsp, nbsp, nbsp, nbsp, nbsp, nbsp);

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPSERT)
            .schema("public")
            .table("form_responses")
            .conflictColumns(Arrays.asList("stage_id", "section_name", "field_name"))
            .returningAll(true)
            .build();

        String sql = buildSQL(plan, jsonBody);

        System.out.println("=== Generated SQL (with NBSP in JSON keys) ===");
        System.out.println(sql);
        System.out.println("===============================================");

        // Check for any non-standard whitespace inside quoted identifiers
        // Look for any identifier (starts with word char) that ends with whitespace before the closing quote
        Pattern anyTrailingWhitespacePattern = Pattern.compile("\"(\\w[^\"]*[\\s\\u00A0\\u2007\\u202F\\u3000\\uFEFF])\"");
        Matcher matcher = anyTrailingWhitespacePattern.matcher(sql);

        StringBuilder issues = new StringBuilder();
        while (matcher.find()) {
            String identifier = matcher.group(1);
            issues.append("Found trailing whitespace in identifier: \"").append(identifier).append("\"\n");
            issues.append("  Last char code point: U+").append(String.format("%04X", (int) identifier.charAt(identifier.length() - 1))).append("\n");
        }

        if (issues.length() > 0) {
            System.out.println("=== ISSUES FOUND ===");
            System.out.println(issues);
            System.out.println("====================");
            fail("SQL contains quoted identifiers with trailing whitespace:\n" + issues);
        }

        // Verify the column names are correctly trimmed
        assertTrue(sql.contains("\"stage_id\""), "Should contain properly quoted stage_id");
        assertTrue(sql.contains("\"section_name\""), "Should contain properly quoted section_name");
        assertTrue(sql.contains("\"field_name\""), "Should contain properly quoted field_name");
        assertTrue(sql.contains("\"field_value\""), "Should contain properly quoted field_value");
        assertTrue(sql.contains("\"responded_by\""), "Should contain properly quoted responded_by");
        assertTrue(sql.contains("\"updated_at\""), "Should contain properly quoted updated_at");
    }

    @Test
    @DisplayName("UPSERT with leading/trailing spaces in JSON keys should be trimmed")
    void testUpsertWithLeadingTrailingSpaces() throws Exception {
        // Test with leading/trailing regular spaces (valid JSON)
        // Note: JSON keys with embedded spaces ARE valid and will be preserved by Jackson
        // The stripAllWhitespace should remove leading/trailing whitespace
        String nbsp = "\u00A0";

        // Build JSON programmatically to ensure valid format
        String jsonBody = "{\"stage_id" + nbsp + "\":\"value1\","
            + "\"" + nbsp + "section_name\":\"value2\","
            + "\"field_name" + nbsp + "\":\"value3\"}";

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPSERT)
            .schema("public")
            .table("test_table")
            .conflictColumns(Arrays.asList("stage_id"))
            .returningAll(true)
            .build();

        String sql = buildSQL(plan, jsonBody);

        System.out.println("=== Generated SQL (with NBSP in keys) ===");
        System.out.println(sql);
        System.out.println("==========================================");

        // All identifiers should be properly trimmed
        assertTrue(sql.contains("\"stage_id\""), "stage_id should be trimmed: " + sql);
        assertTrue(sql.contains("\"section_name\""), "section_name should be trimmed: " + sql);
        assertTrue(sql.contains("\"field_name\""), "field_name should be trimmed: " + sql);
    }

    @Test
    @DisplayName("Detailed SQL structure analysis for UPSERT")
    void testUpsertSqlStructureAnalysis() throws Exception {
        String jsonBody = """
            {
                "stage_id": "test-uuid",
                "section_name": "test_section",
                "field_name": "test_field",
                "field_value": "test_value",
                "responded_by": "user-uuid",
                "updated_at": "2026-02-21T01:02:23.767Z"
            }
            """;

        QueryPlan plan = QueryPlan.builder()
            .type(QueryPlan.QueryType.UPSERT)
            .schema("public")
            .table("form_responses")
            .conflictColumns(Arrays.asList("stage_id", "section_name", "field_name"))
            .returningAll(true)
            .build();

        String sql = buildSQL(plan, jsonBody);

        System.out.println("=== Detailed SQL Analysis ===");
        System.out.println("Full SQL:");
        System.out.println(sql);
        System.out.println();

        // Extract and analyze INSERT columns
        Pattern insertColsPattern = Pattern.compile("INSERT INTO [^(]+\\(([^)]+)\\)");
        Matcher insertMatcher = insertColsPattern.matcher(sql);
        if (insertMatcher.find()) {
            String insertCols = insertMatcher.group(1);
            System.out.println("INSERT columns: " + insertCols);
            analyzeColumns("INSERT", insertCols);
        }

        // Extract and analyze ON CONFLICT columns
        Pattern conflictColsPattern = Pattern.compile("ON CONFLICT \\(([^)]+)\\)");
        Matcher conflictMatcher = conflictColsPattern.matcher(sql);
        if (conflictMatcher.find()) {
            String conflictCols = conflictMatcher.group(1);
            System.out.println("ON CONFLICT columns: " + conflictCols);
            analyzeColumns("ON CONFLICT", conflictCols);
        }

        // Extract and analyze DO UPDATE SET columns
        Pattern setColsPattern = Pattern.compile("DO UPDATE SET (.+?) RETURNING");
        Matcher setMatcher = setColsPattern.matcher(sql);
        if (setMatcher.find()) {
            String setCols = setMatcher.group(1);
            System.out.println("DO UPDATE SET clause: " + setCols);

            // Extract individual column names from SET clause
            Pattern colNamePattern = Pattern.compile("\"([^\"]+)\"");
            Matcher colMatcher = colNamePattern.matcher(setCols);
            while (colMatcher.find()) {
                String colName = colMatcher.group(1);
                analyzeIdentifier("SET", colName);
            }
        }

        System.out.println("=============================");

        // Verify no trailing spaces in actual identifiers (not separators like ", ")
        Pattern trailingSpacePattern = Pattern.compile("\"(\\w[^\"]*[\\s\\u00A0])\"");
        Matcher finalMatcher = trailingSpacePattern.matcher(sql);
        if (finalMatcher.find()) {
            fail("SQL contains quoted identifiers with trailing spaces: " + finalMatcher.group(1));
        }
    }

    private void analyzeColumns(String context, String columns) {
        String[] cols = columns.split(",");
        for (String col : cols) {
            col = col.trim();
            if (col.startsWith("\"") && col.endsWith("\"")) {
                String identifier = col.substring(1, col.length() - 1);
                analyzeIdentifier(context, identifier);
            }
        }
    }

    private void analyzeIdentifier(String context, String identifier) {
        StringBuilder sb = new StringBuilder();
        sb.append("  [").append(context).append("] \"").append(identifier).append("\" ");
        sb.append("len=").append(identifier.length()).append(" ");
        sb.append("codepoints=[");
        for (int i = 0; i < identifier.length(); i++) {
            if (i > 0) sb.append(" ");
            sb.append(String.format("U+%04X", (int) identifier.charAt(i)));
        }
        sb.append("]");

        // Check for trailing whitespace
        if (identifier.length() > 0) {
            char lastChar = identifier.charAt(identifier.length() - 1);
            if (Character.isWhitespace(lastChar) || lastChar == '\u00A0' || lastChar == '\u2007' || lastChar == '\u202F') {
                sb.append(" *** HAS TRAILING WHITESPACE ***");
            }
        }

        System.out.println(sb);
    }
}
