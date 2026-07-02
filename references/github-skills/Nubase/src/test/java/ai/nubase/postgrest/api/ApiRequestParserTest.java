package ai.nubase.postgrest.api;

import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.servlet.http.HttpServletRequest;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mock;
import org.mockito.MockitoAnnotations;

import java.io.BufferedReader;
import java.io.StringReader;
import java.util.Collections;
import java.util.Enumeration;
import java.util.HashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.*;

class ApiRequestParserTest {

    private ApiRequestParser parser;

    @Mock
    private HttpServletRequest request;

    @BeforeEach
    void setUp() {
        MockitoAnnotations.openMocks(this);
        parser = new ApiRequestParser(new ObjectMapper());
    }

    @Test
    void testParseSimpleGetRequest() throws Exception {
        // Setup
        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(new HashMap<>());

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest);
        assertEquals("public", apiRequest.getSchema());
        assertEquals("users", apiRequest.getTable());
        assertEquals("GET", apiRequest.getMethod());
    }

    @Test
    void testParseFilterWithEqualOperator() throws Exception {
        // Setup - PostgREST format: ?id=eq.1
        Map<String, String[]> params = new HashMap<>();
        params.put("id", new String[]{"eq.1"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("select")).thenReturn(null);
        when(request.getParameter("order")).thenReturn(null);

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getFilters());
        assertEquals(1, apiRequest.getFilters().size());
        Filter filter = apiRequest.getFilters().get(0);
        assertEquals("id", filter.getColumn());
        assertEquals(Filter.FilterOperator.EQ, filter.getOperator());
        assertEquals("1", filter.getValue());
    }

    @Test
    void testParseRangeHeader() throws Exception {
        // Setup
        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(createEnumeration("Range"));
        when(request.getHeader("Range")).thenReturn("items=0-9");
        when(request.getParameterMap()).thenReturn(new HashMap<>());

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getRange());
        assertEquals("items", apiRequest.getRange().getUnit());
        assertEquals(0L, apiRequest.getRange().getStart());
        assertEquals(9L, apiRequest.getRange().getEnd());
    }

    @Test
    void testParsePreferHeader() throws Exception {
        // Setup
        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(createEnumeration("Prefer"));
        when(request.getHeader("Prefer")).thenReturn("return=representation, count=exact");
        when(request.getParameterMap()).thenReturn(new HashMap<>());

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getPreferences());
        assertEquals(Preferences.ReturnPreference.REPRESENTATION,
            apiRequest.getPreferences().getReturnPreference());
        assertEquals(Preferences.CountPreference.EXACT,
            apiRequest.getPreferences().getCountPreference());
    }

    @Test
    void testParseSelectParameter() throws Exception {
        // Setup
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"id,name,email"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("select")).thenReturn("id,name,email");

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getSelect());
        assertEquals(3, apiRequest.getSelect().size());
        assertEquals("id", apiRequest.getSelect().get(0).getName());
        assertEquals("name", apiRequest.getSelect().get(1).getName());
        assertEquals("email", apiRequest.getSelect().get(2).getName());
    }

    @Test
    void testParseOrderParameter() throws Exception {
        // Setup
        Map<String, String[]> params = new HashMap<>();
        params.put("order", new String[]{"created_at.desc,id.asc"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("order")).thenReturn("created_at.desc,id.asc");

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getOrderBy());
        assertEquals(2, apiRequest.getOrderBy().size());
        assertEquals("created_at", apiRequest.getOrderBy().get(0).getColumn());
        assertEquals(OrderBy.Direction.DESC, apiRequest.getOrderBy().get(0).getDirection());
        assertEquals("id", apiRequest.getOrderBy().get(1).getColumn());
        assertEquals(OrderBy.Direction.ASC, apiRequest.getOrderBy().get(1).getDirection());
    }

    @Test
    void testParsePostRequestWithBody() throws Exception {
        // Setup
        String body = "{\"name\":\"John\",\"email\":\"john@example.com\"}";

        when(request.getMethod()).thenReturn("POST");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(new HashMap<>());
        when(request.getReader()).thenReturn(new BufferedReader(new StringReader(body)));

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getBody());
        assertEquals(body, apiRequest.getBody());
    }

    @Test
    void testParseMultipleFilters() throws Exception {
        // Setup
        Map<String, String[]> params = new HashMap<>();
        params.put("age.gte", new String[]{"18"});
        params.put("status.eq", new String[]{"active"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getFilters());
        assertEquals(2, apiRequest.getFilters().size());
    }

    @Test
    void testParseOrderWithDescAndNullsLast() throws Exception {
        // Setup - PostgREST format: ?order=start_date.desc.nullslast
        // This is what Supabase JS generates for: .order('start_date', { ascending: false, nullsFirst: false })
        Map<String, String[]> params = new HashMap<>();
        params.put("order", new String[]{"start_date.desc.nullslast"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("order")).thenReturn("start_date.desc.nullslast");

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "events");

        // Verify
        assertNotNull(apiRequest.getOrderBy());
        assertEquals(1, apiRequest.getOrderBy().size());
        OrderBy orderBy = apiRequest.getOrderBy().get(0);
        assertEquals("start_date", orderBy.getColumn());
        assertEquals(OrderBy.Direction.DESC, orderBy.getDirection());
        assertEquals(OrderBy.NullsOrder.LAST, orderBy.getNullsOrder());
    }

    @Test
    void testParseOrderWithAscAndNullsFirst() throws Exception {
        // Setup - PostgREST format: ?order=name.asc.nullsfirst
        Map<String, String[]> params = new HashMap<>();
        params.put("order", new String[]{"name.asc.nullsfirst"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("order")).thenReturn("name.asc.nullsfirst");

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getOrderBy());
        assertEquals(1, apiRequest.getOrderBy().size());
        OrderBy orderBy = apiRequest.getOrderBy().get(0);
        assertEquals("name", orderBy.getColumn());
        assertEquals(OrderBy.Direction.ASC, orderBy.getDirection());
        assertEquals(OrderBy.NullsOrder.FIRST, orderBy.getNullsOrder());
    }

    @Test
    void testParseOrderWithOnlyNullsLast() throws Exception {
        // Setup - direction defaults to ASC when not specified
        Map<String, String[]> params = new HashMap<>();
        params.put("order", new String[]{"created_at.nullslast"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("order")).thenReturn("created_at.nullslast");

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "posts");

        // Verify
        assertNotNull(apiRequest.getOrderBy());
        assertEquals(1, apiRequest.getOrderBy().size());
        OrderBy orderBy = apiRequest.getOrderBy().get(0);
        assertEquals("created_at", orderBy.getColumn());
        assertEquals(OrderBy.Direction.ASC, orderBy.getDirection()); // default
        assertEquals(OrderBy.NullsOrder.LAST, orderBy.getNullsOrder());
    }

    @Test
    void testParseComplexOrderWithMultipleColumns() throws Exception {
        // Setup - Multiple columns with different options
        // ?order=priority.desc.nullslast,created_at.asc.nullsfirst,name.desc
        Map<String, String[]> params = new HashMap<>();
        params.put("order", new String[]{"priority.desc.nullslast,created_at.asc.nullsfirst,name.desc"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("order")).thenReturn("priority.desc.nullslast,created_at.asc.nullsfirst,name.desc");

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "tasks");

        // Verify
        assertNotNull(apiRequest.getOrderBy());
        assertEquals(3, apiRequest.getOrderBy().size());

        // First: priority.desc.nullslast
        OrderBy order1 = apiRequest.getOrderBy().get(0);
        assertEquals("priority", order1.getColumn());
        assertEquals(OrderBy.Direction.DESC, order1.getDirection());
        assertEquals(OrderBy.NullsOrder.LAST, order1.getNullsOrder());

        // Second: created_at.asc.nullsfirst
        OrderBy order2 = apiRequest.getOrderBy().get(1);
        assertEquals("created_at", order2.getColumn());
        assertEquals(OrderBy.Direction.ASC, order2.getDirection());
        assertEquals(OrderBy.NullsOrder.FIRST, order2.getNullsOrder());

        // Third: name.desc (no nulls option)
        OrderBy order3 = apiRequest.getOrderBy().get(2);
        assertEquals("name", order3.getColumn());
        assertEquals(OrderBy.Direction.DESC, order3.getDirection());
        assertNull(order3.getNullsOrder());
    }

    private Enumeration<String> createEnumeration(String... values) {
        return Collections.enumeration(java.util.Arrays.asList(values));
    }

    // ==================== RPC Tests ====================

    @Test
    void testParseRpc_GetWithQueryParams() throws Exception {
        // Setup - GET /rpc/my_function?p_id=1&p_name=test
        Map<String, String[]> params = new HashMap<>();
        params.put("p_id", new String[]{"1"});
        params.put("p_name", new String[]{"test"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("select")).thenReturn(null);
        when(request.getParameter("order")).thenReturn(null);
        when(request.getParameter("limit")).thenReturn(null);
        when(request.getParameter("offset")).thenReturn(null);

        // Execute
        ApiRequest apiRequest = parser.parseRpc(request, "public", "my_function");

        // Verify
        assertNotNull(apiRequest);
        assertEquals("public", apiRequest.getSchema());
        assertEquals("my_function", apiRequest.getTable());
        assertTrue(apiRequest.isRpcCall());
        assertEquals("my_function", apiRequest.getRpcFunctionName());
        assertNotNull(apiRequest.getRpcParams());
        assertEquals(2, apiRequest.getRpcParams().size());
        assertEquals(1L, apiRequest.getRpcParams().get("p_id")); // Parsed as number
        assertEquals("test", apiRequest.getRpcParams().get("p_name"));
    }

    @Test
    void testParseRpc_PostWithJsonBody() throws Exception {
        // Setup - POST /rpc/create_user with JSON body
        String body = "{\"name\":\"John\",\"age\":30,\"active\":true}";

        when(request.getMethod()).thenReturn("POST");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(new HashMap<>());
        when(request.getParameter("select")).thenReturn(null);
        when(request.getParameter("order")).thenReturn(null);
        when(request.getParameter("limit")).thenReturn(null);
        when(request.getParameter("offset")).thenReturn(null);
        when(request.getReader()).thenReturn(new BufferedReader(new StringReader(body)));

        // Execute
        ApiRequest apiRequest = parser.parseRpc(request, "public", "create_user");

        // Verify
        assertNotNull(apiRequest);
        assertTrue(apiRequest.isRpcCall());
        assertEquals("create_user", apiRequest.getRpcFunctionName());
        assertNotNull(apiRequest.getRpcParams());
        assertEquals(3, apiRequest.getRpcParams().size());
        assertEquals("John", apiRequest.getRpcParams().get("name"));
        assertEquals(30, apiRequest.getRpcParams().get("age"));
        assertEquals(true, apiRequest.getRpcParams().get("active"));
    }

    @Test
    void testParseRpc_WithSelectAndOrder() throws Exception {
        // Setup - GET /rpc/get_users?select=id,name&order=name.asc
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"id,name"});
        params.put("order", new String[]{"name.asc"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("select")).thenReturn("id,name");
        when(request.getParameter("order")).thenReturn("name.asc");
        when(request.getParameter("limit")).thenReturn(null);
        when(request.getParameter("offset")).thenReturn(null);

        // Execute
        ApiRequest apiRequest = parser.parseRpc(request, "public", "get_users");

        // Verify
        assertNotNull(apiRequest);
        assertTrue(apiRequest.isRpcCall());

        // Select columns
        assertNotNull(apiRequest.getSelect());
        assertEquals(2, apiRequest.getSelect().size());
        assertEquals("id", apiRequest.getSelect().get(0).getName());
        assertEquals("name", apiRequest.getSelect().get(1).getName());

        // Order by
        assertNotNull(apiRequest.getOrderBy());
        assertEquals(1, apiRequest.getOrderBy().size());
        assertEquals("name", apiRequest.getOrderBy().get(0).getColumn());
        assertEquals(OrderBy.Direction.ASC, apiRequest.getOrderBy().get(0).getDirection());
    }

    @Test
    void testParseRpc_WithPagination() throws Exception {
        // Setup - GET /rpc/get_all?limit=10&offset=20
        Map<String, String[]> params = new HashMap<>();
        params.put("limit", new String[]{"10"});
        params.put("offset", new String[]{"20"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("select")).thenReturn(null);
        when(request.getParameter("order")).thenReturn(null);
        when(request.getParameter("limit")).thenReturn("10");
        when(request.getParameter("offset")).thenReturn("20");

        // Execute
        ApiRequest apiRequest = parser.parseRpc(request, "public", "get_all");

        // Verify
        assertNotNull(apiRequest);
        assertTrue(apiRequest.isRpcCall());
        assertNotNull(apiRequest.getRange());
        assertEquals(20L, apiRequest.getRange().getStart());
        assertEquals(29L, apiRequest.getRange().getEnd()); // 20 + 10 - 1 = 29
    }

    @Test
    void testParseRpc_WithJsonArrayParam() throws Exception {
        // Setup - GET /rpc/process_ids?ids=[1,2,3]
        Map<String, String[]> params = new HashMap<>();
        params.put("ids", new String[]{"[1,2,3]"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("select")).thenReturn(null);
        when(request.getParameter("order")).thenReturn(null);
        when(request.getParameter("limit")).thenReturn(null);
        when(request.getParameter("offset")).thenReturn(null);

        // Execute
        ApiRequest apiRequest = parser.parseRpc(request, "public", "process_ids");

        // Verify
        assertNotNull(apiRequest);
        assertTrue(apiRequest.isRpcCall());
        assertNotNull(apiRequest.getRpcParams());
        Object ids = apiRequest.getRpcParams().get("ids");
        assertNotNull(ids);
        assertTrue(ids instanceof java.util.List);
        @SuppressWarnings("unchecked")
        java.util.List<Integer> idList = (java.util.List<Integer>) ids;
        assertEquals(3, idList.size());
    }

    @Test
    void testParseRpc_WithBooleanAndNullParams() throws Exception {
        // Setup - GET /rpc/check?active=true&deleted=false&nullable=null
        Map<String, String[]> params = new HashMap<>();
        params.put("active", new String[]{"true"});
        params.put("deleted", new String[]{"false"});
        params.put("nullable", new String[]{"null"});

        when(request.getMethod()).thenReturn("GET");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("select")).thenReturn(null);
        when(request.getParameter("order")).thenReturn(null);
        when(request.getParameter("limit")).thenReturn(null);
        when(request.getParameter("offset")).thenReturn(null);

        // Execute
        ApiRequest apiRequest = parser.parseRpc(request, "public", "check");

        // Verify
        assertNotNull(apiRequest);
        assertTrue(apiRequest.isRpcCall());
        assertEquals(true, apiRequest.getRpcParams().get("active"));
        assertEquals(false, apiRequest.getRpcParams().get("deleted"));
        assertNull(apiRequest.getRpcParams().get("nullable"));
    }

    // ==================== Missing=Default Tests ====================

    @Test
    void testParseMissingDefaultPreference() throws Exception {
        // Setup - POST with Prefer: missing=default
        when(request.getMethod()).thenReturn("POST");
        when(request.getHeaderNames()).thenReturn(createEnumeration("Prefer"));
        when(request.getHeader("Prefer")).thenReturn("missing=default, return=representation");
        when(request.getParameterMap()).thenReturn(new HashMap<>());
        when(request.getReader()).thenReturn(new BufferedReader(new StringReader("{\"name\":\"test\"}")));

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getPreferences());
        assertEquals(Preferences.MissingPreference.DEFAULT, apiRequest.getPreferences().getMissingPreference());
        assertEquals(Preferences.ReturnPreference.REPRESENTATION, apiRequest.getPreferences().getReturnPreference());
    }

    @Test
    void testParseColumnsParameter() throws Exception {
        // Setup - POST with ?columns=id,name,status
        Map<String, String[]> params = new HashMap<>();
        params.put("columns", new String[]{"id,name,status"});

        when(request.getMethod()).thenReturn("POST");
        when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        when(request.getParameterMap()).thenReturn(params);
        when(request.getParameter("columns")).thenReturn("id,name,status");
        when(request.getReader()).thenReturn(new BufferedReader(new StringReader("{\"name\":\"test\"}")));

        // Execute
        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify
        assertNotNull(apiRequest.getColumns());
        assertEquals(3, apiRequest.getColumns().size());
        assertTrue(apiRequest.getColumns().contains("id"));
        assertTrue(apiRequest.getColumns().contains("name"));
        assertTrue(apiRequest.getColumns().contains("status"));
    }
}
