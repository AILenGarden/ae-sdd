package ai.nubase.common.context;

import ai.nubase.common.config.TenantAuthConfig;
import ai.nubase.common.config.oauth.OAuthProperties;
import ai.nubase.postgrest.multidb.DatabaseConfig;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;
import lombok.extern.slf4j.Slf4j;

import javax.crypto.SecretKey;

/**
 * Unified multi-tenancy context.
 * <p>
 * Merges the functionality of the original TenantContext and DatabaseContext,
 * supporting both Schema and Database level isolation.
 * <p>
 * Stored in a ThreadLocal to guarantee thread safety.
 * <p>
 * <strong>Important:</strong> {@link #clear()} must be called after each request
 * to clean up the ThreadLocal and prevent memory leaks and thread pollution.
 *
 * @author nubase
 * @since 2025-01-02
 */
@Slf4j
public class MultiTenancyContext {

    private static final ThreadLocal<ContextData> CONTEXT = new ThreadLocal<>();

    /**
     * Sets the context data.
     *
     * @param contextData context data
     */
    public static void setContext(ContextData contextData) {
        if (contextData == null) {
            throw new IllegalArgumentException("Context data cannot be null");
        }
        CONTEXT.set(contextData);
        log.trace("MultiTenancyContext set: appCode={}, databaseKey={}, schema={}",
                contextData.getAppCode(),
                contextData.getDatabaseKey(),
                contextData.getSchemaName());
    }

    /**
     * Returns the context data.
     *
     * @return the context data, or null if not set
     */
    public static ContextData getContext() {
        return CONTEXT.get();
    }

    /**
     * Returns the context data, throwing if it has not been set.
     *
     * @return the context data
     * @throws IllegalStateException if the context is not set
     */
    public static ContextData getContextOrThrow() {
        ContextData context = CONTEXT.get();
        if (context == null) {
            throw new IllegalStateException(
                    "MultiTenancyContext is not set for current request. " +
                    "This usually means the request bypassed the multi-tenancy filter."
            );
        }
        return context;
    }

    /**
     * Checks whether the context has been set.
     *
     * @return true if the context has been set
     */
    public static boolean hasContext() {
        return CONTEXT.get() != null;
    }

    // ==================== Convenience accessors (backward compatible) ====================

