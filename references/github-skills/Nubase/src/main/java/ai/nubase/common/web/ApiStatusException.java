package ai.nubase.common.web;

import org.springframework.http.HttpStatus;

/**
 * Base for module API exceptions carrying an HTTP status and a stable error code.
 * The error envelope rendered from these ({"code", "message"}) is the contract
 * consumed by Studio and the MCP bridge — modules subclass this so the shape
 * cannot drift between features.
 */
public class ApiStatusException extends RuntimeException {

    private final HttpStatus status;
    private final String code;

    public ApiStatusException(HttpStatus status, String code, String message) {
        super(message);
        this.status = status;
        this.code = code;
    }

    public HttpStatus status() {
        return status;
    }

    public String code() {
        return code;
    }
}
