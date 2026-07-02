package ai.nubase.ai.gateway.cache;

import ai.nubase.ai.gateway.entity.UpstreamConfig;
import ai.nubase.common.enums.ApiProvider;
import lombok.Getter;

import java.time.Instant;
import java.util.*;
import java.util.stream.Collectors;

/**
 * 基于活跃上游配置构建的不可变路由快照。
 */
@Getter
public class UpstreamConfigSnapshot {

    private static final String WILDCARD_MODEL = "*";
    private static final Comparator<UpstreamConfig> ROUTING_ORDER = Comparator
            .comparing(UpstreamConfigSnapshot::priorityOf)
            .thenComparing(config -> nullToEmpty(config.getName()));

    private final Instant loadedAt;
    private final List<UpstreamConfig> activeUpstreams;
    private final Map<String, UpstreamConfig> byName;
    private final Map<ApiProvider, List<UpstreamConfig>> byProvider;
    private final Map<ApiProvider, UpstreamConfig> defaultByProvider;
    private final Map<String, List<UpstreamConfig>> byChannelCode;
    private final Map<String, UpstreamConfig> defaultByChannelCode;
    private final Map<ApiProvider, Map<String, List<UpstreamConfig>>> byProviderAndModel;
    private final Map<String, Map<String, List<UpstreamConfig>>> byChannelAndModel;

    private UpstreamConfigSnapshot(
            Instant loadedAt,
            List<UpstreamConfig> activeUpstreams,
            Map<String, UpstreamConfig> byName,
            Map<ApiProvider, List<UpstreamConfig>> byProvider,
            Map<ApiProvider, UpstreamConfig> defaultByProvider,
            Map<String, List<UpstreamConfig>> byChannelCode,
            Map<String, UpstreamConfig> defaultByChannelCode,
            Map<ApiProvider, Map<String, List<UpstreamConfig>>> byProviderAndModel,
            Map<String, Map<String, List<UpstreamConfig>>> byChannelAndModel) {
        this.loadedAt = loadedAt;
        this.activeUpstreams = List.copyOf(activeUpstreams);
        this.byName = Map.copyOf(byName);
        this.byProvider = copyNestedListMap(byProvider);
        this.defaultByProvider = Map.copyOf(defaultByProvider);
        this.byChannelCode = copyNestedListMap(byChannelCode);
        this.defaultByChannelCode = Map.copyOf(defaultByChannelCode);
        this.byProviderAndModel = copyDoubleNestedListMap(byProviderAndModel);
        this.byChannelAndModel = copyDoubleNestedListMap(byChannelAndModel);
    }

    public static UpstreamConfigSnapshot empty() {
        return from(List.of());
    }

    public static UpstreamConfigSnapshot from(List<UpstreamConfig> configs) {
        List<UpstreamConfig> activeConfigs = configs == null ? List.of() : configs.stream()
                .filter(Objects::nonNull)
                .filter(config -> Boolean.TRUE.equals(config.getIsActive()))
                .sorted(ROUTING_ORDER)
                .toList();

        Map<String, UpstreamConfig> byName = activeConfigs.stream()
                .filter(config -> config.getName() != null && !config.getName().isBlank())
                .collect(Collectors.toMap(
                        config -> config.getName().trim(),
                        config -> config,
                        (existing, ignored) -> existing,
                        LinkedHashMap::new));

        Map<ApiProvider, List<UpstreamConfig>> byProvider = activeConfigs.stream()
                .filter(config -> config.getProvider() != null)
                .collect(Collectors.groupingBy(
                        UpstreamConfig::getProvider,
                        LinkedHashMap::new,
                        Collectors.collectingAndThen(Collectors.toList(), UpstreamConfigSnapshot::sortedCopy)));

        Map<ApiProvider, UpstreamConfig> defaultByProvider = new LinkedHashMap<>();
        byProvider.forEach((provider, providerConfigs) ->
                chooseDefault(providerConfigs).ifPresent(config -> defaultByProvider.put(provider, config)));

        Map<String, List<UpstreamConfig>> byChannelCode = activeConfigs.stream()
                .collect(Collectors.groupingBy(
                        UpstreamConfigSnapshot::resolveChannelCode,
                        LinkedHashMap::new,
                        Collectors.collectingAndThen(Collectors.toList(), UpstreamConfigSnapshot::sortedCopy)));

        Map<String, UpstreamConfig> defaultByChannelCode = new LinkedHashMap<>();
        byChannelCode.forEach((channelCode, channelConfigs) ->
                chooseDefault(channelConfigs).ifPresent(config -> defaultByChannelCode.put(channelCode, config)));

        Map<ApiProvider, Map<String, List<UpstreamConfig>>> byProviderAndModel = new LinkedHashMap<>();
        Map<String, Map<String, List<UpstreamConfig>>> byChannelAndModel = new LinkedHashMap<>();

        for (UpstreamConfig config : activeConfigs) {
            Set<String> supportedModels = explicitSupportedModels(config);
            if (supportedModels.isEmpty()) {
                continue;
            }
            if (config.getProvider() != null) {
                Map<String, List<UpstreamConfig>> providerModels = byProviderAndModel
                        .computeIfAbsent(config.getProvider(), ignored -> new LinkedHashMap<>());
                supportedModels.forEach(model -> providerModels
                        .computeIfAbsent(model, ignored -> new ArrayList<>())
                        .add(config));
            }

            String channelCode = resolveChannelCode(config);
            Map<String, List<UpstreamConfig>> channelModels = byChannelAndModel
                    .computeIfAbsent(channelCode, ignored -> new LinkedHashMap<>());
            supportedModels.forEach(model -> channelModels
                    .computeIfAbsent(model, ignored -> new ArrayList<>())
                    .add(config));
        }

        sortModelIndexes(byProviderAndModel);
        sortModelIndexes(byChannelAndModel);

        return new UpstreamConfigSnapshot(
                Instant.now(),
                activeConfigs,
                byName,
                byProvider,
                defaultByProvider,
                byChannelCode,
                defaultByChannelCode,
                byProviderAndModel,
                byChannelAndModel);
    }

