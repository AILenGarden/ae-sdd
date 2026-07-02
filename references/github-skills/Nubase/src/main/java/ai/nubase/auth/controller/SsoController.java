package ai.nubase.auth.controller;

import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.service.RedirectUrlValidator;
import ai.nubase.auth.service.SamlService;
import lombok.RequiredArgsConstructor;
import org.apache.commons.lang3.StringUtils;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.util.UriComponentsBuilder;

import java.net.URI;
import java.util.Map;

/**
 * SAML 2.0 Single Sign-On endpoints (Supabase GoTrue parity).
 *
 * <ul>
 *   <li>GET  /auth/v1/sso                 — SP-initiated SSO (302 to IdP)</li>
 *   <li>POST /auth/v1/sso                 — same, returns {"url": ...} for JS clients</li>
 *   <li>GET  /auth/v1/sso/saml/metadata   — SP metadata XML</li>
 *   <li>POST /auth/v1/sso/saml/acs        — Assertion Consumer Service (IdP POST)</li>
 * </ul>
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1/sso")
public class SsoController {

    private final SamlService samlService;
    private final RedirectUrlValidator redirectUrlValidator;

    /** SP-initiated SSO via redirect. */
    @GetMapping
    public ResponseEntity<Void> ssoRedirect(
            @RequestParam(required = false) String domain,
            @RequestParam(name = "provider_id", required = false) String providerId,
            @RequestParam(required = false) String email,
            @RequestParam(name = "redirect_to", required = false) String redirectTo,
            @RequestParam(name = "code_challenge", required = false) String codeChallenge,
            @RequestParam(name = "code_challenge_method", required = false) String codeChallengeMethod) {
        String url = samlService.initiate(domain, providerId, email, redirectTo, codeChallenge, codeChallengeMethod);
        HttpHeaders headers = new HttpHeaders();
        headers.setLocation(URI.create(url));
        return ResponseEntity.status(HttpStatus.SEE_OTHER).headers(headers).build();
    }

    /** SP-initiated SSO returning the redirect URL as JSON (supabase-js signInWithSSO). */
    @PostMapping
    public ResponseEntity<Map<String, String>> ssoUrl(@RequestBody Map<String, Object> body) {
        String url = samlService.initiate(
                (String) body.get("domain"),
                (String) body.get("provider_id"),
                (String) body.get("email"),
                (String) body.get("redirect_to"),
                (String) body.get("code_challenge"),
                (String) body.get("code_challenge_method"));
        return ResponseEntity.ok(Map.of("url", url));
    }

    /** SP metadata XML for IdP administrators. */
    @GetMapping(value = "/saml/metadata", produces = MediaType.APPLICATION_XML_VALUE)
    public ResponseEntity<String> metadata() {
        return ResponseEntity.ok()
                .contentType(MediaType.APPLICATION_XML)
                .body(samlService.spMetadata());
    }

    /** Assertion Consumer Service — receives the IdP SAML Response (HTTP-POST binding). */
    @PostMapping(value = "/saml/acs", consumes = MediaType.APPLICATION_FORM_URLENCODED_VALUE)
    public ResponseEntity<?> acs(
            @RequestParam("SAMLResponse") String samlResponse,
            @RequestParam(name = "RelayState", required = false) String relayState) {
        SamlService.SamlLoginResult result = samlService.handleAcs(samlResponse, relayState);

        // Open-redirect guard: only honour an allow-listed redirect target.
        String redirectTo = redirectUrlValidator.sanitize(result.redirectTo());

        // PKCE: bounce back to the app with a one-time ?code= to exchange.
        if (result.isPkce()) {
            if (StringUtils.isNotBlank(redirectTo)) {
                String redirectUrl = UriComponentsBuilder.fromUriString(redirectTo)
                        .queryParam("code", result.pkceAuthCode())
                        .build().toUriString();
                return redirect(redirectUrl);
            }
            return ResponseEntity.ok(Map.of("code", result.pkceAuthCode()));
        }

        // Implicit: redirect with tokens in the fragment, or return JSON when no redirect set.
        AuthResponse session = result.session();
        if (StringUtils.isNotBlank(redirectTo)) {
            String redirectUrl = UriComponentsBuilder.fromUriString(redirectTo)
                    .fragment("access_token=" + session.getAccessToken()
                            + "&refresh_token=" + session.getRefreshToken()
                            + "&expires_in=" + session.getExpiresIn()
                            + "&token_type=" + session.getTokenType()
                            + "&type=sso")
                    .build().toUriString();
            return redirect(redirectUrl);
        }
        return ResponseEntity.ok(session);
    }

    private ResponseEntity<Void> redirect(String url) {
        HttpHeaders headers = new HttpHeaders();
        headers.setLocation(URI.create(url));
        return ResponseEntity.status(HttpStatus.SEE_OTHER).headers(headers).build();
    }
}
