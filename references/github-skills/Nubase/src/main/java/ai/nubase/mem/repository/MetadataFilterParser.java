package ai.nubase.mem.repository;

import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Pattern;

/**
 * Parses mem0-style metadata filters into a parameterized PostgreSQL {@code WHERE} fragment.
 *
 * <p>Input grammar (closely follows mem0 {@code _process_metadata_filters}):
 * <ul>
 *   <li>{@code {"key": "value"}} — shorthand for {@code {"key": {"eq": "value"}}}</li>
 *   <li>Per-field operators: {@code eq, ne, gt, gte, lt, lte, in, nin, contains, icontains}</li>
 *   <li>Logical groupings: {@code AND, OR, NOT} with array (AND/OR) or single child (NOT)
 *       payloads</li>
 *   <li>Comparison/numeric operators ({@code gt/gte/lt/lte}) cast the metadata field to
 *       {@code numeric} when the rhs is a number; otherwise comparison is lexicographic on text.</li>
 *   <li>{@code in/nin} accept either a list of scalars or a single scalar (which is wrapped).</li>
 * </ul>
 *
 * <p>Translation target — all access goes through {@code metadata->>'key'} jsonb accessors:
 * <pre>
 *   {"category":{"eq":"food"},"year":{"gte":2025}}
 *     → (metadata->>'category' = ? AND (metadata->>'year')::numeric &gt;= ?)
 * </pre>
 *
 * <p><b>Security:</b> field names are strictly validated against {@link #KEY_PATTERN} to
 * prevent SQL injection via the unparameterized {@code metadata->>'…'} key. Values always
 * flow through prepared-statement parameters.
 */
public final class MetadataFilterParser {

    /** Whitelist for metadata key names: alphanumerics, underscore, dot, hyphen. */
    private static final Pattern KEY_PATTERN = Pattern.compile("^[A-Za-z0-9_.\\-]{1,128}$");

    private MetadataFilterParser() {}

    /**
     * Parser output: a SQL fragment (without leading {@code AND}) and the matching ordered
     * argument list. {@link #empty()} means no filter was supplied.
     */
    @Data
    @NoArgsConstructor
    @AllArgsConstructor
    public static class Compiled {
        private String sql;
        private List<Object> args;

        public static Compiled empty() {
            return new Compiled("", List.of());
        }

        public boolean isEmpty() {
            return sql == null || sql.isBlank();
        }
    }

    /**
     * Compile a filter map. Returns {@link Compiled#empty()} for {@code null} / empty input.
     *
     * @throws IllegalArgumentException on invalid key, unknown operator, or malformed shape
     */
    public static Compiled compile(Map<String, Object> filters) {
        if (filters == null || filters.isEmpty()) {
            return Compiled.empty();
        }
        List<Object> args = new ArrayList<>();
        String sql = compileObject(filters, args);
        return new Compiled(sql, args);
    }

    @SuppressWarnings("unchecked")
    private static String compileObject(Map<String, Object> filters, List<Object> args) {
        List<String> conds = new ArrayList<>();
        // Preserve insertion order for stable / debuggable SQL.
        Map<String, Object> ordered = filters instanceof LinkedHashMap
                ? filters
                : new LinkedHashMap<>(filters);

        for (Map.Entry<String, Object> entry : ordered.entrySet()) {
            String key = entry.getKey();
            Object value = entry.getValue();
            if (key == null) continue;

            String upper = key.toUpperCase();
            if ("AND".equals(upper) || "OR".equals(upper)) {
                if (!(value instanceof Collection<?> coll)) {
                    throw new IllegalArgumentException(
                            upper + " expects an array of filter objects");
                }
                List<String> sub = new ArrayList<>();
                for (Object child : coll) {
                    if (!(child instanceof Map<?, ?> m)) {
                        throw new IllegalArgumentException(
                                upper + " children must be filter objects");
                    }
                    String childSql = compileObject((Map<String, Object>) m, args);
                    if (!childSql.isBlank()) {
                        sub.add(childSql);
                    }
                }
                if (!sub.isEmpty()) {
                    conds.add("(" + String.join(" " + upper + " ", sub) + ")");
                }
            } else if ("NOT".equals(upper)) {
                if (!(value instanceof Map<?, ?> m)) {
                    throw new IllegalArgumentException("NOT expects a single filter object");
                }
                String childSql = compileObject((Map<String, Object>) m, args);
                if (!childSql.isBlank()) {
                    conds.add("NOT (" + childSql + ")");
                }
            } else {
                if (!KEY_PATTERN.matcher(key).matches()) {
                    throw new IllegalArgumentException(
                            "Invalid metadata key: '" + key + "' (allowed: A-Z, a-z, 0-9, _.-)");
                }
                conds.add(compileLeaf(key, value, args));
            }
        }
        return String.join(" AND ", conds);
    }

