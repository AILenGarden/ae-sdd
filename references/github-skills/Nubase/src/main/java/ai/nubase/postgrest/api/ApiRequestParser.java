package ai.nubase.postgrest.api;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.servlet.http.HttpServletRequest;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

import java.io.BufferedReader;
import java.io.IOException;
import java.util.*;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Parses HTTP requests into ApiRequest objects
 * Equivalent to PostgREST's ApiRequest parsing logic
 */
@Slf4j
@Service
public class ApiRequestParser {

    private static final Pattern RANGE_PATTERN = Pattern.compile("(\\w+)=(\\d+)?-(\\d+)?");
    private static final Pattern FILTER_PATTERN = Pattern.compile(
            "(\\w+)\\.(eq|neq|gt|gte|lt|lte|like|ilike|match|imatch|in|is|fts|plfts|phfts|wfts|cs|cd|ov|sl|sr|nxr|nxl|adj|isdistinct|not)");

    private final ObjectMapper objectMapper;

    public ApiRequestParser(ObjectMapper objectMapper) {
        this.objectMapper = objectMapper;
    }

    public ApiRequest parse(HttpServletRequest request, String schema, String table) throws IOException {
        ApiRequest.ApiRequestBuilder builder = ApiRequest.builder()
                .schema(schema)
                .table(table)
                .method(request.getMethod())
                .headers(extractHeaders(request))
                .queryParams(extractQueryParams(request));

        // Parse Range header
        String rangeHeader = request.getHeader("Range");
        if (rangeHeader != null) {
            builder.range(parseRange(rangeHeader));
        } else {
            // Parse limit/offset parameters and convert to Range
            // Only if Range header is not present
            String limit = request.getParameter("limit");
            String offset = request.getParameter("offset");
            if (limit != null) {
                builder.range(parseLimitOffset(limit, offset));
            }
        }

        // Parse Prefer header
        String preferHeader = request.getHeader("Prefer");
        if (preferHeader != null) {
            builder.preferences(parsePreferences(preferHeader));
        }

        // Parse select parameter
        String select = request.getParameter("select");
        if (select != null) {
            builder.select(parseSelect(select));
        }

        // Parse filters and logical conditions
        FilterParseResult filterResult = parseAllFilters(request);
        builder.filters(filterResult.filters());
        builder.logicalConditions(filterResult.logicalConditions());

        // Parse order parameter
        String order = request.getParameter("order");
        if (order != null) {
            builder.orderBy(parseOrderBy(order));
        }

        // Parse on_conflict parameter for UPSERT
        String onConflict = request.getParameter("on_conflict");
        if (onConflict != null && !onConflict.isEmpty()) {
            UpsertOption.UpsertOptionBuilder upsertBuilder = UpsertOption.builder();
            // Parse conflict columns (comma-separated)
            List<String> conflictColumns = Arrays.asList(onConflict.split(","));
            upsertBuilder.conflictColumns(conflictColumns.stream()
                .map(String::strip)
                .toList());

            // Determine resolution from Prefer header
            Preferences prefs = builder.build().getPreferences();
            if (prefs != null && prefs.getResolution() == Preferences.Resolution.IGNORE_DUPLICATES) {
                upsertBuilder.resolution(UpsertOption.Resolution.IGNORE_DUPLICATES);
            } else {
                // Default to merge-duplicates (DO UPDATE)
                upsertBuilder.resolution(UpsertOption.Resolution.MERGE_DUPLICATES);
            }
            builder.upsertOption(upsertBuilder.build());
        }

        // Parse columns parameter (for INSERT/UPDATE with missing=default)
        String columns = request.getParameter("columns");
        if (columns != null && !columns.isEmpty()) {
            List<String> columnList = Arrays.stream(columns.split(","))
                .map(String::strip)
                .toList();
            builder.columns(columnList);
        }

        // Parse body
        if ("POST".equals(request.getMethod()) || "PUT".equals(request.getMethod())
                || "PATCH".equals(request.getMethod())) {
            builder.body(readBody(request));
        }

        return builder.build();
    }

