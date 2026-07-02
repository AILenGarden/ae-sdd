package ai.nubase.metadata.repository;

import ai.nubase.metadata.entity.PlatformSetting;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;

@Repository
public interface PlatformSettingRepository
        extends JpaRepository<PlatformSetting, PlatformSetting.PlatformSettingId> {

    List<PlatformSetting> findByCategory(String category);

    void deleteByCategoryAndKey(String category, String key);
}
