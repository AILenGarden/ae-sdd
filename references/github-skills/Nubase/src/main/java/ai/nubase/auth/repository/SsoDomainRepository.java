package ai.nubase.auth.repository;

import ai.nubase.auth.entity.SsoDomain;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface SsoDomainRepository extends JpaRepository<SsoDomain, UUID> {

    @Query("SELECT d FROM SsoDomain d WHERE LOWER(d.domain) = LOWER(:domain)")
    Optional<SsoDomain> findByDomainIgnoreCase(@Param("domain") String domain);

    List<SsoDomain> findBySsoProviderId(UUID ssoProviderId);
}
