package ai.nubase.auth.service;

import ai.nubase.common.config.EmailTemplate;
import lombok.RequiredArgsConstructor;
import org.apache.commons.lang3.StringUtils;
import org.springframework.stereotype.Service;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Resolves and renders transactional auth emails. Each template type has a built-in default
 * (subject + HTML body); a tenant may override either via {@code auth_config.emailTemplates}
 * (see {@link EffectiveAuthConfig#emailTemplateOverride(String)}).
 *
 * <p>Bodies use {@code {{ .Variable }}} placeholders, substituted at send time.
 */
@Service
@RequiredArgsConstructor
public class EmailTemplateService {

    // Template types (keys in auth_config.emailTemplates).
    public static final String CONFIRMATION = "confirmation";
    public static final String RECOVERY = "recovery";
    public static final String INVITE = "invite";
    public static final String MAGIC_LINK = "magic_link";
    public static final String EMAIL_CHANGE = "email_change";
    public static final String REAUTHENTICATION = "reauthentication";

    public static final List<String> TYPES = List.of(
            CONFIRMATION, RECOVERY, INVITE, MAGIC_LINK, EMAIL_CHANGE, REAUTHENTICATION);

    private final EffectiveAuthConfig effectiveAuthConfig;

    /** A rendered email ready to send. */
    public record Rendered(String subject, String body) {}

    /**
     * The effective (tenant override or built-in default) raw template for a type — used by the
     * admin editor so it can show/edit the current template, defaults included.
     */
    public EmailTemplate effective(String type) {
        EmailTemplate override = effectiveAuthConfig.emailTemplateOverride(type);
        if (override != null
                && StringUtils.isNotBlank(override.getSubject())
                && StringUtils.isNotBlank(override.getContent())) {
            return override;
        }
        return defaultFor(type);
    }

    /** Render a type with the given variables substituted. */
    public Rendered render(String type, Map<String, String> vars) {
        EmailTemplate t = effective(type);
        return new Rendered(substitute(t.getSubject(), vars), substitute(t.getContent(), vars));
    }

    /** The supported {@code {{ .X }}} variable names for a template type (for the editor UI). */
    public List<String> variablesFor(String type) {
        return switch (type) {
            case CONFIRMATION, RECOVERY, INVITE -> List.of("ConfirmationURL", "Email", "SiteURL");
            case MAGIC_LINK -> List.of("ConfirmationURL", "Token", "Email", "SiteURL");
            case EMAIL_CHANGE -> List.of("ConfirmationURL", "Email", "NewEmail", "SiteURL");
            case REAUTHENTICATION -> List.of("Token", "Email");
            default -> List.of();
        };
    }

    /** All built-in defaults, keyed by type. */
    public Map<String, EmailTemplate> defaults() {
        Map<String, EmailTemplate> map = new LinkedHashMap<>();
        for (String t : TYPES) {
            map.put(t, defaultFor(t));
        }
        return map;
    }

    public EmailTemplate defaultFor(String type) {
        return switch (type) {
            case CONFIRMATION -> new EmailTemplate("Confirm your email",
                    card("Confirm your email address",
                            "Thanks for signing up for <strong>{{ .SiteURL }}</strong>. Confirm "
                                    + "<strong>{{ .Email }}</strong> to activate your account.",
                            "Confirm Email", "{{ .ConfirmationURL }}"));
            case RECOVERY -> new EmailTemplate("Reset your password",
                    card("Reset your password",
                            "We received a request to reset the password for "
                                    + "<strong>{{ .Email }}</strong>. If this was you, click below.",
                            "Reset Password", "{{ .ConfirmationURL }}"));
            case INVITE -> new EmailTemplate("You have been invited",
                    card("You have been invited",
                            "You have been invited to join <strong>{{ .SiteURL }}</strong>. "
                                    + "Accept the invitation to set up your account.",
                            "Accept Invite", "{{ .ConfirmationURL }}"));
            case MAGIC_LINK -> new EmailTemplate("Your magic link",
                    card("Your magic link",
                            "Click the button to sign in to <strong>{{ .SiteURL }}</strong>, or use "
                                    + "this code: <strong>{{ .Token }}</strong>.",
                            "Sign in", "{{ .ConfirmationURL }}"));
            case EMAIL_CHANGE -> new EmailTemplate("Confirm your new email",
                    card("Confirm your new email",
                            "Confirm changing your email to <strong>{{ .NewEmail }}</strong>.",
                            "Confirm Email Change", "{{ .ConfirmationURL }}"));
            case REAUTHENTICATION -> new EmailTemplate("Confirm it's you",
                    "<p>Use this code to confirm your identity: <strong>{{ .Token }}</strong></p>"
                            + "<p>If you didn't request this, please secure your account.</p>");
            default -> throw new IllegalArgumentException("Unknown email template type: " + type);
        };
    }

    // Minimal responsive-ish card used by the default templates.
    private static String card(String heading, String bodyHtml, String buttonLabel, String buttonHref) {
        return """
                <!DOCTYPE html>
                <html lang="en"><head><meta charset="UTF-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1.0"/></head>
                <body style="margin:0;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
                  <div style="max-width:480px;margin:0 auto;padding:32px 20px;">
                    <h1 style="margin:0 0 12px;font-size:22px;font-weight:600;color:#171717;">%s</h1>
                    <p style="margin:0 0 20px;font-size:15px;line-height:1.6;color:#4a5568;">%s</p>
                    <p style="margin:0 0 24px;">
                      <a href="%s" target="_blank" rel="noopener noreferrer"
                         style="display:inline-block;padding:10px 24px;background:#171717;color:#fff;
                                text-decoration:none;border-radius:6px;font-size:14px;font-weight:500;">%s</a>
                    </p>
                    <p style="margin:0;font-size:13px;color:#737373;">
                      If you didn't request this, no action is required. This link will expire.
                    </p>
                  </div>
                </body></html>
                """.formatted(heading, bodyHtml, buttonHref, buttonLabel);
    }

    private static String substitute(String template, Map<String, String> vars) {
        if (template == null) {
            return "";
        }
        String out = template;
        for (Map.Entry<String, String> e : vars.entrySet()) {
            // matches {{ .Key }} with any inner whitespace
            String pattern = "\\{\\{\\s*\\." + Pattern.quote(e.getKey()) + "\\s*\\}\\}";
            out = out.replaceAll(pattern, Matcher.quoteReplacement(e.getValue() == null ? "" : e.getValue()));
        }
        return out;
    }
}
