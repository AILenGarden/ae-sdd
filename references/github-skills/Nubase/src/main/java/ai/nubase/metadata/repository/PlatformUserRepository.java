package ai.nubase.metadata.repository;

import ai.nubase.metadata.entity.PlatformUser;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface PlatformUserRepository extends JpaRepository<PlatformUser, UUID> {

    Optional<PlatformUser> findByEmailIgnoreCase(String email);

    boolean existsByEmailIgnoreCase(String email);
}
