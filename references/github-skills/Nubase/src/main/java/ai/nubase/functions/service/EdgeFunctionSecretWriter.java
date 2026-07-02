package ai.nubase.functions.service;

import ai.nubase.metadata.edge.entity.EdgeFunctionSecret;
import ai.nubase.metadata.edge.repository.EdgeFunctionSecretRepository;
import lombok.RequiredArgsConstructor;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.stereotype.Component;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;

/**
 * Persists a secret batch in one short metadata transaction, so a failure never
 * leaves a partially-applied set. The caller does all validation/encryption
 * BEFORE this and all remote sync AFTER it — no remote call ever holds a
 * metadata-pool connection.
 */
@Component
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.functions.enabled", havingValue = "true", matchIfMissing = true)
public class EdgeFunctionSecretWriter {

    private final EdgeFunctionSecretRepository secretRepository;

    @Transactional("metadataTransactionManager")
    public void saveAll(List<EdgeFunctionSecret> secrets) {
        secretRepository.saveAll(secrets);
    }
}
