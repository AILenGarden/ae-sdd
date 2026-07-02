# SOLID: Full Before/After Examples

One complete violation → fix pair per principle, Java 21 / Spring 3.x idiom.

## S — Single Responsibility

**Violation** — `InvoiceService` changes when tax rules change, when the PDF layout changes, and
when the email provider changes. Three teams edit one file:

```java
@Service
public class InvoiceService {

    public void issue(Order order) {
        BigDecimal tax = order.total().multiply(taxRateFor(order.country())); // tax rules
        byte[] pdf = renderPdf(order, tax);                                   // presentation
        smtpClient.send(order.customerEmail(), "Your invoice", pdf);          // delivery
        invoiceRepository.save(new Invoice(order.id(), tax));                 // persistence
    }

    private BigDecimal taxRateFor(String country) { /* 80 lines of rules */ }
    private byte[] renderPdf(Order order, BigDecimal tax) { /* 120 lines of layout */ }
}
```

**Fix** — one class per reason-to-change; the service becomes a thin orchestrator:

```java
@Service
public class InvoiceService {

    private final TaxCalculator taxCalculator;        // changes with tax law
    private final InvoiceRenderer renderer;           // changes with layout
    private final InvoiceDelivery delivery;           // changes with provider
    private final InvoiceRepository repository;

    public InvoiceService(TaxCalculator taxCalculator, InvoiceRenderer renderer,
                          InvoiceDelivery delivery, InvoiceRepository repository) {
        this.taxCalculator = taxCalculator;
        this.renderer = renderer;
        this.delivery = delivery;
        this.repository = repository;
    }

    public void issue(Order order) {
        Money tax = taxCalculator.taxFor(order);
        Invoice invoice = repository.save(Invoice.of(order, tax));
        delivery.deliver(invoice, renderer.render(invoice));
    }
}
```

Each collaborator is now unit-testable alone, and tax-law changes can't break PDF rendering.

## O — Open/Closed

**Violation** — adding a notification channel edits this method (and its tests, and everyone's
in-flight branches):

```java
public void notify(User user, Alert alert) {
    if (user.prefersEmail()) {
        emailClient.send(user.email(), alert.subject(), alert.body());
    } else if (user.prefersSms()) {
        smsClient.send(user.phone(), alert.shortBody());
    } else if (user.prefersPush()) {           // added last sprint
        pushClient.push(user.deviceToken(), alert.shortBody());
    }                                          // WhatsApp arrives next sprint...
}
```

**Fix** — new channel = new class; the dispatcher never changes:

```java
public interface NotificationChannel {
    boolean supports(User user);
    void send(User user, Alert alert);
}

@Component
class PushChannel implements NotificationChannel {
    private final PushClient pushClient;

    PushChannel(PushClient pushClient) { this.pushClient = pushClient; }

    @Override public boolean supports(User user) { return user.prefersPush(); }
    @Override public void send(User user, Alert alert) {
        pushClient.push(user.deviceToken(), alert.shortBody());
    }
}

@Service
public class Notifier {
    private final List<NotificationChannel> channels;   // Spring injects every implementation

    public Notifier(List<NotificationChannel> channels) { this.channels = channels; }

    public void notify(User user, Alert alert) {
        channels.stream()
                .filter(c -> c.supports(user))
                .findFirst()
                .orElseThrow(() -> new NoChannelConfiguredException(user.id()))
                .send(user, alert);
    }
}
```

## L — Liskov Substitution

**Violation** — `ReadOnlyAccount` "is-a" `Account` but breaks the contract; every caller now
needs `instanceof` checks, which defeats polymorphism entirely:

```java
public class Account {
    public void withdraw(Money amount) { balance = balance.minus(amount); }
}

public class ReadOnlyAccount extends Account {
    @Override
    public void withdraw(Money amount) {
        throw new UnsupportedOperationException("read-only");   // LSP violation: narrows behavior
    }
}
```

**Fix** — model the capabilities as separate interfaces; a type only promises what it honors:

