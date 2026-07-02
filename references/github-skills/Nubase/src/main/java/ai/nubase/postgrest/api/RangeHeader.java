package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

/**
 * Range header for pagination
 * Format: Range: items=0-9
 */
@Data
@Builder
public class RangeHeader {
    private Long start;
    private Long end;
    private String unit; // typically "items"

    /**
     * Calculate the limit (number of items) from start and end
     * @return The limit, or null if end is null
     */
    public Long getLimit() {
        if (end == null) {
            return null;
        }
        if (start == null) {
            return end + 1; // Suffix range: -N means last N items
        }
        return end - start + 1;
    }
}