    /**
     * Parse an RPC (stored procedure call) request.
     * PostgREST RPC format:
     * - GET /rpc/function_name?param1=value1&param2=value2
     * - POST /rpc/function_name with JSON body {"param1": "value1", "param2": "value2"}
     *
     * @param request The HTTP request
     * @param schema The database schema
     * @param functionName The function name to call
     * @return Parsed ApiRequest with RPC-specific fields
     */
    public ApiRequest parseRpc(HttpServletRequest request, String schema, String functionName) throws IOException {
        ApiRequest.ApiRequestBuilder builder = ApiRequest.builder()
                .schema(schema)
                .table(functionName) // Use table field for function name for compatibility
                .method(request.getMethod())
                .headers(extractHeaders(request))
                .queryParams(extractQueryParams(request))
                .rpcCall(true)
                .rpcFunctionName(functionName);

        // Parse Range header for pagination of results
        String rangeHeader = request.getHeader("Range");
        if (rangeHeader != null) {
            builder.range(parseRange(rangeHeader));
        } else {
            String limit = request.getParameter("limit");
            String offset = request.getParameter("offset");
            if (limit != null) {
                builder.range(parseLimitOffset(limit, offset));
            }
        }

        // Parse Prefer header
        String preferHeader = request.getHeader("Prefer");
        if (preferHeader != null) {
            builder.preferences(parsePreferences(preferHeader));
        }

        // Parse select parameter (for filtering returned columns)
        String select = request.getParameter("select");
        if (select != null) {
            builder.select(parseSelect(select));
        }

        // Parse order parameter (for ordering results)
        String order = request.getParameter("order");
        if (order != null) {
            builder.orderBy(parseOrderBy(order));
        }

        // Parse RPC parameters
        Map<String, Object> rpcParams = new HashMap<>();

        if ("GET".equals(request.getMethod()) || "HEAD".equals(request.getMethod())) {
            // For GET/HEAD, parameters come from query string
            // Skip reserved parameters
            Set<String> reservedParams = Set.of("select", "order", "limit", "offset");
            request.getParameterMap().forEach((name, values) -> {
                if (!reservedParams.contains(name) && values.length > 0) {
                    String value = values[0];
                    // Try to parse as JSON if it looks like JSON
                    rpcParams.put(name, parseRpcParamValue(value));
                }
            });
        } else if ("POST".equals(request.getMethod())) {
            // For POST, parameters come from JSON body
            String body = readBody(request);
            builder.body(body);

            if (body != null && !body.strip().isEmpty()) {
                try {
                    // Parse JSON body as parameters
                    Map<String, Object> bodyParams = objectMapper.readValue(body,
                            new TypeReference<Map<String, Object>>() {});
                    rpcParams.putAll(bodyParams);
                } catch (Exception e) {
                    log.warn("Failed to parse RPC body as JSON: {}", e.getMessage());
                    // For scalar functions, the body might be a single value
                    // In this case, we don't have named parameters
                }
            }
        }

        builder.rpcParams(rpcParams);

        // Parse filters (for filtering function results if it returns a table)
        FilterParseResult filterResult = parseAllFilters(request);
        builder.filters(filterResult.filters());
        builder.logicalConditions(filterResult.logicalConditions());

        return builder.build();
    }

    /**
     * Parse an RPC parameter value, attempting to convert JSON-like strings to appropriate types.
     */
    private Object parseRpcParamValue(String value) {
        if (value == null) {
            return null;
        }

        // Try to parse as JSON if it looks like JSON (object, array, or quoted string)
        String trimmed = value.strip();
        if ((trimmed.startsWith("{") && trimmed.endsWith("}")) ||
            (trimmed.startsWith("[") && trimmed.endsWith("]"))) {
            try {
                return objectMapper.readValue(trimmed, Object.class);
            } catch (Exception e) {
                // Not valid JSON, treat as string
            }
        }

        // Try to parse as number
        try {
            if (trimmed.contains(".")) {
                return Double.parseDouble(trimmed);
            } else {
                return Long.parseLong(trimmed);
            }
        } catch (NumberFormatException e) {
            // Not a number
        }

        // Check for boolean
        if ("true".equalsIgnoreCase(trimmed)) {
            return true;
        } else if ("false".equalsIgnoreCase(trimmed)) {
            return false;
        }

        // Check for null
        if ("null".equalsIgnoreCase(trimmed)) {
            return null;
        }

        // Return as string
        return value;
    }

