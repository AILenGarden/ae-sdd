package ai.nubase.auth.annotation;

import java.lang.annotation.*;

/**
 * Annotation to mark methods or classes that require service_role key authentication.
 * Only requests with valid service_role key will be allowed to access these endpoints.
 *
 * Usage:
 * @RequireServiceRole
 * public class AdminController { ... }
 *
 * or
 *
 * @RequireServiceRole
 * public ResponseEntity<User> deleteUser(@PathVariable String userId) { ... }
 */
@Target({ElementType.METHOD, ElementType.TYPE})
@Retention(RetentionPolicy.RUNTIME)
@Documented
public @interface RequireServiceRole {
    /**
     * Custom error message when service role check fails
     * @return error message
     */
    String message() default "Service role key required";
}
