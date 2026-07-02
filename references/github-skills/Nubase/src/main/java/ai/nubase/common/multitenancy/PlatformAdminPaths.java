package ai.nubase.common.multitenancy;

import java.util.List;

/**
 * Shared path classification for platform control-plane APIs.
 *
 * <p>These endpoints are authenticated by {@link AdminInitAuthFilter} and must not enter the
 * tenant apikey/JWT multi-tenancy chain.</p>
 */
public final class PlatformAdminPaths {

    private static final List<String> PLATFORM_ADMIN_PATHS = List.of(
            "/auth/v1/admin/init",
            "/auth/v1/admin/projects",
            "/auth/v1/admin/platform",
            "/deployments/platform/v1/app-workers"
    );

    private static final List<String> PLATFORM_JWT_ACCEPTED_PATHS = PLATFORM_ADMIN_PATHS;

    private static final List<String> TENANT_MULTITENANCY_SKIP_PATHS = List.of(
            "/auth/v1/admin/init",
            "/auth/v1/admin/projects",
            "/auth/v1/admin/platform",
            "/auth/v1/platform",
            "/deployments/platform/v1/app-workers"
    );

    private static final List<String> SUPER_ADMIN_REQUIRED_PATHS = List.of(
            "/deployments/platform/v1/app-workers"
    );

    private PlatformAdminPaths() {
    }

    public static boolean isPlatformAdminPath(String requestPath) {
        return matchesAny(PLATFORM_ADMIN_PATHS, requestPath);
    }

    public static boolean acceptsPlatformJwt(String requestPath) {
        return matchesAny(PLATFORM_JWT_ACCEPTED_PATHS, requestPath);
    }

    public static boolean skipsTenantMultitenancy(String requestPath) {
        return matchesAny(TENANT_MULTITENANCY_SKIP_PATHS, requestPath);
    }

    public static boolean requiresSuperAdmin(String requestPath) {
        return matchesAny(SUPER_ADMIN_REQUIRED_PATHS, requestPath);
    }

    private static boolean matchesAny(List<String> prefixes, String requestPath) {
        if (requestPath == null) {
            return false;
        }
        for (String prefix : prefixes) {
            if (matchesPrefix(prefix, requestPath)) {
                return true;
            }
        }
        return false;
    }

    private static boolean matchesPrefix(String prefix, String requestPath) {
        String normalized = prefix.endsWith("/")
                ? prefix.substring(0, prefix.length() - 1)
                : prefix;
        return requestPath.equals(normalized) || requestPath.startsWith(normalized + "/");
    }
}
