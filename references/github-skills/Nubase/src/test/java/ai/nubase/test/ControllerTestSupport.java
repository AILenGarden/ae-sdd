package ai.nubase.test;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.http.converter.json.MappingJackson2HttpMessageConverter;
import org.springframework.validation.FieldError;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.setup.MockMvcBuilders;

import java.util.Map;
import java.util.stream.Collectors;

public final class ControllerTestSupport {

    public static final ObjectMapper OBJECT_MAPPER = new ObjectMapper()
            .registerModule(new JavaTimeModule());

    private ControllerTestSupport() {
    }

    public static MockMvc mockMvc(Object controller) {
        return MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new TestExceptionHandler())
                .setMessageConverters(new MappingJackson2HttpMessageConverter(OBJECT_MAPPER))
                .build();
    }

    public static String json(Object value) throws Exception {
        return OBJECT_MAPPER.writeValueAsString(value);
    }

    @RestControllerAdvice
    static class TestExceptionHandler {

        @ExceptionHandler(MethodArgumentNotValidException.class)
        ResponseEntity<Map<String, Object>> validation(MethodArgumentNotValidException ex) {
            Map<String, String> fields = ex.getBindingResult().getFieldErrors().stream()
                    .collect(Collectors.toMap(
                            FieldError::getField,
                            FieldError::getDefaultMessage,
                            (left, right) -> left));
            return ResponseEntity.badRequest().body(Map.of(
                    "error", "validation_failed",
                    "fields", fields));
        }

        @ExceptionHandler(IllegalArgumentException.class)
        ResponseEntity<Map<String, Object>> illegalArgument(IllegalArgumentException ex) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST).body(Map.of(
                    "error", "bad_request",
                    "message", ex.getMessage()));
        }
    }
}
