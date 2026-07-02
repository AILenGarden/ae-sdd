package ai.nubase.auth.repository;

import ai.nubase.auth.entity.SamlRelayState;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface SamlRelayStateRepository extends JpaRepository<SamlRelayState, UUID> {

    Optional<SamlRelayState> findByRequestId(String requestId);
}
