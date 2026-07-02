package ai.nubase.postgrest.schema;

import lombok.Builder;
import lombok.Data;

/**
 * Represents a function parameter
 */
@Data
@Builder
public class RoutineParam {
    private String name;
    private String dataType;
    private boolean required;
    private boolean variadic;
    private String defaultValue;
}
