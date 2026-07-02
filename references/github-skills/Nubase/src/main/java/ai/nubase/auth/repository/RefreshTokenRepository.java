package ai.nubase.auth.repository;

import ai.nubase.auth.entity.RefreshToken;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface RefreshTokenRepository extends JpaRepository<RefreshToken, Long> {

    Optional<RefreshToken> findByToken(String token);

    Optional<RefreshToken> findByTokenAndRevokedFalse(String token);

    List<RefreshToken> findByUser(User user);

    List<RefreshToken> findByUserId(UUID userId);

    List<RefreshToken> findBySession(Session session);

    List<RefreshToken> findBySessionId(UUID sessionId);

    void deleteByUserId(UUID userId);

    void deleteBySessionId(UUID sessionId);
}
