package ai.nubase.ai.gateway.service;

import ai.nubase.ai.gateway.dto.TokenUsage;
import ai.nubase.ai.gateway.entity.ModelPricing;
import ai.nubase.ai.gateway.repository.ModelPricingRepository;
import lombok.RequiredArgsConstructor;
import lombok.Value;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.math.BigDecimal;
import java.math.RoundingMode;

/**
 * 计算单次请求的 token 成本（USD + CNY），仅用于统计展示，不做任何扣费。
 * <p>
 * 定价来自当前租户库 {@code ai_gateway.model_pricing}（项目内可编辑）。找不到模型定价时成本记 0。
 * 必须在已设置租户上下文（{@code MultiTenancyContext}）的请求线程内调用。
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class PricingService {

    private static final BigDecimal ONE_MILLION = new BigDecimal("1000000");

    private final ModelPricingRepository modelPricingRepository;

    /** 单次请求的成本，含美元与人民币两种币种。 */
    @Value
    public static class Cost {
        BigDecimal usd;
        BigDecimal cny;

        public static Cost zero() {
            return new Cost(BigDecimal.ZERO, BigDecimal.ZERO);
        }
    }

    /**
     * 按模型定价与 token 用量计算成本。找不到 active 定价时返回 0。
     */
    @Transactional(readOnly = true)
    public Cost computeCost(String model, TokenUsage usage) {
        if (model == null || usage == null) {
            return Cost.zero();
        }
        ModelPricing pricing = modelPricingRepository.findFirstByModelAndIsActiveTrue(model).orElse(null);
        if (pricing == null) {
            log.debug("No active pricing for model={}, cost defaults to 0", model);
            return Cost.zero();
        }

        long input = nz(usage.getInputTokens());
        long output = nz(usage.getOutputTokens());
        long cacheCreation = nz(usage.getCacheCreationInputTokens());
        long cacheRead = nz(usage.getCacheReadInputTokens());

        BigDecimal usd = component(input, pricing.getInputPricePer1MUsd())
                .add(component(output, pricing.getOutputPricePer1MUsd()))
                .add(component(cacheCreation, pricing.getCacheCreationPricePer1MUsd()))
                .add(component(cacheRead, pricing.getCacheReadPricePer1MUsd()))
                .setScale(6, RoundingMode.HALF_UP);

        BigDecimal cny = component(input, pricing.getInputPricePer1MCny())
                .add(component(output, pricing.getOutputPricePer1MCny()))
                .add(component(cacheCreation, pricing.getCacheCreationPricePer1MCny()))
                .add(component(cacheRead, pricing.getCacheReadPricePer1MCny()))
                .setScale(6, RoundingMode.HALF_UP);

        return new Cost(usd, cny);
    }

    /** 仅计算美元成本（兼容旧调用点）。 */
    @Transactional(readOnly = true)
    public BigDecimal computeUsd(String model, TokenUsage usage) {
        return computeCost(model, usage).getUsd();
    }

    private static BigDecimal component(long tokens, BigDecimal pricePer1M) {
        if (tokens <= 0 || pricePer1M == null) {
            return BigDecimal.ZERO;
        }
        return BigDecimal.valueOf(tokens).divide(ONE_MILLION, 10, RoundingMode.HALF_UP).multiply(pricePer1M);
    }

    private static long nz(Integer v) {
        return v == null ? 0L : v.longValue();
    }
}
