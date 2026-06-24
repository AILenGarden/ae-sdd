# ae-sdd Git Insight Standard

## 1. Purpose

Git Insight converts read-only Git information into structured ae-sdd evidence.
It is used by CodingPlan, CodingReport, CodeReview, and postmortem analysis.

## 2. Allowed Commands

The `ae-sdd git` toolset is read-only:

```bash
ae-sdd git status
ae-sdd git diff --base <ref> --head <ref>
ae-sdd git log --path <file> --limit 20
ae-sdd git blame --file <file>
ae-sdd git impact --file <changed-file>
```

## 3. Prohibited Scope

The Git Insight toolset must not run:

- `git add`
- `git commit`
- `git tag`
- `git branch`
- `git checkout`
- `git reset`
- `git clean`
- any shell composition that mutates files or Git history

Mutating Git operations require explicit user intent outside this toolset.

## 4. Usage by Phase

| Phase | Usage |
|---|---|
| RA/design | inspect existing history when a requirement touches legacy behavior |
| CodingPlan | identify changed modules, historical owners, and risky files |
| Coding | check current diff and avoid unrelated changes |
| CodeReview | produce changed-file evidence and risk hints |

## 5. Evidence Shape

Git evidence must include:

- repo root
- branch
- dirty state or changed files
- base/head refs when available
- risk hints for DB/API/security/test changes
