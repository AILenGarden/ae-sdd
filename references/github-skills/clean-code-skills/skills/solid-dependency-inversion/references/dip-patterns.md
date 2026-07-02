# Dependency Inversion Patterns

## Pattern 1: Constructor Injection

Pass dependencies through the constructor. The most common and recommended pattern.

```
class NotificationService {
  constructor(
    private emailSender: EmailSender,
    private templateEngine: TemplateEngine
  ) {}

  notify(user, event) {
    const body = this.templateEngine.render(event.template, event.data)
    this.emailSender.send(user.email, body)
  }
}
```

**Testing:**
```
const mockSender = { send: jest.fn() }
const mockEngine = { render: () => 'Hello' }
const service = new NotificationService(mockSender, mockEngine)
```

## Pattern 2: Interface Ownership

The interface belongs to the consumer (high-level module), not the provider (low-level module).

```
// domain/ports/OrderRepository.ts (owned by domain)
interface OrderRepository {
  save(order: Order): void
  findById(id: string): Order
}

// infrastructure/PostgresOrderRepository.ts (implements domain interface)
import { OrderRepository } from '../domain/ports/OrderRepository'

class PostgresOrderRepository implements OrderRepository { ... }
```

**Key insight:** The domain module never imports from infrastructure. Infrastructure imports from domain.

## Pattern 3: Composition Root

Wire all dependencies at the application entry point.

```
// main.ts — the composition root
const db = new PostgresClient(config)
const orderRepo = new PostgresOrderRepository(db)
const emailSender = new SmtpEmailSender(smtpConfig)
const orderService = new OrderService(orderRepo, emailSender)

app.post('/orders', (req, res) => {
  orderService.placeOrder(req.body)
})
```

**Rule:** Only the composition root knows about concrete types. All other modules depend on abstractions.

## Pattern 4: Factory Function

Use a factory when the concrete type must be selected at runtime.

```
interface PaymentGateway { charge(amount): Receipt }

function createPaymentGateway(config): PaymentGateway {
  if (config.provider === 'stripe') return new StripeGateway(config.apiKey)
  if (config.provider === 'paypal') return new PaypalGateway(config.clientId)
  throw new Error(`Unknown provider: ${config.provider}`)
}
```

## When NOT to Invert

Not every dependency needs inversion. Avoid inverting:

| Dependency Type | Invert? | Reason |
|----------------|---------|--------|
| Standard library (Math, String) | No | Stable, never changes |
| Value objects | No | Simple, no side effects |
| Domain entities | No | Same layer, same change rate |
| Infrastructure (DB, HTTP, FS) | Yes | Changes independently |
| External APIs | Yes | Unstable, controlled by third parties |
| Framework code | Maybe | Depends on coupling level |

## Dependency Direction Rule

```
Domain ← Application ← Infrastructure
              ↓
         Interfaces (Ports)
```

- Domain defines interfaces (ports).
- Infrastructure implements them (adapters).
- Application wires them together.
- Arrows point inward: infrastructure depends on domain, never the reverse.
