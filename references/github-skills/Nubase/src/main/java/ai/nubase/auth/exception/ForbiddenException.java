package ai.nubase.auth.exception;

/**
 * Exception thrown when a user attempts to access a resource without proper authorization.
 * Typically results in HTTP 403 Forbidden response.
 */
public class ForbiddenException extends AuthException {

    public ForbiddenException(String message) {
        super("forbidden", message);
    }

    public ForbiddenException(String message, Throwable cause) {
        super("forbidden", message, cause);
    }
}
