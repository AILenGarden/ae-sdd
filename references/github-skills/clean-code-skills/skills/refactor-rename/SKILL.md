---
name: refactor-rename
description: This skill should be used when the user asks to "rename a variable", "improve naming", mentions "meaningful names", "naming conventions", discusses unclear or misleading identifiers, or wants to make code more readable through better names.
version: 1.0.0
---

# Refactoring: Meaningful Rename

Apply clear, intention-revealing names to variables, methods, classes, and modules so that code communicates its purpose without comments.

## When This Skill Applies
- Variable, method, or class names are abbreviations or single characters
- Names are misleading or do not match current behavior
- A reader needs to look at the implementation to understand a name
- The user asks to improve readability or naming
- During code review when names cause confusion

## Core Principle
A name should tell you why something exists, what it does, and how it is used. If a name requires a comment to explain it, the name is wrong. Good names reduce the need for documentation and make code changes safer because intent is explicit.

## Workflow

### Step 1: Identify Naming Violations
Scan for single-character variables (outside trivial loops), abbreviations, generic names (`data`, `info`, `temp`, `result`, `manager`, `handler`), misleading names, or names that do not match current behavior.

### Step 2: Understand the Purpose
Read how the identifier is used. What does it represent in the domain? What role does it play? The name should match the domain language, not the implementation mechanism.

### Step 3: Choose a Better Name
Apply the naming rules: use nouns for variables and classes, verbs for methods, adjective phrases for booleans. Be specific. Prefer `customerEmailAddress` over `email`, `isExpired` over `flag`.

### Step 4: Rename Across the Codebase
Use IDE rename refactoring or search-and-replace to update all references. Check test files, configuration, documentation, and API boundaries.

### Step 5: Verify
Run the test suite. Check that no references were missed. Confirm the new name reads naturally in context.

## Detection / Indicators
- Single-letter variables outside loop counters: `x`, `d`, `t`
- Generic names: `data`, `info`, `result`, `temp`, `obj`, `val`
- Misleading names: `isEnabled` when it means `isVisible`
- Abbreviated names: `usr`, `mgr`, `btn`, `cfg`
- Names that include type: `userList`, `nameString` (the type system handles this)
- Encoded names: `m_name`, `str_value`, `i_count` (Hungarian notation)

## Transformation Pattern

| Before | After | Reason |
|--------|-------|--------|
| `d` | `elapsedDays` | Reveals meaning |
| `list` | `activeUsers` | Specifies the content |
| `flag` | `isEligibleForDiscount` | Describes the condition |
| `doIt()` | `sendWelcomeEmail()` | Names the action |
| `DataManager` | `OrderRepository` | Names the domain role |
| `process()` | `calculateShippingCost()` | Names the computation |

## Common Pitfalls
- Making names too long: `theListOfAllActiveUserEmailAddresses` (find the right length)
- Renaming without updating all references (use automated tools)
- Using synonyms inconsistently: `fetch` / `get` / `retrieve` for the same operation
- Encoding type information in names: `userArray`, `nameStr`
- Not renaming test code to match (tests become confusing)

## Additional Resources
### Reference Files
- **`references/naming-conventions.md`** — Naming rules by identifier type with examples
