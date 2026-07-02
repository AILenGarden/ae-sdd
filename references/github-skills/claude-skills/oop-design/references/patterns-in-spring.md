# Design Patterns, Idiomatically in Spring

The GoF patterns that earn their keep in a Spring Boot 3.x service, in the form the container
makes natural — plus the ones the container makes obsolete.

## Strategy — injected bean map

The flagship version is in SKILL.md (payment handlers). Variations worth knowing:

**Keyed by something other than an enum** — qualify by name or a `supports()` predicate:

```java
public interface ExportFormat {
    String key();                              // "csv", "xlsx", "json"
    Resource export(ReportData data);
}

@Service
public class ReportExporter {
    private final Map<String, ExportFormat> formats;

    public ReportExporter(List<ExportFormat> formatBeans) {
        this.formats = formatBeans.stream()
                .collect(Collectors.toUnmodifiableMap(ExportFormat::key, f -> f));
    }

    public Resource export(String formatKey, ReportData data) {
        return Optional.ofNullable(formats.get(formatKey))
                .orElseThrow(() -> new UnsupportedFormatException(formatKey))
                .export(data);
    }
}
```

**Ordered chain** (all strategies run, not one): inject `List<Validator>` and annotate
implementations with `@Order`; Spring sorts the list. Useful for validation pipelines and
enrichment steps.

## Factory — @Bean methods first, factory classes for runtime data

Configuration-time creation (wiring, settings) belongs in `@Configuration`:

```java
@Configuration
public class ClientConfig {

    @Bean
    RestClient billingRestClient(RestClient.Builder builder, BillingProperties props) {
        return builder.baseUrl(props.baseUrl()).build();   // this IS the factory pattern
    }
}
```

Runtime creation (depends on request data) gets an explicit factory bean — never `new` scattered
through services when construction has rules:

```java
@Component
public class StatementFactory {

    private final Clock clock;

    public StatementFactory(Clock clock) { this.clock = clock; }   // injected Clock = testable dates

    public Statement createFor(Account account, YearMonth period) {
        if (period.isAfter(YearMonth.now(clock))) {
            throw new StatementPeriodInFutureException(period);
        }
        return new Statement(StatementId.generate(), account.id(), period, clock.instant());
    }
}
```

## Builder — for 4+ optional parameters

Records cover most construction. Reach for a builder when construction has many optional parts or
needs staged readability — and put it ON the record:

```java
public record SearchCriteria(
        String text,
        Set<Status> statuses,
        LocalDate from,
        LocalDate to,
        int limit) {

    public static Builder builder() { return new Builder(); }

    public static final class Builder {
        private String text;
        private Set<Status> statuses = EnumSet.allOf(Status.class);
        private LocalDate from;
        private LocalDate to;
        private int limit = 50;

        public Builder text(String text) { this.text = text; return this; }
        public Builder statuses(Set<Status> statuses) { this.statuses = EnumSet.copyOf(statuses); return this; }
        public Builder between(LocalDate from, LocalDate to) { this.from = from; this.to = to; return this; }
        public Builder limit(int limit) { this.limit = limit; return this; }

        public SearchCriteria build() {
            if (from != null && to != null && from.isAfter(to)) {
                throw new IllegalArgumentException("from after to");
            }
            return new SearchCriteria(text, statuses, from, to, limit);
        }
    }
}
```

Don't hand-write builders for 2–3 required params — that's a constructor. (If the team uses
MapStruct/record patterns heavily, weigh whether the builder adds anything before writing one.)

## Decorator — layering concerns without touching the core

Wrap an interface to add caching/metrics/fallback; the core implementation stays pure:

```java
public interface ExchangeRateProvider {
    Rate rateFor(Currency from, Currency to);
}

@Component
class HttpExchangeRateProvider implements ExchangeRateProvider { /* the real call */ }

@Component
@Primary                                            // callers get the decorated chain
class CachingExchangeRateProvider implements ExchangeRateProvider {

    private final ExchangeRateProvider delegate;
    private final Cache<CurrencyPair, Rate> cache = Caffeine.newBuilder()
            .expireAfterWrite(Duration.ofMinutes(5))
            .maximumSize(1_000)
            .build();

    CachingExchangeRateProvider(HttpExchangeRateProvider delegate) {   // concrete inner by type
        this.delegate = delegate;
    }

    @Override
    public Rate rateFor(Currency from, Currency to) {
        return cache.get(new CurrencyPair(from, to), p -> delegate.rateFor(p.from(), p.to()));
    }
}
```

Notes: `@Primary` on the outermost decorator + injecting the concrete inner type avoids circular
ambiguity. For cross-cutting concerns Spring already decorates for you — `@Cacheable`,
`@Transactional`, `@Retry` are proxy-based decorators; prefer them, and hand-roll only when you
need per-call logic the annotations can't express. (Proxy caveat: self-invocation —
`this.method()` — bypasses annotation decorators; that's a reason to split the class.)

## Template Method → lambdas (usually)

❌ Abstract-class hooks force inheritance, one subclass per variation, and `protected`-field
coupling:

```java
public abstract class AbstractImporter {
    public final ImportResult run(Path file) {
        var rows = parse(file);
        var valid = rows.stream().filter(this::isValid).toList();
        persist(valid);
        return summarize(valid);
    }
    protected abstract boolean isValid(Row row);
    protected abstract void persist(List<Row> rows);
}
```

✅ The skeleton becomes a class; the variation points become injected functions:

```java
@Component
public class Importer {

    public ImportResult run(Path file, Predicate<Row> validation, Consumer<List<Row>> sink) {
        var rows = parse(file);
        var valid = rows.stream().filter(validation).toList();
        sink.accept(valid);
        return summarize(valid);
    }
}

// call site reads as the whole story:
importer.run(file, row -> row.amount().signum() > 0, batchWriter::writeOrders);
```

Keep classic Template Method when the variations are a *closed, named* family — then make the
parent `sealed` and the compiler enforces the family boundary.

## Observer → Spring events

Don't hand-roll listener registries. In-process decoupling:

```java
public record OrderPlacedEvent(OrderId orderId, CustomerId customerId) {}

@Service
public class OrderService {
    private final ApplicationEventPublisher events;

    public OrderService(ApplicationEventPublisher events) { this.events = events; }

    @Transactional
    public Order place(PlaceOrderCommand cmd) {
        Order order = /* ... */;
        events.publishEvent(new OrderPlacedEvent(order.id(), cmd.customerId()));
        return order;
    }
}

@Component
class LoyaltyPointsListener {
    @TransactionalEventListener(phase = TransactionPhase.AFTER_COMMIT)   // only after the order is real
    void on(OrderPlacedEvent event) { /* award points */ }
}
```

`@TransactionalEventListener(AFTER_COMMIT)` is the load-bearing detail — a plain `@EventListener`
fires even if the transaction rolls back. For cross-service events, this is the wrong tool
entirely — route to kafka-event-patterns.

## Anti-pattern: hand-rolled Singleton

```java
// ❌ Never in a Spring service:
public class ConfigHolder {
    private static ConfigHolder INSTANCE;
    public static synchronized ConfigHolder getInstance() { ... }
}
```

Spring beans are singletons *per container* already — with lifecycle management, proxying, and
test-context isolation. A `static INSTANCE` escapes all of that: it leaks state across test
classes, can't be mocked without reflection hacks, and hides the dependency from constructors.
If something must be globally unique, it's a `@Bean`. If it must be unique across *pods*, that's
a distributed-coordination problem (route to designing-systems), not a JVM pattern.
