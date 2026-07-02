package ai.nubase.auth.exception;

import ai.nubase.auth.controller.storage.StorageBucketController;
import ai.nubase.auth.controller.storage.StorageCdnController;
import ai.nubase.auth.controller.storage.StorageHealthController;
import ai.nubase.auth.controller.storage.StorageObjectController;
import ai.nubase.auth.controller.storage.StorageObjectReadController;
import ai.nubase.auth.controller.storage.StorageRenderController;
import ai.nubase.auth.controller.storage.StorageTusController;
import ai.nubase.auth.controller.storage.StorageVectorController;
import lombok.extern.slf4j.Slf4j;
import org.springframework.core.Ordered;
import org.springframework.core.annotation.Order;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.validation.FieldError;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.util.Map;
import java.util.stream.Collectors;

/**
 * Error response adapter dedicated to storage endpoints.
 *
 * Supabase Storage error responses follow this format:
 * {"statusCode":"400","error":"Invalid Request","message":"..."}
 */
@Slf4j
@Order(Ordered.HIGHEST_PRECEDENCE)
@RestControllerAdvice(assignableTypes = {
        StorageBucketController.class,
        StorageObjectController.class,
        StorageObjectReadController.class,
        StorageRenderController.class,
        StorageCdnController.class,
        StorageHealthController.class,
        StorageTusController.class,
        StorageVectorController.class
})
public class StorageExceptionHandler {

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ResponseEntity<Map<String, String>> handleValidationErrors(MethodArgumentNotValidException ex) {
        String message = ex.getBindingResult().getFieldErrors().stream()
                .map(FieldError::getDefaultMessage)
                .collect(Collectors.joining(", "));

        return build(HttpStatus.BAD_REQUEST, "Invalid Request", message);
    }

    @ExceptionHandler(IllegalArgumentException.class)
    public ResponseEntity<Map<String, String>> handleIllegalArgument(IllegalArgumentException ex) {
        String message = ex.getMessage() == null ? "Invalid request" : ex.getMessage();
        String lower = message.toLowerCase();

        if (lower.contains("already exists") || lower.contains("duplicate")) {
            return build(HttpStatus.CONFLICT, "Duplicate", message);
        }
        if (lower.contains("file size exceeds") || lower.contains("payload too large")) {
            return build(HttpStatus.PAYLOAD_TOO_LARGE, "Payload Too Large", message);
        }
        if (lower.contains("forbidden") || lower.contains("access denied")) {
            return build(HttpStatus.FORBIDDEN, "Forbidden", message);
        }
        if (lower.contains("bucket not found") || lower.contains("invalid bucket")) {
            return build(HttpStatus.BAD_REQUEST, "Invalid bucket", message);
        }

        return build(HttpStatus.BAD_REQUEST, "Invalid Request", message);
    }

    @ExceptionHandler(Exception.class)
    public ResponseEntity<Map<String, String>> handleException(Exception ex) {
        log.error("Storage endpoint error", ex);
        return build(HttpStatus.INTERNAL_SERVER_ERROR, "Internal", "Internal Server Error");
    }

    private ResponseEntity<Map<String, String>> build(HttpStatus status, String error, String message) {
        return ResponseEntity.status(status).body(Map.of(
                "statusCode", String.valueOf(status.value()),
                "error", error,
                "message", message
        ));
    }
}
