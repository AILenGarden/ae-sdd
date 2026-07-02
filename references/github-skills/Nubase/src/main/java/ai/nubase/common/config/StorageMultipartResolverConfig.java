package ai.nubase.common.config;

import jakarta.servlet.http.HttpServletRequest;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.multipart.MultipartResolver;
import org.springframework.web.multipart.support.StandardServletMultipartResolver;

/**
 * Globally keep the Spring multipart resolver, but skip storage object upload endpoints.
 * Storage uploads are parsed manually from the raw request body to support file parts with name="".
 */
@Configuration
public class StorageMultipartResolverConfig {

    private static final String STORAGE_OBJECT_PATH_PREFIX = "/storage/v1/object/";

    @Bean(name = "multipartResolver")
    public MultipartResolver multipartResolver() {
        return new StandardServletMultipartResolver() {
            @Override
            public boolean isMultipart(HttpServletRequest request) {
                String pathWithinApp = extractPathWithinApplication(request);
                if (pathWithinApp != null && pathWithinApp.startsWith(STORAGE_OBJECT_PATH_PREFIX)) {
                    return false;
                }
                return super.isMultipart(request);
            }
        };
    }

    private static String extractPathWithinApplication(HttpServletRequest request) {
        String requestUri = request.getRequestURI();
        if (requestUri == null) {
            return null;
        }
        String contextPath = request.getContextPath();
        if (contextPath != null && !contextPath.isEmpty() && requestUri.startsWith(contextPath)) {
            return requestUri.substring(contextPath.length());
        }
        return requestUri;
    }
}
