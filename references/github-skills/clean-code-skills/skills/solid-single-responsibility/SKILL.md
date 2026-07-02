---
name: solid-single-responsibility
description: This skill should be used when the user asks about "single responsibility", "SRP", mentions "separation of concerns", discusses classes doing too much, or wants to identify when a class has multiple reasons to change.
version: 1.0.0
---

# SOLID: Single Responsibility Principle

Ensure each class, module, or function has one and only one reason to change by encapsulating a single cohesive responsibility.

## When This Skill Applies
- A class or module seems to do "too many things"
- Changes to one feature require modifying unrelated code
- The user mentions SRP, single responsibility, or separation of concerns
- A class has methods that serve different stakeholders
- Test setup is complex because the class has many dependencies

## Core Principle
A class should have one, and only one, reason to change. A "reason to change" corresponds to a stakeholder or business capability. When a class serves multiple stakeholders, changes requested by one stakeholder risk breaking functionality for another. Separate responsibilities into distinct classes to isolate change.

## Workflow

### Step 1: Identify Responsibilities
List everything the class does. Group methods by the stakeholder or capability they serve. If you find more than one group, the class has multiple responsibilities.

### Step 2: Name Each Responsibility
Give each group a clear name. If you cannot name a responsibility in domain terms, it may be an infrastructure concern (logging, persistence, formatting) mixed with business logic.

### Step 3: Extract into Separate Classes
Move each responsibility into its own class. The original class delegates to the new classes or is replaced entirely. Each new class should have a focused, cohesive API.

### Step 4: Verify Independence
Change one responsibility and confirm that no other class needs to change. Run the test suite. If a change in one class forces changes in another, the split was not clean.

## Detection / Indicators
- A class with more than 5-7 public methods serving different purposes
- Methods that do not use the same instance variables
- The class name contains "And" or is a generic noun: `UserManagerAndValidator`
- Tests require unrelated setup: testing email logic requires setting up the database
- Multiple developers frequently edit the same class for different reasons
- The class imports from many unrelated modules

## Transformation Pattern

**Before (mixed responsibilities):**
```
class Employee {
  calculatePay()       // payroll stakeholder
  generateReport()     // reporting stakeholder
  saveToDatabase()     // IT/persistence stakeholder
}
```

**After (single responsibility each):**
```
class PayCalculator { calculatePay(employee) }
class EmployeeReporter { generateReport(employee) }
class EmployeeRepository { save(employee) }
```

## Common Pitfalls
- Splitting too aggressively (every method in its own class)
- Confusing "does one thing" with "has one method" — a class can have multiple methods if they all serve the same responsibility
- Creating god classes that delegate to everything but own nothing
- Ignoring SRP for "small" classes that gradually grow

## Additional Resources
### Reference Files
- **`references/srp-indicators.md`** — Checklist for identifying SRP violations
