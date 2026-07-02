package ai.nubase.cron.controller;

import ai.nubase.cron.service.CronExceptions.CronException;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.util.Map;

@RestControllerAdvice(assignableTypes = ScheduledJobAdminController.class)
@ConditionalOnProperty(value = "nubase.cron.enabled", havingValue = "true", matchIfMissing = true)
public class CronExceptionHandler {

    @ExceptionHandler(CronException.class)
    public ResponseEntity<Map<String, Object>> handleCronException(CronException e) {
        return ResponseEntity.status(e.status())
                .body(Map.of("code", e.code(), "message", e.getMessage()));
    }
}
