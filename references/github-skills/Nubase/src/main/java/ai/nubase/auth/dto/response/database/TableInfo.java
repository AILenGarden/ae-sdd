package ai.nubase.auth.dto.response.database;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * Table information response
 * Compatible with Supabase MCP's table structure
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class TableInfo {
    /**
     * Schema name
     */
    private String schema;

    /**
     * Table name
     */
    private String name;

    /**
     * Table description/comment
     */
    private String comment;

    /**
     * Estimated number of live rows
     */
    private Long rows;

    /**
     * List of columns
     */
    private List<ColumnInfo> columns;

    /**
     * Primary key column names
     */
    @JsonProperty("primary_keys")
    private List<String> primaryKeys;

    /**
     * Foreign key constraints
     */
    @JsonProperty("foreign_key_constraints")
    private List<ForeignKeyInfo> foreignKeyConstraints;
}
