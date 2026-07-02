---
name: resilience-performance
description: >
  Use when a Spring microservice is slow, flaky, or failing under load:
  missing timeouts on remote calls, retry storms, cascading failures across
  services, circuit breaker tuning, HikariCP pool starvation ("unable to
  acquire JDBC Connection", "Connection is not available, request timed
  out"), thread pool exhaustion, p99 latency spikes, cache stampedes,
  dropped requests during rolling deploys, or readiness/liveness probe
  confusion. Covers timeouts (RestClient, WebClient, Feign, Kafka, JDBC,
  Redis), retries with exponential backoff and jitter, Resilience4j circuit
  breakers and bulkheads, connection pool sizing, graceful degradation and
  fallbacks, backpressure, caching TTLs, graceful shutdown, and load
  shedding. Not for Kafka consumer semantics or rebalancing — use
  kafka-event-patterns. Not for slow JPA queries or N+1 — use
  jpa-database-patterns. Not for general service structure — use
  spring-boot-standards.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---

# Resilience and Performance for Distributed Spring Services

## When to use

- A downstream service degraded and the failure cascaded across multiple pods or services
- Logs show `Connection is not available, request timed out after 30000ms` or `unable to acquire JDBC Connection`
- p99 latency spikes while p50 looks fine, or throughput collapses under moderate load
- A retry "fix" made an outage worse (retry storm), or duplicate side effects appeared after retries
- Pods serve errors during rolling deploys, or Kubernetes restarts healthy pods
- Reviewing or writing any code that calls another service, a database, a cache, or a broker

## Quick reference

| Problem | Symptom | Solution |
|---|---|---|
| Missing remote-call timeout | Threads stuck for minutes, pool exhaustion, hung requests | Explicit connect + read timeout on every client (see checklist) |
| Retry storm | Downstream outage amplified, traffic spikes during incidents | Exponential backoff + jitter, retry idempotent ops only, cap attempts at 3 |
| Cascading failure | One slow dependency takes down callers | Circuit breaker per downstream + fallback |
| Pool starvation | `unable to acquire JDBC Connection`, `hikaricp.connections.pending > 0` | Fix long transactions first, then size pool (cores-based, small) |
| Thread exhaustion | All requests slow when one dependency is slow | Bulkhead / dedicated executor per downstream |
| Cache stampede | DB load spike when a hot key expires | TTL + single-flight loading cache (Caffeine) or lock/early-refresh |
| Deploy-time errors | 502/connection-reset during rollout | `server.shutdown: graceful` + readiness probe distinct from liveness |
| Restart loops | K8s kills pods when a downstream is down | Liveness must NOT include downstream health checks |

## MUST / MUST NOT

**MUST**

- Set an explicit connect and read timeout on every remote call: HTTP, JDBC, Kafka, Redis. No client ships with safe defaults.
- Make retries exponential with jitter, capped (max 3 attempts), and applied only to idempotent operations.
- Put a circuit breaker between this service and every synchronous downstream dependency.
- Keep total budget coherent: caller timeout > sum of (downstream timeout × retries) is a bug; budget top-down.
- Enable `server.shutdown: graceful` and separate readiness (`/actuator/health/readiness`) from liveness.
- Expose Resilience4j and Hikari metrics through Micrometer (they are the first thing you look at in an incident).

**MUST NOT**

- MUST NOT call `RestClient.create()`, `WebClient.create()`, or `new RestTemplate()` inline — no timeouts, no metrics, no breaker.
- MUST NOT retry POSTs or any non-idempotent operation without an idempotency key honored by the server.
- MUST NOT put downstream dependency checks in the liveness probe — that converts a dependency outage into a restart loop.
- MUST NOT "fix" pool starvation by raising `maximum-pool-size` — it usually moves the bottleneck to the database.
- MUST NOT use unbounded queues (`Executors.newFixedThreadPool`, `new LinkedBlockingQueue<>()`) for request-path work — they hide overload until OOM.
- MUST NOT cache without a TTL.

## Resilience checklist

Work through this table against the codebase. "Detected pattern" is what to `grep` for (or confirm is absent).

| Practice | Detected pattern (grep / look for) | Severity | Fix |
|---|---|---|---|
| HTTP client timeouts | `RestClient.create()`, `WebClient.create(`, `new RestTemplate()`; builder beans with no `ClientHttpRequestFactorySettings` / `responseTimeout` | Critical | Configure connect 1–2s, read 2–5s per client bean (example below) |
| Feign timeouts | No `spring.cloud.openfeign.client.config.*.connect-timeout`/`read-timeout` in config | Critical | Set both per client; default config block at minimum |
| JDBC/Hikari timeouts | No `spring.datasource.hikari.connection-timeout`; no statement/query timeout (`spring.jpa.properties.jakarta.persistence.query.timeout`) | High | `connection-timeout: 3000`, query timeout ≤ request budget |
| Kafka producer timeouts | No `delivery.timeout.ms`, `request.timeout.ms`, `max.block.ms` in producer props | High | `max.block.ms: 3000` (bounds `send()` blocking), `delivery.timeout.ms: 120000` deliberate, not default |
| Redis timeouts | No `spring.data.redis.timeout` | High | Set 200ms–1s; a cache slower than the DB is harmful |
| Retries idempotent-only | `@Retry`/`@Retryable` on methods doing POST/insert; `retry-exceptions` containing broad `Exception` | Critical | Retry GET/PUT/idempotent only; narrow exceptions to `IOException`/timeouts; idempotency keys for writes |
| Backoff + jitter | Retry config without `enable-exponential-backoff` + `enable-randomized-wait`; hand-rolled `for` retry loops | High | Resilience4j exponential-random backoff (see references/resilience4j-config.md) |
| Circuit breaker per downstream | Remote call sites without `@CircuitBreaker`; one shared breaker name for all dependencies | High | One named breaker per downstream; count-based window, slow-call threshold set |
| Bulkhead isolation | All async work on default executor / common `ForkJoinPool`; `@Async` without qualifier | Medium | Dedicated bounded executor or Resilience4j `Bulkhead` per downstream |
| Pool sizing | `maximum-pool-size` ≥ 30; pool size copied between services | Medium | Start at `cores × 2`; fix transaction length before resizing (references/performance-triage.md) |
| Fallback / degradation | Breaker without `fallbackMethod`; user-facing 500 on optional-dependency failure | Medium | Fallback to cached/stale/default value; degrade features, not the request |
| Backpressure | `Executors.newFixedThreadPool`, `new LinkedBlockingQueue<>()` (no capacity), unbounded `@Async` queue | High | Bounded queues + `CallerRunsPolicy` or rejection; reject early at the edge |
| Cache TTL + stampede | `@Cacheable` with no `expireAfterWrite`/Redis TTL; hot key loaded by many threads | Medium | TTL on every cache; Caffeine `LoadingCache` (single-flight) for hot keys |
| Graceful shutdown | No `server.shutdown: graceful`; no `spring.lifecycle.timeout-per-shutdown-phase` | High | Enable both; K8s `terminationGracePeriodSeconds` > shutdown phase timeout |
| Readiness vs liveness | One health URL used for both probes; liveness includes DB/downstream indicators | High | Liveness = process health only; readiness = "can take traffic" (DB, warmup) |
| Load shedding | No request limit; latency grows unbounded under overload instead of fast 429s | Medium | Resilience4j `RateLimiter`/`Bulkhead` at the edge; bounded Tomcat threads + queue |

## Core pattern: outbound call with timeout, breaker, retry, fallback

❌ **BAD** — no timeout, blind retry of a POST, no breaker:

```java
@Service
public class PaymentClient {

    private final RestClient restClient = RestClient.create(); // infinite-ish timeouts, no metrics

    public PaymentStatus charge(ChargeRequest request) {
        for (int i = 0; i < 3; i++) {                 // hand-rolled retry, no backoff, no jitter
            try {
                return restClient.post()              // POST retried -> duplicate charges
                        .uri("http://payment-service/api/v1/charges")
                        .body(request)
                        .retrieve()
                        .body(PaymentStatus.class);
            } catch (Exception e) {                   // retries on 4xx and bugs too
                // immediate retry -> retry storm during downstream outage
            }
        }
        throw new IllegalStateException("payment failed");
    }
}
```

Failure modes: a slow downstream pins this thread indefinitely → caller's pool exhausts → cascade. The retry triples traffic exactly when the downstream is dying, and duplicates charges.

✅ **GOOD** — timeouts on the client bean, breaker + fallback, retry only on the idempotent read, idempotency key on the write:

```java
@Configuration
public class PaymentClientConfig {

    @Bean
    RestClient paymentRestClient(RestClient.Builder builder) {
        var settings = ClientHttpRequestFactorySettings.defaults()
                .withConnectTimeout(Duration.ofSeconds(2))
                .withReadTimeout(Duration.ofSeconds(3));   // < caller's own budget
        return builder
                .baseUrl("http://payment-service")
                .requestFactory(ClientHttpRequestFactoryBuilder.jdk().build(settings))
                .build();
    }
}

@Service
public class PaymentClient {

    private final RestClient paymentRestClient;

    public PaymentClient(RestClient paymentRestClient) {
        this.paymentRestClient = paymentRestClient;
    }

    // GET is idempotent -> safe to retry. Breaker stops hammering a dead service.
    @Retry(name = "payment-reads")
    @CircuitBreaker(name = "payment", fallbackMethod = "statusUnavailable")
    public PaymentStatus status(String chargeId) {
        return paymentRestClient.get()
                .uri("/api/v1/charges/{id}", chargeId)
                .retrieve()
                .body(PaymentStatus.class);
    }

    // POST is NOT retried here. The idempotency key makes a later replay safe server-side.
    @CircuitBreaker(name = "payment", fallbackMethod = "chargeUnavailable")
    public PaymentStatus charge(ChargeRequest request) {
        return paymentRestClient.post()
                .uri("/api/v1/charges")
                .header("Idempotency-Key", request.idempotencyKey())
                .body(request)
                .retrieve()
                .body(PaymentStatus.class);
    }

    PaymentStatus statusUnavailable(String chargeId, Throwable cause) {
        return PaymentStatus.unknown(chargeId);   // degrade: stale/unknown beats 500
    }

    PaymentStatus chargeUnavailable(ChargeRequest request, Throwable cause) {
        throw new PaymentTemporarilyUnavailableException(request.idempotencyKey(), cause);
    }
}
```

```yaml
resilience4j:
  circuitbreaker:
    instances:
      payment:
        sliding-window-type: COUNT_BASED
        sliding-window-size: 20
        minimum-number-of-calls: 10
        failure-rate-threshold: 50
        slow-call-duration-threshold: 2s     # slow IS failure — breakers must see latency
        slow-call-rate-threshold: 80
        wait-duration-in-open-state: 10s
        permitted-number-of-calls-in-half-open-state: 5
        register-health-indicator: true
  retry:
    instances:
      payment-reads:
        max-attempts: 3
        wait-duration: 200ms
        enable-exponential-backoff: true
        exponential-backoff-multiplier: 2
        enable-randomized-wait: true         # jitter — without it all pods retry in sync
        retry-exceptions:
          - java.io.IOException
          - org.springframework.web.client.ResourceAccessException
```

Full property reference and rationale per value: `references/resilience4j-config.md`.

## Connection pool sizing (Hikari)

Bigger pools are usually slower. Start from `pool size = cores × 2` (the classic
`connections = cores × 2 + effective_spindle_count` with SSDs ≈ cores × 2) **on the database server**,
divided across all pods: 10 pods × 25 connections = 250 connections hitting one PostgreSQL is self-inflicted load.

`pending > 0` sustained means demand exceeds supply — but the fix is almost always shortening
connection hold time (transactions doing remote calls inside them, missing `@Transactional(readOnly = true)`,
slow queries), not a bigger pool. Diagnosis workflow: `references/performance-triage.md`.

## Readiness vs liveness

- **Liveness** answers "is this process broken beyond recovery?" — JVM up, not deadlocked. Failing it triggers a **restart**. Never include DB/downstream checks.
- **Readiness** answers "can this pod take traffic right now?" — DB reachable, caches warmed, graceful shutdown in progress. Failing it removes the pod from the Service **without killing it**.

```yaml
management:
  endpoint:
    health:
      probes:
        enabled: true
      group:
        readiness:
          include: readinessState, db
server:
  shutdown: graceful
spring:
  lifecycle:
    timeout-per-shutdown-phase: 25s   # must be < K8s terminationGracePeriodSeconds (default 30s)
```

## Verification

Static checks (run from repo root):

```bash
grep -rn "RestClient.create()\|new RestTemplate()\|WebClient.create(" src/main/java
grep -rn "newFixedThreadPool\|new LinkedBlockingQueue<>()" src/main/java
grep -rn "server.shutdown" src/main/resources || echo "MISSING graceful shutdown"
```

Any hit on the first two = a checklist violation; fix per the table. Build and tests:

```bash
mvn verify                  # Gradle: ./gradlew check
```

Runtime spot checks (local or port-forwarded pod):

```bash
curl -s localhost:8080/actuator/health/readiness
curl -s localhost:8080/actuator/health/liveness
curl -s localhost:8080/actuator/metrics/hikaricp.connections.pending
curl -s localhost:8080/actuator/circuitbreakers
```

What failure looks like: readiness returning `DOWN` while liveness is `UP` is correct behavior during a DB
outage — if both go `DOWN`, liveness is misconfigured. `hikaricp.connections.pending` > 0 sustained =
pool starvation → follow `references/performance-triage.md` step by step before touching pool size.
A breaker stuck `OPEN` after the downstream recovered = `wait-duration-in-open-state` too long or
half-open calls still failing on timeout — check the timeout budget first.

## References

| File | Contents | When to load |
|---|---|---|
| references/resilience4j-config.md | Full Resilience4j YAML reference: circuit breaker, retry, bulkhead, rate limiter, time limiter — recommended values with rationale, aspect ordering, actuator exposure | Tuning or reviewing any Resilience4j configuration |
| references/performance-triage.md | Incident triage workflow: symptom → RED/USE metrics in Grafana/Prometheus → Hikari metrics → thread dumps → GC analysis, with common diagnoses | Diagnosing a live latency/throughput/starvation problem |
| references/observability.md | Micrometer naming and tag conventions, what to instrument, percentile histograms, MDC correlation IDs, structured logging, trace propagation across threads | Adding or reviewing instrumentation and logging |

## Related skills

- **spring-boot-standards** — general service structure, configuration, and Spring conventions (not load/failure behavior).
- **jpa-database-patterns** — slow queries, N+1, transaction design; fix those before resizing any pool.
- **kafka-event-patterns** — consumer groups, rebalancing, exactly-once, DLQs; this skill only covers Kafka client timeouts.
- **dependency-management** — adding/upgrading Resilience4j, Micrometer, or client libraries safely.
- **designing-systems** — choosing sync vs async boundaries so you need less of this skill.
- **reviewing-java-code** — general review workflow; pull this checklist in when the diff touches remote calls.
