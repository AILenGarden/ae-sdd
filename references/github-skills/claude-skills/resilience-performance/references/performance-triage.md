# Performance Triage Workflow

Step-by-step diagnosis for a Spring service that is slow, erroring, or saturated in a multi-pod
deployment. Follow in order — each step either ends the investigation or narrows it. Resist the
urge to jump to a fix (bigger pool, more pods, more memory) before step 6.

## Step 0 — Scope the blast radius

Before any metric, answer:

- **One pod or all pods?** One pod = local cause (bad node, GC, stuck thread, hot partition assigned to it). All pods = shared cause (downstream, DB, traffic shape, deploy).
- **One endpoint or all endpoints?** One endpoint = code path (query, remote call). All = shared resource (pool, GC, CPU throttling).
- **Started when?** Correlate with deploys, config changes, traffic changes, downstream incidents. "Slow since 14:32" + "payment-service deployed 14:30" ends most investigations.

## Step 1 — RED metrics (the service as its callers see it)

In Grafana/Prometheus, per endpoint (`uri` tag) and per pod (`instance`/`pod` tag):

| Signal | PromQL sketch | Read it as |
|---|---|---|
| **R**ate | `sum(rate(http_server_requests_seconds_count[5m])) by (uri)` | Traffic shift? Retry amplification shows here first |
| **E**rrors | `... {status=~"5.."}` over total | Which endpoints, which pods |
| **D**uration | `histogram_quantile(0.99, sum(rate(http_server_requests_seconds_bucket[5m])) by (le, uri))` | p99 vs p50 divergence = queueing or a slow subset |

Patterns:

- **p99 up, p50 flat** → a subset of requests waits on something: pool acquisition, a slow downstream, GC pauses. Go to steps 3–5.
- **p50 and p99 both up** → systemic: CPU saturation/throttling, GC, or a downstream on the critical path of everything. Go to step 2.
- **Errors without latency** → fast failures: open breaker, connection refused, rejected from a bounded queue. Check breaker state and rejection metrics.
- **Rate spike + downstream errors** → suspect retry storm (yours or your callers'). Check `resilience4j_retry_calls_total`.

## Step 2 — USE metrics (the resources)

For each resource — CPU, memory, pool, threads — check **U**tilization, **S**aturation, **E**rrors:

| Resource | Utilization | Saturation | Errors |
|---|---|---|---|
| CPU | `process_cpu_usage`, container CPU | **K8s throttling**: `container_cpu_cfs_throttled_periods_total` | — |
| JVM heap | `jvm_memory_used_bytes{area="heap"}` | GC time (step 5) | `OutOfMemoryError` in logs |
| Hikari pool | `hikaricp_connections_active` / `_max` | `hikaricp_connections_pending` | `hikaricp_connections_timeout_total` |
| Tomcat threads | `tomcat_threads_busy_threads` / `_config_max` | busy == max sustained | 503s |
| Kafka consumer | — | `kafka_consumer_fetch_manager_records_lag` | rebalance count |

CPU throttling deserves special mention: a pod at 60% "usage" can still be throttled hard if usage
is spiky and the CPU **limit** is low — manifests as mysterious p99 latency with healthy-looking
dashboards. Check throttled periods before blaming code.

## Step 3 — Hikari pool starvation

Symptom: `Connection is not available, request timed out after 30000ms` /
`unable to acquire JDBC Connection`, `hikaricp_connections_pending > 0`.

Diagnose **hold time**, not pool size:

1. `hikaricp_connections_usage_seconds` (histogram) — how long connections are held. Healthy: milliseconds. If p99 is seconds, find who holds them.
2. Enable Hikari leak detection in a non-prod or temporarily: `spring.datasource.hikari.leak-detection-threshold: 10000` — logs the stack trace of any thread holding a connection > 10s.
3. Usual suspects, in order of frequency:
   - `@Transactional` method that makes a **remote call inside the transaction** (HTTP/Kafka send) — connection held for the remote call's full duration. Move the call outside the transaction.
   - Long queries — fix the query (route to jpa-database-patterns), add statement timeout.
   - `@Transactional` on a method far up the call stack covering far more work than the DB writes need.
4. Only after hold time is fixed: size pool ≈ `cores × 2` per pod, and check the **sum across pods** against PostgreSQL `max_connections` (and connection-per-backend memory cost). 10 pods × 50 = 500 backends is an outage waiting for traffic.

## Step 4 — Thread dumps (where are the threads stuck?)

Capture 3 dumps ~10s apart (a single dump lies — you want what's *consistently* stuck):

```bash
kubectl exec <pod> -- jstack <pid> > dump-$(date +%s).txt          # or:
curl -s localhost:8080/actuator/threaddump > dump-$(date +%s).txt
```

What to look for:

| Pattern in dump | Meaning |
|---|---|
| Many threads in `HikariPool.getConnection` / parked on pool | Pool starvation — back to step 3 |
| Many threads in `SocketInputStream.read` / `Http11...` on the same remote host | Missing/too-long read timeout on that client |
| `http-nio-*` threads mostly parked in pool, few RUNNABLE | Server idle but slow → waiting on a dependency, not CPU |
| All `http-nio-*` RUNNABLE in app code | CPU-bound — profile (async-profiler) instead of reading dumps |
| `BLOCKED` on the same monitor across dumps | Lock contention — note the owning thread, find the synchronized block |
| Threads on an unbounded queue's `take()` growing in count | Executor leak / queue backlog |

## Step 5 — GC

```text
jvm_gc_pause_seconds (Micrometer) — max and sum per minute
```

- Pause p99 > 100ms or GC time > 5% of wall clock → GC is a latency contributor.
- Sawtooth heap that recovers = healthy. Baseline creeping up across collections = leak → heap dump (`jcmd <pid> GC.heap_dump`), analyze dominators.
- Frequent full GCs at high old-gen occupancy → undersized heap or a cache without bounds (check `@Cacheable` without TTL/maximum size first — it is cheaper than more memory).
- Default collector G1 is right for most services; do not hand-tune flags as a first response.

## Step 6 — Downstream attribution

If steps 2–5 are clean, the time is being spent elsewhere:

- Client metrics: `http_client_requests_seconds` by `client.name`/`uri` — which dependency's p99 moved?
- Traces (if propagated — see references/observability.md): one slow exemplar trace usually names the culprit span outright.
- Compare against the dependency's own RED dashboard. If their p99 moved at the same timestamp, hand off with evidence: timestamps, traffic delta, trace IDs.

## Anti-patterns in triage

- **Raising pool sizes/timeouts/replicas as a first move** — treats symptoms, usually shifts saturation to the next bottleneck (often the database, which is the most expensive place to be saturated).
- **Restarting pods before capturing a thread dump/heap dump** — destroys the evidence; capture first, restart second.
- **Reading averages** — `_sum/_count` hides bimodal latency. Use histograms and percentiles.
- **Trusting a single pod's metrics** — always compare across pods to separate local from systemic.
