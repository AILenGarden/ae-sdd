---
name: fp-pure-functions
description: This skill should be used when the user asks about "pure functions", "side effects", mentions "immutability", "referential transparency", discusses hidden state changes, or wants to make functions more predictable and testable.
version: 1.0.0
---

# FP: Pure Functions and Immutability

Write functions that depend only on their inputs and produce no side effects, combined with immutable data structures, to create predictable, testable, and composable code.

## When This Skill Applies
- Functions modify external state (global variables, databases, files)
- Functions produce different results for the same inputs
- The user mentions pure functions, immutability, or side effects
- Testing requires complex setup due to hidden dependencies
- Debugging is hard because state changes are scattered

## Core Principle
A pure function has two properties: (1) given the same inputs, it always returns the same output, and (2) it produces no side effects. Combined with immutable data (data that cannot be changed after creation), pure functions make programs easier to reason about, test, and parallelize. Push side effects to the edges of the system and keep the core logic pure.

## Workflow

### Step 1: Identify Impurities
Find functions that read or modify external state: global variables, instance fields, database, filesystem, network, current time, random numbers. These are side effects.

### Step 2: Separate Pure Logic from Side Effects
Extract the pure computation into its own function. The impure function becomes a thin shell that gathers inputs, calls the pure function, and applies the outputs.

### Step 3: Make Data Immutable
Replace mutation (`obj.field = value`) with creation of new objects (`{ ...obj, field: value }`). Use `const` instead of `let`. Return new collections instead of modifying existing ones.

### Step 4: Pass Dependencies as Arguments
Replace hidden dependencies (global config, singletons, current time) with explicit function parameters. The caller provides the values.

### Step 5: Verify Purity
A function is pure if you can replace the call with its return value (referential transparency). Run the test suite — pure functions need only input/output assertions, no mocks.

## Detection / Indicators
- Function reads from or writes to global variables
- Function calls `Date.now()`, `Math.random()`, or reads environment variables
- Function modifies its input parameters
- Function result changes depending on when or how often it is called
- Testing requires mocking side-effecting dependencies
- Variables declared with `let` and mutated throughout a function

## Transformation Pattern

**Before (impure):**
```
let total = 0
function addToTotal(amount) {
  total += amount        // mutates external state
  log(`Total: ${total}`) // side effect
  return total
}
```

**After (pure):**
```
function addAmount(currentTotal, amount) {
  return currentTotal + amount  // new value, no mutation
}

// Side effects at the boundary
let total = addAmount(total, amount)
log(`Total: ${total}`)
```

## Common Pitfalls
- Treating "no side effects" as "no I/O anywhere" — side effects are pushed to boundaries, not eliminated
- Cloning deeply nested objects unnecessarily (use structural sharing or immutable libraries)
- Making everything immutable in performance-critical hot paths without measuring
- Forgetting that array methods like `sort()` and `splice()` mutate in place
- Confusing `const` (binding immutability) with deep immutability (object frozen)

## Additional Resources
### Reference Files
- **`references/purity-rules.md`** — Rules for identifying and maintaining function purity
