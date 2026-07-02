# Iteration Loop Strategy

How to use coverage feedback to drive successive UT improvements without
modifying business code.

## Loop budget

Default: **max 6 iterations** for a single class. Each iteration costs one
`mvn test` invocation; six is enough for genuinely well-structured code, and
sufficient signal that something else is wrong if it's not converging.

Hard stop conditions (exit the loop and report):
1. Both thresholds met **and** quality checker reports zero errors
2. Iteration budget exhausted
3. Two consecutive iterations made the *same* uncovered line set unchanged
   (the loop is stuck — see "Stuck loops" below)
4. Tests fail and the failure is on production code (not test code)
5. The user interrupts

## Each iteration

1. Run `scripts/run_coverage.sh <root> <TestFQN>` to execute tests and refresh
   the JaCoCo report.
2. Run `scripts/parse_coverage.py <jacoco.xml> --line 80 --branch 70 --class <SUT>` to
   get the structured uncovered-line list.
3. If the previous step reports `summary.all_pass: true`, run
   `scripts/check_test_quality.py <TestFile.java> --sut <SUT>` and exit only
   when there are zero errors (warnings are acceptable).
4. Otherwise, for each uncovered line in `uncovered_lines` and each entry in
   `partial_branches`:
   - **Read the source line in context** (3 lines before, 3 after) to
     understand the predicate
   - Determine what **input state + collaborator behavior** would route execution
     through that line
   - Add **one new test method per logical branch**, not per uncovered line
     (a single new test can cover several adjacent lines that all belong to
     the same path)
5. Re-run quality checker on the new test file. Fix any errors before
   re-running coverage.
6. Repeat from step 1.

## Reading uncovered_lines into test ideas

The parser gives you line numbers; you must read the source to interpret them.
Common shapes:

| What you see in `uncovered_lines` | What to add |
|---|---|
| Single line inside an `if (x)` body | a test where `x` is true with the assertions on what that branch does |
| Multiple consecutive lines after `else` | a test triggering the else branch |
| The signature line of a private/static helper | the public method that calls it isn't being exercised — add a test for that public method |
| Lines inside a `catch (FooException)` | a test that arranges a collaborator to throw FooException |
| The single line `throw new IllegalArgumentException(...)` | a test that asserts the exception is thrown for the matching guard |

For `partial_branches` (a line where some branches are covered, others not),
you need a test that picks the *opposite* path of whatever the existing tests
already cover. Read existing tests first to see what they assert, then design
the missing case.

## Stuck loops — what they mean

If two iterations leave `uncovered_lines` identical, the loop is not making
progress. Inspect the unreachable lines and classify:

| Cause | Action |
|---|---|
| Lines are genuinely unreachable (dead code) | Stop. Report the dead lines to the user; do not write tests for them |
| Lines require changing static state (System.exit, System.getenv, file I/O) | Stop. Suggest the user refactor the SUT to inject the dependency, or annotate the lines as `<excludes>` in JaCoCo config |
| Lines are inside a default constructor of a utility class | Often acceptable to exclude; ask user |
| Lines require a specific timing window | Stop. Recommend Awaitility or a Clock injection — but those are SUT changes, so ask the user |

Do NOT try to bypass with reflection-based hacks (rule R5).

## When tests fail (not coverage failure — actual test failure)

`run_coverage.sh` returns non-zero on failed tests but still writes
`jacoco.xml`. Triage:

1. Read the surefire output (it's in stderr from the runner)
2. If failure is in **a test you just added** — fix the test (wrong
   stubbing, wrong expected value, missing import)
3. If failure is in **an existing test** that previously passed — your new
   test changed shared state (mutable static, mock leakage). Make tests
   independent: avoid `@MockitoSettings(strictness=LENIENT)` global state,
   reset shared mocks in `@BeforeEach`
4. If failure looks like it's in the **production code** — STOP. Report to the
   user. Do not modify production code. The user might:
   - Confirm a real bug they want to fix separately
   - Confirm the test is wrong and ask you to revise

## Measuring progress

After each iteration, print a one-line status to the user (no novel-length
narration):

```
iter 3/6 — line 78%→84% (target 80% ✓), branch 60%→68% (target 70% ✗), 2 quality errors → 0
```

Concrete deltas; nothing else.

## When to use parameterized tests

If a single condition has 4+ equivalence classes (e.g. four cases of a
status enum), use one `@ParameterizedTest` rather than four individual
`@Test` methods. This reduces noise without losing coverage signal.

## Final report shape

When the loop exits successfully:

- file paths of every test file you created or modified
- final coverage numbers (line, branch, before → after)
- iteration count used
- any quality warnings remaining (errors should be zero — that was the exit
  condition)
- any source lines that remained uncovered with reason

When the loop exits unsuccessfully:

- last coverage numbers reached
- specific blockers (failing tests, unreachable lines, plugin missing, etc.)
- recommended next step