    /**
     * Returns the tenant identifier (app_code).
     */
    public static String getAppCode() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getAppCode() : null;
    }


    /**
     * Returns the app_name.
     */
    public static String getAppName() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getAppName() : null;
    }

    /**
     * Returns the database identifier (database_key).
     * <p>
     * Used with database-level isolation.
     */
    public static String getDatabaseKey() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getDatabaseKey() : null;
    }

    /**
     * Returns the database configuration.
     * <p>
     * Used with database-level isolation.
     */
    public static DatabaseConfig getDatabaseConfig() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getDatabaseConfig() : null;
    }

    /**
     * Returns the schema name.
     * <p>
     * Used for schema-level isolation, and may also be needed for database-level isolation.
     */
    public static String getSchemaName() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getSchemaName() : "public";
    }

    /**
     * Returns the JWT secret (string form).
     */
    public static String getJwtSecret() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getJwtSecret() : null;
    }

    /**
     * Returns the JWT secret, throwing if it is not set.
     */
    public static String getJwtSecretOrThrow() {
        String jwtSecret = getJwtSecret();
        if (jwtSecret == null || jwtSecret.isBlank()) {
            throw new IllegalStateException("JWT secret is missing in tenant context");
        }
        return jwtSecret;
    }

    /**
     * Returns the JWT secret key (SecretKey form).
     */
    public static SecretKey getJwtSecretKey() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getJwtSecretKey() : null;
    }

    /**
     * Returns the API key.
     */
    public static String getApiKey() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getApikey() : null;
    }

    /**
     * Checks whether the request is acting as the service role.
     */
    public static boolean isServiceRole() {
        ContextData context = CONTEXT.get();
        return context != null && context.isServiceRole();
    }

    /**
     * Returns the OAuth configuration.
     */
    public static OAuthProperties getOAuthProperties() {
        ContextData context = CONTEXT.get();
        if(context == null) {
            return null;
        }
        return context.getOauthProperties();
    }

    /**
     * Returns the per-tenant auth settings override (may be null → use global defaults).
     */
    public static TenantAuthConfig getTenantAuthConfig() {
        ContextData context = CONTEXT.get();
        return context != null ? context.getTenantAuthConfig() : null;
    }

    /**
     * Clears the context.
     * <p>
     * <strong>Must</strong> be called after each request to prevent memory leaks.
     */
    public static void clear() {
        ContextData context = CONTEXT.get();
        if (context != null) {
            log.trace("Clearing MultiTenancyContext: appCode={}, databaseKey={}, schema={}",
                    context.getAppCode(),
                    context.getDatabaseKey(),
                    context.getSchemaName());
        }
        CONTEXT.remove();
    }

    /**
     * Forcibly clears the context (even when an exception occurs).
     * <p>
     * Intended for exception-recovery scenarios.
     */
    public static void forceClear() {
        try {
            CONTEXT.remove();
        } catch (Exception e) {
            log.warn("Error clearing MultiTenancyContext ThreadLocal", e);
        }
    }

    // ==================== Context data class ====================

    /**
     * Context data.
     * <p>
     * Contains all tenant configuration information.
     */
    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class ContextData {

        // ==================== Common fields ====================

        /**
         * Tenant identifier (app_code).
         * <p>
         * Corresponds to the 'ref' claim in the JWT.
         */
        private String appCode;


        /**
         * appName
         */
        private String appName;

        /**
         * Schema name.
         * <p>
         * - Schema mode: used for SET search_path.
         * - Database mode: may also be needed inside the database.
         */
        private String schemaName;

        /**
         * JWT secret (string form).
         * <p>
         * Used to validate user tokens.
         */
        private String jwtSecret;

        /**
         * JWT secret key (SecretKey form).
         * <p>
         * Used by the JJWT library.
         */
        private SecretKey jwtSecretKey;

        // ==================== Database-level isolation fields ====================

        /**
         * Database identifier (database_key).
         * <p>
         * Only used with database-level isolation; corresponds to database_configs.db_key.
         */
        private String databaseKey;

        /**
         * Database configuration.
         * <p>
         * Only used with database-level isolation.
         */
        private DatabaseConfig databaseConfig;

        // ==================== Auth-related fields ====================

        /**
         * API key (JWT).
         * <p>
         * The apikey supplied by the client on the request.
         */
        private String apikey;

        /**
         * Whether the caller is acting as the service role.
         * <p>
         * The service role has administrator privileges.
         */
        private boolean serviceRole;

        /**
         * OAuth configuration.
         * <p>
         * Contains third-party login configuration such as Google and GitHub.
         */
        private OAuthProperties oauthProperties;

        /**
         * Per-tenant auth settings override (parsed from database_configs.auth_config).
         * <p>
         * Null when the tenant has no override → global {@link ai.nubase.common.config.AuthConfig}
         * defaults apply. Resolved via {@link ai.nubase.auth.service.EffectiveAuthConfig}.
         */
        private TenantAuthConfig tenantAuthConfig;

        // ==================== Helper methods ====================

        /**
         * Validates the context data.
         *
         * @throws IllegalArgumentException if the data is invalid
         */
        public void validate() {
            if (appCode == null || appCode.isBlank()) {
                throw new IllegalArgumentException("appCode cannot be null or blank");
            }

            if (schemaName == null || schemaName.isBlank()) {
                throw new IllegalArgumentException("schemaName cannot be null or blank");
            }

            if (jwtSecret == null || jwtSecret.isBlank()) {
                throw new IllegalArgumentException("jwtSecret cannot be null or blank");
            }
        }

        /**
         * Checks whether this is database-level isolation.
         *
         * @return true if databaseKey is set
         */
        public boolean isDatabaseIsolation() {
            return databaseKey != null && !databaseKey.isBlank();
        }

        /**
         * Checks whether this is schema-level isolation.
         *
         * @return true if databaseKey is not set
         */
        public boolean isSchemaIsolation() {
            return !isDatabaseIsolation();
        }

        @Override
        public String toString() {
            return "ContextData{" +
                    "appCode='" + appCode + '\'' +
                    ", databaseKey='" + databaseKey + '\'' +
                    ", schemaName='" + schemaName + '\'' +
                    ", serviceRole=" + serviceRole +
                    ", isolationMode=" + (isDatabaseIsolation() ? "DATABASE" : "SCHEMA") +
                    '}';
        }
    }
}
