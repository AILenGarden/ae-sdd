# java-ut-coverage-loop

A Claude Code skill that generates JUnit 5 + Mockito + AssertJ unit tests for a Java class, runs them via Maven + JaCoCo, and iteratively expands the test code from coverage feedback until thresholds are met — without ever modifying production code.

## What it does

Given a project root and a target Java class, the skill loops:

1. Generate / extend the test class under `src/test/java`
2. `mvn test -Dtest=<TargetTest> jacoco:report`
3. Parse `jacoco.xml`; identify uncovered lines and partial branches
4. Run static quality checks on the test file (no tautological asserts, no SUT mocking, no reflection on private members, etc.)
5. Add focused tests for the missing branches and repeat

Default thresholds: **line ≥ 80%, branch ≥ 70%**. Iteration budget: **6**. The loop stops with a clear report when thresholds + zero quality errors are reached, when budget is exhausted, when the same uncovered lines persist (stuck), or when production-code changes would be required.

## Hard constraints

- **Never modifies `src/main/`.** Only creates or edits `src/test/java/.../*Test.java`.
- **Never lowers thresholds** to make the loop "pass".
- **Asks before editing `pom.xml`** (e.g. to add the `jacoco-maven-plugin` if missing).
- **Stops at 6 iterations** rather than spinning indefinitely.

## Installation

Clone into your Claude Code skills directory:

```bash
git clone https://github.com/superheromeZzh/java-ut-coverage-loop.git \
  ~/.claude/skills/java-ut-coverage-loop
```

The skill becomes available immediately to Claude Code. Trigger it by asking things like:

- "Add unit tests for `com.example.OrderService` until coverage hits 80% line / 70% branch"
- "提升 com.example.OrderService 的单测覆盖率到 80%"
- "Generate tests for OrderService"

## Requirements

- Maven project (Gradle is not supported by this skill yet)
- `jacoco-maven-plugin` configured in the relevant module's `pom.xml` — if missing, the skill will offer the snippet from `assets/jacoco-pom-snippet.xml` and ask before editing your pom
- JDK 11+ (JaCoCo 0.8.12 supports up to Java 21)

## Layout

```
SKILL.md                          orchestrator — workflow, hard constraints, loop steps
scripts/
  detect_project.py               find Maven layout + jacoco status (JSON)
  run_coverage.sh                 mvn test + jacoco:report; print jacoco.xml path
  parse_coverage.py               parse jacoco.xml → JSON with uncovered lines + threshold check
  check_test_quality.py           static quality checks (R1–R9) on the test file
references/
  test_style.md                   JUnit5 / Mockito / AssertJ idioms, AAA, naming, assertion patterns
  quality_rails.md                R1–R9 hard rules + failure-mode catalog
  jacoco_setup.md                 when and how to add jacoco-maven-plugin (with consent)
  loop_strategy.md                iteration mechanics, stuck-loop handling, exit conditions
assets/
  jacoco-pom-snippet.xml          drop-in plugin block (JaCoCo 0.8.12)
  test_class_template.java        test class scaffold with placeholders
```

## Quality rails (what the checker enforces)

| Rule | Severity | Meaning |
|---|---|---|
| R1 no tautological asserts | error | `assertTrue(true)` is meaningless |
| R2 no null-only assertions | error | `assertNotNull(x)` as the only assertion isn't testing behavior |
| R3 every test has an assertion | error | A `@Test` body without an assertion proves nothing |
| R4 never mock the SUT | error | Mock collaborators, not the system under test |
| R5 no reflection on private members | warn | Drive private logic through the public API |
| R6 no flaky primitives | warn | `Thread.sleep`, unseeded `Random`, `LocalDateTime.now()` |
| R7 no catch-and-pass | warn | Don't silently swallow exceptions in tests |
| R8 naming pattern | warn | `should_xxx_when_yyy` / `testXxx` / `given_when_then` |
| R9 disabled tests don't count | warn | Fix or delete `@Disabled` |

## Status

v0.1.0 — initial release. Tested on synthetic Maven fixtures; not yet validated against a real production codebase. Feedback welcome via issues.

## License

MIT — see [LICENSE](LICENSE).
