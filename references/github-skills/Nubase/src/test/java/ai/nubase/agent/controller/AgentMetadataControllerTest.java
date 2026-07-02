package ai.nubase.agent.controller;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.test.web.servlet.MockMvc;

import static ai.nubase.test.ControllerTestSupport.mockMvc;
import static org.hamcrest.Matchers.containsString;
import static org.hamcrest.Matchers.not;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.content;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

class AgentMetadataControllerTest {

    private MockMvc mvc;

    @BeforeEach
    void setUp() {
        mvc = mockMvc(new AgentMetadataController());
    }

    @Test
    void instructionsReturnsAgentGuidanceWithoutSecrets() throws Exception {
        mvc.perform(get("/agent/v1/instructions"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.instructions").value(containsString("Memory")))
                .andExpect(jsonPath("$.instructions").value(containsString("REST")))
                .andExpect(jsonPath("$.instructions").value(containsString("Auth")))
                .andExpect(jsonPath("$.instructions").value(containsString("Storage")))
                .andExpect(jsonPath("$.instructions").value(containsString("MCP")))
                .andExpect(content().string(not(containsString("service_role="))))
                .andExpect(content().string(not(containsString("password="))));
    }

    @Test
    void capabilitiesReturnsStablePublicPaths() throws Exception {
        mvc.perform(get("/agent/v1/capabilities"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Nubase"))
                .andExpect(jsonPath("$.services.memory.enabled").value(true))
                .andExpect(jsonPath("$.services.memory.basePath").value("/mem/v1"))
                .andExpect(jsonPath("$.services.database.enabled").value(true))
                .andExpect(jsonPath("$.services.database.basePath").value("/rest/v1"))
                .andExpect(jsonPath("$.services.auth.enabled").value(true))
                .andExpect(jsonPath("$.services.auth.basePath").value("/auth/v1"))
                .andExpect(jsonPath("$.services.storage.enabled").value(true))
                .andExpect(jsonPath("$.services.storage.basePath").value("/storage/v1"))
                .andExpect(jsonPath("$.services.mcp.enabled").value(true))
                .andExpect(jsonPath("$.services.mcp.endpoint").value("/mcp"))
                .andExpect(jsonPath("$.services.aiGateway.enabled").value(true))
                .andExpect(jsonPath("$.services.aiGateway.openAIBasePath").value("/v1"))
                .andExpect(jsonPath("$.services.aiGateway.anthropicMessagesPath").value("/v1/messages"));
    }

    @Test
    void connectConfigReturnsClientTemplateWithoutRealSecrets() throws Exception {
        mvc.perform(get("/agent/v1/connect-config").param("client", "codex"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.client").value("codex"))
                .andExpect(jsonPath("$.mcp.endpoint").value("http://localhost:9999/mcp"))
                .andExpect(jsonPath("$.mcp.headers.apikey").value("YOUR_NUBASE_PROJECT_KEY"))
                .andExpect(jsonPath("$.env.NUBASE_URL").value("http://localhost:9999"))
                .andExpect(jsonPath("$.env.NUBASE_API_KEY").value("YOUR_NUBASE_PROJECT_KEY"))
                .andExpect(jsonPath("$.aiGateway.openAI.baseUrl").value("http://localhost:9999/v1"))
                .andExpect(jsonPath("$.aiGateway.openAI.apiKey").value("YOUR_NUBASE_AI_GATEWAY_KEY"))
                .andExpect(jsonPath("$.aiGateway.anthropic.baseUrl").value("http://localhost:9999"))
                .andExpect(jsonPath("$.aiGateway.anthropic.authToken").value("YOUR_NUBASE_AI_GATEWAY_KEY"))
                .andExpect(jsonPath("$.templates.mcpServers.nubase.url").value("http://localhost:9999/mcp"))
                .andExpect(content().string(org.hamcrest.Matchers.not(org.hamcrest.Matchers.containsString("service_role"))));
    }

    @Test
    void connectConfigFallsBackToGenericClient() throws Exception {
        mvc.perform(get("/agent/v1/connect-config").param("client", "unknown-client"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.client").value("generic"));
    }
}
