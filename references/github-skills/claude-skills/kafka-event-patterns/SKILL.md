---
name: kafka-event-patterns
description: >
  Use when building or reviewing Kafka producers, consumers, or event-driven flows in
  Spring Boot: topic naming and keys, event schema/versioning headers, consumer groups,
  manual acknowledgment, retry topics and dead-letter topics (DLT), poison-pill messages,
  rebalancing storms from max.poll abuse, the transactional outbox pattern (events
  missing after a DB commit, dual-write inconsistency), idempotent consumers (duplicate
  events processed twice, dedup by event id), and testing Kafka flows with Testcontainers
  or EmbeddedKafka. Triggers include "@KafkaListener", "KafkaTemplate", "event published
  but DB rolled back", "consumer lag", "CommitFailedException", "same event consumed
  twice", "stuck partition", "DeadLetterPublishingRecoverer". Not for REST/DTO
  conventions — use spring-boot-standards. Not for the JPA/transaction mechanics
  underneath the outbox — use jpa-database-patterns. Not for broker-level retry/timeout
  tuning beyond consumers — use resilience-performance.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---

# Kafka Event Patterns

Producing, consuming, and testing events reliably with Spring Kafka: naming, ordering,
outbox, retry/DLT, and idempotency for at-least-once systems.

## When to use

- Designing a new event/topic or wiring a `@KafkaListener` / `KafkaTemplate`
- DB record exists but the event never arrived (or the event arrived for a rolled-back tx)
- Same event processed twice → duplicate side effects (double email, double ledger entry)
- One bad message stops a partition (poison pill, endless redelivery)
- `CommitFailedException` / rebalance loops because processing exceeds `max.poll.interval.ms`
- Consumer lag growing; deciding retry topic vs DLT topology
- Writing tests for producers/consumers and choosing Testcontainers vs EmbeddedKafka

## Quick reference

| Problem | Symptom | Solution |
|---|---|---|
| Dual write | Event missing after commit, or event for rolled-back tx | Transactional outbox: event row + business row in one tx, relay publishes |
| Lost ordering | Related events interleave across partitions | Key by aggregate id (`orderId`); never null keys for entity events |
| Duplicate processing | Side effect happens twice | Idempotent consumer: dedup table on `eventId`, or natural upsert |
| Poison pill | Same message redelivered forever, partition stuck | Retry topics with backoff → DLT; deserialization errors via `ErrorHandlingDeserializer` straight to DLT |
| Auto-ack data loss | Offsets committed before processing finishes | `AckMode.RECORD` (or retry-topic default); ack after success only |
| Rebalance storms | `CommitFailedException`, group churns | Lower `max.poll.records`, keep per-record work short, raise `max.poll.interval.ms` deliberately |
| Schema drift | Consumers break on payload change | Additive-only changes; version header; new topic for breaking changes |
| Untestable flow | Mocks asserting nothing real | Testcontainers Kafka integration test, producer unit tests with mocked `KafkaTemplate` |

## MUST

- Name topics `<domain>.<entity>.<event>` — `orders.order.created` (lowercase, dot-separated, past-tense event)
- Key every entity event by aggregate id — ordering is only guaranteed within a partition
- Carry `eventId` (UUID), `eventType`, `schemaVersion`, `occurredAt` — as headers or envelope fields — on every event
- Publish state changes through the transactional outbox (event row committed atomically with the business change)
- Make every consumer idempotent — at-least-once delivery means duplicates are a certainty, not an edge case
- Use manual/record-level acknowledgment; commit offsets only after successful processing
- Route exhausted retries to a DLT and monitor DLT depth + consumer lag (Micrometer → Grafana, alert on growth)
- Set explicit `groupId` per logical consumer; one consumer group per service-purpose

## MUST NOT

