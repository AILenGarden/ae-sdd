package ai.nubase.auth.util;

import ai.nubase.auth.entity.User;
import ai.nubase.common.context.MultiTenancyContext;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;

import java.util.UUID;

/**
 * Security utility - retrieves the current request's user information from the SecurityContext.
 * <p>
 * UnifiedMultiTenancyFilter has already verified the JWT and written the user into the
 * SecurityContext, so this utility simply reads it without re-verifying.
 */
@Component
public class SecurityUtil {

    public User getCurrentUser() {
        Authentication authentication = SecurityContextHolder.getContext().getAuthentication();
        if (authentication != null && authentication.isAuthenticated()
                && authentication.getPrincipal() instanceof User user) {
            return user;
        }
        return null;
    }

    public User getCurrentUserOrThrow() {
        User user = getCurrentUser();
        if (user == null) {
            throw new IllegalStateException("User is not authenticated");
        }
        return user;
    }

    public UUID getCurrentUserId() {
        User user = getCurrentUser();
        return user != null ? user.getId() : null;
    }

    public UUID getCurrentUserIdOrThrow() {
        return getCurrentUserOrThrow().getId();
    }

    public boolean isAuthenticated() {
        return getCurrentUser() != null;
    }

    public boolean isCurrentUser(UUID userId) {
        UUID currentUserId = getCurrentUserId();
        return currentUserId != null && currentUserId.equals(userId);
    }

    /**
     * Require that the current request is authenticated as a user (via Bearer token) or
     * uses a service_role apikey. Used by authenticated routes: anon keys are rejected,
     * while service_role keys or logged-in users are accepted.
     */
    public void requireAuthenticatedOrServiceRole() {
        if (!isAuthenticated() && !MultiTenancyContext.isServiceRole()) {
            throw new IllegalStateException("Authentication required");
        }
    }
}
