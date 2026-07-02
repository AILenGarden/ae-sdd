# Interface Segregation Patterns

## Pattern 1: Role Interfaces

Split a single interface into multiple interfaces, each representing a role.

```
// Before
interface Document {
  read(): string
  write(content: string): void
  print(): void
  fax(): void
}

// After
interface Readable { read(): string }
interface Writable { write(content: string): void }
interface Printable { print(): void }
interface Faxable { fax(): void }
```

**Benefit:** A `ReadOnlyDocument` implements only `Readable`. A `FullDocument` implements all four.

## Pattern 2: Interface Composition

Combine small interfaces when a client needs multiple roles.

```
interface ReadWrite extends Readable, Writable {}

function processDocument(doc: ReadWrite) {
  const content = doc.read()
  doc.write(transform(content))
}
```

## Pattern 3: Adapter for Legacy Interfaces

When you cannot change a fat interface (third-party library), create an adapter.

```
// Fat third-party interface
interface BigLibraryApi {
  methodA(): void
  methodB(): void
  methodC(): void
  methodD(): void
}

// Your focused interface
interface OnlyWhatINeed {
  methodA(): void
  methodB(): void
}

class LibraryAdapter implements OnlyWhatINeed {
  constructor(private lib: BigLibraryApi) {}
  methodA() { this.lib.methodA() }
  methodB() { this.lib.methodB() }
}
```

## Sizing Guide

| Interface Size | Assessment | Action |
|---------------|------------|--------|
| 1 method | Possibly too granular | OK for functional interfaces (lambdas) |
| 2-5 methods | Ideal | Likely well-segregated |
| 6-10 methods | Review | Check if all clients use all methods |
| 10+ methods | Likely violation | Split by client usage patterns |

## Common Mistake: Over-Segregation

Do not create one interface per method. Group methods that are always used together by the same client. The goal is **client-focused** grouping, not **method-count minimization**.

### Test: Do all clients of this interface call all its methods?
- **Yes** → Interface is fine.
- **No** → Split by client usage.
