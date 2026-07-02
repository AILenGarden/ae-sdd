package ai.nubase.functions.executor;

import ai.nubase.functions.executor.cloudflare.CloudflareEdgeFunctionExecutor;
import lombok.RequiredArgsConstructor;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.stereotype.Component;

@Component
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.functions.enabled", havingValue = "true", matchIfMissing = true)
public class EdgeFunctionExecutorRouter implements EdgeFunctionExecutor {

    private final EdgeFunctionExecutorProperties properties;
    private final LocalHttpEdgeFunctionExecutor localExecutor;
    private final CloudflareEdgeFunctionExecutor cloudflareExecutor;

    @Override
    public String provider() {
        return delegate().provider();
    }

    @Override
    public EdgeFunctionDeploymentResponse deploy(EdgeFunctionDeploymentRequest request) {
        return delegate().deploy(request);
    }

    @Override
    public void delete(String projectRef, String functionSlug, String providerDeploymentId) {
        delegate().delete(projectRef, functionSlug, providerDeploymentId);
    }

    @Override
    public EdgeFunctionInvocationResponse invoke(EdgeFunctionInvocationRequest request) {
        return delegate().invoke(request);
    }

    @Override
    public boolean injectsEnvAtInvoke() {
        return delegate().injectsEnvAtInvoke();
    }

    @Override
    public void syncSecrets(String projectRef, String functionSlug, String providerDeploymentId, java.util.Map<String, String> env) {
        delegate().syncSecrets(projectRef, functionSlug, providerDeploymentId, env);
    }

    private EdgeFunctionExecutor delegate() {
        if ("cloudflare".equalsIgnoreCase(properties.getProvider())) {
            return cloudflareExecutor;
        }
        return localExecutor;
    }
}
