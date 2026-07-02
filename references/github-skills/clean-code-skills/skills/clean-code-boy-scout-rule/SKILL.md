---
name: clean-code-boy-scout-rule
description: This skill should be used when the user asks about "boy scout rule", "leaving code cleaner", mentions "continuous improvement", discusses incremental code quality improvements, or wants to improve code they are already touching.
version: 1.0.0
---

# Clean Code: Boy Scout Rule

Leave every file you touch cleaner than you found it by making small, incremental improvements alongside your primary changes.

## When This Skill Applies
- Modifying a file that has minor quality issues
- Working in a codebase with accumulated technical debt
- The user mentions the boy scout rule or leaving code better
- During code review when reviewers suggest "while you're here" improvements
- After completing a feature, considering what small improvements to make

## Core Principle
"Always leave the campground cleaner than you found it." When you open a file to make a change, make one small improvement before you leave. Over time, these incremental improvements compound. The code gets cleaner every day without dedicated refactoring sprints. The rule applies only to code you are already touching — do not refactor files unrelated to your task.

## Workflow

### Step 1: Make Your Primary Change
Complete the feature, bug fix, or task that brought you to the file. Ensure it works and tests pass.

### Step 2: Scan for Quick Wins
Look for small improvements in the code you just touched or read:
- A variable that could have a better name
- A method that could be extracted
- A comment that is redundant or outdated
- Dead code that can be removed
- An import that is unused

### Step 3: Apply One Improvement
Make one small, low-risk improvement. Keep it in the same commit or a separate commit — but keep it small. The improvement should take less than 5 minutes.

### Step 4: Verify
Run the test suite. The improvement must not change behavior. If it does, it is not a "clean up" — it is a refactoring that deserves its own process.

### Step 5: Document If Needed
If the improvement is not obvious from the diff, add a brief note in the commit message: "Also: renamed ambiguous variable `d` to `elapsedDays`."

## Detection / Indicators
- Files with minor quality issues that persist for months because "it's not my task"
- Codebase quality slowly degrades over time
- Developers avoid touching certain files because they are messy
- Code review comments like "this name is confusing but it's out of scope"
- Technical debt grows without any effort to reduce it

## Transformation Pattern

**Primary change:** Add a new validation rule.

**Boy scout improvements (pick one):**
- Rename `val` to `validationResult`
- Remove the commented-out code block above
- Delete the unused import at the top
- Fix the misleading comment on line 42
- Extract the duplicated validation into a helper

## Common Pitfalls
- Making too many improvements in one pass (keep it small)
- Improving files you did not need to touch (scope creep)
- Making improvements that change behavior (that is refactoring, not cleanup)
- Skipping tests after improvements (always verify)
- Using the rule to justify large refactorings inside a feature PR

## Additional Resources
### Reference Files
- **`references/improvement-checklist.md`** — Quick-reference checklist for boy scout improvements
