# Either, Result, and Option Types

## Option / Maybe

Represents a value that may or may not exist.

```
type Option<T> = Some<T> | None

Some(42)   // has a value
None       // no value
```

### When to Use
- Looking up a value that may not exist (dictionary, database find)
- Optional configuration or parameters
- First/last element of a possibly empty collection

### API
```
Option.map(fn)         // transform the value if present
Option.flatMap(fn)     // chain with another Option-returning function
Option.getOrElse(def)  // unwrap with a default value
Option.match({ some, none })  // pattern match
```

### Example
```
function findUser(id): Option<User> {
  const user = db.find(id)
  return user ? Some(user) : None
}

findUser(42)
  .map(user => user.email)
  .getOrElse('unknown@example.com')
```

---

## Result / Either

Represents a computation that may succeed or fail with an error value.

```
type Result<T, E> = Ok<T> | Err<E>

Ok(42)                    // success
Err('Not found')          // failure with reason
Err(new ValidationError)  // failure with typed error
```

### When to Use
- Parsing or validation that can fail
- External operations (API calls, file reads) that may fail
- Any function where callers need to know *why* it failed

### API
```
Result.map(fn)         // transform success value
Result.mapErr(fn)      // transform error value
Result.flatMap(fn)     // chain with another Result-returning function
Result.match({ ok, err })  // pattern match
Result.unwrap()        // get value or throw (use at boundaries only)
```

### Chaining Example
```
function processOrder(rawData) {
  return parseOrderData(rawData)       // Result<OrderData, ParseError>
    .flatMap(validateOrder)             // Result<ValidOrder, ValidationError>
    .flatMap(calculatePricing)          // Result<PricedOrder, PricingError>
    .map(formatConfirmation)            // Result<Confirmation, Error>
}
```

---

## Decision Guide

| Scenario | Type | Reason |
|----------|------|--------|
| Value may not exist | `Option` | No error info needed |
| Operation can fail, caller needs reason | `Result` | Error info preserved |
| Multiple error types possible | `Result<T, ErrorUnion>` | Typed error variants |
| Simple null check | Nullable type or `Option` | Depends on language |
| Critical system failure | Exception | Truly exceptional, not recoverable |

## Language-Specific Implementations

| Language | Option | Result |
|----------|--------|--------|
| Rust | `Option<T>` | `Result<T, E>` |
| Kotlin | `T?` | `Result<T>` / `Either` (Arrow) |
| Swift | `Optional<T>` / `T?` | `Result<Success, Failure>` |
| TypeScript | Custom / `fp-ts Option` | Custom / `fp-ts Either` |
| Java | `Optional<T>` | Custom / `vavr Either` |
| Scala | `Option[T]` | `Either[E, T]` / `Try[T]` |
| Go | `nil` return | `value, error` tuple |

## Anti-Patterns

### Catch-All
```
// Bad: loses error information
try { ... } catch (e) { return null }
```

### Ignore Result
```
// Bad: discards the error
parseAge(input)  // Result not used
```

### Result Inside Try-Catch
```
// Bad: mixing paradigms
try {
  const result = parseAge(input)
  if (result.isErr()) throw result.error  // defeats purpose
} catch (e) { ... }
```
