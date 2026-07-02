package ai.nubase.common.config;

import org.springframework.core.io.Resource;
import org.springframework.lang.NonNull;
import org.springframework.web.servlet.config.annotation.ResourceHandlerRegistry;
import org.springframework.web.servlet.config.annotation.ViewControllerRegistry;
import org.springframework.web.servlet.config.annotation.WebMvcConfigurer;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.servlet.resource.PathResourceResolver;

import java.io.IOException;

/**
 * Serves the statically-exported Studio UI bundled into the jar under {@code /studio}.
 *
 * <p>The Studio is built with Next.js {@code output: 'export'} and {@code basePath: '/studio'}
 * (see the Maven {@code with-frontend} profile), then copied to
 * {@code classpath:/static/studio/}. This config exposes it at {@code /studio/**} so a single
 * {@code java -jar} serves both the API (at the root paths) and the UI.
 *
 * <p>Because the export is per-route (not a single SPA shell), client-only dynamic routes
 * — {@code /studio/project/{ref}/...}, {@code .../memory/{id}}, {@code .../storage/{bucket}}
 * — have no concrete file. The build emits a single {@code __shell__} page per dynamic
 * segment; the resolver below rewrites the runtime ref/id/bucket to {@code __shell__} and
 * serves that shell, letting the client read the real value from the URL.
 *
 * <p>When the frontend was not bundled (plain backend build), {@code classpath:/static/studio}
 * is empty and every {@code /studio/**} request simply 404s — no effect on the API.
 */
@Configuration
public class StudioWebConfig implements WebMvcConfigurer {

    private static final String LOCATION = "classpath:/static/studio/";

    @Override
    public void addViewControllers(@NonNull ViewControllerRegistry registry) {
        // Open the app at the root; land directly on the projects list. These exact paths
        // would otherwise have no static file (the export's index just client-redirects).
        registry.addRedirectViewController("/", "/studio/projects/");
        registry.addRedirectViewController("/studio", "/studio/projects/");
        registry.addRedirectViewController("/studio/", "/studio/projects/");
    }

    @Override
    public void addResourceHandlers(@NonNull ResourceHandlerRegistry registry) {
        registry.addResourceHandler("/studio/**")
                .addResourceLocations(LOCATION)
                .resourceChain(true)
                .addResolver(new StudioResourceResolver());
    }

    /**
     * Resolves a request path under {@code /studio/**} to an exported file, falling back to
     * the {@code __shell__} page for client-side dynamic routes.
     */
    static final class StudioResourceResolver extends PathResourceResolver {

        @Override
        protected Resource getResource(@NonNull String resourcePath, @NonNull Resource location)
                throws IOException {
            // Assets (anything with a file extension in the last segment) are served as-is.
            // Next's static App Router also emits route data as */index.txt; those files need
            // the same dynamic-route shell fallback as HTML pages.
            if (hasExtension(resourcePath)) {
                Resource r = location.createRelative(resourcePath);
                if (r.exists() && r.isReadable()) {
                    return r;
                }
                if (isRouteDataRequest(resourcePath)) {
                    r = dynamicRouteResource(location, stripIndexTxt(resourcePath), "index.txt");
                    if (r != null) {
                        return r;
                    }
                }
                return null;
            }

            // Navigation route → directory index.html.
            Resource r = indexAt(location, resourcePath);
            if (r != null) {
                return r;
            }

            r = dynamicRouteResource(location, resourcePath, "index.html");
            if (r != null) {
                return r;
            }
            // Unknown client route → serve the SPA's 404 page (avoids a hard 500 from the
            // global exception handler on NoResourceFoundException).
            Resource notFound = location.createRelative("404.html");
            return (notFound.exists() && notFound.isReadable()) ? notFound : null;
        }

        private Resource indexAt(Resource location, String dir) throws IOException {
            return fileAt(location, dir, "index.html");
        }

        private Resource fileAt(Resource location, String dir, String fileName) throws IOException {
            String d = trim(dir);
            String rel = d.isEmpty() ? fileName : d + "/" + fileName;
            Resource r = location.createRelative(rel);
            return (r.exists() && r.isReadable()) ? r : null;
        }

        private Resource dynamicRouteResource(Resource location, String resourcePath, String fileName)
                throws IOException {
            // Dynamic route fallback: rewrite the project ref (segment 1) to __shell__, then,
            // if still missing, the last segment too (covers memory/{id} and storage/{bucket}).
            String trimmed = trim(resourcePath);
            if (trimmed.isEmpty()) {
                return null;
            }
            String[] seg = trimmed.split("/");
            if (seg.length < 2 || !"project".equals(seg[0])) {
                return null;
            }
            String[] withRef = seg.clone();
            withRef[1] = "__shell__";
            Resource r = fileAt(location, String.join("/", withRef), fileName);
            if (r != null) {
                return r;
            }
            String[] withLeaf = withRef.clone();
            withLeaf[withLeaf.length - 1] = "__shell__";
            return fileAt(location, String.join("/", withLeaf), fileName);
        }

        private static boolean hasExtension(String path) {
            String p = trim(path);
            int slash = p.lastIndexOf('/');
            String last = slash >= 0 ? p.substring(slash + 1) : p;
            return last.contains(".");
        }

        private static boolean isRouteDataRequest(String path) {
            String p = trim(path);
            return "index.txt".equals(p) || p.endsWith("/index.txt");
        }

        private static String stripIndexTxt(String path) {
            String p = trim(path);
            if ("index.txt".equals(p)) {
                return "";
            }
            return p.substring(0, p.length() - "/index.txt".length());
        }

        private static String trim(String p) {
            if (p == null) {
                return "";
            }
            int start = 0;
            int end = p.length();
            while (start < end && p.charAt(start) == '/') {
                start++;
            }
            while (end > start && p.charAt(end - 1) == '/') {
                end--;
            }
            return p.substring(start, end);
        }
    }
}
