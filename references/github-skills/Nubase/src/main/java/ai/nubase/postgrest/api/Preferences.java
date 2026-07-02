package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

/**
 * Request preferences from Prefer header
 * Examples:
 * - Prefer: return=representation
 * - Prefer: return=minimal
 * - Prefer: count=exact
 */
@Data
@Builder
public class Preferences {
    private ReturnPreference returnPreference;
    private CountPreference countPreference;
    private Resolution resolution;
    private TransactionPreference transactionPreference;
    private HandlingPreference handlingPreference;
    private MissingPreference missingPreference;
    private String timezone;
    private Integer maxRows;

    public enum ReturnPreference {
        REPRESENTATION,  // Return the modified resource
        MINIMAL,         // Return no body
        HEADERS_ONLY     // Return only headers
    }

    public enum CountPreference {
        NONE,      // No count
        EXACT,     // Exact count (can be slow)
        PLANNED,   // Estimated count from query planner
        ESTIMATED  // Estimated count
    }

    public enum Resolution {
        MERGE_DUPLICATES,
        IGNORE_DUPLICATES
    }

    public enum TransactionPreference {
        COMMIT,
        ROLLBACK
    }

    public enum HandlingPreference {
        STRICT,
        LENIENT
    }

    /**
     * How to handle missing columns in INSERT/UPDATE.
     * missing=default: Use column's DEFAULT value instead of NULL
     */
    public enum MissingPreference {
        NULL,    // Default: missing columns become NULL
        DEFAULT  // Use column's DEFAULT value for missing columns
    }
}
