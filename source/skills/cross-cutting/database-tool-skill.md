---
name: database-tool
description: Local-profile database toolset for ae-sdd. AI writes SQL, ae-sdd db reads local connection profiles, enforces read-first policy, executes supported adapters, and returns auditable evidence.
---

# Database Tool Skill

## 1. Purpose

The Database Tool lets ae-sdd validate real schema and SQL evidence without
letting the Agent invent connection details or silently mutate data.

AI may draft SQL. The tool owns:

- locating local connection profiles
- redacting secrets
- enforcing read-first policy
- executing supported adapters
- returning structured evidence

## 2. Profile Location

Profiles live in:

```text
<project>/.ae-sdd/secrets/db-connections.local.json
```

The schema is defined in
[`db-connection-profile.schema.md`](../../standards/toolsets/db-connection-profile.schema.md).

## 3. Commands

```bash
ae-sdd db profiles --init
ae-sdd db profiles
ae-sdd db query --profile <name> --sql-file <file>
ae-sdd db query --profile <name> --sql-file <file> --write
ae-sdd db explain --profile <name> --sql-file <file>
ae-sdd db audit
```

The first executable adapter is SQLite. Other drivers may be configured, but
until their adapters exist the tool must return `blocked`.

## 4. Phase Rules

| Phase | Allowed DB Usage |
|---|---|
| RA | read-only schema/table/field checks |
| Design | read-only validation of existing data model |
| CodingPlan | read-only query and EXPLAIN evidence |
| Coding | read/write only with explicit flag, transaction/rollback evidence required |
| Review | read-only verification and audit evidence |

## 5. Hard Rules

- No raw passwords in repo files or reports.
- No write SQL without `--write`.
- No production data mutation by default.
- If DB is unavailable, mark evidence as missing; do not switch to fake data.
- DB evidence used in reports must cite the command result or saved output.
