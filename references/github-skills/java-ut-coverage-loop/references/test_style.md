# Test Style: JUnit 5 + Mockito + AssertJ

The style this skill produces. Follow these conventions when writing or modifying tests.

## Imports (canonical set)

```java
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;
import org.junit.jupiter.params.provider.CsvSource;

import org.mockito.InjectMocks;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.junit.jupiter.api.extension.ExtendWith;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;
```

Use AssertJ (`assertThat`) for value assertions; use JUnit Jupiter `assertThrows` only when AssertJ is unavailable. Prefer `assertThatThrownBy` to AssertJ.

## Class skeleton

```java
@ExtendWith(MockitoExtension.class)
class OrderServiceTest {

    @Mock private InventoryRepository inventoryRepository;
    @Mock private PaymentClient paymentClient;
    @InjectMocks private OrderService orderService;

    @Nested
    @DisplayName("placeOrder")
    class PlaceOrder {
        @Test
        void should_return_confirmed_order_when_inventory_available() { ... }

        @Test
        void should_throw_OutOfStockException_when_inventory_zero() { ... }
    }
}
```

- One outer test class per SUT, named `<SUT>Test`.
- Group by method-under-test using `@Nested` inner classes when there are >3 tests for one method.
- Use `@DisplayName` only for nested classes / parameterized scenarios where the method name alone is unclear.

## Naming

Use `should_<expected_outcome>_when_<condition>`:
- `should_return_empty_list_when_filter_excludes_all`
- `should_throw_IllegalArgumentException_when_amount_negative`

Acceptable alternatives: `given_<state>_when_<action>_then_<outcome>`, or `<actionVerb>_<expectation>`. **Avoid** `test1`, `testFoo`, `goodCase`, `happyPath`.

## AAA structure

Every test body has three visually separated blocks:

```java
@Test
void should_apply_discount_when_loyalty_tier_is_GOLD() {
    // given
    Customer gold = aCustomerWithTier(LoyaltyTier.GOLD);
    Cart cart = aCartWithTotal(BigDecimal.valueOf(100));
    when(loyaltyService.discountFor(gold)).thenReturn(BigDecimal.valueOf(0.10));

    // when
    Receipt receipt = checkoutService.checkout(cart, gold);

    // then
    assertThat(receipt.total()).isEqualByComparingTo("90.00");
    verify(paymentClient).charge(gold, BigDecimal.valueOf(90));
}
```

The blank lines and `// given / // when / // then` comments are mandatory in this style — they make AAA visible at a glance and help reviewers spot tests that mix setup with assertion.

## Assertion patterns

| Need | Use |
|---|---|
| equality of value object | `assertThat(actual).isEqualTo(expected)` |
| BigDecimal | `assertThat(amount).isEqualByComparingTo("9.00")` |
| collection contents (order-insensitive) | `assertThat(list).containsExactlyInAnyOrder(a, b, c)` |
| collection contents (order-sensitive) | `assertThat(list).containsExactly(a, b, c)` |
| optional present | `assertThat(opt).contains(value)` |
| exception | `assertThatThrownBy(() -> svc.x()).isInstanceOf(Foo.class).hasMessageContaining("...")` |
| field-by-field on POJO | `assertThat(actual).usingRecursiveComparison().isEqualTo(expected)` |

## Mockito patterns

- **Mock collaborators only**, never the SUT.
- Stub with `when(...).thenReturn(...)`. For voids: `doThrow(...).when(mock).method()`.
- Use `verify` only when the side effect is the contract; do not over-verify trivial getter calls.
- Argument captors for asserting structured arguments: `ArgumentCaptor<Order> cap = ArgumentCaptor.forClass(Order.class); verify(repo).save(cap.capture()); assertThat(cap.getValue().status()).isEqualTo(...)`.
- `lenient()` only when a setup is shared across tests where one branch genuinely doesn't use the stub. Default to strict (which is the MockitoExtension default).

## Parameterized tests

Use for branches over the same logical method. One test per branch class.

```java
@ParameterizedTest
@CsvSource({
    "0,    REJECT",
    "100,  ACCEPT",
    "999,  ACCEPT",
    "1000, REJECT"  // edge of allowed range
})
void should_classify_amount_at_boundary(int amount, Decision expected) {
    assertThat(rule.classify(amount)).isEqualTo(expected);
}
```

## Coverage-driven test design (what to test for which lines)

| Line type | What to write |
|---|---|
| `if (cond) { A } else { B }` | one test for `A`, one for `B` |
| `switch (x) { case A: ...; case B: ...; default: ... }` | one test per case + one for default |
| `for / while` loop | empty input, single element, multiple elements |
| try/catch | one test for happy path, one that triggers each catch block |
| guard clause `if (x == null) throw IAE` | one test asserting the exception |
| early return | one test that triggers the early return, one that doesn't |
| stream pipelines | one test for empty stream, one with at least 2 elements covering the predicate |

## What good test files look like

- 1 file per production class, mirroring package
- 5–15 test methods is typical; >25 means the SUT probably needs splitting
- No conditionals (`if`, ternary) in test bodies — use parameterized tests instead
- No `Thread.sleep`, no `new Random()` without seed, no time-of-day dependencies (inject a `Clock`)
- Test data builders or `@BeforeEach` setup methods, not duplicated literals
