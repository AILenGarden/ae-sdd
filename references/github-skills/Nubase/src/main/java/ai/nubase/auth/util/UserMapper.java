package ai.nubase.auth.util;

import ai.nubase.auth.dto.response.IdentityResponse;
import ai.nubase.auth.dto.response.UserResponse;
import ai.nubase.auth.entity.Identity;
import ai.nubase.auth.entity.User;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.stream.Collectors;

@Component
public class UserMapper {

    public UserResponse toUserResponse(User user, List<Identity> identities) {
        return UserResponse.builder()
                .id(user.getId().toString())
                .aud(user.getAud())
                .role(user.getRole())
                .email(user.getEmail())
                .emailConfirmedAt(user.getEmailConfirmedAt())
                .phone(user.getPhone())
                .phoneConfirmedAt(user.getPhoneConfirmedAt())
                .confirmedAt(getConfirmedAt(user))
                .lastSignInAt(user.getLastSignInAt())
                .bannedUntil(user.getBannedUntil())
                .appMetadata(user.getRawAppMetaData())
                .userMetadata(user.getRawUserMetaData())
                .identities(toIdentityResponses(identities))
                .createdAt(user.getCreatedAt())
                .updatedAt(user.getUpdatedAt())
                .build();
    }

    public IdentityResponse toIdentityResponse(Identity identity) {
        return IdentityResponse.builder()
                .id(identity.getId().toString())
                .userId(identity.getUser().getId().toString())
                .identityData(identity.getIdentityData())
                .provider(identity.getProvider())
                .lastSignInAt(identity.getLastSignInAt())
                .createdAt(identity.getCreatedAt())
                .updatedAt(identity.getUpdatedAt())
                .build();
    }

    private List<IdentityResponse> toIdentityResponses(List<Identity> identities) {
        if (identities == null) {
            return List.of();
        }
        return identities.stream()
                .map(this::toIdentityResponse)
                .collect(Collectors.toList());
    }

    private java.time.Instant getConfirmedAt(User user) {
        if (user.getEmailConfirmedAt() != null) {
            return user.getEmailConfirmedAt();
        }
        if (user.getPhoneConfirmedAt() != null) {
            return user.getPhoneConfirmedAt();
        }
        return null;
    }
}
