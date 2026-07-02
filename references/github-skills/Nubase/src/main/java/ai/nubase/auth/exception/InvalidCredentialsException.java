package ai.nubase.auth.exception;

public class InvalidCredentialsException extends AuthException {

    public InvalidCredentialsException(String message) {
        super("invalid_grant", message);
    }
}