    private Map<String, String> extractHeaders(HttpServletRequest request) {
        Map<String, String> headers = new HashMap<>();
        Enumeration<String> headerNames = request.getHeaderNames();
        while (headerNames.hasMoreElements()) {
            String name = headerNames.nextElement();
            headers.put(name.toLowerCase(), request.getHeader(name));
        }
        return headers;
    }

    private List<QueryParam> extractQueryParams(HttpServletRequest request) {
        List<QueryParam> params = new ArrayList<>();
        request.getParameterMap().forEach((name, values) -> {
            if (values.length > 0) {
                params.add(QueryParam.builder()
                        .name(name)
                        .value(values[0])
                        .build());
            }
        });
        return params;
    }

    private RangeHeader parseRange(String rangeHeader) {
        Matcher matcher = RANGE_PATTERN.matcher(rangeHeader);
        if (matcher.find()) {
            String unit = matcher.group(1);
            String startStr = matcher.group(2);
            String endStr = matcher.group(3);

            return RangeHeader.builder()
                    .unit(unit)
                    .start(startStr != null ? Long.parseLong(startStr) : null)
                    .end(endStr != null ? Long.parseLong(endStr) : null)
                    .build();
        }
        return null;
    }

    /**
     * Convert limit/offset query parameters to RangeHeader
     * PostgREST format: ?limit=10&offset=0 -> Range: items=0-9
     */
    private RangeHeader parseLimitOffset(String limitStr, String offsetStr) {
        try {
            long limit = Long.parseLong(limitStr);
            long offset = offsetStr != null ? Long.parseLong(offsetStr) : 0;

            // Validate parameters
            if (limit <= 0) {
                log.warn("Invalid limit parameter: {} (must be positive)", limit);
                return null;
            }
            if (offset < 0) {
                log.warn("Invalid offset parameter: {} (must be non-negative)", offset);
                return null;
            }

            return RangeHeader.builder()
                    .unit("items")
                    .start(offset)
                    .end(offset + limit - 1)
                    .build();
        } catch (NumberFormatException e) {
            log.warn("Invalid limit/offset parameters: limit={}, offset={}", limitStr, offsetStr);
            return null;
        }
    }

    private Preferences parsePreferences(String preferHeader) {
        Preferences.PreferencesBuilder builder = Preferences.builder();

        String[] prefs = preferHeader.split(",");
        for (String pref : prefs) {
            pref = pref.strip();

            if (pref.startsWith("return=")) {
                String value = pref.substring(7);
                builder.returnPreference(switch (value) {
                    case "representation" -> Preferences.ReturnPreference.REPRESENTATION;
                    case "minimal" -> Preferences.ReturnPreference.MINIMAL;
                    case "headers-only" -> Preferences.ReturnPreference.HEADERS_ONLY;
                    default -> Preferences.ReturnPreference.MINIMAL;
                });
            } else if (pref.startsWith("count=")) {
                String value = pref.substring(6);
                builder.countPreference(switch (value) {
                    case "exact" -> Preferences.CountPreference.EXACT;
                    case "planned" -> Preferences.CountPreference.PLANNED;
                    case "estimated" -> Preferences.CountPreference.ESTIMATED;
                    default -> Preferences.CountPreference.NONE;
                });
            } else if (pref.startsWith("resolution=")) {
                String value = pref.substring(11);
                builder.resolution(switch (value) {
                    case "merge-duplicates" -> Preferences.Resolution.MERGE_DUPLICATES;
                    case "ignore-duplicates" -> Preferences.Resolution.IGNORE_DUPLICATES;
                    default -> null;
                });
            } else if (pref.startsWith("timezone=")) {
                builder.timezone(pref.substring(9));
            } else if (pref.startsWith("missing=")) {
                String value = pref.substring(8);
                builder.missingPreference(switch (value) {
                    case "default" -> Preferences.MissingPreference.DEFAULT;
                    default -> Preferences.MissingPreference.NULL;
                });
            }
        }

        return builder.build();
    }

    // Pattern for aggregate functions: column.function() or function()
    private static final Pattern AGGREGATE_PATTERN = Pattern.compile(
        "^([a-zA-Z_][a-zA-Z0-9_]*)?\\.?(count|sum|avg|min|max)\\(\\)(.*)$"
    );

