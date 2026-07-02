package ai.nubase.assets.repository;

import ai.nubase.assets.entity.AssetSettings;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

@Repository
public interface AssetSettingsRepository extends JpaRepository<AssetSettings, Short> {
}
