package ai.nubase.auth.repository;

import ai.nubase.auth.entity.MfaFactor;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface MfaFactorRepository extends JpaRepository<MfaFactor, UUID> {

    List<MfaFactor> findByUserId(UUID userId);

    List<MfaFactor> findByUserIdAndStatus(UUID userId, String status);

    Optional<MfaFactor> findByIdAndUserId(UUID id, UUID userId);

    long countByUserIdAndStatus(UUID userId, String status);
}
