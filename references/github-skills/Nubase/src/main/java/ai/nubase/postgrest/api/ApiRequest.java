package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

import java.util.List;
import java.util.Map;

/**
 * Parsed API request representation
 * Equivalent to PostgREST's ApiRequest type
 */
@Data
@Builder
public class ApiRequest {
    private String schema;
    private String table;
    private String method; // GET, POST, PUT, PATCH, DELETE, HEAD
    private List<QueryParam> queryParams;
    private Map<String, String> headers;
    private String body;
    private RangeHeader range;
    private Preferences preferences;
    private List<SelectColumn> select;
    private List<Filter> filters;  // Simple filters (backward compatibility)
    private List<LogicalCondition> logicalConditions;  // Complex OR/AND conditions
    private List<OrderBy> orderBy;
    private UpsertOption upsertOption; // For PUT requests

    // RPC-specific fields
    private boolean rpcCall;            // True if this is an RPC call (/rpc/function_name)
    private String rpcFunctionName;     // Function name to call
    private Map<String, Object> rpcParams; // Function parameters (from body or query params)

    // Column specification for INSERT/UPDATE
    private List<String> columns;       // Columns specified in ?columns= param
}
