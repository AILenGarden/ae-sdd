package ai.nubase.metadata.entity;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.IdClass;
import jakarta.persistence.PrePersist;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.io.Serializable;
import java.time.Instant;
import java.util.Objects;
import java.util.UUID;

/**
 * One key/value pair in the platform-wide settings table.
 *
 * <p>Composite primary key (category, key) — see {@link PlatformSettingId}.
 *
 * <p>{@code encrypted=true} means {@code value} is the prefixed ciphertext produced by
 * {@code EncryptionService#encrypt}; readers must call {@code decryptIfEncrypted}.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@Entity
@Table(name = "platform_settings")
@IdClass(PlatformSetting.PlatformSettingId.class)
public class PlatformSetting {

    @Id
    @Column(name = "category", nullable = false, length = 64)
    private String category;

    @Id
    @Column(name = "key", nullable = false, length = 128)
    private String key;

    @Column(name = "value", columnDefinition = "TEXT")
    private String value;

    @Column(name = "encrypted", nullable = false)
    private Boolean encrypted;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    @Column(name = "updated_by")
    private UUID updatedBy;

    @PrePersist
    void onCreate() {
        if (encrypted == null) encrypted = Boolean.FALSE;
        updatedAt = Instant.now();
    }

    @PreUpdate
    void onUpdate() {
        if (encrypted == null) encrypted = Boolean.FALSE;
        updatedAt = Instant.now();
    }

    @Data
    @NoArgsConstructor
    @AllArgsConstructor
    public static class PlatformSettingId implements Serializable {
        private String category;
        private String key;

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (!(o instanceof PlatformSettingId that)) return false;
            return Objects.equals(category, that.category) && Objects.equals(key, that.key);
        }

        @Override
        public int hashCode() {
            return Objects.hash(category, key);
        }
    }
}
