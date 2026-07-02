package ai.nubase.auth.service;

import ai.nubase.auth.dto.oauth.OAuthUserInfo;
import ai.nubase.common.config.AuthConfig;
import cn.hutool.json.JSONUtil;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.stereotype.Service;
import org.springframework.web.client.RestTemplate;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.KeyFactory;
import java.security.PublicKey;
import java.security.spec.RSAPublicKeySpec;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Verifies third-party OpenID Connect ID tokens for native social sign-in
 * ({@code POST /token?grant_type=id_token}). Fetches the provider JWKS, validates the
 * RS256 signature + issuer + audience + nonce, and returns normalized user info.
 * Mirrors Supabase GoTrue's {@code signInWithIdToken}.
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class IdTokenService {

    private final AuthConfig authConfig;
    private final RestTemplate restTemplate = new RestTemplate();
    private final ObjectMapper objectMapper = new ObjectMapper();

    // kid -> PublicKey cache (provider JWKS rotate rarely; refetched on cache miss).
    private final Map<String, PublicKey> keyCache = new ConcurrentHashMap<>();

    /**
     * Verify an ID token and return the normalized user info.
     *
     * @param provider provider id (e.g. "google", "apple")
     * @param idToken  the OIDC ID token (JWT)
     * @param nonce    optional expected nonce
     */
    public OAuthUserInfo verify(String provider, String idToken, String nonce) {
        AuthConfig.IdTokenSettings.Provider cfg = authConfig.getIdToken().getProviders().get(provider);
        if (cfg == null || StringUtils.isBlank(cfg.getJwksUri())) {
            throw new IllegalArgumentException("Unsupported id_token provider: " + provider);
        }

        String kid = extractKid(idToken);
        PublicKey key = resolveKey(cfg.getJwksUri(), kid);

        Claims claims;
        try {
            claims = Jwts.parser()
                    .setSigningKey(key)
                    .build()
                    .parseClaimsJws(idToken)
                    .getBody();
        } catch (Exception e) {
            throw new RuntimeException("Invalid id_token: " + e.getMessage());
        }

        // Issuer check
        if (StringUtils.isNotBlank(cfg.getIssuer())) {
            List<String> allowed = List.of(cfg.getIssuer().split(","));
            String iss = claims.getIssuer();
            boolean ok = allowed.stream().map(String::trim).anyMatch(a -> a.equals(iss));
            if (!ok) {
                throw new RuntimeException("id_token issuer not allowed: " + iss);
            }
        }

        // Audience check (when configured)
        if (cfg.getAudiences() != null && !cfg.getAudiences().isEmpty()) {
            Object aud = claims.get("aud");
            boolean ok = aud != null && cfg.getAudiences().stream()
                    .anyMatch(a -> aud.toString().contains(a));
            if (!ok) {
                throw new RuntimeException("id_token audience not allowed");
            }
        }

        // Nonce check (when supplied by the caller)
        if (StringUtils.isNotBlank(nonce)) {
            Object claimNonce = claims.get("nonce");
            if (claimNonce == null || !nonce.equals(claimNonce.toString())) {
                throw new RuntimeException("id_token nonce mismatch");
            }
        }

        String sub = claims.getSubject();
        String email = claims.get("email", String.class);
        Object emailVerified = claims.get("email_verified");
        boolean verified = emailVerified != null && Boolean.parseBoolean(emailVerified.toString());

        return OAuthUserInfo.builder()
                .provider(provider)
                .providerId(sub)
                .email(email)
                .emailVerified(verified)
                .name(claims.get("name", String.class))
                .avatarUrl(claims.get("picture", String.class))
                .rawData(JSONUtil.toJsonStr(claims))
                .build();
    }

    private String extractKid(String idToken) {
        try {
            String[] parts = idToken.split("\\.");
            String headerJson = new String(Base64.getUrlDecoder().decode(parts[0]), StandardCharsets.UTF_8);
            Map<?, ?> header = objectMapper.readValue(headerJson, Map.class);
            Object kid = header.get("kid");
            return kid != null ? kid.toString() : null;
        } catch (Exception e) {
            throw new IllegalArgumentException("Malformed id_token header");
        }
    }

    // package-private/overridable so tests can supply a known key without hitting the network
    protected PublicKey resolveKey(String jwksUri, String kid) {
        if (kid != null) {
            PublicKey cached = keyCache.get(jwksUri + "#" + kid);
            if (cached != null) {
                return cached;
            }
        }
        PublicKey key = fetchAndCache(jwksUri, kid);
        if (key == null) {
            throw new RuntimeException("Signing key not found in JWKS for kid=" + kid);
        }
        return key;
    }

    @SuppressWarnings("unchecked")
    private PublicKey fetchAndCache(String jwksUri, String kid) {
        try {
            Map<String, Object> jwks = restTemplate.getForObject(jwksUri, Map.class);
            if (jwks == null || !(jwks.get("keys") instanceof List<?> keys)) {
                return null;
            }
            PublicKey match = null;
            for (Object k : keys) {
                Map<String, Object> jwk = (Map<String, Object>) k;
                if (!"RSA".equals(jwk.get("kty"))) {
                    continue;
                }
                String thisKid = (String) jwk.get("kid");
                PublicKey pub = buildRsaKey((String) jwk.get("n"), (String) jwk.get("e"));
                if (thisKid != null) {
                    keyCache.put(jwksUri + "#" + thisKid, pub);
                }
                if (kid == null || kid.equals(thisKid)) {
                    match = pub;
                }
            }
            return match;
        } catch (Exception e) {
            log.error("Failed to fetch JWKS from {}: {}", jwksUri, e.getMessage());
            return null;
        }
    }

    private PublicKey buildRsaKey(String nB64, String eB64) throws Exception {
        BigInteger modulus = new BigInteger(1, Base64.getUrlDecoder().decode(nB64));
        BigInteger exponent = new BigInteger(1, Base64.getUrlDecoder().decode(eB64));
        return KeyFactory.getInstance("RSA")
                .generatePublic(new RSAPublicKeySpec(modulus, exponent));
    }
}
