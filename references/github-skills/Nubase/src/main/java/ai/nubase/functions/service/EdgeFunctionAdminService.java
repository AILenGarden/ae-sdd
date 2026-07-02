package ai.nubase.functions.service;

import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.functions.dto.EdgeFunctionDtos.CreateFunctionRequest;
import ai.nubase.functions.dto.EdgeFunctionDtos.DeployFunctionRequest;
import ai.nubase.functions.dto.EdgeFunctionDtos.SetFunctionSecretsRequest;
import ai.nubase.functions.dto.EdgeFunctionDtos.UpdateFunctionRequest;
import ai.nubase.functions.executor.EdgeFunctionDeploymentRequest;
import ai.nubase.functions.executor.EdgeFunctionDeploymentResponse;
import ai.nubase.functions.executor.EdgeFunctionExecutorRouter;
import ai.nubase.functions.util.EdgeFunctionNames;
import ai.nubase.metadata.edge.entity.EdgeFunction;
import ai.nubase.metadata.edge.entity.EdgeFunctionInvocation;
import ai.nubase.metadata.edge.entity.EdgeFunctionSecret;
import ai.nubase.metadata.edge.entity.EdgeFunctionVersion;
import ai.nubase.metadata.edge.repository.EdgeFunctionInvocationRepository;
import ai.nubase.metadata.edge.repository.EdgeFunctionRepository;
import ai.nubase.metadata.edge.repository.EdgeFunctionSecretRepository;
import ai.nubase.metadata.edge.repository.EdgeFunctionVersionRepository;
import ai.nubase.postgrest.multidb.EncryptionService;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.data.domain.PageRequest;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import org.springframework.util.StringUtils;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import static ai.nubase.functions.service.EdgeFunctionExceptions.EdgeFunctionException;

@Slf4j
@Service
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.functions.enabled", havingValue = "true", matchIfMissing = true)
public class EdgeFunctionAdminService {

    private final EdgeFunctionRepository functionRepository;
    private final EdgeFunctionVersionRepository versionRepository;
    private final EdgeFunctionSecretRepository secretRepository;
    private final EdgeFunctionInvocationRepository invocationRepository;
    private final EdgeFunctionExecutorRouter executor;
    private final EdgeFunctionDeploymentRecorder deploymentRecorder;
    private final EncryptionService encryptionService;
    private final EdgeFunctionSecretEnv secretEnv;
    private final EdgeFunctionSecretWriter secretWriter;
    private final ai.nubase.functions.executor.EdgeFunctionExecutorProperties executorProperties;

    @Transactional(transactionManager = "metadataTransactionManager", readOnly = true)
    public List<EdgeFunction> listFunctions() {
        return functionRepository.findByProjectRefOrderByCreatedAtDesc(projectRef());
    }

    @Transactional(transactionManager = "metadataTransactionManager", readOnly = true)
    public EdgeFunction getFunction(String slug) {
        return findFunction(slug);
    }

    @Transactional("metadataTransactionManager")
    public EdgeFunction createFunction(CreateFunctionRequest request) {
        rejectPrivileged(request.privileged());
        String projectRef = projectRef();
        String slug = normalizeSlug(StringUtils.hasText(request.slug()) ? request.slug() : request.name());
        if (functionRepository.existsByProjectRefAndSlug(projectRef, slug)) {
            throw new EdgeFunctionException(HttpStatus.CONFLICT, "FUNCTION_EXISTS", "Function already exists");
        }
        EdgeFunction fn = EdgeFunction.builder()
                .projectRef(projectRef)
                .name(request.name().trim())
                .slug(slug)
                .description(request.description())
                .verifyJwt(request.verifyJwt())
                .enabled(request.enabled())
                .privileged(request.privileged())
                .importMap(request.importMap())
                .entrypoint(StringUtils.hasText(request.entrypoint()) ? request.entrypoint().trim() : "index.ts")
                .build();
        return functionRepository.save(fn);
    }

    @Transactional("metadataTransactionManager")
    public EdgeFunction updateFunction(String slug, UpdateFunctionRequest request) {
        rejectPrivileged(request.privileged());
        EdgeFunction fn = findFunction(slug);
        if (StringUtils.hasText(request.name())) fn.setName(request.name().trim());
        if (request.description() != null) fn.setDescription(request.description());
        if (request.verifyJwt() != null) fn.setVerifyJwt(request.verifyJwt());
        if (request.enabled() != null) fn.setEnabled(request.enabled());
        if (request.privileged() != null) fn.setPrivileged(Boolean.FALSE);
        if (request.importMap() != null) fn.setImportMap(request.importMap());
        if (StringUtils.hasText(request.entrypoint())) fn.setEntrypoint(request.entrypoint().trim());
        return functionRepository.save(fn);
    }

