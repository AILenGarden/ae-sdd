# Complete Web-Layer Example

One coherent feature slice — DTOs, controller, service, domain exceptions, global handler,
and `@WebMvcTest` — for an `orders` resource. Copy and adapt; keep the layering intact.

## DTOs (`dto/`)

```java
public record CreateOrderRequest(
        @NotNull UUID customerId,
        @NotEmpty @Size(max = 100) List<@Valid OrderLineRequest> lines) {
}

public record OrderLineRequest(
        @NotNull UUID productId,
        @Positive int quantity) {
}

public record OrderResponse(
        UUID id, UUID customerId, OrderStatus status,
        List<OrderLineResponse> lines, Instant createdAt) {

    public static OrderResponse from(Order order) {
        return new OrderResponse(
                order.getId(), order.getCustomerId(), order.getStatus(),
                order.getLines().stream().map(OrderLineResponse::from).toList(),
                order.getCreatedAt());
    }
}

public record OrderLineResponse(UUID productId, int quantity) {
    static OrderLineResponse from(OrderLine line) {
        return new OrderLineResponse(line.getProductId(), line.getQuantity());
    }
}
```

## Domain exceptions (`exception/`)

```java
public class OrderNotFoundException extends RuntimeException {
    public OrderNotFoundException(UUID id) {
        super("Order %s does not exist".formatted(id));
    }
}

public class DuplicateIdempotencyKeyException extends RuntimeException {
    public DuplicateIdempotencyKeyException(String key) {
        super("Idempotency-Key %s was already used with a different payload".formatted(key));
    }
}
```

## Service (`service/`)

```java
@Service
public class OrderService {

    private final OrderRepository orderRepository;
    private final IdempotencyKeyRepository idempotencyKeyRepository;

    public OrderService(OrderRepository orderRepository,
                        IdempotencyKeyRepository idempotencyKeyRepository) {
        this.orderRepository = orderRepository;
        this.idempotencyKeyRepository = idempotencyKeyRepository;
    }

    @Transactional
    public OrderResponse create(String idempotencyKey, CreateOrderRequest request) {
        var existing = idempotencyKeyRepository.findByKey(idempotencyKey);
        if (existing.isPresent()) {
            if (!existing.get().matches(request)) {
                throw new DuplicateIdempotencyKeyException(idempotencyKey);
            }
            return OrderResponse.from(
                    orderRepository.getReferenceById(existing.get().getOrderId()));
        }
        Order order = Order.create(request.customerId(), toLines(request.lines()));
        orderRepository.save(order);
        idempotencyKeyRepository.save(IdempotencyKey.of(idempotencyKey, request, order.getId()));
        return OrderResponse.from(order);
    }

    @Transactional(readOnly = true)
    public OrderResponse getById(UUID id) {
        return orderRepository.findWithLinesById(id)
                .map(OrderResponse::from)
                .orElseThrow(() -> new OrderNotFoundException(id));
    }

    private List<OrderLine> toLines(List<OrderLineRequest> lines) {
        return lines.stream()
                .map(l -> new OrderLine(l.productId(), l.quantity()))
                .toList();
    }
}
```

Notes: the transaction wraps both the idempotency record and the order (atomic dedup);
`findWithLinesById` is a fetch-join query — plain `findById` plus lazy access would be an
N+1 (see jpa-database-patterns).

## Controller (`controller/`)

```java
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
        return ResponseEntity
                .created(URI.create("/api/v1/orders/" + created.id()))
                .body(created);
    }

    @GetMapping("/{id}")
    public OrderResponse get(@PathVariable UUID id) {
        return orderService.getById(id);
    }
}
```

## Global handler (`exception/`)

