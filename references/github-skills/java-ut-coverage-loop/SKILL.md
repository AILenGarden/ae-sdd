---
name: java-ut-coverage-loop
description: Generate JUnit 5 + Mockito + AssertJ unit tests for a Java class, run them via Maven + JaCoCo, and iteratively expand the test code based on coverage feedback until line ≥ 80% and branch ≥ 70% are met. The skill never modifies production code (only creates or edits files under src/test/java). Use when the user asks to "add unit tests for", "improve UT coverage of", "write tests until coverage hits X%", "generate tests for a class", "提升单测覆盖率", "给某个类补单测", or names a Java SUT and asks for tests with a coverage target. Scope is one production class at a time. Works on any Maven project: if the jacoco-maven-plugin is configured the skill uses it, otherwise it injects the JaCoCo agent at the command line without touching pom.xml.
---

# Java UT Coverage Loop

Generate and iteratively refine JUnit 5 / Mockito / AssertJ unit tests against a single Java class until the JaCoCo line and branch coverage thresholds are met, without changing production code.

## Hard constraints (do not violate)

1. **Never modify files under `src/main/`**. Only create/edit `src/test/java/...Test.java`. If the only path to higher coverage requires a production change, stop and report — do not edit `src/main/`.
2. **Never disable, exclude, or relax coverage thresholds** to make the loop "pass". Default thresholds: line ≥ 80%, branch ≥ 70%. The user may override at the start; you may not override mid-loop.
3. **Confirm with the user before editing `pom.xml`** (e.g. to add the JaCoCo plugin). pom.xml is not test code.
4. **Iteration budget: 6 by default.** If unsatisfied after 6 iterations, stop and report what's blocking; do not loop indefinitely.
5. **Quality rails are part of "done"**. Coverage thresholds met but quality checker reports `errors > 0` is NOT done.
6. **Don't work around corporate build configuration.** If preflight fails (settings.xml, internal repos, parent POM, JDK mismatch), surface the original Maven error verbatim and stop. Never delete `<mirrors>`, `<servers>`, or proxy settings; never disable SSL verification; never switch the project off its mvnw wrapper "to make it work".

## Inputs

The skill needs from the user:
- Project root (the directory containing the Maven pom)
- Target class FQCN (e.g. `com.example.OrderService`)
- Optional: line / branch coverage thresholds (default 80 / 70)

If any are missing, ask once, then proceed.

## Workflow

### Step 1 — Preflight the environment

Run `scripts/preflight.sh <project_root> [--module <module>]` first. This separates "environment broken" from "coverage too low" so the loop never burns iterations on a build that can't run.

The script returns JSON with `ok`, `first_failure`, and per-check `detail`. If `ok` is false:
- Show the user the `first_failure`, the original Maven error from `detail`, and the `hint`
- **Stop. Do not try to fix the environment by editing settings.xml, mirrors, proxy, or removing the mvnw wrapper.** Constraint 6 applies.
- Common failures and what they mean:

| `first_failure` | What's wrong | Who fixes it |
|---|---|---|
| `mvn_available` | No `mvn` or `./mvnw` reachable | User installs Maven or uses a project with mvnw |
| `effective_settings` | `~/.m2/settings.xml` malformed or missing required server creds | User / their build-infra team |
| `effective_pom` | Parent POM in private repo unreachable | User configures internal mirror; usually a credentials issue |
| `dependency_resolve` | Private artifacts not available | User / build-infra team |
| `test_compile` | Existing test sources don't compile | User fixes compile errors first |

### Step 2 — Detect the project

Run `scripts/detect_project.py <project_root> --class <FQCN>` and read the JSON.

Validate before continuing:
- `is_maven` must be true. If false, stop: this skill targets Maven projects.
- `target_class.main_file` must exist. If null, the FQCN is wrong; ask the user.
- Note the SUT module's `has_jacoco_plugin` flag. This selects the coverage path in Step 4 — **it does not gate execution**. Both paths are supported.

### Step 3 — Read the SUT and existing tests

Read:
- The SUT source file (`target_class.main_file`)
- Existing test file if it exists (`target_class.expected_test_file`)

Identify:
- Public methods (one branch family per method)
- Constructor-injected collaborators (these become `@Mock` fields)
- Side effects (writes to collaborators that will need `verify(...)`)
- Conditional branches: `if`, `switch`, ternary, try/catch, early returns, guard clauses

Refer to `references/test_style.md` for the canonical class skeleton, imports, and naming. Use `assets/test_class_template.java` as the starting scaffold if no test file exists.

### Step 4 — Initial test generation

Write the test file at `target_class.expected_test_file`. Aim for one happy-path test per public method plus an obvious failure path. Don't try to cover everything in iteration 0 — the loop will fill gaps.

Apply the style from `references/test_style.md`:
- `@ExtendWith(MockitoExtension.class)`, `@InjectMocks` SUT, `@Mock` collaborators
- AAA structure with `// given / // when / // then` comments and blank line separators
- AssertJ `assertThat(...)` for value assertions, `assertThatThrownBy(...)` for exceptions
- Names: `should_<expected>_when_<condition>` or equivalent

