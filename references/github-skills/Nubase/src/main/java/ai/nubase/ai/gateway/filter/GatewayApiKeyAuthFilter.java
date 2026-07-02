package ai.nubase.ai.gateway.filter;

import ai.nubase.ai.gateway.entity.ApiKey;
import ai.nubase.ai.gateway.repository.ApiKeyRepository;
import ai.nubase.ai.gateway.util.GatewayKeyUtil;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.service.JwtSecretService;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.common.enums.DatabaseInitStatus;
import ai.nubase.common.enums.Role;
import ai.nubase.postgrest.multidb.DatabaseConfig;
import ai.nubase.postgrest.multidb.DatabaseConfigRepository;
import ai.nubase.postgrest.multidb.RoutingDataSource;
import cn.hutool.json.JSONObject;
import cn.hutool.json.JSONUtil;
import io.jsonwebtoken.security.Keys;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.core.annotation.Order;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.security.web.authentication.WebAuthenticationDetailsSource;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.time.LocalDateTime;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

/**
 * AI 网关数据面鉴权过滤器（自路由项目密钥）。
 * <p>
 * 外部 AI 客户端（Claude Code / OpenAI SDK 等）以 {@code nbk_<appCode>_<secret>} 形式的项目密钥访问，
 * 经 {@code x-api-key} 或 {@code Authorization: Bearer} 传入。本过滤器：
 * <ol>
 *   <li>从密钥前缀解析 appCode；</li>
 *   <li>{@code findByAppCode} 定位项目并设置 {@link MultiTenancyContext}（与 UnifiedMultiTenancyFilter 同款）；</li>
 *   <li>在租户上下文里按密钥哈希校验 {@code ai_gateway.api_keys}（启用 / 未撤销 / 未过期）。</li>
 * </ol>
 * 仅作用于数据面路径，且这些路径在 UnifiedMultiTenancyFilter 的 NON_FILTERED 列表内被跳过，
 * 因此租户上下文唯一来源是本过滤器。
 */
@Slf4j
@Component
@Order(0)
@RequiredArgsConstructor
public class GatewayApiKeyAuthFilter extends OncePerRequestFilter {

    private final DatabaseConfigRepository databaseConfigRepository;
    private final RoutingDataSource routingDataSource;
    private final ApiKeyRepository apiKeyRepository;
    private final JwtSecretService jwtSecretService;
    private final UserRepository userRepository;

    /** 数据面路径前缀（转发端点）。 */
    private static final List<String> DATA_PLANE_PREFIXES = List.of(
            "/v1/", "/openai", "/ai/");

    @Override
    protected void doFilterInternal(HttpServletRequest request,
                                    HttpServletResponse response,
                                    FilterChain filterChain) throws ServletException, IOException {
        String path = request.getRequestURI();
        if (!isDataPlane(path) || isHealth(path)) {
            filterChain.doFilter(request, response);
            return;
        }

        String gatewayKey = extractGatewayKey(request);
        if (gatewayKey != null) {
            authenticateGatewayKey(request, response, filterChain, gatewayKey);
            return;
        }

        String projectApiKey = extractProjectApiKey(request);
        String userBearer = extractBearer(request);
        if (projectApiKey != null && userBearer != null && !GatewayKeyUtil.isGatewayKey(userBearer)) {
            authenticateProjectUser(request, response, filterChain, projectApiKey, userBearer);
            return;
        }

        unauthorized(response, "Missing gateway API key or project apikey + user Bearer token");
    }

