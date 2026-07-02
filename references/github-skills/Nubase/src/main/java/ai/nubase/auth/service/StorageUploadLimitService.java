package ai.nubase.auth.service;

import ai.nubase.auth.entity.User;
import ai.nubase.auth.util.SecurityUtil;
import ai.nubase.common.config.R2StorageProperties;
import ai.nubase.common.enums.MembershipTierEnum;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;
import org.springframework.util.StringUtils;

import java.util.Map;

@Service
@RequiredArgsConstructor
public class StorageUploadLimitService {

    private final R2StorageProperties r2StorageProperties;
    private final SecurityUtil securityUtil;

    public long getMaxUploadBytesForCurrentUser() {
        Long configuredLimit = r2StorageProperties.getMaxFileSize();
        if (configuredLimit == null || configuredLimit <= 0) {
            throw new IllegalStateException("nubase.storage.r2.max-file-size must be configured and greater than 0");
        }
        long defaultLimit = configuredLimit;

        User currentUser = securityUtil.getCurrentUser();
        if (currentUser == null) {
            return defaultLimit;
        }

        String tier = resolveTier(currentUser);
        if (!StringUtils.hasText(tier)) {
            return defaultLimit;
        }

        return MembershipTierEnum.fromValue(tier)
                .map(MembershipTierEnum::getMaxUploadBytes)
                .orElse(defaultLimit);
    }

    private String resolveTier(User user) {
        String tierFromAppMeta = getTierFromMetadata(user.getRawAppMetaData());
        if (StringUtils.hasText(tierFromAppMeta)) {
            return tierFromAppMeta;
        }
        return getTierFromMetadata(user.getRawUserMetaData());
    }

    private String getTierFromMetadata(Map<String, Object> metadata) {
        if (metadata == null || metadata.isEmpty()) {
            return null;
        }
        for (TierMetadataKey key : TierMetadataKey.values()) {
            Object value = metadata.get(key.key());
            if (value == null) {
                continue;
            }
            String tier = value.toString().trim();
            if (!tier.isEmpty()) {
                return tier;
            }
        }
        return null;
    }

    private enum TierMetadataKey {
        MEMBERSHIP_TIER_CAMEL("membershipTier"),
        MEMBERSHIP_TIER_SNAKE("membership_tier"),
        TIER("tier"),
        PLAN("plan");

        private final String key;

        TierMetadataKey(String key) {
            this.key = key;
        }

        public String key() {
            return key;
        }
    }
}