```java
@RestControllerAdvice
public class GlobalExceptionHandler {

    private static final Logger log = LoggerFactory.getLogger(GlobalExceptionHandler.class);

    @ExceptionHandler(OrderNotFoundException.class)
    public ProblemDetail handleNotFound(OrderNotFoundException ex) {
        return problem(HttpStatus.NOT_FOUND, "Order not found", ex.getMessage(),
                "order-not-found");
    }

    @ExceptionHandler(DuplicateIdempotencyKeyException.class)
    public ProblemDetail handleDuplicateKey(DuplicateIdempotencyKeyException ex) {
        return problem(HttpStatus.CONFLICT, "Idempotency conflict", ex.getMessage(),
                "idempotency-conflict");
    }

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ProblemDetail handleValidation(MethodArgumentNotValidException ex) {
        ProblemDetail p = problem(HttpStatus.BAD_REQUEST, "Invalid request",
                "Request validation failed", "validation-error");
        p.setProperty("errors", ex.getBindingResult().getFieldErrors().stream()
                .map(fe -> Map.of("field", fe.getField(),
                        "message", String.valueOf(fe.getDefaultMessage())))
                .toList());
        return p;
    }

    @ExceptionHandler(MissingRequestHeaderException.class)
    public ProblemDetail handleMissingHeader(MissingRequestHeaderException ex) {
        return problem(HttpStatus.BAD_REQUEST, "Missing header",
                "Required header '%s' is missing".formatted(ex.getHeaderName()),
                "missing-header");
    }

    @ExceptionHandler(Exception.class)
    public ProblemDetail handleUnexpected(Exception ex) {
        log.error("Unhandled exception", ex);                     // full stack to logs only
        return problem(HttpStatus.INTERNAL_SERVER_ERROR, "Internal error",
                "An unexpected error occurred", "internal-error"); // generic detail to client
    }

    private ProblemDetail problem(HttpStatus status, String title, String detail, String slug) {
        ProblemDetail p = ProblemDetail.forStatusAndDetail(status, detail);
        p.setTitle(title);
        p.setType(URI.create("https://api.example.com/problems/" + slug));
        return p;
    }
}
```

## `@WebMvcTest`

```java
@WebMvcTest(OrderController.class)
class OrderControllerTest {

    @Autowired MockMvc mockMvc;        // framework-managed test wiring is fine here
    @MockitoBean OrderService orderService;

    @Test
    void createReturns201WithLocation() throws Exception {
        UUID id = UUID.randomUUID();
        UUID customerId = UUID.randomUUID();
        given(orderService.create(anyString(), any())).willReturn(
                new OrderResponse(id, customerId, OrderStatus.NEW, List.of(), Instant.now()));

        mockMvc.perform(post("/api/v1/orders")
                        .header("Idempotency-Key", UUID.randomUUID().toString())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {"customerId":"%s","lines":[{"productId":"%s","quantity":2}]}
                                """.formatted(customerId, UUID.randomUUID())))
                .andExpect(status().isCreated())
                .andExpect(header().string("Location", "/api/v1/orders/" + id))
                .andExpect(jsonPath("$.id").value(id.toString()));
    }

    @Test
    void createWithEmptyLinesReturns400ProblemDetail() throws Exception {
        mockMvc.perform(post("/api/v1/orders")
                        .header("Idempotency-Key", UUID.randomUUID().toString())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {"customerId":"%s","lines":[]}
                                """.formatted(UUID.randomUUID())))
                .andExpect(status().isBadRequest())
                .andExpect(content().contentType(MediaType.APPLICATION_PROBLEM_JSON))
                .andExpect(jsonPath("$.title").value("Invalid request"))
                .andExpect(jsonPath("$.errors[0].field").value("lines"));
    }

    @Test
    void getUnknownIdReturns404ProblemDetail() throws Exception {
        UUID id = UUID.randomUUID();
        given(orderService.getById(id)).willThrow(new OrderNotFoundException(id));

        mockMvc.perform(get("/api/v1/orders/{id}", id))
                .andExpect(status().isNotFound())
                .andExpect(content().contentType(MediaType.APPLICATION_PROBLEM_JSON))
                .andExpect(jsonPath("$.type")
                        .value("https://api.example.com/problems/order-not-found"));
    }
}
```

Always include the negative cases (400, 404) — they pin the error contract, which is the
part that silently drifts. Service-layer behavior gets its own unit tests with Mockito;
persistence gets Testcontainers integration tests (see tdd-java and jpa-database-patterns).
