package ai.nubase.auth.exception;

public class UserNotFoundException extends AuthException {

    public UserNotFoundException(String message) {
        super("user_not_found", message);
    }
}
