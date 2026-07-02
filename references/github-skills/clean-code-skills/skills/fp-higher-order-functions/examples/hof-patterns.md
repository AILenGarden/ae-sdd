# Higher-Order Function Patterns

## Pattern 1: Transform (map)

Apply a transformation to every element.

```
// Extract names from objects
const names = users.map(user => user.name)

// Convert temperatures
const celsius = fahrenheitValues.map(f => (f - 32) * 5/9)

// Format for display
const labels = items.map(item => `${item.name}: $${item.price.toFixed(2)}`)
```

## Pattern 2: Filter (filter)

Select elements that match a criterion.

```
const adults = users.filter(user => user.age >= 18)
const errors = logs.filter(log => log.level === 'error')
const inStock = products.filter(p => p.quantity > 0)
```

## Pattern 3: Accumulate (reduce)

Combine all elements into a single value.

```
const total = items.reduce((sum, item) => sum + item.price, 0)

const grouped = events.reduce((groups, event) => {
  const key = event.type
  groups[key] = groups[key] || []
  groups[key].push(event)
  return groups
}, {})
```

## Pattern 4: Function Factory

Return a function configured with specific behavior.

```
function createMultiplier(factor) {
  return (value) => value * factor
}

const double = createMultiplier(2)
const triple = createMultiplier(3)

double(5)  // 10
triple(5)  // 15
```

## Pattern 5: Function Composition

Combine multiple functions into one.

```
const compose = (...fns) => (x) => fns.reduceRight((v, f) => f(v), x)

const processUser = compose(
  formatForDisplay,
  validateAge,
  normalizeEmail
)

const result = processUser(rawUserData)
```

## Pattern 6: Middleware / Pipeline

Chain functions where each can modify or short-circuit the flow.

```
function createPipeline(...middlewares) {
  return (input) => middlewares.reduce(
    (result, middleware) => middleware(result),
    input
  )
}

const processRequest = createPipeline(
  authenticate,
  authorize,
  validateInput,
  execute
)
```

## Pattern 7: Partial Application

Pre-fill some arguments to create a specialized function.

```
function partial(fn, ...presetArgs) {
  return (...laterArgs) => fn(...presetArgs, ...laterArgs)
}

const addTax = partial(multiply, 1.08)
addTax(100)  // 108
```

## Chaining Best Practices

### Good: Clear pipeline
```
const result = orders
  .filter(order => order.status === 'completed')
  .map(order => order.total)
  .reduce((sum, total) => sum + total, 0)
```

### Bad: Unreadable nesting
```
const result = orders.reduce((sum, o) => o.status === 'completed' ? sum + o.total : sum, 0)
```

### Better: Named intermediate steps when complex
```
const completedOrders = orders.filter(isCompleted)
const orderTotals = completedOrders.map(toTotal)
const grandTotal = sum(orderTotals)
```
