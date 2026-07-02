package ai.nubase.metadata.edge.repository;

import ai.nubase.metadata.edge.entity.EdgeFunctionInvocation;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

import java.time.Instant;
import java.util.List;
import java.util.UUID;

public interface EdgeFunctionInvocationRepository extends JpaRepository<EdgeFunctionInvocation, UUID> {

    List<EdgeFunctionInvocation> findByProjectRefOrderByCreatedAtDesc(String projectRef, Pageable pageable);

    List<EdgeFunctionInvocation> findByProjectRefAndFunctionSlugOrderByCreatedAtDesc(
            String projectRef,
            String functionSlug,
            Pageable pageable
    );

    // Bulk JPQL delete: the derived form loads every matching entity into the
    // persistence context and deletes row by row — an OOM/long-lock hazard once the
    // log table grows to millions of rows.
    @Modifying
    @Query("delete from EdgeFunctionInvocation i where i.createdAt < :cutoff")
    int deleteByCreatedAtBefore(@Param("cutoff") Instant cutoff);
}
