package ai.nubase.deploy.service;

import org.springframework.util.StringUtils;

public enum AppWorkerDeploymentTarget {
    PREVIEW("preview"),
    PRODUCTION("production");

    private final String value;

    AppWorkerDeploymentTarget(String value) {
        this.value = value;
    }

    public String value() {
        return value;
    }

    public static AppWorkerDeploymentTarget from(String raw) {
        if (!StringUtils.hasText(raw)) {
            return PREVIEW;
        }
        for (AppWorkerDeploymentTarget target : values()) {
            if (target.value.equalsIgnoreCase(raw.trim())) {
                return target;
            }
        }
        throw new AppWorkerDeploymentException("Invalid app worker deploymentTarget: " + raw);
    }
}