- No `kafkaTemplate.send()` inside or directly after a `@Transactional` business method — that's the dual-write bug the outbox exists for
- Never swallow exceptions in a listener to "keep the consumer alive" — that is silent data loss; let the error handler route to retry/DLT
- No unbounded in-listener retry loops (`while`/`Thread.sleep`) — they trigger `max.poll.interval.ms` evictions
- Don't reuse one consumer group across different purposes, and don't change a group id casually (offsets reset)
- No breaking payload changes on an existing topic — additive evolution or a new `.v2` topic
- Don't rely on EmbeddedKafka for the final integration proof — Testcontainers runs the real broker

## Producer conventions

❌ BAD — dual write, no key, anonymous payload:

```java
@Transactional
public Order create(CreateOrderRequest request) {
    Order order = orderRepository.save(Order.create(request));
    kafkaTemplate.send("order-topic", toJson(order));   // no key; and if the tx rolls back
    return order;                                       // after send: phantom event. If the
}                                                       // broker is down: lost event.
```

✅ GOOD — outbox write in the same transaction; relay publishes with key + headers:

```java
@Transactional
public Order create(CreateOrderRequest request) {
    Order order = orderRepository.save(Order.create(request));
    outboxRepository.save(OutboxEvent.of(
            "orders.order.created",            // topic
            order.getId().toString(),          // key = aggregate id → ordering per order
            OrderCreatedEvent.from(order)));   // payload record + eventId/version headers
    return order;                              // one commit covers both rows
}
```

The relay (scheduled poller with `FOR UPDATE SKIP LOCKED`, or Debezium CDC) reads the
outbox table and publishes. Full DDL, entity, relay options, and tradeoffs:
`references/outbox-implementation.md`. Direct `KafkaTemplate.send` is acceptable only for
fire-and-forget telemetry where loss and phantoms are tolerable.

Event payloads are records, versioned additively:

```java
public record OrderCreatedEvent(
        UUID eventId, int schemaVersion, Instant occurredAt,
        UUID orderId, UUID customerId, List<OrderLinePayload> lines) {
}
```

## Consumer conventions

❌ BAD — auto-ack, swallow-all, infinite in-place retry:

```java
@KafkaListener(topics = "orders.order.created")
public void onMessage(String payload) {
    try {
        process(parse(payload));
    } catch (Exception e) {
        log.warn("failed, will be fine", e);   // offset commits anyway: message gone
    }
}
```

✅ GOOD — typed record, explicit group, retry topics + DLT, ack on success only:

```java
@Component
public class OrderCreatedConsumer {

    private final OrderProjectionService projectionService;

    public OrderCreatedConsumer(OrderProjectionService projectionService) {
        this.projectionService = projectionService;
    }

    @RetryableTopic(
            attempts = "4",                                    // main + 3 retry topics
            backoff = @Backoff(delay = 1_000, multiplier = 3), // 1s, 3s, 9s
            dltStrategy = DltStrategy.FAIL_ON_ERROR,
            exclude = {DeserializationException.class, ValidationException.class}) // straight to DLT
    @KafkaListener(topics = "orders.order.created", groupId = "billing-order-projection")
    public void onOrderCreated(ConsumerRecord<String, OrderCreatedEvent> record,
                               Acknowledgment ack) {
        projectionService.apply(record.value());   // idempotent — see below
        ack.acknowledge();                         // offset commits only after success
    }

    @DltHandler
    public void onDlt(ConsumerRecord<String, OrderCreatedEvent> record) {
        // alert + persist for replay; never silently drop
    }
}
```

Container config: `AckMode.MANUAL` (or `RECORD`), `ErrorHandlingDeserializer` wrapping the
JSON deserializer so poison bytes become handleable errors instead of an infinite
deserialize-crash loop. `max.poll.records` small enough that
`records × worst-case-per-record < max.poll.interval.ms` with margin. Complete container
factory, `DefaultErrorHandler`/`DeadLetterPublishingRecoverer` alternative, DLT replay
runbook: `references/consumer-error-handling.md`.

