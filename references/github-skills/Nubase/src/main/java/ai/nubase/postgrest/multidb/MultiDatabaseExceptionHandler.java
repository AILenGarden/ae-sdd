package ai.nubase.postgrest.multidb;

import ai.nubase.auth.controller.PostgrestController;
import ai.nubase.postgrest.multidb.exception.DatabaseNotAvailableException;
import ai.nubase.postgrest.multidb.exception.DatabaseNotFoundException;
import ai.nubase.postgrest.multidb.exception.InvalidDatabaseKeyException;
import lombok.extern.slf4j.Slf4j;
import org.springframework.core.NestedExceptionUtils;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

/**
 * Global exception handler for multi-database errors
 */
@Slf4j
@RestControllerAdvice
public class MultiDatabaseExceptionHandler {

    @ExceptionHandler(DatabaseNotFoundException.class)
    public ResponseEntity<PostgrestController.ErrorResponse> handleDatabaseNotFound(
            DatabaseNotFoundException ex) {

        log.warn("Database not found: {}", ex.getDatabaseKey());

        return ResponseEntity
                .status(HttpStatus.NOT_FOUND)
                .body(new PostgrestController.ErrorResponse(
                        "Database not found: " + ex.getDatabaseKey()
                ));
    }

    @ExceptionHandler(DatabaseNotAvailableException.class)
    public ResponseEntity<PostgrestController.ErrorResponse> handleDatabaseNotAvailable(
            DatabaseNotAvailableException ex) {

        log.error("Database not available: {}", ex.getDatabaseKey(), ex);

        return ResponseEntity
                .status(HttpStatus.SERVICE_UNAVAILABLE)
                .body(new PostgrestController.ErrorResponse(
                        "Database temporarily unavailable. Please try again later." + " (" + ex.getDatabaseKey() + ")"
                ));
    }

    @ExceptionHandler(InvalidDatabaseKeyException.class)
    public ResponseEntity<PostgrestController.ErrorResponse> handleInvalidDatabaseKey(
            InvalidDatabaseKeyException ex) {

        log.warn("Invalid database key: {}", ex.getDatabaseKey());

        return ResponseEntity
                .status(HttpStatus.BAD_REQUEST)
                .body(new PostgrestController.ErrorResponse(
                        "Invalid database key format " + ex.getDatabaseKey()
                ));
    }

    @ExceptionHandler(IllegalStateException.class)
    public ResponseEntity<PostgrestController.ErrorResponse> handleIllegalState(
            IllegalStateException ex) {
        log.error("BadSqlGrammarException state error", ex);
        return ResponseEntity
                .status(HttpStatus.INTERNAL_SERVER_ERROR)
                .body(new PostgrestController.ErrorResponse(
                        NestedExceptionUtils.getMostSpecificCause(ex).getMessage()
                ));
    }

    @ExceptionHandler(RuntimeException.class)
    public ResponseEntity<PostgrestController.ErrorResponse> handleRuntimeException(
            RuntimeException ex) {
        log.error("BadSqlGrammarException state error", ex);
        return ResponseEntity
                .status(HttpStatus.INTERNAL_SERVER_ERROR)
                .body(new PostgrestController.ErrorResponse(
                        NestedExceptionUtils.getMostSpecificCause(ex).getMessage()
                ));
    }
}
