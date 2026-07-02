# Testing Event Flows

Producer unit tests with Mockito, integration tests with Testcontainers Kafka, duplicate
delivery and DLT assertions. JUnit 5 + Awaitility throughout.

## Tooling choice

| Tool | Use for | Why |
|---|---|---|
| Mockito (`KafkaTemplate`/outbox repo mocked) | Producer logic unit tests | Asserts topic/key/payload/headers in ms, no broker |
| Testcontainers Kafka | Consumer + topology integration tests | Real broker, real (de)serialization, retry/DLT actually exercised |
| EmbeddedKafka | Fast in-JVM wiring checks only | Shares the JVM, hides serialization/config issues; never the final proof |

Dependencies (test scope): `spring-kafka-test`, `org.testcontainers:kafka`,
`spring-boot-testcontainers`, `org.awaitility:awaitility`.

## Producer-side unit test (outbox write)

```java
@ExtendWith(MockitoExtension.class)
class OrderServiceTest {

    @Mock OrderRepository orderRepository;
    @Mock OutboxRepository outboxRepository;
    @InjectMocks OrderService orderService;

    @Test
    void createWritesOutboxEventWithAggregateKey() {
        when(orderRepository.save(any())).thenAnswer(inv -> inv.getArgument(0));

        Order order = orderService.create(validRequest());

        ArgumentCaptor<OutboxEvent> captor = ArgumentCaptor.forClass(OutboxEvent.class);
        verify(outboxRepository).save(captor.capture());
        OutboxEvent event = captor.getValue();
        assertThat(event.getEventType()).isEqualTo("orders.order.created");
        assertThat(event.getAggregateId()).isEqualTo(order.getId().toString());
        assertThat(event.getPayload()).contains(order.getCustomerId().toString());
    }
}
```

Atomicity of the two writes (business row + outbox row in one tx) is *not* provable with
mocks — that needs a `@DataJpaTest` with Testcontainers Postgres asserting both rows
exist after commit and neither after a forced rollback.

## Shared Testcontainers setup

```java
@TestConfiguration(proxyBeanMethods = false)
class KafkaTestcontainersConfig {

    @Bean
    @ServiceConnection                       // wires spring.kafka.bootstrap-servers
    KafkaContainer kafkaContainer() {
        return new KafkaContainer(DockerImageName.parse("confluentinc/cp-kafka:7.6.1"));
    }
}
```

`@ServiceConnection` (Boot 3.1+) replaces manual `@DynamicPropertySource`. Reuse one
container across test classes (static container or singleton pattern) — broker startup
dominates test time.

## Consumer integration test: happy path + idempotency

```java
@SpringBootTest
@Import(KafkaTestcontainersConfig.class)
class OrderCreatedConsumerIT {

    @Autowired KafkaTemplate<String, OrderCreatedEvent> kafkaTemplate;
    @Autowired BillingProjectionRepository projectionRepository;
    @Autowired ProcessedEventRepository processedEventRepository;

    @Test
    void consumesEventAndCreatesProjection() {
        OrderCreatedEvent event = anOrderCreatedEvent();

        kafkaTemplate.send("orders.order.created", event.orderId().toString(), event);

        await().atMost(Duration.ofSeconds(10)).untilAsserted(() ->
                assertThat(projectionRepository.findByOrderId(event.orderId())).isPresent());
    }

    @Test
    void duplicateDeliveryProcessesExactlyOnce() {
        OrderCreatedEvent event = anOrderCreatedEvent();
        String key = event.orderId().toString();

        kafkaTemplate.send("orders.order.created", key, event);   // same eventId twice —
        kafkaTemplate.send("orders.order.created", key, event);   // simulates redelivery

        await().atMost(Duration.ofSeconds(10)).untilAsserted(() ->
                assertThat(processedEventRepository.existsById(event.eventId())).isTrue());

        // settle window, then exactly-once on the side effect
        await().during(Duration.ofSeconds(3)).atMost(Duration.ofSeconds(15)).untilAsserted(() ->
                assertThat(projectionRepository.countByOrderId(event.orderId())).isEqualTo(1));
    }
}
```

The duplicate test is the one that catches real bugs — it fails when the dedup write and
the projection write are not in the same transaction.

## DLT routing test

```java
@SpringBootTest
@Import(KafkaTestcontainersConfig.class)
class OrderCreatedDltIT {

    @Autowired KafkaTemplate<String, String> rawTemplate;   // String template: send bad JSON
    @Autowired ConsumerFactory<String, String> consumerFactory;

    @Test
    void poisonMessageRoutesToDltWithoutBlockingPartition() {
        rawTemplate.send("orders.order.created", "key-1", "{not valid json");

        try (Consumer<String, String> dltConsumer =
                     consumerFactory.createConsumer("dlt-assert-" + UUID.randomUUID(), null)) {
            dltConsumer.subscribe(List.of("orders.order.created-dlt"));

            ConsumerRecords<String, String> records =
                    KafkaTestUtils.getRecords(dltConsumer, Duration.ofSeconds(15));

            assertThat(records.count()).isGreaterThanOrEqualTo(1);
            ConsumerRecord<String, String> dlt = records.iterator().next();
            assertThat(dlt.key()).isEqualTo("key-1");
            assertThat(dlt.headers().lastHeader("kafka_exception-fqcn")).isNotNull();
        }
    }

    @Test
    void partitionKeepsFlowingAfterPoisonMessage() {
        rawTemplate.send("orders.order.created", "key-2", "{not valid json");
        OrderCreatedEvent good = anOrderCreatedEvent();
        rawTemplate.send("orders.order.created", "key-2", JsonSupport.toJson(good));

        await().atMost(Duration.ofSeconds(15)).untilAsserted(() ->
                assertThat(projectionRepository.findByOrderId(good.orderId())).isPresent());
    }
}
```

The second test pins the property that actually matters in production: one poison pill
must not stall subsequent records.

## Practices that keep these tests honest

- Unique consumer group per assertion consumer (`"dlt-assert-" + UUID.randomUUID()`) —
  reused groups inherit committed offsets and read nothing.
- Await on **observable state** (DB row, DLT record), never `Thread.sleep`. Use
  `await().during(...)` for "nothing else happens" assertions.
- Generate unique aggregate ids per test; with a shared container + `auto-offset-reset:
  earliest`, leftover records from earlier tests are otherwise indistinguishable.
- Test the contract (state changes, emitted events), not Spring Kafka internals — don't
  assert on listener invocation counts via spies.
- Retry-topic timing: keep test-profile backoff short (`@RetryableTopic` with property
  placeholders, e.g. `backoff = @Backoff(delayExpression = "${app.retry.delay:1000}")`)
  so DLT tests don't wait through production backoff schedules.
- `mvn verify` runs these as ITs (Failsafe) or via Surefire if you keep them in the
  standard test phase; Gradle: `./gradlew check`. Docker must be available — guard CI
  agents accordingly.
