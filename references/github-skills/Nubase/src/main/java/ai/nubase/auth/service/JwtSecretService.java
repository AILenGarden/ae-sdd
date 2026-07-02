package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import ai.nubase.auth.entity.MfaAmrClaim;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.MfaAmrClaimRepository;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.common.enums.Role;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.SignatureAlgorithm;
import lombok.RequiredArgsConstructor;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import javax.crypto.SecretKey;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Date;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

@Service
@RequiredArgsConstructor
public class JwtSecretService {

    private static final Logger log = LoggerFactory.getLogger(JwtSecretService.class);


    private final AuthConfig authConfig;
    private final MfaAmrClaimRepository mfaAmrClaimRepository;


    public Claims validateToken(String token) {
        SecretKey secretKey = MultiTenancyContext.getJwtSecretKey();
        if (secretKey == null) {
            throw new RuntimeException("Secret key not found for token: " + token);
        }

        try {
            return Jwts.parser()
                    .setSigningKey(secretKey)
                    .build()
                    .parseClaimsJws(token)
                    .getBody();
        } catch (Exception e) {
            throw new RuntimeException("Invalid or expired JWT token", e);
        }
    }

    public String getUserIdFromToken(String token) {
        Claims claims = validateToken(token);
        return claims.getSubject();
    }

    public boolean isTokenExpired(String token) {
        try {
            Claims claims = validateToken(token);
            return claims.getExpiration().before(new Date());
        } catch (Exception e) {
            return true;
        }
    }

    public String generateToken(User user, Session session) {
        SecretKey secretKey = MultiTenancyContext.getJwtSecretKey();
        Map<String, Object> claims = new HashMap<>();
        claims.put("sub", user.getId().toString());
        claims.put("aud", user.getAud() != null ? user.getAud() : Role.AUTHENTICATED.getValue());
        claims.put("role", user.getRole() != null ? user.getRole() : Role.AUTHENTICATED.getValue());
        claims.put("email", user.getEmail());
        claims.put("phone", user.getPhone());

        if (user.getRawAppMetaData() != null) {
            claims.put("app_metadata", user.getRawAppMetaData());
        }

        if (user.getRawUserMetaData() != null) {
            claims.put("user_metadata", user.getRawUserMetaData());
        }

        if (session != null) {
            claims.put("session_id", session.getId().toString());
            if (session.getAal() != null) {
                claims.put("aal", session.getAal());
            }
            // amr: authentication methods references backing this session
            List<Map<String, Object>> amr = buildAmrClaim(session);
            if (!amr.isEmpty()) {
                claims.put("amr", amr);
            }
            claims.put("is_anonymous", Boolean.TRUE.equals(user.getIsAnonymous()));
        }

        Instant now = Instant.now();
        Instant expiration = now.plusSeconds(authConfig.getJwt().getExpiration());

        return Jwts.builder()
                .setClaims(claims)
                .setIssuedAt(Date.from(now))
                .setExpiration(Date.from(expiration))
                .setIssuer(authConfig.getJwt().getIssuer())
                .signWith(secretKey, SignatureAlgorithm.HS256)
                .compact();
    }

    /**
     * Build the {@code amr} (Authentication Methods References) claim from the
     * {@code mfa_amr_claims} rows recorded for this session.
     */
    private List<Map<String, Object>> buildAmrClaim(Session session) {
        List<Map<String, Object>> amr = new ArrayList<>();
        try {
            for (MfaAmrClaim claim : mfaAmrClaimRepository.findBySessionId(session.getId())) {
                Map<String, Object> entry = new HashMap<>();
                entry.put("method", claim.getAuthenticationMethod());
                entry.put("timestamp", claim.getCreatedAt() != null
                        ? claim.getCreatedAt().getEpochSecond() : null);
                amr.add(entry);
            }
        } catch (Exception e) {
            log.debug("Could not load amr claims for session {}: {}", session.getId(), e.getMessage());
        }
        return amr;
    }

}
