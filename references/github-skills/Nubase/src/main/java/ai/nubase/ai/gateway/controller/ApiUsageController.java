package ai.nubase.ai.gateway.controller;

import ai.nubase.ai.gateway.entity.ApiUsageLog;
import ai.nubase.ai.gateway.entity.DailyTokenUsage;
import ai.nubase.ai.gateway.repository.ApiUsageLogRepository;
import ai.nubase.ai.gateway.repository.DailyTokenUsageRepository;
import ai.nubase.auth.annotation.RequireServiceRole;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.math.BigDecimal;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.util.*;

/**
 * 项目级用量统计与请求日志（控制面）。
 * <p>作用于当前项目租户库的 ai_gateway.{daily_token_usage, api_usage_logs}，按项目（整库）聚合，
 * 可选按 api_key_id 过滤。租户由 service_role apikey 解析，{@link RequireServiceRole} 限定访问。
 */
@Slf4j
@RestController
@RequestMapping("/ai-gateway/admin/v1/usage")
@RequiredArgsConstructor
@RequireServiceRole
public class ApiUsageController {

    private final DailyTokenUsageRepository dailyUsageRepository;
    private final ApiUsageLogRepository apiUsageLogRepository;

    /** 项目总览：时间窗内总 token / 请求 / 费用 + 近 14 天曲线 + 首 token 平均延迟。 */
    @GetMapping("/overview")
    public ResponseEntity<Map<String, Object>> overview(
            @RequestParam(value = "start_date", required = false) String startDate,
            @RequestParam(value = "end_date", required = false) String endDate) {
        LocalDate end = endDate == null ? LocalDate.now() : LocalDate.parse(endDate);
        LocalDate start = startDate == null ? end.withDayOfMonth(1) : LocalDate.parse(startDate);

        Long totalTokens = dailyUsageRepository.sumTokensByDateBetween(start, end);
        Long totalRequests = dailyUsageRepository.sumRequestCountByDateBetween(start, end);
        BigDecimal totalCostUsd = dailyUsageRepository.sumCostUsdByDateBetween(start, end);

        LocalDate today = LocalDate.now();
        Map<LocalDate, long[]> daily = new TreeMap<>();
        for (int i = 13; i >= 0; i--) {
            daily.put(today.minusDays(i), new long[]{0, 0});
        }
        for (DailyTokenUsage s : dailyUsageRepository
                .findByUsageDateBetweenOrderByUsageDateDesc(today.minusDays(13), today)) {
            long[] cur = daily.get(s.getUsageDate());
            if (cur != null) {
                cur[0] += s.getTotalTokens() == null ? 0 : s.getTotalTokens();
                cur[1] += s.getRequestCount() == null ? 0 : s.getRequestCount();
            }
        }
        List<Map<String, Object>> series = new ArrayList<>();
        for (Map.Entry<LocalDate, long[]> e : daily.entrySet()) {
            Map<String, Object> point = new LinkedHashMap<>();
            point.put("date", e.getKey().toString().substring(5));
            point.put("tokens", e.getValue()[0]);
            point.put("requests", e.getValue()[1]);
            series.add(point);
        }

        Double avgFirstToken = apiUsageLogRepository.averageFirstTokenLatencyMsByCreatedAtRange(
                start.atStartOfDay(), end.plusDays(1).atStartOfDay());

        Map<String, Object> resp = new LinkedHashMap<>();
        resp.put("totalTokens", totalTokens == null ? 0 : totalTokens);
        resp.put("totalRequests", totalRequests == null ? 0 : totalRequests);
        resp.put("totalCostUsd", totalCostUsd == null ? BigDecimal.ZERO : totalCostUsd);
        resp.put("avgFirstTokenLatencyMs", avgFirstToken == null ? 0 : Math.round(avgFirstToken));
        resp.put("series", series);
        resp.put("startDate", start.toString());
        resp.put("endDate", end.toString());
        return ResponseEntity.ok(resp);
    }

    /** 每日 token / 费用统计（可选按 api_key_id 过滤）。 */
    @GetMapping("/daily")
    public ResponseEntity<Map<String, Object>> daily(
            @RequestParam(value = "api_key_id", required = false) Long apiKeyId,
            @RequestParam(value = "start_date", required = false) String startDate,
            @RequestParam(value = "end_date", required = false) String endDate) {
        LocalDate end = endDate == null ? LocalDate.now() : LocalDate.parse(endDate);
        LocalDate start = startDate == null ? end.minusDays(29) : LocalDate.parse(startDate);

        List<DailyTokenUsage> rows = apiKeyId == null
                ? dailyUsageRepository.findByUsageDateBetweenOrderByUsageDateDesc(start, end)
                : dailyUsageRepository.findByApiKeyIdAndUsageDateBetweenOrderByUsageDateDesc(apiKeyId, start, end);

        List<Map<String, Object>> data = new ArrayList<>();
        for (DailyTokenUsage r : rows) {
            Map<String, Object> dto = new LinkedHashMap<>();
            dto.put("date", r.getUsageDate().toString());
            dto.put("model", r.getModel());
            dto.put("apiKeyId", r.getApiKeyId());
            dto.put("requestCount", r.getRequestCount());
            dto.put("errorCount", r.getErrorCount());
            dto.put("inputTokens", r.getInputTokens());
            dto.put("outputTokens", r.getOutputTokens());
            dto.put("cacheCreationInputTokens", r.getCacheCreationInputTokens());
            dto.put("cacheReadInputTokens", r.getCacheReadInputTokens());
            dto.put("totalTokens", r.getTotalTokens());
            dto.put("costUsd", r.getCostUsd());
            dto.put("costCny", r.getCostCny());
            data.add(dto);
        }
        return ResponseEntity.ok(Map.of("data", data, "startDate", start.toString(), "endDate", end.toString()));
    }

