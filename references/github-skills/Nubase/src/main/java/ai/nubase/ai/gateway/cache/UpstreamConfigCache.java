package ai.nubase.ai.gateway.cache;

import ai.nubase.ai.gateway.entity.UpstreamConfig;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * 请求热路径使用的活跃上游路由快照缓存。
 * <p>
 * 按租户（项目）隔离：每个 databaseKey 各自维护一份快照。上游配置存放在各项目租户库的
 * {@code ai_gateway.upstream_configs} 表中，因此缓存必须按 databaseKey 分桶，不能全局共享。
 * 快照在该项目首个请求时惰性加载，配置变更时按 databaseKey 失效。
 */
@Slf4j
@Component
public class UpstreamConfigCache {

    private final Map<String, UpstreamConfigSnapshot> byTenant = new ConcurrentHashMap<>();

    /** 返回指定租户的快照；不存在时返回 null，由调用方惰性加载。 */
    public UpstreamConfigSnapshot getSnapshot(String tenantKey) {
        return byTenant.get(tenantKey);
    }

    /** 用给定的活跃上游列表重建并缓存指定租户的快照。 */
    public UpstreamConfigSnapshot reload(String tenantKey, List<UpstreamConfig> activeUpstreams) {
        UpstreamConfigSnapshot snapshot = UpstreamConfigSnapshot.from(activeUpstreams);
        byTenant.put(tenantKey, snapshot);
        log.info("【upstream_snapshot_reload】tenant={}, active={}, providers={}, channels={}",
                tenantKey,
                snapshot.getActiveUpstreams().size(),
                snapshot.getByProvider().size(),
                snapshot.getByChannelCode().size());
        return snapshot;
    }

    /** 使指定租户的快照失效（下次请求会重新加载）。 */
    public void invalidate(String tenantKey) {
        byTenant.remove(tenantKey);
    }

    public Map<String, Object> diagnostics(String tenantKey) {
        UpstreamConfigSnapshot snapshot = byTenant.get(tenantKey);
        return snapshot == null ? Map.of() : snapshot.diagnostics();
    }
}
