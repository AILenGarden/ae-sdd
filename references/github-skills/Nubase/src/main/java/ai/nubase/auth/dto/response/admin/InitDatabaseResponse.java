package ai.nubase.auth.dto.response.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * Response DTO for database initialization results.
 * Returns success status and details about created Supabase schemas.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class InitDatabaseResponse {

    /**
     * Whether the initialization was successful
     */
    private boolean success;

    /**
     * Message describing the result
     */
    private String message;

    /**
     * List of executed steps
     */
    private List<String> steps;

    /**
     * Total execution time in milliseconds
     */
    @JsonProperty("execution_time_ms")
    private Long executionTimeMs;

    /**
     * Error message if initialization failed
     */
    private String error;

    /**
     * Detailed error information
     */
    @JsonProperty("error_details")
    private String errorDetails;


    /**
     * JWT Secret (plain text, used to validate API keys and user tokens)
     */
    private String jwtSecret;

    /**
     * Service Role Key (used to identify admin-level API requests)
     */
    private String serviceRoleToken;

    private String authenticatedToken;

    private String initStatus;

    /**
     * Create a success response
     */
    public static InitDatabaseResponse success(String jwtSecret,String serviceRoleToken,String authenticatedToken,String initStatus, List<String> steps, long executionTimeMs) {
        return InitDatabaseResponse.builder()
                .success(true)
                .jwtSecret(jwtSecret)
                .serviceRoleToken(serviceRoleToken)
                .authenticatedToken(authenticatedToken)
                .initStatus(initStatus)
                .steps(steps)
                .executionTimeMs(executionTimeMs)
                .build();
    }

    /**
     * Create an error response
     */
    public static InitDatabaseResponse error(String errorMessage, String errorDetails, long executionTimeMs) {
        return InitDatabaseResponse.builder()
                .success(false)
                .error(errorMessage)
                .errorDetails(errorDetails)
                .executionTimeMs(executionTimeMs)
                .build();
    }
}
