package ai.nubase.auth.repository;

import ai.nubase.auth.entity.VectorBucket;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;

@Repository
public interface VectorBucketRepository extends JpaRepository<VectorBucket, String> {

    /**
     * Paginated bucket query aligned with Supabase cursor pagination:
     * WHERE id LIKE prefix% AND id > nextToken ORDER BY id ASC LIMIT maxResults+1
     * Fetches one extra row to determine hasMore.
     */
    @Query("SELECT b FROM VectorBucket b " +
            "WHERE (:prefix IS NULL OR b.id LIKE CONCAT(:prefix, '%')) " +
            "AND (:nextToken IS NULL OR b.id > :nextToken) " +
            "ORDER BY b.id ASC " +
            "LIMIT :limit")
    List<VectorBucket> findBucketsWithCursor(
            @Param("prefix") String prefix,
            @Param("nextToken") String nextToken,
            @Param("limit") int limit
    );
}
