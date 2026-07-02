package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Service;
import org.springframework.util.LinkedMultiValueMap;
import org.springframework.util.MultiValueMap;
import org.springframework.web.client.RestTemplate;

/**
 * Outbound SMS sender for phone OTP / phone-MFA codes.
 * <p>
 * Defaults to the {@code log} provider (prints the code to the log — handy for dev).
 * A minimal Twilio REST integration is included; set {@code nubase.auth.sms.provider=twilio}
 * with credentials to enable real delivery. Additional providers can be plugged in here.
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class SmsService {

    private final EffectiveAuthConfig effectiveAuthConfig;
    private final RestTemplate restTemplate = new RestTemplate();

    /**
     * Send an OTP code to a phone number.
     *
     * @return true if the message was dispatched (or logged in dev mode)
     */
    public boolean sendOtp(String phone, String code) {
        AuthConfig.SmsSettings sms = effectiveAuthConfig.sms();
        String message = "Your verification code is " + code;

        if (!sms.isEnabled() || "log".equalsIgnoreCase(sms.getProvider())) {
            log.info("[SMS:{}] to={} message=\"{}\"", sms.getProvider(), phone, message);
            return true;
        }

        if ("twilio".equalsIgnoreCase(sms.getProvider())) {
            return sendViaTwilio(sms, phone, message);
        }

        log.warn("Unknown SMS provider '{}' — falling back to log", sms.getProvider());
        log.info("[SMS:fallback] to={} message=\"{}\"", phone, message);
        return true;
    }

    private boolean sendViaTwilio(AuthConfig.SmsSettings sms, String phone, String message) {
        try {
            String url = "https://api.twilio.com/2010-04-01/Accounts/"
                    + sms.getAccountSid() + "/Messages.json";

            MultiValueMap<String, String> form = new LinkedMultiValueMap<>();
            form.add("To", phone);
            form.add("From", sms.getFromNumber());
            form.add("Body", message);

            org.springframework.http.HttpHeaders headers = new org.springframework.http.HttpHeaders();
            headers.setContentType(MediaType.APPLICATION_FORM_URLENCODED);
            headers.setBasicAuth(sms.getAccountSid(), sms.getAuthToken());

            restTemplate.postForEntity(url,
                    new org.springframework.http.HttpEntity<>(form, headers), String.class);
            log.info("Sent SMS via Twilio to {}", phone);
            return true;
        } catch (Exception e) {
            log.error("Failed to send SMS via Twilio to {}: {}", phone, e.getMessage());
            return false;
        }
    }
}
