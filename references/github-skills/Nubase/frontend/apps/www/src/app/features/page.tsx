import Link from 'next/link';
import {
  Brain,
  Clock,
  Database,
  Globe,
  HardDrive,
  KeyRound,
  Sparkles,
  Zap,
  CheckCircle2,
} from 'lucide-react';

/**
 * Detailed feature inventory across the eight modules. Each section is the
 * source-of-truth for "what does Nubase actually do" — keep it specific so engineering
 * evaluators don't have to dig through commits. Ordered by how broadly an app needs
 * them: data & identity, then the deploy layer, then AI enhancements and scheduling.
 */

interface Pillar {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  accent: string;
  intro: string;
  groups: Array<{
    label: string;
    items: string[];
  }>;
  docsHref: string;
}

const PILLARS: Pillar[] = [
  {
    icon: Database,
    title: 'Database',
    accent: 'border-emerald-500/30 bg-emerald-500/5',
    docsHref: '/docs/database',
    intro:
      'Every project gets a dedicated PostgreSQL database — not a schema in a shared instance. Full SQL access, RLS by default, REST API generated for every table.',
    groups: [
      {
        label: 'Isolation',
        items: [
          'Database-level multi-tenancy via RoutingDataSource + HikariCP per tenant',
          'GuardianDataSource refuses any unauthenticated DB access',
          'Per-tenant encrypted credentials, JWT secrets, role mapping',
        ],
      },
      {
        label: 'REST API',
        items: [
          'PostgREST-compatible /rest/v1/* implemented in Java (no separate process)',
          'select / filter / order / limit / offset / range pagination',
          'Schema metadata cache, refreshed via PostgreSQL NOTIFY',
        ],
      },
      {
        label: 'Security',
        items: [
          'RLS executed via SET LOCAL ROLE + request.jwt.claims GUC variable',
          'service_role / authenticated / anon role separation, BYPASSRLS for admin',
          '@RequireServiceRole AOP guard for management endpoints',
        ],
      },
    ],
  },
  {
    icon: KeyRound,
    title: 'Auth',
    accent: 'border-amber-500/30 bg-amber-500/5',
    docsHref: '/docs/auth',
    intro:
      'Supabase GoTrue-compatible: email/password, OAuth, JWT issuance, refresh-token rotation. Per-tenant JWT secrets mean a breach of one tenant cannot forge tokens for another.',
    groups: [
      {
        label: 'Identity',
        items: [
          'Email + password sign-up / sign-in / recovery',
          'OAuth providers: Google, GitHub (extensible via OAuthProvider interface)',
          'MFA / TOTP, OTP & magic links, anonymous sign-in',
        ],
      },
      {
        label: 'Tokens',
        items: [
          'JWT access token signed with per-tenant secret (no cross-tenant forgery)',
          'Refresh token rotation with parent-link tracking',
          'Two-layer apikey: tenant-level (ref claim) + user-level (Bearer)',
        ],
      },
      {
        label: 'Admin',
        items: [
          'Provision new tenant databases via POST /auth/v1/admin/init/database',
          'Service-role token generation, schema/RLS DDL export',
          'Ad-hoc SQL execution and admin user CRUD',
        ],
      },
    ],
  },
  {
    icon: HardDrive,
    title: 'Storage',
    accent: 'border-sky-500/30 bg-sky-500/5',
    docsHref: '/docs/storage',
    intro:
      'S3-compatible object storage with metadata in Postgres. Bucket policies, signed URLs, RLS-aware ACLs — all under the same JWT model your app already uses.',
    groups: [
      {
        label: 'Buckets & objects',
        items: [
          'Create/list/update/delete buckets via /storage/v1/bucket',
          'Public vs. private buckets, per-bucket size limits + MIME allow-list',
          'File metadata stored in storage.objects with RLS policies',
        ],
      },
      {
        label: 'Backend',
        items: [
          'AWS S3 SDK — works with Cloudflare R2, MinIO, LocalStack, any S3-compatible',
          'Per-tenant key prefix layout under one global bucket',
          'Signed URLs for time-limited public access',
        ],
      },
      {
        label: 'Vector storage (optional)',
        items: [
          'Separate AWS S3 Vectors integration for large file-content vectors',
          'Independent from Memory module — used for document/asset embeddings',
        ],
      },
    ],
  },
  {
    icon: Globe,
    title: 'Assets',
    accent: 'border-teal-500/30 bg-teal-500/5',
    docsHref: '/docs/getting-started',
    intro:
      'Where the generated frontend goes live. Your agent uploads static files and serves them from a public CDN — no separate static host, same project token model as everything else.',
    groups: [
      {
        label: 'Publish',
        items: [
          'Upload / list / delete via /assets/admin/v1 and MCP assets_upload',
          'UTF-8 text or base64 bodies; Content-Type inferred from the path',
          'Returns the resolved public URL for every asset',
        ],
      },
      {
        label: 'Deliver',
        items: [
          'Public read at /assets/v1/{path} — no apikey, tenant from subdomain',
          'Cache-Control / ETag / Last-Modified with 304 conditional GETs',
          'Per-project default cache policy + optional custom CDN domain',
        ],
      },
      {
        label: 'Modes',
        items: [
          'CDN mode: a dedicated R2 bucket behind a custom domain',
          'Backend mode: served by Nubase under a reserved key prefix',
        ],
      },
    ],
  },
  {
    icon: Zap,
    title: 'Functions',
    accent: 'border-yellow-500/30 bg-yellow-500/5',
    docsHref: '/docs/getting-started',
    intro:
      'Deploy AI-written backend logic as edge functions, with Nubase as the public gateway. Secrets, logs and rate limits included.',
    groups: [
      {
        label: 'Deploy',
        items: [
          'Scaffold / deploy / invoke via CLI and MCP functions_deploy',
          'TypeScript bundled with esbuild; every deploy kept as a version',
          'Local executor or Cloudflare Workers for Platforms dispatcher',
        ],
      },
      {
        label: 'Invoke',
        items: [
          'Public path /functions/v1/{slug} with verify_jwt',
          'Per-project & per-function rate limits, invocation logs',
          'service_role never injected by default',
        ],
      },
      {
        label: 'Secrets',
        items: [
          'Per-function secrets encrypted in the metadata DB',
          'Set by name via functions_secrets_set; values never returned',
          'Injected as env / Worker secret_text bindings',
        ],
      },
    ],
  },
  {
    icon: Sparkles,
    title: 'AI Gateway',
    accent: 'border-violet-500/30 bg-violet-500/5',
    docsHref: '/docs/getting-started',
    intro:
      'Route model calls through Nubase with per-project keys and usage tracking — bring your own model.',
    groups: [
      {
        label: 'Endpoints',
        items: [
          'OpenAI-compatible /v1 and Anthropic-compatible /v1/messages',
          'Streaming + token counting passthrough',
          'Model routing across providers',
        ],
      },
      {
        label: 'Keys & usage',
        items: [
          'Per-project nbk_ keys (issue / revoke)',
          'Token, request and cost analytics per key and model',
          'Daily and by-model breakdowns',
        ],
      },
    ],
  },
  {
    icon: Brain,
    title: 'Memory',
    accent: 'border-fuchsia-500/30 bg-fuchsia-500/5',
    docsHref: '/docs/memory',
    intro:
      'A first-class LLM memory layer — not bolted on. mem0-compatible API, multi-signal retrieval, audit history, and per-tenant isolation that rides the same auth model as the rest of nubase.',
    groups: [
      {
        label: 'Write & decide',
        items: [
          'POST /mem/v1/memories with infer=true: LLM extracts facts and emits ADD / UPDATE / DELETE / NONE per fact',
          'infer=false path stores raw messages verbatim',
          'Per-call user / agent / run scope; deduplication by content hash',
          'Per-fact entity extraction in the same LLM call (no extra round-trip)',
        ],
      },
      {
        label: 'Retrieve',
        items: [
          'Hybrid fusion: pgvector cosine top-K + BM25 (ts_rank_cd) + entity-link boost',
          'Spread-attenuated entity boost (mem0 v3 algorithm)',
          'PG text-search config configurable (simple / english / zhparser for CJK)',
          'Advanced metadata filters: eq/ne/gt/gte/lt/lte/in/nin/contains/icontains + AND/OR/NOT',
        ],
      },
      {
        label: 'Manage & audit',
        items: [
          'Full audit history (ADD/UPDATE/DELETE) per memory id',
          'Entity store with linked_memory_ids array, hard cap for hot entities',
          'Batch delete by owner, full tenant reset with double-confirm',
          'Admin Studio: list, search, edit, history, entities, settings, danger zone',
        ],
      },
      {
        label: 'Providers',
        items: [
          'Chat: OpenAI · Anthropic · any OpenAI-compatible (DashScope, DeepSeek, Moonshot, vLLM, Ollama)',
          'Embeddings: OpenAI · generic OpenAI-compatible (1536-dim default, configurable)',
          'In-process Caffeine cache for embeddings (content-addressed, safe across tenants)',
          'Pre-flight isAvailable() — no wasted HTTP when keys missing',
        ],
      },
    ],
  },
  {
    icon: Clock,
    title: 'cron',
    accent: 'border-orange-500/30 bg-orange-500/5',
    docsHref: '/docs/getting-started',
    intro:
      'Recurring jobs run by the control plane — invoke an edge function or a database function on a schedule, safely across instances.',
    groups: [
      {
        label: 'Schedule',
        items: [
          'Crontab (UTC); 5-field and 6-field forms accepted',
          'Targets: edge_function, or a named db_function with JSON args',
          'Per-job timeout; manage via /cron/admin/v1 and MCP cron_create',
        ],
      },
      {
        label: 'Semantics',
        items: [
          'Control-plane scheduler with a row-level claim — no double-run',
          'Run history per job with status / duration / error',
          'Pause / resume without a catch-up storm',
        ],
      },
    ],
  },
];

