package ai.nubase.auth.dto.request.admin;

import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;
import lombok.Data;

@Data
public class AddMemberRequest {

    @NotBlank
    @Email
    private String email;

    /** 'owner' | 'member' — defaults to 'member' if blank. */
    private String role;
}
