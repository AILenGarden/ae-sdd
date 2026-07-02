# Quality Rails (hard rules to keep tests honest)

These are the rules that `scripts/check_test_quality.py` enforces, plus the why and the fix-it pattern. When the checker reports a finding, refer back here.

## R1 — No tautological assertions (error)

Forbidden:
```java
assertTrue(true);
assertFalse(false);
assertThat(true).isTrue();
```

Why: a test must be able to fail when the SUT is wrong. These never can.

Fix: assert on the actual behavior or returned value of the SUT.

## R2 — No null-only assertions (error)

Forbidden as the *only* assertion in a test:
```java
assertNotNull(result);
assertThat(result).isNotNull();
```

Why: most production code returns non-null without doing anything correct. `isNotNull` rarely catches a real bug.

Fix: assert on a property of `result` that depends on inputs (`isEqualTo`, `hasSize`, field values, ...). `isNotNull` is fine as one of several assertions.

## R3 — Every test has at least one assertion or verification (error)

A test with no `assertThat`, no `assertEquals`, no `verify` is dead weight. It contributes coverage but proves nothing.

Fix: every `@Test` body ends in at least one assertion that depends on the act-phase output, or a `verify(...)` that asserts a contract was honored.

## R4 — Never mock the SUT (error)

Forbidden:
```java
@Mock private OrderService orderService;          // OrderService is the SUT
when(orderService.placeOrder(any())).thenReturn(...); // stubbing the SUT
```

Why: if you mock the SUT, the test is testing your mock, not the code.

Fix: `@InjectMocks` the SUT and `@Mock` only its collaborators. If you find yourself wanting to stub a method on the SUT, that method is probably a separate seam — extract it to a collaborator and inject.

## R5 — Don't bypass private members via reflection (warn)

Forbidden:
```java
Method m = sut.getClass().getDeclaredMethod("internalCalc", int.class);
m.setAccessible(true);
m.invoke(sut, 42);

ReflectionTestUtils.invokeMethod(sut, "internalCalc", 42);
```

Why: tests against private members lock down the implementation, not the contract. They break on every refactor.

Fix: drive private logic through the public API. If a private method has logic complex enough to need its own tests, extract it to a package-private helper or a separate class.

## R6 — No flaky primitives (warn)

Forbidden:
```java
Thread.sleep(100);
new Random().nextInt();
LocalDateTime.now();
new Date();
```

Why: nondeterministic tests fail intermittently and erode trust in the suite.

Fix:
- Time: inject a `Clock` (`Clock.fixed(Instant.parse("2026-01-01T00:00:00Z"), ZoneOffset.UTC)`)
- Randomness: inject a seeded `Random(42L)` or a stub
- Async: use Awaitility (`await().atMost(Duration.ofSeconds(2)).until(...)`)

## R7 — No catch-and-pass (warn)

Forbidden:
```java
try {
    sut.doIt();
} catch (Exception e) {
    // ignore
}
```

Why: silent swallowing turns an exception bug into a pass.

Fix: `assertThatThrownBy(() -> sut.doIt()).isInstanceOf(...)` — let the test fail if the wrong exception (or none) is thrown.

## R8 — Naming follows the pattern (warn)

Names must start with `should_`, `test`, `given_`, or `when_`, or use camelCase that includes an underscore separator (`accepts_when_amount_positive`). `goodCase`, `happyPath`, `runIt`, `test1` are flagged.

Why: a name is the first thing a reviewer reads. `should_throw_when_negative` tells you the contract; `runIt` tells you nothing.

## R9 — Disabled tests do not count (warn)

`@Disabled` / `@Ignore` tests contribute zero coverage. If a test is disabled, either fix it or delete it; do not let it sit in the file.

## What is NOT enforced (judgment calls)

- Test count per method — sometimes one parameterized test covers 6 cases
- Use of `@BeforeEach` vs inline setup — context-dependent
- Whether to use `@Nested` — stylistic
- Argument captors vs eq() — depends on how much of the argument matters

These are documented in `test_style.md` as preferences, not hard rules.

## Failure-mode catalog (to recognize when reading test failures)

When `mvn test` fails, classify before fixing:

| Symptom | Likely cause | Action |
|---|---|---|
| `NullPointerException` in setup | unstubbed collaborator returned null | add `when(mock.x()).thenReturn(...)` |
| `UnnecessaryStubbingException` | stub configured but not exercised | remove the stub or move it inside the specific test |
| `WantedButNotInvoked` | `verify` against a method the SUT never called | check the SUT path actually flows through that branch; otherwise remove verify |
| `ArgumentMatchers + raw values mixed` | `verify(x).y(eq(1), 2)` instead of `eq(2)` | use matchers consistently across all args |
| compile error: cannot find symbol | imported class doesn't exist | re-check the FQCN; the SUT may use a different package |
| test passes but coverage didn't rise | the asserted path doesn't actually exercise the line | re-read the SUT, find the *exact* condition required to enter that branch, build inputs that satisfy it |
