---
name: spring-boot-standards
description: >
  Use when writing or reviewing Spring Boot service code and questions of structure or
  convention come up: package layout, where a class belongs, controller/service/repository
  wiring, DTOs vs entities in API responses, REST URL design (/api/v1, plural nouns, status
  codes, pagination, idempotency keys), error responses via @RestControllerAdvice and
  ProblemDetail (RFC 9457), bean validation with @Valid, @ConfigurationProperties records,
  Spring profiles, secrets in application.yml, or OpenAPI annotations. Triggers include
  "where should this class go", "field injection", "@Autowired on fields", "return the
  entity from the controller", "inconsistent error JSON", "400 vs 422", "hardcoded
  password in properties". Not for JPA/Hibernate performance or migrations — use
  jpa-database-patterns. Not for Kafka producers/consumers — use kafka-event-patterns.
  Not for timeouts, retries, or resilience config — use resilience-performance.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---

# Spring Boot Standards

Conventions for structuring Spring Boot 3.x / Java 21 services: packages, injection,
DTOs, REST design, error contract, validation, and configuration.

## When to use

- A controller returns a JPA entity, or request/response shapes are mutable classes with setters
- `@Autowired` on fields, or beans wired via setter injection
- API errors come back as inconsistent ad-hoc JSON (stack traces, `{"error": "..."}` blobs)
- URLs mix verbs and nouns (`/getUser`, `/api/createOrder`) or have no version prefix
- POST endpoints create duplicates on client retry (no idempotency key)
- Passwords/API keys committed in `application.yml`
- Request payloads reach the service layer unvalidated
- New service or module and you need the standard package layout

## Quick reference

| Problem | Symptom | Solution |
|---|---|---|
| Field injection | `@Autowired private Foo foo;` | Constructor injection; single constructor needs no annotation |
| Entity leaks to API | Controller returns `@Entity` type | Record DTO + mapper; entity never crosses controller boundary |
| Ad-hoc error JSON | Different error shape per endpoint | `@RestControllerAdvice` + `ProblemDetail` (RFC 9457) |
| Unvalidated input | Bad data deep in service layer | `@Valid` on `@RequestBody`, constraints on DTO record |
| Verb URLs | `/api/getOrders`, `/createUser` | `/api/v1/orders` — plural nouns, HTTP method carries the verb |
| Duplicate POSTs | Client retry creates two orders | `Idempotency-Key` header, dedup before create |
| Unbounded list endpoint | `GET /orders` returns everything | `Pageable` parameter, return page metadata |
| Scattered `@Value` | Same property string in 5 classes | One `@ConfigurationProperties` record per prefix |
| Secrets in git | Password literal in `application.yml` | Env var placeholder `${DB_PASSWORD}`; secret store injects it |

## MUST

- Use constructor injection for all dependencies; declare fields `private final`
- Use Java records for request/response DTOs; map to/from entities in the service layer
- Prefix REST routes with `/api/v1` (version in path); use plural nouns for collections
- Return `201 Created` + `Location` header on create; `204 No Content` on delete
- Handle errors centrally with `@RestControllerAdvice` returning `ProblemDetail`
- Annotate `@RequestBody` parameters with `@Valid` and put constraints on the DTO
- Bind configuration through `@ConfigurationProperties` records with `@Validated`
- Reference secrets as `${ENV_VAR}` placeholders — values come from the environment/secret store

## MUST NOT

- No field or setter injection (`@Autowired` on a field is a review blocker)
- Never expose JPA entities in controller signatures (request or response)
- No business logic in controllers — controllers translate HTTP, services decide
- No `@Transactional` in controllers (transaction boundary lives in the service layer — see jpa-database-patterns)
- No secrets, tokens, or passwords as literals in any `application*.yml` / properties file
- No catch-and-return-200 error swallowing; let exceptions reach the advice
- Don't version via headers or query params — path versioning only, for consistency

## Package layout

One feature-agnostic standard layout per service:

```
com.example.orders
├── config/          # @Configuration, @ConfigurationProperties records, security config
├── controller/      # @RestController — HTTP in/out only
├── dto/             # request/response records, mappers
├── exception/       # domain exceptions + @RestControllerAdvice handler
├── model/           # JPA entities, enums, value objects
├── repository/      # Spring Data interfaces
└── service/         # business logic, transaction boundaries
```

