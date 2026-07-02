package ai.nubase.ai.gateway.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

/**
 * AI 网关模块配置。
 * <p>
 * {@code enabled} 控制新建项目时是否为其租户库创建 {@code ai_gateway} schema（与 {@code nubase.mem.enabled}
 * 同款机制：关闭时 {@link ai.nubase.auth.service.DatabaseInitService} 跳过建表并剥离 init_roles.sql 中的
 * AI_GATEWAY 授权块）。
 */
@Data
@Component
@ConfigurationProperties(prefix = "nubase.ai-gateway")
public class AiGatewayProperties {

    /** 是否为新项目启用 AI 网关（创建 ai_gateway schema）。 */
    private boolean enabled = true;
}
