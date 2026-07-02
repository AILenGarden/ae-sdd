# Red-Green-Refactor Cycle Steps

## Phase 1: Red — Write a Failing Test

### Decision Criteria
- What is the simplest next behavior the system should exhibit?
- Can this behavior be expressed in a single assertion?
- Does this test add new information that no existing test covers?

### Checklist
1. Name the test after the behavior, not the method: `should_calculate_total_with_discount` not `test_calculate`.
2. Write the assertion first, then work backward to the setup.
3. Run the test. It must fail.
4. Read the failure message. It should indicate exactly what is missing (a class, a method, a return value).
5. If the test passes immediately, it is redundant — delete it and pick a different behavior.

### Common Red Phase Mistakes
- Writing multiple assertions in one test (split them).
- Describing implementation instead of behavior.
- Skipping the run step (you must see it fail).

---

## Phase 2: Green — Make It Pass

### Decision Criteria
- What is the absolute minimum code to make this test green?
- Am I writing code that is not demanded by a test?

### Checklist
1. Write only enough code to make the failing test pass.
2. It is acceptable to hard-code, use constants, or duplicate code.
3. Run the full test suite. All tests must be green.
4. If a previously passing test breaks, fix it before proceeding.

### Common Green Phase Mistakes
- Implementing the "real" algorithm too early.
- Adding error handling before a test demands it.
- Refactoring during the green phase (wait).

---

## Phase 3: Refactor — Improve the Design

### Decision Criteria
- Is there duplication I can remove?
- Are names clear and intention-revealing?
- Are there long methods that should be extracted?
- Can I simplify any conditional logic?

### Checklist
1. All tests are green before starting.
2. Make one structural change at a time.
3. Run tests after each change.
4. Stop when the code clearly expresses its intent.
5. Do not add new behavior during refactoring.

### Refactoring Moves
- **Remove duplication**: extract shared logic into a method or variable.
- **Rename**: make names match the domain language.
- **Extract method**: break long methods into focused, named steps.
- **Inline**: remove unnecessary indirection.
- **Simplify conditionals**: replace nested ifs with guard clauses or polymorphism.

### Common Refactor Phase Mistakes
- Adding new features disguised as refactoring.
- Refactoring without running tests between changes.
- Over-engineering (extracting abstractions too early).
