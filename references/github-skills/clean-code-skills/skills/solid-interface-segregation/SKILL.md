---
name: solid-interface-segregation
description: This skill should be used when the user asks about "interface segregation", "ISP", mentions "fat interfaces", discusses clients forced to depend on methods they don't use, or wants to design smaller, focused interfaces.
version: 1.0.0
---

# SOLID: Interface Segregation Principle

Design small, focused interfaces so that clients depend only on the methods they actually use, avoiding forced dependencies on irrelevant functionality.

## When This Skill Applies
- An interface has methods that some implementors leave empty or throw NotImplementedException
- Clients import an interface but only call a subset of its methods
- The user mentions ISP, interface segregation, or fat interfaces
- Changing one interface method forces recompilation or retesting of unrelated clients
- Mock objects in tests implement many methods just to satisfy the interface

## Core Principle
No client should be forced to depend on methods it does not use. Large, general-purpose interfaces create coupling between unrelated clients. When one client needs a change to the interface, other clients are affected even though they do not use the changed method. Split fat interfaces into smaller, role-specific ones.

## Workflow

### Step 1: Identify the Fat Interface
Find interfaces with many methods where different clients use different subsets. Look for implementors that leave methods empty or throw exceptions.

### Step 2: Group Methods by Client
Identify which clients call which methods. Group methods that are always used together. Each group becomes a candidate for a separate interface.

### Step 3: Define Role Interfaces
Create a focused interface for each group. Name each interface after the role it represents: `Readable`, `Writable`, `Closeable` instead of one `FileOperations`.

### Step 4: Update Implementors
Classes that previously implemented the fat interface now implement the specific role interfaces they support. A class can implement multiple interfaces.

### Step 5: Update Clients
Each client depends on the smallest interface that covers its needs. Update parameter types and dependency declarations.

## Detection / Indicators
- Interface with more than 5-7 methods
- Implementors with empty method bodies or `throw new NotImplementedException()`
- Tests that mock many methods but only exercise one
- Clients that import an interface but call only 1-2 methods
- Interface changes that cause unrelated code to recompile

## Transformation Pattern

**Before (fat interface):**
```
interface Worker {
  work(): void
  eat(): void
  sleep(): void
}
// Robot implements Worker but cannot eat() or sleep()
```

**After (segregated interfaces):**
```
interface Workable { work(): void }
interface Feedable { eat(): void }
interface Restable { sleep(): void }

class Human implements Workable, Feedable, Restable { ... }
class Robot implements Workable { ... }
```

## Common Pitfalls
- Creating one interface per method (too granular)
- Splitting interfaces that are always used together (keep them unified)
- Ignoring ISP for internal code that only has one client (YAGNI)
- Not updating client types after splitting (clients still depend on the fat interface)

## Additional Resources
### Reference Files
- **`references/isp-patterns.md`** — Patterns for interface segregation with real-world examples
