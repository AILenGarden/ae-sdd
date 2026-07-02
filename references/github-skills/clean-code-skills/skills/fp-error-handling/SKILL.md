---
name: fp-error-handling
description: This skill should be used when the user asks about "functional error handling", mentions "Result type", "Either type", "Option type", "Maybe monad", discusses replacing try-catch with types, or wants to handle errors without exceptions.
version: 1.0.0
---

# FP: Functional Error Handling

Handle errors using Result, Either, and Option types instead of exceptions, making error paths explicit, composable, and impossible to ignore.

## When This Skill Applies
- Try-catch blocks are scattered throughout the codebase
- Functions return null to indicate failure (null is ambiguous)
- Error handling is inconsistent or often forgotten
- The user mentions Result, Either, Option, Maybe, or functional error handling
- Thrown exceptions are used for control flow, not exceptional situations

## Core Principle
Exceptions are invisible in type signatures and easy to forget. Returning typed error values (Result, Either, Option) makes failure an explicit part of the function's contract. The caller must handle the error case — the type system enforces it. This creates self-documenting code where error paths are visible and composable.

## Workflow

### Step 1: Identify Error-Prone Functions
Find functions that throw exceptions, return null/undefined on failure, or use boolean success flags. These are candidates for typed error handling.

### Step 2: Choose the Right Type
- **Option/Maybe**: the value may or may not exist (no error details needed).
- **Result/Either**: the operation may succeed or fail with an error value.

### Step 3: Replace Exceptions with Return Types
Change the function signature to return `Result<Value, Error>` instead of throwing. The caller pattern-matches or chains on the result.

### Step 4: Compose Operations
Chain multiple fallible operations using `map`, `flatMap`/`chain`, and `mapError`. Each step transforms the success value while preserving the error path.

### Step 5: Handle at the Boundary
Convert typed errors to exceptions or HTTP responses at the system boundary (API handler, main function). Keep the domain pure.

## Detection / Indicators
- Try-catch used for business logic flow control
- Functions that return `null` to mean "not found" or "failed"
- Boolean return values for success/failure: `if (!save(data)) { ... }`
- Inconsistent error handling: some callers catch, others do not
- Deeply nested try-catch blocks
- Caught exceptions immediately re-thrown or logged and swallowed

## Transformation Pattern

**Before (exception-based):**
```
function parseAge(input) {
  const age = parseInt(input)
  if (isNaN(age)) throw new Error('Invalid age')
  if (age < 0) throw new Error('Age cannot be negative')
  return age
}

// Caller must remember to try-catch
try {
  const age = parseAge(input)
  save(age)
} catch (e) {
  showError(e.message)
}
```

**After (Result-based):**
```
function parseAge(input): Result<number, string> {
  const age = parseInt(input)
  if (isNaN(age)) return Err('Invalid age')
  if (age < 0) return Err('Age cannot be negative')
  return Ok(age)
}

// Caller must handle both cases
parseAge(input)
  .map(age => save(age))
  .mapErr(msg => showError(msg))
```

## Common Pitfalls
- Wrapping every function in Result (only use for operations that can genuinely fail)
- Using Option when the caller needs to know why something failed (use Result instead)
- Not providing `mapErr` or error transformation utilities
- Converting Result back to exceptions inside the domain (defeats the purpose)
- Over-engineering: simple validation can use exceptions if the language idiom supports it

## Additional Resources
### Reference Files
- **`references/either-result-option.md`** — Reference for Either, Result, and Option types with usage patterns
