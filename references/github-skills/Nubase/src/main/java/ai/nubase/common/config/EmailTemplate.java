package ai.nubase.common.config;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * A customizable transactional email template (subject + HTML body). Bodies use simple
 * {@code {{ .Variable }}} placeholders (e.g. {@code {{ .ConfirmationURL }}}, {@code {{ .Token }}},
 * {@code {{ .Email }}}). Per-tenant overrides live in {@link TenantAuthConfig#getEmailTemplates()};
 * the built-in defaults are defined in {@code EmailTemplateService}.
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
@JsonIgnoreProperties(ignoreUnknown = true)
public class EmailTemplate {
    private String subject;
    private String content;
}
