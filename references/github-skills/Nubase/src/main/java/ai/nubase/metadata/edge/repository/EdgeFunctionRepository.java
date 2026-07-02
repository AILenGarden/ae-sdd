package ai.nubase.metadata.edge.repository;

import ai.nubase.metadata.edge.entity.EdgeFunction;
import org.springframework.data.jpa.repository.EntityGraph;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

public interface EdgeFunctionRepository extends JpaRepository<EdgeFunction, UUID> {

    // activeVersion is fetched eagerly here because callers map it to DTOs after the
    // transaction has closed (open-in-view is off for the metadata persistence unit).
    @EntityGraph(attributePaths = "activeVersion")
    Optional<EdgeFunction> findByProjectRefAndSlug(String projectRef, String slug);

    @EntityGraph(attributePaths = "activeVersion")
    List<EdgeFunction> findByProjectRefOrderByCreatedAtDesc(String projectRef);

    boolean existsByProjectRefAndSlug(String projectRef, String slug);
}
