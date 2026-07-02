package ai.nubase.auth.dto.request.admin;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Size;
import lombok.Data;

@Data
public class SaveSnippetRequest {

    @NotBlank
    @Size(max = 255)
    private String name;

    @NotBlank
    private String query;
}
