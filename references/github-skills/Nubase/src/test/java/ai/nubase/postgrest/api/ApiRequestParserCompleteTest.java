package ai.nubase.postgrest.api;

import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.servlet.http.HttpServletRequest;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

import java.io.BufferedReader;
import java.io.StringReader;
import java.util.Collections;
import java.util.HashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.*;

/**
 * Complete functional tests for ApiRequestParser
 * Covers all scenarios for PostgREST API request parsing
 */
@DisplayName("API request parser complete tests")
class ApiRequestParserCompleteTest {

    private ApiRequestParser parser;
    private HttpServletRequest request;
    private Map<String, String[]> params;

    @BeforeEach
    void setUp() {
        parser = new ApiRequestParser(new ObjectMapper());
        request = mock(HttpServletRequest.class);
        params = new HashMap<>();

        // Default mocks
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
    }

    // ==================== Filter parsing tests ====================

    @Test
    @DisplayName("Parse basic filter - eq operator")
    void testParseBasicFilter() throws Exception {
        mockParameter("id", "eq.1");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest.getFilters());
        assertEquals(1, apiRequest.getFilters().size());
        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("id", filter.getColumn());
        assertEquals(Filter.FilterOperator.EQ, filter.getOperator());
        assertEquals("1", filter.getValue());
    }

    @Test
    @DisplayName("Parse multiple filters - AND-connected")
    void testParseMultipleFilters() throws Exception {
        mockParameter("age", "gte.18");
        mockParameter("status", "eq.active");
        mockParameter("verified", "is.true");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(3, apiRequest.getFilters().size());
    }

    @ParameterizedTest
    @CsvSource({
        "eq.value, EQ, value",
        "neq.value, NEQ, value",
        "gt.100, GT, 100",
        "gte.18, GTE, 18",
        "lt.65, LT, 65",
        "lte.100, LTE, 100"
    })
    @DisplayName("Parse comparison operators")
    void testParseComparisonOperators(String filterValue, String expectedOp, String expectedValue) throws Exception {
        mockParameter("column", filterValue);
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "table");
        Filter filter = apiRequest.getFilters().get(0);

        assertEquals(Filter.FilterOperator.valueOf(expectedOp), filter.getOperator());
        assertEquals(expectedValue, filter.getValue());
    }

    @Test
    @DisplayName("Parse LIKE operator - wildcards")
    void testParseLikeOperator() throws Exception {
        mockParameter("name", "like.*John*");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");
        Filter filter = apiRequest.getFilters().get(0);

        assertEquals(Filter.FilterOperator.LIKE, filter.getOperator());
        assertEquals("*John*", filter.getValue());
    }

    @Test
    @DisplayName("Parse IN operator - list values")
    void testParseInOperator() throws Exception {
        mockParameter("status", "in.(active,pending,approved)");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "orders");
        Filter filter = apiRequest.getFilters().get(0);

        assertEquals(Filter.FilterOperator.IN, filter.getOperator());
        assertEquals("(active,pending,approved)", filter.getValue());
    }

    @Test
    @DisplayName("Parse IS operator - NULL value")
    void testParseIsNullOperator() throws Exception {
        mockParameter("deleted_at", "is.null");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");
        Filter filter = apiRequest.getFilters().get(0);

        assertEquals(Filter.FilterOperator.IS, filter.getOperator());
        assertEquals("null", filter.getValue());
    }

    @Test
    @DisplayName("Parse full-text search operator - fts")
    void testParseFullTextSearch() throws Exception {
        mockParameter("content", "fts.cat & dog");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "articles");
        Filter filter = apiRequest.getFilters().get(0);

        assertEquals(Filter.FilterOperator.FTS, filter.getOperator());
    }

    @Test
    @DisplayName("Parse array operator - cs (contains)")
    void testParseArrayContainsOperator() throws Exception {
        mockParameter("tags", "cs.{tech,news}");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "posts");
        Filter filter = apiRequest.getFilters().get(0);

        assertEquals(Filter.FilterOperator.CS, filter.getOperator());
        assertEquals("{tech,news}", filter.getValue());
    }

    @Test
    @DisplayName("Parse NOT prefix - negated filter")
    void testParseNotPrefix() throws Exception {
        mockParameter("status", "not.eq.inactive");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");
        Filter filter = apiRequest.getFilters().get(0);

        assertTrue(filter.isNegate());
        assertEquals(Filter.FilterOperator.EQ, filter.getOperator());
        assertEquals("inactive", filter.getValue());
    }

    // ==================== Column selection parsing tests ====================

    @Test
    @DisplayName("Parse select - basic columns")
    void testParseSelectBasicColumns() throws Exception {
        mockParameter("select", "id,name,email");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest.getSelect());
        assertEquals(3, apiRequest.getSelect().size());
        assertEquals("id", apiRequest.getSelect().get(0).getName());
        assertEquals("name", apiRequest.getSelect().get(1).getName());
        assertEquals("email", apiRequest.getSelect().get(2).getName());
    }

    @Test
    @DisplayName("Parse select - with alias")
    void testParseSelectWithAlias() throws Exception {
        mockParameter("select", "id,full_name:name,user_email:email");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals("name", apiRequest.getSelect().get(1).getName());
        assertEquals("full_name", apiRequest.getSelect().get(1).getAlias());
    }

    @Test
    @DisplayName("Parse select - embedded resource")
    void testParseSelectWithEmbedding() throws Exception {
        mockParameter("select", "id,name,posts(*)");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(3, apiRequest.getSelect().size());
        SelectColumn postsColumn = apiRequest.getSelect().get(2);
        assertEquals("posts", postsColumn.getName());
        assertNotNull(postsColumn.getEmbedded());
        assertEquals(1, postsColumn.getEmbedded().size());
        assertEquals("*", postsColumn.getEmbedded().get(0).getName());
    }

    @Test
    @DisplayName("Parse select - nested embedding")
    void testParseSelectWithNestedEmbedding() throws Exception {
        mockParameter("select", "id,posts(id,title,comments(*))");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        SelectColumn postsColumn = apiRequest.getSelect().get(1);
        assertEquals("posts", postsColumn.getName());
        assertEquals(3, postsColumn.getEmbedded().size());

        SelectColumn commentsColumn = postsColumn.getEmbedded().get(2);
        assertEquals("comments", commentsColumn.getName());
        assertNotNull(commentsColumn.getEmbedded());
    }

    // ==================== Order parsing tests ====================

    @Test
    @DisplayName("Parse order - single column ascending")
    void testParseOrderSingleAsc() throws Exception {
        mockParameter("order", "created_at.asc");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "posts");

        assertNotNull(apiRequest.getOrderBy());
        assertEquals(1, apiRequest.getOrderBy().size());
        assertEquals("created_at", apiRequest.getOrderBy().get(0).getColumn());
        assertEquals(OrderBy.Direction.ASC, apiRequest.getOrderBy().get(0).getDirection());
    }

    @Test
    @DisplayName("Parse order - multiple columns")
    void testParseOrderMultipleColumns() throws Exception {
        mockParameter("order", "priority.desc,created_at.asc");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "tasks");

        assertEquals(2, apiRequest.getOrderBy().size());
        assertEquals("priority", apiRequest.getOrderBy().get(0).getColumn());
        assertEquals(OrderBy.Direction.DESC, apiRequest.getOrderBy().get(0).getDirection());
    }

    @Test
    @DisplayName("Parse order - NULLS FIRST")
    void testParseOrderWithNullsFirst() throws Exception {
        mockParameter("order", "priority.desc.nullsfirst");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "tasks");

        OrderBy orderBy = apiRequest.getOrderBy().get(0);
        assertEquals(OrderBy.NullsOrder.FIRST, orderBy.getNullsOrder());
    }

    @Test
    @DisplayName("Parse order - NULLS LAST")
    void testParseOrderWithNullsLast() throws Exception {
        mockParameter("order", "id.asc.nullslast");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        OrderBy orderBy = apiRequest.getOrderBy().get(0);
        assertEquals(OrderBy.NullsOrder.LAST, orderBy.getNullsOrder());
    }

    // ==================== Pagination parsing tests ====================

    @Test
    @DisplayName("Parse Range header - standard format")
    void testParseRangeHeader() throws Exception {
        when(request.getHeader("Range")).thenReturn("items=0-9");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest.getRange());
        assertEquals("items", apiRequest.getRange().getUnit());
        assertEquals(0L, apiRequest.getRange().getStart());
        assertEquals(9L, apiRequest.getRange().getEnd());
    }

    @Test
    @DisplayName("Parse Range header - start only")
    void testParseRangeHeaderStartOnly() throws Exception {
        when(request.getHeader("Range")).thenReturn("items=10-");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(10L, apiRequest.getRange().getStart());
        assertNull(apiRequest.getRange().getEnd());
    }

    @Test
    @DisplayName("Parse limit and offset parameters")
    void testParseLimitOffset() throws Exception {
        mockParameter("limit", "10");
        mockParameter("offset", "20");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest.getRange());
        assertEquals(20L, apiRequest.getRange().getStart());
        assertEquals(29L, apiRequest.getRange().getEnd());
    }

    @Test
    @DisplayName("Parse limit without offset")
    void testParseLimitWithoutOffset() throws Exception {
        mockParameter("limit", "5");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(0L, apiRequest.getRange().getStart());
        assertEquals(4L, apiRequest.getRange().getEnd());
    }

    // ==================== Prefer header parsing tests ====================

    @Test
    @DisplayName("Parse Prefer header - return=representation")
    void testParsePreferReturnRepresentation() throws Exception {
        when(request.getHeader("Prefer")).thenReturn("return=representation");
        when(request.getMethod()).thenReturn("POST");
        mockRequestBody("{}");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest.getPreferences());
        assertEquals(Preferences.ReturnPreference.REPRESENTATION,
                    apiRequest.getPreferences().getReturnPreference());
    }

    @Test
    @DisplayName("Parse Prefer header - count=exact")
    void testParsePreferCountExact() throws Exception {
        when(request.getHeader("Prefer")).thenReturn("count=exact");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(Preferences.CountPreference.EXACT,
                    apiRequest.getPreferences().getCountPreference());
    }

    @Test
    @DisplayName("Parse Prefer header - multiple options")
    void testParsePreferMultipleOptions() throws Exception {
        when(request.getHeader("Prefer")).thenReturn("return=minimal,count=exact");
        when(request.getMethod()).thenReturn("POST");
        mockRequestBody("{}");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        Preferences prefs = apiRequest.getPreferences();
        assertEquals(Preferences.ReturnPreference.MINIMAL, prefs.getReturnPreference());
        assertEquals(Preferences.CountPreference.EXACT, prefs.getCountPreference());
    }

    @Test
    @DisplayName("Parse Prefer header - resolution (UPSERT)")
    void testParsePreferResolution() throws Exception {
        when(request.getHeader("Prefer")).thenReturn("resolution=merge-duplicates");
        when(request.getMethod()).thenReturn("POST");
        mockRequestBody("{}");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(Preferences.Resolution.MERGE_DUPLICATES,
                    apiRequest.getPreferences().getResolution());
    }

    @Test
    @DisplayName("Parse Prefer header - timezone")
    void testParsePreferTimezone() throws Exception {
        when(request.getHeader("Prefer")).thenReturn("timezone=UTC");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "events");

        assertEquals("UTC", apiRequest.getPreferences().getTimezone());
    }

    // ==================== Request body parsing tests ====================

    @Test
    @DisplayName("Parse POST request body - JSON")
    void testParsePostBody() throws Exception {
        when(request.getMethod()).thenReturn("POST");
        String jsonBody = "{\"name\":\"John\",\"email\":\"john@example.com\"}";
        mockRequestBody(jsonBody);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(jsonBody, apiRequest.getBody());
    }

    @Test
    @DisplayName("Parse PATCH request body - JSON")
    void testParsePatchBody() throws Exception {
        when(request.getMethod()).thenReturn("PATCH");
        String jsonBody = "{\"status\":\"active\"}";
        mockRequestBody(jsonBody);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(jsonBody, apiRequest.getBody());
    }

    @Test
    @DisplayName("Parse PUT request body - JSON")
    void testParsePutBody() throws Exception {
        when(request.getMethod()).thenReturn("PUT");
        String jsonBody = "{\"id\":1,\"name\":\"Updated\"}";
        mockRequestBody(jsonBody);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(jsonBody, apiRequest.getBody());
    }

    // ==================== Edge case tests ====================

    @Test
    @DisplayName("Parse empty request - no parameters")
    void testParseEmptyRequest() throws Exception {
        when(request.getMethod()).thenReturn("GET");
        when(request.getParameterMap()).thenReturn(new HashMap<>());

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest);
        assertEquals("public", apiRequest.getSchema());
        assertEquals("users", apiRequest.getTable());
        assertEquals("GET", apiRequest.getMethod());
    }

    @Test
    @DisplayName("Parse reserved parameters - should be ignored")
    void testParseReservedParameters() throws Exception {
        mockParameter("select", "id,name");
        mockParameter("order", "id.asc");
        mockParameter("limit", "10");
        mockParameter("offset", "0");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // These parameters should not be treated as filters
        assertTrue(apiRequest.getFilters() == null || apiRequest.getFilters().isEmpty());
    }

    @Test
    @DisplayName("Parse special characters - URL encoded")
    void testParseUrlEncodedValues() throws Exception {
        mockParameter("name", "eq.John%20Doe");
        when(request.getMethod()).thenReturn("GET");

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // URL decoding should happen at the framework layer
        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("John%20Doe", filter.getValue());
    }

    // ==================== Helper Methods ====================

    private void mockParameter(String name, String value) {
        when(request.getParameter(name)).thenReturn(value);
        params.put(name, new String[]{value});
    }

    private void mockRequestBody(String body) throws Exception {
        BufferedReader reader = new BufferedReader(new StringReader(body));
        when(request.getReader()).thenReturn(reader);
    }
}
