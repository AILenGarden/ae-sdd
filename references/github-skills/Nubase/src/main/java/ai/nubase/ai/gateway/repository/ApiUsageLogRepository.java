package ai.nubase.ai.gateway.repository;

import ai.nubase.ai.gateway.entity.ApiUsageLog;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.time.LocalDateTime;
import java.util.List;
import java.util.UUID;

/**
 * API Usage Log Repository
 */
@Repository
public interface ApiUsageLogRepository extends JpaRepository<ApiUsageLog, Long>,
        JpaSpecificationExecutor<ApiUsageLog> {

    /**
     * 分页查询指定API Key的使用日志
     */
    Page<ApiUsageLog> findByApiKeyOrderByCreatedAtDesc(String apiKey, Pageable pageable);

    /**
     * 分页查询指定用户的使用日志
     */
    Page<ApiUsageLog> findByUserIdOrderByCreatedAtDesc(UUID userId, Pageable pageable);

    /**
     * 查询指定API Key在时间范围内的日志
     */
    List<ApiUsageLog> findByApiKeyAndCreatedAtBetweenOrderByCreatedAtDesc(
            String apiKey, LocalDateTime startTime, LocalDateTime endTime);

    /**
     * 查询指定请求ID的日志
     */
    List<ApiUsageLog> findByRequestId(String requestId);

    /**
     * 统计指定API Key的总请求数
     */
    @Query("SELECT COUNT(l) FROM ApiUsageLog l WHERE l.apiKey = :apiKey")
    Long countByApiKey(@Param("apiKey") String apiKey);

    /**
     * 统计指定API Key在时间范围内的请求数
     */
    Long countByApiKeyAndCreatedAtBetween(String apiKey, LocalDateTime startTime, LocalDateTime endTime);

    /**
     * 统计指定用户在时间范围内的平均请求耗时（毫秒）。
     */
    @Query("SELECT AVG(l.durationMs) FROM ApiUsageLog l " +
            "WHERE l.userId = :userId " +
            "AND l.createdAt >= :startTime " +
            "AND l.createdAt < :endTime " +
            "AND l.durationMs IS NOT NULL")
    Double averageDurationMsByUserIdAndCreatedAtRange(
            @Param("userId") UUID userId,
            @Param("startTime") LocalDateTime startTime,
            @Param("endTime") LocalDateTime endTime);

    /**
     * 项目（整库）范围内时间窗的首 token 平均延迟（毫秒）。
     */
    @Query("SELECT AVG(l.firstTokenLatencyMs) FROM ApiUsageLog l " +
            "WHERE l.createdAt >= :startTime " +
            "AND l.createdAt < :endTime " +
            "AND l.firstTokenLatencyMs IS NOT NULL")
    Double averageFirstTokenLatencyMsByCreatedAtRange(
            @Param("startTime") LocalDateTime startTime,
            @Param("endTime") LocalDateTime endTime);

    /**
     * 指定用户在时间范围内的首 token 平均延迟 (毫秒); 非流式请求 firstTokenLatencyMs 为 NULL, 已过滤。
     */
    @Query("SELECT AVG(l.firstTokenLatencyMs) FROM ApiUsageLog l " +
            "WHERE l.userId = :userId " +
            "AND l.createdAt >= :startTime " +
            "AND l.createdAt < :endTime " +
            "AND l.firstTokenLatencyMs IS NOT NULL")
    Double averageFirstTokenLatencyMsByUserIdAndCreatedAtRange(
            @Param("userId") UUID userId,
            @Param("startTime") LocalDateTime startTime,
            @Param("endTime") LocalDateTime endTime);

    /**
     * 删除指定日期之前的日志（用于清理旧数据）
     */
    void deleteByCreatedAtBefore(LocalDateTime dateTime);
}
