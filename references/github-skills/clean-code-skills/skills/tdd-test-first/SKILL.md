---
name: tdd-test-first
description: This skill should be used when the user asks to "write tests first", "start with a test", mentions "test-first development", discusses writing tests before implementation, or wants to ensure code is testable by design.
version: 1.0.0
---

# TDD: Test-First Approach

Enforce the discipline of writing tests before production code to ensure that every behavior is specified, verified, and designed for testability from the start.

## When This Skill Applies
- The user wants to add a new feature or fix a bug
- The user mentions writing tests first or test-first development
- Code is being written without corresponding tests
- The user wants to improve testability of existing code

## Core Principle
Write the test before the code it tests. A test-first approach forces you to think about the desired behavior and public API before committing to an implementation. Code that is hard to test is usually hard to use — test-first design produces better interfaces naturally.

## Workflow

### Step 1: Define the Desired Behavior
State in plain language what the code should do. Focus on observable outcomes, not implementation details. Ask: "What should the caller see happen?"

### Step 2: Write the Test
Translate the behavior into a test. Choose a descriptive name. Set up the minimum context (Arrange), invoke the behavior (Act), and verify the outcome (Assert). Do not write any production code yet.

### Step 3: Watch It Fail
Run the test and confirm it fails for the right reason — typically a missing class, method, or incorrect return value. If it fails for an unexpected reason (syntax error, wrong import), fix the test first.

### Step 4: Implement Just Enough
Write the simplest production code that makes the test pass. Do not anticipate future requirements.

### Step 5: Verify and Continue
Run the full suite. If green, consider the next behavior. If a test breaks, fix the issue before moving on.

## Detection / Indicators
- Production code exists without a corresponding test
- Tests were added after the code was "done"
- Tests mirror implementation structure instead of describing behavior
- The user writes a full class before any test
- Mocking is excessive because the code was not designed for testability

## Transformation Pattern

**Before (test-after):**
1. Write `UserService.register(email, password)` with validation, hashing, database save.
2. Realize it is hard to test because of database dependency.
3. Retrofit mocks. Tests become fragile.

**After (test-first):**
1. Test: "registering a user with valid input returns a success result."
2. This forces you to define what "valid input" and "success result" mean.
3. The `UserService` naturally accepts a `UserRepository` interface (for testability).
4. Each behavior (validation, hashing, persistence) gets its own test and emerges incrementally.

## Common Pitfalls
- Writing the test and the implementation in the same mental step (slow down)
- Testing private methods directly (test through the public API)
- Writing integration tests when unit tests suffice for the current behavior
- Designing the full architecture before writing the first test
- Skipping the failure step (you must see the test fail)

## Additional Resources
### Reference Files
- **`references/test-first-checklist.md`** — Step-by-step checklist for adopting a test-first workflow
