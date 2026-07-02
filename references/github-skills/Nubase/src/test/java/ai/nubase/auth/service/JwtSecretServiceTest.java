package ai.nubase.auth.service;

import ai.nubase.auth.entity.MfaAmrClaim;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.MfaAmrClaimRepository;
import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.security.Keys;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import javax.crypto.SecretKey;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

/**
 * Unit tests for {@link JwtSecretService#generateToken}: claim shaping incl. aal / amr /
 * is_anonymous, verified by decoding with the tenant key.
 */
@DisplayName("JwtSecretService claims")
class JwtSecretServiceTest {

    private static final String SECRET = "test-secret-test-secret-test-secret-1234567890";

    private MfaAmrClaimRepository amrRepo;
    private JwtSecretService svc;
    private final SecretKey key = Keys.hmacShaKeyFor(SECRET.getBytes(StandardCharsets.UTF_8));

    @BeforeEach
    void setUp() {
        amrRepo = mock(MfaAmrClaimRepository.class);
        svc = new JwtSecretService(new AuthConfig(), amrRepo);
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder()
                .appCode("demo").schemaName("public").jwtSecret(SECRET).jwtSecretKey(key).build());
    }

    @AfterEach
    void clear() {
        MultiTenancyContext.clear();
    }

    private Claims decode(String token) {
        return Jwts.parser().setSigningKey(key).build().parseClaimsJws(token).getBody();
    }

    @Test
    @DisplayName("token carries sub/role/email, session aal and the amr claim")
    void claims() {
        UUID uid = UUID.randomUUID();
        User user = User.builder().id(uid).email("a@b.com").role("authenticated").aud("authenticated")
                .isAnonymous(false).rawAppMetaData(Map.of("provider", "email")).build();
        Session session = Session.builder().id(UUID.randomUUID()).user(user).aal("aal2").build();

        MfaAmrClaim pwd = MfaAmrClaim.builder().authenticationMethod("password").build();
        pwd.setCreatedAt(Instant.now());
        MfaAmrClaim totp = MfaAmrClaim.builder().authenticationMethod("totp").build();
        totp.setCreatedAt(Instant.now());
        when(amrRepo.findBySessionId(session.getId())).thenReturn(List.of(pwd, totp));

        Claims c = decode(svc.generateToken(user, session));

        assertThat(c.getSubject()).isEqualTo(uid.toString());
        assertThat(c.get("role")).isEqualTo("authenticated");
        assertThat(c.get("email")).isEqualTo("a@b.com");
        assertThat(c.get("aal")).isEqualTo("aal2");
        assertThat(c.get("session_id")).isEqualTo(session.getId().toString());
        assertThat(c.get("is_anonymous")).isEqualTo(false);
        assertThat(c.get("amr").toString()).contains("password").contains("totp");
    }

    @Test
    @DisplayName("anonymous user is flagged in the token")
    void anonymous() {
        User user = User.builder().id(UUID.randomUUID()).role("authenticated").aud("authenticated")
                .isAnonymous(true).build();
        Session session = Session.builder().id(UUID.randomUUID()).user(user).aal("aal1").build();
        when(amrRepo.findBySessionId(any())).thenReturn(List.of());

        Claims c = decode(svc.generateToken(user, session));
        assertThat(c.get("is_anonymous")).isEqualTo(true);
        assertThat(c.get("aal")).isEqualTo("aal1");
    }
}
