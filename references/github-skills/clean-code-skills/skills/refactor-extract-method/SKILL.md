---
name: refactor-extract-method
description: This skill should be used when the user asks to "extract a method", "break up a long function", mentions "extract method refactoring", discusses reducing method length, or wants to decompose complex logic into smaller pieces.
version: 1.0.0
---

# Refactoring: Extract Method

Identify blocks of code that do a distinct thing and extract them into well-named methods to improve readability, reduce duplication, and enable reuse.

## When This Skill Applies
- A method is longer than 10-15 lines of logic
- A comment explains what the next block of code does (the comment becomes the method name)
- The same code block appears in multiple places
- A method operates at mixed levels of abstraction
- The user asks to simplify or decompose a function

## Core Principle
If you need a comment to explain what a block of code does, that block should be a method whose name replaces the comment. Short, focused methods with intention-revealing names make code read like prose. Extract until each method does exactly one thing at one level of abstraction.

## Workflow

### Step 1: Identify the Extraction Candidate
Look for code blocks separated by blank lines or comments, nested blocks inside loops or conditionals, or duplicated logic. The block should have a clear single purpose.

### Step 2: Name the New Method
Choose a name that describes what the code does, not how it does it. Use verbs for actions: `calculateTotal`, `validateInput`, `formatAddress`. If you struggle to name it, the block may be doing too much.

### Step 3: Determine Parameters and Return Value
Identify which local variables the block reads (these become parameters) and which it writes (these become the return value). Minimize parameters — more than three suggests the block needs further decomposition or a parameter object.

### Step 4: Extract and Replace
Move the code block to a new method. Replace the original block with a call to the new method. Ensure all variable references are resolved through parameters or return values.

### Step 5: Verify
Run the test suite. The behavior must not change. If tests fail, the extraction introduced a bug — review parameter passing and variable scope.

## Detection / Indicators
- Methods longer than 15 lines of logic
- Comments that explain "what" rather than "why"
- Deeply nested code (3+ levels of indentation)
- Repeated code blocks across methods
- Methods that mix high-level orchestration with low-level details

## Transformation Pattern

**Before:**
```
function processOrder(order) {
  // validate order
  if (!order.items || order.items.length === 0) {
    throw new Error('Order must have items')
  }
  if (!order.customer) {
    throw new Error('Order must have customer')
  }

  // calculate total
  let total = 0
  for (const item of order.items) {
    total += item.price * item.quantity
  }
  if (order.discount) {
    total = total * (1 - order.discount)
  }

  // send confirmation
  emailService.send(order.customer.email, `Total: ${total}`)
}
```

**After:**
```
function processOrder(order) {
  validateOrder(order)
  const total = calculateTotal(order)
  sendConfirmation(order.customer, total)
}
```

## Common Pitfalls
- Extracting methods that still need many parameters (decompose further)
- Naming methods after implementation: `loopOverItems` instead of `calculateTotal`
- Extracting too little (a one-liner that adds no clarity)
- Breaking variable scope without adjusting parameters
- Extracting into a different class when it should stay local

## Additional Resources
### Reference Files
- **`references/extraction-heuristics.md`** — Heuristics for when and how to extract methods
