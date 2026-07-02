import Link from 'next/link';

export default function Quickstart() {
  return (
    <div>
      <h1 className="text-3xl font-semibold tracking-tight">Quickstart</h1>
      <p className="mt-3 text-muted-foreground">
        Get a project running locally in about three minutes — then try a memory write.
      </p>

      <h2 className="mt-8 text-xl font-semibold">1. Start the backend</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Requires Postgres 15 with the <code>pgvector</code> extension installed (use the{' '}
        <code>pgvector/pgvector:pg15</code> Docker image), and Redis for caching.
      </p>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`git clone https://github.com/your-org/nubase
cd nubase

# Optional: set the LLM key used by the Memory module
export OPENAI_API_KEY="sk-..."

mvn spring-boot:run`}
      </pre>

      <h2 className="mt-8 text-xl font-semibold">2. Open the Studio</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Visit <code>http://localhost:3000</code>, sign up as a platform user, and create
        your first project. Provisioning creates a dedicated Postgres database for the
        tenant with auth / storage / mem schemas and a service-role apikey.
      </p>

      <h2 className="mt-8 text-xl font-semibold">3. Make a table</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Open <em>SQL Editor</em> in Studio (or use psql) and create a business table:
      </p>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`create table public.todos (
  id bigserial primary key,
  text text not null,
  done bool default false
);`}
      </pre>

      <h2 className="mt-8 text-xl font-semibold">4. Hit the REST API</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Every table gets a PostgREST-compatible endpoint automatically. Use the project&apos;s
        <code> apikey</code> from Studio Settings:
      </p>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`curl http://localhost:9999/rest/v1/todos \\
  -H "apikey: $NUBASE_ANON_KEY"`}
      </pre>

      <h2 className="mt-8 text-xl font-semibold">5. Write your first memory</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        The Memory module turns conversation messages into durable, queryable facts. The
        LLM extracts facts and decides ADD / UPDATE / DELETE / NONE per fact.
      </p>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`curl -X POST http://localhost:9999/mem/v1/memories \\
  -H "apikey: $NUBASE_SERVICE_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{
    "userId": "user-42",
    "messages": [{
      "role": "user",
      "content": "I prefer steak over sushi, and my dog is named Mochi."
    }]
  }'

curl -X POST http://localhost:9999/mem/v1/search \\
  -H "apikey: $NUBASE_SERVICE_KEY" \\
  -d '{ "userId": "user-42", "query": "what do they eat?" }'`}
      </pre>

      <h2 className="mt-8 text-xl font-semibold">6. Manage it in Studio</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Studio&apos;s <strong>Memory</strong> tab gives you a list view with stats, search,
        per-memory history (ADD/UPDATE/DELETE), an entity browser, and a Settings page that
        shows the live LLM/embedding config plus danger-zone actions (migrate, reset).
      </p>

      <h2 className="mt-8 text-xl font-semibold">Where next</h2>
      <ul className="mt-3 list-disc space-y-1 pl-5 text-sm">
        <li>
          <Link href="/docs/concepts" className="underline">The eight modules</Link>
          {' '}— how data, auth, the deploy layer (Assets · Functions · cron), AI Gateway and Memory share one auth model and one tenant DB.
        </li>
        <li>
          <Link href="/docs/memory/quickstart" className="underline">Memory deep-dive</Link>
          {' '}— update / history / entity inspection from curl.
        </li>
      </ul>
    </div>
  );
}
