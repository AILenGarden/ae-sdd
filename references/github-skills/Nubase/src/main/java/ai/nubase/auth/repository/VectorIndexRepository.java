package ai.nubase.auth.repository;

import ai.nubase.auth.entity.VectorIndex;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;

@Repository
public interface VectorIndexRepository extends JpaRepository<VectorIndex, String> {

    Optional<VectorIndex> findByBucketIdAndName(String bucketId, String name);

    /**
     * Paginated index query aligned with Supabase cursor pagination:
     * WHERE bucket_id = ? AND name LIKE prefix% AND name > nextToken ORDER BY name ASC LIMIT maxResults+1
     */
    @Query("SELECT i FROM VectorIndex i " +
            "WHERE i.bucketId = :bucketId " +
            "AND (:prefix IS NULL OR i.name LIKE CONCAT(:prefix, '%')) " +
            "AND (:nextToken IS NULL OR i.name > :nextToken) " +
            "ORDER BY i.name ASC " +
            "LIMIT :limit")
    List<VectorIndex> findIndexesWithCursor(
            @Param("bucketId") String bucketId,
            @Param("prefix") String prefix,
            @Param("nextToken") String nextToken,
            @Param("limit") int limit
    );

    long countByBucketId(String bucketId);

    void deleteByBucketIdAndName(String bucketId, String name);

    boolean existsByBucketId(String bucketId);
}
