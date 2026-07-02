---
name: oop-design
description: >
  Use when designing or refactoring Java classes and packages: god classes
  and 500-line services, switch/if-else chains on a type field, SOLID
  violations, deep inheritance hierarchies that should be composition,
  mutable DTOs and shared-state bugs, primitive obsession (String userId,
  BigDecimal money everywhere), train-wreck call chains
  (a.getB().getC().getD()), choosing package-by-feature vs
  package-by-layer, applying design patterns idiomatically in Spring
  (Strategy via injected bean maps, Factory via @Bean, Builder, Decorator)
  or avoiding obsolete ones (hand-rolled Singleton), and designing
  exceptions (checked vs unchecked, domain exception hierarchies). Not for
  test design or TDD workflow — use tdd-java. Not for service boundaries
  and inter-service contracts — use designing-systems. Not for
  Spring-specific layering conventions — use spring-boot-standards.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---

# OOP and Design Quality in Modern Java

## When to use

- A class keeps growing and every feature touches it; merge conflicts cluster in the same files
- Adding a new variant (payment method, document type, country) means editing a switch in N places
- Bugs from state mutated in surprising places, or defensive `null` checks everywhere
- A subclass overrides half its parent's behavior, or `instanceof` chains appear
- Reviewing a design/PR and something feels wrong but needs a name and a fix

## Quick reference

| Problem | Symptom | Solution |
|---|---|---|
| God class | 500+ lines, 8+ dependencies in constructor, name ends in Manager/Util/Helper | Split by responsibility (SRP); each collaborator cluster becomes a class |
| Switch on type code | Same `switch`/`if-else` over an enum repeated in multiple methods | Strategy: one class per variant, dispatched via injected `Map` (example below) |
| Inheritance for reuse | `extends BaseService`, overridden hooks, `protected` field access | Composition: inject the shared behavior as a collaborator |
| Mutable data classes | Setters on DTOs, state changed far from creation | Records for data, `final` fields, defensive copies of collections/dates |
| Primitive obsession | `String customerId`, `String email`, `(BigDecimal, String)` for money | Value objects as records with validation in compact constructor |
| Train wreck | `order.getCustomer().getAddress().getZip()` | Tell, don't ask: `order.shippingZip()`; Demeter — talk to direct collaborators |
| Layer-sliced packages | Change to one feature touches controller/, service/, repository/ | Package-by-feature; layers inside the feature package |
| Exception soup | `throws Exception`, `catch (Exception e)` swallowing, errors as nulls | Unchecked domain exceptions per failure category; one handler at the boundary |

## MUST / MUST NOT

**MUST**

- Give every class one reason to change; if the description of a class needs "and", split it.
- Use records for DTOs and value objects; make entity/service fields `final` and set them in the constructor.
- Prefer composition + interfaces over inheritance for sharing behavior; reserve inheritance for true is-a with stable contracts (or `sealed` hierarchies for closed sums).
- Wrap domain-meaningful primitives in value objects once they acquire rules (validation, formatting, arithmetic).
- Throw unchecked, domain-named exceptions (`InsufficientFundsException`) carrying context fields; translate to transport codes at the boundary only.
- Keep new variants additive: adding a payment method should mean adding a class, not editing existing ones (OCP).

**MUST NOT**

- MUST NOT add a hand-rolled Singleton (`private static INSTANCE`, `getInstance()`) — the Spring container already manages scope; static singletons break testability and hide dependencies.
- MUST NOT expose mutable internals: return `List.copyOf(items)` or unmodifiable views, never the backing collection.
- MUST NOT use inheritance to share utility code (`extends AbstractBaseHelper`) — inject or compose it.
- MUST NOT design APIs around checked exceptions for business failures — they metastasize through signatures; checked is for recoverable conditions the *immediate* caller handles.
- MUST NOT let a "temporary" `instanceof`/type-code switch survive review once there are 3+ variants.
- MUST NOT create interfaces with exactly one implementation "for flexibility" — add the interface when the second implementation (or a test seam need) actually arrives.

