import Link from 'next/link';

export default function MemoryQuickstartPage() {
  return (
    <div>
      <h1 className="text-3xl font-semibold tracking-tight">Memory quickstart</h1>
      <p className="mt-3 text-muted-foreground">
        End-to-end: write a memory, search it, inspect the entity, replay history. Assumes
        the backend is up and you have a service-role <code>apikey</code> for one tenant.
      </p>

      <h2 className="mt-10 text-xl font-semibold">0. Prerequisites</h2>
      <ul className="mt-3 list-disc space-y-1 pl-5 text-sm">
        <li>Nubase backend running on <code>:9999</code></li>
        <li>Tenant provisioned via <code>POST /auth/v1/admin/init/database</code></li>
        <li><code>OPENAI_API_KEY</code> (or Anthropic / DashScope key) set in the backend env</li>
        <li>An export <code>NUBASE_SERVICE_KEY=&quot;eyJ…&quot;</code> with the project&apos;s service_role JWT</li>
      </ul>

      <h2 className="mt-10 text-xl font-semibold">1. Write a memory</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Pass conversation messages. The LLM extracts facts and the entities they reference
        in one call.
      </p>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`curl -X POST http://localhost:9999/mem/v1/memories \\
  -H "apikey: $NUBASE_SERVICE_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{
    "userId": "user-42",
    "messages": [
      { "role": "user",
        "content": "Hi, I am John. I work at Anthropic and my dog is named Mochi." }
    ]
  }'
`}
      </pre>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`{
  "results": [
    { "id": "…", "event": "ADD", "memory": "Name is John" },
    { "id": "…", "event": "ADD", "memory": "Works at Anthropic" },
    { "id": "…", "event": "ADD", "memory": "Has a dog named Mochi" }
  ]
}`}
      </pre>

      <h2 className="mt-10 text-xl font-semibold">2. Search</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Multi-signal: vector cosine, BM25 keyword, and entity boost (the LLM extracts
        entities from the query if <code>entity-boost-enabled</code>).
      </p>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`curl -X POST http://localhost:9999/mem/v1/search \\
  -H "apikey: $NUBASE_SERVICE_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{
    "userId": "user-42",
    "query": "tell me about Mochi",
    "topK": 3
  }'
`}
      </pre>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`{
  "results": [
    { "id": "…", "memory": "Has a dog named Mochi", "score": 0.94 },
    { "id": "…", "memory": "Name is John",          "score": 0.42 }
  ]
}`}
      </pre>

      <h2 className="mt-10 text-xl font-semibold">3. Update — let the LLM decide</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Send a contradicting fact; the decision LLM emits <code>UPDATE</code> with the old
        memory&apos;s id.
      </p>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`curl -X POST http://localhost:9999/mem/v1/memories \\
  -H "apikey: $NUBASE_SERVICE_KEY" \\
  -d '{
    "userId": "user-42",
    "messages": [{ "role": "user",
                   "content": "I actually moved to OpenAI last month." }]
  }'
# → {"results":[{"id":"<orig>","event":"UPDATE",
#                "memory":"Works at OpenAI",
#                "previousMemory":"Works at Anthropic"}]}
`}
      </pre>

      <h2 className="mt-10 text-xl font-semibold">4. Inspect history</h2>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`curl http://localhost:9999/mem/v1/memories/<id>/history \\
  -H "apikey: $NUBASE_SERVICE_KEY"
# → [
#    {"event":"ADD",    "newValue":"Works at Anthropic", "createdAt":"…"},
#    {"event":"UPDATE", "oldValue":"Works at Anthropic",
#                       "newValue":"Works at OpenAI",    "createdAt":"…"}
#   ]
`}
      </pre>

      <h2 className="mt-10 text-xl font-semibold">5. Browse entities</h2>
      <pre className="my-3 overflow-auto rounded-md border border-border bg-card p-4 text-xs">
{`curl "http://localhost:9999/mem/v1/entities?userId=user-42&type=person" \\
  -H "apikey: $NUBASE_SERVICE_KEY"
# → { "items":[
#       {"text":"John", "entityType":"person",
#        "linkedMemoryIds":["…"]}
#     ], "total":1, "page":1, "pageSize":25 }
`}
      </pre>

      <h2 className="mt-10 text-xl font-semibold">6. Or use the Studio UI</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Everything above is available in <Link href="http://localhost:3000/projects" className="underline">Studio</Link> under
        the <strong>Memory</strong> tab: list with stats, search, edit, history, entities,
        settings. No curl required for day-to-day inspection.
      </p>

      <h2 className="mt-10 text-xl font-semibold">From your app</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        Use the same two-layer auth model as the rest of nubase:
      </p>
      <ul className="mt-3 list-disc space-y-1 pl-5 text-sm">
        <li>
          <strong>Server-to-server (admin):</strong> send the tenant&apos;s service_role
          JWT as <code>apikey</code>. You can write/read for any userId.
        </li>
        <li>
          <strong>End user:</strong> send the tenant&apos;s anon/authenticated apikey plus
          the user&apos;s Bearer JWT. The body&apos;s <code>userId</code> is forced to
          match <code>sub</code> — there&apos;s no way for one user to read another&apos;s
          memories.
        </li>
      </ul>
    </div>
  );
}
