# Observability Conventions

Micrometer + Prometheus/Grafana instrumentation, structured logging, and trace propagation for
Spring Boot 3.x services. The goal: when an incident starts, the data to triage it already exists.

## What to instrument (and what not to)

Spring Boot auto-instruments the important 80%: `http.server.requests`, `http.client.requests`
(when clients come from auto-configured builders — another reason never to `new RestTemplate()`),
Hikari, JVM, Kafka clients, caches (`cache.gets`, `cache.puts` — requires the cache to be registered).

Add custom metrics only for:

- **Business-critical operations** with no HTTP/Kafka boundary: batch jobs, scheduled reconciliation, internal queues.
- **Domain counters that pages would reference**: payments rejected, messages dead-lettered, fallback served.
- **Queue/backlog depths** for anything bounded you introduced (bulkheads and executors expose theirs if registered).

Do **not** duplicate what auto-instrumentation provides, and do not time trivial in-memory operations.

## Naming and tag conventions

| Rule | Example |
|---|---|
| Names: lowercase, dot-separated, unit as suffix only via base unit (Micrometer appends) | `orders.placed`, `reconciliation.duration` |
| Name = what is measured; tags = how it's sliced | `payments.processed` + tag `outcome=approved\|declined` — not `payments.approved` and `payments.declined` |
| Tags MUST be low-cardinality | `outcome`, `client.name`, `topic` — never user IDs, order IDs, full URIs with path variables expanded |
| Every timer that feeds an SLO gets percentile histograms | see config below |

Cardinality is the production killer: one tag with unbounded values (an ID, a raw path) multiplies
series count until Prometheus or your bill falls over. If you need per-entity detail, that's a log
or a trace, not a metric.

```yaml
management:
  metrics:
    distribution:
      percentiles-histogram:
        http.server.requests: true
        http.client.requests: true
    tags:
      application: ${spring.application.name}   # common tag on every metric
```

## Custom metrics: idioms

```java
@Service
public class ReconciliationService {

    private final Counter mismatches;
    private final Timer runTimer;

    public ReconciliationService(MeterRegistry registry) {
        this.mismatches = Counter.builder("reconciliation.mismatches")
                .tag("source", "ledger")
                .register(registry);
        this.runTimer = Timer.builder("reconciliation.duration")
                .publishPercentileHistogram()
                .register(registry);
    }

    public void runNightly() {
        runTimer.record(() -> {
            // ... compare, count
            mismatches.increment(found);
        });
    }
}
```

For one-off method timing, `@Observed` (with `ObservedAspect` bean) or `@Timed` is fine; prefer
`@Observed` on Boot 3.x — it produces a metric **and** a trace span from one annotation.

## Correlation IDs and MDC

Every log line in a request's lifetime must carry the same correlation ID, across services.

With Micrometer Tracing on the classpath, `traceId`/`spanId` are put in the MDC automatically and
the W3C `traceparent` header propagates them across HTTP and Kafka. Prefer that over a hand-rolled
ID. If you must support a legacy `X-Correlation-Id` as well:

```java
@Component
public class CorrelationIdFilter extends OncePerRequestFilter {

    public static final String HEADER = "X-Correlation-Id";

    @Override
    protected void doFilterInternal(HttpServletRequest request, HttpServletResponse response,
                                    FilterChain chain) throws ServletException, IOException {
        String id = Optional.ofNullable(request.getHeader(HEADER))
                .orElse(UUID.randomUUID().toString());
        MDC.put("correlationId", id);
        response.setHeader(HEADER, id);
        try {
            chain.doFilter(request, response);
        } finally {
            MDC.remove("correlationId");      // ALWAYS — Tomcat reuses threads; leaked MDC lies
        }
    }
}
```

The `finally` block is not optional: servlet threads are pooled, and stale MDC values attribute
one user's logs to another's request — the worst kind of debugging trap.

## Structured logging

JSON logs, one event per line, so Loki/Elastic can index fields instead of regexing strings.
Spring Boot 3.4+ has it built in:

```yaml
logging:
  structured:
    format:
      console: ecs        # or logstash; MDC fields (traceId, correlationId) included automatically
```

Rules:

- Log **events with fields**, not prose: `log.info("payment.declined reason={} amount={}", reason, amount)` — greppable, parseable.
- One `ERROR` per failure, at the place that handles it. Log-and-rethrow at every layer turns one incident into five stack traces and breaks error-rate alerts.
- `WARN` = degraded but handled (fallback served, retry succeeded). `ERROR` = someone may need to act.
- Never log payloads with PII/credentials; log IDs and outcomes.

## Trace propagation

Dependencies (Boot 3.x): `io.micrometer:micrometer-tracing-bridge-otel` +
`io.opentelemetry:opentelemetry-exporter-otlp`.

```yaml
management:
  tracing:
    sampling:
      probability: 0.1    # 10%; raise temporarily during investigations
```

Propagation is automatic for: auto-configured RestClient/WebClient builders, `@KafkaListener`/
`KafkaTemplate` (with `observation-enabled: true` on the template/container), scheduled tasks via
`@Observed`.

Propagation **breaks** when work hops threads manually. Fix with context-propagation:

```java
// ExecutorService that carries trace context + MDC into the pool:
@Bean
ExecutorService downstreamExecutor() {
    return ContextExecutorService.wrap(
            new ThreadPoolExecutor(4, 8, 60, TimeUnit.SECONDS, new ArrayBlockingQueue<>(50)),
            ContextSnapshotFactory.builder().build()::captureAll);
}
```

Symptoms of broken propagation: traces that end at your service boundary, log lines mid-request
with empty `traceId`, child spans appearing as new root traces. Grep for raw `new Thread(`,
`CompletableFuture.supplyAsync(` without an executor, and unwrapped executors when you see these.

## Dashboard minimum per service

1. RED: rate, error rate, p50/p95/p99 per endpoint (from `http_server_requests_seconds`).
2. Dependency panel: `http_client_requests_seconds` p99 by `client.name`; breaker states.
3. Saturation: Hikari active/pending, Tomcat busy threads, CPU + throttling, GC pause time.
4. Domain counters: the 2–3 business metrics that define "this service is doing its job".
