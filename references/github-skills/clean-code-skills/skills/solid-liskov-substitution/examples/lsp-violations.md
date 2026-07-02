# LSP Violation Examples

## Violation 1: Strengthened Precondition

### Bad
```
class Collection {
  add(item) { this.items.push(item) }  // accepts any item
}

class UniqueCollection extends Collection {
  add(item) {
    if (this.items.includes(item)) throw new Error('Duplicate')  // rejects duplicates
    super.add(item)
  }
}
```
**Problem:** `UniqueCollection` rejects inputs that `Collection` accepts. Code written against `Collection` will break.

### Fix
```
// Use composition, not inheritance
class UniqueCollection {
  constructor() { this.items = new Set() }
  add(item) { this.items.add(item) }
}
```

---

## Violation 2: Weakened Postcondition

### Bad
```
class Repository {
  findById(id) { /* always returns an entity or throws NotFound */ }
}

class CachedRepository extends Repository {
  findById(id) { return this.cache[id] || null }  // returns null instead of throwing
}
```
**Problem:** Callers expect an entity or an exception, but get null instead.

### Fix
```
class CachedRepository extends Repository {
  findById(id) {
    if (this.cache[id]) return this.cache[id]
    return super.findById(id)  // preserves the contract
  }
}
```

---

## Violation 3: Broken Invariant

### Bad
```
class Rectangle {
  setWidth(w) { this.width = w }
  setHeight(h) { this.height = h }
  area() { return this.width * this.height }
}

class Square extends Rectangle {
  setWidth(w) { this.width = w; this.height = w }   // side effect!
  setHeight(h) { this.width = h; this.height = h }  // side effect!
}
```
**Problem:** `Rectangle` allows independent width and height. `Square` breaks this by coupling them. Code that sets width then height independently gets wrong results.

### Fix
```
// Separate types, no inheritance
class Rectangle { constructor(width, height) { ... } }
class Square { constructor(side) { ... } }
// Or use a shared Shape interface with area()
```

---

## Violation 4: Exception Introduction

### Bad
```
class FileStorage {
  save(data) { writeToFile(data) }
}

class ReadOnlyStorage extends FileStorage {
  save(data) { throw new Error('Read-only storage') }
}
```
**Problem:** Callers of `FileStorage.save()` do not expect an exception from `save()`.

### Fix
```
interface ReadableStorage { read(key): Data }
interface WritableStorage extends ReadableStorage { save(data): void }

class FileStorage implements WritableStorage { ... }
class ReadOnlyStorage implements ReadableStorage { ... }
// ReadOnlyStorage does not promise save()
```

---

## The Substitution Test

For any subtype S of base type T, this test must pass:

```
function testContract(instance: T) {
  // Call every public method of T with valid inputs
  // Assert postconditions hold
  // Assert invariants are preserved
}

// Must pass for every subtype
testContract(new ConcreteT())
testContract(new SubtypeS())
testContract(new SubtypeR())
```

If any subtype fails, it violates LSP.
