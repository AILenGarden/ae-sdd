# Consumer Error Handling: Retry Topics, DLT, Poison Pills

Complete Spring Kafka 3.x error-handling setup for at-least-once consumers.

## Topology

```
orders.order.created            main topic
orders.order.created-retry-0    1s backoff   ┐
orders.order.created-retry-1    3s backoff   ├─ created by @RetryableTopic
orders.order.created-retry-2    9s backoff   ┘
orders.order.created-dlt        exhausted / non-retryable
```

Why retry *topics* instead of blocking in-listener retries: the main partition keeps
flowing while a failing record waits in a retry topic — one bad downstream dependency
doesn't stall every message behind it, and long backoffs don't trip
`max.poll.interval.ms`. Cost: per-aggregate ordering is **not preserved across retries**
(a later event for the same key can be processed while an earlier one waits in retry).
If strict per-key ordering matters more than throughput, use the blocking
`DefaultErrorHandler` variant below instead.

Error classification:

- **Transient** (timeouts, 5xx from downstream, lock conflicts) → retry with backoff.
- **Permanent** (deserialization failure, validation failure, business rule violation) →
  straight to DLT; retrying cannot succeed.

## Listener with @RetryableTopic

```java
@Component
public class OrderCreatedConsumer {

    private static final Logger log = LoggerFactory.getLogger(OrderCreatedConsumer.class);

    private final OrderProjectionService projectionService;
    private final DltEventStore dltEventStore;

    public OrderCreatedConsumer(OrderProjectionService projectionService,
                                DltEventStore dltEventStore) {
        this.projectionService = projectionService;
        this.dltEventStore = dltEventStore;
    }

    @RetryableTopic(
            attempts = "4",                                       // 1 main + 3 retries
            backoff = @Backoff(delay = 1_000, multiplier = 3.0),  // 1s, 3s, 9s
            topicSuffixingStrategy = TopicSuffixingStrategy.SUFFIX_WITH_INDEX_VALUE,
            dltStrategy = DltStrategy.FAIL_ON_ERROR,              // DLT publish failure ≠ drop
            exclude = {DeserializationException.class,
                       MessageConversionException.class,
                       ValidationException.class})                // permanent → DLT immediately
    @KafkaListener(topics = "orders.order.created", groupId = "billing-order-projection")
    public void onOrderCreated(ConsumerRecord<String, OrderCreatedEvent> record,
                               Acknowledgment ack) {
        projectionService.apply(record.value());
        ack.acknowledge();
    }

    @DltHandler
    public void onDlt(ConsumerRecord<String, OrderCreatedEvent> record,
                      @Header(KafkaHeaders.EXCEPTION_MESSAGE) String error) {
        log.error("DLT message topic={} partition={} offset={} key={} error={}",
                record.topic(), record.partition(), record.offset(), record.key(), error);
        dltEventStore.store(record, error);     // persist for replay; metric + alert
    }
}
```

## Container and deserializer config

```yaml
spring:
  kafka:
    consumer:
      group-id: billing-order-projection
      auto-offset-reset: earliest
      enable-auto-commit: false                    # container manages commits
      key-deserializer: org.apache.kafka.common.serialization.StringDeserializer
      value-deserializer: org.springframework.kafka.support.serializer.ErrorHandlingDeserializer
      properties:
        spring.deserializer.value.delegate.class: org.springframework.kafka.support.serializer.JsonDeserializer
        spring.json.trusted.packages: "com.example.events"
        spring.json.value.default.type: com.example.events.OrderCreatedEvent
      max-poll-records: 100
      properties.max.poll.interval.ms: 300000      # records × worst-case must fit with margin
    listener:
      ack-mode: MANUAL                              # ack after success only
      concurrency: 3                                # ≤ partition count
```

`ErrorHandlingDeserializer` is non-negotiable: without it, undeserializable bytes throw
*before* your listener, the offset never advances, and the consumer loops on the same
record forever — the classic stuck partition.

## Alternative: blocking retries with DefaultErrorHandler

When you can't use retry topics (ordering-critical consumers, no auto-topic-creation):

```java
@Bean
public DefaultErrorHandler errorHandler(KafkaTemplate<String, Object> template) {
    var recoverer = new DeadLetterPublishingRecoverer(template,
            (record, ex) -> new TopicPartition(record.topic() + "-dlt", record.partition()));

    var handler = new DefaultErrorHandler(recoverer,
            new ExponentialBackOffWithMaxRetries(3) {{
                setInitialInterval(1_000);
                setMultiplier(3.0);
                setMaxInterval(10_000);
            }});
    handler.addNotRetryableExceptions(DeserializationException.class,
            ValidationException.class, IllegalArgumentException.class);
    return handler;   // set on the ListenerContainerFactory
}
```

Preserves per-partition ordering (the partition *does* pause during backoff — that's the
tradeoff). Total blocking time must stay well under `max.poll.interval.ms`.

## max.poll guidance

Eviction math: the consumer must call `poll()` again within `max.poll.interval.ms`
(default 5 min) or the group coordinator evicts it → rebalance → redelivery → often a
rebalance storm if processing is uniformly slow.

- Budget: `max-poll-records × worst-case-seconds-per-record × 2 < max.poll.interval.ms`.
- Slow consumers: lower `max-poll-records` first (smaller batches, smoother commits);
  raise `max.poll.interval.ms` only with a comment justifying it.
- Never `Thread.sleep` backoff inside a listener — that's what retry topics are for.
- Watch `kafka.consumer.fetch.manager.records.lag` and rebalance counts in Grafana.

## Poison pills — summary

| Cause | Symptom | Handling |
|---|---|---|
| Corrupt/incompatible bytes | Deserialization exception loop, partition stuck | `ErrorHandlingDeserializer` → handler routes to DLT |
| Valid bytes, invalid content | Listener throws on every delivery | `exclude`/not-retryable list → DLT without retries |
| Payload triggers a bug | Same offset fails across deploys | DLT + fix consumer + replay |

## DLT replay runbook

1. Inspect: consume the DLT with a throwaway group
   (`kafka-console-consumer --group dlt-inspect-$(date +%s) --from-beginning
   --property print.headers=true`); the `kafka_exception_*` and original-topic headers
   identify the failure.
2. Fix the root cause (consumer bug, downstream outage, bad producer data).
3. Replay: small batches — copy DLT records back to the main topic with the **original
   key** (preserves partition routing); a small replay CLI/admin endpoint beats ad-hoc
   scripts. Consumers are idempotent (SKILL.md), so replaying already-recovered events is
   safe.
4. Verify: DLT depth back to zero, no re-arrivals, lag normal.
5. If a record is genuinely unprocessable (bad upstream data, never fixable), record the
   decision and archive it — an empty DLT with documented dispositions is the goal state.

Alert on DLT message count > 0 (warning) and growth rate (critical) — a quiet DLT that has
been accumulating for a week is an incident nobody noticed.
