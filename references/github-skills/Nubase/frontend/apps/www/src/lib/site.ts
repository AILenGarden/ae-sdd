// Central site config. Override the domain at build time with NEXT_PUBLIC_SITE_URL.
export const SITE_URL = (process.env.NEXT_PUBLIC_SITE_URL ?? 'https://nubase.ai').replace(/\/+$/, '');

export const SITE = {
  name: 'Nubase',
  tagline: 'Turn AI-written code into real apps',
  description:
    'Turn AI-written code into real apps. Configure the plugin once and your coding agent ships the '
    + 'whole app online — frontend (Assets), backend (Functions), Database, Auth, Storage, AI Gateway, '
    + 'Memory and cron — on one free, self-hostable service, with MCP for Claude & Codex.',
  github: 'https://github.com/OtterMind/Nubase',
  npm: 'https://www.npmjs.com/package/nubase_cli',
  ogImage: '/og.png',
  keywords: [
    'deploy AI-generated app',
    'ship AI code',
    'AI app deployment',
    'open source backend',
    'self-hosted backend',
    'AI-native backend',
    'edge functions',
    'static site hosting',
    'AI memory',
    'Supabase alternative',
    'Firebase alternative',
    'PostgREST',
    'Row Level Security',
    'MCP',
    'Claude Code backend',
    'Codex backend',
    'BaaS',
    'backend as a service',
  ],
} as const;

export function url(path = '/'): string {
  return `${SITE_URL}${path.startsWith('/') ? path : `/${path}`}`;
}
