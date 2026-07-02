# Purity Rules

## The Two Rules of Pure Functions

### Rule 1: Deterministic
Same inputs → same output. Always. Every time.

**Pure:**
```
function add(a, b) { return a + b }
function fullName(first, last) { return `${first} ${last}` }
```

**Impure (non-deterministic):**
```
function getTimestamp() { return Date.now() }
function generateId() { return Math.random().toString(36) }
function readConfig() { return process.env.CONFIG }
```

### Rule 2: No Side Effects
The function does not change anything outside its scope.

**Side effects include:**
- Modifying a global or external variable
- Modifying an input parameter
- Writing to a database, file, or network
- Logging to the console
- Throwing an exception
- Reading from mutable external state

## The Purity Test

Replace the function call with its return value. Does the program behave identically?

```
// Pure: can replace call with value
const x = add(2, 3)    // → const x = 5 ✓

// Impure: cannot replace call with value
const y = saveUser(u)  // → const y = { id: 1 } ✗ (database not updated)
```

## Immutability Patterns

### Objects
```
// Mutating (bad)
user.name = 'Alice'

// Immutable (good)
const updatedUser = { ...user, name: 'Alice' }
```

### Arrays
```
// Mutating (bad)
items.push(newItem)
items.sort()

// Immutable (good)
const newItems = [...items, newItem]
const sorted = [...items].sort()
```

### Nested Updates
```
// Mutating (bad)
order.customer.address.city = 'Berlin'

// Immutable (good)
const updated = {
  ...order,
  customer: {
    ...order.customer,
    address: { ...order.customer.address, city: 'Berlin' }
  }
}
```

## The Impure Shell / Pure Core Pattern

```
// Pure core — all business logic
function calculatePrice(items, taxRate, discountRules) {
  const subtotal = items.reduce((sum, i) => sum + i.price * i.qty, 0)
  const discount = applyDiscountRules(subtotal, discountRules)
  const tax = (subtotal - discount) * taxRate
  return { subtotal, discount, tax, total: subtotal - discount + tax }
}

// Impure shell — I/O at the edges
async function handleCheckout(cartId) {
  const items = await db.getCartItems(cartId)          // impure: DB read
  const taxRate = await taxService.getRate(region)     // impure: API call
  const discountRules = await db.getDiscountRules()    // impure: DB read

  const price = calculatePrice(items, taxRate, discountRules)  // PURE

  await db.saveOrder(cartId, price)                    // impure: DB write
  await emailService.sendReceipt(price)                // impure: email
}
```

## Common Impure Operations and Pure Alternatives

| Impure | Pure Alternative |
|--------|-----------------|
| `Date.now()` | Pass timestamp as parameter |
| `Math.random()` | Pass random seed or value as parameter |
| `console.log()` | Return log messages as data |
| `throw new Error()` | Return `Result` or `Either` type |
| `array.push(item)` | `[...array, item]` |
| `obj.field = value` | `{ ...obj, field: value }` |
