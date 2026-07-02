package ai.nubase.auth.dto.response.database;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Foreign key constraint information
 * Compatible with Supabase MCP's foreign key structure
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ForeignKeyInfo {
    /**
     * Constraint name
     */
    private String name;

    /**
     * Source table (schema.table)
     */
    private String source;

    /**
     * Target table (schema.table)
     */
    private String target;
}
