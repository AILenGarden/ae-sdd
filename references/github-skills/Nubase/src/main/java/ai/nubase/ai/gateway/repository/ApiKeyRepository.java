package ai.nubase.ai.gateway.repository;

import ai.nubase.ai.gateway.entity.ApiKey;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

/**
 * 网关密钥仓库（作用于当前租户库的 ai_gateway.api_keys）。
 */
@Repository
public interface ApiKeyRepository extends JpaRepository<ApiKey, Long> {

    /** 入站校验主路径：按完整密钥的 SHA-256 哈希查找。 */
    Optional<ApiKey> findByKeyHash(String keyHash);

    boolean existsByKeyHash(String keyHash);

    /** 兼容旧明文密钥查找。 */
    Optional<ApiKey> findByApiKey(String apiKey);

    boolean existsByApiKey(String apiKey);

    /** 按所有者（auth.users.id）查找。 */
    List<ApiKey> findByUserId(UUID userId);
}
