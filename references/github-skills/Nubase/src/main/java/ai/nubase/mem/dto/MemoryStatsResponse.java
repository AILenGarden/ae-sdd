package ai.nubase.mem.dto;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;
import java.util.UUID;

/**
 * Aggregate metrics for the admin Memory dashboard. Computed in a single round-trip
 * against the tenant DB (4 small SELECTs).
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class MemoryStatsResponse {

    /** Total non-deleted memories in the queried scope. */
    private long totalMemories;

    /** Total distinct entities in the queried scope (mem.entities row count). */
    private long totalEntities;

    /** Counts of audit events in the last 24 hours, broken down by event type. */
    private RecentActivity last24h;

    /** Top {@code N} users by memory count (only meaningful when caller is service_role). */
    private List<UserCount> topUsers;

    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class RecentActivity {
        private long add;
        private long update;
        private long delete;
    }

    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class UserCount {
        private UUID userId;
        private long count;
    }
}
