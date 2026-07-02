package ai.nubase.common.dto;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Token update request DTO
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class TokenUpdateRequest {

    /**
     * Total used input tokens (not incremental)
     */
    private Long usedInputTokens;

    /**
     * Total used output tokens (not incremental)
     */
    private Long usedOutputTokens;
}
