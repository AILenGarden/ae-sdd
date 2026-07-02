package ai.nubase.postgrest;

import jakarta.servlet.http.HttpServletRequest;
import org.mockito.Mockito;

import java.io.BufferedReader;
import java.io.StringReader;
import java.util.Collections;
import java.util.Enumeration;
import java.util.Map;

import static org.mockito.Mockito.when;

/**
 * Test helper utilities for creating mock objects
 */
public class TestHelper {

    /**
     * Create a mock HttpServletRequest with common setup
     */
    public static HttpServletRequest createMockRequest(
        String method,
        Map<String, String[]> params,
        Map<String, String> headers
    ) {
        HttpServletRequest request = Mockito.mock(HttpServletRequest.class);

        when(request.getMethod()).thenReturn(method);
        when(request.getParameterMap()).thenReturn(params != null ? params : Collections.emptyMap());

        // Setup getParameter for each param
        if (params != null) {
            params.forEach((key, values) -> {
                when(request.getParameter(key)).thenReturn(values.length > 0 ? values[0] : null);
            });
        }

        // Setup headers
        if (headers != null && !headers.isEmpty()) {
            Enumeration<String> headerNames = Collections.enumeration(headers.keySet());
            when(request.getHeaderNames()).thenReturn(headerNames);

            headers.forEach((key, value) -> {
                when(request.getHeader(key)).thenReturn(value);
            });
        } else {
            when(request.getHeaderNames()).thenReturn(Collections.emptyEnumeration());
        }

        return request;
    }

    /**
     * Create a mock request with body
     */
    public static HttpServletRequest createMockRequestWithBody(
        String method,
        String body,
        Map<String, String[]> params
    ) throws Exception {
        HttpServletRequest request = createMockRequest(method, params, null);

        if (body != null) {
            when(request.getReader()).thenReturn(new BufferedReader(new StringReader(body)));
        }

        return request;
    }

    /**
     * Create enumeration from varargs
     */
    public static <T> Enumeration<T> enumeration(T... values) {
        return Collections.enumeration(java.util.Arrays.asList(values));
    }
}
