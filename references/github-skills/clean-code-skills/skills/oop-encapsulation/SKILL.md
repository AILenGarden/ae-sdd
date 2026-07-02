---
name: oop-encapsulation
description: This skill should be used when the user asks about "encapsulation", "information hiding", mentions "exposing internal state", discusses public fields, getter/setter abuse, or wants to protect object invariants.
version: 1.0.0
---

# OOP: Encapsulation

Protect internal state and enforce invariants by hiding implementation details behind a well-defined public API, exposing behavior rather than data.

## When This Skill Applies
- Classes expose their fields directly (public instance variables)
- Getter/setter pairs exist for every field without protecting invariants
- External code manipulates object state instead of asking the object to do something
- The user mentions encapsulation, information hiding, or data protection
- Anemic domain models exist (data classes with no behavior)

## Core Principle
Objects should hide how they work and expose what they can do. Encapsulation means bundling data with the methods that operate on it and restricting direct access to internal state. Callers interact through behavior-rich methods that enforce business rules. This protects invariants, reduces coupling, and makes refactoring safe.

## Workflow

### Step 1: Identify Exposed State
Find classes with public fields, or private fields with trivial getters and setters. Look for external code that reads a field, makes a decision, and writes back — this logic belongs inside the object.

### Step 2: Identify the Behavior
Ask: "What decisions does external code make using this data?" Move that decision-making into the object as a method. The method name should describe the behavior, not the data access.

### Step 3: Replace Access with Behavior
Replace `if (account.balance >= amount) account.balance -= amount` with `account.withdraw(amount)`. The object enforces its own rules internally.

### Step 4: Remove Unnecessary Accessors
Delete getters and setters that are no longer needed. Keep only those required by framework constraints (serialization, ORM) and mark them accordingly.

### Step 5: Verify
Ensure all invariants are enforced by the object itself. No external code should be able to put the object into an invalid state. Run the test suite.

## Detection / Indicators
- Public fields or properties with no validation
- Getter/setter pairs for every field (anemic model)
- External code that reads a field, decides, then writes: `if (order.status == 'new') order.status = 'confirmed'`
- Feature envy: a method in class A uses mostly data from class B
- Objects passed around as data bags with no behavior
- Setters that allow invalid state: `account.setBalance(-100)`

## Transformation Pattern

**Before (exposed state):**
```
class BankAccount {
  public balance: number

  getBalance() { return this.balance }
  setBalance(b) { this.balance = b }
}

// External code
if (account.getBalance() >= amount) {
  account.setBalance(account.getBalance() - amount)
}
```

**After (encapsulated behavior):**
```
class BankAccount {
  private balance: number

  withdraw(amount) {
    if (amount > this.balance) throw new InsufficientFundsError()
    this.balance -= amount
  }

  hasAvailableFunds(amount) { return this.balance >= amount }
}
```

## Common Pitfalls
- Adding getters/setters reflexively for every field
- Exposing mutable collections (return copies or unmodifiable views)
- Using `toString()` to leak internal structure
- Breaking encapsulation "just for tests" — redesign instead
- Over-encapsulating: simple value objects can have public read access

## Additional Resources
### Reference Files
- **`references/encapsulation-checklist.md`** — Checklist for evaluating encapsulation quality
