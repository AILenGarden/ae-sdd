package ai.nubase.deploy.service;

import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.stereotype.Component;

@Component
@ConditionalOnProperty(value = "nubase.deploy.app-worker.cloudflare.enabled", havingValue = "false", matchIfMissing = true)
public class DisabledAppWorkerDeployer implements AppWorkerDeployer {

    private static final String DISABLED = "App worker deployment requires nubase.deploy.app-worker.cloudflare.enabled=true";

    @Override
    public AppWorkerDeploymentResult deploy(AppWorkerDeploymentRequest request) {
        throw new AppWorkerDeploymentException(DISABLED);
    }

    @Override
    public AppWorkerInfo get(String workerName) {
        throw new AppWorkerDeploymentException(DISABLED);
    }

    @Override
    public void delete(String workerName) {
        throw new AppWorkerDeploymentException(DISABLED);
    }
}
