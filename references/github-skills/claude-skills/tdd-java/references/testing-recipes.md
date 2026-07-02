# Testing Recipes (Java 21 / Spring Boot 3.x / JUnit 5)

Load this when setting up slice tests, Testcontainers, Mockito, parameterized tests, or async assertions. Commands assume Maven (`mvn verify`); Gradle equivalent is `./gradlew check`.

## Slice tests

### `@WebMvcTest` — controller layer only

Loads MVC infrastructure + the named controller. Services must be mocked.

```java
@WebMvcTest(OrderController.class)
class OrderControllerTest {

    @Autowired MockMvc mockMvc;
    @MockitoBean OrderService orderService;   // Spring Boot 3.4+; earlier: @MockBean

    @Test
    void getOrder_existingId_returns200WithBody() throws Exception {
        given(orderService.findById(42L))
            .willReturn(new OrderDto(42L, new BigDecimal("19.90")));

        mockMvc.perform(get("/api/v1/orders/42"))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.id").value(42))
            .andExpect(jsonPath("$.total").value(19.90));
    }

    @Test
    void getOrder_unknownId_returns404() throws Exception {
        given(orderService.findById(99L)).willThrow(new OrderNotFoundException(99L));

        mockMvc.perform(get("/api/v1/orders/99"))
            .andExpect(status().isNotFound());
    }

    @Test
    void createOrder_negativeTotal_returns400() throws Exception {
        mockMvc.perform(post("/api/v1/orders")
                .contentType(MediaType.APPLICATION_JSON)
                .content("""
                    {"total": -5.00}
                    """))
            .andExpect(status().isBadRequest());
    }
}
```

Use it for: serialization, validation (`@Valid` rejection paths), status codes, exception-handler mapping. NOT for business logic — that belongs in unit tests.

### `@DataJpaTest` — repository layer only

Loads JPA + repositories, runs each test in a rolled-back transaction. By default swaps in an embedded DB — override that to test against the real engine (next section).

```java
@DataJpaTest
@AutoConfigureTestDatabase(replace = AutoConfigureTestDatabase.Replace.NONE)
@Testcontainers
class OrderRepositoryTest {

    @Container
    @ServiceConnection
    static PostgreSQLContainer<?> postgres = new PostgreSQLContainer<>("postgres:16-alpine");

    @Autowired OrderRepository repository;
    @Autowired TestEntityManager em;

    @Test
    void findByStatus_mixedStatuses_returnsOnlyMatching() {
        em.persist(anOrder(Status.PAID));
        em.persist(anOrder(Status.CANCELLED));
        em.flush();
        em.clear();   // force real SQL on read, defeat first-level cache

        assertThat(repository.findByStatus(Status.PAID)).hasSize(1);
    }
}
```

Use it for: derived queries, `@Query` JPQL/native, mappings, constraint violations. `em.flush(); em.clear();` before assertions or you may be asserting against the persistence context, not the database.

## Testcontainers boilerplate

`@ServiceConnection` (Spring Boot 3.1+) replaces manual `@DynamicPropertySource` wiring for supported containers.

### Postgres + Kafka integration test

```java
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@Testcontainers
class OrderFlowIntegrationTest {

    @Container
    @ServiceConnection
    static PostgreSQLContainer<?> postgres = new PostgreSQLContainer<>("postgres:16-alpine");

    @Container
    @ServiceConnection
    static KafkaContainer kafka = new KafkaContainer(
        DockerImageName.parse("confluentinc/cp-kafka:7.6.0"));

    @Autowired TestRestTemplate rest;
    @Autowired OrderRepository repository;
}
```

Maven deps (test scope): `org.springframework.boot:spring-boot-testcontainers`, `org.testcontainers:postgresql`, `org.testcontainers:kafka`, `org.testcontainers:junit-jupiter`.

- `static` container fields = one container per test class, reused across methods. Non-static = container per test, much slower; almost never what you want.
- For cross-class reuse, define containers in a `@TestConfiguration` with `@Bean @ServiceConnection` and import it, or enable `testcontainers.reuse.enable=true` in `~/.testcontainers.properties` (local dev only — not CI).
- Pin image tags (`postgres:16-alpine`, not `postgres:latest`): unpinned images make tests fail for reasons unrelated to your code.

## Mockito do / don't

| ✅ Do | ❌ Don't |
|---|---|
| Mock ports you own at architecture boundaries (repository interface, payment gateway client) | Mock value objects, entities, DTOs — construct real ones |
| `given(...).willReturn(...)` (BDD style) for stubbing | Mock types you don't own (`RestClient`, `KafkaTemplate`) — wrap them, or use slice/integration tests |
| `verify` for genuine outbound side effects: `verify(eventPublisher).publish(orderPaidEvent)` | `verify(repository).findById(42L)` when you can assert on the returned result instead |
| `ArgumentCaptor` to assert on a published message's content | Deep-stub chains (`RETURNS_DEEP_STUBS`) — a smell that you're mocking structure, not behavior |
| Strict stubbing (the JUnit 5 extension default) — unused stubs fail the test | `lenient()` sprinkled everywhere to silence strictness — those warnings are dead test code |

Rule of thumb: if a test has more `verify` lines than assertions on outcomes, it is testing the implementation and will break on the next harmless refactor.

## Parameterized tests

Collapse same-shape cases; keep distinct behaviors as distinct tests.

```java
@ParameterizedTest(name = "total {0} -> discounted {1}")
@CsvSource({
    "100.00, 100.00",   // at threshold: no discount (boundary)
    "100.01,  90.01",   // just above: 10% off (boundary)
    "200.00, 180.00",
    "0.00,     0.00"
})
void applyDiscount_variousTotals_appliesThresholdRule(BigDecimal in, BigDecimal expected) {
    var total = new DiscountService().applyDiscount(new Order(in));
    assertThat(total).isEqualByComparingTo(expected);
}
```

`@MethodSource` for non-trivial fixtures, `@EnumSource` to cover every enum constant (catches unhandled new constants), `@NullAndEmptySource` for input-validation cases.

## Awaitility for async

Never `Thread.sleep()` in tests — it is both too slow and too flaky. Poll for the expected state:

```java
// dep: org.awaitility:awaitility (test scope)
kafkaTemplate.send("orders", orderPaidEvent);

await().atMost(Duration.ofSeconds(10))
       .untilAsserted(() ->
           assertThat(repository.findById(orderId))
               .hasValueSatisfying(o -> assertThat(o.status()).isEqualTo(Status.PAID)));
```

- `untilAsserted` gives AssertJ messages on timeout; prefer it over boolean `until`.
- Also applies to `@Async` methods and `@EventListener` side effects.
- For `@Scheduled` logic, don't wait for the schedule: extract the task into a bean method and invoke it directly; reserve Awaitility for the truly asynchronous path.

## Running

```bash
mvn test -Dtest=OrderServiceTest                 # one class
mvn test -Dtest=OrderServiceTest#applyDiscount_orderAboveThreshold_reducesTotalByTenPercent
mvn verify                                       # full suite incl. integration (failsafe)

./gradlew test --tests OrderServiceTest          # Gradle equivalents
./gradlew check
```