export default function FeaturesPage() {
  return (
    <main className="container py-20">
      <header className="max-w-3xl">
        <p className="mb-3 text-xs uppercase tracking-wider text-muted-foreground">
          Capabilities
        </p>
        <h1 className="text-4xl font-semibold tracking-tight">
          Eight modules. One backend. Generate → live.
        </h1>
        <p className="mt-3 text-pretty text-muted-foreground">
          A complete inventory of what Nubase ships out of the box — the modules an AI-written app
          needs to go online: <strong>Database · Auth · Storage · Assets · Functions · AI Gateway ·
          Memory · cron</strong>. Detailed reference lives in{' '}
          <Link href="/docs" className="underline">the docs</Link>.
        </p>
      </header>

      <div className="mt-12 space-y-8">
        {PILLARS.map((p) => (
          <PillarSection key={p.title} {...p} />
        ))}
      </div>
    </main>
  );
}

function PillarSection({ icon: Icon, title, accent, intro, groups, docsHref }: Pillar) {
  return (
    <section className={`rounded-2xl border p-6 ${accent}`}>
      <header className="flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-border bg-background">
            <Icon className="h-5 w-5" />
          </div>
          <div>
            <h2 className="text-2xl font-semibold tracking-tight">{title}</h2>
            <p className="mt-1 max-w-3xl text-sm text-muted-foreground">{intro}</p>
          </div>
        </div>
        <Link
          href={docsHref}
          className="shrink-0 text-xs text-muted-foreground hover:text-foreground"
        >
          Docs →
        </Link>
      </header>

      <div className="mt-6 grid gap-6 sm:grid-cols-2">
        {groups.map((g) => (
          <div key={g.label}>
            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {g.label}
            </h3>
            <ul className="space-y-1.5 text-sm">
              {g.items.map((item) => (
                <li key={item} className="flex items-start gap-2">
                  <CheckCircle2 className="mt-0.5 h-3.5 w-3.5 shrink-0 text-foreground/60" />
                  <span>{item}</span>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </section>
  );
}
