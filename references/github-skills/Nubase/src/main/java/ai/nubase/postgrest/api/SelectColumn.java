package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

import java.util.List;

/**
 * Select column specification
 * Supports column selection and resource embedding
 */
@Data
@Builder
public class SelectColumn {
    private String name;
    private String alias;
    private boolean isAggregate;
    private String aggregateFunction; // count, sum, avg, max, min
    private List<SelectColumn> embedded; // For resource embedding
    private String hint; // Hint for relationship resolution
}
