---
name: tdd-red-green-refactor
description: This skill should be used when the user asks to "do TDD", "write tests first", mentions "red-green-refactor", discusses test-driven development cycles, or wants to build features incrementally with tests.
version: 1.0.0
---

# TDD: Red-Green-Refactor Cycle

Guide the development process through the disciplined red-green-refactor cycle to produce well-tested, well-designed code in small, confident increments.

## When This Skill Applies
- Starting a new feature or behavior from scratch
- The user mentions TDD, red-green-refactor, or test-driven development
- Building production code incrementally with automated tests
- The user wants to ensure every line of code is justified by a failing test

## Core Principle
Write a failing test first (Red), make it pass with the simplest possible implementation (Green), then improve the design without changing behavior (Refactor). Each cycle should take minutes, not hours. Never skip a phase — the discipline of the cycle is what produces both correctness and clean design.

## Workflow

### Step 1: Red — Write a Failing Test
Write a single, focused test that describes the next small increment of desired behavior. Run the test suite and confirm it fails for the expected reason. The failure message should clearly indicate what is missing.

### Step 2: Green — Make It Pass
Write the minimum amount of production code to make the failing test pass. Resist the urge to write more than necessary. Hard-code values if that is the simplest path. The goal is a green test suite, not elegant code.

### Step 3: Refactor — Improve the Design
With all tests green, improve the code's structure. Remove duplication, extract methods, rename for clarity, simplify conditionals. Run tests after every change to confirm behavior is preserved. Stop when the code clearly expresses its intent.

### Step 4: Repeat
Pick the next small behavior increment and return to Step 1. Let the tests drive the design forward one assertion at a time.

## Detection / Indicators
- Production code written before any test exists
- Large batches of code written between test runs
- Tests that pass on the first run (they were not written first)
- Refactoring skipped because "it works"
- Tests that test implementation details instead of behavior

## Transformation Pattern

**Before (no TDD):**
Write the entire function, then add tests after the fact to verify it. Tests become coupled to implementation details.

**After (red-green-refactor):**
1. Test: `expect(calculator.add(2, 3)).toBe(5)` — Red
2. Code: `add(a, b) { return 5; }` — Green (simplest)
3. Test: `expect(calculator.add(1, 1)).toBe(2)` — Red (forces generalization)
4. Code: `add(a, b) { return a + b; }` — Green
5. Refactor: rename if needed, extract if growing — Refactor

## Common Pitfalls
- Writing too much production code in the Green phase
- Skipping the Refactor phase under time pressure
- Writing tests that are too large (testing multiple behaviors at once)
- Refactoring while tests are red
- Getting stuck on what test to write next (pick the simplest next behavior)

## Additional Resources
### Reference Files
- **`references/cycle-steps.md`** — Detailed breakdown of each phase with decision criteria and examples
