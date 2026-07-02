package ai.nubase.auth.repository;

import ai.nubase.auth.entity.SamlProvider;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface SamlProviderRepository extends JpaRepository<SamlProvider, UUID> {

    Optional<SamlProvider> findBySsoProviderId(UUID ssoProviderId);

    Optional<SamlProvider> findByEntityId(String entityId);
}
