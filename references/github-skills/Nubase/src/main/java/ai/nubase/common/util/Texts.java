package ai.nubase.common.util;

public final class Texts {

    private Texts() {
    }

    /** Truncates to at most {@code max} chars; null-safe. */
    public static String truncate(String value, int max) {
        if (value == null || value.length() <= max) return value;
        return value.substring(0, max);
    }
}
