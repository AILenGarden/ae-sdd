package ai.nubase.functions.executor;

import java.util.Map;

public interface EdgeFunctionExecutor {

    String provider();

    EdgeFunctionDeploymentResponse deploy(EdgeFunctionDeploymentRequest request);

    void delete(String projectRef, String functionSlug, String providerDeploymentId);

    EdgeFunctionInvocationResponse invoke(EdgeFunctionInvocationRequest request);

    /**
     * Whether this executor expects function env (secrets) to be supplied on every
     * invocation. Executors that bind env at deploy time (e.g. Cloudflare) return false
     * so the caller can skip decrypting secrets on the hot path.
     */
    default boolean injectsEnvAtInvoke() {
        return false;
    }

    /**
     * Applies the given env to an existing deployment without redeploying, so secret
     * changes take effect immediately. No-op for executors that read env at invoke time.
     */
    default void syncSecrets(String projectRef, String functionSlug, String providerDeploymentId, Map<String, String> env) {
    }
}
