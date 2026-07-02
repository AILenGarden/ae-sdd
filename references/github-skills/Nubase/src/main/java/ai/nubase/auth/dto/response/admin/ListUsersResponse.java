package ai.nubase.auth.dto.response.admin;

import ai.nubase.auth.dto.response.UserResponse;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * Response DTO for listing users via Admin API.
 * Compatible with Supabase listUsers response format.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ListUsersResponse {

    /**
     * List of users
     */
    private List<UserResponse> users;

    /**
     * Audience claim (deprecated in Supabase but included for compatibility)
     */
    private String aud;

    /**
     * Total count of users (for pagination)
     */
    @JsonProperty("total")
    private Long total;

    /**
     * Current page number
     */
    @JsonProperty("page")
    private Integer page;

    /**
     * Number of users per page
     */
    @JsonProperty("per_page")
    private Integer perPage;
}
