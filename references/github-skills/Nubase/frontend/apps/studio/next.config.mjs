import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// When STUDIO_STATIC_EXPORT=true (set by the Maven `with-frontend` build) produce a
// fully static export served by the Java backend under the `/studio` base path, so a
// single `java -jar` serves both the API and the UI. Otherwise keep the standard
// standalone output used by `pnpm dev:studio` (root path, separate :3000 dev server).
const staticExport = process.env.STUDIO_STATIC_EXPORT === 'true';

// In standalone mode (the all-in-one Docker image, and `pnpm dev:studio`) the Studio UI
// is served on its own origin/port (:3000) while the backend API lives on :9999. Rather
// than baking an absolute API URL into the browser bundle at build time — which breaks the
// moment the server is reached at any host other than localhost — the client uses RELATIVE
// API paths (see API_BASE in src/lib/api.ts) and this Next server proxies them to the
// backend. The browser therefore only ever talks to the same host it loaded Studio from, so
// the image works at any IP/domain with no rebuild. The proxy target is the *internal*
// backend address (constant inside the container), so it's safe to fix at build time; set
// NUBASE_INTERNAL_API_URL only for split deployments where the backend lives elsewhere.
const internalApiUrl = process.env.NUBASE_INTERNAL_API_URL || 'http://127.0.0.1:9999';
const apiProxyPrefixes = ['/auth/v1', '/rest/v1', '/storage/v1', '/functions', '/assets', '/cron', '/mem', '/v1'];

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  transpilePackages: ['@nubase/ui', '@nubase/config'],
  experimental: {
    outputFileTracingRoot: path.join(__dirname, '../../'),
    optimizePackageImports: ['lucide-react'],
  },
  ...(staticExport
    ? {
        // Served same-origin by the Java backend under /studio, so relative API paths
        // already resolve to the backend — no proxy needed (and `output: export` can't
        // do rewrites anyway).
        output: 'export',
        basePath: '/studio',
        trailingSlash: true,
      }
    : {
        output: 'standalone',
        async rewrites() {
          return apiProxyPrefixes.map((prefix) => ({
            source: `${prefix}/:path*`,
            destination: `${internalApiUrl}${prefix}/:path*`,
          }));
        },
      }),
};

export default nextConfig;
