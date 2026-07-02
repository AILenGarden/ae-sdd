package ai.nubase.common.service;

import ai.nubase.common.context.MultiTenancyContext;
import org.springframework.stereotype.Service;

/**
 * 网关数据面客户端鉴权的轻量桥接。
 * <p>
 * 真正的鉴权由 {@code GatewayApiKeyAuthFilter} 完成：它校验 {@code nbk_<appCode>_<secret>} 形式的
 * 项目密钥，解析出对应项目并设置 {@link MultiTenancyContext}。控制器无需重复校验密钥，
 * 这里只确认请求确实已落在某个项目（租户）上下文中且携带了密钥。
 */
@Service
public class AppAuthService {

    /**
     * 当请求已通过网关密钥过滤器（存在租户上下文）且携带了非空客户端密钥时返回 true。
     */
    public boolean validatePrivateKey(String clientApiKey) {
        return clientApiKey != null && !clientApiKey.isBlank() && MultiTenancyContext.hasContext();
    }
}
