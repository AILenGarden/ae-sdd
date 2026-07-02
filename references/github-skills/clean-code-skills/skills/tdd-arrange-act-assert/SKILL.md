---
name: tdd-arrange-act-assert
description: This skill should be used when the user asks to "structure tests", "organize test code", mentions "arrange act assert", "AAA pattern", "given when then", discusses test readability, or wants to improve test structure.
version: 1.0.0
---

# TDD: Arrange-Act-Assert Pattern

Structure every test into three distinct phases — setup, execution, and verification — to make tests readable, consistent, and maintainable.

## When This Skill Applies
- Writing new tests for any behavior
- Reviewing or refactoring existing tests
- The user mentions AAA, Arrange-Act-Assert, or Given-When-Then
- Tests are hard to read or have unclear intent
- Tests mix setup, execution, and assertions

## Core Principle
Every test tells a story in three acts: Arrange the context, Act on the system under test, Assert the expected outcome. Separating these phases makes tests self-documenting. A reader should understand the scenario, the trigger, and the expected result at a glance.

## Workflow

### Step 1: Arrange — Set Up the Context
Create the objects, data, and dependencies needed for the test. Use descriptive variable names that communicate intent. Keep the setup minimal — only include what is relevant to this specific behavior.

### Step 2: Act — Execute the Behavior
Call the method or trigger the action under test. This should be a single logical operation. If you need multiple calls, consider whether you are testing multiple behaviors.

### Step 3: Assert — Verify the Outcome
Check that the result matches the expected behavior. Prefer one logical assertion per test. If you need multiple assertions, ensure they all verify the same behavior from different angles.

## Detection / Indicators
- Tests with no clear separation between setup, action, and verification
- Assertions scattered throughout the test body
- Setup code duplicated across many tests (extract to helper or beforeEach)
- Tests that perform multiple actions before asserting
- Test names that do not describe the scenario

## Transformation Pattern

**Before (unstructured test):**
```
test('user') {
  db.insert({name: 'Alice', role: 'admin'})
  const u = service.getUser('Alice')
  expect(u).toBeTruthy()
  expect(u.role).toBe('admin')
  service.promote(u)
  expect(u.role).toBe('superadmin')
}
```

**After (AAA-structured):**
```
test('getUser returns user with assigned role') {
  // Arrange
  db.insert({name: 'Alice', role: 'admin'})

  // Act
  const user = service.getUser('Alice')

  // Assert
  expect(user.role).toBe('admin')
}

test('promote elevates user to superadmin') {
  // Arrange
  const user = createUser({role: 'admin'})

  // Act
  service.promote(user)

  // Assert
  expect(user.role).toBe('superadmin')
}
```

## Common Pitfalls
- Combining multiple actions in one test (split into separate tests)
- Arranging more context than necessary (minimizes confusion about what matters)
- Asserting implementation details instead of outcomes
- Omitting the "Arrange" section when the test relies on implicit global state
- Using vague test names that do not describe the scenario

## Additional Resources
### Reference Files
- **`examples/aaa-patterns.md`** — Examples of the AAA pattern across different testing scenarios
