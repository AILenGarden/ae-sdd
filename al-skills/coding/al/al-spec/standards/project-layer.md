# ALSpec Project Layer Standard

The project layer supplies repository-specific facts, templates, and stricter
conventions on top of the selected global type SKILL. It is selected only by an exact
`project_id` match from `framework/registry.yaml`.

## Allowed Project Content

- repository and module identity;
- authoritative constraint files and commands;
- required document storage paths and naming conventions;
- project-specific status values, roles, environments, and release gates;
- technology, architecture, API, database, security, and testing facts;
- project-specific examples, anti-patterns, and approved deviations.

## Prohibited Project Content

- copying the entire global writing standard;
- changing the meaning of `REQ-*`, `AC-*`, `TC-*`, or `TASK-*` IDs;
- silently weakening a global `MUST` or deleting required traceability;
- registering a project layer without a stable `project_id`;
- inferring a project-level SKILL merely because a project template exists;
- treating a project assumption as fact without a cited source or decision.

## Project Adapter Shape

Use [`templates/project/project-adapter.md`](../templates/project/project-adapter.md)
for a new project layer. Keep the adapter short and factual. Link to the
project's authoritative constraints instead of duplicating them.

When a project layer narrows a global template, document the exact section,
new rule, reason, owner, and effective version. A project template may be
selected by exact `project_id` while the global type SKILL remains the sole
writing standard. A project-level SKILL is an explicit, separate registration;
it is never inferred from the existence of a project template.

## Selection and Precedence

1. Resolve the repository's `project_id` before selecting a template.
2. When an exact project template mapping exists, load that template with the
   global type SKILL; the project template changes output shape only.
3. When no exact project template mapping exists, load the global type SKILL and
   its global type template as fallback.
4. An explicitly registered project SKILL may add facts or stricter checks for
   its type, but it must not silently weaken a global `MUST`.
5. Conflicts are blockers until a `DEC-*` decision resolves them.
6. The context note must list selected SKILLs and templates and why they apply.