    /** 按模型聚合（项目范围）。 */
    @GetMapping("/by-model")
    public ResponseEntity<Map<String, Object>> byModel(
            @RequestParam(value = "start_date", required = false) String startDate,
            @RequestParam(value = "end_date", required = false) String endDate) {
        LocalDate end = endDate == null ? LocalDate.now() : LocalDate.parse(endDate);
        LocalDate start = startDate == null ? end.minusDays(30) : LocalDate.parse(startDate);

        List<Map<String, Object>> data = new ArrayList<>();
        long totalTokens = 0;
        long totalRequests = 0;
        for (Object[] r : dailyUsageRepository.sumUsageByModelAndDateBetween(start, end)) {
            long tokens = r[1] == null ? 0 : ((Number) r[1]).longValue();
            long reqs = r[2] == null ? 0 : ((Number) r[2]).longValue();
            Map<String, Object> dto = new LinkedHashMap<>();
            dto.put("model", String.valueOf(r[0]));
            dto.put("totalTokens", tokens);
            dto.put("requestCount", reqs);
            dto.put("costUsd", r[3] == null ? BigDecimal.ZERO : r[3]);
            data.add(dto);
            totalTokens += tokens;
            totalRequests += reqs;
        }
        Map<String, Object> resp = new LinkedHashMap<>();
        resp.put("data", data);
        resp.put("totalTokens", totalTokens);
        resp.put("totalRequests", totalRequests);
        resp.put("startDate", start.toString());
        resp.put("endDate", end.toString());
        return ResponseEntity.ok(resp);
    }

    /** 请求日志分页（可选按 api_key_id 过滤）。 */
    @GetMapping("/logs")
    public ResponseEntity<Map<String, Object>> logs(
            @RequestParam(value = "api_key_id", required = false) Long apiKeyId,
            @RequestParam(value = "start_date", required = false) String startDate,
            @RequestParam(value = "end_date", required = false) String endDate,
            @RequestParam(value = "page", defaultValue = "0") int page,
            @RequestParam(value = "size", defaultValue = "20") int size) {
        size = Math.min(Math.max(size, 1), 100);
        LocalDateTime startTime = startDate == null || startDate.isBlank()
                ? null : LocalDate.parse(startDate).atStartOfDay();
        LocalDateTime endTime = endDate == null || endDate.isBlank()
                ? null : LocalDate.parse(endDate).plusDays(1).atStartOfDay();

        Page<ApiUsageLog> p = apiUsageLogRepository.findAll(
                logsSpec(apiKeyId, startTime, endTime),
                PageRequest.of(Math.max(page, 0), size, Sort.by(Sort.Direction.DESC, "createdAt")));

        List<Map<String, Object>> content = new ArrayList<>();
        for (ApiUsageLog l : p.getContent()) {
            Map<String, Object> dto = new LinkedHashMap<>();
            dto.put("id", l.getId());
            dto.put("apiKeyId", l.getApiKeyId());
            dto.put("userId", l.getUserId());
            dto.put("keyPrefix", l.getApiKey());
            dto.put("authMode", l.getUserId() != null && l.getApiKeyId() == null ? "USER_JWT" : "GATEWAY_KEY");
            dto.put("requestId", l.getRequestId());
            dto.put("model", l.getModel());
            dto.put("endpoint", l.getEndpoint());
            dto.put("statusCode", l.getStatusCode());
            dto.put("inputTokens", l.getInputTokens());
            dto.put("outputTokens", l.getOutputTokens());
            dto.put("totalTokens", l.getTotalTokens());
            dto.put("cacheCreationInputTokens", l.getCacheCreationInputTokens());
            dto.put("cacheReadInputTokens", l.getCacheReadInputTokens());
            dto.put("durationMs", l.getDurationMs());
            dto.put("firstTokenLatencyMs", l.getFirstTokenLatencyMs());
            dto.put("costUsd", l.getCostUsd());
            dto.put("errorMessage", l.getErrorMessage());
            dto.put("createdAt", l.getCreatedAt());
            content.add(dto);
        }
        Map<String, Object> resp = new LinkedHashMap<>();
        resp.put("content", content);
        resp.put("totalElements", p.getTotalElements());
        resp.put("totalPages", p.getTotalPages());
        resp.put("number", p.getNumber());
        resp.put("size", p.getSize());
        return ResponseEntity.ok(resp);
    }

    private Specification<ApiUsageLog> logsSpec(Long apiKeyId, LocalDateTime startTime, LocalDateTime endTime) {
        return (root, query, cb) -> {
            List<jakarta.persistence.criteria.Predicate> predicates = new ArrayList<>();
            if (apiKeyId != null) {
                predicates.add(cb.equal(root.get("apiKeyId"), apiKeyId));
            }
            if (startTime != null) {
                predicates.add(cb.greaterThanOrEqualTo(root.get("createdAt"), startTime));
            }
            if (endTime != null) {
                predicates.add(cb.lessThan(root.get("createdAt"), endTime));
            }
            return cb.and(predicates.toArray(new jakarta.persistence.criteria.Predicate[0]));
        };
    }
}
