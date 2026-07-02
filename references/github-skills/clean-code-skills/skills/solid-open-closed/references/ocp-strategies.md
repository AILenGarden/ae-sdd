# Open-Closed Principle Strategies

## Strategy 1: Polymorphism (Strategy / Template Method)
Replace conditionals with an interface and multiple implementations.

**Use when:** Behavior varies by type and new types are expected.

```
interface Formatter { format(data): string }
class JsonFormatter implements Formatter { ... }
class XmlFormatter implements Formatter { ... }
// New: class CsvFormatter implements Formatter { ... }
```

## Strategy 2: Composition and Decoration
Wrap existing behavior with decorators that add functionality.

**Use when:** You want to layer behavior without modifying the original.

```
interface Logger { log(message): void }
class ConsoleLogger implements Logger { ... }
class TimestampLogger implements Logger {
  constructor(private inner: Logger) {}
  log(message) { this.inner.log(`[${Date.now()}] ${message}`) }
}
```

## Strategy 3: Configuration and Data-Driven Design
Move variation into configuration (maps, files, databases) rather than code.

**Use when:** Variants differ only in data, not in logic.

```
const taxRates = { 'US': 0.08, 'UK': 0.20, 'JP': 0.10 }
// Adding a new country: add an entry, no code change
function calculateTax(country, amount) { return amount * taxRates[country] }
```

## Strategy 4: Event-Driven / Plugin Architecture
Use events or hooks to allow external code to extend behavior.

**Use when:** Extensions come from separate modules or third parties.

```
class OrderProcessor {
  process(order) {
    // core logic
    this.emit('orderProcessed', order)  // plugins react
  }
}
```

## Strategy 5: Dependency Injection
Accept dependencies through constructors or parameters instead of hard-coding them.

**Use when:** The implementation of a dependency may change or vary.

```
class UserService {
  constructor(private repository: UserRepository) {}
  // Works with any UserRepository implementation
}
```

## Choosing the Right Strategy

| Situation | Best Strategy |
|-----------|--------------|
| Behavior varies by type | Polymorphism |
| Cross-cutting concerns | Decoration |
| Data-only variation | Configuration |
| Third-party extensibility | Events / Plugins |
| Swappable dependencies | Dependency Injection |

## The Rule of Three
Do not abstract on the first variation. Implement it concretely. When the second variation appears, consider abstracting. By the third, the pattern is clear — abstract with confidence.
