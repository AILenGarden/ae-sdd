package ai.nubase.postgrest.api;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.springframework.mock.web.MockHttpServletRequest;

import java.lang.reflect.Method;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests for parsing any() and all() quantifiers in ApiRequestParser.
 *
 * PostgREST syntax examples:
 *   ?id=eq(any).{1,2,3}
 *   ?name=like(all).{%a%,%b%}
 *   ?status=not.eq(any).{active,pending}
 */
@DisplayName("Quantifier Parsing Tests")
class QuantifierParsingTest {

    private ApiRequestParser parser;
    private Method parseAllFiltersMethod;

    @BeforeEach
    void setUp() throws Exception {
        ObjectMapper objectMapper = new ObjectMapper();
        parser = new ApiRequestParser(objectMapper);

        // Access private parseAllFilters method via reflection
        parseAllFiltersMethod = ApiRequestParser.class.getDeclaredMethod("parseAllFilters",
            jakarta.servlet.http.HttpServletRequest.class);
        parseAllFiltersMethod.setAccessible(true);
    }

    @SuppressWarnings("unchecked")
    private List<Filter> parseFilters(MockHttpServletRequest request) throws Exception {
        Object result = parseAllFiltersMethod.invoke(parser, request);
        // Result is FilterParseResult record, need to get filters field
        Method filtersMethod = result.getClass().getMethod("filters");
        return (List<Filter>) filtersMethod.invoke(result);
    }

    // ==================== ANY Quantifier Parsing ====================

    @Nested
    @DisplayName("ANY quantifier parsing")
    class AnyQuantifierParsingTests {

        @Test
        @DisplayName("Parse eq(any) filter")
        void testParseEqAny() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("id", "eq(any).{1,2,3}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals("id", filter.getColumn());
            assertEquals(Filter.FilterOperator.EQ, filter.getOperator());
            assertEquals("{1,2,3}", filter.getValue());
            assertEquals(Filter.Quantifier.ANY, filter.getQuantifier());
            assertFalse(filter.isNegate());
        }

        @Test
        @DisplayName("Parse like(any) filter")
        void testParseLikeAny() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("name", "like(any).{%john%,%jane%}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals("name", filter.getColumn());
            assertEquals(Filter.FilterOperator.LIKE, filter.getOperator());
            assertEquals("{%john%,%jane%}", filter.getValue());
            assertEquals(Filter.Quantifier.ANY, filter.getQuantifier());
        }

        @Test
        @DisplayName("Parse ilike(any) filter")
        void testParseIlikeAny() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("email", "ilike(any).{%GMAIL%,%YAHOO%}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.ILIKE, filter.getOperator());
            assertEquals(Filter.Quantifier.ANY, filter.getQuantifier());
        }

        @Test
        @DisplayName("Parse match(any) filter - regex")
        void testParseMatchAny() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("content", "match(any).{^test,end$}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.MATCH, filter.getOperator());
            assertEquals(Filter.Quantifier.ANY, filter.getQuantifier());
        }

        @Test
        @DisplayName("Parse imatch(any) filter - case-insensitive regex")
        void testParseImatchAny() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("title", "imatch(any).{ERROR,WARNING}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.IMATCH, filter.getOperator());
            assertEquals(Filter.Quantifier.ANY, filter.getQuantifier());
        }
    }

    // ==================== ALL Quantifier Parsing ====================

    @Nested
    @DisplayName("ALL quantifier parsing")
    class AllQuantifierParsingTests {

        @Test
        @DisplayName("Parse gt(all) filter")
        void testParseGtAll() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("price", "gt(all).{10,20}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals("price", filter.getColumn());
            assertEquals(Filter.FilterOperator.GT, filter.getOperator());
            assertEquals("{10,20}", filter.getValue());
            assertEquals(Filter.Quantifier.ALL, filter.getQuantifier());
        }

        @Test
        @DisplayName("Parse gte(all) filter")
        void testParseGteAll() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("score", "gte(all).{50,60}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.GTE, filter.getOperator());
            assertEquals(Filter.Quantifier.ALL, filter.getQuantifier());
        }

        @Test
        @DisplayName("Parse lt(all) filter")
        void testParseLtAll() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("age", "lt(all).{18,21}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.LT, filter.getOperator());
            assertEquals(Filter.Quantifier.ALL, filter.getQuantifier());
        }

        @Test
        @DisplayName("Parse lte(all) filter")
        void testParseLteAll() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("count", "lte(all).{100,200}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.LTE, filter.getOperator());
            assertEquals(Filter.Quantifier.ALL, filter.getQuantifier());
        }

        @Test
        @DisplayName("Parse like(all) filter - must match all patterns")
        void testParseLikeAll() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("description", "like(all).{%urgent%,%important%}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.LIKE, filter.getOperator());
            assertEquals(Filter.Quantifier.ALL, filter.getQuantifier());
        }
    }

    // ==================== Negation Tests ====================

    @Nested
    @DisplayName("Negation with quantifiers")
    class NegationParsingTests {

        @Test
        @DisplayName("Parse not.eq(any) filter")
        void testParseNotEqAny() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("status", "not.eq(any).{deleted,archived}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.EQ, filter.getOperator());
            assertEquals(Filter.Quantifier.ANY, filter.getQuantifier());
            assertTrue(filter.isNegate(), "Should be negated");
        }

        @Test
        @DisplayName("Parse not.like(all) filter")
        void testParseNotLikeAll() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("name", "not.like(all).{%test%,%demo%}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.LIKE, filter.getOperator());
            assertEquals(Filter.Quantifier.ALL, filter.getQuantifier());
            assertTrue(filter.isNegate(), "Should be negated");
        }

