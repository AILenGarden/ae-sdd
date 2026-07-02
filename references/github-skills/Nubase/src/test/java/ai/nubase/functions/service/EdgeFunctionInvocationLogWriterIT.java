package ai.nubase.functions.service;

import ai.nubase.metadata.edge.entity.EdgeFunctionInvocation;
import ai.nubase.metadata.edge.repository.EdgeFunctionInvocationRepository;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.data.domain.PageRequest;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Regression test for the invocation-log rollback bug: a failed invocation rethrows
 * EdgeFunctionException to the gateway, which previously rolled back the log row
 * written in the finally block. The writer now uses REQUIRES_NEW, so the row must
 * survive a caller transaction that rolls back.
 */
@SpringBootTest
@ActiveProfiles("dev")
@DisplayName("EdgeFunctionInvocationLogWriter (dev metadata DB)")
class EdgeFunctionInvocationLogWriterIT {

    private static final String PROJECT_REF = "edge-fn-log-writer-it";

    @Autowired
    private EdgeFunctionInvocationLogWriter logWriter;
    @Autowired
    private EdgeFunctionInvocationRepository repository;
    @Autowired
    @Qualifier("metadataTransactionManager")
    private PlatformTransactionManager metadataTransactionManager;

    @AfterEach
    void cleanup() {
        repository.deleteAll(repository.findByProjectRefOrderByCreatedAtDesc(PROJECT_REF, PageRequest.of(0, 200)));
    }

    @Test
    void logRowSurvivesCallerRollback() {
        String requestId = "it-" + UUID.randomUUID();
        TransactionTemplate callerTx = new TransactionTemplate(metadataTransactionManager);

        assertThatThrownBy(() -> callerTx.executeWithoutResult(status -> {
            logWriter.write(invocation(requestId));
            throw new RuntimeException("simulated invocation failure");
        })).hasMessage("simulated invocation failure");

        boolean logged = repository.findByProjectRefOrderByCreatedAtDesc(PROJECT_REF, PageRequest.of(0, 200))
                .stream().anyMatch(i -> requestId.equals(i.getRequestId()));
        assertThat(logged).as("failed-invocation log row must survive the caller rollback").isTrue();
    }

    private EdgeFunctionInvocation invocation(String requestId) {
        return EdgeFunctionInvocation.builder()
                .requestId(requestId)
                .projectRef(PROJECT_REF)
                .functionSlug("it-fn")
                .method("GET")
                .path("")
                .statusCode(404)
                .durationMs(1)
                .executorProvider("local")
                .errorCode("FUNCTION_NOT_FOUND")
                .callerRole("anon")
                .build();
    }
}
