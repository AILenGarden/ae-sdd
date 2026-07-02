package ai.nubase.ai.gateway.repository;

import ai.nubase.ai.gateway.entity.UpstreamConfig;
import ai.nubase.common.enums.ApiProvider;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.time.LocalDateTime;
import java.util.List;
import java.util.Optional;

/**
 * 上游 API 配置仓库
 */
@Repository
public interface UpstreamConfigRepository extends JpaRepository<UpstreamConfig, Long> {

    /**
     * 根据名称查找配置
     */
    Optional<UpstreamConfig> findByName(String name);

    /**
     * 查找所有标记为默认的上游配置（每个 provider 可以有各自的默认上游）
     */
    List<UpstreamConfig> findByIsDefaultTrue();

    /**
     * 查找所有激活的配置（按优先级升序排序，数字越小优先级越高）
     */
    List<UpstreamConfig> findByIsActiveTrueOrderByPriorityAsc();

    /**
     * 检查名称是否存在
     */
    boolean existsByName(String name);

    /**
     * 根据 provider 查找默认的活跃上游配置
     */
    Optional<UpstreamConfig> findByProviderAndIsDefaultTrueAndIsActiveTrue(ApiProvider provider);

    /**
     * 根据 provider 查找所有活跃上游，按优先级升序排列（数值越小优先级越高）
     */
    List<UpstreamConfig> findByProviderAndIsActiveTrueOrderByPriorityAsc(ApiProvider provider);

    /**
     * 根据 provider 查找活跃上游，排除指定名称，按优先级升序排列
     * 用于故障转移候选选择
     */
    List<UpstreamConfig> findByProviderAndIsActiveTrueAndNameNotInOrderByPriorityAsc(
            ApiProvider provider, List<String> excludedNames);

    /**
     * Find the default active upstream for an OpenAI-compatible routing channel.
     */
    Optional<UpstreamConfig> findByChannelCodeAndIsDefaultTrueAndIsActiveTrue(String channelCode);

    /**
     * Find active upstreams for an OpenAI-compatible routing channel by priority.
     */
    List<UpstreamConfig> findByChannelCodeAndIsActiveTrueOrderByPriorityAsc(String channelCode);

    /**
     * Find active upstreams for an OpenAI-compatible routing channel excluding tried names.
     */
    List<UpstreamConfig> findByChannelCodeAndIsActiveTrueAndNameNotInOrderByPriorityAsc(
            String channelCode, List<String> excludedNames);

    /**
     * Find active upstreams that explicitly support a model.
     */
    @Query(value = """
            SELECT *
            FROM upstream_configs
            WHERE is_active = true
              AND supported_models @> jsonb_build_array(CAST(:model AS text))
            ORDER BY priority ASC
            """, nativeQuery = true)
    List<UpstreamConfig> findActiveBySupportedModelOrderByPriorityAsc(@Param("model") String model);

    /**
     * 查找所有活跃上游，排除指定 provider 类型
     * 用于健康检查批量加载。
     */
    List<UpstreamConfig> findByIsActiveTrueAndProviderNot(ApiProvider excludedProvider);

    /**
     * 直接通过 JPQL UPDATE 更新健康状态，绕过 JPA 实体状态管理
     */
    @Modifying
    @Query("UPDATE UpstreamConfig u SET u.healthStatus = :status, u.healthMessage = :message, "
            + "u.lastHealthCheck = :checkTime WHERE u.id = :id")
    int updateHealthStatusById(@Param("id") Long id,
                               @Param("status") String status,
                               @Param("message") String message,
                               @Param("checkTime") LocalDateTime checkTime);

    @Modifying
    @Query("UPDATE UpstreamConfig u SET u.lastUsedAt = :lastUsedAt WHERE u.name = :name")
    int updateLastUsedAtByName(@Param("name") String name, @Param("lastUsedAt") LocalDateTime lastUsedAt);

    /**
     * 直接从数据库查询健康状态，绕过 JPA 一级缓存
     */
    @Query("SELECT u.healthStatus FROM UpstreamConfig u WHERE u.id = :id")
    String findHealthStatusById(@Param("id") Long id);
}
