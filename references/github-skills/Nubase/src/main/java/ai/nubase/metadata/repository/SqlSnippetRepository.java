package ai.nubase.metadata.repository;

import ai.nubase.metadata.entity.SqlSnippet;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.UUID;

@Repository
public interface SqlSnippetRepository extends JpaRepository<SqlSnippet, Long> {

    List<SqlSnippet> findByPlatformUserIdAndDbKeyOrderByUpdatedAtDesc(UUID platformUserId, String dbKey);
}
