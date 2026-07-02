package ai.nubase.common.multitenancy;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

/**
 * Unified tenant configuration.
 * Maps app_code (API key) to a specific database and schema.
 * Supports database-level multi-tenancy isolation.
 *
 * @author nubase
 * @since 2025-01-02
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class UnifiedTenantConfig {

    /**
     * Tenant unique identifier (app_code).
     * Corresponds to the 'ref' claim in the JWT.
     */
    private String appCode;

    /**
     * Tenant name (for display).
     */
    private String appName;

    /**
     * Database identifier (corresponds to DatabaseConfig.db_key).
     * Used to route to the correct database connection.
     */
    private String databaseKey;

    /**
     * Schema name.
     * Under database-level isolation, each database typically has one primary schema.
     */
    private String schemaName;

    /**
     * JWT secret (plaintext, used to validate the API key and user token).
     */
    private String jwtSecret;

    /**
     * Service role key (used to identify administrator-level API requests).
     */
    private String serviceRoleKey;

    /**
     * OAuth configuration (JSON format).
     */
    private String oauthConfig;

    /**
     * Whether the tenant is enabled.
     */
    private Boolean enabled;

    /**
     * Creation timestamp.
     */
    private Instant createdAt;

    /**
     * Last update timestamp.
     */
    private Instant updatedAt;

    /**
     * Creator.
     */
    private String createdBy;

    /**
     * Updater.
     */
    private String updatedBy;

    /**
     * Validates the configuration.
     */
    public void validate() {
        if (appCode == null || appCode.isBlank()) {
            throw new IllegalArgumentException("app_code cannot be null or blank");
        }

        if (!appCode.matches("^[a-zA-Z0-9_-]+$")) {
            throw new IllegalArgumentException(
                    "app_code must match pattern [a-zA-Z0-9_-]+, got: " + appCode
            );
        }

        if (databaseKey == null || databaseKey.isBlank()) {
            throw new IllegalArgumentException("database_key cannot be null or blank");
        }

        if (schemaName == null || schemaName.isBlank()) {
            throw new IllegalArgumentException("schema_name cannot be null or blank");
        }

        if (jwtSecret == null || jwtSecret.isBlank()) {
            throw new IllegalArgumentException("jwt_secret cannot be null or blank");
        }

        if (jwtSecret.length() < 32) {
            throw new IllegalArgumentException("jwt_secret must be at least 32 characters");
        }
    }

    /**
     * Checks whether this configuration is available (enabled).
     */
    public boolean isAvailable() {
        return Boolean.TRUE.equals(enabled);
    }

    @Override
    public String toString() {
        return "UnifiedTenantConfig{" +
                "appCode='" + appCode + '\'' +
                ", appName='" + appName + '\'' +
                ", databaseKey='" + databaseKey + '\'' +
                ", schemaName='" + schemaName + '\'' +
                ", enabled=" + enabled +
                '}';
    }
}
