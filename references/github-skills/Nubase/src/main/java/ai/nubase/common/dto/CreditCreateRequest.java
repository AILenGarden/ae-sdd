package ai.nubase.common.dto;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.math.BigDecimal;
import java.time.LocalDateTime;

/**
 * Credit creation request DTO
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class CreditCreateRequest {

    /**
     * User ID
     */
    private Long userId;

    /**
     * Credit type (INCREASE/CONSUME)
     */
    private String type;

    /**
     * Credit sub-type (APP_USAGE, AI_IMAGE, etc.)
     */
    private String subType;

    /**
     * Credit amount (negative for consumption)
     */
    private BigDecimal credit;

    /**
     * Start time of validity period
     */
    private LocalDateTime startTime;

    /**
     * End time of validity period
     */
    private LocalDateTime endTime;

    /**
     * Input tokens used
     */
    private Long usedInputTokens;

    /**
     * Output tokens used
     */
    private Long usedOutputTokens;

    /**
     * Description
     */
    private String description;

    /**
     * Turn ID from request header (x-turn-id)
     */
    private Integer turnId;

    /**
     * App Key (private_key)
     */
    private String appKey;

    /**
     * Question ID (for refund tracking)
     */
    private Long questionId;
}
