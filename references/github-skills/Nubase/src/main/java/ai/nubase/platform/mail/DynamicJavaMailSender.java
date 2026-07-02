package ai.nubase.platform.mail;

import ai.nubase.platform.event.SettingsChangedEvent;
import ai.nubase.platform.service.PlatformSettingsService;
import jakarta.annotation.PostConstruct;
import jakarta.mail.internet.MimeMessage;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Primary;
import org.springframework.context.event.EventListener;
import org.springframework.core.io.InputStreamSource;
import org.springframework.mail.MailException;
import org.springframework.mail.SimpleMailMessage;
import org.springframework.mail.javamail.JavaMailSender;
import org.springframework.mail.javamail.JavaMailSenderImpl;
import org.springframework.stereotype.Component;

import java.io.InputStream;
import java.util.Properties;

/**
 * {@link JavaMailSender} backed by runtime-editable {@link PlatformSettingsService} values
 * (category {@code smtp}), with YAML defaults under {@code spring.mail.*} as a fallback.
 *
 * <p>The wrapped {@link JavaMailSenderImpl} is rebuilt on construction and whenever a
 * {@link SettingsChangedEvent} for {@code category=smtp} fires, so a super admin can rotate
 * SMTP credentials without a restart.
 *
 * <p>Marked {@link Primary} so it wins over the bean Spring Boot auto-configures from
 * {@code spring.mail.*}.
 */
@Slf4j
@Component
@Primary
@RequiredArgsConstructor
public class DynamicJavaMailSender implements JavaMailSender {

    public static final String CATEGORY = "smtp";

    private final PlatformSettingsService settingsService;

    @Value("${spring.mail.host:}")
    private String defaultHost;

    @Value("${spring.mail.port:587}")
    private int defaultPort;

    @Value("${spring.mail.username:}")
    private String defaultUsername;

    @Value("${spring.mail.password:}")
    private String defaultPassword;

    /** Volatile so a rebuild on settings change is visible to sends in flight. */
    private volatile JavaMailSenderImpl delegate;

    @PostConstruct
    void init() {
        rebuild();
    }

    @EventListener
    public void onSettingsChanged(SettingsChangedEvent event) {
        if (CATEGORY.equals(event.getCategory())) {
            log.info("SMTP settings changed; rebuilding mail sender");
            rebuild();
        }
    }

    private synchronized void rebuild() {
        java.util.Map<String, String> s = settingsService.getCategory(CATEGORY);

        String host = firstNonBlank(s.get("host"), defaultHost);
        int port = parseInt(s.get("port"), defaultPort);
        String username = firstNonBlank(s.get("username"), defaultUsername);
        String password = firstNonBlank(s.get("password"), defaultPassword);
        boolean starttls = parseBool(s.get("starttls_enabled"), true);
        boolean auth = parseBool(s.get("auth"), username != null && !username.isEmpty());

        JavaMailSenderImpl impl = new JavaMailSenderImpl();
        impl.setHost(host);
        impl.setPort(port);
        impl.setUsername(username);
        impl.setPassword(password);
        impl.setDefaultEncoding("UTF-8");

        Properties props = impl.getJavaMailProperties();
        props.put("mail.transport.protocol", "smtp");
        props.put("mail.smtp.auth", Boolean.toString(auth));
        props.put("mail.smtp.starttls.enable", Boolean.toString(starttls));

        this.delegate = impl;
        log.info("Mail sender rebuilt: host={} port={} starttls={} auth={} username={}",
                host == null || host.isEmpty() ? "<unset>" : host,
                port, starttls, auth,
                username == null || username.isEmpty() ? "<unset>" : "***");
    }

    private JavaMailSenderImpl current() {
        JavaMailSenderImpl d = delegate;
        if (d == null) {
            throw new IllegalStateException("Mail sender not initialised");
        }
        return d;
    }

    // ----- JavaMailSender delegations -----

    @Override
    public MimeMessage createMimeMessage() {
        return current().createMimeMessage();
    }

    @Override
    public MimeMessage createMimeMessage(InputStream contentStream) throws MailException {
        return current().createMimeMessage(contentStream);
    }

    @Override
    public void send(MimeMessage mimeMessage) throws MailException {
        current().send(mimeMessage);
    }

    @Override
    public void send(MimeMessage... mimeMessages) throws MailException {
        current().send(mimeMessages);
    }

    @Override
    public void send(org.springframework.mail.javamail.MimeMessagePreparator mimeMessagePreparator)
            throws MailException {
        current().send(mimeMessagePreparator);
    }

    @Override
    public void send(org.springframework.mail.javamail.MimeMessagePreparator... mimeMessagePreparators)
            throws MailException {
        current().send(mimeMessagePreparators);
    }

    @Override
    public void send(SimpleMailMessage simpleMessage) throws MailException {
        current().send(simpleMessage);
    }

    @Override
    public void send(SimpleMailMessage... simpleMessages) throws MailException {
        current().send(simpleMessages);
    }

    // ----- helpers -----

    private static String firstNonBlank(String a, String b) {
        return (a != null && !a.isEmpty()) ? a : b;
    }

    private static int parseInt(String value, int fallback) {
        if (value == null || value.isEmpty()) return fallback;
        try {
            return Integer.parseInt(value.trim());
        } catch (NumberFormatException e) {
            return fallback;
        }
    }

    private static boolean parseBool(String value, boolean fallback) {
        if (value == null || value.isEmpty()) return fallback;
        return "true".equalsIgnoreCase(value.trim()) || "1".equals(value.trim());
    }

    /**
     * Helper for callers (e.g. EmailService) — current from-address either from settings or
     * from the supplied YAML fallback.
     */
    public String resolveFromAddress(String yamlDefault) {
        String settingsValue = settingsService.get(CATEGORY, "from_address");
        return firstNonBlank(settingsValue, yamlDefault);
    }

    public String resolveFromName(String yamlDefault) {
        String settingsValue = settingsService.get(CATEGORY, "from_name");
        return firstNonBlank(settingsValue, yamlDefault);
    }
}
