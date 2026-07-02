---
name: hexagonal-domain-isolation
description: This skill should be used when the user asks about "domain isolation", "anti-corruption layer", mentions "keeping domain pure", discusses preventing infrastructure leakage into business logic, or wants to protect the domain model from external concerns.
version: 1.0.0
---

# Hexagonal Architecture: Domain Isolation

Protect the domain model from infrastructure, framework, and external system concerns by enforcing strict boundaries that keep business logic pure and independent.

## When This Skill Applies
- Domain objects contain serialization annotations, ORM decorators, or framework dependencies
- Business logic changes when an external API contract changes
- The domain model mirrors a database schema instead of business concepts
- The user mentions domain isolation, anti-corruption layer, or clean domain
- Framework upgrades require modifying business logic

## Core Principle
The domain model represents business concepts and rules in their purest form. It must not be contaminated by infrastructure concerns like database schemas, serialization formats, API contracts, or framework conventions. An anti-corruption layer (ACL) translates between the domain's language and external systems' languages, ensuring the domain remains stable even when external systems change.

## Workflow

### Step 1: Audit Domain Dependencies
Check what the domain module imports. Any import from an infrastructure, framework, or external library is a boundary violation. The domain should depend only on the language's standard library and its own types.

### Step 2: Identify Contamination
Look for: ORM annotations on domain entities, JSON serialization attributes, framework base classes, external API types used in domain logic, database column names influencing domain field names.

### Step 3: Introduce Translation Layers
Create separate models for each boundary:
- **Domain model**: pure business objects.
- **Persistence model**: ORM entities that map to database tables.
- **API model**: DTOs that match external API contracts.

Mappers translate between these models at the boundary.

### Step 4: Build the Anti-Corruption Layer
For external systems whose model differs from yours, create an ACL that translates their concepts into your domain language. The ACL adapter implements a domain port.

### Step 5: Verify Isolation
The domain module should compile without any infrastructure dependency. Running domain tests should require no database, network, or framework. Run the test suite.

## Detection / Indicators
- Domain entities with `@Entity`, `@Column`, `@JsonProperty` annotations
- Domain objects that extend framework base classes
- Business logic that references table names, column names, or API field names
- Domain methods that accept or return HTTP request/response objects
- Importing ORM, HTTP, or serialization libraries in domain files
- Domain field names that match database column names but not business terminology

## Transformation Pattern

**Before (contaminated domain):**
```
@Entity()
class Order {
  @Column()
  order_total: number    // database naming leaks in

  @JsonProperty('cust_email')
  customerEmail: string  // API naming leaks in

  save() { return db.save(this) }  // persistence in domain
}
```

**After (isolated domain):**
```
// Domain — pure
class Order {
  total: Money
  customerEmail: EmailAddress

  applyDiscount(discount: Percentage) {
    this.total = this.total.reduce(discount)
  }
}

// Persistence model — separate
@Entity()
class OrderEntity {
  @Column() order_total: number
  @Column() cust_email: string
}

// Mapper
class OrderMapper {
  toDomain(entity: OrderEntity): Order { ... }
  toEntity(order: Order): OrderEntity { ... }
}
```

## Common Pitfalls
- Sharing a single model across all layers ("it's simpler" until the first schema migration)
- Mapping overhead for trivial CRUD apps (domain isolation is most valuable for complex domains)
- Anti-corruption layers that grow into a second domain model (keep them thin)
- Domain objects that know about their own persistence lifecycle
- Leaking database-generated IDs into domain identity

## Additional Resources
### Reference Files
- **`references/acl-patterns.md`** — Anti-corruption layer patterns and mapping strategies