    private void authenticateGatewayKey(HttpServletRequest request,
                                        HttpServletResponse response,
                                        FilterChain filterChain,
                                        String key) throws IOException, ServletException {
        String appCode = GatewayKeyUtil.parseAppCode(key);
        if (appCode == null) {
            unauthorized(response, "Cannot resolve project from gateway API key");
            return;
        }

        DatabaseConfig dbConfig = databaseConfigRepository.findByAppCode(appCode);
        if (dbConfig == null || !dbConfig.isAvailable()) {
            unauthorized(response, "Invalid project or project not enabled");
            return;
        }

        try {
            // 注册并预热该项目的数据源，设置租户上下文（与 UnifiedMultiTenancyFilter 一致）
            if (DatabaseInitStatus.INITIALIZED.name().equals(dbConfig.getInitStatus())) {
                if (!routingDataSource.hasDataSource(dbConfig.getDbKey())) {
                    routingDataSource.initializeDataSource(dbConfig);
                }
                routingDataSource.recordAccess(dbConfig.getDbKey());
            }

            MultiTenancyContext.ContextData ctx = MultiTenancyContext.ContextData.builder()
                    .appCode(appCode)
                    .schemaName(dbConfig.getSchemaName())
                    .jwtSecret(dbConfig.getJwtSecret())
                    .jwtSecretKey(Keys.hmacShaKeyFor(dbConfig.getJwtSecret().getBytes(StandardCharsets.UTF_8)))
                    .databaseKey(dbConfig.getDbKey())
                    .databaseConfig(dbConfig)
                    .apikey(key)
                    .serviceRole(false)
                    .build();
            MultiTenancyContext.setContext(ctx);

            // 上下文就绪后，在该项目租户库内按哈希校验密钥
            if (!validateKey(key)) {
                unauthorized(response, "Invalid, inactive, revoked or expired gateway API key");
                return;
            }

            filterChain.doFilter(request, response);
        } catch (Exception e) {
            log.warn("Gateway data-plane auth failed for {}: {}", request.getRequestURI(), e.getMessage());
            unauthorized(response, "Gateway authentication error");
        } finally {
            SecurityContextHolder.clearContext();
            MultiTenancyContext.clear();
        }
    }

    private void authenticateProjectUser(HttpServletRequest request,
                                         HttpServletResponse response,
                                         FilterChain filterChain,
                                         String projectApiKey,
                                         String userBearer) throws IOException, ServletException {
        try {
            JSONObject jsonObject = extractAppCodeAndRole(projectApiKey);
            String appCode = jsonObject.getStr("ref");
            String role = jsonObject.getStr("role");
            if (appCode == null || appCode.isBlank() || role == null || role.isBlank()) {
                unauthorized(response, "Project apikey is missing ref or role claim");
                return;
            }

            DatabaseConfig dbConfig = databaseConfigRepository.findByAppCode(appCode);
            if (dbConfig == null || !dbConfig.isAvailable()) {
                unauthorized(response, "Invalid project or project not enabled");
                return;
            }

            try {
                Jwts.parser()
                        .setSigningKey(Keys.hmacShaKeyFor(dbConfig.getJwtSecret().getBytes(StandardCharsets.UTF_8)))
                        .build()
                        .parseClaimsJws(projectApiKey);
            } catch (Exception e) {
                unauthorized(response, "Invalid or expired project apikey");
                return;
            }

            if (DatabaseInitStatus.INITIALIZED.name().equals(dbConfig.getInitStatus())) {
                if (!routingDataSource.hasDataSource(dbConfig.getDbKey())) {
                    routingDataSource.initializeDataSource(dbConfig);
                }
                routingDataSource.recordAccess(dbConfig.getDbKey());
            }

            boolean serviceRole = Role.fromString(role).isServiceRole();
            MultiTenancyContext.ContextData ctx = MultiTenancyContext.ContextData.builder()
                    .appCode(appCode)
                    .schemaName(dbConfig.getSchemaName())
                    .jwtSecret(dbConfig.getJwtSecret())
                    .jwtSecretKey(Keys.hmacShaKeyFor(dbConfig.getJwtSecret().getBytes(StandardCharsets.UTF_8)))
                    .databaseKey(dbConfig.getDbKey())
                    .databaseConfig(dbConfig)
                    .apikey(projectApiKey)
                    .serviceRole(serviceRole)
                    .build();
            MultiTenancyContext.setContext(ctx);

            authenticateUserJwt(request, userBearer);
            filterChain.doFilter(request, response);
        } catch (Exception e) {
            log.warn("Project-user AI auth failed for {}: {}", request.getRequestURI(), e.getMessage());
            unauthorized(response, "Project-user authentication error");
        } finally {
            SecurityContextHolder.clearContext();
            MultiTenancyContext.clear();
        }
    }

