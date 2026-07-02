package ai.nubase.metadata.repository;

import ai.nubase.metadata.entity.PlatformUserProject;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface PlatformUserProjectRepository extends JpaRepository<PlatformUserProject, Long> {

    List<PlatformUserProject> findByUserId(UUID userId);

    List<PlatformUserProject> findByDbKey(String dbKey);

    boolean existsByUserIdAndDbKey(UUID userId, String dbKey);

    Optional<PlatformUserProject> findByUserIdAndDbKey(UUID userId, String dbKey);

    void deleteByUserIdAndDbKey(UUID userId, String dbKey);
}
