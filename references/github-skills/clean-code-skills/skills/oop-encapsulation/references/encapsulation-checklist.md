# Encapsulation Checklist

## Field-Level Checks

- [ ] Are all fields private (or at least protected with justification)?
- [ ] Does every setter validate its input and enforce invariants?
- [ ] Are mutable collections defensively copied before returning?
- [ ] Can an external caller put the object into an invalid state?

## Behavior-Level Checks

- [ ] Does the class expose behavior (methods that do something) rather than just data (getters)?
- [ ] Is decision-making logic inside the class, not scattered across callers?
- [ ] Do method names describe actions, not data access? (`withdraw` not `setBalance`)
- [ ] Does the class enforce its own business rules internally?

## Design Smells

| Smell | Encapsulation Fix |
|-------|-------------------|
| Feature Envy | Move the logic into the class that owns the data |
| Anemic Domain Model | Add behavior methods, remove getters where possible |
| Data Clump | Bundle related fields into a value object |
| Inappropriate Intimacy | Reduce public API surface, hide details |
| Message Chains | Provide a method that encapsulates the chain |

## Tell, Don't Ask Principle

Instead of asking an object for data and acting on it, tell the object what to do.

| Ask (bad) | Tell (good) |
|-----------|-------------|
| `if (user.getAge() >= 18) grant(user)` | `user.grantIfEligible()` |
| `order.getItems().add(item)` | `order.addItem(item)` |
| `if (light.isOn()) light.setOn(false)` | `light.toggle()` |

## When Getters Are Acceptable

- Value objects that are inherently data-centric (Point, Money, Color)
- DTOs at system boundaries (serialization, API responses)
- Read-only views of state for display purposes
- Framework requirements (ORM, serialization libraries)

Even in these cases, prefer named query methods over generic getters: `account.availableBalance()` over `account.getBalance()`.
