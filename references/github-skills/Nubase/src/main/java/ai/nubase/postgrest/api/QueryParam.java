package ai.nubase.postgrest.api;

import lombok.Builder;
import lombok.Data;

/**
 * Query parameter representation
 */
@Data
@Builder
public class QueryParam {
    private String name;
    private String value;
}
