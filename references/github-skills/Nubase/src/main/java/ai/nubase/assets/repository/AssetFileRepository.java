package ai.nubase.assets.repository;

import ai.nubase.assets.entity.AssetFile;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface AssetFileRepository extends JpaRepository<AssetFile, UUID> {

    Optional<AssetFile> findByPath(String path);

    @Query("SELECT f FROM AssetFile f WHERE f.path LIKE CONCAT(:prefix, '%') ORDER BY f.path")
    List<AssetFile> findByPathPrefix(@Param("prefix") String prefix);

    List<AssetFile> findAllByOrderByPathAsc();
}
