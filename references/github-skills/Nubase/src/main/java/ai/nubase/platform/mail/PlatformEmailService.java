package ai.nubase.platform.mail;

import jakarta.mail.internet.MimeMessage;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.mail.javamail.JavaMailSender;
import org.springframework.mail.javamail.MimeMessageHelper;
import org.springframework.stereotype.Service;

/**
 * Sends one-time verification codes for platform (Studio) developer accounts.
 *
 * <p>Unlike {@link ai.nubase.auth.service.EmailService}, this path is <b>context-free</b>: platform
 * flows run against the metadata DB with no {@link ai.nubase.common.context.MultiTenancyContext}, so
 * this service only depends on the global {@code @Primary} {@link JavaMailSender}
 * ({@link DynamicJavaMailSender}) and the configured from-address.
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class PlatformEmailService {

    private final JavaMailSender mailSender;
    private final DynamicJavaMailSender dynamicMailSender;

    @Value("${nubase.auth.email.from-address:noreply@example.com}")
    private String fromAddress;

    @Value("${nubase.platform.app-name:Nubase}")
    private String appName;

    /** Reason a code was issued — drives the email copy. */
    public enum Purpose {
        SIGNUP("Confirm your %s account"),
        LOGIN("Your %s sign-in code"),
        PASSWORD_CHANGE("Confirm your %s password change");

        private final String subjectTemplate;

        Purpose(String subjectTemplate) {
            this.subjectTemplate = subjectTemplate;
        }

        String subject(String appName) {
            return String.format(subjectTemplate, appName);
        }
    }

    /**
     * Send a verification code. Failures are logged, not thrown — the caller has already persisted
     * the code; a transient SMTP error should not roll back the issuing transaction.
     */
    public void sendOtp(String to, String code, Purpose purpose, long ttlSeconds) {
        try {
            MimeMessage message = mailSender.createMimeMessage();
            MimeMessageHelper helper = new MimeMessageHelper(message, true, "UTF-8");
            helper.setFrom(dynamicMailSender.resolveFromAddress(fromAddress));
            helper.setTo(to);
            helper.setSubject(purpose.subject(appName));
            helper.setText(body(code, ttlSeconds), true);
            mailSender.send(message);
            log.info("Platform OTP email sent to {} (purpose={})", to, purpose);
        } catch (Exception e) {
            log.error("Failed to send platform OTP email to {} (purpose={})", to, purpose, e);
        }
    }

    private String body(String code, long ttlSeconds) {
        long minutes = Math.max(1, ttlSeconds / 60);
        return """
                <div style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;\
                max-width:480px;margin:0 auto;padding:32px 24px;color:#0f172a;">
                  <h2 style="font-size:18px;font-weight:600;margin:0 0 8px;">%s verification code</h2>
                  <p style="font-size:14px;color:#475569;margin:0 0 24px;">
                    Enter this code to continue. It expires in %d minutes.
                  </p>
                  <div style="font-size:32px;font-weight:700;letter-spacing:8px;text-align:center;\
                background:#f1f5f9;border-radius:8px;padding:16px 0;color:#0f172a;">%s</div>
                  <p style="font-size:12px;color:#94a3b8;margin:24px 0 0;">
                    If you didn't request this, you can safely ignore this email.
                  </p>
                </div>
                """.formatted(appName, minutes, code);
    }
}
