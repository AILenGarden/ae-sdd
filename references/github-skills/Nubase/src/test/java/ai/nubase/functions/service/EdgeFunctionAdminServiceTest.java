package ai.nubase.functions.service;

import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.functions.dto.EdgeFunctionDtos.DeployFunctionRequest;
import ai.nubase.functions.dto.EdgeFunctionDtos.SetFunctionSecretsRequest;
import ai.nubase.functions.executor.EdgeFunctionDeploymentRequest;
import ai.nubase.functions.executor.EdgeFunctionDeploymentResponse;
import ai.nubase.functions.executor.EdgeFunctionExecutorRouter;
import ai.nubase.metadata.edge.entity.EdgeFunction;
import ai.nubase.metadata.edge.entity.EdgeFunctionVersion;
import ai.nubase.metadata.edge.repository.EdgeFunctionInvocationRepository;
import ai.nubase.metadata.edge.repository.EdgeFunctionRepository;
import ai.nubase.metadata.edge.repository.EdgeFunctionSecretRepository;
import ai.nubase.metadata.edge.repository.EdgeFunctionVersionRepository;
import ai.nubase.postgrest.multidb.EncryptionService;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.ArgumentCaptor;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

import static ai.nubase.functions.service.EdgeFunctionExceptions.EdgeFunctionException;
import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.doThrow;
import static org.mockito.Mockito.lenient;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

@ExtendWith(MockitoExtension.class)
class EdgeFunctionAdminServiceTest {

    @Mock
    private EdgeFunctionRepository functionRepository;
    @Mock
    private EdgeFunctionVersionRepository versionRepository;
    @Mock
    private EdgeFunctionSecretRepository secretRepository;
    @Mock
    private EdgeFunctionInvocationRepository invocationRepository;
    @Mock
    private EdgeFunctionExecutorRouter executor;
    @Mock
    private EdgeFunctionDeploymentRecorder deploymentRecorder;
    @Mock
    private EncryptionService encryptionService;
    @Mock
    private EdgeFunctionSecretEnv secretEnv;
    @Mock
    private EdgeFunctionSecretWriter secretWriter;

    private EdgeFunctionAdminService service;