## Idempotent consumers

At-least-once + retries + rebalances ⇒ duplicates will arrive. Two strategies:

**Dedup table** (general case) — same transaction as the side effect:

```java
@Transactional
public void apply(OrderCreatedEvent event) {
    try {
        processedEventRepository.save(new ProcessedEvent(event.eventId(), Instant.now()));
    } catch (DataIntegrityViolationException duplicate) {
        return;                                   // unique(event_id) → already processed: no-op
    }
    billingProjectionRepository.save(BillingProjection.from(event));
}
```

The unique constraint is the guard — atomic with the business write, race-proof across
pods. Prune old rows past the replay horizon.

**Natural idempotency** (when the model allows) — upsert keyed by aggregate id
(`INSERT ... ON CONFLICT (order_id) DO UPDATE`), or state transitions that ignore replays
(`if (order.getStatus() == SHIPPED) return;`). Prefer it when the write is the only side
effect; use the dedup table when processing triggers emails, payments, or further events.

## Testing events

| Layer | Tool | What it proves |
|---|---|---|
| Producer logic | Mockito-mocked `KafkaTemplate`/outbox repo | Right topic, key, payload, headers — fast unit test |
| Outbox write | `@DataJpaTest` + Testcontainers Postgres | Business row + outbox row in one tx |
| Consumer + topology | `@SpringBootTest` + Testcontainers Kafka (`@ServiceConnection`) | Real deserialization, retry/DLT routing, idempotency under redelivery |

Testcontainers is the default for integration tests — real broker, prod-like config.
EmbeddedKafka is acceptable for fast in-JVM listener wiring checks, but don't let it be
the only proof: it shares the JVM, masks serialization/config issues, and behaves
differently under rebalance. Full examples (producer test, consumer happy path, duplicate
delivery, DLT assertion, awaitility patterns): `references/testing-events.md`.

## Verification

```bash
mvn verify                 # Maven (primary): unit + Testcontainers integration tests
./gradlew check            # Gradle equivalent
```

- Kafka integration tests hanging or `Connection to node -1 could not be established` →
  container not wired into properties; check `@ServiceConnection` / `@DynamicPropertySource`
  before touching timeouts.
- Duplicate-delivery test failing → idempotency guard isn't atomic with the side effect;
  verify the unique constraint and shared transaction.
- DLT test green but retry counts off → `@RetryableTopic` `attempts` includes the first
  delivery; recount topics (`orders.order.created-retry-0…`, `-dlt`).
- Quick hygiene grep — direct sends from transactional services (outbox bypass):

```bash
grep -rn "kafkaTemplate.send" src/main/java --include='*.java'
```

Every hit outside the outbox relay needs a justification or a refactor.

## References

| File | Contents | When to load |
|---|---|---|
| `references/outbox-implementation.md` | Outbox table DDL, JPA entity, polling relay with SKIP LOCKED, Debezium CDC option, cleanup and tradeoffs | Implementing or reviewing the outbox |
| `references/consumer-error-handling.md` | Full retry/DLT topology, container factory + `DefaultErrorHandler` config, poison-pill handling, DLT replay runbook | Building consumer error handling or debugging a stuck partition |
| `references/testing-events.md` | Complete Testcontainers Kafka test classes: producer, consumer, duplicate delivery, DLT assertions | Writing event tests |

## Related skills

- **spring-boot-standards** — service structure, DTO records, config properties used by producers/consumers.
- **jpa-database-patterns** — the transaction, locking (`SKIP LOCKED`), and migration mechanics the outbox depends on.
- **resilience-performance** — timeouts, circuit breakers, and backpressure around downstream calls made from consumers.
- **tdd-java** — driving these flows test-first; this skill defines what event tests must prove.
- **designing-systems** — deciding event-driven vs synchronous, event granularity, and topic ownership across services.
- **reviewing-java-code** — review checklists that apply these rules to PRs.
