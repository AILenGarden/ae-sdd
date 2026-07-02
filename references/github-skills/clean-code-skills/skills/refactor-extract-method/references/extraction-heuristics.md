# Extraction Heuristics

## When to Extract

### The Comment Heuristic
If a comment describes what the next block does, the comment text is your method name. Delete the comment and replace the block with a method call.

### The Indent Heuristic
Code inside an `if`, `for`, `while`, or `try` block is a candidate for extraction, especially if the body is longer than 5 lines.

### The Reuse Heuristic
If the same logic appears in two or more places, extract it. Even slight variations can often be unified with a parameter.

### The Abstraction Level Heuristic
When a method mixes orchestration (`processOrder`) with details (`total += item.price * item.quantity`), extract the details into their own methods so the parent reads at a consistent level of abstraction.

### The Testability Heuristic
If you want to test a specific behavior that is buried inside a larger method, extract it so you can test it in isolation.

## How to Name Extracted Methods

| Bad Name | Good Name | Why |
|----------|-----------|-----|
| `doStuff` | `validateInput` | Describes the purpose |
| `helper` | `formatCurrency` | Describes the result |
| `process` | `applyDiscount` | Specifies the action |
| `handle` | `retryWithBackoff` | Reveals the strategy |

### Naming Rules
1. Use a verb or verb phrase.
2. Name after the **what**, not the **how**.
3. If the name includes "and", the method does too much — split it.
4. If you cannot name it, you do not understand what it does yet.

## Parameter Guidelines

| Parameter Count | Action |
|----------------|--------|
| 0 | Ideal — method uses instance state |
| 1-2 | Good — clear inputs |
| 3 | Acceptable — consider a parameter object |
| 4+ | Too many — decompose further |

## Extraction Checklist

1. [ ] Identify the block boundaries.
2. [ ] List all variables the block reads (inputs).
3. [ ] List all variables the block writes (outputs).
4. [ ] Choose an intention-revealing name.
5. [ ] Create the method with inputs as parameters and output as return value.
6. [ ] Move the code block into the new method.
7. [ ] Replace the original block with a call.
8. [ ] Run the tests.
9. [ ] Check that the parent method now reads at one level of abstraction.
