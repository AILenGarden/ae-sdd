package ai.nubase.auth.service;

import ai.nubase.auth.dto.request.admin.ExportSchemaDdlRequest;
import ai.nubase.auth.dto.response.admin.ExportSchemaDdlResponse;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Service;

import java.util.*;
import java.util.stream.Collectors;

/**
 * Service for exporting PostgreSQL schema DDL statements
 * <p>
 * Generates complete DDL for all tables in a schema including:
 * - Table structure with column definitions
 * - Column comments
 * - Primary keys
 * - Foreign keys
 * - Unique constraints
 * - Check constraints
 * - Indexes
 *
 * @author nubase
 * @since 2025-01-03
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class SchemaDdlExportService {

    private final JdbcTemplate jdbcTemplate;

    /**
     * Export DDL for all tables in the specified schema
     */
    public ExportSchemaDdlResponse exportSchemaDdl(ExportSchemaDdlRequest request) {
        long startTime = System.currentTimeMillis();

        try {
            String schemaName = request.getSchemaName();
            Set<String> tableFilter = parseTableFilter(request.getTableNames());
            boolean includeDropStatements = Boolean.TRUE.equals(request.getIncludeDropStatements());
            boolean includeIfNotExists = request.getIncludeIfNotExists() == null || request.getIncludeIfNotExists();

            // Get all tables in the schema
            List<String> tables = getTables(schemaName, tableFilter);

            if (tables.isEmpty()) {
                return ExportSchemaDdlResponse.builder()
                        .success(true)
                        .schemaName(schemaName)
                        .executionTimeMs(System.currentTimeMillis() - startTime)
                        .build();
            }

            // Generate DDL for each table
            Map<String, String> tableDdls = new LinkedHashMap<>();
            for (String tableName : tables) {
                String ddl = generateTableDdl(schemaName, tableName, includeDropStatements, includeIfNotExists);
                tableDdls.put(tableName, ddl);
            }

            return ExportSchemaDdlResponse.builder()
                    .success(true)
                    .schemaName(schemaName)
                    .tableDdls(tableDdls)
                    .tableOrder(tables)
                    .tableCount(tables.size())
                    .executionTimeMs(System.currentTimeMillis() - startTime)
                    .build();

        } catch (Exception e) {
            log.error("Failed to export schema DDL: {}", e.getMessage(), e);
            return ExportSchemaDdlResponse.builder()
                    .success(false)
                    .schemaName(request.getSchemaName())
                    .error("Failed to export schema DDL: " + e.getMessage())
                    .errorDetails(e.toString())
                    .executionTimeMs(System.currentTimeMillis() - startTime)
                    .build();
        }
    }

    /**
     * Parse comma-separated table names into a set
     */
    private Set<String> parseTableFilter(String tableNames) {
        if (tableNames == null || tableNames.strip().isEmpty()) {
            return Collections.emptySet();
        }
        return Arrays.stream(tableNames.split(","))
                .map(String::strip)
                .filter(s -> !s.isEmpty())
                .collect(Collectors.toSet());
    }

    /**
     * Get all tables in the schema
     */
    private List<String> getTables(String schemaName, Set<String> tableFilter) {
        String sql = """
                SELECT table_name
                FROM information_schema.tables
                WHERE table_schema = ?
                  AND table_type = 'BASE TABLE'
                ORDER BY table_name
                """;

        List<String> allTables = jdbcTemplate.queryForList(sql, String.class, schemaName);

        if (tableFilter.isEmpty()) {
            return allTables;
        }

        return allTables.stream()
                .filter(tableFilter::contains)
                .collect(Collectors.toList());
    }

    /**
     * Generate complete DDL for a single table
     */
    private String generateTableDdl(String schemaName, String tableName, boolean includeDropStatements, boolean includeIfNotExists) {
        StringBuilder ddl = new StringBuilder();

        // DROP statement
        if (includeDropStatements) {
            ddl.append(String.format("DROP TABLE IF EXISTS %s.%s CASCADE;\n\n", schemaName, tableName));
        }

        // CREATE TABLE statement
        ddl.append(generateCreateTableStatement(schemaName, tableName, includeIfNotExists));

        // Table comment
        String tableComment = getTableComment(schemaName, tableName);
        if (tableComment != null && !tableComment.isEmpty()) {
            ddl.append(String.format("\nCOMMENT ON TABLE %s.%s IS '%s';\n",
                    schemaName, tableName, escapeSingleQuote(tableComment)));
        }

        // Column comments
        ddl.append(generateColumnComments(schemaName, tableName));

        // Indexes (excluding primary key and unique constraint indexes)
        ddl.append(generateIndexStatements(schemaName, tableName));

        return ddl.toString();
    }

    /**
     * Generate CREATE TABLE statement with columns and constraints
     */
    private String generateCreateTableStatement(String schemaName, String tableName, boolean includeIfNotExists) {
        StringBuilder sql = new StringBuilder();
        sql.append("CREATE TABLE ");
        if (includeIfNotExists) {
            sql.append("IF NOT EXISTS ");
        }
        sql.append(String.format("%s.%s\n(\n", schemaName, tableName));

        // Get columns
        List<ColumnDefinition> columns = getColumns(schemaName, tableName);
        List<String> columnDefs = new ArrayList<>();

        for (ColumnDefinition col : columns) {
            StringBuilder colDef = new StringBuilder();
            colDef.append(String.format("    %s %s", col.columnName, col.dataType));

            // Nullable
            if ("NO".equals(col.isNullable)) {
                colDef.append(" NOT NULL");
            }

            // Default value
            if (col.columnDefault != null) {
                colDef.append(" DEFAULT ").append(col.columnDefault);
            }

            columnDefs.add(colDef.toString());
        }

        sql.append(String.join(",\n", columnDefs));

        // Add constraints
        List<String> constraints = getConstraints(schemaName, tableName);
        if (!constraints.isEmpty()) {
            sql.append(",\n\n");
            sql.append(String.join(",\n", constraints));
        }

        sql.append("\n);\n");

        return sql.toString();
    }

    /**
     * Get column definitions for a table
     */
    private List<ColumnDefinition> getColumns(String schemaName, String tableName) {
        String sql = """
                SELECT
                    column_name,
                    data_type,
                    character_maximum_length,
                    numeric_precision,
                    numeric_scale,
                    is_nullable,
                    column_default,
                    udt_name,
                    ordinal_position
                FROM information_schema.columns
                WHERE table_schema = ?
                  AND table_name = ?
                ORDER BY ordinal_position
                """;

        return jdbcTemplate.query(sql, (rs, rowNum) -> {
            ColumnDefinition col = new ColumnDefinition();
            col.columnName = rs.getString("column_name");
            col.isNullable = rs.getString("is_nullable");
            col.columnDefault = rs.getString("column_default");

            // Build data type with length/precision
            String dataType = rs.getString("data_type");
            String udtName = rs.getString("udt_name");
            Integer maxLength = (Integer) rs.getObject("character_maximum_length");
            Integer precision = (Integer) rs.getObject("numeric_precision");
            Integer scale = (Integer) rs.getObject("numeric_scale");

            col.dataType = buildDataType(dataType, udtName, maxLength, precision, scale);

            return col;
        }, schemaName, tableName);
    }

    /**
     * Build complete data type string with length/precision
     */
    private String buildDataType(String dataType, String udtName, Integer maxLength, Integer precision, Integer scale) {
        // Handle special PostgreSQL types
        if ("ARRAY".equals(dataType)) {
            return udtName.substring(1).toUpperCase() + "[]";
        }

        if ("USER-DEFINED".equals(dataType)) {
            return udtName.toUpperCase();
        }

        String type = dataType.toUpperCase();

        // Add length for character types
        if (maxLength != null && ("CHARACTER VARYING".equals(dataType) || "VARCHAR".equals(dataType))) {
            return String.format("VARCHAR(%d)", maxLength);
        }

        if (maxLength != null && "CHARACTER".equals(dataType)) {
            return String.format("CHAR(%d)", maxLength);
        }

        // Add precision/scale for numeric types
        if (precision != null && scale != null && scale > 0) {
            return String.format("NUMERIC(%d,%d)", precision, scale);
        }

        if (precision != null && ("NUMERIC".equals(dataType) || "DECIMAL".equals(dataType))) {
            return String.format("NUMERIC(%d)", precision);
        }

        // Map common types to PostgreSQL standard names
        return switch (type) {
            case "CHARACTER VARYING" -> "VARCHAR";
            case "TIMESTAMP WITHOUT TIME ZONE" -> "TIMESTAMP";
            case "TIMESTAMP WITH TIME ZONE" -> "TIMESTAMP WITH TIME ZONE";
            case "TIME WITHOUT TIME ZONE" -> "TIME";
            case "TIME WITH TIME ZONE" -> "TIME WITH TIME ZONE";
            default -> type;
        };
    }

    /**
     * Get table constraints (PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK)
     */
    private List<String> getConstraints(String schemaName, String tableName) {
        List<String> constraints = new ArrayList<>();

        // Primary key
        String pk = getPrimaryKeyConstraint(schemaName, tableName);
        if (pk != null) {
            constraints.add(pk);
        }

        // Foreign keys
        constraints.addAll(getForeignKeyConstraints(schemaName, tableName));

        // Unique constraints
        constraints.addAll(getUniqueConstraints(schemaName, tableName));

        // Check constraints
        constraints.addAll(getCheckConstraints(schemaName, tableName));

        return constraints;
    }

    /**
     * Get primary key constraint
     */
    private String getPrimaryKeyConstraint(String schemaName, String tableName) {
        String sql = """
                SELECT
                    tc.constraint_name,
                    string_agg(kcu.column_name, ', ' ORDER BY kcu.ordinal_position) as columns
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                    ON tc.constraint_name = kcu.constraint_name
                    AND tc.table_schema = kcu.table_schema
                WHERE tc.table_schema = ?
                  AND tc.table_name = ?
                  AND tc.constraint_type = 'PRIMARY KEY'
                GROUP BY tc.constraint_name
                """;

        List<Map<String, Object>> results = jdbcTemplate.queryForList(sql, schemaName, tableName);
        if (results.isEmpty()) {
            return null;
        }

        Map<String, Object> row = results.get(0);
        String constraintName = (String) row.get("constraint_name");
        String columns = (String) row.get("columns");

        return String.format("    CONSTRAINT %s PRIMARY KEY (%s)", constraintName, columns);
    }

    /**
     * Get foreign key constraints
     */
    private List<String> getForeignKeyConstraints(String schemaName, String tableName) {
        String sql = """
                SELECT
                    tc.constraint_name,
                    kcu.column_name,
                    ccu.table_schema AS foreign_table_schema,
                    ccu.table_name AS foreign_table_name,
                    ccu.column_name AS foreign_column_name,
                    rc.update_rule,
                    rc.delete_rule
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                    ON tc.constraint_name = kcu.constraint_name
                    AND tc.table_schema = kcu.table_schema
                JOIN information_schema.constraint_column_usage ccu
                    ON ccu.constraint_name = tc.constraint_name
                    AND ccu.table_schema = tc.table_schema
                JOIN information_schema.referential_constraints rc
                    ON rc.constraint_name = tc.constraint_name
                    AND rc.constraint_schema = tc.table_schema
                WHERE tc.table_schema = ?
                  AND tc.table_name = ?
                  AND tc.constraint_type = 'FOREIGN KEY'
                ORDER BY tc.constraint_name
                """;

        List<Map<String, Object>> results = jdbcTemplate.queryForList(sql, schemaName, tableName);

        Map<String, ForeignKeyInfo> fkMap = new LinkedHashMap<>();
        for (Map<String, Object> row : results) {
            String constraintName = (String) row.get("constraint_name");
            ForeignKeyInfo fk = fkMap.computeIfAbsent(constraintName, k -> new ForeignKeyInfo());
            fk.constraintName = constraintName;
            fk.columns.add((String) row.get("column_name"));
            fk.foreignTableSchema = (String) row.get("foreign_table_schema");
            fk.foreignTableName = (String) row.get("foreign_table_name");
            fk.foreignColumns.add((String) row.get("foreign_column_name"));
            fk.updateRule = (String) row.get("update_rule");
            fk.deleteRule = (String) row.get("delete_rule");
        }

        List<String> constraints = new ArrayList<>();
        for (ForeignKeyInfo fk : fkMap.values()) {
            StringBuilder constraint = new StringBuilder();
            constraint.append(String.format("    CONSTRAINT %s FOREIGN KEY (%s)",
                    fk.constraintName,
                    String.join(", ", fk.columns)));
            constraint.append(String.format(" REFERENCES %s.%s (%s)",
                    fk.foreignTableSchema,
                    fk.foreignTableName,
                    String.join(", ", fk.foreignColumns)));

            if (!"NO ACTION".equals(fk.deleteRule)) {
                constraint.append(" ON DELETE ").append(fk.deleteRule);
            }
            if (!"NO ACTION".equals(fk.updateRule)) {
                constraint.append(" ON UPDATE ").append(fk.updateRule);
            }

            constraints.add(constraint.toString());
        }

        return constraints;
    }

    /**
     * Get unique constraints
     */
    private List<String> getUniqueConstraints(String schemaName, String tableName) {
        String sql = """
                SELECT
                    tc.constraint_name,
                    string_agg(kcu.column_name, ', ' ORDER BY kcu.ordinal_position) as columns
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                    ON tc.constraint_name = kcu.constraint_name
                    AND tc.table_schema = kcu.table_schema
                WHERE tc.table_schema = ?
                  AND tc.table_name = ?
                  AND tc.constraint_type = 'UNIQUE'
                GROUP BY tc.constraint_name
                """;

        List<Map<String, Object>> results = jdbcTemplate.queryForList(sql, schemaName, tableName);
        List<String> constraints = new ArrayList<>();

        for (Map<String, Object> row : results) {
            String constraintName = (String) row.get("constraint_name");
            String columns = (String) row.get("columns");
            constraints.add(String.format("    CONSTRAINT %s UNIQUE (%s)", constraintName, columns));
        }

        return constraints;
    }

    /**
     * Get check constraints
     */
    private List<String> getCheckConstraints(String schemaName, String tableName) {
        String sql = """
                SELECT
                    con.conname as constraint_name,
                    pg_get_constraintdef(con.oid) as constraint_def
                FROM pg_catalog.pg_constraint con
                JOIN pg_catalog.pg_class rel ON rel.oid = con.conrelid
                JOIN pg_catalog.pg_namespace nsp ON nsp.oid = rel.relnamespace
                WHERE nsp.nspname = ?
                  AND rel.relname = ?
                  AND con.contype = 'c'
                ORDER BY con.conname
                """;

        List<Map<String, Object>> results = jdbcTemplate.queryForList(sql, schemaName, tableName);
        List<String> constraints = new ArrayList<>();

        for (Map<String, Object> row : results) {
            String constraintName = (String) row.get("constraint_name");
            String constraintDef = (String) row.get("constraint_def");
            constraints.add(String.format("    CONSTRAINT %s %s", constraintName, constraintDef));
        }

        return constraints;
    }

    /**
     * Get table comment
     */
    private String getTableComment(String schemaName, String tableName) {
        String sql = """
                SELECT obj_description(
                    (quote_ident(?) || '.' || quote_ident(?))::regclass,
                    'pg_class'
                ) as comment
                """;

        return jdbcTemplate.queryForObject(sql, String.class, schemaName, tableName);
    }

    /**
     * Generate column comments
     */
    private String generateColumnComments(String schemaName, String tableName) {
        String sql = """
                SELECT
                    cols.column_name,
                    pg_catalog.col_description(
                        (quote_ident(cols.table_schema) || '.' || quote_ident(cols.table_name))::regclass::oid,
                        cols.ordinal_position
                    ) as comment
                FROM information_schema.columns cols
                WHERE cols.table_schema = ?
                  AND cols.table_name = ?
                  AND pg_catalog.col_description(
                        (quote_ident(cols.table_schema) || '.' || quote_ident(cols.table_name))::regclass::oid,
                        cols.ordinal_position
                      ) IS NOT NULL
                ORDER BY cols.ordinal_position
                """;

        List<Map<String, Object>> results = jdbcTemplate.queryForList(sql, schemaName, tableName);
        if (results.isEmpty()) {
            return "";
        }

        StringBuilder comments = new StringBuilder("\n");
        for (Map<String, Object> row : results) {
            String columnName = (String) row.get("column_name");
            String comment = (String) row.get("comment");
            comments.append(String.format("COMMENT ON COLUMN %s.%s.%s IS '%s';\n",
                    schemaName, tableName, columnName, escapeSingleQuote(comment)));
        }

        return comments.toString();
    }

    /**
     * Generate index statements (excluding primary key and unique constraint indexes)
     */
    private String generateIndexStatements(String schemaName, String tableName) {
        String sql = """
                SELECT
                    i.indexname,
                    i.indexdef
                FROM pg_catalog.pg_indexes i
                LEFT JOIN pg_catalog.pg_constraint c
                    ON c.conname = i.indexname
                WHERE i.schemaname = ?
                  AND i.tablename = ?
                  AND c.conname IS NULL
                ORDER BY i.indexname
                """;

        List<Map<String, Object>> results = jdbcTemplate.queryForList(sql, schemaName, tableName);
        if (results.isEmpty()) {
            return "";
        }

        StringBuilder indexes = new StringBuilder("\n");
        for (Map<String, Object> row : results) {
            String indexDef = (String) row.get("indexdef");
            // Convert CREATE INDEX to CREATE INDEX IF NOT EXISTS
            indexDef = indexDef.replaceFirst("CREATE INDEX", "CREATE INDEX IF NOT EXISTS");
            indexes.append(indexDef).append(";\n");
        }

        return indexes.toString();
    }

    /**
     * Escape single quotes in strings for SQL
     */
    private String escapeSingleQuote(String str) {
        if (str == null) {
            return null;
        }
        return str.replace("'", "''");
    }

    /**
     * Column definition holder
     */
    private static class ColumnDefinition {
        String columnName;
        String dataType;
        String isNullable;
        String columnDefault;
    }

    /**
     * Foreign key information holder
     */
    private static class ForeignKeyInfo {
        String constraintName;
        List<String> columns = new ArrayList<>();
        String foreignTableSchema;
        String foreignTableName;
        List<String> foreignColumns = new ArrayList<>();
        String updateRule;
        String deleteRule;
    }
}
