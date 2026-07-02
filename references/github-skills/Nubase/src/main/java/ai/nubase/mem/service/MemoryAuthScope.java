package ai.nubase.mem.service;

import ai.nubase.auth.entity.User;
import ai.nubase.auth.exception.ForbiddenException;
import ai.nubase.common.context.MultiTenancyContext;
import lombok.AllArgsConstructor;
import lombok.Getter;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.UUID;

/**
 * Authorization scope for a single memory API call.
 *
 * <p>Resolves the effective {@code (userId, agentId, runId)} that the request is allowed to
 * touch, based on the runtime authentication state:
 *
 * <ul>
 *   <li><b>service_role</b> (admin apikey): scope is whatever the caller asked for. {@code null}
 *       userId means "every user" — admin can search across users for support / migration.</li>
 *   <li><b>authenticated user</b> (Bearer token over an authenticated apikey): {@code userId} is
 *       <em>forced</em> to the JWT {@code sub}. If the caller passed a different {@code userId}
 *       in the request body, we throw {@link ForbiddenException} (no silent override — that hides
 *       client bugs).</li>
 *   <li><b>neither</b> (apikey-only with anon/authenticated role but no Bearer): the call is
 *       rejected with {@link ForbiddenException}. We can't trust whatever {@code userId} the
 *       body contains.</li>
 * </ul>
 *
 * <p>The class is the single source of truth — every {@code MemoryService} entry point goes
 * through {@link #fromContext} so authorization can never be skipped by accident.
 */
@Getter
@AllArgsConstructor
public final class MemoryAuthScope {

    /** True if the caller authenticated with a service_role apikey. */
    private final boolean serviceRole;

    /**
     * Effective user id for this call. For service_role calls, this is whatever the client
     * asked for (or {@code null} = no restriction). For authenticated users, this is the
     * JWT {@code sub}, regardless of what the client sent.
     */
    private final UUID userId;

    private final String agentId;
    private final String runId;

    /**
     * Build the scope for the current request.
     *
     * @param requestedUserId  what the client passed in body/query (may be null)
     * @param agentId          forwarded as-is
     * @param runId            forwarded as-is
     * @throws ForbiddenException if a non-service-role caller has no JWT principal, or tried
     *                            to spoof another user's id
     */
    public static MemoryAuthScope fromContext(UUID requestedUserId, String agentId, String runId) {
        boolean isService = MultiTenancyContext.isServiceRole();
        if (isService) {
            return new MemoryAuthScope(true, requestedUserId, agentId, runId);
        }

        UUID jwtUserId = currentJwtUserId();
        if (jwtUserId == null) {
            // No Bearer token, or token didn't resolve to a known User — refuse to make
            // authorization decisions based on body input.
            throw new ForbiddenException(
                    "Memory API requires an authenticated user (Bearer token) or service_role apikey");
        }
        if (requestedUserId != null && !requestedUserId.equals(jwtUserId)) {
            throw new ForbiddenException(
                    "Cannot access memories for another user");
        }
        return new MemoryAuthScope(false, jwtUserId, agentId, runId);
    }

    /**
     * Build a scope that requires {@link #userId} to be non-null afterwards. Used by batch
     * operations (delete-all, list) where "every user" is only ever legal for service_role.
     */
    public static MemoryAuthScope fromContextRequireOwner(UUID requestedUserId,
                                                          String agentId,
                                                          String runId) {
        MemoryAuthScope scope = fromContext(requestedUserId, agentId, runId);
        if (scope.userId == null && (agentId == null || agentId.isBlank())
                && (runId == null || runId.isBlank())) {
            throw new IllegalArgumentException(
                    "At least one of userId / agentId / runId must be provided");
        }
        return scope;
    }

    /** Extract the User principal's id from Spring Security, or {@code null} if missing. */
    private static UUID currentJwtUserId() {
        Authentication auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null || !auth.isAuthenticated()) {
            return null;
        }
        Object principal = auth.getPrincipal();
        if (principal instanceof User u) {
            return u.getId();
        }
        return null;
    }

    /**
     * True when this scope should be allowed to read/write across the entire tenant,
     * i.e. {@code service_role} caller who did not pin a specific {@code userId}.
     */
    public boolean isUnrestricted() {
        return serviceRole && userId == null;
    }

    /**
     * True when the calling user is allowed to operate on a memory row owned by
     * {@code rowUserId}. {@code service_role} can touch anything. Regular users can only
     * touch rows where {@code rowUserId} equals their JWT {@code sub}.
     */
    public boolean canAccess(UUID rowUserId) {
        if (serviceRole) {
            return userId == null || userId.equals(rowUserId);
        }
        return userId != null && userId.equals(rowUserId);
    }
}
