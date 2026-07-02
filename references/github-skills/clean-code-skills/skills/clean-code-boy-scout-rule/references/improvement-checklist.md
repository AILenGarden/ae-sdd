# Boy Scout Improvement Checklist

Use this checklist when you are about to leave a file you have modified. Pick at most one or two items.

## Naming (1-2 minutes)
- [ ] Rename a single-letter or abbreviated variable to a descriptive name
- [ ] Rename a method to better describe what it does
- [ ] Fix a misleading name that does not match current behavior

## Dead Code (1 minute)
- [ ] Remove an unused import
- [ ] Remove a commented-out code block
- [ ] Remove an unused variable or method
- [ ] Remove a TODO comment that has been done

## Comments (1-2 minutes)
- [ ] Remove a comment that restates the code
- [ ] Update an outdated comment that no longer matches the code
- [ ] Replace a "what" comment with a well-named method

## Structure (2-5 minutes)
- [ ] Extract a clearly identifiable block into a named method
- [ ] Inline a variable that is used only once and adds no clarity
- [ ] Replace a magic number with a named constant
- [ ] Simplify a boolean expression: `if (x == true)` → `if (x)`

## Formatting (1 minute)
- [ ] Fix inconsistent indentation in the section you changed
- [ ] Add a blank line to separate logical blocks
- [ ] Remove unnecessary blank lines that break reading flow

## Safety Rules

1. **Scope**: Only improve code in files you are already modifying.
2. **Size**: One or two improvements per file, per commit.
3. **Risk**: Zero behavioral change. If in doubt, skip it.
4. **Tests**: Run the suite after every improvement.
5. **Time**: If it takes more than 5 minutes, it is a separate task.

## Examples of Good Boy Scout Changes

| Before | After | Time |
|--------|-------|------|
| `int d; // elapsed time in days` | `int elapsedDays;` | 30 sec |
| `// TODO: remove this after migration` (migration done 6 months ago) | *(deleted)* | 10 sec |
| `if (result != null && result.isValid() == true)` | `if (result?.isValid())` | 30 sec |
| Unused `import { debounce } from 'lodash'` | *(deleted)* | 10 sec |
