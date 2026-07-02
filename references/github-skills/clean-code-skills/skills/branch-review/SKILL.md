---
name: branch-review
description: This skill should be used when the user asks to "review a branch", "compare code quality", mentions "branch review", "code quality scorecard", discusses comparing old vs new code, or wants a scored assessment of changes on a feature branch.
version: 1.0.0
---

# Branch Code Quality Review

Compare old code on the base branch against new code on the feature branch and produce a detailed, evidence-backed scorecard across 11 quality dimensions.

## When This Skill Applies
- The user wants to review the quality of changes on a feature branch
- Comparing old vs new implementation during a migration or rewrite
- The user asks for a scored code quality assessment
- Evaluating whether a branch is ready to merge
- Tracking quality progression across multiple review iterations

## Core Principle
Measure code quality objectively by scoring 11 dimensions with real evidence — file names, line counts, pattern counts, and concrete examples. Honest, specific scores with improvement paths are more valuable than inflated ratings. Architecture, type safety, and testing are weighted more heavily because they compound over time.

## Workflow

### Step 1: Identify the Branch
Determine the feature branch and the base branch (`main` or `master`). If a branch name is provided, use it. Otherwise use the current branch.

### Step 2: Gather Evidence
Run the following analyses against the diff between base and feature branch:

- `git diff <base>...HEAD --stat` to list all changed files.
- `git diff <base>...HEAD` to read the actual changes.
- Count lines per file. Flag files over 200 lines.
- Check type safety: strict mode, `@ts-ignore`, `@ts-expect-error`, `as any`, `as unknown`, `Record<string, any>`.
- Check styling approach: CSS modules vs inline styles (`style={{`, `style={`).
- Check test coverage: test files, `describe`/`it`/`test` blocks, integration vs unit tests.
- Check accessibility: `aria-` attributes, `role=`, semantic HTML, color-only indicators.
- Check error handling: try/catch, error boundaries, API error handling.
- Check code duplication: repeated constants, copy-pasted components, duplicated helpers.
- Check API integration: cancellation, retries, typed responses, caching.
- Check routing: lazy loading, nested layouts, 404 handling, breadcrumbs.
- Check state management: prop drilling depth, global state, custom hooks.
- Check DX tooling: linting, formatting, pre-commit hooks, import sorting.

### Step 3: Check for Previous Reviews
Look for `BRANCH_REVIEW.md` in the repo root or `.claude/` directory. If found, include a "Previous New" column to show progression.

### Step 4: Score Each Dimension
Rate each dimension 1-10 for both old and new code. Back every score with evidence. Do not guess — use `git diff`, `grep`, `find`, and file reads.

### Step 5: Produce the Report
Generate the scorecard, improvements, regressions, and bottom line summary following the output format.

## Detection / Indicators
- A feature branch with significant code changes ready for review
- Migration branches comparing frameworks or architectural approaches
- Quality regression concerns during code review
- Team wanting objective metrics on branch readiness
- Iterative reviews tracking quality improvement over time

## Scoring Rubrics

| Dimension | 1-3 | 4-6 | 7-8 | 9-10 |
|-----------|-----|-----|-----|------|
| **Type Safety** | No types or `any` everywhere | Partial types with casts | Strict mode, few casts | Complete types, generic utilities, no unsafe casts |
| **State Management** | Global mutations, no patterns | Basic hooks/state | Proper state libs, minimal prop drilling | Optimized selectors, derived state, signal-based |
| **Component Architecture** | Monolithic files 1000+ lines | Some splitting, large files remain | Most files under 200 lines, clear separation | Composable, single-responsibility, under 150 lines |
| **Routing** | No client routing or full reloads | Basic routing | Lazy loading, nested layouts | Code splitting, prefetching, 404/error routes |
| **Styling** | All inline or massive CSS files | Mixed inline and modules | CSS modules or tokens, few inline | Design token system, zero inline, themeable |
| **Testing** | No tests | Shallow smoke tests only | Behavior tests, good coverage, some integration | Integration + e2e, sub-component tests, a11y tests |
| **Error Handling** | Unhandled errors crash app | Basic try/catch | Error boundaries, typed errors, user feedback | Retry logic, graceful degradation, monitoring |
| **API Integration** | Raw fetch, no error handling | Wrapper with basic error handling | Typed responses, caching, cancellation | Retry, pagination, optimistic updates |
| **Accessibility** | No ARIA, no semantic HTML | Some ARIA labels | Keyboard nav, screen reader support | WCAG AA compliant, a11y tests |
| **Code Duplication** | Extensive copy-paste | Some shared utils | Centralized constants, shared components | DRY, shared hooks, component library |
| **DX & Maintainability** | No tooling | Basic linting | Lint + format + pre-commit + aliases | CI checks, auto-fix, consistent conventions |

## Transformation Pattern

**Scorecard format:**

| Dimension | Old | New | Verdict |
|---|---|---|---|
| **Type Safety** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |

**With previous review:**

| Dimension | Old | Previous New | Current New | Verdict |
|---|---|---|---|---|

## Common Pitfalls
- Inflating scores to avoid difficult conversations
- Scoring without evidence (every number needs file names, counts, or patterns)
- Averaging all dimensions equally (weight architecture, type safety, and testing higher)
- Ignoring the base branch comparison (a 6/10 that was a 3/10 is great progress)
- Reviewing only the diff without understanding the broader context of changed files

## Additional Resources
### Reference Files
- **`references/scoring-evidence-guide.md`** — How to gather and present evidence for each scoring dimension
