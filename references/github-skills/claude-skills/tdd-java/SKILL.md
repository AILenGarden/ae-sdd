---
name: tdd-java
description: >
  Use when implementing or changing any Java/Spring production code — new
  features, bug fixes, refactors — or when tempted to "just write the code
  and add tests after". Also use when a test is failing and the urge appears
  to weaken, delete, or hardcode around it, when test coverage feels like a
  chore to backfill, or when deciding between unit, slice (@WebMvcTest,
  @DataJpaTest), and Testcontainers integration tests.
---

# TDD for Java/Spring

Core principle: a test you watched fail is the only proof the test can fail — everything else is hope.

```
THE IRON LAW

NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST.
No exceptions: not for "trivial" changes, not for bug fixes,
not for "I'll add tests right after". After is too late.
```

If the change is pure non-behavior (rename via IDE, comment, formatting), say so explicitly and proceed. Everything else goes through the loop.

## Phase 1: RED — write the smallest failing test

1. Pick ONE behavior. Write the smallest test that demands it.
2. Name it `methodUnderTest_condition_expectedOutcome` or use `@DisplayName`.
3. Run it: `mvn test -Dtest=OrderServiceTest` (Gradle: `./gradlew test --tests OrderServiceTest`).
4. **GATE: confirm it fails FOR THE RIGHT REASON.** Show the assertion error in your output. A compile error or `NullPointerException` in setup is not a valid RED — fix the test until the failure is the missing behavior itself.

```java
@Test
void applyDiscount_orderAboveThreshold_reducesTotalByTenPercent() {
    var order = new Order(new BigDecimal("200.00"));

    var total = new DiscountService().applyDiscount(order);

    assertThat(total).isEqualByComparingTo("180.00");
}
// RED run output:
// expected: 180.00 but was: 200.00   <-- failing for the right reason
```

## Phase 2: GREEN — minimum code to pass

1. Write the least production code that makes the test pass. No speculative parameters, no extra branches "while you're in there".
2. Run the test again. **GATE: show the passing result.** Then run the full suite (`mvn verify` / `./gradlew check`) — a pass that breaks something else is not green.

```java
public BigDecimal applyDiscount(Order order) {
    if (order.total().compareTo(new BigDecimal("100")) > 0) {
        return order.total().multiply(new BigDecimal("0.90"));
    }
    return order.total();
}
```

**Circuit-breaker: 3 consecutive failed GREEN attempts → STOP.** The design is fighting you. Do not try a 4th patch. Re-examine the design — consult `designing-systems` — and bring the analysis to the user before continuing.

## Phase 3: REFACTOR — only with a green bar

- Refactor production code AND test code, but only while every test passes.
- Run the full suite after each refactor step. Red bar during refactor → revert the step, not the test.
- Then loop back to RED for the next behavior.

## Rules that have no workaround

- **Every behavior gets positive AND negative cases.** Happy path alone is half a spec: nulls, empty collections, boundary values, exception paths.
- **Never weaken or delete a failing test to make the build pass.** If you believe the test itself is wrong, STOP, say so explicitly, and let the user decide. Changing the assertion to match the bug is the same as deleting the test.
- **Assert behavior, not implementation.** If a state assertion is possible, do not `verify()` internal interactions instead. Mockito `verify` is for observable side effects (a message published, an email sent), not for "the service called the repository".
- **No test gaming.** Hardcoding the expected value in production code, catching the assertion's exception, or special-casing test input is violating the letter AND the spirit.

## Test pyramid

| Level | Tool | Use for | Cost |
|---|---|---|---|
| Unit | JUnit 5 + Mockito | Domain logic, services — the bulk | ms |
| Slice | `@WebMvcTest`, `@DataJpaTest` | Controller serialization/validation, JPA queries | ~1s |
| Integration | `@SpringBootTest` + Testcontainers | Wiring, real Postgres/Kafka, critical flows only | ~10s+ |

Default downward: write a unit test unless the behavior under test IS the framework boundary. Setup boilerplate, Testcontainers config, Mockito do/don't, parameterized tests, and Awaitility for async: see `references/testing-recipes.md`.

## Rationalization table

| Excuse | Reality |
|---|---|
| "It's a trivial change, a test is overkill" | Trivial changes cause outages precisely because nobody tests them. Trivial test, then. |
| "I'll write the tests right after the code works" | Tests written after pass against the code, not the requirement. They prove nothing. |
| "The deadline is tight, TDD slows us down" | Debugging untested code under the same deadline is slower. TDD is the fast path. |
| "This is just a spike / prototype" | Fine — but spike code gets deleted, not merged. If it's heading to a branch, it gets tests first. |
| "The failing test is outdated, I'll just update the assertion" | Maybe — but that's the user's call. Updating an assertion to match new output is how regressions ship. |
| "It's too hard to test, the class needs too many mocks" | That's the test telling you the design is wrong. Fix the design (see `designing-systems`), don't skip the test. |
| "I already manually verified it works" | Manual verification evaporates on the next change. The test is the durable proof. |
| "Existing code here has no tests anyway" | You're touching it now, so the behavior you change gets a test. Broken windows are not a license. |

## Red flags — stop if you catch yourself writing

- "Let me implement this first and then add tests"
- "The test fails but the code is correct, so I'll adjust the test"
- "I'll mark this test `@Disabled` for now"
- "Testing this case is unlikely to matter"
- "I'll verify the mock was called instead of checking the result"
- "Let me just hardcode this to get the test green"
- Writing a second production method before the first one's test exists

Any of these means: return to Phase 1.

## Verification checklist

Before declaring the task done:

- [ ] Every new behavior had a test that I ran and SAW fail with an assertion error
- [ ] Every test now passes; I showed the passing run, not just claimed it
- [ ] Positive and negative cases exist for each behavior
- [ ] No test was weakened, deleted, or `@Disabled` to get to green
- [ ] Assertions check outcomes/state, not internal call sequences
- [ ] Full suite passes: `mvn verify` (or `./gradlew check`)

## Related skills

- `designing-systems` — when the circuit-breaker trips or the code resists testing
- `reviewing-java-code` — review pass over the finished change, including test quality
- `spring-boot-standards` — what the production code itself should look like
- `jpa-database-patterns` — what `@DataJpaTest` and Testcontainers tests should catch
