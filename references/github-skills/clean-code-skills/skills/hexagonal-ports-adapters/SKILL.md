---
name: hexagonal-ports-adapters
description: This skill should be used when the user asks about "ports and adapters", "hexagonal architecture", mentions "clean architecture boundaries", discusses separating infrastructure from domain, or wants to structure code with clear system boundaries.
version: 1.0.0
---

# Hexagonal Architecture: Ports and Adapters

Structure applications around a technology-independent domain core by defining ports (interfaces the domain exposes or requires) and adapters (implementations that connect to specific technologies).

## When This Skill Applies
- Business logic is tangled with database, HTTP, or messaging code
- Swapping a technology (database, queue, API provider) requires rewriting business logic
- The user mentions hexagonal architecture, ports and adapters, or clean architecture
- Testing business logic requires spinning up infrastructure
- The codebase has no clear boundary between domain and infrastructure

## Core Principle
The domain sits at the center and defines ports — interfaces that describe what it needs from the outside world (driven ports) and what it offers to the outside world (driving ports). Adapters implement these ports for specific technologies. The domain never imports infrastructure code. All dependencies point inward.

## Workflow

### Step 1: Identify the Domain Core
Determine what the application does independent of any technology. Business rules, validations, calculations, and state machines belong in the domain.

### Step 2: Define Driving Ports (Inbound)
These are the use cases the application offers. Define them as interfaces or service methods: `PlaceOrder`, `RegisterUser`, `GenerateReport`. External actors (HTTP controllers, CLI, message consumers) call these.

### Step 3: Define Driven Ports (Outbound)
These are the dependencies the domain needs. Define them as interfaces: `OrderRepository`, `PaymentGateway`, `NotificationSender`. The domain calls these; infrastructure implements them.

### Step 4: Implement Adapters
Create adapters for each port:
- **Driving adapters** (inbound): HTTP controllers, GraphQL resolvers, CLI handlers, message consumers.
- **Driven adapters** (outbound): PostgreSQL repository, Stripe payment gateway, SMTP email sender.

### Step 5: Wire at the Composition Root
Connect adapters to ports at the application entry point. The domain never knows which concrete adapter is connected.

## Detection / Indicators
- Domain classes import `express`, `pg`, `axios`, or other infrastructure libraries
- Business logic lives inside controller or handler functions
- Changing a database query requires modifying business rules
- Unit testing domain logic requires a database connection
- No clear separation between "what the app does" and "how it connects to the world"

## Transformation Pattern

**Before (tangled):**
```
class OrderController {
  async placeOrder(req, res) {
    const items = req.body.items
    const total = items.reduce((s, i) => s + i.price, 0)
    if (total < 10) return res.status(400).json({ error: 'Minimum order is $10' })
    await pg.query('INSERT INTO orders ...')
    await smtp.send(req.body.email, 'Order confirmed')
    res.json({ status: 'ok' })
  }
}
```

**After (hexagonal):**
```
// Domain — port definitions
interface OrderRepository { save(order: Order): void }
interface NotificationSender { sendConfirmation(email: string, order: Order): void }

class PlaceOrderUseCase {
  constructor(private repo: OrderRepository, private notifier: NotificationSender) {}
  execute(items, customerEmail) {
    const order = Order.create(items)  // domain validates minimum
    this.repo.save(order)
    this.notifier.sendConfirmation(customerEmail, order)
    return order
  }
}

// Adapters
class PostgresOrderRepository implements OrderRepository { ... }
class SmtpNotificationSender implements NotificationSender { ... }
class OrderHttpController {
  constructor(private useCase: PlaceOrderUseCase) {}
  async handle(req, res) {
    const order = this.useCase.execute(req.body.items, req.body.email)
    res.json({ status: 'ok', orderId: order.id })
  }
}
```

## Common Pitfalls
- Placing port interfaces in the infrastructure layer (they belong to the domain)
- Creating adapters that contain business logic
- Over-engineering: not every CRUD app needs full hexagonal architecture
- Forgetting the composition root (dependency wiring must happen somewhere)
- Domain objects that serialize themselves (serialization is an adapter concern)

## Additional Resources
### Reference Files
- **`references/port-adapter-anatomy.md`** — Anatomy of ports and adapters with directory structure examples
