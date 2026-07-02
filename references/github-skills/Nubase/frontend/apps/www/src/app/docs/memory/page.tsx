import Link from 'next/link';

export default function MemoryDocsPage() {
  return (
    <div>
      <h1 className="text-3xl font-semibold tracking-tight">Memory</h1>
      <p className="mt-3 text-muted-foreground">
        A durable, queryable, evolving knowledge base per user — the fourth primitive next
        to Database, Storage and Auth. mem0-compatible REST surface, multi-signal retrieval,
        full audit history, per-tenant isolation.
      </p>

      <h2 className="mt-10 text-xl font-semibold">What it does, in one paragraph</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        You hand it conversation messages. It (a) asks an LLM to extract durable facts and
        entities, (b) decides — fact by fact — whether each should be added, used to update
        an existing memory, used to delete a now-obsolete one, or skipped because we
        already know it. Writes go into a per-tenant <code>mem.memories</code> table (with
        pgvector embeddings + entity links + audit history). Reads fuse vector cosine,
        BM25 keyword search and entity-link boost into a single ranked list.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Data layout</h2>
      <ul className="mt-3 list-disc space-y-1 pl-5 text-sm">
        <li><code>mem.memories</code> — fact text + vector(N) embedding + owner triple + metadata</li>
        <li><code>mem.memory_history</code> — append-only ADD/UPDATE/DELETE audit log</li>
        <li><code>mem.entities</code> — extracted entities with <code>linked_memory_ids[]</code></li>
        <li><code>mem.session_messages</code> — rolling N-message window per conversation</li>
      </ul>

      <h2 className="mt-10 text-xl font-semibold">Retrieval pipeline</h2>
      <ol className="mt-3 list-decimal space-y-1 pl-5 text-sm">
        <li>Embed query (cached in-process via Caffeine)</li>
        <li>Vector search top-K with cosine distance (pgvector HNSW index)</li>
        <li>BM25 keyword search using <code>ts_rank_cd</code> against a GIN index</li>
        <li>Entity boost: extract query entities, look them up in <code>mem.entities</code>, distribute spread-attenuated boost to linked memories</li>
        <li>Fuse: <code>combined = (sim + bm25_norm + entity_boost) / max_possible</code></li>
        <li>Return top-K ranked</li>
      </ol>

      <h2 className="mt-10 text-xl font-semibold">Authorization</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Every mem endpoint goes through <code>MemoryAuthScope</code>:
      </p>
      <ul className="mt-2 list-disc space-y-1 pl-5 text-sm">
        <li><strong>service_role apikey</strong> — sees the whole tenant; can pin any userId</li>
        <li><strong>Authenticated user</strong> — userId is forced from the JWT <code>sub</code>; if the body asks for a different userId, 403</li>
        <li><strong>Anonymous</strong> (apikey only, no Bearer) — refused</li>
      </ul>
      <p className="mt-2 text-sm text-muted-foreground">
        Defense in depth: the repository layer also takes the owner into the WHERE clause
        (<code>findByIdForOwner</code>), so even if the service layer ever has a bug, the
        SQL refuses cross-owner reads.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Providers</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Chat LLM and embedding provider are independent, both configurable via{' '}
        <code>application.yml</code>:
      </p>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`nubase:
  mem:
    enabled: true
    chat-provider: openai          # openai | anthropic | generic
    embedding-provider: openai     # openai | generic
    embedding:
      model: text-embedding-3-small
      dimensions: 1536             # baked into vector(N) at tenant init
    chat:
      model: gpt-4o-mini
    search:
      default-top-k: 5
      entity-boost-enabled: true
      fts-config: simple           # english / zhparser for Chinese
`}
      </pre>
      <p className="mt-2 text-sm text-muted-foreground">
        The <code>generic</code> provider talks any OpenAI-compatible API: DashScope,
        DeepSeek, Moonshot, vLLM, Ollama. Anthropic uses the Claude Messages API and
        translates the system message + JSON mode automatically.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Operations</h2>
      <ul className="mt-3 list-disc space-y-1 pl-5 text-sm">
        <li>
          <strong>Migrate existing tenants:</strong>{' '}
          <code>POST /auth/v1/admin/init/mem-schema?dbKey=&lt;tenant&gt;</code> — idempotent,
          backfills the <code>mem.*</code> schema and grants on tenants that predate the
          memory module.
        </li>
        <li>
          <strong>Reset:</strong> <code>POST /mem/v1/reset</code> (service_role) truncates
          every <code>mem.*</code> table for the current tenant. The Studio Settings page
          gates this behind a type-the-project-name confirmation.
        </li>
        <li>
          <strong>Disable globally:</strong> Set <code>nubase.mem.enabled=false</code> —
          API returns 404, new tenants skip mem schema entirely, migrate endpoint refuses.
        </li>
        <li>
          <strong>pgvector required:</strong> when <code>enabled=true</code> the tenant
          init fail-fasts if pgvector isn&apos;t available on the Postgres server.
          Recommended image: <code>pgvector/pgvector:pg15</code>.
        </li>
      </ul>

      <div className="mt-10 flex gap-3">
        <Link
          href="/docs/memory/quickstart"
          className="rounded-md border border-border bg-card px-4 py-2 text-sm hover:border-foreground/20"
        >
          Memory Quickstart →
        </Link>
        <Link
          href="/docs/concepts"
          className="rounded-md border border-border bg-card px-4 py-2 text-sm hover:border-foreground/20"
        >
          The eight modules
        </Link>
      </div>
    </div>
  );
}
