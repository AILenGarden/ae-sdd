# Composition Patterns

## Pattern 1: Strategy Pattern

Encapsulate an algorithm behind an interface and inject it.

```
interface SortStrategy { sort(items): items }
class QuickSort implements SortStrategy { sort(items) { /* ... */ } }
class MergeSort implements SortStrategy { sort(items) { /* ... */ } }

class DataProcessor {
  constructor(private sortStrategy: SortStrategy) {}
  process(data) {
    return this.sortStrategy.sort(data)
  }
}
```

**Benefit:** Change sorting algorithm at runtime without modifying `DataProcessor`.

## Pattern 2: Decorator Pattern

Wrap an object to add behavior while keeping the same interface.

```
interface Logger { log(msg: string): void }

class ConsoleLogger implements Logger {
  log(msg) { console.log(msg) }
}

class TimestampLogger implements Logger {
  constructor(private inner: Logger) {}
  log(msg) { this.inner.log(`[${new Date().toISOString()}] ${msg}`) }
}

class FilteredLogger implements Logger {
  constructor(private inner: Logger, private minLevel: string) {}
  log(msg) { if (this.shouldLog(msg)) this.inner.log(msg) }
}

// Compose: filtered + timestamped + console
const logger = new FilteredLogger(new TimestampLogger(new ConsoleLogger()), 'WARN')
```

## Pattern 3: Delegation

A class delegates work to a contained object instead of inheriting behavior.

```
// Instead of: class Stack extends ArrayList
class Stack {
  private items = new ArrayList()

  push(item) { this.items.add(item) }
  pop() { return this.items.removeLast() }
  peek() { return this.items.getLast() }
  // Stack does NOT expose ArrayList's full API
}
```

**Benefit:** `Stack` controls its own API. Callers cannot call `ArrayList.get(index)` on a `Stack`.

## Pattern 4: Mixins / Traits (Functional Composition)

Compose behavior by mixing functions into an object.

```
const Serializable = (Base) => class extends Base {
  serialize() { return JSON.stringify(this) }
}

const Validatable = (Base) => class extends Base {
  validate() { /* validation logic */ }
}

class User extends Serializable(Validatable(BaseModel)) { ... }
```

## When Inheritance Is Still Appropriate

| Scenario | Recommendation |
|----------|---------------|
| True "is-a" relationship that is stable | Inheritance OK |
| Sharing implementation across 2-3 related classes | Inheritance OK |
| Framework requires it (e.g., React class components) | Inheritance OK |
| Varying behavior across many dimensions | Use composition |
| Combining behaviors from multiple sources | Use composition |
| Hierarchy deeper than 3 levels | Refactor to composition |

## Decision Flowchart

1. Is there a genuine, stable "is-a" relationship? → Consider inheritance.
2. Does the child use all of the parent's behavior? → If no, use composition.
3. Will you need to combine behaviors in different ways? → Use composition.
4. Is the hierarchy deeper than 2-3 levels? → Refactor to composition.
5. Could the base class change in ways that break children? → Use composition.
