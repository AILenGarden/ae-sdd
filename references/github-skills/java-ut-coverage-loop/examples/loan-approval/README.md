# loan-approval (toy fixture for end-to-end skill testing)

A self-contained Maven project used to validate `java-ut-coverage-loop` end-to-end. The SUT, `com.example.loan.LoanApprovalService`, has rich branching (15+ branches across 3 public methods), four collaborator interfaces to mock, and several exception paths — enough to exercise the skill's iteration loop, quality rails, and stuck-loop detection.

## What's deliberate about this fixture

| Feature | Why it's there |
|---|---|
| Multiple `@Mock` collaborators (4 interfaces) | Tests the skill's `@InjectMocks` + `@Mock` setup |
| Exceptions as control flow (`CreditBureauUnavailableException`, employment verifier `RuntimeException`) | Tests `assertThatThrownBy` and try/catch branch coverage |
| `Clock` injection for `LocalDate.now(clock)` | Tests the rule R6 (no flaky time primitives) — skill should use `Clock.fixed(...)` |
| Branching on enum (`EmploymentStatus`) and integer thresholds (credit tiers) | Tests `@ParameterizedTest` use; many small branches favor parameterization |
| `BigDecimal` arithmetic with rounding | Tests AssertJ's `isEqualByComparingTo` over `isEqualTo` |
| Three public methods (`evaluate`, `isPreApproved`, `computeMonthlyPayment`) | Lets the skill organize tests with `@Nested` classes |
| Guard clauses throwing `IllegalArgumentException` / `InvalidApplicationException` | Tests exception-path coverage |
| **No initial test file** | Forces the skill through the "create from scratch" path using `assets/test_class_template.java` |

## How to use this fixture to test the skill

### Step 1 — Verify the project builds on its own

```bash
cd examples/loan-approval
mvn compile
```

You should see `BUILD SUCCESS`. If this fails, the fixture is broken — fix it before invoking the skill.

### Step 2 — Confirm coverage tooling works

```bash
mvn test jacoco:report
```

**Important nuance:** with **zero test files**, you'll see `Skipping JaCoCo execution due to missing execution data file` and **no `jacoco.xml` is produced**. This is JaCoCo's intended behavior — no `.exec` data without at least one test. The skill's first iteration must generate initial test methods before JaCoCo will emit a report; subsequent iterations then read coverage normally.

To smoke-test the pipeline before invoking the skill, drop in any minimal test:

```java
// src/test/java/com/example/loan/SmokeTest.java
class SmokeTest {
    @Test void test() { assertThat(1).isEqualTo(1); }
}
```

Then `mvn test jacoco:report` → expect `target/site/jacoco/jacoco.xml` to exist. Delete `SmokeTest.java` after confirming.

### Step 2.5 — TLS workaround (Java 25 + Maven 3.9)

If `mvn` fails with `Remote host terminated the handshake`, run with explicit TLS settings:

```bash
MAVEN_OPTS="-Dhttps.protocols=TLSv1.2,TLSv1.3 -Dmaven.wagon.http.retryHandler.count=5" \
  mvn test jacoco:report
```

This is a Maven Wagon ↔ Java 25 TLS interaction, not a project issue.

### Step 3 — Trigger the skill in a fresh Claude Code session

Open a new Claude Code session in the **fixture directory** (not the skill repo):

```bash
cd /Users/z/projects/java-ut-coverage-loop/examples/loan-approval
claude
```

Then paste a triggering prompt like:

> 帮我把 `com.example.loan.LoanApprovalService` 的单测覆盖率提升到 line ≥ 80%、branch ≥ 70%。项目根目录就是当前目录。

The skill should auto-trigger from the description match. Watch for:

- Does Claude run `scripts/detect_project.py` and recognize the Maven layout?
- Does it correctly identify the SUT's collaborators (`CreditBureauClient`, `FraudDetectionService`, `EmploymentVerifier`, `LoanRepository`, `Clock`)?
- Does the initial test file land at `src/test/java/com/example/loan/LoanApprovalServiceTest.java`?
- Does each iteration print the one-line `iter N/6 — line A%→B% ...` status?
- Does it converge within the 6-iteration budget?
- Final quality-checker output: zero errors?

### Step 4 — Read the diff

After the skill exits, inspect what it produced:

```bash
cat src/test/java/com/example/loan/LoanApprovalServiceTest.java
mvn test jacoco:report
open target/site/jacoco/index.html  # human-readable coverage browser
```

Verify by hand:

- AAA structure with `// given / // when / // then` comments
- Test names follow `should_xxx_when_yyy` (or equivalent)
- All collaborators are `@Mock`, SUT is `@InjectMocks`
- Tests exercise both happy path AND each declined/referred reason code
- `Clock.fixed(...)` used to control age computation
- No `assertTrue(true)`, no null-only assertions
- No reflection on private members (`computeInterestRate`, `computeDti`, `computeAge` should be tested *through* `evaluate` and `computeMonthlyPayment`)

### Step 5 — Reset for re-runs

To re-run the skill from scratch:

```bash
rm -rf src/test/java target/
```

This clears the generated test file and Maven build output. Each fresh invocation starts from zero coverage.

## Expected approximate test count for full convergence

To hit 80% line / 70% branch on this SUT, the skill realistically needs roughly **15–25 test methods**, including:

- ~8 tests for declined/referred paths in `evaluate` (one per reason code)
- ~2–3 tests for approved paths (PRIME vs STANDARD, with employment-rate adjustments)
- ~2–3 tests for `evaluate`'s validation guards (null app, null applicant, non-positive amount, bad term)
- ~3–4 tests for `isPreApproved` (no applicant, low income, prime customer, delinquent, bureau unavailable)
- ~4–5 tests for `computeMonthlyPayment` (zero rate, normal rate, negative term, negative principal, negative rate)

A skilled human writes this in 30–60 minutes. A successful skill run should land in this neighborhood.

## What "failure" looks like for the skill

If the skill cannot reach the thresholds within 6 iterations on this fixture, common causes:

| Symptom | Likely root cause |
|---|---|
| Stuck on the same uncovered lines for 2+ iterations | Skill didn't read the SUT carefully enough to identify the branch condition |
| Tests fail with `NullPointerException` in setup | Forgot to stub a collaborator return value (e.g. `when(verifier.verify(any())).thenReturn(...)`) |
| Coverage rises but plateaus around 60–70% | Skill is missing the exception/error paths (try/catch blocks for `CreditBureauUnavailableException`, employment `RuntimeException`) |
| Quality checker reports `errors > 0` after coverage met | Tests use null-only assertions or are tautological |
| Tests pass but `evaluate(null)` branch shows uncovered | Skill forgot to write a test for the `IllegalArgumentException` guard |

These are all natural skill iteration failures — useful signals to surface in `references/loop_strategy.md` if they recur.