    private static List<SelectColumn> parseSelect(String select) {
        List<SelectColumn> columns = new ArrayList<>();
        List<String> parts = splitTopLevel(select, ',');

        for (String part : parts) {
            part = part.strip();

            // Check for alias prefix first: alias:rest
            String alias = null;
            String restOfPart = part;

            // Handle alias:something format (but not count() which has no :)
            if (part.contains(":") && !isAggregateOnly(part)) {
                int colonIndex = part.indexOf(":");
                // Make sure this colon is not inside parentheses
                int parenDepth = 0;
                boolean colonOutside = true;
                for (int i = 0; i < colonIndex; i++) {
                    if (part.charAt(i) == '(') parenDepth++;
                    if (part.charAt(i) == ')') parenDepth--;
                }
                if (parenDepth == 0) {
                    alias = part.substring(0, colonIndex).strip();
                    restOfPart = part.substring(colonIndex + 1).strip();
                }
            }

            // Check for aggregate functions: column.sum(), column.avg(), count(), etc.
            SelectColumn aggregateCol = parseAggregateColumn(restOfPart, alias);
            if (aggregateCol != null) {
                columns.add(aggregateCol);
                continue;
            }

            // Check for embedded resources: [alias:]table!hint(subselect)
            if (restOfPart.contains("(")) {
                int parenIndex = restOfPart.indexOf("(");
                String beforeParen = restOfPart.substring(0, parenIndex);
                String embedded = restOfPart.substring(parenIndex + 1, restOfPart.lastIndexOf(")"));

                String tableName = beforeParen;
                String hint = null;

                // If alias was already extracted at the beginning, don't re-parse
                if (alias == null && beforeParen.contains(":")) {
                    String[] aliasParts = beforeParen.split(":", 2);
                    alias = aliasParts[0].strip();
                    tableName = aliasParts[1].strip();
                }

                // Extract hint: table!inner(...) or table!left(...)
                if (tableName.contains("!")) {
                    int bangIndex = tableName.indexOf("!");
                    hint = tableName.substring(bangIndex + 1).strip();
                    tableName = tableName.substring(0, bangIndex).strip();
                }

                columns.add(SelectColumn.builder()
                        .name(tableName.strip())
                        .alias(alias != null ? alias.strip() : null)
                        .hint(hint != null ? hint.strip() : null)
                        .embedded(parseSelect(embedded))
                        .build());
            } else if (alias != null) {
                // Already have alias from earlier parsing
                columns.add(SelectColumn.builder()
                        .name(restOfPart.strip())
                        .alias(alias.strip())
                        .build());
            } else {
                columns.add(SelectColumn.builder()
                        .name(restOfPart.strip())
                        .build());
            }
        }

        return columns;
    }

    /**
     * Check if a part is a standalone aggregate like count()
     */
    private static boolean isAggregateOnly(String part) {
        return part.matches("^(count|sum|avg|min|max)\\(\\).*$");
    }

    /**
     * Parse an aggregate column specification.
     * Formats:
     *   - count() - count all rows
     *   - column.sum() - sum of column
     *   - column.avg()::int - with cast
     *
     * @param columnSpec The column specification (after alias is removed)
     * @param alias The alias (may be null)
     * @return SelectColumn if this is an aggregate, null otherwise
     */
    private static SelectColumn parseAggregateColumn(String columnSpec, String alias) {
        // Check for aggregate pattern: column.function() or function()
        Matcher matcher = AGGREGATE_PATTERN.matcher(columnSpec);
        if (matcher.matches()) {
            String columnName = matcher.group(1); // may be null for count()
            String aggregateFunc = matcher.group(2);
            String suffix = matcher.group(3); // may contain cast like ::int

            // Handle type casting suffix (e.g., ::int, ::numeric)
            String cast = null;
            if (suffix != null && suffix.startsWith("::")) {
                cast = suffix.substring(2);
            }

            // For count() without column, use "*" as column name
            if (columnName == null || columnName.isEmpty()) {
                columnName = "*";
            }

            return SelectColumn.builder()
                .name(columnName)
                .alias(alias)
                .isAggregate(true)
                .aggregateFunction(aggregateFunc)
                .build();
        }

        return null;
    }

