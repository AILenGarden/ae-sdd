package ai.nubase.postgrest.query;

import lombok.Builder;
import lombok.Data;

import ai.nubase.postgrest.api.Filter;

/**
 * Represents a WHERE clause condition
 */
@Data
@Builder
public class WhereClause {
    private String column;
    private String operator;
    private Object value;
    private boolean negate;
    private LogicalOperator logicalOperator; // AND, OR
    private String operatorType; // For distinguishing FTS variants, array types, etc.
    private Filter.Quantifier quantifier; // ANY or ALL quantifier for array comparisons

    public enum LogicalOperator {
        AND,
        OR
    }
}
