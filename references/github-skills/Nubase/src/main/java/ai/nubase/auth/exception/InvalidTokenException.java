package ai.nubase.auth.exception;

public class InvalidTokenException extends AuthException {

    public InvalidTokenException(String message) {
        super("invalid_token", message);
    }
}
