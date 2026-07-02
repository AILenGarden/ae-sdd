# Code Smell Catalog

## Category 1: Bloaters

### Long Method
**What:** A method with too many lines of code (typically more than 15 lines of logic).
**Why it hurts:** Hard to read, hard to test, hard to reuse parts.
**Fix:** Extract Method — break into smaller, named methods.

### Large Class
**What:** A class that does too much (many fields, many methods, many lines).
**Why it hurts:** Hard to understand, violates SRP, attracts more responsibility.
**Fix:** Extract Class — split by responsibility.

### Long Parameter List
**What:** A method with more than 3 parameters.
**Why it hurts:** Hard to call correctly, hard to remember parameter order.
**Fix:** Introduce Parameter Object or use a builder pattern.

### Data Clumps
**What:** The same group of data items appears together in multiple places (e.g., `street`, `city`, `zipCode`).
**Why it hurts:** Duplication, missing abstraction.
**Fix:** Extract a value object (e.g., `Address`).

### Primitive Obsession
**What:** Using primitive types for domain concepts: `string` for email, `number` for money.
**Why it hurts:** No validation, no behavior, easy to confuse (pass dollars where cents expected).
**Fix:** Create value objects: `EmailAddress`, `Money`, `PhoneNumber`.

---

## Category 2: Couplers

### Feature Envy
**What:** A method that accesses data from another object more than its own.
**Why it hurts:** Logic is in the wrong place. Changes to the data class require changes here.
**Fix:** Move the method to the class it envies.

### Inappropriate Intimacy
**What:** Two classes that know too much about each other's internals.
**Why it hurts:** Tight coupling, changes ripple between both.
**Fix:** Move methods, extract class, or introduce an interface.

### Message Chains
**What:** A chain of method calls: `order.getCustomer().getAddress().getCity()`.
**Why it hurts:** Caller is coupled to the entire chain structure.
**Fix:** Hide the delegate — add `order.getShippingCity()`.

### Middle Man
**What:** A class that delegates almost everything to another class.
**Why it hurts:** Adds indirection without value.
**Fix:** Remove the middle man or inline the class.

---

## Category 3: Change Preventers

### Divergent Change
**What:** One class is modified for many different reasons.
**Why it hurts:** Violates SRP, high merge conflict risk.
**Fix:** Extract classes by responsibility.

### Shotgun Surgery
**What:** A single change requires editing many classes across the codebase.
**Why it hurts:** Easy to miss one edit, high regression risk.
**Fix:** Move related logic into one class or module.

### Parallel Inheritance Hierarchies
**What:** Adding a subclass in one hierarchy forces adding one in another.
**Why it hurts:** Hidden coupling between hierarchies.
**Fix:** Merge hierarchies or use composition.

---

## Category 4: Dispensables

### Dead Code
**What:** Code that is never executed — unused variables, unreachable branches, commented-out blocks.
**Why it hurts:** Noise, misleading, maintenance burden.
**Fix:** Delete it. Version control has the history.

### Speculative Generality
**What:** Abstractions, hooks, or parameters that exist "just in case" but have only one user.
**Why it hurts:** Complexity without value.
**Fix:** Remove the abstraction. Add it back when needed (YAGNI).

### Duplicate Code
**What:** Identical or near-identical code in multiple places.
**Why it hurts:** Bug fixes must be applied everywhere; easy to miss one.
**Fix:** Extract Method, Extract Class, or Pull Up Method.

### Lazy Class
**What:** A class that does too little to justify its existence.
**Why it hurts:** Adds indirection without value.
**Fix:** Inline the class into its caller.

---

## Prioritization Matrix

| Impact \ Effort | Low Effort | Medium Effort | High Effort |
|-----------------|-----------|--------------|------------|
| **High Impact** | Fix now | Fix soon | Plan it |
| **Medium Impact** | Fix during boy scout | Schedule | Defer |
| **Low Impact** | Boy scout | Defer | Ignore |

**High impact smells:** Smells in code that is changed frequently.
**Low impact smells:** Smells in stable code that is rarely touched.

Fix smells in code you are actively working on. Leave stable, rarely-changed code alone.