    public Optional<UpstreamConfig> getByName(String name) {
        String normalizedName = normalizeName(name);
        if (normalizedName == null) {
            return Optional.empty();
        }
        return Optional.ofNullable(byName.get(normalizedName));
    }

    public Optional<UpstreamConfig> getDefaultByProvider(ApiProvider provider) {
        return Optional.ofNullable(defaultByProvider.get(provider));
    }

    public Optional<UpstreamConfig> getDefaultByChannelCode(String channelCode) {
        String normalizedChannelCode = normalizeChannelCode(channelCode);
        if (normalizedChannelCode == null) {
            return Optional.empty();
        }
        return Optional.ofNullable(defaultByChannelCode.get(normalizedChannelCode));
    }

    public boolean hasActiveUpstreamForChannelCode(String channelCode) {
        String normalizedChannelCode = normalizeChannelCode(channelCode);
        return normalizedChannelCode != null
                && byChannelCode.containsKey(normalizedChannelCode)
                && !byChannelCode.get(normalizedChannelCode).isEmpty();
    }

    public boolean hasActiveUpstreamForProviderModel(ApiProvider provider, String model) {
        return !getExplicitProviderModelCandidates(provider, model).isEmpty();
    }

    public boolean hasActiveUpstreamForModel(String model) {
        String normalizedModel = normalizeModel(model);
        if (normalizedModel == null) {
            return false;
        }
        return byProviderAndModel.values().stream()
                .anyMatch(modelIndex -> modelIndex.containsKey(normalizedModel)
                        || modelIndex.containsKey(WILDCARD_MODEL));
    }

    public UpstreamConfig selectForProviderAndModel(ApiProvider provider, String model) {
        UpstreamConfig defaultUpstream = getDefaultByProvider(provider)
                .orElseThrow(() -> new IllegalStateException("No active upstream found for provider: " + provider));

        String normalizedModel = normalizeModel(model);
        if (normalizedModel == null) {
            return defaultUpstream;
        }

        if (supportsModel(defaultUpstream, normalizedModel)) {
            return defaultUpstream;
        }

        List<UpstreamConfig> explicitCandidates = getExplicitProviderModelCandidates(provider, normalizedModel);
        if (!explicitCandidates.isEmpty()) {
            return explicitCandidates.get(0);
        }

        return defaultUpstream;
    }

    public UpstreamConfig selectForChannelAndModel(String channelCode, String model) {
        String normalizedChannelCode = requireChannelCode(channelCode);
        UpstreamConfig defaultUpstream = getDefaultByChannelCode(normalizedChannelCode)
                .orElseThrow(() -> new IllegalStateException(
                        "No active upstream found for channel: " + normalizedChannelCode));

        String normalizedModel = normalizeModel(model);
        if (normalizedModel == null) {
            return defaultUpstream;
        }

        if (supportsModel(defaultUpstream, normalizedModel)) {
            return defaultUpstream;
        }

        List<UpstreamConfig> explicitCandidates = getExplicitChannelModelCandidates(
                normalizedChannelCode, normalizedModel);
        if (!explicitCandidates.isEmpty()) {
            return explicitCandidates.get(0);
        }

        return defaultUpstream;
    }

