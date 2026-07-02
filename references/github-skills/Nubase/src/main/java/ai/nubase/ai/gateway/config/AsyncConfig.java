package ai.nubase.ai.gateway.config;

import org.springframework.context.annotation.Configuration;
import org.springframework.scheduling.annotation.EnableAsync;

/**
 * 异步配置
 * 用于支持异步的API使用量跟踪
 */
@Configuration
@EnableAsync
public class AsyncConfig {
}
