package ai.nubase.functions.service;

import ai.nubase.metadata.edge.entity.EdgeFunctionInvocation;
import ai.nubase.metadata.edge.repository.EdgeFunctionInvocationRepository;
import lombok.RequiredArgsConstructor;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.stereotype.Component;
import org.springframework.transaction.annotation.Propagation;
import org.springframework.transaction.annotation.Transactional;

/**
 * Persists invocation logs in their own transaction so the row survives the
 * EdgeFunctionException rethrown to the caller (failed invocations are exactly
 * the ones operators need to see).
 */
@Component
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.functions.enabled", havingValue = "true", matchIfMissing = true)
public class EdgeFunctionInvocationLogWriter {

    private final EdgeFunctionInvocationRepository invocationRepository;

    @Transactional(transactionManager = "metadataTransactionManager", propagation = Propagation.REQUIRES_NEW)
    public void write(EdgeFunctionInvocation invocation) {
        invocationRepository.save(invocation);
    }
}