    /**
     * Split string by delimiter at top-level only (ignoring delimiters inside
     * parentheses).
     */
    private static List<String> splitTopLevel(String input, char delimiter) {
        List<String> result = new ArrayList<>();
        int depth = 0;
        int start = 0;

        for (int i = 0; i < input.length(); i++) {
            char c = input.charAt(i);
            if (c == '(') {
                depth++;
            } else if (c == ')') {
                depth--;
            } else if (c == delimiter && depth == 0) {
                result.add(input.substring(start, i));
                start = i + 1;
            }
        }
        result.add(input.substring(start));
        return result;
    }

    /**
     * Parse all filter conditions from request, including simple filters and logical conditions (or/and).
     * Returns a record containing both types.
     */
    private FilterParseResult parseAllFilters(HttpServletRequest request) {
        List<Filter> filters = new ArrayList<>();
        List<LogicalCondition> logicalConditions = new ArrayList<>();

        request.getParameterMap().forEach((name, values) -> {
            // Skip reserved parameters
            if (name.equals("select") || name.equals("order") ||
                    name.equals("limit") || name.equals("offset") || name.equals("on_conflict")) {
                return;
            }

            String value = values[0];

            // Check for negation prefix first
            boolean negate = false;
            String nameToProcess = name;
            if (name.startsWith("not.")) {
                negate = true;
                nameToProcess = name.substring(4); // Remove "not." prefix
            }

            // Check if this is a logical operator (or/and)
            if (nameToProcess.equals("or") || nameToProcess.equals("and")) {
                LogicalCondition condition = parseLogicalCondition(nameToProcess, value, negate);
                if (condition != null) {
                    logicalConditions.add(condition);
                }
                return;
            }

            // Check for negation in value
            String valueToProcess = value;
            if (value.startsWith("not.")) {
                negate = true;
                valueToProcess = value.substring(4); // Remove "not." prefix
            }

            // PostgREST format: ?column=operator.value or ?column=operator(quantifier).{values}
            // Try to match the value part for operator (without "not" in the pattern)
            // Pattern with quantifier: operator(any|all).{value1,value2,...}
            Pattern quantifierPattern = Pattern.compile(
                    "^(eq|neq|gt|gte|lt|lte|like|ilike|match|imatch)\\((any|all)\\)\\.(.*)$");
            Matcher quantifierMatcher = quantifierPattern.matcher(valueToProcess);

            // Standard pattern: operator.value
            Pattern valuePattern = Pattern.compile(
                    "^(eq|neq|gt|gte|lt|lte|like|ilike|match|imatch|in|is|fts|plfts|phfts|wfts|cs|cd|ov|sl|sr|nxr|nxl|adj|isdistinct)\\.(.*)$");
            Matcher valueMatcher = valuePattern.matcher(valueToProcess);

            if (quantifierMatcher.matches()) {
                // Parse quantified filter: eq(any).{1,2,3}
                String operatorStr = quantifierMatcher.group(1);
                String quantifierStr = quantifierMatcher.group(2);
                String actualValue = quantifierMatcher.group(3);

                Filter.FilterOperator operator = parseFilterOperator(operatorStr);
                Filter.Quantifier quantifier = "any".equals(quantifierStr)
                    ? Filter.Quantifier.ANY
                    : Filter.Quantifier.ALL;

                filters.add(Filter.builder()
                        .column(nameToProcess.strip())
                        .operator(operator)
                        .value(actualValue)
                        .negate(negate)
                        .quantifier(quantifier)
                        .build());
            } else if (valueMatcher.matches()) {
                String operatorStr = valueMatcher.group(1);
                String actualValue = valueMatcher.group(2);

                Filter.FilterOperator operator = parseFilterOperator(operatorStr);

                filters.add(Filter.builder()
                        .column(nameToProcess.strip())
                        .operator(operator)
                        .value(actualValue)
                        .negate(negate)
                        .build());
            } else {
                // Default to equality
                filters.add(Filter.builder()
                        .column(nameToProcess.strip())
                        .operator(Filter.FilterOperator.EQ)
                        .value(value)
                        .negate(negate)
                        .build());
            }
        });

        return new FilterParseResult(filters, logicalConditions);
    }

