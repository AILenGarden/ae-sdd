package ai.nubase.ai.gateway.entity;

import jakarta.persistence.*;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.math.BigDecimal;
import java.time.LocalDate;
import java.time.LocalDateTime;

@Data
@NoArgsConstructor
@AllArgsConstructor
@Entity
@Table(name = "model_pricing", schema = "ai_gateway")
public class ModelPricing {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "model", nullable = false, length = 120)
    private String model;

    @Column(name = "provider", nullable = false, length = 32)
    private String provider;

    @Column(name = "display_name", length = 120)
    private String displayName;

    @Column(name = "input_price_per_1m_cny", nullable = false, precision = 12, scale = 4)
    private BigDecimal inputPricePer1MCny;

    @Column(name = "output_price_per_1m_cny", nullable = false, precision = 12, scale = 4)
    private BigDecimal outputPricePer1MCny;

    @Column(name = "cache_creation_price_per_1m_cny", nullable = false, precision = 12, scale = 4)
    private BigDecimal cacheCreationPricePer1MCny;

    @Column(name = "cache_read_price_per_1m_cny", nullable = false, precision = 12, scale = 4)
    private BigDecimal cacheReadPricePer1MCny;

    @Column(name = "input_price_per_1m_usd", nullable = false, precision = 12, scale = 4)
    private BigDecimal inputPricePer1MUsd;

    @Column(name = "output_price_per_1m_usd", nullable = false, precision = 12, scale = 4)
    private BigDecimal outputPricePer1MUsd;

    @Column(name = "cache_creation_price_per_1m_usd", nullable = false, precision = 12, scale = 4)
    private BigDecimal cacheCreationPricePer1MUsd;

    @Column(name = "cache_read_price_per_1m_usd", nullable = false, precision = 12, scale = 4)
    private BigDecimal cacheReadPricePer1MUsd;

    @Column(name = "currency", nullable = false, length = 8)
    private String currency;

    @Column(name = "effective_from", nullable = false)
    private LocalDate effectiveFrom;

    @Column(name = "effective_to")
    private LocalDate effectiveTo;

    @Column(name = "is_active", nullable = false)
    private Boolean isActive;

    @Column(name = "notes", columnDefinition = "TEXT")
    private String notes;

    @Column(name = "quickstart_example", columnDefinition = "TEXT")
    private String quickstartExample;

    @Column(name = "sort_order", nullable = false)
    private Integer sortOrder;

    @Column(name = "created_at", nullable = false)
    private LocalDateTime createdAt;

    @Column(name = "updated_at", nullable = false)
    private LocalDateTime updatedAt;

    @PrePersist
    protected void onCreate() {
        LocalDateTime now = LocalDateTime.now();
        if (createdAt == null) createdAt = now;
        if (updatedAt == null) updatedAt = now;
        applyDefaults();
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = LocalDateTime.now();
        applyDefaults();
    }

    private void applyDefaults() {
        if (isActive == null) isActive = Boolean.TRUE;
        if (currency == null) currency = "USD";
        if (effectiveFrom == null) effectiveFrom = LocalDate.now();
        if (sortOrder == null) sortOrder = 0;
        if (inputPricePer1MCny == null) inputPricePer1MCny = BigDecimal.ZERO;
        if (outputPricePer1MCny == null) outputPricePer1MCny = BigDecimal.ZERO;
        if (cacheCreationPricePer1MCny == null) cacheCreationPricePer1MCny = BigDecimal.ZERO;
        if (cacheReadPricePer1MCny == null) cacheReadPricePer1MCny = BigDecimal.ZERO;
        if (inputPricePer1MUsd == null) inputPricePer1MUsd = BigDecimal.ZERO;
        if (outputPricePer1MUsd == null) outputPricePer1MUsd = BigDecimal.ZERO;
        if (cacheCreationPricePer1MUsd == null) cacheCreationPricePer1MUsd = BigDecimal.ZERO;
        if (cacheReadPricePer1MUsd == null) cacheReadPricePer1MUsd = BigDecimal.ZERO;
    }
}
