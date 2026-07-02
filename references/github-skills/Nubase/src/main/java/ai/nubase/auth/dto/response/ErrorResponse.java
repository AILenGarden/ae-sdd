package ai.nubase.auth.dto.response;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class ErrorResponse {

    private String error;

    @JsonProperty("error_description")
    private String errorDescription;

    private String message;

    public static ErrorResponse of(String error, String errorDescription) {
        return ErrorResponse.builder()
                .error(error)
                .errorDescription(errorDescription)
                .build();
    }

    public static ErrorResponse of(String error, String errorDescription, String message) {
        return ErrorResponse.builder()
                .error(error)
                .errorDescription(errorDescription)
                .message(message)
                .build();
    }
}
