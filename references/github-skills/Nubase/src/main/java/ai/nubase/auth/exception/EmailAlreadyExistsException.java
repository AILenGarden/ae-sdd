package ai.nubase.auth.exception;

public class EmailAlreadyExistsException extends AuthException {

    public EmailAlreadyExistsException(String message) {
        super("user_already_exists", message);
    }
}