        @Test
        @DisplayName("Parse not.gt(all) filter")
        void testParseNotGtAll() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("value", "not.gt(all).{5,10}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals(Filter.FilterOperator.GT, filter.getOperator());
            assertEquals(Filter.Quantifier.ALL, filter.getQuantifier());
            assertTrue(filter.isNegate());
        }
    }

    // ==================== Multiple Filters ====================

    @Nested
    @DisplayName("Multiple filters with quantifiers")
    class MultipleFiltersTests {

        @Test
        @DisplayName("Parse multiple quantified filters")
        void testMultipleQuantifiedFilters() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("id", "eq(any).{1,2,3}");
            request.setParameter("status", "neq(all).{deleted,archived}");

            List<Filter> filters = parseFilters(request);

            assertEquals(2, filters.size());

            Filter idFilter = filters.stream()
                .filter(f -> "id".equals(f.getColumn()))
                .findFirst()
                .orElseThrow();
            assertEquals(Filter.Quantifier.ANY, idFilter.getQuantifier());

            Filter statusFilter = filters.stream()
                .filter(f -> "status".equals(f.getColumn()))
                .findFirst()
                .orElseThrow();
            assertEquals(Filter.Quantifier.ALL, statusFilter.getQuantifier());
        }

        @Test
        @DisplayName("Mix quantified and regular filters")
        void testMixedFilters() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("id", "eq(any).{1,2,3}");
            request.setParameter("name", "like.%john%");  // Regular filter

            List<Filter> filters = parseFilters(request);

            assertEquals(2, filters.size());

            Filter idFilter = filters.stream()
                .filter(f -> "id".equals(f.getColumn()))
                .findFirst()
                .orElseThrow();
            assertEquals(Filter.Quantifier.ANY, idFilter.getQuantifier());

            Filter nameFilter = filters.stream()
                .filter(f -> "name".equals(f.getColumn()))
                .findFirst()
                .orElseThrow();
            assertNull(nameFilter.getQuantifier(), "Regular filter should not have quantifier");
        }
    }

    // ==================== Edge Cases ====================

    @Nested
    @DisplayName("Edge cases")
    class EdgeCaseTests {

        @Test
        @DisplayName("Parse filter with single value in array")
        void testSingleValueArray() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("id", "eq(any).{42}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals("{42}", filter.getValue());
            assertEquals(Filter.Quantifier.ANY, filter.getQuantifier());
        }

        @Test
        @DisplayName("Parse filter with special characters in values")
        void testSpecialCharactersInValues() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("email", "eq(any).{test@example.com,user+tag@domain.org}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals("{test@example.com,user+tag@domain.org}", filter.getValue());
        }

        @Test
        @DisplayName("Parse filter with asterisk wildcards")
        void testAsteriskWildcards() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("name", "like(any).{J*,*son}");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertEquals("{J*,*son}", filter.getValue());
            assertEquals(Filter.Quantifier.ANY, filter.getQuantifier());
        }

        @Test
        @DisplayName("Regular filter without quantifier should not have quantifier set")
        void testRegularFilterNoQuantifier() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("id", "eq.123");

            List<Filter> filters = parseFilters(request);

            assertEquals(1, filters.size());
            Filter filter = filters.get(0);
            assertNull(filter.getQuantifier(), "Regular filter should not have quantifier");
        }
    }

    // ==================== All Supported Operators ====================

    @Nested
    @DisplayName("All supported operators with quantifiers")
    class SupportedOperatorsTests {

        @Test
        @DisplayName("eq with any/all")
        void testEqQuantifiers() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("col1", "eq(any).{a,b}");
            request.setParameter("col2", "eq(all).{c,d}");

            List<Filter> filters = parseFilters(request);

            assertEquals(2, filters.size());
        }

        @Test
        @DisplayName("neq with any/all")
        void testNeqQuantifiers() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("col1", "neq(any).{a,b}");
            request.setParameter("col2", "neq(all).{c,d}");

            List<Filter> filters = parseFilters(request);

            assertEquals(2, filters.size());
            assertTrue(filters.stream().allMatch(f -> f.getOperator() == Filter.FilterOperator.NEQ));
        }

        @Test
        @DisplayName("gt/gte/lt/lte with any/all")
        void testComparisonQuantifiers() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("a", "gt(any).{1,2}");
            request.setParameter("b", "gte(all).{3,4}");
            request.setParameter("c", "lt(any).{5,6}");
            request.setParameter("d", "lte(all).{7,8}");

            List<Filter> filters = parseFilters(request);

            assertEquals(4, filters.size());
            assertTrue(filters.stream().allMatch(f -> f.getQuantifier() != null));
        }

        @Test
        @DisplayName("like/ilike with any/all")
        void testPatternQuantifiers() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("a", "like(any).{%x%,%y%}");
            request.setParameter("b", "ilike(all).{%X%,%Y%}");

            List<Filter> filters = parseFilters(request);

            assertEquals(2, filters.size());
        }

        @Test
        @DisplayName("match/imatch with any/all")
        void testRegexQuantifiers() throws Exception {
            MockHttpServletRequest request = new MockHttpServletRequest();
            request.setParameter("a", "match(any).{^start,end$}");
            request.setParameter("b", "imatch(all).{PATTERN1,PATTERN2}");

            List<Filter> filters = parseFilters(request);

            assertEquals(2, filters.size());
            assertEquals(Filter.FilterOperator.MATCH,
                filters.stream().filter(f -> "a".equals(f.getColumn())).findFirst().get().getOperator());
            assertEquals(Filter.FilterOperator.IMATCH,
                filters.stream().filter(f -> "b".equals(f.getColumn())).findFirst().get().getOperator());
        }
    }
}
