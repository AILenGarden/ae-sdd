package ai.nubase.mem.dto;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * Standard envelope for paginated list responses across the mem module.
 *
 * <p>Hand-rolled instead of Spring Data {@code Page} to avoid serializing Spring internals
 * (sort, pageable, etc.) into the JSON payload — the frontend only needs total + items.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class PagedResponse<T> {

    /** Rows on the current page. */
    private List<T> items;

    /** Total rows matching the filter, across all pages. */
    private long total;

    /** 1-based page number that was returned. */
    private int page;

    /** Page size that was used. */
    private int pageSize;

    public static <T> PagedResponse<T> of(List<T> items, long total, int page, int pageSize) {
        return PagedResponse.<T>builder()
                .items(items)
                .total(total)
                .page(page)
                .pageSize(pageSize)
                .build();
    }
}
