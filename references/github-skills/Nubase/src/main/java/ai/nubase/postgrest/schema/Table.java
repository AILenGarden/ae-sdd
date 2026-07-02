package ai.nubase.postgrest.schema;

import lombok.Builder;
import lombok.Data;

import java.util.List;
import java.util.Map;

/**
 * Represents a database table or view
 * Equivalent to PostgREST's SchemaCache.Table
 */
@Data
@Builder
public class Table {
    private String schema;
    private String name;
    private String description;
    private boolean insertable;
    private boolean updatable;
    private boolean deletable;
    private List<Column> columns;
    private List<String> primaryKey;
    private Map<String, ForeignKey> foreignKeys;
}
