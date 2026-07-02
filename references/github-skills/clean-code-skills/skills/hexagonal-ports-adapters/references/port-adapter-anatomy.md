# Port and Adapter Anatomy

## Conceptual Model

```
                    [Driving Adapter]
                         |
                    [Driving Port]
                         |
                   ┌─────────────┐
                   │   Domain     │
                   │   Core       │
                   └─────────────┘
                         |
                    [Driven Port]
                         |
                    [Driven Adapter]
```

## Port Types

### Driving Ports (Inbound / Primary)
Expose what the application can do. Defined as use case interfaces.

| Port | Description |
|------|-------------|
| `PlaceOrder` | Accept and validate a new order |
| `RegisterUser` | Create a new user account |
| `GenerateReport` | Produce a report for a date range |

### Driven Ports (Outbound / Secondary)
Declare what the application needs from the outside world. Defined as repository or gateway interfaces.

| Port | Description |
|------|-------------|
| `OrderRepository` | Persist and retrieve orders |
| `PaymentGateway` | Charge a payment method |
| `EmailSender` | Send transactional emails |
| `Clock` | Get current time (testable) |

## Adapter Types

### Driving Adapters (Inbound)
Translate external input into domain calls.

| Adapter | Technology | Drives |
|---------|-----------|--------|
| `OrderHttpController` | Express/Fastify | `PlaceOrder` |
| `OrderCliCommand` | Commander/yargs | `PlaceOrder` |
| `OrderMessageConsumer` | RabbitMQ/Kafka | `PlaceOrder` |

### Driven Adapters (Outbound)
Implement domain ports using specific technologies.

| Adapter | Technology | Implements |
|---------|-----------|------------|
| `PostgresOrderRepository` | PostgreSQL | `OrderRepository` |
| `InMemoryOrderRepository` | Array/Map | `OrderRepository` |
| `StripePaymentGateway` | Stripe SDK | `PaymentGateway` |
| `SmtpEmailSender` | Nodemailer | `EmailSender` |

## Directory Structure

```
src/
├── domain/
│   ├── model/
│   │   ├── Order.ts
│   │   ├── Customer.ts
│   │   └── Money.ts
│   ├── ports/
│   │   ├── inbound/
│   │   │   ├── PlaceOrder.ts
│   │   │   └── CancelOrder.ts
│   │   └── outbound/
│   │       ├── OrderRepository.ts
│   │       ├── PaymentGateway.ts
│   │       └── EmailSender.ts
│   └── services/
│       └── PlaceOrderUseCase.ts
├── adapters/
│   ├── inbound/
│   │   ├── http/
│   │   │   └── OrderController.ts
│   │   └── cli/
│   │       └── OrderCommand.ts
│   └── outbound/
│       ├── persistence/
│       │   ├── PostgresOrderRepository.ts
│       │   └── InMemoryOrderRepository.ts
│       ├── payment/
│       │   └── StripePaymentGateway.ts
│       └── email/
│           └── SmtpEmailSender.ts
└── main.ts  ← composition root
```

## Dependency Rules

1. **Domain imports nothing** from adapters or frameworks.
2. **Adapters import from domain** (port interfaces, domain model).
3. **Composition root imports everything** and wires them together.
4. **Port interfaces live in the domain** — they are owned by the domain, not by the adapter.

## Testing Strategy

| Layer | Test Type | Dependencies |
|-------|-----------|-------------|
| Domain | Unit tests | None (pure logic) |
| Use cases | Unit tests | In-memory adapters |
| Adapters | Integration tests | Real infrastructure |
| Full system | E2E tests | Everything |
