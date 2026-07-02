package ai.nubase.ai.gateway.entity;

import jakarta.persistence.*;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.LocalDateTime;
import java.util.UUID;

/**
 * 自路由网关密钥实体（位于各项目租户库的 ai_gateway.api_keys）。
 * 完整密钥形如 nbk_&lt;appCode&gt;_&lt;secret&gt;，库内只存其 SHA-256 哈希(keyHash)。
 * userId 可选，指向本项目租户库 auth.users(id)（UUID）。
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
@Builder
@Entity
@Table(name = "api_keys", schema = "ai_gateway")
public class ApiKey {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "user_id")
    private UUID userId;

    /** 旧字段，保留兼容；新自路由密钥仅存 keyHash，本列可为空。 */
    @Column(name = "api_key", length = 255)
    private String apiKey;

    /** 完整密钥的 SHA-256 十六进制哈希，入站校验按此列查找。 */
    @Column(name = "key_hash", unique = true, length = 64)
    private String keyHash;

    @Column(name = "key_prefix", length = 32)
    private String keyPrefix;

    @Column(name = "name", length = 255)
    private String name;

    @Column(name = "description", columnDefinition = "TEXT")
    private String description;

    @Column(name = "scope", length = 64)
    private String scope;

    @Column(name = "is_active", nullable = false)
    private Boolean isActive;

    @Column(name = "expires_at")
    private LocalDateTime expiresAt;

    @Column(name = "revoked_at")
    private LocalDateTime revokedAt;

    @Column(name = "last_used_at")
    private LocalDateTime lastUsedAt;

    @Column(name = "created_at", nullable = false)
    private LocalDateTime createdAt;

    @Column(name = "updated_at", nullable = false)
    private LocalDateTime updatedAt;

    @PrePersist
    protected void onCreate() {
        LocalDateTime now = LocalDateTime.now();
        if (createdAt == null) createdAt = now;
        if (updatedAt == null) updatedAt = now;
        if (isActive == null) isActive = Boolean.TRUE;
        if (scope == null) scope = "all";
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = LocalDateTime.now();
    }
}
