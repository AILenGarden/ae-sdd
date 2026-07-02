# Refactoring Catalog: Smell → Move

Name the smell, pick the move. Every move assumes a green test suite before and after
(tdd-java owns that discipline). Examples are deliberately short — the shape, not the ceremony.

## The table

| Smell | How you spot it | Refactoring move |
|---|---|---|
| God class | 500+ lines, 8+ constructor params, plural responsibilities in its name/docs | Extract Class — one per reason-to-change |
| Long method | Scrolling to read it; comments as section headers | Extract Method — each comment block becomes a named method |
| Switch on type code | Same `switch`/`if-else` over a type in 2+ places | Replace Conditional with Polymorphism (Strategy bean map) |
| Primitive obsession | `String customerId`, `String iban`, money as bare `BigDecimal` | Introduce Value Object (record + compact-constructor validation) |
| Long parameter list | 4+ params, callers mix up argument order | Introduce Parameter Object (a record) |
| Data clumps | Same 3 fields travel together through signatures | Same — extract the clump into a record |
| Feature envy | Method reads 3+ getters of another object to compute something | Move Method to the data's class |
| Train wreck (Demeter) | `a.getB().getC().getD()` | Tell-don't-ask: add the question to the direct collaborator |
| Inheritance for reuse | `extends BaseX` just to call protected helpers | Replace Inheritance with Delegation (inject the helper) |
| Boolean flag parameter | `process(order, true, false)` | Split Method (two named methods) or enum parameter |
| Null as a domain answer | Callers null-check before every use | Return `Optional`, empty collection, or a Null-Object |
| Swallowed exception | `catch (Exception e) { log.warn(...); }` then continue | Translate to a domain exception or handle meaningfully; never both log-and-rethrow per layer |
| Dead flexibility | Interface with one impl forever, unused generics, "just in case" hooks | Inline/delete — re-introduce when the second case is real |
| Temporal coupling | Must call `init()` before `run()` or it NPEs | Constructor injection of full state; make invalid states unrepresentable |

## Worked micro-examples

**Extract Method (long method):**

```java
// before: one 60-line process() with comment headers
// --- validate ---  ... 15 lines
// --- enrich ---    ... 20 lines
// --- persist ---   ... 15 lines

// after: the method IS the table of contents
public Receipt process(Payment payment) {
    validate(payment);
    EnrichedPayment enriched = enrich(payment);
    return persist(enriched);
}
```

**Introduce Parameter Object:**

```java
// before — callers regularly swap from/to:
List<Trip> findTrips(String origin, String destination, LocalDate from, LocalDate to, int limit)

// after:
record TripQuery(City origin, City destination, DateRange range, int limit) {}
List<Trip> findTrips(TripQuery query)
```

**Move Method (feature envy):**

```java
// before — PricingService envies Order's data:
BigDecimal discount = order.getItems().stream()
        .filter(i -> i.getCategory() == Category.BOOK)
        .map(LineItem::getPrice)
        .reduce(BigDecimal.ZERO, BigDecimal::add)
        .multiply(BOOK_DISCOUNT_RATE);

// after — Order answers questions about itself:
Money discount = order.bookSubtotal().times(BOOK_DISCOUNT_RATE);
```

**Tell-don't-ask (Demeter):**

```java
// before — caller knows Order -> Customer -> Address -> zip:
String zip = order.getCustomer().getAddress().getZip();
shippingService.quote(zip, order.weight());

// after — ask the direct collaborator:
shippingService.quote(order.shippingZip(), order.weight());
```

Demeter scope note: the law applies to object *structure*, not fluent APIs — builder chains and
Stream pipelines return "the same conversation" and are fine.

**Replace Inheritance with Delegation:**

```java
// before:
public class ReportService extends AbstractCsvHelper { ... uses inherited toCsvLine() ... }

// after:
public class ReportService {
    private final CsvFormatter csv;
    public ReportService(CsvFormatter csv) { this.csv = csv; }
}
```

**Null Object / Optional:**

```java
// before:
Discount d = discountRepository.findForCustomer(id);   // returns null
if (d != null && d.isActive()) { ... }

// after — repository returns the absence as a value:
Discount d = discountRepository.findForCustomer(id).orElse(Discount.NONE);
price = d.apply(price);                                 // NONE applies zero discount; no branch
```

## Exception design moves

| Smell | Move |
|---|---|
| `throws Exception` on public APIs | Define an unchecked base (`extends RuntimeException`) per module: `BillingException` |
| Checked exceptions tunneled as `RuntimeException(e)` everywhere | Make the hierarchy unchecked at the source; checked only where the immediate caller truly recovers |
| Error codes / booleans returned for failures | Throw domain exceptions with context fields (`InsufficientFundsException(accountId, requested, available)`) |
| HTTP statuses decided deep in services | Services throw domain exceptions; ONE `@RestControllerAdvice` maps them to statuses |
| Same error logged at 4 stack levels | Log once, at the boundary that handles it; intermediate layers just let it fly |

```java
// Domain exception with context — the catch block needs no string parsing:
public class InsufficientFundsException extends BillingException {
    private final AccountId accountId;
    private final Money requested;
    private final Money available;

    public InsufficientFundsException(AccountId accountId, Money requested, Money available) {
        super("Insufficient funds in %s: requested %s, available %s"
                .formatted(accountId, requested, available));
        this.accountId = accountId;
        this.requested = requested;
        this.available = available;
    }
    // accessors used by the boundary handler to build the API error body
}
```

## Sequencing rules for any of the above

1. Green suite before starting; refactor and behavior change never share a commit.
2. Smallest move first: Extract Method before Extract Class; Extract Class before introducing a pattern.
3. Lean on the IDE's automated refactorings (rename, extract, move, inline) — they preserve semantics; hand-edits introduce typo-bugs.
4. One smell per commit, named in the message ("Extract TaxCalculator from InvoiceService — SRP").
5. If tests must change beyond imports/names, stop: either tests were coupled to internals (fix that first) or this isn't a refactoring — it's a behavior change wearing a costume.
