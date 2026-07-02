package ai.nubase.ai.gateway.controller;

import ai.nubase.ai.gateway.dto.UpstreamConfigResponse;
import ai.nubase.ai.gateway.entity.UpstreamConfig;
import ai.nubase.ai.gateway.service.UpstreamConfigService;
import ai.nubase.auth.annotation.RequireServiceRole;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;

/**
 * 项目级上游网关管理（控制面）。
 * <p>作用于当前项目租户库的 {@code ai_gateway.upstream_configs}。租户由 UnifiedMultiTenancyFilter 通过
 * 项目 service_role apikey 解析；{@link RequireServiceRole} 限定仅服务角色可访问。
 */
@Slf4j
@RestController
@RequestMapping("/ai-gateway/admin/v1/upstreams")
@RequiredArgsConstructor
@RequireServiceRole
public class UpstreamConfigController {

    private final UpstreamConfigService upstreamConfigService;

    /** 列出本项目全部上游配置（含未激活）。 */
    @GetMapping
    public ResponseEntity<List<UpstreamConfigResponse>> list() {
        return ResponseEntity.ok(upstreamConfigService.getAllUpstreams()
                .stream()
                .map(UpstreamConfigResponse::from)
                .toList());
    }

    @GetMapping("/{id}")
    public ResponseEntity<UpstreamConfigResponse> get(@PathVariable Long id) {
        return upstreamConfigService.getById(id)
                .map(UpstreamConfigResponse::from)
                .map(ResponseEntity::ok)
                .orElseGet(() -> ResponseEntity.notFound().build());
    }

    /** 新建上游配置。 */
    @PostMapping
    public ResponseEntity<UpstreamConfigResponse> create(@RequestBody UpstreamConfig body) {
        body.setId(null);
        return ResponseEntity.ok(UpstreamConfigResponse.from(upstreamConfigService.save(body)));
    }

    /** 更新上游配置。 */
    @PutMapping("/{id}")
    public ResponseEntity<?> update(@PathVariable Long id, @RequestBody UpstreamConfig body) {
        return upstreamConfigService.getById(id)
                .map(existing -> {
                    body.setId(id);
                    body.setCreatedAt(existing.getCreatedAt());
                    if (body.getAuthToken() == null || body.getAuthToken().isBlank()) {
                        body.setAuthToken(existing.getAuthToken());
                    }
                    return ResponseEntity.ok(UpstreamConfigResponse.from(upstreamConfigService.save(body)));
                })
                .orElseGet(() -> ResponseEntity.notFound().build());
    }

    @DeleteMapping("/{id}")
    public ResponseEntity<Map<String, Object>> delete(@PathVariable Long id) {
        upstreamConfigService.deleteById(id);
        return ResponseEntity.ok(Map.of("ok", true));
    }

    /** 刷新当前项目的上游路由缓存。 */
    @PostMapping("/cache/refresh")
    public ResponseEntity<Map<String, Object>> refreshCache() {
        return ResponseEntity.ok(upstreamConfigService.reloadActiveUpstreamSnapshot());
    }
}
