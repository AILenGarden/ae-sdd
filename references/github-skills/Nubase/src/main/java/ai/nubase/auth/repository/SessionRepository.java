package ai.nubase.auth.repository;

import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.UUID;

@Repository
public interface SessionRepository extends JpaRepository<Session, UUID> {

    List<Session> findByUser(User user);

    List<Session> findByUserId(UUID userId);

    void deleteByUserId(UUID userId);
}