    public EdgeFunctionVersion deploy(String slug, DeployFunctionRequest request) {
        EdgeFunction fn = findFunction(slug);
        request = validateAndCanonicalizeBundle(request);
        EdgeFunctionVersion previousActive = fn.getActiveVersion();
        String previousDeploymentId = previousActive == null ? null : previousActive.getProviderDeploymentId();
        EdgeFunctionDeploymentResponse deployment = executor.deploy(new EdgeFunctionDeploymentRequest(
                fn.getProjectRef(),
                fn.getSlug(),
                fn.getEntrypoint(),
                request.sourceBundleBase64(),
                secretEnv.decryptedEnv(fn)
        ));
        EdgeFunctionVersion saved;
        try {
            saved = deploymentRecorder.record(fn.getId(), request, deployment);
        } catch (Exception e) {
            // The provider upload already happened (same worker name → overwritten in
            // place); without this record the live code and version metadata have
            // diverged. The bundle is not retained, so compensation is impossible —
            // make the divergence loudly observable instead.
            log.error("Edge function deployed to provider but metadata record failed — live code and "
                            + "version history have diverged: projectRef={}, slug={}, providerDeploymentId={}",
                    fn.getProjectRef(), fn.getSlug(), deployment.providerDeploymentId(), e);
            throw e;
        }
        // A provider-id change (e.g. the worker naming-scheme migration) leaves the
        // previous worker deployed with its old code and secret bindings; delete it
        // once superseded. Best-effort: delete already tolerates 404 and a failure
        // must not fail the deploy.
        if ("deployed".equals(saved.getStatus())
                && StringUtils.hasText(previousDeploymentId)
                && !previousDeploymentId.equals(saved.getProviderDeploymentId())) {
            try {
                executor.delete(fn.getProjectRef(), fn.getSlug(), previousDeploymentId);
            } catch (Exception e) {
                log.warn("Failed to delete superseded deployment: projectRef={}, slug={}, oldDeploymentId={}, error={}",
                        fn.getProjectRef(), fn.getSlug(), previousDeploymentId, e.toString());
            }
        }
        return saved;
    }

    // The recorded sourceHash must describe the bundle that was actually deployed —
    // a client-supplied hash would let version history lie about its content,
    // breaking any future dedupe/rollback-by-hash. Also bounds the bundle size:
    // the admin endpoint has no other payload limit.
    private DeployFunctionRequest validateAndCanonicalizeBundle(DeployFunctionRequest request) {
        if (!StringUtils.hasText(request.sourceBundleBase64())) {
            return request; // placeholder deploys carry no bundle
        }
        long approxBytes = request.sourceBundleBase64().length() * 3L / 4;
        if (approxBytes > executorProperties.getMaxRequestBytes()) {
            throw new EdgeFunctionException(HttpStatus.PAYLOAD_TOO_LARGE, "BUNDLE_TOO_LARGE",
                    "Source bundle exceeds the maximum size of " + executorProperties.getMaxRequestBytes() + " bytes");
        }
        byte[] decoded;
        try {
            decoded = java.util.Base64.getDecoder().decode(request.sourceBundleBase64());
        } catch (IllegalArgumentException e) {
            throw new EdgeFunctionException(HttpStatus.BAD_REQUEST, "INVALID_BUNDLE", "sourceBundleBase64 is not valid base64");
        }
        String serverHash = sha256Hex(decoded);
        return new DeployFunctionRequest(serverHash, request.artifactUri(), request.artifactType(), request.sourceBundleBase64());
    }

