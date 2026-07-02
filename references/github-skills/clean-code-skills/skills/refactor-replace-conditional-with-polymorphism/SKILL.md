---
name: refactor-replace-conditional-with-polymorphism
description: This skill should be used when the user asks to "replace conditionals", "refactor switch statements", mentions "polymorphism", discusses complex branching logic, type-based conditionals, or wants to eliminate long if-else or switch-case chains.
version: 1.0.0
---

# Refactoring: Replace Conditional with Polymorphism

Replace complex conditional logic that branches on type or category with polymorphic dispatch, distributing each branch into its own class with a shared interface.

## When This Skill Applies
- A switch or if-else chain branches on an object's type, status, or category
- The same conditional structure is duplicated across multiple methods
- Adding a new type requires modifying existing conditionals
- The user wants to apply the Open-Closed Principle to branching logic
- Business rules vary by type and are tangled together

## Core Principle
When a conditional selects behavior based on type, replace the conditional with polymorphism. Each type gets its own class that implements a shared interface. The conditional disappears — the correct behavior is selected by the object's type at runtime. This makes the system open for extension (add a new type) and closed for modification (existing types are untouched).

## Workflow

### Step 1: Identify the Conditional
Find switch statements or if-else chains that branch on type, status, category, or kind. Look for the same branching pattern repeated in multiple methods — this is a strong signal.

### Step 2: Define the Interface
Extract the common method signature from the conditional branches. This becomes the shared interface or abstract method that all variants will implement.

### Step 3: Create Subclasses or Implementations
For each branch, create a class that implements the interface. Move the branch's logic into the corresponding class. Each class encapsulates the behavior for one variant.

### Step 4: Replace the Conditional with Dispatch
Replace the original conditional with a call to the interface method on the appropriate object. Use a factory or registry to select the correct implementation at construction time.

### Step 5: Verify
Run the test suite. Add a test for each variant to confirm the polymorphic behavior matches the original conditional logic.

## Detection / Indicators
- Switch statements with more than 3 cases
- If-else chains with `instanceof`, type checks, or string comparisons
- The same switch structure appears in multiple methods
- Adding a new type requires editing multiple files
- Comments like "if it's a Premium user, do X; if Basic, do Y"

## Transformation Pattern

**Before:**
```
function calculateShipping(order) {
  switch (order.type) {
    case 'standard':
      return order.weight * 5.0
    case 'express':
      return order.weight * 10.0 + 15.0
    case 'overnight':
      return order.weight * 20.0 + 30.0
  }
}
```

**After:**
```
interface ShippingStrategy {
  calculate(order): number
}

class StandardShipping implements ShippingStrategy {
  calculate(order) { return order.weight * 5.0 }
}

class ExpressShipping implements ShippingStrategy {
  calculate(order) { return order.weight * 10.0 + 15.0 }
}

class OvernightShipping implements ShippingStrategy {
  calculate(order) { return order.weight * 20.0 + 30.0 }
}

// Usage: order.shippingStrategy.calculate(order)
```

## Common Pitfalls
- Replacing trivial conditionals (2 simple branches are fine as-is)
- Creating a class hierarchy for a conditional that will never grow
- Forgetting to handle the factory/registry that selects the right implementation
- Introducing polymorphism when a simple lookup table or map would suffice
- Over-engineering: not every if-else needs polymorphism

## Additional Resources
### Reference Files
- **`examples/polymorphism-before-after.md`** — Extended before/after examples with factory patterns
