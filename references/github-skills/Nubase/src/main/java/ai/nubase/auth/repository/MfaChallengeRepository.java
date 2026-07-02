package ai.nubase.auth.repository;

import ai.nubase.auth.entity.MfaChallenge;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface MfaChallengeRepository extends JpaRepository<MfaChallenge, UUID> {

    Optional<MfaChallenge> findByIdAndFactorId(UUID id, UUID factorId);
}
