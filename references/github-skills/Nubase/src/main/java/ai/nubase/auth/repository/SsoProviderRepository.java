package ai.nubase.auth.repository;

import ai.nubase.auth.entity.SsoProvider;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface SsoProviderRepository extends JpaRepository<SsoProvider, UUID> {

    Optional<SsoProvider> findByResourceId(String resourceId);
}
