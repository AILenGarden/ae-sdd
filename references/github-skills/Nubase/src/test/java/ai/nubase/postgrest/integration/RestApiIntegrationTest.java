package ai.nubase.postgrest.integration;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.EnabledIfEnvironmentVariable;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

/**
 * Integration tests for REST API
 * These tests require a running PostgreSQL database
 */
@SpringBootTest
@AutoConfigureMockMvc
@EnabledIfEnvironmentVariable(named = "RUN_INTEGRATION_TESTS", matches = "true")
class RestApiIntegrationTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private ObjectMapper objectMapper;

    @Test
    void testGetAllUsers() throws Exception {
        mockMvc.perform(get("/users"))
            .andExpect(status().isOk())
            .andExpect(content().contentType(MediaType.APPLICATION_JSON));
    }

    @Test
    void testGetUsersWithFilter() throws Exception {
        mockMvc.perform(get("/users?age.gte=18"))
            .andExpect(status().isOk())
            .andExpect(content().contentType(MediaType.APPLICATION_JSON));
    }

    @Test
    void testGetUsersWithSelect() throws Exception {
        mockMvc.perform(get("/users?select=id,name,email"))
            .andExpect(status().isOk())
            .andExpect(content().contentType(MediaType.APPLICATION_JSON));
    }

    @Test
    void testGetUsersWithOrdering() throws Exception {
        mockMvc.perform(get("/users?order=created_at.desc"))
            .andExpect(status().isOk())
            .andExpect(content().contentType(MediaType.APPLICATION_JSON));
    }

    @Test
    void testGetUsersWithPagination() throws Exception {
        mockMvc.perform(get("/users")
                .header("Range", "items=0-9"))
            .andExpect(status().isOk())
            .andExpect(header().exists("Content-Range"));
    }

    @Test
    void testCreateUser() throws Exception {
        String newUser = objectMapper.writeValueAsString(
            java.util.Map.of(
                "name", "Test User",
                "email", "test@example.com",
                "age", 25
            )
        );

        mockMvc.perform(post("/users")
                .contentType(MediaType.APPLICATION_JSON)
                .content(newUser))
            .andExpect(status().is2xxSuccessful());
    }

    @Test
    void testCreateUserWithReturnRepresentation() throws Exception {
        String newUser = objectMapper.writeValueAsString(
            java.util.Map.of(
                "name", "Test User 2",
                "email", "test2@example.com"
            )
        );

        mockMvc.perform(post("/users")
                .contentType(MediaType.APPLICATION_JSON)
                .header("Prefer", "return=representation")
                .content(newUser))
            .andExpect(status().isOk())
            .andExpect(content().contentType(MediaType.APPLICATION_JSON))
            .andExpect(jsonPath("$").isArray());
    }

    @Test
    void testUpdateUser() throws Exception {
        String updateData = objectMapper.writeValueAsString(
            java.util.Map.of("name", "Updated Name")
        );

        mockMvc.perform(patch("/users?id=eq.1")
                .contentType(MediaType.APPLICATION_JSON)
                .content(updateData))
            .andExpect(status().is2xxSuccessful());
    }

    @Test
    void testDeleteUser() throws Exception {
        mockMvc.perform(delete("/users?id=eq.999"))
            .andExpect(status().is2xxSuccessful());
    }

    @Test
    void testUpsertUser() throws Exception {
        String userData = objectMapper.writeValueAsString(
            java.util.Map.of(
                "id", 100,
                "name", "Upsert User",
                "email", "upsert@example.com"
            )
        );

        mockMvc.perform(put("/users?on_conflict=id")
                .contentType(MediaType.APPLICATION_JSON)
                .content(userData))
            .andExpect(status().is2xxSuccessful());
    }

    @Test
    void testRpcCall() throws Exception {
        String params = objectMapper.writeValueAsString(
            java.util.Map.of(
                "search_term", "Test",
                "min_age", 18
            )
        );

        mockMvc.perform(post("/rpc/search_users")
                .contentType(MediaType.APPLICATION_JSON)
                .content(params))
            .andExpect(status().isOk());
    }

    @Test
    void testSchemaReload() throws Exception {
        mockMvc.perform(post("/-/reload"))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.message").exists());
    }

    @Test
    void testOptionsRequest() throws Exception {
        mockMvc.perform(options("/users"))
            .andExpect(status().isOk())
            .andExpect(header().exists("Access-Control-Allow-Methods"));
    }

    @Test
    void testHeadRequest() throws Exception {
        mockMvc.perform(head("/users"))
            .andExpect(status().isOk());
    }
}
