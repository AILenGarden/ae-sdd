package ai.nubase.ai.gateway.util;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.*;

public final class RequestLogSanitizer {

    private static final int MEDIA_PAYLOAD_PREVIEW_LENGTH = 32;
    private static final int MEDIA_PAYLOAD_HASH_LENGTH = 16;
    private static final Set<String> MEDIA_PAYLOAD_FIELDS = Set.of(
            "base64",
            "bytesbase64encoded",
            "bytes_base64_encoded",
            "imagebytes",
            "image_bytes",
            "imagebase64",
            "image_base64",
            "videobytes",
            "video_bytes",
            "videobase64",
            "video_base64",
            "b64_json"
    );
    private static final Set<String> MEDIA_CONTAINER_FIELDS = Set.of(
            "image",
            "video",
            "lastframe",
            "last_frame",
            "inlinedata",
            "inline_data",
            "source",
            "inputimages",
            "input_images",
            "referenceimage",
            "reference_image",
            "referenceimages",
            "reference_images",
            "subjectreference",
            "subject_reference"
    );
    private static final Set<String> CONDITIONAL_MEDIA_VALUE_FIELDS = Set.of(
            "image",
            "video",
            "firstframeimage",
            "first_frame_image",
            "lastframeimage",
            "last_frame_image"
    );

    private RequestLogSanitizer() {
    }

    public static String toLogJson(ObjectMapper objectMapper, Object request) {
        try {
            JsonNode requestNode = objectMapper.valueToTree(request);
            JsonNode sanitizedNode = sanitizeMediaPayloads(objectMapper, requestNode, null, null);
            return objectMapper.writeValueAsString(sanitizedNode);
        } catch (Exception exception) {
            return "[unavailable]";
        }
    }

    private static JsonNode sanitizeMediaPayloads(
            ObjectMapper objectMapper,
            JsonNode node,
            String fieldName,
            String parentFieldName) {
        if (node == null) {
            return null;
        }
        if (node.isObject()) {
            ObjectNode objectNode = (ObjectNode) node;
            List<String> fieldNames = new ArrayList<>();
            objectNode.fieldNames().forEachRemaining(fieldNames::add);
            for (String childFieldName : fieldNames) {
                JsonNode childNode = objectNode.get(childFieldName);
                objectNode.set(childFieldName,
                        sanitizeMediaPayloads(objectMapper, childNode, childFieldName, fieldName));
            }
            return objectNode;
        }
        if (node.isArray()) {
            ArrayNode arrayNode = (ArrayNode) node;
            for (int index = 0; index < arrayNode.size(); index++) {
                JsonNode childNode = arrayNode.get(index);
                arrayNode.set(index,
                        sanitizeMediaPayloads(objectMapper, childNode, fieldName, parentFieldName));
            }
            return arrayNode;
        }
        if (node.isTextual() && shouldSummarizeMediaPayload(fieldName, parentFieldName, node.asText())) {
            return mediaPayloadSummary(objectMapper, node.asText());
        }
        return node;
    }

    private static boolean shouldSummarizeMediaPayload(String fieldName, String parentFieldName, String value) {
        if (value == null || value.isBlank()) {
            return false;
        }
        String normalizedFieldName = normalizeFieldName(fieldName);
        if (MEDIA_PAYLOAD_FIELDS.contains(normalizedFieldName)) {
            return true;
        }
        if (isDataUrlMedia(value)) {
            return true;
        }
        boolean dataFieldInMediaContainer = "data".equals(normalizedFieldName)
                && MEDIA_CONTAINER_FIELDS.contains(normalizeFieldName(parentFieldName));
        if (dataFieldInMediaContainer || ("data".equals(normalizedFieldName) && looksLikeBase64Payload(value))) {
            return true;
        }
        boolean mediaNamedField = CONDITIONAL_MEDIA_VALUE_FIELDS.contains(normalizedFieldName)
                || containsMediaKeyword(normalizedFieldName);
        return mediaNamedField && looksLikeBase64Payload(value);
    }

    private static JsonNode mediaPayloadSummary(ObjectMapper objectMapper, String value) {
        ObjectNode summary = objectMapper.createObjectNode();
        summary.put("omitted", true);
        summary.put("length", value.length());
        summary.put("sha256Prefix", sha256Prefix(value));
        summary.put("preview", preview(value));
        return summary;
    }

    private static String preview(String value) {
        if (value.length() <= MEDIA_PAYLOAD_PREVIEW_LENGTH) {
            return value;
        }
        int halfLength = MEDIA_PAYLOAD_PREVIEW_LENGTH / 2;
        return value.substring(0, halfLength) + "..." + value.substring(value.length() - halfLength);
    }

    private static String sha256Prefix(String value) {
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256").digest(value.getBytes(StandardCharsets.UTF_8));
            String hash = HexFormat.of().formatHex(digest);
            return hash.substring(0, Math.min(MEDIA_PAYLOAD_HASH_LENGTH, hash.length()));
        } catch (NoSuchAlgorithmException exception) {
            throw new IllegalStateException("SHA-256 is unavailable", exception);
        }
    }

    private static boolean isDataUrlMedia(String value) {
        return value.startsWith("data:image/") || value.startsWith("data:video/");
    }

    private static boolean looksLikeBase64Payload(String value) {
        if (value.length() < 128) {
            return false;
        }
        int inspectedLength = Math.min(value.length(), 256);
        for (int index = 0; index < inspectedLength; index++) {
            char current = value.charAt(index);
            boolean validBase64Char = (current >= 'A' && current <= 'Z')
                    || (current >= 'a' && current <= 'z')
                    || (current >= '0' && current <= '9')
                    || current == '+'
                    || current == '/'
                    || current == '-'
                    || current == '_'
                    || current == '='
                    || current == '\r'
                    || current == '\n';
            if (!validBase64Char) {
                return false;
            }
        }
        return true;
    }

    private static boolean containsMediaKeyword(String fieldName) {
        return fieldName.contains("image") || fieldName.contains("video") || fieldName.contains("frame");
    }

    private static String normalizeFieldName(String fieldName) {
        return fieldName == null ? "" : fieldName.toLowerCase(Locale.ROOT);
    }
}