    @BeforeEach
    void setUp() {
        service = new EdgeFunctionAdminService(
                functionRepository, versionRepository, secretRepository, invocationRepository,
                executor, deploymentRecorder, encryptionService, secretEnv, secretWriter,
                new ai.nubase.functions.executor.EdgeFunctionExecutorProperties());
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder().appCode("app1").build());
        lenient().when(secretRepository.findByFunctionOrderByNameAsc(any())).thenReturn(List.of());
    }

    @AfterEach
    void tearDown() {
        MultiTenancyContext.clear();
    }

    @Test
    void setSecretsSyncsOnlyChangedKeysToActiveDeployment() throws Exception {
        EdgeFunction fn = function(deployedVersion());
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));
        when(secretRepository.findByFunctionAndName(eq(fn), anyString())).thenReturn(Optional.empty());
        when(encryptionService.encrypt(anyString())).thenReturn("encrypted");

        service.setSecrets("hello", new SetFunctionSecretsRequest(Map.of("API_KEY", "v1")));

        // Only the changed entries are pushed (plaintext already in hand) — never the
        // full re-decrypted set.
        verify(executor).syncSecrets("app1", "hello", "deployment-1", Map.of("API_KEY", "v1"));
        verify(secretEnv, never()).decryptedEnv(any());
        verify(secretWriter).saveAll(any());
        verify(secretEnv).evict(fn.getId());
    }

    @Test
    void setSecretsValidatesWholeBatchBeforeAnyWrite() {
        EdgeFunction fn = function(null);
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));

        java.util.Map<String, String> batch = new java.util.LinkedHashMap<>();
        batch.put("API_KEY", "v1");
        batch.put("bad name!", "x");
        assertThatThrownBy(() -> service.setSecrets("hello", new SetFunctionSecretsRequest(batch)))
                .isInstanceOf(EdgeFunctionException.class)
                .satisfies(e -> assertThat(((EdgeFunctionException) e).code()).isEqualTo("INVALID_SECRET_NAME"));

        // Phase-1 failure must leave ZERO side effects — no half-applied secret set.
        verify(secretWriter, never()).saveAll(any());
        verify(secretEnv, never()).evict(any());
    }

    @Test
    void setSecretsSkipsSyncWhenFunctionNotDeployed() throws Exception {
        EdgeFunction fn = function(null);
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));
        when(secretRepository.findByFunctionAndName(eq(fn), anyString())).thenReturn(Optional.empty());
        when(encryptionService.encrypt(anyString())).thenReturn("encrypted");

        service.setSecrets("hello", new SetFunctionSecretsRequest(Map.of("API_KEY", "v1")));

        verify(executor, never()).syncSecrets(anyString(), anyString(), anyString(), any());
    }

    @Test
    void setSecretsSurfacesSyncFailureAsSavedButNotApplied() throws Exception {
        EdgeFunction fn = function(deployedVersion());
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));
        when(secretRepository.findByFunctionAndName(eq(fn), anyString())).thenReturn(Optional.empty());
        when(encryptionService.encrypt(anyString())).thenReturn("encrypted");
        doThrow(new IllegalStateException("cloudflare down"))
                .when(executor).syncSecrets(anyString(), anyString(), anyString(), any());

        assertThatThrownBy(() -> service.setSecrets("hello", new SetFunctionSecretsRequest(Map.of("API_KEY", "v1"))))
                .isInstanceOf(EdgeFunctionException.class)
                .satisfies(e -> {
                    assertThat(((EdgeFunctionException) e).code()).isEqualTo("SECRET_SYNC_FAILED");
                    // Storage IS durably updated at this point — the message must say so.
                    assertThat(e.getMessage()).contains("saved");
                });
    }

    @Test
    void deployPassesEntrypointAndDecryptedSecretsToExecutor() {
        EdgeFunction fn = function(null);
        fn.setEntrypoint("main.js");
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));
        when(secretEnv.decryptedEnv(fn)).thenReturn(Map.of("API_KEY", "v1"));
        when(executor.deploy(any())).thenReturn(EdgeFunctionDeploymentResponse.failed("local", "boom"));
        when(deploymentRecorder.record(eq(fn.getId()), any(), any())).thenReturn(deployedVersion());

        service.deploy("hello", new DeployFunctionRequest("hash", null, null, "bundle"));

        ArgumentCaptor<EdgeFunctionDeploymentRequest> captor = ArgumentCaptor.forClass(EdgeFunctionDeploymentRequest.class);
        verify(executor).deploy(captor.capture());
        assertThat(captor.getValue().entrypoint()).isEqualTo("main.js");
        assertThat(captor.getValue().env()).containsEntry("API_KEY", "v1");
        verify(deploymentRecorder).record(eq(fn.getId()), any(), any());
    }

    @Test
    void deployDeletesSupersededProviderDeployment() {
        EdgeFunction fn = function(version("deployed", "old-name-worker"));
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));
        when(secretEnv.decryptedEnv(fn)).thenReturn(Map.of());
        when(executor.deploy(any())).thenReturn(EdgeFunctionDeploymentResponse.deployed("cloudflare", "new-hashed-worker"));
        when(deploymentRecorder.record(eq(fn.getId()), any(), any())).thenReturn(version("deployed", "new-hashed-worker"));

        service.deploy("hello", new DeployFunctionRequest("hash", null, null, "bundle"));

        // The old worker (old code + old secret bindings) must not leak on the provider.
        verify(executor).delete("app1", "hello", "old-name-worker");
    }

    @Test
    void deployDoesNotDeleteWhenProviderIdUnchanged() {
        EdgeFunction fn = function(version("deployed", "same-worker"));
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));
        when(secretEnv.decryptedEnv(fn)).thenReturn(Map.of());
        when(executor.deploy(any())).thenReturn(EdgeFunctionDeploymentResponse.deployed("cloudflare", "same-worker"));
        when(deploymentRecorder.record(eq(fn.getId()), any(), any())).thenReturn(version("deployed", "same-worker"));

        service.deploy("hello", new DeployFunctionRequest("hash", null, null, "bundle"));

        verify(executor, never()).delete(anyString(), anyString(), anyString());
    }

    @Test
    void deployRecomputesSourceHashServerSide() throws Exception {
        EdgeFunction fn = function(null);
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));
        when(secretEnv.decryptedEnv(fn)).thenReturn(Map.of());
        when(executor.deploy(any())).thenReturn(EdgeFunctionDeploymentResponse.failed("local", "boom"));
        when(deploymentRecorder.record(eq(fn.getId()), any(), any())).thenReturn(version("failed", null));

        String bundleBase64 = java.util.Base64.getEncoder()
                .encodeToString("{\"files\":[]}".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        service.deploy("hello", new DeployFunctionRequest("client-claimed-lie", null, null, bundleBase64));

        ArgumentCaptor<DeployFunctionRequest> captor = ArgumentCaptor.forClass(DeployFunctionRequest.class);
        verify(deploymentRecorder).record(eq(fn.getId()), captor.capture(), any());
        String expected = java.util.HexFormat.of().formatHex(
                java.security.MessageDigest.getInstance("SHA-256").digest("{\"files\":[]}".getBytes(java.nio.charset.StandardCharsets.UTF_8)));
        // Version history must describe the bundle actually deployed, not the client's claim.
        assertThat(captor.getValue().sourceHash()).isEqualTo(expected);
    }

    @Test
    void deployRejectsOversizedAndInvalidBundles() {
        EdgeFunction fn = function(null);
        when(functionRepository.findByProjectRefAndSlug("app1", "hello")).thenReturn(Optional.of(fn));

        String huge = "A".repeat(15 * 1024 * 1024); // > default 10MB after base64 discount
        assertThatThrownBy(() -> service.deploy("hello", new DeployFunctionRequest("h", null, null, huge)))
                .isInstanceOf(EdgeFunctionException.class)
                .satisfies(e -> assertThat(((EdgeFunctionException) e).code()).isEqualTo("BUNDLE_TOO_LARGE"));

        assertThatThrownBy(() -> service.deploy("hello", new DeployFunctionRequest("h", null, null, "!!!not-base64!!!")))
                .isInstanceOf(EdgeFunctionException.class)
                .satisfies(e -> assertThat(((EdgeFunctionException) e).code()).isEqualTo("INVALID_BUNDLE"));
        verify(executor, never()).deploy(any());
    }

    private EdgeFunctionVersion version(String status, String providerDeploymentId) {
        return EdgeFunctionVersion.builder()
                .status(status)
                .providerDeploymentId(providerDeploymentId)
                .build();
    }

    private EdgeFunction function(EdgeFunctionVersion activeVersion) {
        return EdgeFunction.builder()
                .id(UUID.randomUUID())
                .projectRef("app1")
                .slug("hello")
                .name("hello")
                .enabled(true)
                .activeVersion(activeVersion)
                .build();
    }

    private EdgeFunctionVersion deployedVersion() {
        return EdgeFunctionVersion.builder()
                .status("deployed")
                .providerDeploymentId("deployment-1")
                .build();
    }
}
