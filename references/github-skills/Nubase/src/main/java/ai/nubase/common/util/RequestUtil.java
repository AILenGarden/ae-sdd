package ai.nubase.common.util;

import ai.nubase.common.constant.HttpHeaderConstant;
import jakarta.servlet.ServletRequest;
import jakarta.servlet.ServletRequestWrapper;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.Part;
import lombok.extern.slf4j.Slf4j;
import org.springframework.util.AntPathMatcher;
import org.springframework.web.servlet.HandlerMapping;
import org.springframework.web.util.ContentCachingRequestWrapper;
import org.springframework.web.multipart.MultipartFile;
import org.springframework.web.multipart.MultipartHttpServletRequest;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;

/**
 * HTTP request utility class.
 *
 * TODO(perf): Optimize request-body and multipart parsing paths.
 * Current implementation may copy large payloads multiple times in memory.
 */
@Slf4j
public final class RequestUtil {

    private RequestUtil() {
    }

    private static final int BODY_READ_BUFFER_SIZE = 8192;

    private static final AntPathMatcher matcher = new AntPathMatcher();

    /**
     * Extracts the portion of the request path matched by the /** wildcard.
     * <p>
     * Example: mapping = /storage/v1/object/{bucketId}/**, request = /storage/v1/object/test/folder/a.jpg
     * returns folder/a.jpg.
     */
    public static String extractPathVariable(HttpServletRequest request) {
        String pattern = (String) request.getAttribute(HandlerMapping.BEST_MATCHING_PATTERN_ATTRIBUTE);
        String path = (String) request.getAttribute(HandlerMapping.PATH_WITHIN_HANDLER_MAPPING_ATTRIBUTE);
        return matcher.extractPathWithinPattern(pattern, path);
    }

    /**
     * Parses the file and cacheControl from an upload request.
     * For multipart/form-data the raw boundary parser is used, which tolerates file fields
     * declared as name="".
     */
    public record ParsedUpload(byte[] fileBytes, String contentType, String cacheControl) {
    }

    /**
     * Reads the current request body first; if it has already been consumed,
     * falls back to the cache held by ContentCachingRequestWrapper.
     */
    public static byte[] readRawRequestBody(HttpServletRequest request) throws IOException {
        return readRawRequestBody(request, Long.MAX_VALUE);
    }

    /**
     * Reads the request body and fails immediately if it exceeds maxBytes.
     */
    public static byte[] readRawRequestBody(HttpServletRequest request, long maxBytes) throws IOException {
        long normalizedMaxBytes = normalizeMaxBytes(maxBytes);
        byte[] body = readInputStreamWithLimit(request.getInputStream(), normalizedMaxBytes);
        if (body.length > 0) {
            return body;
        }
        byte[] cached = extractCachedBody(request);
        if (cached.length > normalizedMaxBytes) {
            throw payloadTooLargeException(normalizedMaxBytes);
        }
        return cached.length > 0 ? cached : body;
    }

    public static ParsedUpload parseUploadParts(HttpServletRequest request) throws IOException {
        return parseUploadParts(request, readRawRequestBody(request));
    }

    public static ParsedUpload parseUploadParts(HttpServletRequest request, byte[] rawBody) {
        byte[] requestBody = rawBody == null ? new byte[0] : rawBody;
        String requestContentType = request.getContentType();
        String cacheControl = request.getHeader(HttpHeaderConstant.CACHE_CONTROL);
        log.debug("parseUploadParts: uri={}, requestType={}, contentType={}, rawBodySize={}",
                request.getRequestURI(), request.getClass().getName(), requestContentType, requestBody.length);

        if (requestContentType != null
                && requestContentType.toLowerCase(Locale.ROOT).startsWith("multipart/form-data")) {
            if (requestBody.length > 0) {
                try {
                    return parseMultipartUpload(requestBody, requestContentType, cacheControl);
                } catch (Exception e) {
                    log.debug("Raw multipart parse failed, fallback to framework multipart: {}", e.getMessage());
                }
            }
            ParsedUpload fallbackParsed = parseUploadPartsFromFramework(request, cacheControl);
            if (fallbackParsed != null) {
                return fallbackParsed;
            }
            log.warn("Multipart upload parse failed: uri={}, rawBodySize={}, requestType={}",
                    request.getRequestURI(), requestBody.length, request.getClass().getName());
            throw new IllegalArgumentException("No uploaded file was found");
        }

        if (requestBody.length == 0) {
            log.warn("Non-multipart upload has empty body: uri={}, requestType={}",
                    request.getRequestURI(), request.getClass().getName());
            throw new IllegalArgumentException("No uploaded file was found");
        }
        return new ParsedUpload(requestBody, requestContentType, cacheControl);
    }

