package ai.nubase.auth.dto.response.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * Response DTO for schema initialization results.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class InitSchemaResponse {

    /**
     * Whether the initialization was successful
     */
    private boolean success;

    /**
     * Schema name that was created/initialized
     */
    private String schema;

    /**
     * Admin role name that was created
     */
    @JsonProperty("admin_role")
    private String adminRole;

    /**
     * User role name that was created
     */
    @JsonProperty("user_role")
    private String userRole;

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
     * Create a success response
     */
    public static InitSchemaResponse success(String schema, String adminRole, String userRole,
                                             List<String> steps, long executionTimeMs) {
        return InitSchemaResponse.builder()
                .success(true)
                .schema(schema)
                .adminRole(adminRole)
                .userRole(userRole)
                .steps(steps)
                .executionTimeMs(executionTimeMs)
                .build();
    }

    /**
     * Create an error response
     */
    public static InitSchemaResponse error(String errorMessage, String errorDetails, long executionTimeMs) {
        return InitSchemaResponse.builder()
                .success(false)
                .error(errorMessage)
                .errorDetails(errorDetails)
                .executionTimeMs(executionTimeMs)
                .build();
    }
}
