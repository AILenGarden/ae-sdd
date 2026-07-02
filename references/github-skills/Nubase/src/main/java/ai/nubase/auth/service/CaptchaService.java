package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Service;
import org.springframework.util.LinkedMultiValueMap;
import org.springframework.util.MultiValueMap;
import org.springframework.web.client.RestTemplate;

import java.util.Map;

/**
 * Verifies CAPTCHA tokens (hCaptcha or Cloudflare Turnstile) when enabled.
 * When disabled (default) every request passes. Mirrors Supabase GoTrue's
 * optional CAPTCHA gate on signup / signin / recover / otp.
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class CaptchaService {

    private static final String HCAPTCHA_URL = "https://hcaptcha.com/siteverify";
    private static final String TURNSTILE_URL =
            "https://challenges.cloudflare.com/turnstile/v0/siteverify";

    private final EffectiveAuthConfig effectiveAuthConfig;
    private final RestTemplate restTemplate = new RestTemplate();

    /**
     * Validate the supplied CAPTCHA token. No-op when CAPTCHA is disabled.
     *
     * @throws IllegalArgumentException if verification is required and fails
     */
    public void verify(String captchaToken) {
        AuthConfig.CaptchaSettings captcha = effectiveAuthConfig.captcha();
        if (!captcha.isEnabled()) {
            return;
        }
        if (StringUtils.isBlank(captchaToken)) {
            throw new IllegalArgumentException("captcha protection: captcha token is required");
        }

        String url = "turnstile".equalsIgnoreCase(captcha.getProvider())
                ? TURNSTILE_URL : HCAPTCHA_URL;
        try {
            MultiValueMap<String, String> form = new LinkedMultiValueMap<>();
            form.add("secret", captcha.getSecret());
            form.add("response", captchaToken);

            org.springframework.http.HttpHeaders headers = new org.springframework.http.HttpHeaders();
            headers.setContentType(MediaType.APPLICATION_FORM_URLENCODED);

            @SuppressWarnings("unchecked")
            Map<String, Object> result = restTemplate.postForObject(url,
                    new org.springframework.http.HttpEntity<>(form, headers), Map.class);

            boolean success = result != null && Boolean.TRUE.equals(result.get("success"));
            if (!success) {
                throw new IllegalArgumentException("captcha protection: request disallowed");
            }
        } catch (IllegalArgumentException e) {
            throw e;
        } catch (Exception e) {
            log.error("CAPTCHA verification error: {}", e.getMessage());
            throw new IllegalArgumentException("captcha protection: verification failed");
        }
    }
}
