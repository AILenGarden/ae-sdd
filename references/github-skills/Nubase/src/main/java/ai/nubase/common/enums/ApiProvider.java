package ai.nubase.common.enums;

import java.util.Locale;

/**
 * Upstream AI provider type for the gateway.
 * <p>
 * Stored as a string (via {@code @Enumerated(EnumType.STRING)}) on {@code upstream_configs.provider}
 * and used to route an inbound request to the correct upstream-forwarding service.
 */
public enum ApiProvider {
    /** Anthropic Claude (native /v1/messages protocol). */
    CLAUDE,

    /** OpenAI / OpenAI-compatible chat completions. */
    OPENAI;

    /**
     * Parse a provider from a string (case-insensitive). Returns {@code CLAUDE} as the default
     * when the value is null or unrecognized.
     */
    public static ApiProvider fromString(String value) {
        if (value == null) {
            return CLAUDE;
        }
        for (ApiProvider p : values()) {
            if (p.name().equalsIgnoreCase(value)) {
                return p;
            }
        }
        return CLAUDE;
    }

    /** Lowercase channel code form, e.g. {@code CLAUDE -> "claude"}. */
    public String channelCode() {
        return name().toLowerCase(Locale.ROOT);
    }
}
