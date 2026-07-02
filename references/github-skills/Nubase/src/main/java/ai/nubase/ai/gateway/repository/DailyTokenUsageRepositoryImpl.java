package ai.nubase.ai.gateway.repository;

import jakarta.persistence.EntityManager;
import jakarta.persistence.PersistenceContext;
import org.springframework.stereotype.Repository;

import java.math.BigDecimal;
import java.time.LocalDate;
import java.util.UUID;

@Repository
public class DailyTokenUsageRepositoryImpl implements DailyTokenUsageRepositoryCustom {

    @PersistenceContext
    private EntityManager entityManager;

    @Override
    public void upsertDailyUsage(
            UUID userId,
            Long apiKeyId,
            LocalDate usageDate,
            String model,
            int errorCount,
            long inputTokens,
            long outputTokens,
            long cacheCreationInputTokens,
            long cacheReadInputTokens,
            long totalTokens,
            BigDecimal costUsd,
            BigDecimal costCny) {
        entityManager.createNativeQuery("""
                        INSERT INTO ai_gateway.daily_token_usage (
                            user_id,
                            api_key_id,
                            usage_date,
                            model,
                            request_count,
                            error_count,
                            input_tokens,
                            output_tokens,
                            cache_creation_input_tokens,
                            cache_read_input_tokens,
                            total_tokens,
                            cost_cny,
                            cost_usd,
                            created_at,
                            updated_at
                        ) VALUES (
                            :userId,
                            :apiKeyId,
                            :usageDate,
                            :model,
                            1,
                            :errorCount,
                            :inputTokens,
                            :outputTokens,
                            :cacheCreationInputTokens,
                            :cacheReadInputTokens,
                            :totalTokens,
                            :costCny,
                            :costUsd,
                            CURRENT_TIMESTAMP,
                            CURRENT_TIMESTAMP
                        )
                        ON CONFLICT (api_key_id, usage_date, model) DO UPDATE SET
                            request_count = ai_gateway.daily_token_usage.request_count + EXCLUDED.request_count,
                            error_count = ai_gateway.daily_token_usage.error_count + EXCLUDED.error_count,
                            input_tokens = ai_gateway.daily_token_usage.input_tokens + EXCLUDED.input_tokens,
                            output_tokens = ai_gateway.daily_token_usage.output_tokens + EXCLUDED.output_tokens,
                            cache_creation_input_tokens = ai_gateway.daily_token_usage.cache_creation_input_tokens
                                + EXCLUDED.cache_creation_input_tokens,
                            cache_read_input_tokens = ai_gateway.daily_token_usage.cache_read_input_tokens
                                + EXCLUDED.cache_read_input_tokens,
                            total_tokens = ai_gateway.daily_token_usage.total_tokens + EXCLUDED.total_tokens,
                            cost_cny = ai_gateway.daily_token_usage.cost_cny + EXCLUDED.cost_cny,
                            cost_usd = ai_gateway.daily_token_usage.cost_usd + EXCLUDED.cost_usd,
                            updated_at = CURRENT_TIMESTAMP
                        """)
                .setParameter("userId", userId)
                .setParameter("apiKeyId", apiKeyId)
                .setParameter("usageDate", usageDate)
                .setParameter("model", model)
                .setParameter("errorCount", errorCount)
                .setParameter("inputTokens", inputTokens)
                .setParameter("outputTokens", outputTokens)
                .setParameter("cacheCreationInputTokens", cacheCreationInputTokens)
                .setParameter("cacheReadInputTokens", cacheReadInputTokens)
                .setParameter("totalTokens", totalTokens)
                .setParameter("costUsd", costUsd == null ? BigDecimal.ZERO : costUsd)
                .setParameter("costCny", costCny == null ? BigDecimal.ZERO : costCny)
                .executeUpdate();
    }
}