    /**
     * Compile one leaf condition like {@code "category": {"eq": "food"}} or
     * {@code "category": "food"} (shorthand for eq).
     */
    @SuppressWarnings("unchecked")
    private static String compileLeaf(String key, Object value, List<Object> args) {
        if (!(value instanceof Map)) {
            // Shorthand: scalar value → equality
            args.add(String.valueOf(value));
            return "metadata->>'" + key + "' = ?";
        }
        Map<String, Object> ops = (Map<String, Object>) value;
        if (ops.isEmpty()) {
            // Vacuous condition — match-everything.
            return "TRUE";
        }
        List<String> parts = new ArrayList<>();
        for (Map.Entry<String, Object> opEntry : ops.entrySet()) {
            String op = opEntry.getKey() == null ? "" : opEntry.getKey().toLowerCase();
            Object v = opEntry.getValue();
            switch (op) {
                case "eq" -> {
                    args.add(String.valueOf(v));
                    parts.add("metadata->>'" + key + "' = ?");
                }
                case "ne" -> {
                    args.add(String.valueOf(v));
                    parts.add("metadata->>'" + key + "' <> ?");
                }
                case "gt", "gte", "lt", "lte" -> parts.add(compileNumericCompare(key, op, v, args));
                case "in" -> parts.add(compileIn(key, v, args, false));
                case "nin" -> parts.add(compileIn(key, v, args, true));
                case "contains" -> {
                    args.add("%" + escapeLike(String.valueOf(v)) + "%");
                    parts.add("metadata->>'" + key + "' LIKE ? ESCAPE '\\'");
                }
                case "icontains" -> {
                    args.add("%" + escapeLike(String.valueOf(v)) + "%");
                    parts.add("metadata->>'" + key + "' ILIKE ? ESCAPE '\\'");
                }
                default -> throw new IllegalArgumentException(
                        "Unsupported operator '" + op + "' on key '" + key + "'");
            }
        }
        if (parts.size() == 1) {
            return parts.get(0);
        }
        return "(" + String.join(" AND ", parts) + ")";
    }

    /** Regex (PG flavor) that matches anything {@code ::numeric} would accept. */
    private static final String NUMERIC_REGEX = "^-?(\\d+(\\.\\d+)?|\\.\\d+)$";

    private static String compileNumericCompare(String key, String op, Object v, List<Object> args) {
        String sqlOp = switch (op) {
            case "gt" -> ">";
            case "gte" -> ">=";
            case "lt" -> "<";
            case "lte" -> "<=";
            default -> throw new IllegalStateException(op);
        };
        if (v instanceof Number) {
            args.add(((Number) v).doubleValue());
            // Safe cast: rows whose key value is NULL or non-numeric become NULL in the CASE,
            // and NULL comparisons evaluate to UNKNOWN → row is excluded. Without this guard,
            // a single dirty row in the table would crash the whole query with
            // "invalid input syntax for type numeric".
            return "(CASE WHEN metadata->>'" + key + "' ~ '" + NUMERIC_REGEX + "' "
                    + "THEN (metadata->>'" + key + "')::numeric END) " + sqlOp + " ?";
        }
        // Lexicographic comparison when rhs is non-numeric (useful for ISO dates).
        args.add(String.valueOf(v));
        return "metadata->>'" + key + "' " + sqlOp + " ?";
    }

    /**
     * Escape PG LIKE metacharacters so user-supplied substrings match literally.
     *
     * <p>Without this, {@code contains: "100%"} compiled to {@code LIKE '%100%%'} and matched
     * anything containing "100" followed by anything — surprise for the caller. We use
     * backslash as the escape character (PG default needs explicit {@code ESCAPE '\'} clause,
     * which the callers above add).
     */
    static String escapeLike(String s) {
        if (s == null) return "";
        StringBuilder sb = new StringBuilder(s.length() + 4);
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '\\' || c == '%' || c == '_') {
                sb.append('\\');
            }
            sb.append(c);
        }
        return sb.toString();
    }

    private static String compileIn(String key, Object v, List<Object> args, boolean negate) {
        Collection<?> items;
        if (v instanceof Collection<?> c) {
            items = c;
        } else {
            // Allow a scalar — wrap it.
            items = List.of(v);
        }
        if (items.isEmpty()) {
            // Empty set: `in` matches nothing, `nin` matches everything.
            return negate ? "TRUE" : "FALSE";
        }
        List<String> placeholders = new ArrayList<>(items.size());
        for (Object item : items) {
            args.add(String.valueOf(item));
            placeholders.add("?");
        }
        String inExpr = "metadata->>'" + key + "' IN (" + String.join(",", placeholders) + ")";
        return negate ? "NOT " + inExpr : inExpr;
    }
}
