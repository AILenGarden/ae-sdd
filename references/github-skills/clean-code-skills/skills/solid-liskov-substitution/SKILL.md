---
name: solid-liskov-substitution
description: This skill should be used when the user asks about "Liskov substitution", "LSP", mentions "subtype behavior", discusses inheritance hierarchies, or wants to verify that subtypes are safe replacements for their base types.
version: 1.0.0
---

# SOLID: Liskov Substitution Principle

Verify that subtypes can replace their base types without altering the correctness of the program — every subclass must honor the contract of its parent.

## When This Skill Applies
- Designing or reviewing inheritance hierarchies
- A subclass overrides a method in a way that changes expected behavior
- Runtime errors or bugs appear when substituting a derived type
- The user mentions LSP, Liskov substitution, or behavioral subtyping
- Code uses `instanceof` checks to handle different subtypes differently

## Core Principle
If S is a subtype of T, then objects of type T can be replaced with objects of type S without altering any of the desirable properties of the program. Subtypes must honor the base type's contract: same preconditions (or weaker), same postconditions (or stronger), and preserved invariants. Violations create fragile hierarchies where polymorphism becomes unreliable.

## Workflow

### Step 1: Define the Base Contract
Document the base type's contract: what preconditions callers must satisfy, what postconditions the method guarantees, and what invariants the class maintains.

### Step 2: Check Each Subtype
For every overriding method in each subtype, verify:
- Preconditions are not strengthened (subtype must accept everything the base accepts)
- Postconditions are not weakened (subtype must guarantee at least what the base guarantees)
- Invariants are preserved (subtype does not break class-level rules)

### Step 3: Test Substitutability
Write tests using the base type reference. Run them with every subtype. All must pass without modification.

### Step 4: Fix Violations
If a subtype cannot honor the base contract, it should not extend that base. Use composition instead, or redesign the hierarchy.

## Detection / Indicators
- A subclass throws an exception where the base class does not
- A subclass ignores or no-ops a base class method
- Code checks `instanceof` before calling a method
- A subclass has stricter input validation than its parent
- Overridden methods return different types or null where the parent returns a value
- The "is-a" relationship feels forced: "is a Square a Rectangle?"

## Transformation Pattern

**Before (LSP violation):**
```
class Bird { fly() { /* moves through air */ } }
class Penguin extends Bird { fly() { throw new Error("Can't fly") } }
// Code expecting Bird.fly() breaks with Penguin
```

**After (LSP compliant):**
```
class Bird { move() { /* base movement */ } }
class FlyingBird extends Bird { move() { /* fly */ } }
class Penguin extends Bird { move() { /* swim/walk */ } }
// All Birds can move(); specific movement varies
```

## Common Pitfalls
- Using inheritance for code reuse instead of behavioral subtyping
- The "Circle-Ellipse" or "Square-Rectangle" problem: geometric is-a is not behavioral is-a
- Throwing NotImplementedException in subclass methods
- Changing return types in ways that break callers
- Ignoring LSP in test mocks (mocks that do not honor the interface contract)

## Additional Resources
### Reference Files
- **`examples/lsp-violations.md`** — Common LSP violation patterns with corrections
