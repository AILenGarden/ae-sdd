package ai.nubase.auth.repository;

import ai.nubase.auth.entity.Identity;
import ai.nubase.auth.entity.User;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface IdentityRepository extends JpaRepository<Identity, UUID> {

    List<Identity> findByUser(User user);

    List<Identity> findByUserId(UUID userId);

    Optional<Identity> findByProviderAndProviderId(String provider, String providerId);

    List<Identity> findByProvider(String provider);

    void deleteByUserId(UUID userId);
}
