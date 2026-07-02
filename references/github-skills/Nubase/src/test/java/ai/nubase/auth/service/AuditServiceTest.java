package ai.nubase.auth.service;

import ai.nubase.auth.entity.AuditLogEntry;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.AuditLogEntryRepository;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.SimpleTransactionStatus;

import java.util.Map;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatCode;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

/**
 * Unit tests for {@link AuditService}: payload shaping + failures are swallowed (never propagate).
 */
@DisplayName("AuditService")
class AuditServiceTest {

    private AuditLogEntryRepository repo;
    private AuditService svc;

    @BeforeEach
    void setUp() {
        repo = mock(AuditLogEntryRepository.class);
        PlatformTransactionManager txm = mock(PlatformTransactionManager.class);
        when(txm.getTransaction(any())).thenReturn(new SimpleTransactionStatus());
        svc = new AuditService(repo, txm);
    }

    @Test
    @DisplayName("records action + actor + extra into the payload")
    void records() {
        User u = User.builder().id(UUID.randomUUID()).email("a@b.com").build();

        svc.record(AuditService.LOGIN, u, Map.of("provider", "email"));

        ArgumentCaptor<AuditLogEntry> cap = ArgumentCaptor.forClass(AuditLogEntry.class);
        verify(repo).save(cap.capture());
        Map<String, Object> payload = cap.getValue().getPayload();
        assertThat(payload.get("action")).isEqualTo("login");
        assertThat(payload.get("actor_id")).isEqualTo(u.getId().toString());
        assertThat(payload.get("actor_username")).isEqualTo("a@b.com");
        assertThat(payload.get("provider")).isEqualTo("email");
        assertThat(payload.get("timestamp")).isNotNull();
    }

    @Test
    @DisplayName("a persistence failure is swallowed (does not break the caller)")
    void swallowsFailure() {
        when(repo.save(any())).thenThrow(new RuntimeException("db down"));
        assertThatCode(() -> svc.record(AuditService.LOGOUT, User.builder().id(UUID.randomUUID()).build()))
                .doesNotThrowAnyException();
    }
}
