---
name: branch-review
description: Perform a deep code quality review of a feature branch comparing old vs new code, producing a scored scorecard across 11 dimensions with evidence-backed ratings.
---

# Branch Code Quality Review

Compare the old code on the base branch against the new code on the feature branch and produce a detailed, evidence-backed scorecard.

## Review Process

### Step 1: Identify the Branch

Use the provided branch argument if available, otherwise use the current branch. Determine the base branch (`main` or `master`).

### Step 2: Gather Data

Run the following analyses against the diff between the base branch and the feature branch:

- `git diff <base>...HEAD --stat` to list all changed files.
- `git diff <base>...HEAD` to read the actual changes.
- Count lines per component or file. Flag files over 200 lines.
- Check type safety: strict mode in `tsconfig.json`, count `@ts-ignore`, `@ts-expect-error`, `as any`, `as unknown`, `Record<string, any>`.
- Check styling approach: count CSS modules vs inline styles (`style={{`, `style={`).
- Check test coverage: count test files, `describe`/`it`/`test` blocks, distinguish integration vs unit vs shallow tests.
- Check accessibility: count `aria-` attributes, `role=`, semantic HTML elements, color-only status indicators.
- Check error handling: try/catch patterns, error boundaries, API error handling.
- Check code duplication: repeated constants, copy-pasted components, duplicated helpers.
- Check API integration: cancellation, retries, typed responses, caching.
- Check routing: lazy loading, nested layouts, 404 handling, breadcrumbs.
- Check state management: prop drilling depth, global state, custom hooks.
- Check DX tooling: linting, formatting, pre-commit hooks, import sorting.

### Step 3: Check for Previous Reviews

Look for a file called `BRANCH_REVIEW.md` in the repo root or `.claude/` directory. If found, include a "Previous New" column in the scorecard to show progression.

### Step 4: Score Each Dimension

Rate each dimension on a 1-10 scale for both old and new code. Back every score with file names, line counts, or pattern counts.

## Scoring Rubrics

- **Type Safety**: 1-3 = no types or `any` everywhere; 4-6 = partial types with casts; 7-8 = strict mode, few casts; 9-10 = complete types, generic utilities, no unsafe casts.
- **State Management**: 1-3 = global mutations, no patterns; 4-6 = basic hooks/state; 7-8 = proper state libs, minimal prop drilling; 9-10 = optimized selectors, derived state, signal-based.
- **Component Architecture**: 1-3 = monolithic files 1000+ lines; 4-6 = some splitting but large files remain; 7-8 = most files under 200 lines, clear separation; 9-10 = composable, single-responsibility, under 150 lines.
- **Routing**: 1-3 = no client routing or full reloads; 4-6 = basic routing; 7-8 = lazy loading, nested layouts; 9-10 = code splitting, prefetching, 404/error routes, breadcrumbs.
- **Styling**: 1-3 = all inline or massive CSS files; 4-6 = mixed inline and modules; 7-8 = CSS modules or tokens, few inline; 9-10 = design token system, zero inline, themeable.
- **Testing**: 1-3 = no tests; 4-6 = shallow smoke tests only; 7-8 = behavior tests, good coverage, some integration; 9-10 = integration + e2e, sub-component tests, a11y tests.
- **Error Handling**: 1-3 = unhandled errors crash app; 4-6 = basic try/catch; 7-8 = error boundaries, typed errors, user feedback; 9-10 = retry logic, graceful degradation, monitoring.
- **API Integration**: 1-3 = raw fetch, no error handling; 4-6 = wrapper with basic error handling; 7-8 = typed responses, caching, cancellation; 9-10 = retry, pagination, optimistic updates.
- **Accessibility**: 1-3 = no ARIA, no semantic HTML; 4-6 = some ARIA labels; 7-8 = keyboard nav, screen reader support, semantic HTML; 9-10 = WCAG AA compliant, a11y tests.
- **Code Duplication**: 1-3 = extensive copy-paste; 4-6 = some shared utils; 7-8 = centralized constants, shared components; 9-10 = DRY, shared hooks, component library.
- **DX & Maintainability**: 1-3 = no tooling; 4-6 = basic linting; 7-8 = lint + format + pre-commit + aliases; 9-10 = CI checks, auto-fix, consistent conventions, documentation.

## Output Format

Produce a report with the following structure:

```
## [Old Technology] vs [New Technology] — Review

### Scorecard

| Dimension | Old | New | Verdict |
|---|---|---|---|
| **Type Safety** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **State Management** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **Component Architecture** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **Routing** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **Styling** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **Testing** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **Error Handling** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **API Integration** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **Accessibility** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **Code Duplication** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |
| **DX & Maintainability** | X/10 (evidence) | X/10 (evidence) | Verdict — specific details |

### What Got Better
1. Concrete improvement with specific evidence (file names, line counts, patterns).

### What Got Worse (or What Still Needs Work)
1. Regression or remaining issue with specific evidence.

### Bottom Line
2-3 sentence honest summary.
**The old code was X/10. The new code is Y/10.**
```

If a previous review exists, use a four-column format instead:

```
| Dimension | Old | Previous New | Current New | Verdict |
|---|---|---|---|---|
```

And end with: **The old code was X/10. The previous new code was Y/10. The current new code is Z/10.**

## Important Rules

- Be honest and specific. Back every score with file names, line counts, or pattern counts.
- Do not inflate scores. A 5/10 with a clear improvement path is more useful than a generous 7/10.
- Use `git diff`, `grep`, `find`, and file reads to gather real evidence. Never guess.
- If the old code does not exist (greenfield), score the old as N/A and only rate the new code.
- The overall score is not an average. Weigh architecture, type safety, and testing more heavily as they compound.
