package ai.nubase.mem.repository;

import org.junit.jupiter.api.Test;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Tests the SQL-fragment shape and the strict key whitelist for {@link MetadataFilterParser}.
 *
 * <p>We assert on the generated SQL string because the parser is the one place where untrusted
 * filter input meets the database — verifying both the produced fragment and that injected
 * keys are rejected here is the cheapest possible defense.
 */
class MetadataFilterParserTest {

    @Test
    void empty_returnsEmptyCompiled() {
        MetadataFilterParser.Compiled c = MetadataFilterParser.compile(null);
        assertThat(c.isEmpty()).isTrue();
        c = MetadataFilterParser.compile(Map.of());
        assertThat(c.isEmpty()).isTrue();
    }

    @Test
    void scalarShorthand_treatedAsEq() {
        var c = MetadataFilterParser.compile(Map.of("category", "food"));
        assertThat(c.getSql()).isEqualTo("metadata->>'category' = ?");
        assertThat(c.getArgs()).containsExactly("food");
    }

    @Test
    void explicitEq() {
        var c = MetadataFilterParser.compile(Map.of("category", Map.of("eq", "food")));
        assertThat(c.getSql()).isEqualTo("metadata->>'category' = ?");
        assertThat(c.getArgs()).containsExactly("food");
    }

    @Test
    void ne() {
        var c = MetadataFilterParser.compile(Map.of("status", Map.of("ne", "archived")));
        assertThat(c.getSql()).isEqualTo("metadata->>'status' <> ?");
    }

    @Test
    void gteWithNumberCastsToNumeric_safelyHandlesDirtyData() {
        var c = MetadataFilterParser.compile(Map.of("year", Map.of("gte", 2025)));
        // Uses CASE-WHEN regex guard so non-numeric values become NULL (excluded) instead
        // of crashing the whole query with "invalid input syntax for type numeric".
        assertThat(c.getSql()).contains("CASE WHEN metadata->>'year' ~");
        assertThat(c.getSql()).contains("THEN (metadata->>'year')::numeric");
        assertThat(c.getSql()).endsWith(") >= ?");
        assertThat(c.getArgs()).containsExactly(2025.0);
    }

    @Test
    void gteWithStringFallsBackToLexicographicCompare() {
        var c = MetadataFilterParser.compile(Map.of("date", Map.of("gte", "2025-01-01")));
        assertThat(c.getSql()).isEqualTo("metadata->>'date' >= ?");
        assertThat(c.getArgs()).containsExactly("2025-01-01");
    }

    @Test
    void inWithList() {
        var c = MetadataFilterParser.compile(Map.of(
                "tags", Map.of("in", List.of("work", "urgent"))));
        assertThat(c.getSql()).isEqualTo("metadata->>'tags' IN (?,?)");
        assertThat(c.getArgs()).containsExactly("work", "urgent");
    }

    @Test
    void inWithScalarWrapsToSingletonList() {
        var c = MetadataFilterParser.compile(Map.of("tags", Map.of("in", "work")));
        assertThat(c.getSql()).isEqualTo("metadata->>'tags' IN (?)");
        assertThat(c.getArgs()).containsExactly("work");
    }

    @Test
    void nin() {
        var c = MetadataFilterParser.compile(Map.of(
                "tags", Map.of("nin", List.of("draft"))));
        assertThat(c.getSql()).isEqualTo("NOT metadata->>'tags' IN (?)");
    }

    @Test
    void inWithEmptyListEvaluatesToFalse() {
        var c = MetadataFilterParser.compile(Map.of("tags", Map.of("in", List.of())));
        assertThat(c.getSql()).isEqualTo("FALSE");
    }

    @Test
    void containsAndIcontains() {
        var c1 = MetadataFilterParser.compile(Map.of("name", Map.of("contains", "abc")));
        assertThat(c1.getSql()).isEqualTo("metadata->>'name' LIKE ? ESCAPE '\\'");
        assertThat(c1.getArgs()).containsExactly("%abc%");

        var c2 = MetadataFilterParser.compile(Map.of("name", Map.of("icontains", "abc")));
        assertThat(c2.getSql()).isEqualTo("metadata->>'name' ILIKE ? ESCAPE '\\'");
        assertThat(c2.getArgs()).containsExactly("%abc%");
    }

    @Test
    void containsEscapesLikeWildcards() {
        // User passes "100%" — without escape, this becomes "%100%%" which matches anything
        // containing "100" followed by anything. We want literal "100%" substring match.
        var c = MetadataFilterParser.compile(Map.of("name", Map.of("contains", "100%_done")));
        assertThat(c.getSql()).isEqualTo("metadata->>'name' LIKE ? ESCAPE '\\'");
        assertThat(c.getArgs()).containsExactly("%100\\%\\_done%");
    }

