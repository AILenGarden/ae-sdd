package ai.nubase.auth.util;

import com.alibaba.druid.sql.SQLUtils;
import com.alibaba.druid.sql.ast.SQLStatement;
import com.alibaba.druid.util.JdbcConstants;
import net.sf.jsqlparser.parser.CCJSqlParserUtil;
import net.sf.jsqlparser.statement.Statement;
import net.sf.jsqlparser.statement.Statements;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;

/**
 * SQL splitter utility (PostgreSQL-specific edition).
 * Strategy: JSqlParser -> Druid -> hand-written scanner.
 */
public class SqlSplitterUtil {

    private static final Logger log = LoggerFactory.getLogger(SqlSplitterUtil.class);

    /**
     * Public entry point: split a mixed SQL string into individual SQL statements.
     */
    public static List<String> splitSql(String rawSql) {
        if (rawSql == null || rawSql.strip().isEmpty()) {
            return new ArrayList<>();
        }
        // 1. Try JSqlParser
        try {
            return splitByJSqlParser(rawSql);
        } catch (Throwable e) {
            log.warn("Strategy [JSqlParser] failed to split SQL, trying fallback to Druid. Error: ");
        }

        // 2. Try Druid
        try {
            return splitByDruid(rawSql);
        } catch (Throwable e) {
            log.warn("Strategy [Druid] failed to split SQL, trying fallback to Manual Scanner. Error: {}", e.getMessage());
        }

        // 3. Fallback: hand-written scanner
        log.info("Using Strategy [Manual Scanner] to split SQL.");
        return splitByHand(rawSql);
    }

    // ================= Strategy 1: JSqlParser =================
    private static List<String> splitByJSqlParser(String sql) throws Exception {
        Statements statements = CCJSqlParserUtil.parseStatements(sql);
        List<String> result = new ArrayList<>();
        for (Statement stmt : statements.getStatements()) {
            // Note: toString() may reformat the SQL and strip original comments
            result.add(stmt.toString());
        }
        return result;
    }

    // ================= Strategy 2: Alibaba Druid =================
    private static List<String> splitByDruid(String sql) {
        // Parse with Druid, specifying the PostgreSQL dialect
        List<SQLStatement> statements = SQLUtils.parseStatements(sql, JdbcConstants.POSTGRESQL);
        List<String> result = new ArrayList<>();
        for (SQLStatement stmt : statements) {
            // Druid's toString also reformats the SQL
            result.add(stmt.toString());
        }
        return result;
    }

    // ================= Strategy 3: Hand-written fault-tolerant scanner (fallback) =================
    /**
     * Does not validate syntax; splits purely on semicolons.
     * Supports: PostgreSQL $$ placeholders, single quotes, double quotes, single-line / multi-line comments.
     */
// ================= Strategy 3: Hand-written fault-tolerant scanner (fallback) - NPE fixed =================
    private static List<String> splitByHand(String sql) {
        List<String> statements = new ArrayList<>();
        StringBuilder sb = new StringBuilder();
        int len = sql.length();

        boolean inString = false;       // '...'
        boolean inIdentifier = false;   // "..."
        boolean inComment = false;      // -- ...
        boolean inBlockComment = false; // /* ... */
        String dollarTag = null;        // $tag$

        for (int i = 0; i < len; i++) {
            char c = sql.charAt(i);
            char next = (i + 1 < len) ? sql.charAt(i + 1) : '\0';

            // 1. Handle Dollar Quotes $$...$$ (PG-specific)
            if (!inString && !inIdentifier && !inComment && !inBlockComment) {
                if (dollarTag == null) {
                    if (c == '$') {
                        int tagEnd = findDollarTagEnd(sql, i);
                        if (tagEnd != -1) {
                            dollarTag = sql.substring(i, tagEnd + 1);
                            sb.append(dollarTag);
                            i = tagEnd;
                            continue;
                        }
                    }
                } else {
                    // Detect whether the closing tag is encountered
                    if (c == '$' && sql.startsWith(dollarTag, i)) {
                        sb.append(dollarTag);           // 1. Append the closing tag first
                        i += dollarTag.length() - 1;    // 2. Skip over the tag's length
                        dollarTag = null;               // 3. Finally, reset the state to null
                        continue;
                    }
                }
            }

            // While inside a Dollar Quote, append characters directly and ignore all special symbols
            if (dollarTag != null) {
                sb.append(c);
                continue;
            }

            // 2. Handle comments
            if (!inString && !inIdentifier) {
                if (!inComment && !inBlockComment) {
                    if (c == '-' && next == '-') {
                        inComment = true;
                    } else if (c == '/' && next == '*') {
                        inBlockComment = true;
                        sb.append(c).append(next);
                        i++;
                        continue;
                    }
                } else {
                    if (inComment && c == '\n') {
                        inComment = false;
                    } else if (inBlockComment && c == '*' && next == '/') {
                        inBlockComment = false;
                        sb.append(c).append(next);
                        i++;
                        continue;
                    }
                }
            }

            // 3. Handle quotes
            if (!inComment && !inBlockComment) {
                if (c == '\'' && !inIdentifier) {
                    if (inString && next == '\'') { // Escaped '
                        sb.append(c).append(next);
                        i++;
                        continue;
                    }
                    inString = !inString;
                } else if (c == '"' && !inString) {
                    if (inIdentifier && next == '"') { // Escaped "
                        sb.append(c).append(next);
                        i++;
                        continue;
                    }
                    inIdentifier = !inIdentifier;
                }
            }

            // 4. Split
            if (c == ';' && !inString && !inIdentifier && !inComment && !inBlockComment && dollarTag == null) {
                String stmt = sb.toString().strip();
                if (!stmt.isEmpty()) {
                    statements.add(stmt);
                }
                sb.setLength(0);
            } else {
                sb.append(c);
            }
        }

        if (sb.length() > 0) {
            String stmt = sb.toString().strip();
            if (!stmt.isEmpty()) {
                statements.add(stmt);
            }
        }
        return statements;
    }

    private static int findDollarTagEnd(String sql, int start) {
        for (int i = start + 1; i < sql.length(); i++) {
            char c = sql.charAt(i);
            if (c == '$') return i;
            if (!Character.isLetterOrDigit(c) && c != '_') return -1;
        }
        return -1;
    }
}
