# Nubase Database Reference

Use this reference when designing or changing Nubase database tables, RLS policies, SQL, PostgREST-style `/rest/v1` APIs, schema inspection, migrations, SQL dry-runs, or generated app data access.

## Tools

- `fetch_docs({ "topic": "database" })`
- `db_export_schema({ schema?, tables?, includeDrop? })` — export table DDL to inspect structure (read-only; `schema` defaults to `public`)
- `db_list_migrations({ limit? })` — read the audit trail of schema changes applied through `sql_execute` (read-only)
- `rest_select(table, query?)`
- `sql_dry_run(sql)`
- `sql_execute(sql)`

## Database Workflow

1. Inspect existing schema before changing it — call `db_export_schema` (default schema `public`, or pass `tables` to narrow). Treat its DDL as the source of truth for what already exists.
2. Prefer additive changes.
3. Add primary keys and timestamps where appropriate.
4. Add indexes for common filters.
5. Add RLS policies for user-owned data.
6. Call `sql_dry_run`.
7. Execute only when the user requested implementation or gave approval.
8. Store durable schema decisions with `memory_write`.

## Schema Change Audit Trail

Every successful `sql_execute` that changes schema (risk `SCHEMA_WRITE` or `DANGEROUS`) is recorded to an append-only `nubase.migrations` table (timestamp, risk, statement count, the SQL text, and `agent_id`/`run_id`/`user_id` when set). Pure reads and data writes are not recorded.

- Review history with `db_list_migrations({ limit? })` — most recent first. Use it to see what changed, when, and by which agent, or to replay the SQL elsewhere.
- The successful `sql_execute` result includes `migrationRecorded: true`; if recording fails it returns `migrationRecorded: false` with `migrationError` but the DDL itself still succeeded.
- Disable the trail with `NUBASE_RECORD_MIGRATIONS=false`.

This is a lightweight audit/replay log, not a full migration engine — there is no diffing, versioning of files, or rollback.

## Worked Example: a `todos` table end to end

This is the full path from empty schema to a working, user-scoped table.

**1. Inspect what exists** (don't guess):

```text
nubase_overview()            # tables, buckets, users, gateway keys, permissions in one call
# or, just the schema:
db_export_schema({ "schema": "public" })
```

**2. Draft the DDL** — primary key, owner column, timestamps, an index, and RLS:

```sql
create table public.todos (
  id          uuid primary key default gen_random_uuid(),
  user_id     uuid not null references auth.users (id) on delete cascade,
  title       text not null,
  done        boolean not null default false,
  created_at  timestamptz not null default now()
);
create index todos_user_id_idx on public.todos (user_id);

alter table public.todos enable row level security;

create policy "owner can read"   on public.todos for select using (auth.uid() = user_id);
create policy "owner can insert" on public.todos for insert with check (auth.uid() = user_id);
create policy "owner can modify" on public.todos for update using (auth.uid() = user_id);
create policy "owner can delete" on public.todos for delete using (auth.uid() = user_id);
```

**3. Classify before running:**

```text
sql_dry_run({ "sql": "<the DDL above>" })
# -> { "success": true, "risk": "SCHEMA_WRITE", "statementCount": 6, "executable": true }
```

**4. Execute** (only after the user asked you to implement; needs `NUBASE_ALLOW_SQL_EXECUTE=true`):

```text
sql_execute({ "sql": "<the DDL above>" })
```

**5. Read and write through `/rest/v1` from app code** — RLS scopes rows to the JWT's user automatically:

```http
POST /rest/v1/todos
apikey: <anon or authenticated key>
Authorization: Bearer <user JWT>
Content-Type: application/json
Prefer: return=representation

{ "title": "Buy milk" }
```

Response:

```json
[{ "id": "9b2...c1", "user_id": "f3a...07", "title": "Buy milk", "done": false, "created_at": "2026-06-03T10:00:00Z" }]
```

```http
GET   /rest/v1/todos?select=*&done=eq.false&order=created_at.desc
PATCH /rest/v1/todos?id=eq.9b2...c1      Body: { "done": true }
DELETE /rest/v1/todos?id=eq.9b2...c1
```

`user_id` is omitted on insert on purpose — set it server-side via a column default or a trigger, or let the app pass it; never trust a client-supplied owner id.

## REST API Patterns

Use `/rest/v1` for generated app data access:

```http
GET /rest/v1/todos?select=*
POST /rest/v1/todos
PATCH /rest/v1/todos?id=eq.<id>
DELETE /rest/v1/todos?id=eq.<id>
```

Headers:

```http
apikey: <anon or authenticated key>
Authorization: Bearer <user JWT>
```

Use service_role only in trusted server-side or local agent contexts.

## SQL Risk Handling

- `READ`: safe to run when needed.
- `DATA_WRITE`: ensure the user requested data mutation.
- `SCHEMA_WRITE`: proceed only for implementation tasks or after confirmation.
- `DANGEROUS`: ask for explicit confirmation and require environment permission.
- `UNKNOWN`: inspect manually before execution.

Destructive operations requiring confirmation:

- `drop`
- `truncate`
- bulk `delete`
- disabling RLS
- deleting schemas
- resetting Memory

> `/rest/v1` is PostgREST-style and Supabase-compatible for common workflows; don't assume full `supabase-js` compatibility without a test.
