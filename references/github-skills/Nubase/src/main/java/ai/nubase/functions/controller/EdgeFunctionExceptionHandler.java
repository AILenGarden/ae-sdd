package ai.nubase.functions.controller;

import ai.nubase.functions.service.EdgeFunctionExceptions.EdgeFunctionException;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.util.Map;

@RestControllerAdvice(assignableTypes = {
        EdgeFunctionAdminController.class,
        EdgeFunctionGatewayController.class
})
@ConditionalOnProperty(value = "nubase.functions.enabled", havingValue = "true", matchIfMissing = true)
public class EdgeFunctionExceptionHandler {

    @ExceptionHandler(EdgeFunctionException.class)
    public ResponseEntity<Map<String, Object>> handleEdgeFunctionException(EdgeFunctionException e) {
        return ResponseEntity.status(e.status())
                .body(Map.of("code", e.code(), "message", e.getMessage()));
    }
}
