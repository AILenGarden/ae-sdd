import Link from 'next/link';

export default function DocsIndex() {
  return (
    <div>
      <h1 className="text-3xl font-semibold tracking-tight">Nubase docs</h1>
      <p className="mt-3 text-muted-foreground">
        Nubase turns AI-written code into real apps — eight modules
        (<strong>Database · Auth · Storage · Assets · Functions · AI&nbsp;Gateway · Memory · cron</strong>)
        under one self-hostable backend and one Studio.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Where to start</h2>
      <ul className="mt-3 space-y-2 text-sm">
        <li>
          <Link href="/docs/concepts" className="font-medium underline">The eight modules</Link>
          {' '}— how data, auth, the deploy layer (Assets · Functions · cron), AI Gateway and Memory share one auth model and one tenant DB.
        </li>
        <li>
          <Link href="/docs/getting-started" className="font-medium underline">Quickstart</Link>
          {' '}— spin up a backend, create a project, write your first memory and table in ~3 min.
        </li>
        <li>
          <Link href="/docs/memory" className="font-medium underline">Memory guide</Link>
          {' '}— what the LLM memory layer does, how it scores, and the curl recipes.
        </li>
      </ul>

      <h2 className="mt-10 text-xl font-semibold">Core modules</h2>
      <div className="mt-3 grid gap-3 sm:grid-cols-2">
        <DocCard
          title="Memory"
          href="/docs/memory"
          body="mem0-compatible REST API, vector + BM25 + entity boost fusion retrieval, audit history, pluggable providers (OpenAI / Anthropic / OpenAI-compatible)."
        />
        <DocCard
          title="Database"
          href="/docs/database"
          body="Dedicated Postgres per tenant. PostgREST-compatible REST API for every table. Row-Level Security executed with JWT claims."
        />
        <DocCard
          title="Storage"
          href="/docs/storage"
          body="S3 / R2-compatible buckets, file metadata in Postgres, signed URLs, RLS-aware ACLs."
        />
        <DocCard
          title="Auth"
          href="/docs/auth"
          body="Email + OAuth, JWT issuance with per-tenant secrets, refresh-token rotation. Drop-in for Supabase client SDKs."
        />
      </div>
      <p className="mt-3 text-sm text-muted-foreground">
        Plus the deploy layer and more —{' '}
        <Link href="/docs/concepts" className="underline">Assets, Functions, AI Gateway and cron</Link>.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Mental model</h2>
      <ul className="mt-3 list-disc space-y-1 pl-5 text-sm">
        <li>
          Every request carries an <code>apikey</code> header — a JWT identifying the
          tenant and the role (<code>anon</code> / <code>authenticated</code> / <code>service_role</code>).
        </li>
        <li>
          A second <code>Authorization: Bearer &lt;jwt&gt;</code> identifies the end user
          (used for <code>auth.uid()</code> in RLS and for Memory&apos;s userId binding).
        </li>
        <li>
          Each tenant lives in its own physical Postgres database. Memory, auth tables,
          storage metadata and your business tables all live there together.
        </li>
      </ul>
    </div>
  );
}

function DocCard({ title, href, body }: { title: string; href: string; body: string }) {
  return (
    <Link
      href={href}
      className="block rounded-lg border border-border bg-card p-4 transition-colors hover:border-foreground/20"
    >
      <h3 className="text-base font-semibold">{title}</h3>
      <p className="mt-1 text-sm text-muted-foreground">{body}</p>
    </Link>
  );
}
