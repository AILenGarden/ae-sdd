package ai.nubase.auth.exception;

import lombok.Getter;

@Getter
public class AuthException extends RuntimeException {

    private final String errorCode;

    public AuthException(String errorCode, String message) {
        super(message);
        this.errorCode = errorCode;
    }

    public AuthException(String errorCode, String message, Throwable cause) {
        super(message, cause);
        this.errorCode = errorCode;
    }
}
