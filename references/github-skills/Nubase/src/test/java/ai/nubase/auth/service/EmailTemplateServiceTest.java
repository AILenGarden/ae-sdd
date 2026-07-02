package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.config.EmailTemplate;
import ai.nubase.common.config.TenantAuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Unit tests for {@link EmailTemplateService}: defaults, variable substitution, tenant override.
 */
@DisplayName("EmailTemplateService")
class EmailTemplateServiceTest {

    private final EmailTemplateService svc =
            new EmailTemplateService(new EffectiveAuthConfig(new AuthConfig()));

    @AfterEach
    void clear() {
        MultiTenancyContext.clear();
    }

    private void setTemplates(Map<String, EmailTemplate> templates) {
        TenantAuthConfig tac = new TenantAuthConfig();
        tac.setEmailTemplates(templates);
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder()
                .appCode("demo").schemaName("public").jwtSecret("s").tenantAuthConfig(tac).build());
    }

    @Test
    @DisplayName("renders built-in default with variables substituted")
    void rendersDefault() {
        EmailTemplateService.Rendered r = svc.render(EmailTemplateService.CONFIRMATION,
                Map.of("ConfirmationURL", "https://app/cb?t=1", "Email", "a@b.com", "SiteURL", "Acme"));
        assertThat(r.subject()).isEqualTo("Confirm your email");
        assertThat(r.body()).contains("https://app/cb?t=1").contains("a@b.com").contains("Acme");
        assertThat(r.body()).doesNotContain("{{");
    }

    @Test
    @DisplayName("substitution tolerates whitespace variants")
    void whitespaceTolerant() {
        setTemplates(Map.of("reauthentication",
                new EmailTemplate("Code", "a={{ .Token }} b={{.Token}} c={{  .Token  }}")));
        EmailTemplateService.Rendered r = svc.render(EmailTemplateService.REAUTHENTICATION, Map.of("Token", "123456"));
        assertThat(r.subject()).isEqualTo("Code");
        assertThat(r.body()).isEqualTo("a=123456 b=123456 c=123456");
    }

    @Test
    @DisplayName("tenant override wins over the default")
    void overrideWins() {
        setTemplates(Map.of("recovery", new EmailTemplate("Reset now", "Hello {{ .Email }}")));
        EmailTemplateService.Rendered r = svc.render(EmailTemplateService.RECOVERY, Map.of("Email", "z@z.com"));
        assertThat(r.subject()).isEqualTo("Reset now");
        assertThat(r.body()).isEqualTo("Hello z@z.com");
        // a type without an override still uses the default
        assertThat(svc.effective(EmailTemplateService.INVITE).getSubject()).isEqualTo("You have been invited");
    }

    @Test
    @DisplayName("blank override falls back to the default")
    void blankOverrideFallsBack() {
        setTemplates(Map.of("recovery", new EmailTemplate("", "")));
        assertThat(svc.effective(EmailTemplateService.RECOVERY).getSubject()).isEqualTo("Reset your password");
    }

    @Test
    @DisplayName("variablesFor exposes the supported placeholders")
    void variables() {
        assertThat(svc.variablesFor(EmailTemplateService.MAGIC_LINK)).contains("Token", "ConfirmationURL");
        assertThat(svc.variablesFor(EmailTemplateService.EMAIL_CHANGE)).contains("NewEmail");
        assertThat(svc.defaults()).containsKeys(
                EmailTemplateService.CONFIRMATION, EmailTemplateService.REAUTHENTICATION);
    }
}
