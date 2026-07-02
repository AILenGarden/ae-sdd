package ai.nubase.mem.llm;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Provider-neutral chat message.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class ChatMessage {

    /** "system", "user", "assistant". */
    private String role;

    /** Message content. */
    private String content;

    /** Optional actor name (mapped to OpenAI {@code name} field). */
    private String name;

    public static ChatMessage system(String content) {
        return ChatMessage.builder().role("system").content(content).build();
    }

    public static ChatMessage user(String content) {
        return ChatMessage.builder().role("user").content(content).build();
    }

    public static ChatMessage assistant(String content) {
        return ChatMessage.builder().role("assistant").content(content).build();
    }
}
