---
name: solid-dependency-inversion
description: This skill should be used when the user asks about "dependency inversion", "DIP", mentions "depending on abstractions", discusses decoupling from implementations, or wants to make high-level modules independent of low-level details.
version: 1.0.0
---

# SOLID: Dependency Inversion Principle

Ensure that high-level modules do not depend on low-level modules — both should depend on abstractions. Abstractions should not depend on details; details should depend on abstractions.

## When This Skill Applies
- High-level business logic imports database drivers, HTTP clients, or file system modules directly
- Changing an infrastructure detail requires modifying business logic
- Unit testing requires complex mocking of concrete dependencies
- The user mentions DIP, dependency inversion, or decoupling
- A module cannot be reused because it is coupled to specific infrastructure

## Core Principle
Dependency Inversion flips the traditional dependency direction. Instead of high-level modules calling low-level modules directly, both depend on a shared abstraction (interface). The high-level module defines the interface it needs; the low-level module implements it. This makes the high-level module stable and reusable, while infrastructure details become swappable.

## Workflow

### Step 1: Identify the Dependency Direction
Draw the dependency arrows. If a domain/business class imports a concrete infrastructure class (database, HTTP, filesystem), the dependency points in the wrong direction.

### Step 2: Define an Abstraction
Create an interface that the high-level module needs. Name it from the domain perspective: `OrderRepository`, not `PostgresClient`. The interface belongs to the high-level module's package.

### Step 3: Implement the Abstraction
The low-level module implements the interface. The implementation lives in the infrastructure layer and imports the interface from the domain layer.

### Step 4: Inject the Dependency
Pass the implementation to the high-level module through its constructor or a factory. The high-level module never creates the low-level dependency directly.

### Step 5: Verify
The high-level module should compile and be testable without any infrastructure dependency. Tests use a simple in-memory or stub implementation.

## Detection / Indicators
- Business logic classes that import `pg`, `mysql`, `axios`, `fs` directly
- Constructor creates its own dependencies: `this.db = new PostgresClient()`
- Cannot test a class without a running database or network
- Changing from PostgreSQL to MongoDB requires editing business logic
- Domain classes import framework-specific types

## Transformation Pattern

**Before (dependency on concretion):**
```
class OrderService {
  constructor() {
    this.db = new PostgresClient()  // direct dependency on low-level module
  }
  placeOrder(order) {
    this.db.query('INSERT INTO orders ...')
  }
}
```

**After (dependency on abstraction):**
```
interface OrderRepository {
  save(order: Order): void
}

class OrderService {
  constructor(private repository: OrderRepository) {}  // depends on abstraction
  placeOrder(order) {
    this.repository.save(order)
  }
}

class PostgresOrderRepository implements OrderRepository {
  save(order) { /* SQL implementation */ }
}
```

## Common Pitfalls
- Placing the interface in the infrastructure layer instead of the domain layer
- Creating too many trivial interfaces for stable dependencies (no need to abstract `Math.sqrt`)
- Over-using dependency injection frameworks — constructor injection is usually sufficient
- Not inverting: creating an interface but still importing the concrete class
- Interface that mirrors the implementation API instead of the domain need

## Additional Resources
### Reference Files
- **`references/dip-patterns.md`** — Dependency inversion patterns and composition root strategies