## SOLID, operationally

One-line test per principle — full before/after for each in `references/solid-examples.md`:

| Principle | The smell that violates it | The move |
|---|---|---|
| **S**ingle responsibility | Class changes for unrelated reasons (parsing AND persistence AND notification) | Extract one class per reason-to-change |
| **O**pen/closed | New variant = edit existing switch statements | Strategy/polymorphism; new variant = new class |
| **L**iskov substitution | Subclass throws `UnsupportedOperationException`, narrows behavior, or callers `instanceof`-check | Don't inherit; split the interface or compose |
| **I**nterface segregation | Implementers stub half the methods of a fat interface | Split into role interfaces clients actually use |
| **D**ependency inversion | Domain service imports concrete infrastructure (SMTP client, S3 SDK) | Domain defines the port (interface); infrastructure implements it |

## Core pattern: switch-on-type → Strategy via injected bean map

❌ **BAD** — every new payment method edits this class (OCP violation), and the switch is
duplicated wherever fees, validation, or receipts differ by method:

```java
@Service
public class PaymentProcessor {

    public Receipt process(Payment payment) {
        switch (payment.method()) {                       // 2nd copy of this switch lives in FeeService,
            case CREDIT_CARD -> {                         // 3rd in RefundService — they WILL drift
                validateCard(payment);
                chargeCard(payment);
                return cardReceipt(payment);
            }
            case PIX -> {
                validatePixKey(payment);
                transferPix(payment);
                return pixReceipt(payment);
            }
            case BOLETO -> {
                generateBoleto(payment);                  // 600-line class: all methods' logic in one place
                return boletoReceipt(payment);
            }
            default -> throw new IllegalArgumentException("Unknown: " + payment.method());
        }
    }
    // ... 500 more lines of per-method private helpers
}
```

✅ **GOOD** — one class per variant; Spring injects all implementations as a list, the processor
builds the dispatch map once. Adding `APPLE_PAY` = adding one class, zero edits elsewhere:

```java
public interface PaymentHandler {
    PaymentMethod method();                               // the dispatch key
    Receipt process(Payment payment);
}

@Component
class PixPaymentHandler implements PaymentHandler {

    private final PixGateway gateway;

    PixPaymentHandler(PixGateway gateway) {
        this.gateway = gateway;
    }

    @Override
    public PaymentMethod method() {
        return PaymentMethod.PIX;
    }

    @Override
    public Receipt process(Payment payment) {
        gateway.validateKey(payment.pixKey());
        var transferId = gateway.transfer(payment.amount(), payment.pixKey());
        return Receipt.pix(payment.id(), transferId);
    }
}

@Service
public class PaymentProcessor {

    private final Map<PaymentMethod, PaymentHandler> handlers;

    public PaymentProcessor(List<PaymentHandler> handlerBeans) {   // Spring injects ALL implementations
        this.handlers = handlerBeans.stream()
                .collect(Collectors.toUnmodifiableMap(PaymentHandler::method, h -> h));
    }

    public Receipt process(Payment payment) {
        var handler = handlers.get(payment.method());
        if (handler == null) {
            throw new UnsupportedPaymentMethodException(payment.method());
        }
        return handler.process(payment);
    }
}
```

Tradeoff: more classes and one indirection hop in exchange for isolated, independently testable
variants. Below 3 variants with trivial per-variant logic, the switch is honestly simpler — make
the switch exhaustive over a `sealed` interface or enum (no `default`) so the compiler flags new
variants, and convert when the third non-trivial variant lands.

## Immutability by default

