---
name: clean-code-detect-smells
description: This skill should be used when the user asks to "find code smells", "review code quality", mentions "code smells", "technical debt", discusses identifying problematic code patterns, or wants a systematic code quality assessment.
version: 1.0.0
---

# Clean Code: Detect Code Smells

Systematically identify code smells — surface indicators of deeper design problems — and prioritize them for refactoring.

## When This Skill Applies
- Reviewing code for quality issues
- The codebase feels "hard to work with" but the problems are not obvious
- The user mentions code smells, technical debt, or code quality
- Preparing a refactoring plan
- During code review to provide structured feedback

## Core Principle
A code smell is a surface indication that usually corresponds to a deeper problem in the system. Smells are not bugs — the code works. But they signal that the design is becoming harder to understand, modify, or extend. Detecting smells early prevents them from compounding into costly structural problems.

## Workflow

### Step 1: Scan for Bloater Smells
Look for entities that have grown too large: long methods, large classes, long parameter lists, data clumps, primitive obsession.

### Step 2: Scan for Coupler Smells
Look for excessive coupling: feature envy, inappropriate intimacy, message chains, middle man.

### Step 3: Scan for Change-Prevention Smells
Look for patterns that make changes risky: divergent change (one class changed for many reasons), shotgun surgery (one change touches many classes), parallel inheritance hierarchies.

### Step 4: Scan for Dispensable Smells
Look for things that should not exist: dead code, speculative generality, lazy class, duplicate code, unnecessary comments.

### Step 5: Prioritize by Impact
Rate each smell by: frequency (how often it occurs), impact (how much it slows development), and effort (how hard it is to fix). Fix high-frequency, high-impact, low-effort smells first.

## Detection / Indicators

### Bloaters
| Smell | Detection Rule |
|-------|---------------|
| Long Method | More than 15 lines of logic |
| Large Class | More than 200 lines or 7+ public methods |
| Long Parameter List | More than 3 parameters |
| Data Clumps | Same group of fields appears together in multiple places |
| Primitive Obsession | Using strings, numbers, or booleans instead of value objects |

### Couplers
| Smell | Detection Rule |
|-------|---------------|
| Feature Envy | Method uses more data from another class than its own |
| Inappropriate Intimacy | Two classes access each other's internals |
| Message Chains | `a.getB().getC().getD().doSomething()` |
| Middle Man | Class delegates all work to another class |

### Change Preventers
| Smell | Detection Rule |
|-------|---------------|
| Divergent Change | One class is modified for multiple unrelated features |
| Shotgun Surgery | One feature change requires editing many classes |
| Parallel Inheritance | Adding a subclass in one hierarchy requires adding one in another |

### Dispensables
| Smell | Detection Rule |
|-------|---------------|
| Dead Code | Unreachable code, unused variables, methods never called |
| Speculative Generality | Abstract classes, interfaces, or parameters used by only one type |
| Duplicate Code | Identical or near-identical blocks in multiple locations |

## Transformation Pattern

**Smell → Refactoring mapping:**

| Smell | Recommended Refactoring |
|-------|------------------------|
| Long Method | Extract Method |
| Large Class | Extract Class |
| Long Parameter List | Introduce Parameter Object |
| Data Clumps | Extract Class or Value Object |
| Primitive Obsession | Replace Primitive with Value Object |
| Feature Envy | Move Method |
| Message Chains | Hide Delegate |
| Duplicate Code | Extract Method, Pull Up Method |
| Dead Code | Delete it |
| Middle Man | Remove Middle Man or Inline Class |

## Common Pitfalls
- Treating every smell as urgent (prioritize by impact)
- Refactoring without tests (fix the safety net first)
- Chasing smells in code that is rarely modified (focus on active code)
- Confusing "smell" with "bug" — smells are design issues, not functional defects
- Applying refactoring mechanically without understanding the underlying design problem

## Additional Resources
### Reference Files
- **`references/smell-catalog.md`** — Complete catalog of code smells with examples and remedies
