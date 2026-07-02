package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

/**
 * Order by specification
 */
@Data
@Builder
public class OrderBy {
    private String column;
    private Direction direction;
    private NullsOrder nullsOrder;

    public enum Direction {
        ASC,
        DESC
    }

    public enum NullsOrder {
        FIRST,
        LAST
    }
}
