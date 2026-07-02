package ai.nubase.assets.controller;

import ai.nubase.assets.service.AssetsExceptions.AssetsException;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.util.Map;

@RestControllerAdvice(assignableTypes = {AssetsAdminController.class, AssetsPublicController.class})
@ConditionalOnProperty(value = "nubase.assets.enabled", havingValue = "true", matchIfMissing = true)
public class AssetsExceptionHandler {

    @ExceptionHandler(AssetsException.class)
    public ResponseEntity<Map<String, Object>> handleAssetsException(AssetsException e) {
        return ResponseEntity.status(e.status())
                .body(Map.of("code", e.code(), "message", e.getMessage()));
    }
}
