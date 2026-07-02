package ai.nubase.postgrest.auth;

import ai.nubase.common.context.MultiTenancyContext;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.jsonwebtoken.Claims;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Component;

import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Applies the transaction-local PostgreSQL role and request GUCs that PostgREST
 * functions expect. Shared by HTTP /rest/v1 and background cron RPC execution.
 */
@Slf4j
@Component
@RequiredArgsConstructor
public class PostgrestRequestContext {

    private final JdbcTemplate jdbcTemplate;
    private final ObjectMapper objectMapper;

    public void apply(String role, HttpServletRequest request, Claims claims) {
        setDatabaseRole(role);
        setRequestContext(request, claims);
    }

    public void applySynthetic(String role, String path, String method) {
        setDatabaseRole(role);
        Map<String, String> gucs = new LinkedHashMap<>();
        gucs.put("request.jwt.claims", "{}");
        gucs.put("request.headers", "{}");
        gucs.put("request.cookies", "{}");
        gucs.put("request.path", path == null ? "" : path);
        gucs.put("request.method", method == null ? "POST" : method);
        setGucVariables(gucs);
    }

    public void resetDatabaseRole() {
        try {
            jdbcTemplate.execute("RESET ROLE");
            log.debug("Reset database role");
        } catch (Exception e) {
            log.warn("Failed to reset database role: {}", e.getMessage());
        }
    }

    private void setDatabaseRole(String role) {
        // The tenant connection runs as the table OWNER (db_user), which bypasses RLS.
        // Fail closed if the requested role cannot be established.
        if (role == null || !role.matches(ai.nubase.common.util.IdentifierPatterns.SQL_IDENTIFIER)) {
            throw new IllegalStateException("Refusing to set an invalid database role: " + role);
        }
        try {
            jdbcTemplate.execute(String.format("SET LOCAL ROLE %s", quote(role)));
            log.debug("Set database role to: {}", role);
        } catch (Exception e) {
            log.error("Failed to set database role {} — aborting request to avoid an RLS bypass: {}",
                    role, e.getMessage());
            throw new IllegalStateException("Could not establish the database role for this request", e);
        }
    }

    private void setRequestContext(HttpServletRequest request, Claims claims) {
        try {
            Map<String, String> gucs = new LinkedHashMap<>();
            if (claims != null) {
                gucs.put("request.jwt.claims", objectMapper.writeValueAsString(claims));
            }

            Map<String, String> headers = new HashMap<>();
            java.util.Enumeration<String> headerNames = request.getHeaderNames();
            while (headerNames.hasMoreElements()) {
                String headerName = headerNames.nextElement();
                headers.put(headerName.toLowerCase(), request.getHeader(headerName));
            }
            gucs.put("request.headers", objectMapper.writeValueAsString(headers));

            Map<String, String> cookies = new HashMap<>();
            if (request.getCookies() != null) {
                for (jakarta.servlet.http.Cookie cookie : request.getCookies()) {
                    cookies.put(cookie.getName(), cookie.getValue());
                }
            }
            gucs.put("request.cookies", objectMapper.writeValueAsString(cookies));
            gucs.put("request.path", request.getRequestURI());
            gucs.put("request.method", request.getMethod());
            setGucVariables(gucs);
        } catch (Exception e) {
            log.warn("Failed to set request context: {}", e.getMessage());
        }
    }

    // All GUCs go in ONE statement: this runs on every /rest/v1 request and every
    // cron db_function run, and five separate round-trips of constant-shaped
    // set_config chatter is pure waste. Keys are internal constants; values are
    // quote-escaped literals.
    private void setGucVariables(Map<String, String> gucs) {
        if (gucs.isEmpty()) return;
        StringBuilder sql = new StringBuilder("SELECT ");
        boolean first = true;
        for (Map.Entry<String, String> entry : gucs.entrySet()) {
            if (!first) sql.append(", ");
            first = false;
            sql.append("set_config('").append(entry.getKey()).append("', '")
                    .append(entry.getValue().replace("'", "''")).append("', true)");
        }
        try {
            jdbcTemplate.execute(sql.toString());
        } catch (Exception e) {
            log.warn("Failed to set GUC variables {}: {}", gucs.keySet(), e.getMessage());
        }
    }

    private String quote(String identifier) {
        return "\"" + identifier.replace("\"", "\"\"") + "\"";
    }
}
