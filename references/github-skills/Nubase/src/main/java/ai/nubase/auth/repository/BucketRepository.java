package ai.nubase.auth.repository;

import ai.nubase.auth.entity.Bucket;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;

/**
 * Repository for Bucket entity
 */
@Repository
public interface BucketRepository extends JpaRepository<Bucket, String> {

    /**
     * Find bucket by name
     */
    Optional<Bucket> findByName(String name);

    /**
     * Check if bucket exists by name
     */
    boolean existsByName(String name);

    /**
     * Check if bucket exists by id
     */
    boolean existsById(String id);
}
