package ai.nubase.ai.gateway.service;

import ai.nubase.ai.gateway.cache.UpstreamConfigCache;
import ai.nubase.ai.gateway.cache.UpstreamConfigSnapshot;
import ai.nubase.ai.gateway.entity.UpstreamConfig;
import ai.nubase.ai.gateway.repository.UpstreamConfigRepository;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.common.enums.ApiProvider;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * 上游配置管理服务
 * 通过内存快照维护活跃上游路由。
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class UpstreamConfigService {

    private final UpstreamConfigRepository upstreamConfigRepository;
    private final UpstreamConfigCache configCache;
    private final UpstreamUsageTouchService usageTouchService;

    // ==================== 查询方法 ====================

    /**
     * 根据名称加载上游配置。
     */
    public UpstreamConfig getByName(String name) {
        return snapshot().getByName(name)
                .orElseThrow(() -> new IllegalArgumentException("upstream config not found: " + name));
    }

    /**
     * 获取指定 provider 的默认上游配置
     */
    public UpstreamConfig getDefaultByProvider(ApiProvider provider) {
        return snapshot().getDefaultByProvider(provider)
                .orElseThrow(() -> new IllegalStateException("No active upstream found for provider: " + provider));
    }

    /**
     * 获取 OpenAI 兼容路由渠道的默认活跃上游。
     * 如果没有显式默认上游，则回退到该渠道中优先级最高的活跃上游。
     */
    public UpstreamConfig getDefaultByChannelCode(String channelCode) {
        String normalizedChannelCode = normalizeChannelCode(channelCode);
        return snapshot().getDefaultByChannelCode(normalizedChannelCode)
                .orElseThrow(() -> new IllegalStateException("No active upstream found for channel: "
                        + normalizedChannelCode));
    }

    /**
     * 判断指定路由渠道是否至少存在一个活跃上游。
     */
    public boolean hasActiveUpstreamForChannelCode(String channelCode) {
        String normalizedChannelCode = normalizeChannelCode(channelCode);
        return snapshot().hasActiveUpstreamForChannelCode(normalizedChannelCode);
    }

    /**
     * 判断是否至少存在一个显式支持该模型的活跃上游。
     */
    public boolean hasActiveUpstreamForModel(String model) {
        String normalizedModel = normalizeModel(model);
        if (normalizedModel == null) {
            return false;
        }
        return snapshot().hasActiveUpstreamForModel(normalizedModel);
    }

    /**
     * 获取显式支持该模型且优先级最高的活跃上游。
     */
    public UpstreamConfig getDefaultBySupportedModel(String model) {
        String normalizedModel = normalizeModel(model);
        if (normalizedModel == null) {
            throw new IllegalArgumentException("model must not be blank");
        }

        List<UpstreamConfig> candidates = snapshot().getSupportedModelCandidates(normalizedModel, List.of());
        if (!candidates.isEmpty()) {
            UpstreamConfig picked = candidates.get(0);
            log.info("model {} routed to upstream: name={}, priority={}",
                    normalizedModel, picked.getName(), picked.getPriority());
            return picked;
        }
        throw new IllegalStateException("No active upstream found for model: " + normalizedModel);
    }

    public UpstreamConfig selectForProviderAndModel(ApiProvider provider, String model) {
        return snapshot().selectForProviderAndModel(provider, model);
    }

    public UpstreamConfig selectForChannelAndModel(String channelCode, String model) {
        return snapshot().selectForChannelAndModel(channelCode, model);
    }

    public boolean supportsModel(UpstreamConfig config, String model) {
        return UpstreamConfigSnapshot.supportsModel(config, model);
    }

    public boolean allowsModel(UpstreamConfig config, String model) {
        return UpstreamConfigSnapshot.allowsModel(config, model);
    }

    /**
     * 获取指定 provider 的故障转移候选上游，排除已尝试过的上游
     * 按 priority 升序排列（数值越小优先级越高）
     */
    public List<UpstreamConfig> getFailoverUpstreams(ApiProvider provider, List<String> excludeNames) {
        return snapshot().getProviderFailoverCandidates(provider, null, excludeNames);
    }

    public List<UpstreamConfig> getFailoverUpstreamsByProviderAndModel(
            ApiProvider provider, String model, List<String> excludeNames) {
        return snapshot().getProviderFailoverCandidates(provider, model, excludeNames);
    }

    /**
     * 获取同一 OpenAI 兼容路由渠道内的故障转移候选上游。
     */
    public List<UpstreamConfig> getFailoverUpstreamsByChannelCode(String channelCode, List<String> excludeNames) {
        String normalizedChannelCode = normalizeChannelCode(channelCode);
        return snapshot().getChannelFailoverCandidates(normalizedChannelCode, null, excludeNames);
    }

    public List<UpstreamConfig> getFailoverUpstreamsByChannelAndModel(
            String channelCode, String model, List<String> excludeNames) {
        String normalizedChannelCode = normalizeChannelCode(channelCode);
        return snapshot().getChannelFailoverCandidates(normalizedChannelCode, model, excludeNames);
    }

    /**
     * 获取显式支持同一模型的故障转移候选上游。
     */
    public List<UpstreamConfig> getFailoverUpstreamsBySupportedModel(String model, List<String> excludeNames) {
        String normalizedModel = normalizeModel(model);
        if (normalizedModel == null) {
            return List.of();
        }

        return snapshot().getSupportedModelCandidates(normalizedModel, excludeNames);
    }

    /**
     * 获取所有激活的上游配置（按 priority 升序）
     */
    public List<UpstreamConfig> getActiveUpstreams() {
        return upstreamConfigRepository.findByIsActiveTrueOrderByPriorityAsc();
    }

    /**
     * 获取当前项目全部上游配置（含未激活）。
     */
    public List<UpstreamConfig> getAllUpstreams() {
        return upstreamConfigRepository.findAll();
    }

    public java.util.Optional<UpstreamConfig> getById(Long id) {
        return upstreamConfigRepository.findById(id);
    }

    /**
     * 新建或更新上游配置并刷新当前项目的路由快照。
     */
    @Transactional
    public UpstreamConfig save(UpstreamConfig config) {
        UpstreamConfig saved = upstreamConfigRepository.save(config);
        upstreamConfigRepository.flush();
        reloadActiveUpstreamSnapshot();
        return saved;
    }

    /**
     * 按 id 删除上游配置并刷新当前项目的路由快照。
     */
    @Transactional
    public void deleteById(Long id) {
        upstreamConfigRepository.findById(id).ifPresent(cfg -> {
            upstreamConfigRepository.delete(cfg);
            upstreamConfigRepository.flush();
            reloadActiveUpstreamSnapshot();
        });
    }

    private String normalizeChannelCode(String channelCode) {
        if (channelCode == null || channelCode.isBlank()) {
            throw new IllegalArgumentException("channel code must not be blank");
        }
        return channelCode.trim().toLowerCase(Locale.ROOT);
    }

    private String normalizeModel(String model) {
        if (model == null || model.isBlank()) {
            return null;
        }
        return model.trim().toLowerCase(Locale.ROOT);
    }

    // ==================== 修改方法 ====================

    /**
     * 更新最后使用时间
     */
    @Transactional
    public void updateLastUsedAt(String name) {
        usageTouchService.touch(name);
    }

    /**
     * 删除上游配置
     */
    @Transactional
    public void delete(String name) {
        log.info("【upstream_delete】preparing to delete upstream config: {}", name);
        UpstreamConfig config = upstreamConfigRepository.findByName(name)
                .orElseThrow(() -> new IllegalArgumentException("upstream config not found: " + name));
        if (config.getIsDefault()) {
            throw new IllegalStateException("cannot delete a default upstream config");
        }
        upstreamConfigRepository.delete(config);
        upstreamConfigRepository.flush();

        reloadActiveUpstreamSnapshot();
        log.info("【upstream_delete】deleted upstream config: {}", name);
    }

    // ==================== 快照（按租户惰性加载） ====================

    /**
     * 重新加载当前项目（租户）的活跃上游路由快照。
     */
    public Map<String, Object> reloadActiveUpstreamSnapshot() {
        String tenantKey = currentTenantKey();
        List<UpstreamConfig> activeUpstreams = upstreamConfigRepository.findByIsActiveTrueOrderByPriorityAsc();
        return configCache.reload(tenantKey, activeUpstreams).diagnostics();
    }

    /**
     * 返回当前项目（租户）的诊断快照信息。
     */
    public Map<String, Object> snapshotDiagnostics() {
        return snapshot().diagnostics();
    }

    /**
     * 获取当前项目（租户）的活跃上游路由快照；首个请求时从该租户库惰性加载。
     */
    private UpstreamConfigSnapshot snapshot() {
        String tenantKey = currentTenantKey();
        UpstreamConfigSnapshot snap = configCache.getSnapshot(tenantKey);
        if (snap == null) {
            snap = configCache.reload(tenantKey, upstreamConfigRepository.findByIsActiveTrueOrderByPriorityAsc());
        }
        return snap;
    }

    private String currentTenantKey() {
        String tenantKey = MultiTenancyContext.getDatabaseKey();
        if (tenantKey == null || tenantKey.isBlank()) {
            throw new IllegalStateException(
                    "No tenant context: AI gateway upstream routing requires a resolved project (databaseKey)");
        }
        return tenantKey;
    }
}
