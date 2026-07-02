package ai.nubase.postgrest.api;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import jakarta.servlet.http.HttpServletRequest;

import java.util.Collections;
import java.util.HashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.when;

/**
 * Comprehensive filter operator tests
 * Based on PostgREST's Feature/Query/QuerySpec.hs
 */
@ExtendWith(MockitoExtension.class)
class FilterTest {

    @Mock
    private HttpServletRequest request;

    private ApiRequestParser parser;

    @BeforeEach
    void setUp() {
        parser = new ApiRequestParser(new ObjectMapper());
        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
    }

    private void setupMockRequest(Map<String, String[]> params) {
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter(anyString())).thenAnswer(invocation -> {
            String param = invocation.getArgument(0);
            String[] values = params.get(param);
            return values != null ? values[0] : null;
        });
    }

    // ==================== Equality Operators ====================

    @Test
    void testFilter_Equals() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("id", new String[]{"eq.1"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(1, apiRequest.getFilters().size());
        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("id", filter.getColumn());
        assertEquals(Filter.FilterOperator.EQ, filter.getOperator());
        assertEquals("1", filter.getValue());
    }

    @Test
    void testFilter_NotEquals() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("status", new String[]{"neq.inactive"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.NEQ, filter.getOperator());
        assertEquals("inactive", filter.getValue());
    }

    // ==================== Comparison Operators ====================

    @Test
    void testFilter_GreaterThan() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("age", new String[]{"gt.18"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.GT, filter.getOperator());
        assertEquals("18", filter.getValue());
    }

    @Test
    void testFilter_GreaterThanOrEqual() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("price", new String[]{"gte.100"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "products");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.GTE, filter.getOperator());
        assertEquals("100", filter.getValue());
    }

    @Test
    void testFilter_LessThan() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("stock", new String[]{"lt.10"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "products");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.LT, filter.getOperator());
        assertEquals("10", filter.getValue());
    }

    @Test
    void testFilter_LessThanOrEqual() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("rating", new String[]{"lte.5"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "reviews");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.LTE, filter.getOperator());
        assertEquals("5", filter.getValue());
    }

    // ==================== Pattern Matching ====================

    @Test
    void testFilter_Like() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("name", new String[]{"like.*John*"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.LIKE, filter.getOperator());
        assertEquals("*John*", filter.getValue());
    }

    @Test
    void testFilter_ILike() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("email", new String[]{"ilike.*@gmail.com"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.ILIKE, filter.getOperator());
        assertEquals("*@gmail.com", filter.getValue());
    }

    // ==================== List Operators ====================

    @Test
    void testFilter_In() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("status", new String[]{"in.(active,pending,approved)"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "orders");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.IN, filter.getOperator());
        assertEquals("(active,pending,approved)", filter.getValue());
    }

    @Test
    void testFilter_Is_Null() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("deleted_at", new String[]{"is.null"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.IS, filter.getOperator());
        assertEquals("null", filter.getValue());
    }

    @Test
    void testFilter_Is_True() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("is_verified", new String[]{"is.true"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.IS, filter.getOperator());
        assertEquals("true", filter.getValue());
    }

    @Test
    void testFilter_Not_Is_Null() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("checklist_item_id", new String[]{"not.is.null"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "form_responses");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("checklist_item_id", filter.getColumn());
        assertEquals(Filter.FilterOperator.IS, filter.getOperator());
        assertEquals("null", filter.getValue());
        assertTrue(filter.isNegate(), "not.is.null should have negate=true");
    }

    @Test
    void testFilter_Not_Eq() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("status", new String[]{"not.eq.deleted"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "items");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("status", filter.getColumn());
        assertEquals(Filter.FilterOperator.EQ, filter.getOperator());
        assertEquals("deleted", filter.getValue());
        assertTrue(filter.isNegate(), "not.eq should have negate=true");
    }

    // ==================== Full-Text Search ====================

    @Test
    void testFilter_FTS() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("content", new String[]{"fts.fat & cat"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "posts");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.FTS, filter.getOperator());
        assertEquals("fat & cat", filter.getValue());
    }

    @Test
    void testFilter_PLFTS() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("description", new String[]{"plfts.The Fat Cats"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "products");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals(Filter.FilterOperator.PLFTS, filter.getOperator());
        assertEquals("The Fat Cats", filter.getValue());
    }

    // ==================== Multiple Filters ====================

    @Test
    void testFilter_MultipleConditions() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("age", new String[]{"gte.18"});
        params.put("status", new String[]{"eq.active"});
        params.put("city", new String[]{"in.(Beijing,Shanghai)"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(3, apiRequest.getFilters().size());

        // Verify filters exist (order may vary)
        assertTrue(apiRequest.getFilters().stream()
            .anyMatch(f -> f.getColumn().equals("age") &&
                          f.getOperator() == Filter.FilterOperator.GTE));
        assertTrue(apiRequest.getFilters().stream()
            .anyMatch(f -> f.getColumn().equals("status") &&
                          f.getOperator() == Filter.FilterOperator.EQ));
        assertTrue(apiRequest.getFilters().stream()
            .anyMatch(f -> f.getColumn().equals("city") &&
                          f.getOperator() == Filter.FilterOperator.IN));
    }

    // ==================== Edge Cases ====================

    @Test
    void testFilter_EmptyValue() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("notes", new String[]{"eq."});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "tasks");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("", filter.getValue());
    }

    @Test
    void testFilter_SpecialCharacters() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("name", new String[]{"like.*O'Reilly*"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "authors");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("*O'Reilly*", filter.getValue());
    }

    @Test
    void testFilter_DotsInValue() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("version", new String[]{"eq.1.2.3"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "releases");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("1.2.3", filter.getValue());
    }

    @Test
    void testFilter_NoOperator_DefaultsToEquals() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("username", new String[]{"john"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("username", filter.getColumn());
        assertEquals(Filter.FilterOperator.EQ, filter.getOperator());
        assertEquals("john", filter.getValue());
    }

    // ==================== OR/AND Logical Operators ====================

    @Test
    void testFilter_Or_Simple() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("or", new String[]{"(age.lt.18,age.gt.65)"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Should have no simple filters
        assertTrue(apiRequest.getFilters().isEmpty());

        // Should have one logical condition
        assertEquals(1, apiRequest.getLogicalConditions().size());

        LogicalCondition orCondition = apiRequest.getLogicalConditions().get(0);
        assertEquals(LogicalCondition.ConditionType.OR, orCondition.getType());
        assertFalse(orCondition.isNegate());
        assertEquals(2, orCondition.getConditions().size());

        // First sub-condition: age.lt.18
        LogicalCondition sub1 = orCondition.getConditions().get(0);
        assertEquals(LogicalCondition.ConditionType.FILTER, sub1.getType());
        assertEquals("age", sub1.getFilter().getColumn());
        assertEquals(Filter.FilterOperator.LT, sub1.getFilter().getOperator());
        assertEquals("18", sub1.getFilter().getValue());

        // Second sub-condition: age.gt.65
        LogicalCondition sub2 = orCondition.getConditions().get(1);
        assertEquals(LogicalCondition.ConditionType.FILTER, sub2.getType());
        assertEquals("age", sub2.getFilter().getColumn());
        assertEquals(Filter.FilterOperator.GT, sub2.getFilter().getOperator());
        assertEquals("65", sub2.getFilter().getValue());
    }

    @Test
    void testFilter_And_Simple() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("and", new String[]{"(status.eq.active,verified.is.true)"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(1, apiRequest.getLogicalConditions().size());

        LogicalCondition andCondition = apiRequest.getLogicalConditions().get(0);
        assertEquals(LogicalCondition.ConditionType.AND, andCondition.getType());
        assertEquals(2, andCondition.getConditions().size());
    }

    @Test
    void testFilter_Not_Or() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("not.or", new String[]{"(a.eq.1,b.eq.2)"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "items");

        assertEquals(1, apiRequest.getLogicalConditions().size());

        LogicalCondition condition = apiRequest.getLogicalConditions().get(0);
        assertEquals(LogicalCondition.ConditionType.OR, condition.getType());
        assertTrue(condition.isNegate(), "not.or should have negate=true");
        assertEquals(2, condition.getConditions().size());
    }

    @Test
    void testFilter_Nested_Or_And() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        // or=(status.eq.active,and(priority.eq.high,assigned.is.true))
        params.put("or", new String[]{"(status.eq.active,and(priority.eq.high,assigned.is.true))"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "tasks");

        assertEquals(1, apiRequest.getLogicalConditions().size());

        LogicalCondition orCondition = apiRequest.getLogicalConditions().get(0);
        assertEquals(LogicalCondition.ConditionType.OR, orCondition.getType());
        assertEquals(2, orCondition.getConditions().size());

        // First sub-condition: status.eq.active (FILTER)
        LogicalCondition sub1 = orCondition.getConditions().get(0);
        assertEquals(LogicalCondition.ConditionType.FILTER, sub1.getType());

        // Second sub-condition: and(priority.eq.high,assigned.is.true) (AND group)
        LogicalCondition sub2 = orCondition.getConditions().get(1);
        assertEquals(LogicalCondition.ConditionType.AND, sub2.getType());
        assertEquals(2, sub2.getConditions().size());
    }

    @Test
    void testFilter_Or_With_Simple_Filters() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("status", new String[]{"eq.active"});
        params.put("or", new String[]{"(age.lt.18,age.gt.65)"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Should have one simple filter
        assertEquals(1, apiRequest.getFilters().size());
        assertEquals("status", apiRequest.getFilters().get(0).getColumn());

        // Should have one logical condition
        assertEquals(1, apiRequest.getLogicalConditions().size());
        assertEquals(LogicalCondition.ConditionType.OR, apiRequest.getLogicalConditions().get(0).getType());
    }
}
