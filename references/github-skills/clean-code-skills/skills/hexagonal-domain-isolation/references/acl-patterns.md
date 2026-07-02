# Anti-Corruption Layer Patterns

## What Is an Anti-Corruption Layer (ACL)?

An ACL is a translation boundary between your domain and an external system. It prevents the external system's model, naming, and conventions from leaking into your domain.

```
[External System] ←→ [ACL: Adapter + Mapper] ←→ [Domain]
```

## Pattern 1: Adapter + Mapper

The adapter implements a domain port. The mapper translates between models.

```
// Domain port
interface InventoryService {
  checkStock(productId: ProductId): StockLevel
}

// External API returns different model
// { "sku": "ABC-123", "qty_available": 42, "warehouse_code": "US-WEST" }

// ACL
class ExternalInventoryAdapter implements InventoryService {
  constructor(private client: ExternalInventoryClient) {}

  checkStock(productId: ProductId): StockLevel {
    const response = this.client.getInventory(productId.toSku())
    return InventoryMapper.toDomain(response)
  }
}

class InventoryMapper {
  static toDomain(response): StockLevel {
    return new StockLevel(response.qty_available, Warehouse.from(response.warehouse_code))
  }
}
```

## Pattern 2: Separate Read/Write Models

Use different models for reading and writing at boundaries.

```
// Inbound: API request DTO → domain command
class CreateOrderRequest {          // API model
  items: { sku: string, qty: number }[]
  customer_email: string
}

class CreateOrderCommand {          // Domain command
  items: OrderItem[]
  customerEmail: EmailAddress
}

function toCommand(request: CreateOrderRequest): CreateOrderCommand {
  return new CreateOrderCommand(
    request.items.map(i => new OrderItem(ProductId.from(i.sku), Quantity.of(i.qty))),
    EmailAddress.of(request.customer_email)
  )
}

// Outbound: domain entity → persistence entity
class OrderPersistenceMapper {
  toRow(order: Order): OrderRow { ... }
  toDomain(row: OrderRow): Order { ... }
}
```

## Pattern 3: Facade for Complex External Systems

Simplify a complex external API by exposing only what your domain needs.

```
// Complex third-party API
class ThirdPartyShippingApi {
  createShipment(data) { ... }
  addPackage(shipmentId, pkg) { ... }
  setOrigin(shipmentId, addr) { ... }
  setDestination(shipmentId, addr) { ... }
  calculateRate(shipmentId) { ... }
  confirmShipment(shipmentId) { ... }
}

// Your simplified facade
class ShippingAdapter implements ShippingService {
  getQuote(origin, destination, packages): ShippingQuote {
    const id = this.api.createShipment({})
    this.api.setOrigin(id, this.mapAddress(origin))
    this.api.setDestination(id, this.mapAddress(destination))
    packages.forEach(p => this.api.addPackage(id, this.mapPackage(p)))
    const rate = this.api.calculateRate(id)
    return new ShippingQuote(Money.of(rate.total), rate.estimatedDays)
  }
}
```

## Mapping Strategies

| Strategy | When to Use |
|----------|-------------|
| Manual mapping | Simple models, few fields, full control |
| Extension functions | Language supports them (Kotlin, C#), keeps mapper close to type |
| Mapping library | Many models with similar structure, less boilerplate |
| Constructor mapping | Target object validates during construction |

## Boundary Rules

1. External types never cross the domain boundary.
2. Domain types never cross the infrastructure boundary outward.
3. Each boundary has its own model (even if similar).
4. Mappers live on the infrastructure side, not inside the domain.
5. The domain defines the vocabulary; external systems adapt to it.
