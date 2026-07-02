package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

/**
 * Filter condition
 * Supports PostgREST filter operators: eq, neq, gt, gte, lt, lte, like, ilike, is, in, etc.
 * Supports quantifiers: any() and all() for array comparisons.
 * Example: ?id=eq(any).{1,2,3} or ?name=like(all).{%a%,%b%}
 */
@Data
@Builder
public class Filter {
    private String column;
    private FilterOperator operator;
    private String value;
    private boolean negate;
    private Quantifier quantifier;  // any() or all() quantifier

    /**
     * Quantifier for array-based comparisons.
     * - ANY: Match if ANY value in the array satisfies the condition
     * - ALL: Match if ALL values in the array satisfy the condition
     */
    public enum Quantifier {
        ANY,  // = ANY(array), LIKE ANY(array), etc.
        ALL   // = ALL(array), LIKE ALL(array), etc.
    }

    public enum FilterOperator {
        EQ,      // eq - equals
        NEQ,     // neq - not equals
        GT,      // gt - greater than
        GTE,     // gte - greater than or equal
        LT,      // lt - less than
        LTE,     // lte - less than or equal
        LIKE,    // like - LIKE operator
        ILIKE,   // ilike - ILIKE operator (case-insensitive)
        MATCH,   // ~ - regex match
        IMATCH,  // ~* - case-insensitive regex match
        IN,      // in - IN operator
        IS,      // is - IS operator (null, true, false)
        FTS,     // @@ - full-text search (to_tsquery)
        PLFTS,   // plainto_tsquery - plain text full-text search
        PHFTS,   // phraseto_tsquery - phrase full-text search
        WFTS,    // websearch_to_tsquery - web search style
        CS,      // @> - contains
        CD,      // <@ - contained in
        OV,      // && - overlap
        SL,      // << - strictly left of
        SR,      // >> - strictly right of
        NXR,     // &< - does not extend right of
        NXL,     // &> - does not extend left of
        ADJ,     // -|- - is adjacent to
        ISDISTINCT  // IS DISTINCT FROM
    }
}