    @Test
    void containsEscapesBackslashInUserInput() {
        // The escape character itself must be doubled, otherwise PG sees "\" as starting
        // an escape sequence with the next char.
        var c = MetadataFilterParser.compile(Map.of("path", Map.of("contains", "C:\\users")));
        assertThat(c.getArgs()).containsExactly("%C:\\\\users%");
    }

    @Test
    void escapeLikeHelperHandlesEdges() {
        assertThat(MetadataFilterParser.escapeLike(null)).isEmpty();
        assertThat(MetadataFilterParser.escapeLike("")).isEmpty();
        assertThat(MetadataFilterParser.escapeLike("clean")).isEqualTo("clean");
        assertThat(MetadataFilterParser.escapeLike("a%b_c\\d")).isEqualTo("a\\%b\\_c\\\\d");
    }

    @Test
    void multipleFieldsCombineWithAnd() {
        Map<String, Object> filter = new LinkedHashMap<>();
        filter.put("category", "food");
        filter.put("year", Map.of("gte", 2025));
        var c = MetadataFilterParser.compile(filter);
        assertThat(c.getSql()).startsWith("metadata->>'category' = ? AND ");
        assertThat(c.getSql()).contains("CASE WHEN metadata->>'year' ~");
        assertThat(c.getArgs()).containsExactly("food", 2025.0);
    }

    @Test
    void multipleOpsOnSameField() {
        Map<String, Object> ops = new LinkedHashMap<>();
        ops.put("gte", 1);
        ops.put("lte", 10);
        var c = MetadataFilterParser.compile(Map.of("score", ops));
        // Two safe-cast CASE expressions ANDed together inside parens.
        assertThat(c.getSql()).startsWith("((CASE WHEN metadata->>'score' ~");
        assertThat(c.getSql()).contains(") >= ? AND (CASE WHEN metadata->>'score' ~");
        assertThat(c.getSql()).endsWith(") <= ?)");
        assertThat(c.getArgs()).containsExactly(1.0, 10.0);
    }

    @Test
    void andGroup() {
        var c = MetadataFilterParser.compile(Map.of(
                "AND", List.of(
                        Map.of("category", "food"),
                        Map.of("year", Map.of("gte", 2025)))));
        assertThat(c.getSql()).startsWith("(metadata->>'category' = ? AND ");
        assertThat(c.getSql()).contains("CASE WHEN metadata->>'year' ~");
        assertThat(c.getSql()).endsWith(") >= ?)");
    }

    @Test
    void orGroup() {
        var c = MetadataFilterParser.compile(Map.of(
                "OR", List.of(
                        Map.of("category", "food"),
                        Map.of("category", "travel"))));
        assertThat(c.getSql()).isEqualTo(
                "(metadata->>'category' = ? OR metadata->>'category' = ?)");
    }

    @Test
    void notGroup() {
        var c = MetadataFilterParser.compile(Map.of(
                "NOT", Map.of("archived", "true")));
        assertThat(c.getSql()).isEqualTo("NOT (metadata->>'archived' = ?)");
        assertThat(c.getArgs()).containsExactly("true");
    }

    @Test
    void rejectsInjectionAttemptInKey() {
        assertThatThrownBy(() -> MetadataFilterParser.compile(Map.of(
                "category'); DROP TABLE memories;--", "x")))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("Invalid metadata key");
    }

    @Test
    void rejectsKeyWithSpaceOrSingleQuote() {
        assertThatThrownBy(() -> MetadataFilterParser.compile(Map.of("bad key", "x")))
                .isInstanceOf(IllegalArgumentException.class);
        assertThatThrownBy(() -> MetadataFilterParser.compile(Map.of("a'b", "x")))
                .isInstanceOf(IllegalArgumentException.class);
    }

    @Test
    void rejectsUnknownOperator() {
        assertThatThrownBy(() -> MetadataFilterParser.compile(Map.of(
                "category", Map.of("startsWith", "x"))))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("Unsupported operator");
    }

    @Test
    void rejectsMalformedLogicalGroup() {
        assertThatThrownBy(() -> MetadataFilterParser.compile(Map.of(
                "AND", "should-be-an-array")))
                .isInstanceOf(IllegalArgumentException.class);
        assertThatThrownBy(() -> MetadataFilterParser.compile(Map.of(
                "NOT", List.of(Map.of("a", "b")))))
                .isInstanceOf(IllegalArgumentException.class);
    }
}
