# Scoring Evidence Guide

## How to Gather Evidence

Each dimension score must be backed by concrete, verifiable evidence. Use these commands and checks for each dimension.

## Type Safety

### Commands
```bash
# Count unsafe casts
grep -r "as any" --include="*.ts" --include="*.tsx" | wc -l
grep -r "as unknown" --include="*.ts" --include="*.tsx" | wc -l
grep -r "@ts-ignore" --include="*.ts" --include="*.tsx" | wc -l
grep -r "@ts-expect-error" --include="*.ts" --include="*.tsx" | wc -l
grep -r "Record<string, any>" --include="*.ts" --include="*.tsx" | wc -l

# Check strict mode
grep -A5 '"compilerOptions"' tsconfig.json | grep "strict"
```

### Evidence Format
"Type Safety: 7/10 — strict mode enabled, 3 `as any` casts in `src/api/client.ts`, zero `@ts-ignore`."

---

## State Management

### What to Look For
- Prop drilling: count how many levels a prop passes through components
- Global state: check for context providers, Redux stores, Zustand stores
- Custom hooks: count hooks that encapsulate state logic

### Evidence Format
"State Management: 6/10 — 2 context providers, `userId` prop drills through 4 levels in `Dashboard → Sidebar → UserMenu → Avatar`, no memoization on expensive selectors."

---

## Component Architecture

### Commands
```bash
# Find large files
find src -name "*.tsx" -o -name "*.jsx" | xargs wc -l | sort -rn | head -20

# Count components per file
grep -r "export.*function\|export.*const.*=.*=>" --include="*.tsx" | wc -l
```

### Evidence Format
"Component Architecture: 5/10 — `OrderForm.tsx` is 480 lines with 3 exported components, 8 of 22 component files exceed 200 lines."

---

## Routing

### What to Look For
- Lazy loading: `React.lazy`, `loadable`, dynamic imports
- Nested layouts: route nesting with shared layouts
- Error routes: 404 pages, error boundaries on routes
- Breadcrumbs: navigation context

### Evidence Format
"Routing: 8/10 — all routes use `React.lazy` with `Suspense`, nested layouts for `/dashboard/*`, 404 catch-all present, no breadcrumb implementation."

---

## Styling

### Commands
```bash
# Count inline styles
grep -r "style={{" --include="*.tsx" --include="*.jsx" | wc -l
grep -r "style={" --include="*.tsx" --include="*.jsx" | wc -l

# Count CSS modules
find src -name "*.module.css" -o -name "*.module.scss" | wc -l
```

### Evidence Format
"Styling: 4/10 — 47 inline `style={{` occurrences, 3 CSS module files, no design tokens, `colors.ts` constants used inconsistently."

---

## Testing

### Commands
```bash
# Count test files
find src -name "*.test.*" -o -name "*.spec.*" | wc -l

# Count test cases
grep -r "it(\|test(" --include="*.test.*" --include="*.spec.*" | wc -l

# Check for integration tests
find . -path "*/integration/*" -o -path "*/e2e/*" | wc -l
```

### Evidence Format
"Testing: 3/10 — 4 test files out of 22 components, 12 total test cases, all shallow renders, no integration or e2e tests, no accessibility assertions."

---

## Error Handling

### What to Look For
- Error boundaries: `ErrorBoundary` components wrapping route segments
- API error handling: try/catch around fetches, user-facing error states
- Loading/error states: components that handle loading, error, and empty states

### Evidence Format
"Error Handling: 6/10 — top-level `ErrorBoundary` exists, API calls in `useQuery` handle errors, but 3 raw `fetch` calls in `src/api/legacy.ts` have no error handling."

---

## API Integration

### What to Look For
- Request cancellation: `AbortController`, cleanup in `useEffect`
- Retries: retry logic or library support (React Query, SWR)
- Typed responses: TypeScript types on API responses
- Caching: query caching, stale-while-revalidate

### Evidence Format
"API Integration: 7/10 — React Query with typed responses on all endpoints, automatic caching and cancellation, no retry configuration, no optimistic updates."

---

## Accessibility

### Commands
```bash
# Count ARIA attributes
grep -r "aria-" --include="*.tsx" --include="*.jsx" | wc -l

# Count role attributes
grep -r "role=" --include="*.tsx" --include="*.jsx" | wc -l

# Check for semantic HTML
grep -r "<main\|<nav\|<aside\|<header\|<footer\|<section\|<article" --include="*.tsx" --include="*.jsx" | wc -l
```

### Evidence Format
"Accessibility: 5/10 — 12 `aria-label` attributes, semantic `<main>` and `<nav>` used, status badges use color only with no text alternative, no keyboard navigation testing."

---

## Code Duplication

### What to Look For
- Repeated constants across files
- Copy-pasted component structures
- Duplicated utility functions
- Shared patterns not extracted into hooks or helpers

### Evidence Format
"Code Duplication: 6/10 — API base URL hardcoded in 4 files, `formatCurrency` duplicated in `OrderList.tsx` and `Invoice.tsx`, shared `Button` component exists but 3 one-off button implementations found."

---

## DX & Maintainability

### What to Look For
- ESLint/Prettier configuration
- Pre-commit hooks (husky, lint-staged)
- Import sorting rules
- Path aliases
- CI pipeline checks

### Evidence Format
"DX & Maintainability: 8/10 — ESLint + Prettier configured, husky pre-commit runs lint-staged, path aliases via `@/` prefix, no CI pipeline for automated checks."

---

## Overall Score Weighting

The overall score is not an average. Weight these dimensions higher because they compound:

| Dimension | Weight |
|-----------|--------|
| Type Safety | High |
| Component Architecture | High |
| Testing | High |
| Error Handling | Medium |
| State Management | Medium |
| API Integration | Medium |
| Accessibility | Medium |
| Code Duplication | Medium |
| Routing | Low |
| Styling | Low |
| DX & Maintainability | Low |

A project with 9/10 type safety, 9/10 testing, and 4/10 styling is in better shape than one with 9/10 styling, 4/10 type safety, and 4/10 testing.
