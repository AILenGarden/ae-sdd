# Arrange-Act-Assert Pattern Examples

## Pattern 1: Simple Value Return

```
test('calculateDiscount returns 10% for orders over 100') {
  // Arrange
  const order = new Order({ total: 150.00 })

  // Act
  const discount = calculateDiscount(order)

  // Assert
  expect(discount).toBe(15.00)
}
```

**Why it works:** Single behavior, minimal setup, one clear assertion.

---

## Pattern 2: State Change

```
test('deactivateUser sets status to inactive') {
  // Arrange
  const user = new User({ status: 'active' })

  // Act
  user.deactivate()

  // Assert
  expect(user.status).toBe('inactive')
}
```

**Why it works:** The assertion verifies the side effect (state change) of the action.

---

## Pattern 3: Exception / Error Case

```
test('withdraw throws InsufficientFunds when balance is too low') {
  // Arrange
  const account = new Account({ balance: 50 })

  // Act & Assert
  expect(() => account.withdraw(100)).toThrow(InsufficientFundsError)
}
```

**Note:** When testing exceptions, Act and Assert may merge. This is the one acceptable deviation from strict separation.

---

## Pattern 4: Interaction Verification

```
test('registerUser sends welcome email') {
  // Arrange
  const emailService = createMock(EmailService)
  const registration = new RegistrationService(emailService)

  // Act
  registration.register('alice@example.com')

  // Assert
  expect(emailService.send).toHaveBeenCalledWith(
    'alice@example.com',
    expect.stringContaining('Welcome')
  )
}
```

**Why it works:** The mock verifies the interaction without coupling to email content details.

---

## Pattern 5: Parameterized / Table-Driven

```
test.each([
  { input: '',    expected: false },
  { input: 'abc', expected: false },
  { input: 'a@b', expected: false },
  { input: 'a@b.com', expected: true },
])('isValidEmail("$input") returns $expected', ({ input, expected }) => {
  // Arrange — embedded in parameters

  // Act
  const result = isValidEmail(input)

  // Assert
  expect(result).toBe(expected)
})
```

**Why it works:** Multiple scenarios reuse the same AAA structure, reducing duplication.

---

## Anti-Patterns to Avoid

### Mixed Phases
```
// BAD: setup and assertions interleaved
test('process order') {
  const item = createItem()
  cart.add(item)
  expect(cart.count()).toBe(1)    // assertion in the middle
  cart.checkout()
  expect(cart.count()).toBe(0)
}
```

### Excessive Arrangement
```
// BAD: most of this setup is irrelevant
test('getName returns full name') {
  const user = new User({
    name: 'Alice Smith',
    email: 'alice@example.com',   // irrelevant
    age: 30,                       // irrelevant
    address: '123 Main St',        // irrelevant
    preferences: { theme: 'dark' } // irrelevant
  })
  expect(user.getName()).toBe('Alice Smith')
}
```

### No Assert
```
// BAD: test runs code but verifies nothing
test('process data') {
  const data = loadTestData()
  processor.process(data)
  // ... no assertion
}
```
