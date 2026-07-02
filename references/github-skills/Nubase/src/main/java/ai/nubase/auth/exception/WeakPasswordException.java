package ai.nubase.auth.exception;

public class WeakPasswordException extends AuthException {

    public WeakPasswordException(String message) {
        super("weak_password", message);
    }
}
