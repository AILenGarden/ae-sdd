package ai.nubase.mcp.safety;

import org.springframework.stereotype.Component;

import java.util.Arrays;
import java.util.Locale;

@Component
public class SqlRiskClassifier {

    public SqlRisk classify(String sql) {
        String[] statements = splitStatements(sql);
        if (statements.length == 0) {
            return SqlRisk.UNKNOWN;
        }
        SqlRisk highest = SqlRisk.UNKNOWN;
        for (String statement : statements) {
            highest = max(highest, classifyStatement(statement));
        }
        return highest;
    }

    public int countStatements(String sql) {
        return splitStatements(sql).length;
    }

    private SqlRisk classifyStatement(String statement) {
        String normalized = normalize(statement);
        if (normalized.isBlank()) {
            return SqlRisk.UNKNOWN;
        }
        if (startsWithAny(normalized,
                "drop ", "truncate ", "reindex ", "vacuum full", "cluster ")) {
            return SqlRisk.DANGEROUS;
        }
        if (normalized.matches("^delete\\s+from\\s+[^\\s;]+\\s*$")) {
            return SqlRisk.DANGEROUS;
        }
        if (startsWithAny(normalized,
                "create ", "alter ", "grant ", "revoke ", "comment ", "security label ")) {
            return SqlRisk.SCHEMA_WRITE;
        }
        if (startsWithAny(normalized,
                "insert ", "update ", "delete ", "merge ", "copy ", "call ")) {
            return SqlRisk.DATA_WRITE;
        }
        if (startsWithAny(normalized,
                "select ", "with ", "show ", "explain ", "describe ")) {
            return SqlRisk.READ;
        }
        return SqlRisk.UNKNOWN;
    }

    private String[] splitStatements(String sql) {
        if (sql == null || sql.isBlank()) {
            return new String[0];
        }
        return Arrays.stream(sql.split(";"))
                .map(String::strip)
                .filter(s -> !s.isBlank())
                .toArray(String[]::new);
    }

    private String normalize(String statement) {
        return statement
                .replaceAll("(?s)/\\*.*?\\*/", " ")
                .replaceAll("(?m)--.*$", " ")
                .strip()
                .replaceAll("\\s+", " ")
                .toLowerCase(Locale.ROOT);
    }

    private boolean startsWithAny(String value, String... prefixes) {
        for (String prefix : prefixes) {
            if (value.startsWith(prefix)) {
                return true;
            }
        }
        return false;
    }

    private SqlRisk max(SqlRisk left, SqlRisk right) {
        return severity(left) >= severity(right) ? left : right;
    }

    private int severity(SqlRisk risk) {
        return switch (risk) {
            case UNKNOWN -> 0;
            case READ -> 1;
            case DATA_WRITE -> 2;
            case SCHEMA_WRITE -> 3;
            case DANGEROUS -> 4;
        };
    }
}
