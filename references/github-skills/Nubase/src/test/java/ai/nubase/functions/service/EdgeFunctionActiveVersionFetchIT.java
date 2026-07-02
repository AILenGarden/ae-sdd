package ai.nubase.functions.service;

import ai.nubase.functions.dto.EdgeFunctionDtos.EdgeFunctionResponse;
import ai.nubase.metadata.edge.entity.EdgeFunction;
import ai.nubase.metadata.edge.entity.EdgeFunctionVersion;
import ai.nubase.metadata.edge.repository.EdgeFunctionRepository;
import ai.nubase.metadata.edge.repository.EdgeFunctionVersionRepository;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Regression test for the LazyInitializationException on activeVersion: controllers
 * map entities to DTOs after the service transaction has closed (open-in-view is off),
 * so the repository must fetch activeVersion eagerly (@EntityGraph).
 */
@SpringBootTest
@ActiveProfiles("dev")
@DisplayName("EdgeFunction activeVersion fetch (dev metadata DB)")
class EdgeFunctionActiveVersionFetchIT {

    private static final String PROJECT_REF = "edge-fn-fetch-it";

    @Autowired
    private EdgeFunctionRepository functionRepository;
    @Autowired
    private EdgeFunctionVersionRepository versionRepository;
    @Autowired
    @Qualifier("metadataTransactionManager")
    private PlatformTransactionManager metadataTransactionManager;

    @AfterEach
    void cleanup() {
        new TransactionTemplate(metadataTransactionManager).executeWithoutResult(status -> {
            functionRepository.findByProjectRefOrderByCreatedAtDesc(PROJECT_REF).forEach(fn -> {
                fn.setActiveVersion(null);
                functionRepository.save(fn);
                versionRepository.findByFunctionOrderByVersionNoDesc(fn).forEach(versionRepository::delete);
                functionRepository.delete(fn);
            });
        });
    }

    @Test
    void activeVersionIsReadableAfterTransactionCloses() {
        TransactionTemplate tx = new TransactionTemplate(metadataTransactionManager);
        tx.executeWithoutResult(status -> {
            EdgeFunction fn = functionRepository.save(EdgeFunction.builder()
                    .projectRef(PROJECT_REF)
                    .name("hello")
                    .slug("hello")
                    .build());
            EdgeFunctionVersion version = versionRepository.save(EdgeFunctionVersion.builder()
                    .function(fn)
                    .versionNo(1)
                    .sourceHash("hash")
                    .status("deployed")
                    .providerDeploymentId("dep-1")
                    .build());
            fn.setActiveVersion(version);
            functionRepository.save(fn);
        });

        // Outside any transaction — exactly how the admin controller consumes entities.
        EdgeFunction loaded = functionRepository.findByProjectRefAndSlug(PROJECT_REF, "hello").orElseThrow();
        EdgeFunctionResponse response = EdgeFunctionResponse.from(loaded);

        assertThat(response.activeVersion()).isNotNull();
        assertThat(response.activeVersion().versionNo()).isEqualTo(1);
        assertThat(response.activeVersion().status()).isEqualTo("deployed");

        var listed = functionRepository.findByProjectRefOrderByCreatedAtDesc(PROJECT_REF);
        assertThat(EdgeFunctionResponse.from(listed.get(0)).activeVersion()).isNotNull();
    }
}
