package ai.nubase.postgrest.config;

import ai.nubase.common.enums.Role;
import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

import java.util.List;

/**
 * PostgREST configuration properties
 * Maps to PostgREST configuration format for compatibility
 */
@Data
@Component
@ConfigurationProperties(prefix = "pgrst")
public class PostgRESTConfig {

    /**
     * Database connection URI
     */
    private String dbUri;

    /**
     * Database schema to expose
     */
    private List<String> dbSchemas = List.of("public");

    /**
     * Anonymous role for unauthenticated requests
     */
    private String dbAnonRole = Role.ANON.getValue();

    /**
     * Maximum number of rows returned
     */
    private Integer dbMaxRows;

    /**
     * Extra search path for database objects
     */
    private List<String> dbExtraSearchPath = List.of();

    /**
     * Database pool size
     */
    private Integer dbPoolSize = 10;

    /**
     * Database pool timeout (seconds)
     */
    private Integer dbPoolTimeout = 10;

    /**
     * Server host
     */
    private String serverHost = "localhost";

    /**
     * Server port
     */
    private Integer serverPort = 3000;

    /**
     * Unix socket path (alternative to host/port)
     */
    private String serverUnixSocket;

    /**
     * JWT secret for token validation
     */
    private String jwtSecret;

    /**
     * JWT secret file path
     */
    private String jwtSecretFile;

    /**
     * JWT secret base64 encoded
     */
    private Boolean jwtSecretIsBase64 = false;

    /**
     * JWT audience claim validation
     */
    private String jwtAudience;

    /**
     * JWT role claim key
     */
    private String jwtRoleClaimKey = ".role";

    /**
     * OpenAPI server proxy URI
     */
    private String openApiServerProxyUri;

    /**
     * OpenAPI mode
     */
    private String openApiMode = "follow-privileges";

    /**
     * Log level
     */
    private String logLevel = "error";

    /**
     * Raw media types
     */
    private List<String> rawMediaTypes = List.of("application/octet-stream");
}
