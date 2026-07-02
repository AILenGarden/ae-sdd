package ai.nubase.auth.dto.response.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.LocalDateTime;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class SqlHistoryEntry {
    private Long id;
    private String query;
    private Boolean success;

    @JsonProperty("execution_time_ms")
    private Long executionTimeMs;

    @JsonProperty("error_message")
    private String errorMessage;

    @JsonProperty("created_at")
    private LocalDateTime createdAt;
}
