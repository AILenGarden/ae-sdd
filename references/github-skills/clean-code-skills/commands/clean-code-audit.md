---
name: clean-code-audit
description: Run a comprehensive code quality audit using all 20 clean code skills. Analyzes the current codebase for TDD practices, refactoring opportunities, SOLID violations, OOP issues, functional programming improvements, hexagonal architecture alignment, and general code smells.
---

# Clean Code Audit

Perform a comprehensive code quality audit on the current project. Evaluate the codebase against all 20 clean code skills and produce a structured report.

## Audit Process

### Step 1: Scan the Codebase

Identify the primary language(s), framework(s), and project structure. Focus on source files, excluding vendored dependencies and generated code.

### Step 2: TDD Assessment

- Check for test files and test coverage patterns.
- Evaluate whether tests follow the Arrange-Act-Assert pattern.
- Look for test-first indicators (test files matching source files, comprehensive edge case coverage).
- Flag untested public methods or classes.

### Step 3: Refactoring Opportunities

- Identify long methods that should be extracted (more than 20 lines of logic).
- Flag poorly named variables, methods, or classes.
- Detect complex conditionals (nested if/else, switch statements with shared logic) that could use polymorphism.

### Step 4: SOLID Compliance

- **SRP**: Flag classes with multiple reasons to change (mixed concerns).
- **OCP**: Look for modification-heavy code that should use extension points.
- **LSP**: Check inheritance hierarchies for contract violations.
- **ISP**: Find large interfaces that force clients to depend on unused methods.
- **DIP**: Identify high-level modules directly depending on low-level implementations.

### Step 5: OOP Review

- Check for data classes with no behavior (anemic domain models).
- Flag exposed internal state (public fields, getter/setter pairs without invariants).
- Identify deep inheritance hierarchies that should use composition.

### Step 6: Functional Programming Opportunities

- Spot impure functions with hidden side effects.
- Identify loops that could be replaced with map/filter/reduce.
- Find null-based error handling that could use Result/Either/Option patterns.

### Step 7: Architecture Assessment

- Evaluate dependency direction (do domain modules depend on infrastructure?).
- Check for port/adapter separation at system boundaries.
- Flag domain logic mixed with I/O, serialization, or framework code.

### Step 8: Code Hygiene

- Apply the boy scout rule: identify areas where small improvements would compound.
- Catalog code smells: duplicated code, feature envy, long parameter lists, data clumps, primitive obsession.

## Output Format

Produce a report with the following sections:

```
## Clean Code Audit Report

### Summary
- Overall health: [Good / Needs Attention / Critical]
- Files scanned: N
- Issues found: N (H high / M medium / L low)

### Findings by Category

#### TDD (score: X/10)
- [H/M/L] Finding description → Recommendation

#### Refactoring (score: X/10)
...

#### SOLID (score: X/10)
...

#### OOP (score: X/10)
...

#### Functional Programming (score: X/10)
...

#### Architecture (score: X/10)
...

#### Code Hygiene (score: X/10)
...

### Top 5 Priority Actions
1. ...
2. ...
3. ...
4. ...
5. ...
```

Prioritize findings by impact. Be specific — reference file names, line numbers, and concrete suggestions. Keep recommendations actionable and incremental.