Dependencies point inward: `controller → service → repository`. Controllers never touch
repositories directly; repositories never reference DTOs.

## Constructor injection and DTOs

❌ BAD — field injection, entity exposed, logic in controller:

```java
@RestController
public class OrderController {

    @Autowired
    private OrderRepository orderRepository;   // field injection, skips the service

    @PostMapping("/createOrder")               // verb URL, no version
    public Order create(@RequestBody Order order) {  // entity as request AND response
        order.setStatus("NEW");                      // business logic in controller
        return orderRepository.save(order);          // returns 200, not 201
    }
}
```

✅ GOOD — constructor injection, records, service owns the logic:

```java
public record CreateOrderRequest(
        @NotNull UUID customerId,
        @NotEmpty List<@Valid OrderLineRequest> lines) {
}

public record OrderResponse(UUID id, UUID customerId, OrderStatus status, Instant createdAt) {

    static OrderResponse from(Order order) {
        return new OrderResponse(order.getId(), order.getCustomerId(),
                order.getStatus(), order.getCreatedAt());
    }
}

@RestController
@RequestMapping("/api/v1/orders")
public class OrderController {

    private final OrderService orderService;

    public OrderController(OrderService orderService) {
        this.orderService = orderService;
    }

    @PostMapping
    public ResponseEntity<OrderResponse> create(
            @RequestHeader("Idempotency-Key") String idempotencyKey,
            @Valid @RequestBody CreateOrderRequest request) {
        OrderResponse created = orderService.create(idempotencyKey, request);
        URI location = URI.create("/api/v1/orders/" + created.id());
        return ResponseEntity.created(location).body(created);
    }

    @GetMapping("/{id}")
    public OrderResponse get(@PathVariable UUID id) {
        return orderService.getById(id);
    }
}
```

Single-constructor classes need no `@Autowired`. The record DTO carries the validation
constraints; the entity stays behind the service.

## REST design essentials

- `GET /api/v1/orders` (paged list) · `GET /api/v1/orders/{id}` · `POST /api/v1/orders`
  · `PUT` full replace · `PATCH` partial · `DELETE /api/v1/orders/{id}` → 204
- Sub-resources for ownership: `GET /api/v1/orders/{id}/lines`
- Status codes: 200 read/update, 201 create (+`Location`), 204 delete, 400 malformed/validation,
  401/403 auth, 404 missing, 409 conflict (duplicate, optimistic-lock, state), 422 semantically
  invalid business request
- Pagination: accept `Pageable` (`page`, `size`, `sort`); cap `size`; never ship an unbounded
  collection endpoint
- Idempotency: mutation-by-retry-prone POSTs require an `Idempotency-Key` header; persist the
  key with the result and replay the stored response on duplicates

Full URL grammar, status-code matrix, versioning and deprecation policy:
`references/rest-api-conventions.md`.

## Error contract — ProblemDetail

❌ BAD — per-endpoint try/catch with ad-hoc shapes:

```java
@GetMapping("/{id}")
public ResponseEntity<?> get(@PathVariable UUID id) {
    try {
        return ResponseEntity.ok(orderService.getById(id));
    } catch (Exception e) {
        return ResponseEntity.status(500)
                .body(Map.of("error", e.getMessage()));  // leaks internals, shape drifts
    }
}
```

✅ GOOD — one advice, RFC 9457 everywhere:

```java
@RestControllerAdvice
public class GlobalExceptionHandler {

    @ExceptionHandler(OrderNotFoundException.class)
    public ProblemDetail handleNotFound(OrderNotFoundException ex) {
        ProblemDetail problem = ProblemDetail.forStatusAndDetail(HttpStatus.NOT_FOUND, ex.getMessage());
        problem.setTitle("Order not found");
        problem.setType(URI.create("https://api.example.com/problems/order-not-found"));
        return problem;
    }

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ProblemDetail handleValidation(MethodArgumentNotValidException ex) {
        ProblemDetail problem = ProblemDetail.forStatusAndDetail(
                HttpStatus.BAD_REQUEST, "Request validation failed");
        problem.setTitle("Invalid request");
        problem.setProperty("errors", ex.getBindingResult().getFieldErrors().stream()
                .map(fe -> Map.of("field", fe.getField(), "message",
                        String.valueOf(fe.getDefaultMessage())))
                .toList());
        return problem;
    }
}
```

