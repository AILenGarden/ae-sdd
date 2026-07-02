package ai.nubase.postgrest.schema;

import lombok.Builder;
import lombok.Data;

/**
 * Represents a database column
 * Equivalent to PostgREST's Column type
 */
@Data
@Builder
public class Column {
    private String name;
    private String description;
    private String dataType;
    private String udtName;
    private Integer maxLength;
    private Integer numericPrecision;
    private Integer numericScale;
    private boolean nullable;
    private boolean hasDefault;
    private String defaultValue;
    private Integer position;
}