**Every `@Test` method must end on a real assertion.** Easy patterns to get wrong (the quality checker catches them, but it's cheaper to write them right the first time):

- **No-assertion tests**: a test whose body only calls SUT methods without any `assert*` / `verify` / `assertThat` / `StepVerifier` / `fail` is not a test — it's a smoke run that can only fail by exception. If the intent really is "doesn't throw", write that intent: `assertDoesNotThrow(() -> sut.foo())` or `assertThatNoException().isThrownBy(sut::foo)`. The quality checker reports this as **`no-assertion` (error)**.
- **`assertNotNull`-only tests**: `assertNotNull(x)` on a freshly constructed object, `Optional` return, or any value that the contract guarantees is non-null is a fake assertion — JVM and `Optional` already make the same guarantee. Same goes for `assertNotNull(x); assertTrue(x.isEmpty())` — the second call already implies non-null, so the first adds nothing. Assert the actual returned value or behavior instead. The quality checker reports this as **`only-not-null-assert` (error)**.

Run `scripts/check_test_quality.py <test_file> --sut <FQCN>` and fix every error (not just warnings) before proceeding to Step 5. Errors include `no-assertion`, `only-not-null-assert`, and `tautological-assert` — all of them mean the test is structurally incapable of catching the bug it claims to cover.

### Step 5 — The loop

This is the iterative core. See `references/loop_strategy.md` for full mechanics including stuck-loop detection, edge cases, and when to stop.

For each iteration (max 6):

1. **Run tests + coverage**

   Pick the script based on the SUT module's `has_jacoco_plugin` from Step 2:

   | has_jacoco_plugin | Script | What it does |
   |---|---|---|
   | true  | `scripts/run_coverage.sh <root> <FQCN>Test [--module <mod>]` | `mvn test ... jacoco:report` — uses the plugin already in the pom |
   | false | `scripts/run_coverage_agent.sh <root> <FQCN>Test [--module <mod>]` | `-DargLine=-javaagent:jacocoagent.jar=...` + `jacococli report` — **does not touch pom.xml** |

   Both print the absolute jacoco.xml path on stdout. Exit codes: 0 tests passed, 1 tests failed, 2 usage/env error, 3 plugin missing (mojo path only), 4 jacoco jars unobtainable (agent path only).

   If `run_coverage_agent.sh` exits 2 saying "jacoco.exec not produced", the project's pom likely overrides `<argLine>` for surefire (common in Spring Boot projects). Options: (a) ask the user to allow adding the jacoco plugin to the pom and switch to `run_coverage.sh`, or (b) read `references/jacoco_setup.md` for the surefire `argLine` collision workaround.

2. **Parse coverage**
   ```bash
   scripts/parse_coverage.py <jacoco_path> --line 80 --branch 70 --class <FQCN>
   ```
   Read `summary.all_pass`, `classes[0].uncovered_lines`, and `classes[0].partial_branches`.

3. **Check quality**
   ```bash
   scripts/check_test_quality.py <test_file> --sut <FQCN>
   ```

4. **Decide**
   - `summary.all_pass` is true AND quality has 0 errors → exit success, go to Step 6
   - Tests are failing → triage per `references/quality_rails.md` "Failure-mode catalog". Fix the test (never the SUT) and continue
   - Coverage gap → for each `uncovered_line`, read the SUT source around that line, identify the branch condition, write a focused test for that branch. Refer to `references/loop_strategy.md` table "Reading uncovered_lines into test ideas" for shape-based guidance
   - Quality errors → fix them before re-running coverage
   - Same `uncovered_lines` two iterations in a row → stuck; classify per `references/loop_strategy.md` "Stuck loops" and stop with a report

   Print a one-line status per iteration:
   ```
   iter 3/6 — line 78%→84% (target 80% ✓), branch 60%→68% (target 70% ✗), 2 quality errors → 0
   ```

5. **Loop** until exit condition or budget exhausted.

### Step 6 — Final report

When the loop exits, give the user:

- Final coverage numbers (line, branch — before → after)
- Iteration count used (e.g. `4 / 6`)
- Test files created or modified (paths)
- Any remaining quality warnings (errors should be zero by construction)
- Any uncovered lines and the reason (genuinely unreachable, or SUT refactor needed)

Be terse. The diff and the numbers speak for themselves.

## Bundled resources

| File | Purpose |
|---|---|
| `scripts/preflight.sh` | Validate mvn/JDK/settings.xml/parent-pom/deps/test-compile before the loop starts |
| `scripts/detect_project.py` | Find Maven pom, modules, test/main src layout, jacoco status |
| `scripts/run_coverage.sh` | Run `mvn test ... jacoco:report` (project has jacoco plugin), return jacoco.xml |
| `scripts/run_coverage_agent.sh` | Inject jacocoagent.jar via `-DargLine`, render report with jacococli — **no pom edits** |
| `scripts/parse_coverage.py` | Parse jacoco.xml → JSON with uncovered lines + thresholds check |
| `scripts/check_test_quality.py` | Static check for tautological asserts, only-not-null, no-assert, SUT mock, etc. |
| `references/test_style.md` | JUnit5 + Mockito + AssertJ idioms, naming, AAA, assertion patterns |
| `references/quality_rails.md` | Hard rules R1–R9 (what the checker enforces and why) + failure-mode catalog |
| `references/jacoco_setup.md` | When and how to add jacoco-maven-plugin (with consent) |
| `references/loop_strategy.md` | Iteration mechanics, stuck-loop handling, exit conditions |
| `assets/jacoco-pom-snippet.xml` | Drop-in plugin block |
| `assets/test_class_template.java` | Test class scaffold with placeholders |

## Things to refuse / push back on

- "Just lower the threshold" → no; surface the unreachable lines instead and ask whether to refactor or exclude
- "Mock the SUT to make the test simpler" → no; this is rule R4
- "Use reflection to test private methods" → no; this is rule R5; suggest extracting the helper to package-private
- "Add `@Disabled` to skip the failing test" → no; fix or delete
- "Modify `src/main/java/...` to add a setter for testing" → no; ask the user explicitly. If they confirm they want to refactor for testability, that becomes a separate task — not part of the loop
