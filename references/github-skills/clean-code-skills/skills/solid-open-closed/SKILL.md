---
name: solid-open-closed
description: This skill should be used when the user asks about "open-closed principle", "OCP", mentions "extending behavior without modifying code", discusses adding new features to existing systems, or wants to design extensible modules.
version: 1.0.0
---

# SOLID: Open-Closed Principle

Design modules that are open for extension but closed for modification — new behavior is added by writing new code, not by changing existing code.

## When This Skill Applies
- Adding a new feature requires modifying existing, tested code
- Switch statements or if-else chains grow with each new requirement
- The user wants to add behavior without risking regression
- A system needs plugin-like extensibility
- The user mentions OCP or open-closed principle

## Core Principle
Software entities should be open for extension but closed for modification. When requirements change, you should be able to add new behavior by adding new code (new classes, new modules) rather than changing existing code. This protects working code from regression and enables parallel development.

## Workflow

### Step 1: Identify the Variation Point
Find where the system changes when new requirements arrive. If adding a new payment type means editing `processPayment()`, that function is the variation point.

### Step 2: Define an Abstraction
Create an interface or abstract class that captures the behavior that varies. The existing code depends on this abstraction instead of concrete implementations.

### Step 3: Implement Variants
Create a new class for each variant that implements the abstraction. Existing variants remain untouched.

### Step 4: Use a Factory or Registry
Provide a mechanism to select the right implementation at runtime — a factory method, a registry, dependency injection, or configuration.

### Step 5: Verify Extensibility
Add a new variant by creating only a new class and registering it. No existing code should change. Run all tests.

## Detection / Indicators
- Adding a new type requires editing a switch or if-else chain
- Multiple places in the codebase change for the same new feature
- Existing tests break when adding unrelated functionality
- Methods have boolean parameters that select between behaviors
- The same conditional pattern repeats across multiple methods

## Transformation Pattern

**Before (closed for extension):**
```
function calculateArea(shape) {
  if (shape.type === 'circle') return Math.PI * shape.radius ** 2
  if (shape.type === 'rectangle') return shape.width * shape.height
  // Must edit this function for every new shape
}
```

**After (open for extension):**
```
interface Shape { area(): number }
class Circle implements Shape { area() { return Math.PI * this.radius ** 2 } }
class Rectangle implements Shape { area() { return this.width * this.height } }
// Adding Triangle: create a new class, no existing code changes
```

## Common Pitfalls
- Applying OCP prematurely before the variation is clear (wait for the second variant)
- Over-abstracting: not every if-else needs a strategy pattern
- Forgetting the factory/registry — code still needs to select implementations
- Creating abstractions that leak implementation details
- Designing for hypothetical future extensions that never materialize

## Additional Resources
### Reference Files
- **`references/ocp-strategies.md`** — Common strategies for achieving open-closed design
