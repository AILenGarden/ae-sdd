package ai.nubase.auth.dto.response;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class AuthResponse {

    @JsonProperty("access_token")
    private String accessToken;

    @JsonProperty("token_type")
    private String tokenType;

    @JsonProperty("expires_in")
    private Integer expiresIn;

    @JsonProperty("expires_at")
    private Long expiresAt;

    @JsonProperty("refresh_token")
    private String refreshToken;

    private UserResponse user;

    public static AuthResponse success(String accessToken, String refreshToken,
                                      Integer expiresIn, UserResponse user) {
        long expiresAt = System.currentTimeMillis() / 1000 + expiresIn;

        return AuthResponse.builder()
                .accessToken(accessToken)
                .tokenType("bearer")
                .expiresIn(expiresIn)
                .expiresAt(expiresAt)
                .refreshToken(refreshToken)
                .user(user)
                .build();
    }
}
