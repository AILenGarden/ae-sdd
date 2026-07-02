package ai.nubase.ai.gateway.config;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.client.SimpleClientHttpRequestFactory;
import org.springframework.web.client.RestTemplate;

/**
 * Web 客户端配置
 * 提供 RestTemplate 等 HTTP 客户端相关的 Bean
 */
@Configuration
public class WebClientConfig {

    /**
     * 配置 RestTemplate Bean
     * 用于健康检查服务调用上游 API
     */
    @Bean
    public RestTemplate restTemplate() {
        SimpleClientHttpRequestFactory factory = new SimpleClientHttpRequestFactory();

        // 设置连接超时时间（10秒）
        factory.setConnectTimeout(10000);

        // 设置读取超时时间（30秒）
        // 注意：实际的超时时间由 UpstreamConfig 中的 timeoutMs 控制
        factory.setReadTimeout(30000);

        return new RestTemplate(factory);
    }
}