# SRP Violation Indicators

## Quick Assessment

Answer these questions about a class:

1. Can you describe what this class does in one sentence without using "and"?
2. Does every method use the same core set of instance variables?
3. Would a single stakeholder request all changes to this class?
4. Can you test each method without setting up unrelated dependencies?

If any answer is "no," the class likely violates SRP.

## Red Flags

### Naming Red Flags
- Class name includes: `Manager`, `Handler`, `Processor`, `Service` (vague, multi-purpose)
- Class name includes "And": `ReaderAndWriter`, `ValidatorAndFormatter`
- Class name is too broad: `UserHelper`, `DataUtils`, `ApplicationService`

### Structural Red Flags
- More than 200 lines of code
- More than 7 public methods
- More than 5 dependencies (constructor parameters)
- Methods that can be grouped into clusters that do not interact
- Private methods used by only one public method (candidate for extraction)

### Change Red Flags
- A single-line change in one method breaks a test for another method
- Two developers need to edit the same file for different features
- A bug fix for feature A requires understanding feature B's code

## Responsibility Categories

Common responsibilities to separate:

| Responsibility | Examples |
|---------------|----------|
| Business rules | Validation, calculation, policy enforcement |
| Persistence | Save, load, query, cache |
| Presentation | Format, render, serialize |
| Communication | Send email, publish event, call API |
| Orchestration | Coordinate workflow between services |
| Configuration | Read settings, apply defaults |

## Refactoring Strategies

### Strategy 1: Extract Class
Move a group of related methods and their data into a new class. The original class delegates to it.

### Strategy 2: Extract Interface
Define an interface for the responsibility, then move the implementation to a new class. This enables dependency inversion.

### Strategy 3: Decompose by Layer
Split along architectural layers: domain logic, application services, infrastructure adapters.