    private boolean validateKey(String key) {
        String hash = GatewayKeyUtil.sha256Hex(key);
        Optional<ApiKey> match = apiKeyRepository.findByKeyHash(hash);
        if (match.isEmpty()) {
            return false;
        }
        ApiKey k = match.get();
        if (!Boolean.TRUE.equals(k.getIsActive())) {
            return false;
        }
        if (k.getRevokedAt() != null) {
            return false;
        }
        return k.getExpiresAt() == null || k.getExpiresAt().isAfter(LocalDateTime.now());
    }

    private boolean isDataPlane(String path) {
        for (String prefix : DATA_PLANE_PREFIXES) {
            if (path.startsWith(prefix)) {
                return true;
            }
        }
        return false;
    }

    private boolean isHealth(String path) {
        return path.equals("/v1/health") || path.startsWith("/v1/health");
    }

    private String extractGatewayKey(HttpServletRequest request) {
        String apiKey = request.getHeader("x-api-key");
        if (apiKey != null && !apiKey.isBlank() && GatewayKeyUtil.isGatewayKey(apiKey.trim())) {
            return apiKey.trim();
        }
        String bearer = extractBearer(request);
        if (bearer != null && GatewayKeyUtil.isGatewayKey(bearer)) {
            return bearer;
        }
        return null;
    }

    private String extractProjectApiKey(HttpServletRequest request) {
        String apikey = request.getHeader("apikey");
        if (apikey == null || apikey.isBlank()) {
            apikey = request.getHeader("Apikey");
        }
        if (apikey == null || apikey.isBlank()) {
            apikey = request.getParameter("apikey");
        }
        return apikey == null || apikey.isBlank() ? null : apikey.trim();
    }

    private String extractBearer(HttpServletRequest request) {
        String auth = request.getHeader("Authorization");
        if (auth != null && auth.startsWith("Bearer ")) {
            return auth.substring(7).trim();
        }
        return null;
    }

    private JSONObject extractAppCodeAndRole(String jwt) {
        try {
            String[] parts = jwt.split("\\.");
            if (parts.length < 2) {
                throw new IllegalArgumentException("Invalid JWT format");
            }
            String payload = cn.hutool.core.codec.Base64.decodeStr(parts[1]);
            return JSONUtil.parseObj(payload);
        } catch (Exception e) {
            throw new IllegalArgumentException("Failed to parse apikey payload: " + e.getMessage());
        }
    }

    private void authenticateUserJwt(HttpServletRequest request, String token) {
        Claims claims = jwtSecretService.validateToken(token);
        String userId = claims.getSubject();
        if (userId == null || userId.isBlank()) {
            throw new IllegalArgumentException("User token missing subject");
        }
        User user = userRepository.findById(UUID.fromString(userId))
                .orElseThrow(() -> new IllegalArgumentException("User not found"));
        String role = claims.get("role", String.class);
        SimpleGrantedAuthority authority = new SimpleGrantedAuthority(
                "ROLE_" + (role != null ? role.toUpperCase() : "USER"));
        UsernamePasswordAuthenticationToken auth = new UsernamePasswordAuthenticationToken(
                user, null, Collections.singletonList(authority));
        auth.setDetails(new WebAuthenticationDetailsSource().buildDetails(request));
        SecurityContextHolder.getContext().setAuthentication(auth);
    }

    private void unauthorized(HttpServletResponse response, String message) throws IOException {
        response.setStatus(HttpStatus.UNAUTHORIZED.value());
        response.setContentType(MediaType.APPLICATION_JSON_VALUE);
        response.getWriter().write(JSONUtil.toJsonStr(new JSONObject().set("error", message)));
    }
}
