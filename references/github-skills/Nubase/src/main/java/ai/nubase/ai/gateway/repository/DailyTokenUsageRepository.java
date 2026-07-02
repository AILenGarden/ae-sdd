package ai.nubase.ai.gateway.repository;

import ai.nubase.ai.gateway.entity.DailyTokenUsage;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.time.LocalDate;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface DailyTokenUsageRepository extends JpaRepository<DailyTokenUsage, Long>,
        DailyTokenUsageRepositoryCustom {

    Optional<DailyTokenUsage> findByUserIdAndApiKeyIdAndUsageDateAndModel(
            UUID userId, Long apiKeyId, LocalDate usageDate, String model);

    List<DailyTokenUsage> findByUserIdAndUsageDateBetweenOrderByUsageDateDesc(
            UUID userId, LocalDate startDate, LocalDate endDate);

    // ---- project-wide (whole tenant DB = one project) ----

    List<DailyTokenUsage> findByUsageDateBetweenOrderByUsageDateDesc(LocalDate startDate, LocalDate endDate);

    List<DailyTokenUsage> findByApiKeyIdAndUsageDateBetweenOrderByUsageDateDesc(
            Long apiKeyId, LocalDate startDate, LocalDate endDate);

    @Query("SELECT COALESCE(SUM(d.totalTokens), 0) FROM DailyTokenUsage d " +
           "WHERE d.usageDate BETWEEN :startDate AND :endDate")
    Long sumTokensByDateBetween(@Param("startDate") LocalDate startDate, @Param("endDate") LocalDate endDate);

    @Query("SELECT COALESCE(SUM(d.requestCount), 0) FROM DailyTokenUsage d " +
           "WHERE d.usageDate BETWEEN :startDate AND :endDate")
    Long sumRequestCountByDateBetween(@Param("startDate") LocalDate startDate, @Param("endDate") LocalDate endDate);

    @Query("SELECT COALESCE(SUM(d.costUsd), 0) FROM DailyTokenUsage d " +
           "WHERE d.usageDate BETWEEN :startDate AND :endDate")
    java.math.BigDecimal sumCostUsdByDateBetween(@Param("startDate") LocalDate startDate, @Param("endDate") LocalDate endDate);

    // ---- scoped by api key ----

    @Query("SELECT COALESCE(SUM(d.totalTokens), 0) FROM DailyTokenUsage d " +
           "WHERE d.apiKeyId = :apiKeyId AND d.usageDate BETWEEN :startDate AND :endDate")
    Long sumTokensByApiKeyIdAndDateBetween(
            @Param("apiKeyId") Long apiKeyId,
            @Param("startDate") LocalDate startDate,
            @Param("endDate") LocalDate endDate);

    @Query("SELECT COALESCE(SUM(d.requestCount), 0) FROM DailyTokenUsage d " +
           "WHERE d.apiKeyId = :apiKeyId AND d.usageDate BETWEEN :startDate AND :endDate")
    Long sumRequestCountByApiKeyIdAndDateBetween(
            @Param("apiKeyId") Long apiKeyId,
            @Param("startDate") LocalDate startDate,
            @Param("endDate") LocalDate endDate);

    // ---- scoped by user (auth.users UUID) ----

    @Query("SELECT COALESCE(SUM(d.totalTokens), 0) FROM DailyTokenUsage d " +
           "WHERE d.userId = :userId AND d.usageDate BETWEEN :startDate AND :endDate")
    Long sumTokensByUserIdAndDateBetween(
            @Param("userId") UUID userId,
            @Param("startDate") LocalDate startDate,
            @Param("endDate") LocalDate endDate);

    @Query("SELECT COALESCE(SUM(d.requestCount), 0) FROM DailyTokenUsage d " +
           "WHERE d.userId = :userId AND d.usageDate BETWEEN :startDate AND :endDate")
    Long sumRequestCountByUserIdAndDateBetween(
            @Param("userId") UUID userId,
            @Param("startDate") LocalDate startDate,
            @Param("endDate") LocalDate endDate);

    @Query("SELECT COALESCE(SUM(d.costUsd), 0) FROM DailyTokenUsage d " +
           "WHERE d.userId = :userId AND d.usageDate BETWEEN :startDate AND :endDate")
    java.math.BigDecimal sumCostUsdByUserIdAndDateBetween(
            @Param("userId") UUID userId,
            @Param("startDate") LocalDate startDate,
            @Param("endDate") LocalDate endDate);

    // ---- by model (project-wide) ----

    @Query("SELECT d.model, SUM(d.totalTokens), SUM(d.requestCount), SUM(d.costUsd) FROM DailyTokenUsage d " +
           "WHERE d.usageDate BETWEEN :startDate AND :endDate " +
           "GROUP BY d.model")
    List<Object[]> sumUsageByModelAndDateBetween(
            @Param("startDate") LocalDate startDate,
            @Param("endDate") LocalDate endDate);
}
