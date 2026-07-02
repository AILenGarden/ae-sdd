package ai.nubase.functions.util;

import ai.nubase.common.util.IdentifierPatterns;

import java.util.Locale;

public final class EdgeFunctionNames {

    private EdgeFunctionNames() {
    }

    public static String normalizeSlug(String value) {
        if (value == null) {
            throw new IllegalArgumentException("Function slug is required");
        }
        String slug = value.trim()
                .toLowerCase(Locale.ROOT)
                .replaceAll("[^a-z0-9_-]+", "-")
                .replaceAll("^-+", "")
                .replaceAll("-+$", "");
        if (!isValidSlug(slug)) {
            throw new IllegalArgumentException("Function slug must match " + IdentifierPatterns.RESOURCE_NAME);
        }
        return slug;
    }

    public static boolean isValidSlug(String value) {
        return value != null && value.matches(IdentifierPatterns.RESOURCE_NAME);
    }

    public static boolean isValidSecretName(String value) {
        return value != null && value.matches(IdentifierPatterns.SECRET_NAME);
    }
}
