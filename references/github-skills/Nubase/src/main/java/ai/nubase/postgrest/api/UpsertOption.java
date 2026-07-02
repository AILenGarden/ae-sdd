package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

import java.util.List;

/**
 * UPSERT options for PUT requests
 * Handles ON CONFLICT resolution
 */
@Data
@Builder
public class UpsertOption {
    private List<String> conflictColumns; // Columns for ON CONFLICT
    private Resolution resolution;

    public enum Resolution {
        MERGE_DUPLICATES,    // UPDATE on conflict
        IGNORE_DUPLICATES    // DO NOTHING on conflict
    }
}