```java
// Value object: validation in the compact constructor, arithmetic on the type — not on raw BigDecimal
public record Money(BigDecimal amount, Currency currency) {
    public Money {
        Objects.requireNonNull(amount);
        Objects.requireNonNull(currency);
        if (amount.scale() > currency.getDefaultFractionDigits()) {
            throw new IllegalArgumentException("Scale exceeds " + currency);
        }
    }

    public Money plus(Money other) {
        if (!currency.equals(other.currency)) {
            throw new CurrencyMismatchException(currency, other.currency);
        }
        return new Money(amount.add(other.amount), currency);
    }
}

// Records do NOT deep-copy: defend mutable components yourself
public record OrderSnapshot(OrderId id, List<LineItem> items) {
    public OrderSnapshot {
        items = List.copyOf(items);   // caller's later mutations can't reach in
    }
}
```

Mutation is then a *local, visible* act: changed state means a new object, and "who changed this?"
stops being a debugging question. Reserve mutability for JPA entities (the ORM requires it) and
genuine accumulators — and keep those behind small APIs.

## Package-by-feature vs package-by-layer

Default to **package-by-feature** (`payment/`, `customer/`, each containing its controller,
service, repository, model): a feature change touches one package, package-private visibility
becomes a real modularity tool, and feature boundaries are candidate service boundaries later.

Package-by-layer (`controller/`, `service/`, `repository/`) is acceptable for small services
(< ~5 entities) where layers are the only structure worth having, or when an existing codebase
uses it consistently — local consistency beats global preference; don't mix both in one codebase.
Route whole-system boundary questions to **designing-systems**.

## Patterns in Spring: keep / avoid

| Pattern | Verdict | Idiomatic form (details: references/patterns-in-spring.md) |
|---|---|---|
| Strategy | Keep | Injected `List<Handler>` → `Map` dispatch (above) |
| Factory | Keep | `@Bean`/`@Configuration` methods; explicit factory class when creation needs runtime data |
| Builder | Keep | For 4+ optional construction params; records + builder for big configs |
| Decorator | Keep | Wrap an interface to layer caching/metrics/retry without touching the core impl |
| Template Method | Mostly replace | Prefer passing a lambda/`Function` over abstract-class hooks; keep for `sealed` hierarchies |
| Observer | Replaced | `ApplicationEventPublisher` + `@EventListener` |
| Singleton (GoF) | Avoid | The container scopes beans; `static INSTANCE` is a testing and lifecycle bug |

## Verification

Design quality has no single command, but these catch regressions:

```bash
mvn verify                                   # Gradle: ./gradlew check — includes ArchUnit/static analysis if configured
grep -rln "getInstance()" src/main/java      # hand-rolled singletons
grep -rn "extends .*\(Base\|Abstract\).*\(Service\|Helper\|Util\)" src/main/java   # inheritance-for-reuse
grep -rc "public void set" src/main/java/**/dto 2>/dev/null                        # mutable DTOs
```

What failure looks like: grep hits are review flags, not auto-fails — open each and judge against
the MUST NOT list. If the project has ArchUnit tests (layer/package dependency rules) and one
fails after your refactor, the rule encodes an agreed boundary: change the code, or change the
rule in its own reviewed commit — never both silently. After any refactor in this skill, the full
test suite must stay green with **no test edits** beyond moved imports/names; behavior changes
disguised as refactoring are the costliest design bug.

## References

| File | Contents | When to load |
|---|---|---|
| references/solid-examples.md | Full before/after Java code for each SOLID principle | Explaining or fixing a specific principle violation |
| references/patterns-in-spring.md | Complete idiomatic implementations: Strategy, Factory, Builder, Decorator, Template-Method-vs-lambda, events | Implementing a pattern in a Spring service |
| references/refactoring-catalog.md | Smell → refactoring move table with short examples (extract class, replace conditional with polymorphism, introduce parameter object…) | Naming the smell in a review and picking the move |

## Related skills

- **tdd-java** — test-first workflow and test design; refactors here must ride on its green-suite discipline.
- **spring-boot-standards** — Spring layering, configuration, and API conventions; this skill is framework-agnostic design.
- **designing-systems** — boundaries between services; this skill stops at the edge of one codebase.
- **reviewing-java-code** — the review process itself; pull this skill in when the finding is a design smell.
- **jpa-database-patterns** — entity design constraints (why entities stay mutable) and persistence performance.
