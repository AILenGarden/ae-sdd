# Resilience4j Configuration Reference

Spring Boot 3.x with `io.github.resilience4j:resilience4j-spring-boot3`. All values below are
starting points for a typical synchronous service-to-service call with a 2–5s read timeout —
tune against your own SLOs, never copy blindly between dependencies with different latency profiles.

## Aspect ordering

When multiple annotations sit on one method, Resilience4j applies them in this default order
(outermost first):

```
Retry ( CircuitBreaker ( RateLimiter ( TimeLimiter ( Bulkhead ( method ) ) ) ) )
```

Consequences that matter:

- Each retry attempt is a separate call **through** the breaker — the breaker counts every attempt, which is what you want.
- A `CallNotPermittedException` (breaker open) will be retried unless excluded — add it to `ignore-exceptions` on the retry, or each "retry" just bounces off the open breaker.
- Override order with `resilience4j.circuitbreaker.circuit-breaker-aspect-order` etc. only with a written reason.

## Circuit breaker

```yaml
resilience4j:
  circuitbreaker:
    configs:
      default:
        sliding-window-type: COUNT_BASED        # COUNT_BASED is predictable at low traffic
        sliding-window-size: 20                 # last 20 calls decide the state
        minimum-number-of-calls: 10             # don't trip on 1 failure out of 2 calls
        failure-rate-threshold: 50              # % failures to open
        slow-call-duration-threshold: 2s        # latency = failure; align with read timeout
        slow-call-rate-threshold: 80            # % slow calls to open
        wait-duration-in-open-state: 10s        # how long to back off before probing
        permitted-number-of-calls-in-half-open-state: 5
        automatic-transition-from-open-to-half-open-enabled: true
        register-health-indicator: true         # surfaces breaker state in /actuator/health
        record-exceptions:
          - java.io.IOException
          - java.util.concurrent.TimeoutException
          - org.springframework.web.client.ResourceAccessException
        ignore-exceptions:
          - com.example.client.ClientBadRequestException   # 4xx = caller bug, not downstream health
    instances:
      payment:
        base-config: default
      catalog:
        base-config: default
        slow-call-duration-threshold: 500ms     # fast dependency, stricter latency bar
```

Rationale per value:

| Property | Recommendation | Why |
|---|---|---|
| `sliding-window-type` | `COUNT_BASED` | `TIME_BASED` windows behave erratically on low-QPS services (window may contain 0 calls) |
| `minimum-number-of-calls` | ≥ 10 | Below this the failure rate is statistical noise |
| `slow-call-duration-threshold` | ≈ p99 target, ≤ read timeout | A breaker that only sees exceptions is blind to brownouts — slow calls are the common failure mode |
| `wait-duration-in-open-state` | 10–30s | Too short = flapping; too long = slow recovery. With `automatic-transition` enabled, recovery is probe-driven |
| `ignore-exceptions` | All 4xx-mapped exceptions | Client errors must not open the breaker — they say nothing about downstream health |
| `register-health-indicator` | true, but **never** include it in the liveness probe group | An open breaker means a downstream is sick, not this pod |

## Retry

```yaml
resilience4j:
  retry:
    configs:
      default:
        max-attempts: 3                          # 1 original + 2 retries; more rarely helps
        wait-duration: 200ms
        enable-exponential-backoff: true
        exponential-backoff-multiplier: 2        # 200ms -> 400ms -> 800ms
        enable-randomized-wait: true             # jitter: de-synchronizes pods
        randomized-wait-factor: 0.5              # each wait drawn from [0.5w, 1.5w]
        retry-exceptions:
          - java.io.IOException
          - java.util.concurrent.TimeoutException
          - org.springframework.web.client.ResourceAccessException
        ignore-exceptions:
          - io.github.resilience4j.circuitbreaker.CallNotPermittedException
          - com.example.client.ClientBadRequestException
    instances:
      payment-reads:
        base-config: default
```

Rules that override any config:

- **Idempotent operations only.** GET, DELETE, PUT (true idempotent PUT), or writes with a server-honored idempotency key. Never plain POST.
- **Jitter is not optional.** Without `enable-randomized-wait`, every pod that saw the same downstream blip retries at the same instant — synchronized waves. (Combining exponential + randomized requires Resilience4j ≥ 1.7; on older versions register a custom `IntervalFunction.ofExponentialRandomBackoff` bean.)
- **Budget check.** Worst case ≈ attempts × (timeout + max backoff). 3 × (3s + 800ms) ≈ 11.4s — the caller's own timeout and the user-facing SLO must absorb this, or reduce attempts/timeouts.
- Retrying on HTTP 503 + `Retry-After` is legitimate; retrying on 500 repeats a request that may have half-executed.

## Bulkhead

Two implementations — pick by call style:

```yaml
resilience4j:
  bulkhead:                       # semaphore: for synchronous calls on the request thread
    instances:
      payment:
        max-concurrent-calls: 20  # ≈ pool/thread budget you can afford to lose to this dependency
        max-wait-duration: 0      # fail fast; queueing here just hides overload
  thread-pool-bulkhead:           # dedicated pool: for CompletableFuture-based isolation
    instances:
      reporting:
        core-thread-pool-size: 4
        max-thread-pool-size: 8
        queue-capacity: 20        # BOUNDED — the entire point
```

Sizing logic: if Tomcat has 200 threads and the payment bulkhead allows 20 concurrent calls, a fully
hung payment service can pin at most 10% of capacity — the other 90% keeps serving. Without a
bulkhead, it pins 100%.

## Rate limiter (load shedding at the edge or per-client)

```yaml
resilience4j:
  ratelimiter:
    instances:
      ingest-api:
        limit-for-period: 100     # permits per window
        limit-refresh-period: 1s
        timeout-duration: 0       # don't queue — shed: map RequestNotPermitted to HTTP 429
```

Shedding beats queueing: a request that waits 8s and then succeeds is usually already abandoned
by the caller — you paid full cost for zero value. Return 429 fast, let the client back off.

## Time limiter

Only for `CompletableFuture`/reactive returns — it cannot interrupt a blocking RestClient call:

```yaml
resilience4j:
  timelimiter:
    instances:
      payment:
        timeout-duration: 3s
        cancel-running-future: true
```

For blocking clients, the read timeout on the client itself **is** the time limiter. Do not add
`@TimeLimiter` to a blocking method — it fails to enforce and confuses readers.

## Actuator and metrics exposure

```yaml
management:
  endpoints:
    web:
      exposure:
        include: health, metrics, prometheus, circuitbreakers, retries, bulkheads, ratelimiters
  metrics:
    distribution:
      percentiles-histogram:
        http.server.requests: true
        resilience4j.circuitbreaker.calls: true
```

Key Prometheus series to alert on:

| Metric | Alert condition |
|---|---|
| `resilience4j_circuitbreaker_state{state="open"}` | == 1 for > 1m (a breaker is open) |
| `resilience4j_circuitbreaker_slow_call_rate` | > 50 sustained (brownout in progress) |
| `resilience4j_retry_calls_total{kind="failed_with_retry"}` | rate rising (retries exhausted — downstream truly down) |
| `resilience4j_bulkhead_available_concurrent_calls` | == 0 sustained (isolation saturated) |
