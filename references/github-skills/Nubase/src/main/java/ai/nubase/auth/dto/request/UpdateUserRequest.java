package ai.nubase.auth.dto.request;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Email;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;

@Data
@NoArgsConstructor
@AllArgsConstructor
public class UpdateUserRequest {

    @Email(message = "Invalid email format")
    private String email;

    private String password;

    private String phone;

    @JsonProperty("data")
    private Map<String, Object> data;  // User metadata to update

    /** Reauthentication nonce (from /reauthenticate); required for password change when
     *  password.requireReauthentication is enabled. */
    private String nonce;
}
