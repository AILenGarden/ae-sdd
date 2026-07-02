package ai.nubase.auth.repository;

import ai.nubase.auth.entity.FlowState;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface FlowStateRepository extends JpaRepository<FlowState, UUID> {

    Optional<FlowState> findByAuthCode(String authCode);
}
