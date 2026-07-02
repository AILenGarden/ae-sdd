package ai.nubase.common.enums;

import lombok.Getter;
import org.springframework.core.convert.converter.Converter;
import org.springframework.stereotype.Component;

/**
 * True or False Enum
 * <p>
 * Used to handle boolean values in HTTP headers and parameters.
 * <p>
 *
 * @author nubase
 * @since 2026-03-03
 */
@Getter
public enum TrueOrFalseEnum {

    TRUE("true", true),
    FALSE("false", false);

    private final String value;
    private final boolean boolValue;

    TrueOrFalseEnum(String value, boolean boolValue) {
        this.value = value;
        this.boolValue = boolValue;
    }

    /**
     * Converts a string to the corresponding enum value.
     *
     * @param value the string value (case-insensitive)
     * @return the matching enum value, or FALSE if no match
     */
    public static TrueOrFalseEnum fromString(String value) {
        if (value == null) {
            return FALSE;
        }
        return switch (value.toLowerCase()) {
            case "true" -> TRUE;
            case "false" -> FALSE;
            default -> FALSE;
        };
    }

    /**
     * Checks whether the value is true.
     *
     * @return true if the enum value is TRUE
     */
    public boolean isTrue() {
        return this == TRUE;
    }

    /**
     * Checks whether the value is false.
     *
     * @return true if the enum value is FALSE
     */
    public boolean isFalse() {
        return this == FALSE;
    }

    @Override
    public String toString() {
        return value;
    }

    /**
     * Spring converter factory - used for automatic conversion of HTTP parameters/headers.
     */
    @Component
    public static class ConverterFactory implements org.springframework.core.convert.converter.ConverterFactory<String, TrueOrFalseEnum> {

        @Override
        public <T extends TrueOrFalseEnum> Converter<String, T> getConverter(Class<T> targetType) {
            return new Converter<String, T>() {
                @Override
                @SuppressWarnings("unchecked")
                public T convert(String source) {
                    if (source == null || source.isBlank()) {
                        return (T) FALSE;
                    }
                    return (T) TrueOrFalseEnum.fromString(source);
                }
            };
        }
    }
}
