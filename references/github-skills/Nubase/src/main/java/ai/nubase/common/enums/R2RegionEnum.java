package ai.nubase.common.enums;

import lombok.Getter;

import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.function.Function;
import java.util.stream.Collectors;
import java.util.stream.Stream;

/**
 * Supported R2 region values.
 */
@Getter
public enum R2RegionEnum {
    AUTO("auto");

    private static final Map<String, R2RegionEnum> LOOKUP = Stream.of(values())
            .collect(Collectors.toUnmodifiableMap(R2RegionEnum::getValue, Function.identity()));

    private final String value;

    R2RegionEnum(String value) {
        this.value = value;
    }

    public static Optional<R2RegionEnum> fromValue(String value) {
        if (value == null || value.isBlank()) {
            return Optional.empty();
        }
        return Optional.ofNullable(LOOKUP.get(value.trim().toLowerCase(Locale.ROOT)));
    }
}
