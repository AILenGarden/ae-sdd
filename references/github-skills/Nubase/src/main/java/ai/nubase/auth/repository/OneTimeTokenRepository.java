package ai.nubase.auth.repository;

import ai.nubase.auth.entity.OneTimeToken;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface OneTimeTokenRepository extends JpaRepository<OneTimeToken, UUID> {

    Optional<OneTimeToken> findByTokenHashAndTokenType(String tokenHash, String tokenType);

    Optional<OneTimeToken> findByTokenHash(String tokenHash);

    @Modifying
    @Query("DELETE FROM OneTimeToken t WHERE t.user.id = :userId AND t.tokenType = :tokenType")
    void deleteByUserIdAndTokenType(@Param("userId") UUID userId, @Param("tokenType") String tokenType);
}
