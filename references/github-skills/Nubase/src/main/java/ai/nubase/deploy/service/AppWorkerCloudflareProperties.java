package ai.nubase.deploy.service;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

import java.util.LinkedHashMap;
import java.util.Map;

@Data
@Component
@ConfigurationProperties(prefix = "nubase.deploy.app-worker.cloudflare")
public class AppWorkerCloudflareProperties {
    private String apiBaseUrl = "https://api.cloudflare.com/client/v4";
    private String accountId = "";
    private String apiToken = "";
    private int timeoutMs = 30000;
    private Map<String, String> dispatchNamespaces = new LinkedHashMap<>();

    public String dispatchNamespace(AppWorkerDeploymentTarget target) {
        return dispatchNamespaces == null || target == null ? null : dispatchNamespaces.get(target.value());
    }
}
