package ai.nubase.auth.repository;

import ai.nubase.auth.entity.StorageObject;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

/**
 * Repository for storage object entities
 */
@Repository
public interface StorageObjectRepository extends JpaRepository<StorageObject, UUID> {

    /**
     * Find an object by bucket ID and name
     */
    Optional<StorageObject> findByBucketIdAndName(String bucketId, String name);

    /**
     * Find all objects in a bucket
     */
    List<StorageObject> findByBucketId(String bucketId);

    /**
     * Find objects in a bucket whose name starts with the given prefix
     */
    @Query("SELECT o FROM StorageObject o WHERE o.bucketId = :bucketId AND o.name LIKE CONCAT(:prefix, '%')")
    List<StorageObject> findByBucketIdAndNamePrefix(@Param("bucketId") String bucketId, @Param("prefix") String prefix);

    /**
     * Delete an object by bucket ID and name
     */
    void deleteByBucketIdAndName(String bucketId, String name);

    /**
     * Delete all objects in a bucket
     */
    void deleteByBucketId(String bucketId);

    /**
     * Check if an object exists by bucket ID and name
     */
    boolean existsByBucketIdAndName(String bucketId, String name);
}