    public List<UpstreamConfig> getProviderFailoverCandidates(
            ApiProvider provider, String model, List<String> excludedNames) {
        List<UpstreamConfig> explicitCandidates = getExplicitProviderModelCandidates(provider, model);
        if (!explicitCandidates.isEmpty()) {
            return excludeByName(explicitCandidates, excludedNames);
        }
        return excludeByName(byProvider.getOrDefault(provider, List.of()), excludedNames);
    }

    public List<UpstreamConfig> getChannelFailoverCandidates(
            String channelCode, String model, List<String> excludedNames) {
        String normalizedChannelCode = requireChannelCode(channelCode);
        List<UpstreamConfig> explicitCandidates = getExplicitChannelModelCandidates(
                normalizedChannelCode, model);
        if (!explicitCandidates.isEmpty()) {
            return excludeByName(explicitCandidates, excludedNames);
        }
        return excludeByName(byChannelCode.getOrDefault(normalizedChannelCode, List.of()), excludedNames);
    }

    public List<UpstreamConfig> getSupportedModelCandidates(String model, List<String> excludedNames) {
        String normalizedModel = normalizeModel(model);
        if (normalizedModel == null) {
            return List.of();
        }

        List<UpstreamConfig> candidates = byProviderAndModel.values().stream()
                .flatMap(modelIndex -> mergeModelCandidates(modelIndex, normalizedModel).stream())
                .distinct()
                .sorted(ROUTING_ORDER)
                .toList();
        return excludeByName(candidates, excludedNames);
    }

    public Map<String, Object> diagnostics() {
        Map<String, Object> result = new LinkedHashMap<>();
        result.put("loadedAt", loadedAt.toString());
        result.put("activeCount", activeUpstreams.size());
        result.put("byNameCount", byName.size());
        result.put("providerCount", byProvider.size());
        result.put("channelCount", byChannelCode.size());
        result.put("providerModelRouteCount", countModelRoutes(byProviderAndModel));
        result.put("channelModelRouteCount", countModelRoutes(byChannelAndModel));
        result.put("providerDefaults", defaultByProvider.entrySet().stream()
                .collect(Collectors.toMap(
                        entry -> entry.getKey().name(),
                        entry -> entry.getValue().getName(),
                        (existing, ignored) -> existing,
                        LinkedHashMap::new)));
        result.put("channelDefaults", defaultByChannelCode.entrySet().stream()
                .collect(Collectors.toMap(
                        Map.Entry::getKey,
                        entry -> entry.getValue().getName(),
                        (existing, ignored) -> existing,
                        LinkedHashMap::new)));
        return result;
    }

    public Map<String, UpstreamConfig> getByNameSnapshot() {
        return byName;
    }

    public static boolean supportsModel(UpstreamConfig config, String model) {
        String normalizedModel = normalizeModel(model);
        if (config == null || normalizedModel == null) {
            return false;
        }
        return explicitSupportedModels(config).contains(normalizedModel)
                || explicitSupportedModels(config).contains(WILDCARD_MODEL);
    }

    public static boolean allowsModel(UpstreamConfig config, String model) {
        String normalizedModel = normalizeModel(model);
        if (config == null || normalizedModel == null) {
            return true;
        }
        Set<String> supportedModels = explicitSupportedModels(config);
        return supportedModels.isEmpty()
                || supportedModels.contains(WILDCARD_MODEL)
                || supportedModels.contains(normalizedModel);
    }

    private List<UpstreamConfig> getExplicitProviderModelCandidates(ApiProvider provider, String model) {
        if (provider == null) {
            return List.of();
        }
        String normalizedModel = normalizeModel(model);
        if (normalizedModel == null) {
            return List.of();
        }
        Map<String, List<UpstreamConfig>> modelIndex = byProviderAndModel.getOrDefault(provider, Map.of());
        return mergeModelCandidates(modelIndex, normalizedModel);
    }

    private List<UpstreamConfig> getExplicitChannelModelCandidates(String channelCode, String model) {
        String normalizedChannelCode = normalizeChannelCode(channelCode);
        String normalizedModel = normalizeModel(model);
        if (normalizedChannelCode == null || normalizedModel == null) {
            return List.of();
        }
        Map<String, List<UpstreamConfig>> modelIndex = byChannelAndModel.getOrDefault(normalizedChannelCode, Map.of());
        return mergeModelCandidates(modelIndex, normalizedModel);
    }

