---
name: oop-composition-over-inheritance
description: This skill should be used when the user asks about "composition over inheritance", mentions "favoring composition", discusses deep inheritance hierarchies, fragile base class problems, or wants to combine behaviors flexibly.
version: 1.0.0
---

# OOP: Composition Over Inheritance

Favor object composition over class inheritance to achieve flexible, loosely coupled designs that combine behaviors without creating fragile hierarchies.

## When This Skill Applies
- An inheritance hierarchy is deeper than 2-3 levels
- Subclasses override many parent methods or use only a fraction of inherited behavior
- A class needs to combine behaviors from multiple sources (multiple inheritance pressure)
- The user mentions composition over inheritance or the fragile base class problem
- Changing a base class breaks unexpected subclasses

## Core Principle
Inheritance creates tight coupling between parent and child classes. Changes to the base class ripple through the entire hierarchy, often in unexpected ways (fragile base class problem). Composition assembles behavior from independent, interchangeable parts. Prefer "has-a" relationships over "is-a" relationships unless the is-a relationship is genuinely stable and permanent.

## Workflow

### Step 1: Identify the Hierarchy Problem
Check if the inheritance tree is deep (more than 2-3 levels), wide (many subclasses overriding different behaviors), or rigid (need to combine behaviors in ways the hierarchy does not support).

### Step 2: Identify the Behaviors
List the distinct behaviors that vary across the hierarchy. Each behavior is a candidate for a separate component or strategy.

### Step 3: Extract Behaviors into Components
Create a class or interface for each behavior. Move the behavior logic from the base class or subclass into the component.

### Step 4: Compose at Runtime
The original class holds references to behavior components. Inject them via the constructor. Different combinations of components produce different behaviors without new subclasses.

### Step 5: Verify
Ensure the composed behaviors work together. Run the test suite. Confirm that new behavior combinations do not require new classes.

## Detection / Indicators
- Inheritance depth greater than 3 levels
- Subclasses that override most parent methods
- "Diamond problem" or desire for multiple inheritance
- Subclasses that use only a fraction of the parent's API
- Combinatorial explosion of subclasses: `FastRedCar`, `SlowRedCar`, `FastBlueCar`...
- Base class changes unexpectedly break distant subclasses

## Transformation Pattern

**Before (inheritance explosion):**
```
class Animal { move() }
class FlyingAnimal extends Animal { move() { fly } }
class SwimmingAnimal extends Animal { move() { swim } }
// What about a duck that flies AND swims? No good solution with single inheritance.
```

**After (composition):**
```
interface MovementStrategy { move(): void }
class Flying implements MovementStrategy { move() { /* fly */ } }
class Swimming implements MovementStrategy { move() { /* swim */ } }

class Animal {
  constructor(private movements: MovementStrategy[]) {}
  move() { this.movements.forEach(m => m.move()) }
}

const duck = new Animal([new Flying(), new Swimming()])
```

## Common Pitfalls
- Dogmatically avoiding all inheritance (simple, shallow hierarchies are fine)
- Creating too many small components that are hard to assemble
- Losing the readability of "is-a" relationships when they are genuine
- Not using interfaces to define component contracts
- Composition without dependency injection (hard-coding components)

## Additional Resources
### Reference Files
- **`examples/composition-patterns.md`** — Composition patterns with real-world examples