    private String sha256Hex(byte[] bytes) {
        try {
            return java.util.HexFormat.of().formatHex(
                    java.security.MessageDigest.getInstance("SHA-256").digest(bytes));
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    public void deleteFunction(String slug) {
        EdgeFunction fn = findFunction(slug);
        EdgeFunctionVersion active = fn.getActiveVersion();
        if (active != null) {
            executor.delete(fn.getProjectRef(), fn.getSlug(), active.getProviderDeploymentId());
        }
        functionRepository.delete(fn);
    }

    public List<EdgeFunctionSecret> setSecrets(String slug, SetFunctionSecretsRequest request) {
        EdgeFunction fn = findFunction(slug);
        if (request.secrets() == null || request.secrets().isEmpty()) {
            return secretRepository.findByFunctionOrderByNameAsc(fn);
        }
        // Phase 1 — validate and encrypt the WHOLE batch before any write: a failure
        // on the Nth entry must leave zero side effects, never a half-applied set.
        Map<String, String> encrypted = new LinkedHashMap<>();
        for (var entry : request.secrets().entrySet()) {
            String name = entry.getKey();
            if (!EdgeFunctionNames.isValidSecretName(name)) {
                throw new EdgeFunctionException(HttpStatus.BAD_REQUEST, "INVALID_SECRET_NAME", "Invalid secret name: " + name);
            }
            try {
                encrypted.put(name, encryptionService.encrypt(entry.getValue()));
            } catch (Exception e) {
                throw new EdgeFunctionException(HttpStatus.INTERNAL_SERVER_ERROR, "SECRET_ENCRYPTION_FAILED", "Failed to encrypt function secret");
            }
        }
        // Phase 2 — persist the batch in one short metadata transaction.
        List<EdgeFunctionSecret> toSave = new ArrayList<>();
        for (var entry : encrypted.entrySet()) {
            EdgeFunctionSecret secret = secretRepository.findByFunctionAndName(fn, entry.getKey())
                    .orElseGet(() -> EdgeFunctionSecret.builder()
                            .function(fn)
                            .name(entry.getKey())
                            .build());
            secret.setEncryptedValue(entry.getValue());
            toSave.add(secret);
        }
        secretWriter.saveAll(toSave);
        // Phase 3 — visibility and provider sync, outside any transaction.
        // Same-instance readers (the local-executor invoke path) must see the new
        // values immediately; other instances converge within the cache TTL.
        secretEnv.evict(fn.getId());
        syncSecretsToActiveDeployment(fn, request.secrets());
        return secretRepository.findByFunctionOrderByNameAsc(fn);
    }

    // Deploy-time-bound executors (Cloudflare) would otherwise keep serving stale
    // secrets until the next manual deploy. Only the CHANGED entries are pushed
    // (their plaintext is already in hand) — updating one secret must not re-send
    // every secret as one remote call each. Runs outside a metadata transaction so
    // a slow Cloudflare call cannot hold a metadata-pool connection.
    private void syncSecretsToActiveDeployment(EdgeFunction fn, Map<String, String> changedSecrets) {
        EdgeFunctionVersion active = fn.getActiveVersion();
        if (active == null || !"deployed".equals(active.getStatus())) {
            return;
        }
        try {
            executor.syncSecrets(fn.getProjectRef(), fn.getSlug(), active.getProviderDeploymentId(), changedSecrets);
        } catch (EdgeFunctionException e) {
            throw e;
        } catch (Exception e) {
            // Storage IS durably updated at this point — say so, instead of implying
            // the whole operation was a no-op.
            throw new EdgeFunctionException(HttpStatus.BAD_GATEWAY, "SECRET_SYNC_FAILED",
                    "Secrets were saved but not yet applied to the deployed function; retry or redeploy to apply: " + e.getMessage());
        }
    }

    @Transactional(transactionManager = "metadataTransactionManager", readOnly = true)
    public List<EdgeFunctionSecret> listSecrets(String slug) {
        return secretRepository.findByFunctionOrderByNameAsc(findFunction(slug));
    }

    @Transactional(transactionManager = "metadataTransactionManager", readOnly = true)
    public List<EdgeFunctionVersion> listVersions(String slug) {
        return versionRepository.findByFunctionOrderByVersionNoDesc(findFunction(slug));
    }

    @Transactional(transactionManager = "metadataTransactionManager", readOnly = true)
    public List<EdgeFunctionInvocation> listInvocations(String functionSlug, int limit) {
        int safeLimit = Math.max(1, Math.min(limit, 200));
        if (StringUtils.hasText(functionSlug)) {
            return invocationRepository.findByProjectRefAndFunctionSlugOrderByCreatedAtDesc(
                    projectRef(),
                    EdgeFunctionNames.normalizeSlug(functionSlug),
                    PageRequest.of(0, safeLimit)
            );
        }
        return invocationRepository.findByProjectRefOrderByCreatedAtDesc(projectRef(), PageRequest.of(0, safeLimit));
    }

    private EdgeFunction findFunction(String slug) {
        return functionRepository.findByProjectRefAndSlug(projectRef(), normalizeSlug(slug))
                .orElseThrow(() -> new EdgeFunctionException(HttpStatus.NOT_FOUND, "FUNCTION_NOT_FOUND", "Function not found"));
    }

    private String normalizeSlug(String value) {
        try {
            return EdgeFunctionNames.normalizeSlug(value);
        } catch (IllegalArgumentException e) {
            throw new EdgeFunctionException(HttpStatus.BAD_REQUEST, "INVALID_FUNCTION_SLUG", e.getMessage());
        }
    }

    private void rejectPrivileged(Boolean privileged) {
        if (Boolean.TRUE.equals(privileged)) {
            throw new EdgeFunctionException(HttpStatus.BAD_REQUEST, "PRIVILEGED_FUNCTIONS_UNSUPPORTED",
                    "Privileged edge functions are not supported yet");
        }
    }

    private String projectRef() {
        String projectRef = MultiTenancyContext.getAppCode();
        if (!StringUtils.hasText(projectRef)) {
            throw new EdgeFunctionException(HttpStatus.UNAUTHORIZED, "TENANT_CONTEXT_REQUIRED", "Project context is required");
        }
        return projectRef;
    }
}
