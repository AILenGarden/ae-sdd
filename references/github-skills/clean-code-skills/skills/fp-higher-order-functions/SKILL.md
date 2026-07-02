---
name: fp-higher-order-functions
description: This skill should be used when the user asks about "higher-order functions", "HOFs", mentions "map filter reduce", "callbacks", discusses reducing loop duplication, or wants to abstract common patterns with function parameters.
version: 1.0.0
---

# FP: Higher-Order Functions

Use functions that accept or return other functions to abstract common patterns, reduce duplication, and build composable operations.

## When This Skill Applies
- Repetitive loop patterns differ only in the operation applied
- The user mentions map, filter, reduce, higher-order functions, or callbacks
- Functions share structure but vary in one specific behavior
- The user wants to reduce boilerplate in collection processing
- Custom event handlers, middleware, or plugin systems are being designed

## Core Principle
A higher-order function (HOF) either takes a function as an argument or returns a function as its result. HOFs capture common patterns (iteration, transformation, filtering, composition) and parameterize the varying part. This eliminates structural duplication and creates reusable, composable building blocks.

## Workflow

### Step 1: Identify the Pattern
Find two or more code blocks that share the same structure but differ in one operation. The shared structure is the HOF; the varying operation becomes the function argument.

### Step 2: Extract the Higher-Order Function
Create a function that takes the varying behavior as a parameter. Replace the duplicated code blocks with calls to the HOF, passing the specific behavior.

### Step 3: Use Built-in HOFs Where Possible
Most languages provide `map`, `filter`, `reduce`, `forEach`, `flatMap`, `find`, `some`, `every`. Use these instead of writing manual loops.

### Step 4: Compose HOFs
Chain HOFs to build complex operations from simple ones. Each step in the chain does one thing. The pipeline reads as a description of the transformation.

### Step 5: Verify Readability
If the HOF chain is hard to read, extract intermediate steps into named variables or named functions. The goal is clarity, not cleverness.

## Detection / Indicators
- For-loops that transform, filter, or accumulate — replace with `map`, `filter`, `reduce`
- Multiple functions with identical structure except for one operation
- Callback patterns that could be generalized
- Manual iteration where a declarative pipeline would be clearer
- Code that creates closures over configuration

## Transformation Pattern

**Before (manual loops):**
```
// Get active user emails
const emails = []
for (const user of users) {
  if (user.active) {
    emails.push(user.email)
  }
}
```

**After (HOF pipeline):**
```
const emails = users
  .filter(user => user.active)
  .map(user => user.email)
```

## Common Pitfalls
- Over-chaining to the point of unreadability (break into named steps)
- Using `reduce` for everything when `map` or `filter` is clearer
- Nesting HOFs deeply instead of composing them in a pipeline
- Forgetting that HOFs may create new arrays (performance in hot paths)
- Writing custom HOFs when a built-in one exists

## Additional Resources
### Reference Files
- **`examples/hof-patterns.md`** — Common HOF patterns with practical examples
