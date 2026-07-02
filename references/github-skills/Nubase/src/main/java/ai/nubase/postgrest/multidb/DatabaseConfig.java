package ai.nubase.postgrest.multidb;

import com.zaxxer.hikari.HikariConfig;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;
import ai.nubase.postgrest.config.PostgRESTConfig;

import java.time.Instant;
import java.util.List;

/**
 * Database configuration POJO
 * Maps to rows in postgrest_metadata.database_configs table
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class DatabaseConfig {

    // Identity
    private String dbKey;
    private String dbName;
    private String description;

    // Connection Details
    private String jdbcUrl;
    private String dbUser;
    private String dbPasswordEncrypted;

    // Decrypted credentials (transient, not persisted)
    private transient String dbPasswordDecrypted;

    // PostgREST Configuration
    private List<String> dbSchemas;
    private String dbAnonRole;
    private Integer dbMaxRows;
    private List<String> dbExtraSearchPath;

    // JWT Configuration
    private String jwtSecretEncrypted;
    private Boolean jwtSecretIsBase64;
    private String jwtAudience;
    private String jwtRoleClaimKey;

    // Decrypted JWT secret (transient, not persisted)
    private transient String jwtSecretDecrypted;

    // Connection Pool Settings
    private Integer poolSize;
    private Integer poolTimeoutMs;
    private Integer poolMaxLifetimeMs;
    private Integer poolConnectionTimeoutMs;
    private Integer poolIdleTimeoutMs;
    private Integer poolMinimumIdle;

    // Status and Metadata
    private Boolean enabled;
    private Instant createdAt;
    private Instant updatedAt;
    private String createdBy;
    private String updatedBy;

    // Health Check
    private Instant lastHealthCheck;
    private String healthStatus;
    private String healthMessage;

    // Database Initialization Status
    /**
     * Database initialization status
     * - pending_init: Configuration saved, waiting for physical database initialization
     * - initializing: Physical database initialization in progress
     * - initialized: Physical database initialized successfully
     * - init_failed: Physical database initialization failed
     */
    private String initStatus;
    private String initMessage;
    private Instant initStartedAt;
    private Instant initCompletedAt;

    /**
     * Tenant unique identifier (app_code)
     * Corresponds to the 'ref' claim in JWT
     */
    private String appCode;

    /**
     * Tenant name (for display)
     */
    private String appName;
    /**
     * Schema name
     * Under database-level isolation, each database typically has one primary schema
     */
    private String schemaName;

    /**
     * JWT Secret (plaintext, used to validate API key and user token)
     */
    private String jwtSecret;

    /**
     * Service Role Key (used to identify admin-level API requests)
     */
    private String serviceRoleToken;

    private String authenticatedToken;
    /**
     * OAuth configuration (JSON format)
     */
    private String oauthConfig;

    /**
     * Per-tenant auth settings as JSON (TenantAuthConfig); overrides global nubase.auth.* config.
     * May be null → tenant uses global defaults.
     */
    private String authConfigJson;

    /**
     * Convert this DatabaseConfig to PostgRESTConfig
     * Used by SchemaCache and other components that expect PostgRESTConfig
     *
     * @return PostgRESTConfig instance
     */
    public PostgRESTConfig toPostgRESTConfig() {
        PostgRESTConfig config = new PostgRESTConfig();

        // Database settings
        config.setDbUri(jdbcUrl);
        config.setDbSchemas(dbSchemas != null ? dbSchemas : List.of("public"));
        config.setDbAnonRole(dbAnonRole != null ? dbAnonRole : "anonymous");
        config.setDbMaxRows(dbMaxRows);
        config.setDbExtraSearchPath(dbExtraSearchPath != null ? dbExtraSearchPath : List.of());

        // Connection pool settings
        config.setDbPoolSize(poolSize != null ? poolSize : 10);
        config.setDbPoolTimeout(poolTimeoutMs != null ? poolTimeoutMs / 1000 : 10);

        // JWT settings
        config.setJwtSecret(jwtSecretDecrypted);
        config.setJwtSecretIsBase64(jwtSecretIsBase64 != null ? jwtSecretIsBase64 : false);
        config.setJwtAudience(jwtAudience);
        config.setJwtRoleClaimKey(jwtRoleClaimKey != null ? jwtRoleClaimKey : ".role");

        // Other settings (use defaults)
        config.setServerHost("localhost");
        config.setServerPort(3000);
        config.setOpenApiMode("follow-privileges");
        config.setLogLevel("error");

        return config;
    }

    /**
     * Convert this DatabaseConfig to HikariConfig for connection pool
     *
     * @return HikariConfig instance
     * @throws IllegalStateException if password is not decrypted
     */
    public HikariConfig toHikariConfig() {
        if (dbPasswordDecrypted == null || dbPasswordDecrypted.isBlank()) {
            throw new IllegalStateException(
                "Database password must be decrypted before creating HikariConfig for: " + dbKey
            );
        }

        HikariConfig hikariConfig = new HikariConfig();

        // JDBC connection settings
        hikariConfig.setJdbcUrl(jdbcUrl);
        hikariConfig.setUsername(dbUser);
        hikariConfig.setPassword(dbPasswordDecrypted);

        // Pool sizing
        hikariConfig.setMaximumPoolSize(poolSize != null ? poolSize : 10);
        hikariConfig.setMinimumIdle(0);

        // Timeouts
        hikariConfig.setConnectionTimeout(poolConnectionTimeoutMs != null ? poolConnectionTimeoutMs : 30000);
        hikariConfig.setIdleTimeout(poolIdleTimeoutMs != null ? poolIdleTimeoutMs : 600000);
        hikariConfig.setMaxLifetime(poolMaxLifetimeMs != null ? poolMaxLifetimeMs : 1800000);

        // Pool name for monitoring
        hikariConfig.setPoolName("PostgrestPool-" + dbKey);

        // Performance and reliability settings
        hikariConfig.setAutoCommit(false);  // PostgREST manages transactions
        hikariConfig.setReadOnly(false);
        hikariConfig.setIsolateInternalQueries(true);

        // Connection test query
        hikariConfig.setConnectionTestQuery("SELECT 1");

        // Leak detection (useful for development)
        hikariConfig.setLeakDetectionThreshold(60000);  // 60 seconds

        // Register MBeans for monitoring
        hikariConfig.setRegisterMbeans(true);

        return hikariConfig;
    }

    /**
     * Validate the configuration
     *
     * @throws IllegalArgumentException if configuration is invalid
     */
    public void validate() {
        if (dbKey == null || dbKey.isBlank()) {
            throw new IllegalArgumentException("db_key cannot be null or blank");
        }

        if (!dbKey.matches("^[a-zA-Z0-9_-]+$")) {
            throw new IllegalArgumentException(
                "db_key must match pattern [a-zA-Z0-9_-]+, got: " + dbKey
            );
        }

        if (jdbcUrl == null || !jdbcUrl.startsWith("jdbc:postgresql://")) {
            throw new IllegalArgumentException("jdbc_url must be a valid PostgreSQL JDBC URL");
        }

        if (dbUser == null || dbUser.isBlank()) {
            throw new IllegalArgumentException("db_user cannot be null or blank");
        }

        if (dbPasswordEncrypted == null || dbPasswordEncrypted.isBlank()) {
            throw new IllegalArgumentException("db_password_encrypted cannot be null or blank");
        }

        if (dbSchemas == null || dbSchemas.isEmpty()) {
            throw new IllegalArgumentException("db_schemas cannot be null or empty");
        }

        if (poolSize != null && (poolSize < 1 || poolSize > 100)) {
            throw new IllegalArgumentException("pool_size must be between 1 and 100");
        }
    }

    /**
     * Check if this database is healthy
     *
     * @return true if health status is 'healthy'
     */
    public boolean isHealthy() {
        return "healthy".equalsIgnoreCase(healthStatus);
    }

    /**
     * Check if this database is enabled and healthy
     *
     * @return true if enabled and healthy
     */
    public boolean isAvailable() {
        return enabled;
    }

    @Override
    public String toString() {
        return "DatabaseConfig{" +
                "dbKey='" + dbKey + '\'' +
                ", dbName='" + dbName + '\'' +
                ", jdbcUrl='" + jdbcUrl + '\'' +
                ", dbUser='" + dbUser + '\'' +
                ", dbSchemas=" + dbSchemas +
                ", enabled=" + enabled +
                ", healthStatus='" + healthStatus + '\'' +
                '}';
    }
}
