# ae-sdd Toolset Security Standard

## 1. Boundary

ae-sdd toolsets are project-aware adapters used by Skills and Gates. They are
allowed to collect evidence, run read-only checks, and write ae-sdd-local
process state. They must not silently mutate business code, Git history, remote
systems, or production data.

## 2. Secrets

- Secrets never live in `source/`, `dist/`, `assets/`, `ae-sdd-doc/`, or reports.
- Local secrets use `.ae-sdd/secrets/*.local.*`.
- `.ae-sdd/secrets/` must be ignored by the target project.
- Reports may include profile names and redacted endpoints, never passwords,
  tokens, private keys, or full DSNs.

## 3. Read-First Policy

Default tool mode is read-only.

Write operations require all of the following:

1. An explicit write flag or command.
2. A phase that permits the operation.
3. A rollback or recovery statement when data can change.
4. Evidence output written to ae-sdd memory or report artifacts.

## 4. Audit Output

Every toolset command intended for Skill/Gate consumption must support JSON
output and include:

- command intent
- project root or profile name
- phase/story/task when available
- pass/blocked result
- evidence path or structured result
- redacted configuration summary

## 5. Failure Rule

When evidence cannot be collected, the tool must return a blocked result instead
of inventing evidence. Skills must mark the downstream conclusion as unverified.