    private static List<UpstreamConfig> mergeModelCandidates(
            Map<String, List<UpstreamConfig>> modelIndex, String normalizedModel) {
        List<UpstreamConfig> candidates = new ArrayList<>();
        candidates.addAll(modelIndex.getOrDefault(normalizedModel, List.of()));
        candidates.addAll(modelIndex.getOrDefault(WILDCARD_MODEL, List.of()));
        return candidates.stream()
                .distinct()
                .sorted(ROUTING_ORDER)
                .toList();
    }

    private static List<UpstreamConfig> excludeByName(List<UpstreamConfig> candidates, List<String> excludedNames) {
        if (excludedNames == null || excludedNames.isEmpty()) {
            return candidates;
        }
        Set<String> excluded = excludedNames.stream()
                .filter(Objects::nonNull)
                .collect(Collectors.toSet());
        return candidates.stream()
                .filter(config -> !excluded.contains(config.getName()))
                .toList();
    }

    private static Optional<UpstreamConfig> chooseDefault(List<UpstreamConfig> candidates) {
        if (candidates == null || candidates.isEmpty()) {
            return Optional.empty();
        }
        return candidates.stream()
                .filter(config -> Boolean.TRUE.equals(config.getIsDefault()))
                .findFirst()
                .or(() -> Optional.of(candidates.get(0)));
    }

    private static Set<String> explicitSupportedModels(UpstreamConfig config) {
        if (config == null || config.getSupportedModels() == null) {
            return Set.of();
        }
        return config.getSupportedModels().stream()
                .map(UpstreamConfigSnapshot::normalizeModel)
                .filter(Objects::nonNull)
                .collect(Collectors.toSet());
    }

    private static String resolveChannelCode(UpstreamConfig config) {
        String channelCode = normalizeChannelCode(config.getChannelCode());
        if (channelCode != null) {
            return channelCode;
        }
        ApiProvider provider = config.getProvider();
        return provider == null ? "" : provider.name().toLowerCase(Locale.ROOT);
    }

    private static String requireChannelCode(String channelCode) {
        String normalizedChannelCode = normalizeChannelCode(channelCode);
        if (normalizedChannelCode == null) {
            throw new IllegalArgumentException("channel code must not be blank");
        }
        return normalizedChannelCode;
    }

    private static String normalizeChannelCode(String channelCode) {
        if (channelCode == null || channelCode.isBlank()) {
            return null;
        }
        return channelCode.trim().toLowerCase(Locale.ROOT);
    }

    private static String normalizeModel(String model) {
        if (model == null || model.isBlank()) {
            return null;
        }
        return model.trim().toLowerCase(Locale.ROOT);
    }

    private static String normalizeName(String name) {
        if (name == null || name.isBlank()) {
            return null;
        }
        return name.trim();
    }

    private static int priorityOf(UpstreamConfig config) {
        return config.getPriority() == null ? Integer.MAX_VALUE : config.getPriority();
    }

    private static String nullToEmpty(String value) {
        return value == null ? "" : value;
    }

    private static List<UpstreamConfig> sortedCopy(List<UpstreamConfig> configs) {
        return configs.stream()
                .sorted(ROUTING_ORDER)
                .toList();
    }

    private static <K> Map<K, List<UpstreamConfig>> copyNestedListMap(Map<K, List<UpstreamConfig>> source) {
        Map<K, List<UpstreamConfig>> copy = new LinkedHashMap<>();
        source.forEach((key, value) -> copy.put(key, List.copyOf(value)));
        return Map.copyOf(copy);
    }

    private static <K> Map<K, Map<String, List<UpstreamConfig>>> copyDoubleNestedListMap(
            Map<K, Map<String, List<UpstreamConfig>>> source) {
        Map<K, Map<String, List<UpstreamConfig>>> copy = new LinkedHashMap<>();
        source.forEach((key, value) -> copy.put(key, copyNestedListMap(value)));
        return Map.copyOf(copy);
    }

    private static <K> void sortModelIndexes(Map<K, Map<String, List<UpstreamConfig>>> index) {
        index.values().forEach(modelIndex -> modelIndex.replaceAll((model, configs) -> sortedCopy(configs)));
    }

    private static int countModelRoutes(Map<?, Map<String, List<UpstreamConfig>>> index) {
        return index.values().stream()
                .mapToInt(Map::size)
                .sum();
    }
}