Domain exceptions (`OrderNotFoundException`, `DuplicateOrderException`, …) live in
`exception/`; services throw them, the advice maps them. A complete controller + service +
handler + `@WebMvcTest` example is in `references/web-examples.md`.

## Configuration

❌ BAD — scattered `@Value`, secret committed:

```java
@Service
public class PaymentClient {
    @Value("${payment.url}") private String url;
    @Value("${payment.api-key}") private String apiKey;  // literal value sits in application.yml
}
```

✅ GOOD — typed, validated record; secret from the environment:

```java
@Validated
@ConfigurationProperties(prefix = "payment")
public record PaymentProperties(
        @NotBlank String url,
        @NotBlank String apiKey,
        @DefaultValue("5s") Duration timeout) {
}
```

```yaml
# application.yml — placeholder only, never the value
payment:
  url: https://payments.internal.example.com
  api-key: ${PAYMENT_API_KEY}
  timeout: 3s
```

Enable with `@ConfigurationPropertiesScan` (or `@EnableConfigurationProperties`). Profiles:
`application.yml` holds shared defaults, `application-<profile>.yml` holds env deltas only;
activate via `SPRING_PROFILES_ACTIVE`. Details, profile strategy, and secret-store options:
`references/configuration.md`.

## OpenAPI basics

With `springdoc-openapi-starter-webmvc-ui`, annotate intent — not the obvious:

```java
@Operation(summary = "Create an order",
        description = "Idempotent via the Idempotency-Key header.")
@ApiResponse(responseCode = "201", description = "Order created")
@ApiResponse(responseCode = "409", description = "Duplicate idempotency key",
        content = @Content(schema = @Schema(implementation = ProblemDetail.class)))
@PostMapping
public ResponseEntity<OrderResponse> create(...) { ... }
```

Record DTOs generate accurate schemas automatically; add `@Schema(description = ...)` only
where a field name isn't self-explanatory.

## Verification

```bash
mvn verify                 # Maven (primary): compiles, runs unit + integration tests
./gradlew check            # Gradle equivalent
```

- `@WebMvcTest` failures on status codes or JSON shape → the error contract regressed;
  compare the response body against the `ProblemDetail` examples above before changing tests.
- Validation tests failing with 200-instead-of-400 → a `@Valid` is missing on the
  controller parameter, or constraints are missing on the DTO record.
- Quick hygiene greps before review:

```bash
grep -rn "@Autowired" src/main/java --include='*.java'        # expect zero field injections
grep -rnE "(password|secret|api-key|token):[^$]*[a-zA-Z0-9]" src/main/resources  # expect no literals
```

Any hit on either grep is a finding — fix it, don't suppress it.

## References

| File | Contents | When to load |
|---|---|---|
| `references/rest-api-conventions.md` | Full URL/naming grammar, status-code matrix, versioning & deprecation, pagination and idempotency contract, ProblemDetail field standard | Designing or reviewing an API surface, debating a status code |
| `references/web-examples.md` | Complete controller + service + exception handler + `@WebMvcTest`, end to end | Scaffolding a new endpoint or writing web-layer tests |
| `references/configuration.md` | Profiles strategy, `@ConfigurationProperties` patterns, secrets handling, env-specific config | Setting up config for a new service or environment |

## Related skills

- **jpa-database-patterns** — entity design, N+1, transactions, Flyway migrations; route there for anything below the repository interface.
- **kafka-event-patterns** — producing/consuming events, outbox, consumer idempotency.
- **resilience-performance** — timeouts, retries, circuit breakers, connection pools, metrics.
- **dependency-management** — Maven/Gradle versions, BOMs, dependency hygiene.
- **oop-design** — class design, SOLID, when a service is doing too much.
- **tdd-java** — test-first workflow; this skill only covers what web tests assert, not how to drive development.
- **reviewing-java-code** — running a structured review; this skill supplies the standards it checks against.
