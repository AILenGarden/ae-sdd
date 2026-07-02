# Test-First Checklist

Use this checklist before writing any production code.

## Before You Start
- [ ] Can you describe the desired behavior in one sentence?
- [ ] Is this the smallest increment you can test?
- [ ] Do you know what the expected output or side effect is?

## Writing the Test
- [ ] The test name describes a behavior, not a method name.
- [ ] The test has a single reason to fail.
- [ ] The test uses the public API (not internal methods).
- [ ] The test does not depend on execution order.
- [ ] The test does not require complex setup (if it does, the design may need simplification).

## After the Test Fails
- [ ] The failure message clearly indicates what is missing.
- [ ] The failure is not due to a typo or import error.
- [ ] You understand exactly what production code is needed.

## After the Test Passes
- [ ] You wrote the minimum code to pass.
- [ ] All existing tests still pass.
- [ ] The code is ready for refactoring if needed.

## Behavioral Coverage Guide

When deciding what to test next, work through these categories:

1. **Happy path**: the most common successful scenario.
2. **Edge cases**: empty input, zero, null, single element, maximum values.
3. **Error cases**: invalid input, missing dependencies, timeouts.
4. **Boundary conditions**: off-by-one, limits, transitions between states.

Pick the next simplest behavior from this list. If you cannot decide, start with the happy path.

## Signals That You Are Doing It Right
- Each test adds one new assertion about behavior.
- Production code grows in small, confident steps.
- You rarely write code that is not demanded by a failing test.
- Refactoring feels safe because the test suite catches regressions.
- The public API emerges naturally from test requirements.