    private static ParsedUpload parseUploadPartsFromFramework(HttpServletRequest request, String cacheControlHeader) {
        ParsedUpload springMultipartParsed = parseUploadPartsFromSpringMultipart(request, cacheControlHeader);
        if (springMultipartParsed != null) {
            return springMultipartParsed;
        }
        return parseUploadPartsFromServletParts(request, cacheControlHeader);
    }

    private static ParsedUpload parseUploadPartsFromSpringMultipart(HttpServletRequest request, String cacheControlHeader) {
        if (!(request instanceof MultipartHttpServletRequest multipartRequest)) {
            return null;
        }
        String cacheControl = cacheControlHeader;
        if (cacheControl == null) {
            cacheControl = multipartRequest.getParameter("cacheControl");
        }
        for (List<MultipartFile> files : multipartRequest.getMultiFileMap().values()) {
            if (files == null) {
                continue;
            }
            for (MultipartFile file : files) {
                if (file == null) {
                    continue;
                }
                try {
                    return new ParsedUpload(file.getBytes(), file.getContentType(), cacheControl);
                } catch (Exception e) {
                    log.debug("Read MultipartFile failed: {}", e.getMessage());
                }
            }
        }
        return null;
    }

    private static ParsedUpload parseUploadPartsFromServletParts(HttpServletRequest request, String cacheControlHeader) {
        try {
            byte[] fileBytes = null;
            String fileContentType = null;
            String cacheControl = cacheControlHeader;
            for (Part part : request.getParts()) {
                if (part == null) {
                    continue;
                }
                String partContentType = part.getContentType();
                String submittedFileName = part.getSubmittedFileName();
                if ((partContentType != null || submittedFileName != null) && fileBytes == null) {
                    fileBytes = part.getInputStream().readAllBytes();
                    fileContentType = partContentType;
                } else if ("cacheControl".equals(part.getName()) && cacheControl == null) {
                    cacheControl = new String(part.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
                }
            }
            if (fileBytes == null) {
                return null;
            }
            return new ParsedUpload(fileBytes, fileContentType, cacheControl);
        } catch (Exception e) {
            log.debug("Read servlet parts failed: {}", e.getMessage());
            return null;
        }
    }

    private static ParsedUpload parseMultipartUpload(byte[] body, String contentTypeHeader, String cacheControlHeader) {
        String boundary = extractBoundary(contentTypeHeader);
        if (boundary == null) {
            throw new IllegalArgumentException("multipart/form-data is missing the boundary");
        }

        byte[] boundaryStart = ("--" + boundary).getBytes(StandardCharsets.ISO_8859_1);
        byte[] boundaryDelimiterCrlf = ("\r\n--" + boundary).getBytes(StandardCharsets.ISO_8859_1);
        byte[] boundaryDelimiterLf = ("\n--" + boundary).getBytes(StandardCharsets.ISO_8859_1);
        byte[] headerSeparatorCrlf = "\r\n\r\n".getBytes(StandardCharsets.ISO_8859_1);
        byte[] headerSeparatorLf = "\n\n".getBytes(StandardCharsets.ISO_8859_1);

        byte[] fileBytes = null;
        String fileContentType = null;
        String cacheControl = cacheControlHeader;

        int cursor = indexOf(body, boundaryStart, 0);
        while (cursor >= 0 && cursor < body.length) {
            cursor += boundaryStart.length;
            if (startsWith(body, cursor, "--".getBytes(StandardCharsets.ISO_8859_1))) {
                break;
            }
            if (startsWith(body, cursor, "\r\n".getBytes(StandardCharsets.ISO_8859_1))) {
                cursor += 2;
            } else if (startsWith(body, cursor, "\n".getBytes(StandardCharsets.ISO_8859_1))) {
                cursor += 1;
            }

            int headersEnd = indexOf(body, headerSeparatorCrlf, cursor);
            int separatorLength = headerSeparatorCrlf.length;
            if (headersEnd < 0) {
                headersEnd = indexOf(body, headerSeparatorLf, cursor);
                separatorLength = headerSeparatorLf.length;
            }
            if (headersEnd < 0) {
                break;
            }

            String headers = new String(body, cursor, headersEnd - cursor, StandardCharsets.ISO_8859_1);
            int contentStart = headersEnd + separatorLength;
            int nextBoundary = indexOf(body, boundaryDelimiterCrlf, contentStart);
            if (nextBoundary < 0) {
                nextBoundary = indexOf(body, boundaryDelimiterLf, contentStart);
            }
            int contentEnd = nextBoundary >= 0 ? nextBoundary : body.length;
            byte[] partBody = Arrays.copyOfRange(body, contentStart, contentEnd);

            String partContentType = extractHeaderValue(headers, "Content-Type");
            String partName = extractDispositionParam(headers, "name");
            String partFilename = extractDispositionParam(headers, "filename");

            if ((partContentType != null || partFilename != null) && fileBytes == null) {
                fileBytes = partBody;
                fileContentType = partContentType;
            } else if ("cacheControl".equals(partName) && cacheControl == null) {
                cacheControl = new String(partBody, StandardCharsets.UTF_8).trim();
            }

            if (nextBoundary < 0) {
                break;
            }
            cursor = nextBoundary + (body[nextBoundary] == '\r' ? 2 : 1);
        }

        if (fileBytes == null) {
            throw new IllegalArgumentException("No uploaded file was found");
        }

        return new ParsedUpload(fileBytes, fileContentType, cacheControl);
    }

    private static String extractBoundary(String contentTypeHeader) {
        if (contentTypeHeader == null) {
            return null;
        }
        for (String token : contentTypeHeader.split(";")) {
            String trimmed = token.trim();
            if (trimmed.toLowerCase(Locale.ROOT).startsWith("boundary=")) {
                String value = trimmed.substring("boundary=".length()).trim();
                return trimQuotes(value);
            }
        }
        return null;
    }

    private static String extractHeaderValue(String headers, String key) {
        for (String line : headers.split("\\r?\\n")) {
            int idx = line.indexOf(':');
            if (idx <= 0) {
                continue;
            }
            String headerName = line.substring(0, idx).trim();
            if (headerName.equalsIgnoreCase(key)) {
                return line.substring(idx + 1).trim();
            }
        }
        return null;
    }

    private static String extractDispositionParam(String headers, String key) {
        String disposition = extractHeaderValue(headers, "Content-Disposition");
        if (disposition == null) {
            return null;
        }
        String expectedPrefix = key.toLowerCase(Locale.ROOT) + "=";
        for (String token : disposition.split(";")) {
            String trimmed = token.trim();
            if (trimmed.toLowerCase(Locale.ROOT).startsWith(expectedPrefix)) {
                return trimQuotes(trimmed.substring(expectedPrefix.length()));
            }
        }
        return null;
    }

    private static String trimQuotes(String value) {
        if (value == null) {
            return null;
        }
        if (value.length() >= 2 && value.startsWith("\"") && value.endsWith("\"")) {
            return value.substring(1, value.length() - 1);
        }
        return value;
    }

    private static boolean startsWith(byte[] source, int offset, byte[] prefix) {
        if (offset < 0 || source == null || prefix == null || offset + prefix.length > source.length) {
            return false;
        }
        for (int i = 0; i < prefix.length; i++) {
            if (source[offset + i] != prefix[i]) {
                return false;
            }
        }
        return true;
    }

    private static int indexOf(byte[] source, byte[] target, int fromIndex) {
        if (source == null || target == null || target.length == 0) {
            return -1;
        }
        int start = Math.max(fromIndex, 0);
        for (int i = start; i <= source.length - target.length; i++) {
            int j = 0;
            while (j < target.length && source[i + j] == target[j]) {
                j++;
            }
            if (j == target.length) {
                return i;
            }
        }
        return -1;
    }

    private static byte[] extractCachedBody(ServletRequest request) {
        ServletRequest current = request;
        int guard = 0;
        while (current != null && guard++ < 16) {
            if (current instanceof ContentCachingRequestWrapper cachingRequestWrapper) {
                byte[] cached = cachingRequestWrapper.getContentAsByteArray();
                if (cached != null && cached.length > 0) {
                    return cached;
                }
            }
            if (current instanceof ServletRequestWrapper servletRequestWrapper) {
                current = servletRequestWrapper.getRequest();
                continue;
            }
            break;
        }
        return new byte[0];
    }

    private static long normalizeMaxBytes(long maxBytes) {
        if (maxBytes <= 0) {
            throw new IllegalArgumentException("Invalid max upload size");
        }
        return maxBytes;
    }

    private static byte[] readInputStreamWithLimit(InputStream inputStream, long maxBytes) throws IOException {
        ByteArrayOutputStream outputStream = new ByteArrayOutputStream();
        byte[] buffer = new byte[BODY_READ_BUFFER_SIZE];
        long totalRead = 0L;

        int read;
        while ((read = inputStream.read(buffer)) != -1) {
            totalRead += read;
            if (totalRead > maxBytes) {
                throw payloadTooLargeException(maxBytes);
            }
            outputStream.write(buffer, 0, read);
        }
        return outputStream.toByteArray();
    }

    private static IllegalArgumentException payloadTooLargeException(long maxBytes) {
        return new IllegalArgumentException("Payload too large: max allowed is " + maxBytes + " bytes");
    }

    public static String stringValue(Object value) {
        return value == null ? null : value.toString();
    }

    public static Integer intValue(Object value) {
        if (value == null) {
            return null;
        }
        if (value instanceof Number number) {
            return number.intValue();
        }
        try {
            return Integer.parseInt(value.toString());
        } catch (Exception e) {
            return null;
        }
    }

    @SuppressWarnings("unchecked")
    public static List<String> toStringList(Object value) {
        if (!(value instanceof List<?> list)) {
            return null;
        }
        return list.stream().map(item -> item == null ? null : item.toString()).toList();
    }
}
