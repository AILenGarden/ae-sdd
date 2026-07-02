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
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.when;

/**
 * SELECT query parsing tests based on PostgREST's QuerySpec.hs
 */
@ExtendWith(MockitoExtension.class)
class SelectQueryTest {

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

    // ==================== Aggregate Function Tests ====================

    @Test
    void testSelect_SimpleAggregate_Sum() throws Exception {
        // ?select=amount.sum()
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"amount.sum()"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "orders");

        assertNotNull(apiRequest.getSelect());
        assertEquals(1, apiRequest.getSelect().size());
        SelectColumn col = apiRequest.getSelect().get(0);
        assertTrue(col.isAggregate());
        assertEquals("sum", col.getAggregateFunction());
        assertEquals("amount", col.getName());
    }

    @Test
    void testSelect_SimpleAggregate_Count() throws Exception {
        // ?select=count()
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"count()"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest.getSelect());
        assertEquals(1, apiRequest.getSelect().size());
        SelectColumn col = apiRequest.getSelect().get(0);
        assertTrue(col.isAggregate());
        assertEquals("count", col.getAggregateFunction());
        assertEquals("*", col.getName()); // count() without column uses *
    }

    @Test
    void testSelect_AggregateWithAlias() throws Exception {
        // ?select=total:amount.sum()
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"total:amount.sum()"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "orders");

        assertNotNull(apiRequest.getSelect());
        assertEquals(1, apiRequest.getSelect().size());
        SelectColumn col = apiRequest.getSelect().get(0);
        assertTrue(col.isAggregate());
        assertEquals("sum", col.getAggregateFunction());
        assertEquals("amount", col.getName());
        assertEquals("total", col.getAlias());
    }

    @Test
    void testSelect_MultipleAggregates() throws Exception {
        // ?select=total:amount.sum(),average:amount.avg(),rows:count()
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"total:amount.sum(),average:amount.avg(),rows:count()"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "orders");

        assertNotNull(apiRequest.getSelect());
        assertEquals(3, apiRequest.getSelect().size());

        // total:amount.sum()
        SelectColumn col1 = apiRequest.getSelect().get(0);
        assertTrue(col1.isAggregate());
        assertEquals("sum", col1.getAggregateFunction());
        assertEquals("amount", col1.getName());
        assertEquals("total", col1.getAlias());

        // average:amount.avg()
        SelectColumn col2 = apiRequest.getSelect().get(1);
        assertTrue(col2.isAggregate());
        assertEquals("avg", col2.getAggregateFunction());
        assertEquals("amount", col2.getName());
        assertEquals("average", col2.getAlias());

        // rows:count()
        SelectColumn col3 = apiRequest.getSelect().get(2);
        assertTrue(col3.isAggregate());
        assertEquals("count", col3.getAggregateFunction());
        assertEquals("*", col3.getName());
        assertEquals("rows", col3.getAlias());
    }

    @Test
    void testSelect_AggregateWithGroupBy() throws Exception {
        // ?select=order_date,amount.sum()
        // PostgREST automatically groups by non-aggregated columns
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"order_date,amount.sum()"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "orders");

        assertNotNull(apiRequest.getSelect());
        assertEquals(2, apiRequest.getSelect().size());

        // order_date (non-aggregate)
        SelectColumn col1 = apiRequest.getSelect().get(0);
        assertFalse(col1.isAggregate());
        assertEquals("order_date", col1.getName());

        // amount.sum() (aggregate)
        SelectColumn col2 = apiRequest.getSelect().get(1);
        assertTrue(col2.isAggregate());
        assertEquals("sum", col2.getAggregateFunction());
        assertEquals("amount", col2.getName());
    }

    @Test
    void testSelect_AllAggregates() throws Exception {
        // Test all supported aggregates: count, sum, avg, min, max
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"id.count(),amount.sum(),price.avg(),age.min(),score.max()"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "data");

        assertNotNull(apiRequest.getSelect());
        assertEquals(5, apiRequest.getSelect().size());

        assertEquals("count", apiRequest.getSelect().get(0).getAggregateFunction());
        assertEquals("sum", apiRequest.getSelect().get(1).getAggregateFunction());
        assertEquals("avg", apiRequest.getSelect().get(2).getAggregateFunction());
        assertEquals("min", apiRequest.getSelect().get(3).getAggregateFunction());
        assertEquals("max", apiRequest.getSelect().get(4).getAggregateFunction());
    }

    @Test
    void testSelect_AllColumns() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals("GET", apiRequest.getMethod());
        assertEquals("public", apiRequest.getSchema());
        assertEquals("users", apiRequest.getTable());
    }

    @Test
    void testSelect_SpecificColumns() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"id,name,email"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

      List<String> columns = apiRequest.getSelect().stream().map(selectColumn -> selectColumn.getName()).toList();
        assertNotNull(apiRequest.getSelect());
        assertEquals(3, apiRequest.getSelect().size());
        assertTrue(columns.contains("id"));
        assertTrue(columns.contains("name"));
        assertTrue(columns.contains("email"));
    }

    @Test
    void testSelect_WithSingleFilter() throws Exception {
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
    void testSelect_WithMultipleFilters() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("age", new String[]{"gte.18"});
        params.put("status", new String[]{"eq.active"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertEquals(2, apiRequest.getFilters().size());
    }

    @Test
    void testSelect_WithOrderBy_SingleColumn() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("order", new String[]{"created_at.desc"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "posts");

        assertNotNull(apiRequest.getOrderBy());
        assertEquals(1, apiRequest.getOrderBy().size());
        OrderBy orderBy = apiRequest.getOrderBy().get(0);
        assertEquals("created_at", orderBy.getColumn());
        assertEquals(OrderBy.Direction.DESC, orderBy.getDirection());
    }

    @Test
    void testSelect_WithOrderBy_MultipleColumns() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("order", new String[]{"created_at.desc,id.asc"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "posts");

        assertNotNull(apiRequest.getOrderBy());
        assertEquals(2, apiRequest.getOrderBy().size());
    }

    @Test
    void testSelect_WithRange_FirstPage() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("limit", new String[]{"10"});
        params.put("offset", new String[]{"0"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest.getRange());
        assertEquals(0L, apiRequest.getRange().getStart());
        assertEquals(9L, apiRequest.getRange().getEnd());
    }

    @Test
    void testSelect_WithRange_SecondPage() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("limit", new String[]{"10"});
        params.put("offset", new String[]{"10"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        assertNotNull(apiRequest.getRange());
        assertEquals(10L, apiRequest.getRange().getStart());
        assertEquals(19L, apiRequest.getRange().getEnd());
    }

    @Test
    void testSelect_ComplexQuery() throws Exception {
        Map<String, String[]> params = new HashMap<>();
        params.put("select", new String[]{"id,name,email"});
        params.put("age", new String[]{"gte.18"});
        params.put("status", new String[]{"eq.active"});
        params.put("order", new String[]{"name.asc"});
        params.put("limit", new String[]{"20"});
        params.put("offset", new String[]{"0"});
        setupMockRequest(params);

        ApiRequest apiRequest = parser.parse(request, "public", "users");

        // Verify all parts
        assertEquals(3, apiRequest.getSelect().size());
        assertEquals(2, apiRequest.getFilters().size());
        assertEquals(1, apiRequest.getOrderBy().size());
        assertNotNull(apiRequest.getRange());
        assertEquals(20L, apiRequest.getRange().getLimit());
    }
}
