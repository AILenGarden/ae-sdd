package ai.nubase.ai.gateway.service;

import ai.nubase.ai.gateway.repository.UpstreamConfigRepository;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.LocalDateTime;

/**
 * 写入上游最后使用时间。
 * <p>
 * 在请求线程内同步写入（必须如此）：上游表位于当前租户库的 ai_gateway schema，
 * 依赖请求线程上的 {@code MultiTenancyContext} 路由；脱离请求的定时批处理会丢失租户上下文。
 * 写入为「尽力而为」，失败不影响转发。
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class UpstreamUsageTouchService {

    private final UpstreamConfigRepository upstreamConfigRepository;

    @Transactional
    public void touch(String name) {
        if (name == null || name.isBlank() || "config-file".equals(name)) {
            return;
        }
        try {
            int updated = upstreamConfigRepository.updateLastUsedAtByName(name, LocalDateTime.now());
            if (updated == 0) {
                log.debug("【upstream_usage_touch】no upstream row found for name={}", name);
            }
        } catch (Exception e) {
            log.debug("【upstream_usage_touch】failed to touch name={}: {}", name, e.getMessage());
        }
    }
}
