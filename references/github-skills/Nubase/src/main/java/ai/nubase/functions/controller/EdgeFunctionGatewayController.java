package ai.nubase.functions.controller;

import ai.nubase.auth.entity.User;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.common.util.RequestUtil;
import ai.nubase.functions.executor.EdgeFunctionExecutorProperties;
import ai.nubase.functions.executor.EdgeFunctionInvocationResponse;
import ai.nubase.functions.service.EdgeFunctionExceptions.EdgeFunctionException;
import ai.nubase.functions.service.EdgeFunctionInvocationCommand;
import ai.nubase.functions.service.EdgeFunctionInvocationService;
import ai.nubase.functions.service.HeaderSanitizer;
import ai.nubase.functions.util.EdgeFunctionHeaders;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.util.StringUtils;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.servlet.HandlerMapping;

import java.io.IOException;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;

@RestController
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.functions.enabled", havingValue = "true", matchIfMissing = true)
public class EdgeFunctionGatewayController {

    private final EdgeFunctionInvocationService invocationService;
    private final EdgeFunctionExecutorProperties properties;
    private final HeaderSanitizer headerSanitizer;

    // EdgeFunctionException is rendered by EdgeFunctionExceptionHandler — no
    // error-shaping here, so the JSON contract has a single owner.
    // The slug comes from Spring's own pattern binding rather than hand-parsing
    // request.getRequestURI(): the URI form breaks under a servlet context-path
    // (the hard-coded prefix no longer matches) and disagrees with the mapping
    // on percent-encoded paths.
    @RequestMapping("/functions/v1/{functionSlug}/**")
    public ResponseEntity<byte[]> invoke(
            @org.springframework.web.bind.annotation.PathVariable("functionSlug") String functionSlug,
            HttpServletRequest request
    ) throws IOException {
        String suffix = extractSuffix(request, functionSlug);
        byte[] body = readBody(request);
        EdgeFunctionInvocationResponse response = invocationService.invoke(functionSlug,
                new EdgeFunctionInvocationCommand(
                        requestId(request),
                        request.getMethod(),
                        suffix,
                        request.getQueryString(),
                        headerSanitizer.forwardableHeaders(request),
                        body,
                        callerRole(),
                        currentUserId()
                ));
        return toResponseEntity(response);
    }

    private byte[] readBody(HttpServletRequest request) throws IOException {
        try {
            // Streams with an early size-limit abort instead of buffering an
            // arbitrarily large body before checking it.
            return RequestUtil.readRawRequestBody(request, properties.getMaxRequestBytes());
        } catch (IllegalArgumentException e) {
            throw new EdgeFunctionException(HttpStatus.PAYLOAD_TOO_LARGE, "REQUEST_TOO_LARGE", "Function request body is too large");
        }
    }

    private ResponseEntity<byte[]> toResponseEntity(EdgeFunctionInvocationResponse response) {
        HttpHeaders headers = new HttpHeaders();
        for (Map.Entry<String, List<String>> entry : response.headers().entrySet()) {
            String lower = entry.getKey().toLowerCase(Locale.ROOT);
            if (EdgeFunctionHeaders.RESPONSE_BLOCKED.contains(lower)) continue;
            headers.put(entry.getKey(), entry.getValue());
        }
        HttpStatus status = HttpStatus.resolve(response.statusCode());
        return new ResponseEntity<>(response.body(), headers, status == null ? HttpStatus.BAD_GATEWAY : status);
    }

    private String extractSuffix(HttpServletRequest request, String functionSlug) {
        Object best = request.getAttribute(HandlerMapping.PATH_WITHIN_HANDLER_MAPPING_ATTRIBUTE);
        String path = best == null ? request.getRequestURI() : best.toString();
        String prefix = "/functions/v1/" + functionSlug;
        if (!path.startsWith(prefix)) return "";
        String suffix = path.substring(prefix.length());
        return suffix.startsWith("/") ? suffix : "";
    }

    private String requestId(HttpServletRequest request) {
        String requestId = request.getHeader("x-request-id");
        return StringUtils.hasText(requestId) ? requestId : UUID.randomUUID().toString();
    }

    private String callerRole() {
        if (MultiTenancyContext.isServiceRole()) return EdgeFunctionInvocationCommand.ROLE_SERVICE;
        if (currentUserId() != null) return EdgeFunctionInvocationCommand.ROLE_AUTHENTICATED;
        return EdgeFunctionInvocationCommand.ROLE_ANON;
    }

    private UUID currentUserId() {
        Authentication auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth != null && auth.isAuthenticated() && auth.getPrincipal() instanceof User user) {
            return user.getId();
        }
        return null;
    }
}
