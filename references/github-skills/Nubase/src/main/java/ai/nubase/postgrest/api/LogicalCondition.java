package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

import java.util.List;

/**
 * Represents a logical condition that can be:
 * 1. A single filter (column.operator.value)
 * 2. A group of conditions combined with AND/OR
 *
 * This supports PostgREST's nested logical operators:
 * - ?or=(age.lt.18,age.gt.65)
 * - ?and=(status.eq.active,or=(priority.eq.high,priority.eq.critical))
 * - ?not.or=(a.eq.1,b.eq.2)
 */
@Data
@Builder
public class LogicalCondition {

    /**
     * The type of this condition
     */
    private ConditionType type;

    /**
     * For FILTER type: the actual filter
     */
    private Filter filter;

    /**
     * For AND/OR type: the list of sub-conditions
     */
    private List<LogicalCondition> conditions;

    /**
     * Whether this condition is negated (NOT)
     */
    private boolean negate;

    public enum ConditionType {
        FILTER,  // Single filter condition
        AND,     // AND group of conditions
        OR       // OR group of conditions
    }

    /**
     * Create a simple filter condition
     */
    public static LogicalCondition filter(Filter filter) {
        return LogicalCondition.builder()
            .type(ConditionType.FILTER)
            .filter(filter)
            .negate(filter.isNegate())
            .build();
    }

    /**
     * Create an OR group
     */
    public static LogicalCondition or(List<LogicalCondition> conditions, boolean negate) {
        return LogicalCondition.builder()
            .type(ConditionType.OR)
            .conditions(conditions)
            .negate(negate)
            .build();
    }

    /**
     * Create an AND group
     */
    public static LogicalCondition and(List<LogicalCondition> conditions, boolean negate) {
        return LogicalCondition.builder()
            .type(ConditionType.AND)
            .conditions(conditions)
            .negate(negate)
            .build();
    }
}
