package ai.nubase.common.enums;

import lombok.Getter;

import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.function.Function;
import java.util.stream.Collectors;
import java.util.stream.Stream;

/**
 * Membership tiers with built-in upload size limits.
 */
@Getter
public enum MembershipTierEnum {
    FREE("free", 52_428_800L),
    PRO("pro", 209_715_200L),
    ENTERPRISE("enterprise", 1_073_741_824L);

    private static final Map<String, MembershipTierEnum> LOOKUP = Stream.of(values())
            .collect(Collectors.toUnmodifiableMap(MembershipTierEnum::getValue, Function.identity()));

    private final String value;
    private final long maxUploadBytes;

    MembershipTierEnum(String value, long maxUploadBytes) {
        this.value = value;
        this.maxUploadBytes = maxUploadBytes;
    }

    public static Optional<MembershipTierEnum> fromValue(String value) {
        if (value == null || value.isBlank()) {
            return Optional.empty();
        }
        return Optional.ofNullable(LOOKUP.get(value.trim().toLowerCase(Locale.ROOT)));
    }
}