```java
public interface AccountView {
    Money balance();
    AccountId id();
}

public interface WithdrawableAccount extends AccountView {
    void withdraw(Money amount);
}

// Transfer logic demands the capability in its signature — no instanceof, no runtime surprise:
public void transfer(WithdrawableAccount from, AccountId to, Money amount) { ... }
```

Heuristic: if a subclass throws `UnsupportedOperationException`, returns degenerate values, or
strengthens preconditions ("only works if amount < 100"), the hierarchy is wrong — split the
interface or compose.

## I — Interface Segregation

**Violation** — one fat port; the S3 implementation stubs half of it, and every new method forces
all implementers to change:

```java
public interface DocumentStore {
    void save(Document doc);
    Document load(DocumentId id);
    void delete(DocumentId id);
    List<Document> search(Query query);        // S3 impl: throws — "use OpenSearch for that"
    void subscribeToChanges(Listener l);       // only the in-memory impl ever implemented this
}
```

**Fix** — role interfaces sized to actual clients:

```java
public interface DocumentReader {
    Document load(DocumentId id);
}

public interface DocumentWriter {
    void save(Document doc);
    void delete(DocumentId id);
}

public interface DocumentSearch {
    List<Document> search(Query query);
}

// Implementations pick what they honestly support:
@Component
class S3DocumentStore implements DocumentReader, DocumentWriter { ... }

@Component
class OpenSearchIndex implements DocumentSearch { ... }

// Consumers declare ONLY what they use — and the dependency graph documents reality:
@Service
public class DocumentExportService {
    private final DocumentReader reader;       // can't accidentally delete anything

    public DocumentExportService(DocumentReader reader) { this.reader = reader; }
}
```

## D — Dependency Inversion

**Violation** — domain logic imports infrastructure; the fraud rules can't be tested without an
SDK, and swapping vendors rewrites the domain:

```java
@Service
public class FraudCheckService {

    private final AwsSnsClient snsClient;                 // infrastructure type in domain code
    private final RestClient scoringVendorClient;         // concrete vendor baked in

    public FraudDecision check(Payment payment) {
        var score = scoringVendorClient.post()
                .uri("/v2/score").body(VendorRequest.from(payment))   // vendor DTO in domain
                .retrieve().body(VendorScore.class);
        if (score.value() > 700) {
            snsClient.publish("fraud-alerts", payment.id().toString());
            return FraudDecision.block(payment.id());
        }
        return FraudDecision.allow(payment.id());
    }
}
```

**Fix** — the domain defines ports in its own vocabulary; infrastructure implements them at the
edge:

```java
// Domain-owned ports (live in the domain package, no framework/vendor imports):
public interface FraudScorer {
    FraudScore score(Payment payment);
}

public interface FraudAlertPublisher {
    void alert(PaymentId paymentId);
}

@Service
public class FraudCheckService {
    private final FraudScorer scorer;
    private final FraudAlertPublisher alerts;

    public FraudCheckService(FraudScorer scorer, FraudAlertPublisher alerts) {
        this.scorer = scorer;
        this.alerts = alerts;
    }

    public FraudDecision check(Payment payment) {
        if (scorer.score(payment).isHighRisk()) {
            alerts.alert(payment.id());
            return FraudDecision.block(payment.id());
        }
        return FraudDecision.allow(payment.id());
    }
}

// Infrastructure adapter — vendor details quarantined here:
@Component
class VendorXFraudScorer implements FraudScorer {
    private final RestClient vendorClient;

    VendorXFraudScorer(RestClient vendorXRestClient) { this.vendorClient = vendorXRestClient; }

    @Override
    public FraudScore score(Payment payment) {
        var response = vendorClient.post()
                .uri("/v2/score").body(VendorRequest.from(payment))
                .retrieve().body(VendorScore.class);
        return new FraudScore(response.value());
    }
}
```

The domain test is now a plain JUnit test with two fakes — no SDK, no HTTP, no Spring context.
Tradeoff: one interface + adapter per dependency. Apply it at *volatile* boundaries (vendors,
messaging, storage); don't wrap stable JDK types in ports for ceremony's sake.
