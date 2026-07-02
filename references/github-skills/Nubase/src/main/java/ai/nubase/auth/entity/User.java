package ai.nubase.auth.entity;

import ai.nubase.common.enums.Role;
import io.hypersistence.utils.hibernate.type.json.JsonBinaryType;
import jakarta.persistence.*;
import lombok.*;
import org.hibernate.annotations.Type;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

@Entity
@Table(name = "users", schema = "auth")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class User {

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @Column(name = "instance_id")
    private UUID instanceId;

    @Column(name = "aud")
    private String aud;

    @Column(name = "role")
    private String role;

    // Email related
    @Column(name = "email", unique = true)
    private String email;

    @Column(name = "encrypted_password")
    private String encryptedPassword;

    @Column(name = "email_confirmed_at")
    private Instant emailConfirmedAt;

    // Phone related
    @Column(name = "phone", unique = true)
    private String phone;

    @Column(name = "phone_confirmed_at")
    private Instant phoneConfirmedAt;

    // Confirmation token
    @Column(name = "confirmation_token")
    private String confirmationToken;

    @Column(name = "confirmation_sent_at")
    private Instant confirmationSentAt;

    // Recovery token
    @Column(name = "recovery_token")
    private String recoveryToken;

    @Column(name = "recovery_sent_at")
    private Instant recoverySentAt;

    // Email change tokens
    @Column(name = "email_change_token_new")
    private String emailChangeTokenNew;

    @Column(name = "email_change_token_current")
    private String emailChangeTokenCurrent;

    @Column(name = "email_change")
    private String emailChange;

    @Column(name = "email_change_sent_at")
    private Instant emailChangeSentAt;

    @Column(name = "email_change_confirm_status")
    private Short emailChangeConfirmStatus;

    // Metadata (JSONB)
    @Type(JsonBinaryType.class)
    @Column(name = "raw_app_meta_data", columnDefinition = "jsonb")
    private Map<String, Object> rawAppMetaData;

    @Type(JsonBinaryType.class)
    @Column(name = "raw_user_meta_data", columnDefinition = "jsonb")
    private Map<String, Object> rawUserMetaData;

    // Timestamps
    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    @Column(name = "last_sign_in_at")
    private Instant lastSignInAt;

    // Other
    @Column(name = "invited_at")
    private Instant invitedAt;

    @Column(name = "banned_until")
    private Instant bannedUntil;

    @Column(name = "deleted_at")
    private Instant deletedAt;

    // Flags
    @Column(name = "is_super_admin")
    private Boolean isSuperAdmin;

    @Column(name = "is_sso_user")
    private Boolean isSsoUser;

    @Column(name = "is_anonymous")
    private Boolean isAnonymous;

    // Reauthentication token
    @Column(name = "reauthentication_token")
    private String reauthenticationToken;

    @Column(name = "reauthentication_sent_at")
    private Instant reauthenticationSentAt;

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
        updatedAt = Instant.now();
        if (role == null) {
            role = Role.AUTHENTICATED.getValue();
        }
        if (aud == null) {
            aud = Role.AUTHENTICATED.getValue();
        }
        if (isSuperAdmin == null) {
            isSuperAdmin = false;
        }
        if (isSsoUser == null) {
            isSsoUser = false;
        }
        if (isAnonymous == null) {
            isAnonymous = false;
        }
        if (emailChangeConfirmStatus == null) {
            emailChangeConfirmStatus = 0;
        }
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }
}