    /**
     * Record to hold both simple filters and logical conditions
     */
    private record FilterParseResult(List<Filter> filters, List<LogicalCondition> logicalConditions) {}

    /**
     * Parse a logical condition (or/and) with its nested conditions.
     * Format: or=(condition1,condition2,...) or and=(condition1,condition2,...)
     * Each condition can be: column.operator.value or nested or/and
     */
    private LogicalCondition parseLogicalCondition(String logicalOp, String value, boolean negate) {
        // Value should be in format: (condition1,condition2,...)
        if (!value.startsWith("(") || !value.endsWith(")")) {
            log.warn("Invalid logical condition format: {}={}", logicalOp, value);
            return null;
        }

        // Extract content inside parentheses
        String content = value.substring(1, value.length() - 1);

        // Split by comma at top level (respecting nested parentheses)
        List<String> parts = splitTopLevel(content, ',');

        List<LogicalCondition> subConditions = new ArrayList<>();
        for (String part : parts) {
            part = part.strip();
            if (part.isEmpty()) continue;

            LogicalCondition subCondition = parseSingleCondition(part);
            if (subCondition != null) {
                subConditions.add(subCondition);
            }
        }

        if (subConditions.isEmpty()) {
            return null;
        }

        if ("or".equals(logicalOp)) {
            return LogicalCondition.or(subConditions, negate);
        } else {
            return LogicalCondition.and(subConditions, negate);
        }
    }

    /**
     * Parse a single condition which can be:
     * 1. A simple filter: column.operator.value
     * 2. A nested logical group: or(...) or and(...)
     */
    private LogicalCondition parseSingleCondition(String condition) {
        // Check for negation prefix
        boolean negate = false;
        String conditionToProcess = condition;
        if (condition.startsWith("not.")) {
            negate = true;
            conditionToProcess = condition.substring(4);
        }

        // Check if this is a nested or/and
        if (conditionToProcess.startsWith("or(") || conditionToProcess.startsWith("and(")) {
            int parenStart = conditionToProcess.indexOf('(');
            String logicalOp = conditionToProcess.substring(0, parenStart);
            String nestedValue = conditionToProcess.substring(parenStart);
            return parseLogicalCondition(logicalOp, nestedValue, negate);
        }

        // Otherwise, it's a simple filter: column.operator.value
        Filter filter = parseFilterString(conditionToProcess, negate);
        if (filter != null) {
            return LogicalCondition.filter(filter);
        }

        return null;
    }

    /**
     * Parse a filter string in format: column.operator.value or column.operator(quantifier).{values}
     */
    private Filter parseFilterString(String filterStr, boolean negate) {
        // Pattern with quantifier: column.operator(any|all).{value1,value2,...}
        Pattern quantifierPattern = Pattern.compile(
            "^([a-zA-Z_][a-zA-Z0-9_]*)\\.(eq|neq|gt|gte|lt|lte|like|ilike|match|imatch)\\((any|all)\\)\\.(.*)$");
        Matcher quantifierMatcher = quantifierPattern.matcher(filterStr);

        if (quantifierMatcher.matches()) {
            String column = quantifierMatcher.group(1).strip();
            String operatorStr = quantifierMatcher.group(2);
            String quantifierStr = quantifierMatcher.group(3);
            String value = quantifierMatcher.group(4);

            Filter.Quantifier quantifier = "any".equals(quantifierStr)
                ? Filter.Quantifier.ANY
                : Filter.Quantifier.ALL;

            return Filter.builder()
                .column(column)
                .operator(parseFilterOperator(operatorStr))
                .value(value)
                .negate(negate)
                .quantifier(quantifier)
                .build();
        }

        // Standard pattern: column.operator.value
        Pattern pattern = Pattern.compile(
            "^([a-zA-Z_][a-zA-Z0-9_]*)\\.(eq|neq|gt|gte|lt|lte|like|ilike|match|imatch|in|is|fts|plfts|phfts|wfts|cs|cd|ov|sl|sr|nxr|nxl|adj|isdistinct)\\.(.*)$");
        Matcher matcher = pattern.matcher(filterStr);

        if (matcher.matches()) {
            String column = matcher.group(1).strip();
            String operatorStr = matcher.group(2);
            String value = matcher.group(3);

            return Filter.builder()
                .column(column)
                .operator(parseFilterOperator(operatorStr))
                .value(value)
                .negate(negate)
                .build();
        }

        log.warn("Could not parse filter string: {}", filterStr);
        return null;
    }

