package ai.nubase.auth.service;

import ai.nubase.auth.entity.AuditLogEntry;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.AuditLogEntryRepository;
import jakarta.servlet.http.HttpServletRequest;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.TransactionDefinition;
import org.springframework.transaction.support.TransactionTemplate;
import org.springframework.web.context.request.RequestContextHolder;
import org.springframework.web.context.request.ServletRequestAttributes;

import java.time.Instant;
import java.util.HashMap;
import java.util.Map;

/**
 * Writes structured auth audit-trail entries to {@code auth.audit_log_entries}.
 * Mirrors the action vocabulary used by Supabase GoTrue (login, signup, logout,
 * user_modified, factor_*, etc.).
 *
 * <p>Each write runs in its OWN transaction (REQUIRES_NEW) via a {@link TransactionTemplate}
 * so a failure (e.g. on a tenant not yet migrated) rolls back only the audit insert and is
 * swallowed here — it never marks the caller's business transaction rollback-only.
 */
@Service
@Slf4j
public class AuditService {

    // Common action names (kept aligned with GoTrue's audit action vocabulary).
    public static final String LOGIN = "login";
    public static final String LOGOUT = "logout";
    public static final String SIGNUP = "signup";
    public static final String USER_RECOVERY_REQUESTED = "user_recovery_requested";
    public static final String USER_CONFIRMATION_REQUESTED = "user_confirmation_requested";
    public static final String OTP_REQUESTED = "otp_requested";
    public static final String TOKEN_REFRESHED = "token_refreshed";
    public static final String USER_MODIFIED = "user_modified";
    public static final String FACTOR_ENROLLED = "factor_enrolled";
    public static final String FACTOR_VERIFIED = "factor_verified";
    public static final String FACTOR_UNENROLLED = "factor_unenrolled";
    public static final String CHALLENGE_CREATED = "challenge_created";
    public static final String REAUTHENTICATION_REQUESTED = "reauthentication_requested";
    public static final String LOGIN_FAILED = "login_failed";
    public static final String IDENTITY_UNLINKED = "identity_unlinked";

    private final AuditLogEntryRepository auditLogEntryRepository;
    private final TransactionTemplate auditTxTemplate;

    public AuditService(AuditLogEntryRepository auditLogEntryRepository,
                        PlatformTransactionManager transactionManager) {
        this.auditLogEntryRepository = auditLogEntryRepository;
        this.auditTxTemplate = new TransactionTemplate(transactionManager);
        this.auditTxTemplate.setPropagationBehavior(TransactionDefinition.PROPAGATION_REQUIRES_NEW);
    }

    public void record(String action, User user, Map<String, Object> extra) {
        try {
            auditTxTemplate.executeWithoutResult(status -> {
                Map<String, Object> payload = new HashMap<>();
                payload.put("action", action);
                payload.put("timestamp", Instant.now().toString());
                if (user != null) {
                    payload.put("actor_id", user.getId() != null ? user.getId().toString() : null);
                    payload.put("actor_username", user.getEmail());
                }
                if (extra != null) {
                    payload.putAll(extra);
                }

                AuditLogEntry entry = AuditLogEntry.builder()
                        .instanceId(user != null ? user.getInstanceId() : null)
                        .payload(payload)
                        .ipAddress(currentIp())
                        .build();
                auditLogEntryRepository.save(entry);
            });
        } catch (Exception e) {
            log.warn("Failed to write audit log entry for action '{}': {}", action, e.getMessage());
        }
    }

    public void record(String action, User user) {
        record(action, user, null);
    }

    private String currentIp() {
        ServletRequestAttributes attrs =
                (ServletRequestAttributes) RequestContextHolder.getRequestAttributes();
        if (attrs == null) {
            return null;
        }
        HttpServletRequest request = attrs.getRequest();
        String xff = request.getHeader("X-Forwarded-For");
        if (xff != null && !xff.isEmpty()) {
            return xff.split(",")[0].strip();
        }
        return request.getRemoteAddr();
    }
}
