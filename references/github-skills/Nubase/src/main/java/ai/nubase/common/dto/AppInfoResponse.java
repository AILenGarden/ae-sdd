package ai.nubase.common.dto;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Application information response DTO
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class AppInfoResponse {

    /**
     * Application ID
     */
    private Long id;

    /**
     * Application code
     */
    private String appCode;

    /**
     * Application name
     */
    private String appName;

    /**
     * User ID who created this application
     */
    private Long createUserId;

    /**
     * Application status (VALID/INVALID)
     */
    private String status;

    /**
     * Application type
     */
    private String appType;
}