    /**
     * Backward compatible method - returns only simple filters
     */
    private List<Filter> parseFilters(HttpServletRequest request) {
        return parseAllFilters(request).filters();
    }

    /**
     * Get logical conditions from request
     */
    private List<LogicalCondition> parseLogicalConditions(HttpServletRequest request) {
        return parseAllFilters(request).logicalConditions();
    }

    private Filter.FilterOperator parseFilterOperator(String op) {
        return switch (op) {
            case "eq" -> Filter.FilterOperator.EQ;
            case "neq" -> Filter.FilterOperator.NEQ;
            case "gt" -> Filter.FilterOperator.GT;
            case "gte" -> Filter.FilterOperator.GTE;
            case "lt" -> Filter.FilterOperator.LT;
            case "lte" -> Filter.FilterOperator.LTE;
            case "like" -> Filter.FilterOperator.LIKE;
            case "ilike" -> Filter.FilterOperator.ILIKE;
            case "match" -> Filter.FilterOperator.MATCH;
            case "imatch" -> Filter.FilterOperator.IMATCH;
            case "in" -> Filter.FilterOperator.IN;
            case "is" -> Filter.FilterOperator.IS;
            case "fts" -> Filter.FilterOperator.FTS;
            case "plfts" -> Filter.FilterOperator.PLFTS;
            case "phfts" -> Filter.FilterOperator.PHFTS;
            case "wfts" -> Filter.FilterOperator.WFTS;
            case "cs" -> Filter.FilterOperator.CS;
            case "cd" -> Filter.FilterOperator.CD;
            case "ov" -> Filter.FilterOperator.OV;
            case "sl" -> Filter.FilterOperator.SL;
            case "sr" -> Filter.FilterOperator.SR;
            case "nxr" -> Filter.FilterOperator.NXR;
            case "nxl" -> Filter.FilterOperator.NXL;
            case "adj" -> Filter.FilterOperator.ADJ;
            case "isdistinct" -> Filter.FilterOperator.ISDISTINCT;
            default -> Filter.FilterOperator.EQ;
        };
    }

    private List<OrderBy> parseOrderBy(String order) {
        List<OrderBy> orderByList = new ArrayList<>();
        String[] parts = order.split(",");

        for (String part : parts) {
            part = part.strip();

            OrderBy.OrderByBuilder builder = OrderBy.builder();

            // PostgREST format: column.direction.nullsplacement
            // Example: start_date.desc.nullslast
            // Must parse from right to left (nulls first, then direction)

            // 1. Check for NULLS ordering (rightmost suffix)
            if (part.endsWith(".nullsfirst")) {
                builder.nullsOrder(OrderBy.NullsOrder.FIRST);
                part = part.substring(0, part.length() - ".nullsfirst".length());
            } else if (part.endsWith(".nullslast")) {
                builder.nullsOrder(OrderBy.NullsOrder.LAST);
                part = part.substring(0, part.length() - ".nullslast".length());
            }

            // 2. Check for direction (after nulls suffix is removed)
            if (part.endsWith(".desc")) {
                builder.direction(OrderBy.Direction.DESC);
                part = part.substring(0, part.length() - ".desc".length());
            } else if (part.endsWith(".asc")) {
                builder.direction(OrderBy.Direction.ASC);
                part = part.substring(0, part.length() - ".asc".length());
            } else {
                builder.direction(OrderBy.Direction.ASC);
            }

            // 3. Remaining part is the column name
            builder.column(part.strip());
            orderByList.add(builder.build());
        }

        return orderByList;
    }

    private String readBody(HttpServletRequest request) throws IOException {
        StringBuilder buffer = new StringBuilder();
        try (BufferedReader reader = request.getReader()) {
            String line;
            while ((line = reader.readLine()) != null) {
                buffer.append(line);
            }
        }
        return buffer.toString();
    }
}
