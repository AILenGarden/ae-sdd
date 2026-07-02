package ai.nubase.auth.dto.response.database;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * Column information response
 * Compatible with Supabase MCP's column structure
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ColumnInfo {
    /**
     * Column name
     */
    private String name;

    /**
     * Column description/comment
     */
    private String comment;

    /**
     * PostgreSQL data type (e.g., "integer", "text", "timestamp")
     */
    @JsonProperty("data_type")
    private String dataType;

    /**
     * User-defined type name
     */
    private String format;

    /**
     * Whether column allows NULL values
     */
    private Boolean nullable;

    /**
     * Default value expression
     */
    @JsonProperty("default_value")
    private String defaultValue;

    /**
     * Whether column is an identity column
     */
    private Boolean identity;

    /**
     * Identity generation method (ALWAYS or BY DEFAULT)
     */
    @JsonProperty("identity_generation")
    private String identityGeneration;

    /**
     * Whether column is generated
     */
    private Boolean generated;

    /**
     * Whether column is updatable
     */
    private Boolean updatable;

    /**
     * Whether column has a unique constraint
     */
    private Boolean unique;

    /**
     * Enum values for enum types
     */
    private List<String> enums;

    /**
     * Check constraint expression
     */
    private String check;

    /**
     * Column position in table
     */
    private Integer ordinalPosition;
}
