# Transactional Outbox: Complete Implementation

Why: a service that writes to Postgres *and* publishes to Kafka has two resources and no
shared transaction. Send-after-commit loses events when the broker is down;
send-before-commit publishes phantoms for rolled-back transactions. The outbox makes the
event part of the database transaction; a relay moves committed events to Kafka.
Guarantee: **at-least-once** — consumers must be idempotent (see SKILL.md).

## Outbox table (Flyway migration)

```sql
CREATE TABLE outbox_event (
    id             uuid PRIMARY KEY,
    aggregate_type varchar(64)  NOT NULL,          -- 'order'
    aggregate_id   varchar(64)  NOT NULL,          -- Kafka key → ordering per aggregate
    event_type     varchar(128) NOT NULL,          -- 'orders.order.created'
    schema_version int          NOT NULL DEFAULT 1,
    payload        jsonb        NOT NULL,
    created_at     timestamptz  NOT NULL DEFAULT now(),
    published_at   timestamptz                      -- NULL = pending
);

-- relay scans only the pending slice; partial index keeps it tiny
CREATE INDEX idx_outbox_pending ON outbox_event (created_at) WHERE published_at IS NULL;
```

`id` doubles as the `eventId` consumers dedup on. Use UUIDv7 (time-ordered) if available
to keep index locality.

## JPA entity + repository

```java
@Entity
@Table(name = "outbox_event")
public class OutboxEvent {

    @Id private UUID id;
    private String aggregateType;
    private String aggregateId;
    private String eventType;
    private int schemaVersion;
    @JdbcTypeCode(SqlTypes.JSON) private String payload;
    private Instant createdAt;
    private Instant publishedAt;

    protected OutboxEvent() {}

    public static OutboxEvent of(String eventType, String aggregateId, Object payloadObject) {
        OutboxEvent e = new OutboxEvent();
        e.id = UUID.randomUUID();
        e.aggregateType = eventType.split("\\.")[1];
        e.aggregateId = aggregateId;
        e.eventType = eventType;
        e.schemaVersion = 1;
        e.payload = JsonSupport.toJson(payloadObject);   // your Jackson wrapper
        e.createdAt = Instant.now();
        return e;
    }
    // getters + markPublished()
}

public interface OutboxRepository extends JpaRepository<OutboxEvent, UUID> {

    @Query(value = """
            SELECT * FROM outbox_event
            WHERE published_at IS NULL
            ORDER BY created_at
            LIMIT :batchSize
            FOR UPDATE SKIP LOCKED
            """, nativeQuery = true)
    List<OutboxEvent> lockNextBatch(int batchSize);
}
```

`FOR UPDATE SKIP LOCKED` lets multiple relay pods (or overlapping scheduled runs) work
without double-claiming or blocking each other.

## Writing to the outbox (service side)

```java
@Transactional
public Order create(CreateOrderRequest request) {
    Order order = orderRepository.save(Order.create(request));
    outboxRepository.save(OutboxEvent.of(
            "orders.order.created", order.getId().toString(), OrderCreatedEvent.from(order)));
    return order;   // single commit: business row + event row, or neither
}
```

## Relay option A — polling publisher (no extra infra)

```java
@Component
public class OutboxRelay {

    private static final Logger log = LoggerFactory.getLogger(OutboxRelay.class);

    private final OutboxRepository outboxRepository;
    private final KafkaTemplate<String, String> kafkaTemplate;

    public OutboxRelay(OutboxRepository outboxRepository,
                       KafkaTemplate<String, String> kafkaTemplate) {
        this.outboxRepository = outboxRepository;
        this.kafkaTemplate = kafkaTemplate;
    }

    @Scheduled(fixedDelayString = "${app.outbox.poll-interval:1s}")
    @Transactional                       // lock + send + mark in one tx
    public void publishPending() {
        for (OutboxEvent event : outboxRepository.lockNextBatch(100)) {
            ProducerRecord<String, String> record = new ProducerRecord<>(
                    event.getEventType(), event.getAggregateId(), event.getPayload());
            record.headers()
                    .add("eventId", event.getId().toString().getBytes(UTF_8))
                    .add("eventType", event.getEventType().getBytes(UTF_8))
                    .add("schemaVersion",
                            Integer.toString(event.getSchemaVersion()).getBytes(UTF_8));
            try {
                kafkaTemplate.send(record).get(10, TimeUnit.SECONDS);  // sync: confirm before mark
                event.markPublished();
            } catch (Exception ex) {
                log.error("Outbox publish failed for {}", event.getId(), ex);
                throw new OutboxPublishException(event.getId(), ex);   // rollback → retry next tick
            }
        }
    }
}
```

Producer config for the relay: `acks=all`, `enable.idempotence=true` (broker-side dedup of
producer retries), sensible `delivery.timeout.ms`. Ordering note: rows are sent in
`created_at` order and the batch aborts on first failure, preserving per-aggregate order
on retry; with multiple relay pods, strict cross-batch ordering is not guaranteed — keep
one relay instance (or shard by aggregate) if per-aggregate ordering is hard-required.

Tradeoffs: simplest possible setup; costs one poll query per interval and adds up to
`poll-interval` latency. Failure mode: relay down → events accumulate (lag alert on
pending count), nothing lost.

## Relay option B — Debezium CDC (log-based)

Debezium tails the Postgres WAL (logical replication) and publishes outbox inserts via its
**outbox event router** — no polling, low latency, no relay code in your service. Routes
on `aggregate_type`/`event_type` columns, keys by `aggregate_id`, carries `id` as event id
header.

Tradeoffs: real infrastructure (Kafka Connect cluster, connector monitoring, replication
slots — a forgotten slot blocks WAL cleanup and fills the disk). Choose Debezium when
event latency matters or many services share the pattern and a platform team owns
Connect; choose the poller when you want zero new moving parts. The table schema above
works for both, so you can start with A and migrate.

## Cleanup

The partial index keeps the relay fast regardless of table size, but prune published rows:

```sql
DELETE FROM outbox_event
WHERE published_at IS NOT NULL AND published_at < now() - interval '7 days';
```

Run as a scheduled job in batches (see jpa-database-patterns
`references/migrations.md` backfill pattern). Keep a retention window long enough for
incident replay/debugging.

## Monitoring

- Gauge: pending count — `SELECT count(*) FROM outbox_event WHERE published_at IS NULL`
  (cheap via the partial index); alert on sustained growth.
- Age of oldest pending event > a few minutes ⇒ relay stuck or broker unavailable.
- Relay error rate via `OutboxPublishException` log/metric counter.
