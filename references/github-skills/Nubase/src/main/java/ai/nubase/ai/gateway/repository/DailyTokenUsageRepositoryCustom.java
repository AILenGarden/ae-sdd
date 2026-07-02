package ai.nubase.ai.gateway.repository;

import java.math.BigDecimal;
import java.time.LocalDate;
import java.util.UUID;

public interface DailyTokenUsageRepositoryCustom {

    void upsertDailyUsage(
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
            BigDecimal costCny);
}
