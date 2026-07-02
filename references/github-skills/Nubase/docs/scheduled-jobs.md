# Scheduled Jobs (Cron)

Supabase-Cron-style recurring jobs, run by the Nubase control plane. One scheduler
supports two target types:

- **`edge_function`** — invokes an edge function through the same path as gateway
  traffic, so scheduled calls get rate limits, secrets and invocation logging for
  free. Jobs run with `callerRole=cron`, which bypasses `verify_jwt` (they are
  platform-initiated, like Supabase's pg_cron calling functions with the service key).
- **`db_function`** — calls a named Postgres function in the tenant schema via the
  PostgREST RPC engine, with a per-job statement timeout enforced server-side.

Unlike Supabase (which runs pg_cron inside each tenant's dedicated database), Nubase
tenants share clusters with schema isolation, so scheduling lives in the control
plane: jobs are stored in the metadata DB and claimed with a row-level
compare-and-set (`next_run_at` + `locked_until`), which makes any number of Nubase
instances safe to run concurrently without an external lock service. A job still
running when its next occurrence comes due is not re-entered; missed occurrences
coalesce into one delayed run. A claim that waits in the execution queue past its
own lock window is not executed either — it is recorded in run history as
`skipped` (`QUEUE_WAIT_EXCEEDED_LOCK`) and the occurrence passes to whichever
instance holds the current lock, so a job never overlaps with itself.

## Why "call one named function" instead of arbitrary SQL

Supabase Cron accepts raw SQL snippets because each tenant owns its database. On a
shared cluster, Nubase restricts db_function jobs to calling a single named function
with JSON args — same expressive power (`CREATE FUNCTION` first), but the side
effect has a name, a validated surface, and a timeout.

## Admin API

All endpoints require the project's `service_role` apikey.

```
GET    /cron/admin/v1/jobs
POST   /cron/admin/v1/jobs
GET    /cron/admin/v1/jobs/{name}
PATCH  /cron/admin/v1/jobs/{name}
DELETE /cron/admin/v1/jobs/{name}
GET    /cron/admin/v1/jobs/{name}/runs?limit=50
GET    /cron/admin/v1/runs?limit=50
```

Create a nightly DB maintenance job (5-field crontab and Spring's 6-field form are
both accepted; schedules evaluate in UTC):

```json
POST /cron/admin/v1/jobs
{
  "name": "refresh-stats",
  "cronExpression": "30 3 * * *",
  "targetType": "db_function",
  "dbFunctionName": "refresh_stats",
  "dbFunctionArgs": { "days": 7 },
  "timeoutSeconds": 120
}
```

Invoke an edge function every five minutes:

```json
POST /cron/admin/v1/jobs
{
  "name": "poll-upstream",
  "cronExpression": "*/5 * * * *",
  "targetType": "edge_function",
  "functionSlug": "poll-upstream",
  "httpMethod": "POST",
  "requestBody": "{\"source\":\"cron\"}"
}
```

Run history (status, duration, result snippet, error) is kept per job in
`scheduled_job_runs` and pruned on a retention schedule.

## Configuration

```yaml
nubase:
  cron:
    enabled: ${NUBASE_CRON_ENABLED:true}           # kill switch for the whole module
    tick-ms: ${NUBASE_CRON_TICK_MS:30000}          # due-job scan interval
    max-jobs-per-tick: ${NUBASE_CRON_MAX_JOBS_PER_TICK:50}
    default-timeout-seconds: ${NUBASE_CRON_DEFAULT_TIMEOUT_SECONDS:60}
    max-timeout-seconds: ${NUBASE_CRON_MAX_TIMEOUT_SECONDS:600}
    run-history-retention-days: ${NUBASE_CRON_RUN_RETENTION_DAYS:30}
```

## Semantics and limitations

- Minimum effective resolution is the tick interval (30s by default); sub-minute
  6-field schedules fire on the next tick after they come due.
- Edge-function jobs require the functions module (`nubase.functions.enabled`);
  with it disabled, runs fail with `FUNCTIONS_DISABLED` rather than silently no-op.
- Disabling a job keeps its schedule definition; re-enabling re-anchors
  `next_run_at` to the future (no catch-up storm for the paused period).
