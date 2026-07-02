package ai.nubase.auth.controller;

import ai.nubase.auth.service.EffectiveAuthConfig;
import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.config.TenantAuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.postgrest.multidb.DatabaseConfig;
import ai.nubase.postgrest.multidb.DatabaseConfigRepository;
import cn.hutool.json.JSONUtil;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.setup.MockMvcBuilders;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

class AuthSettingsAdminControllerTest {

    private DatabaseConfigRepository repository;
    private MockMvc mvc;

    @BeforeEach
    void setUp() {
        MultiTenancyContext.clear();
        repository = mock(DatabaseConfigRepository.class);
        mvc = MockMvcBuilders.standaloneSetup(new AuthSettingsAdminController(
                new EffectiveAuthConfig(new AuthConfig()),
                repository
        )).build();
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder()
                .databaseKey("appabc")
                .appCode("appabc")
                .schemaName("public")
                .jwtSecret("secret")
                .build());
    }

    @AfterEach
    void tearDown() {
        MultiTenancyContext.clear();
    }

    @Test
    void managedRedirectAllowListReplacesNamespaceOnly() throws Exception {
        TenantAuthConfig stored = new TenantAuthConfig();
        AuthConfig.RedirectSettings redirect = new AuthConfig.RedirectSettings();
        redirect.setAllowList(List.of("https://manual.example.com/auth/callback"));
        stored.setRedirect(redirect);
        stored.setManagedRedirectAllowLists(Map.of(
                "other", List.of("https://other.example.com/auth/callback"),
                "ottermind", List.of("https://old.ottermind.app/auth/callback")
        ));
        when(repository.findByDbKey("appabc")).thenReturn(DatabaseConfig.builder()
                .dbKey("appabc")
                .authConfigJson(JSONUtil.toJsonStr(stored))
                .build());

        mvc.perform(put("/auth/v1/admin/settings/auth/redirect-allowlist/ottermind")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {
                                  "urls": [
                                    "https://appabc.ottermind.app/auth/callback?ignored=1",
                                    "https://appabc.ottermind.app/auth/callback",
                                    "https://appabc-live.ottermind.app/auth/callback#ignored"
                                  ]
                                }
                                """))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.namespace").value("ottermind"))
                .andExpect(jsonPath("$.urls[0]").value("https://appabc.ottermind.app/auth/callback"))
                .andExpect(jsonPath("$.urls[1]").value("https://appabc-live.ottermind.app/auth/callback"));

        org.mockito.ArgumentCaptor<String> captor = org.mockito.ArgumentCaptor.forClass(String.class);
        verify(repository).updateAuthConfig(eq("appabc"), captor.capture());
        TenantAuthConfig updated = JSONUtil.parseObj(captor.getValue()).toBean(TenantAuthConfig.class);
        assertThat(updated.getRedirect().getAllowList())
                .containsExactly("https://manual.example.com/auth/callback");
        assertThat(updated.getManagedRedirectAllowLists().get("other"))
                .containsExactly("https://other.example.com/auth/callback");
        assertThat(updated.getManagedRedirectAllowLists().get("ottermind"))
                .containsExactly(
                        "https://appabc.ottermind.app/auth/callback",
                        "https://appabc-live.ottermind.app/auth/callback");
    }

    @Test
    void managedRedirectAllowListRejectsNonCallbackUrl() throws Exception {
        when(repository.findByDbKey("appabc")).thenReturn(DatabaseConfig.builder()
                .dbKey("appabc")
                .build());

        mvc.perform(put("/auth/v1/admin/settings/auth/redirect-allowlist/ottermind")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"urls\":[\"https://evil.com/phish\"]}"))
                .andExpect(status().isBadRequest());
    }
}
