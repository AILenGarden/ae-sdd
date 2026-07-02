package ai.nubase.auth.controller;

import ai.nubase.common.context.MultiTenancyContext;
import jakarta.servlet.http.HttpServletRequest;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.mock.web.MockHttpServletRequest;

import java.lang.reflect.Method;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Pure unit test for {@link PostgrestController#resolveSchema} — no Spring context, no DB.
 *
 * Uses reflection to invoke the private method directly so we can verify header parsing
 * and identifier validation in isolation.
 */
class PostgrestSchemaResolutionTest {

    private final PostgrestController controller = new PostgrestController(
            null, null, null, null, null);

    private final MultiTenancyContext.ContextData ctx = MultiTenancyContext.ContextData.builder()
            .schemaName("public")
            .build();

    private String resolve(HttpServletRequest req) throws Exception {
        Method m = PostgrestController.class.getDeclaredMethod(
                "resolveSchema", HttpServletRequest.class, MultiTenancyContext.ContextData.class);
        m.setAccessible(true);
        return (String) m.invoke(controller, req, ctx);
    }

    @Test
    @DisplayName("no profile header → default schema")
    void noHeaderUsesDefault() throws Exception {
        MockHttpServletRequest req = new MockHttpServletRequest("GET", "/rest/v1/users");
        assertThat(resolve(req)).isEqualTo("public");
    }

    @Test
    @DisplayName("GET reads Accept-Profile to override")
    void getReadsAcceptProfile() throws Exception {
        MockHttpServletRequest req = new MockHttpServletRequest("GET", "/rest/v1/users");
        req.addHeader("Accept-Profile", "auth");
        assertThat(resolve(req)).isEqualTo("auth");
    }

    @Test
    @DisplayName("HEAD also reads Accept-Profile")
    void headReadsAcceptProfile() throws Exception {
        MockHttpServletRequest req = new MockHttpServletRequest("HEAD", "/rest/v1/users");
        req.addHeader("Accept-Profile", "storage");
        assertThat(resolve(req)).isEqualTo("storage");
    }

    @Test
    @DisplayName("POST reads Content-Profile (not Accept-Profile)")
    void postReadsContentProfile() throws Exception {
        MockHttpServletRequest req = new MockHttpServletRequest("POST", "/rest/v1/users");
        req.addHeader("Content-Profile", "auth");
        req.addHeader("Accept-Profile", "ignored");
        assertThat(resolve(req)).isEqualTo("auth");
    }

    @Test
    @DisplayName("PATCH / PUT / DELETE all read Content-Profile")
    void writeMethodsReadContentProfile() throws Exception {
        for (String method : new String[]{"PATCH", "PUT", "DELETE"}) {
            MockHttpServletRequest req = new MockHttpServletRequest(method, "/rest/v1/users");
            req.addHeader("Content-Profile", "auth");
            assertThat(resolve(req)).as("method=%s", method).isEqualTo("auth");
        }
    }

    @Test
    @DisplayName("invalid identifier in header is rejected; falls back to default")
    void invalidIdentifierRejected() throws Exception {
        MockHttpServletRequest req = new MockHttpServletRequest("GET", "/rest/v1/users");
        req.addHeader("Accept-Profile", "drop table; --");
        assertThat(resolve(req)).isEqualTo("public");
    }

    @Test
    @DisplayName("empty / blank header falls back to default")
    void blankHeaderFallsBack() throws Exception {
        MockHttpServletRequest req = new MockHttpServletRequest("GET", "/rest/v1/users");
        req.addHeader("Accept-Profile", "   ");
        assertThat(resolve(req)).isEqualTo("public");
    }

    @Test
    @DisplayName("identifier longer than 63 chars is rejected")
    void tooLongIdentifierRejected() throws Exception {
        MockHttpServletRequest req = new MockHttpServletRequest("GET", "/rest/v1/users");
        req.addHeader("Accept-Profile", "a".repeat(64));
        assertThat(resolve(req)).isEqualTo("public");
    }

    @Test
    @DisplayName("legal identifier with digits / underscore accepted")
    void legalIdentifierAccepted() throws Exception {
        MockHttpServletRequest req = new MockHttpServletRequest("GET", "/rest/v1/users");
        req.addHeader("Accept-Profile", "tenant_42");
        assertThat(resolve(req)).isEqualTo("tenant_42");
    }
}
