package ai.nubase.common.enums;

/**
 * PostgreSQL Role Enum for Row-Level Security (RLS) and Access Control
 * <p>
 * This enum defines the three standard roles used in Supabase-compatible authentication:
 * - ANON: Anonymous/unauthenticated users
 * - AUTHENTICATED: Logged-in users
 * - SERVICE_ROLE: Admin/system-level access (bypasses RLS)
 */
public enum Role {
    /**
     * Anonymous role - for unauthenticated requests
     * This role has the most restricted permissions
     */
    ANON("anon"),

    /**
     * Authenticated role - for logged-in users
     * This role is subject to RLS policies
     */
    AUTHENTICATED("authenticated"),

    /**
     * Service role - for admin/system operations
     * This role bypasses RLS and has full access
     */
    SERVICE_ROLE("service_role");

    private final String value;

    Role(String value) {
        this.value = value;
    }

    /**
     * Get the string value of the role (e.g., "anon", "authenticated", "service_role")
     */
    public String getValue() {
        return value;
    }

    /**
     * Parse a role from string (case-insensitive)
     * Returns AUTHENTICATED as default if the role is null or unrecognized
     *
     * @param role the role string
     * @return the matching Role enum, or AUTHENTICATED as default
     */
    public static Role fromString(String role) {
        if (role == null) {
            return AUTHENTICATED;
        }
        for (Role r : values()) {
            if (r.value.equalsIgnoreCase(role)) {
                return r;
            }
        }
        return AUTHENTICATED;
    }

    /**
     * Check if this role is service_role (admin)
     */
    public boolean isServiceRole() {
        return this == SERVICE_ROLE;
    }

    /**
     * Check if this role is authenticated (logged-in user)
     */
    public boolean isAuthenticated() {
        return this == AUTHENTICATED;
    }

    /**
     * Check if this role is anon (unauthenticated)
     */
    public boolean isAnon() {
        return this == ANON;
    }

    @Override
    public String toString() {
        return value;
    }
}
