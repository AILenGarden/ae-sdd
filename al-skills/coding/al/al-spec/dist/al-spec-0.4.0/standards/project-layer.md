# ALSpec Project Layer Standard

The project layer supplies repository-specific facts and stricter conventions
on top of the selected global type SKILL. It is selected only by an exact
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
- treating a project assumption as fact without a cited source or decision.

## Project Adapter Shape

Use [`templates/project/project-adapter.md`](../templates/project/project-adapter.md)
for a new project layer. Keep the adapter short and factual. Link to the
project's authoritative constraints instead of duplicating them.

When a project layer narrows a global template, document the exact section,
new rule, reason, owner, and effective version. The Agent must load the global
type SKILL and template first, then the project type SKILL and its project
template; project rules never replace the global SKILL wholesale.

## Selection and Precedence

1. The selected global type SKILL and its global type template are always
   loaded. The SKILL itself is the normative writing standard.
2. Matching project type SKILLs are loaded only after the repository's
   `project_id` is confirmed; each project SKILL names its required project
   template(s) and embeds its project writing rules.
3. A project SKILL may add facts or stricter checks for the same type, but it
   must not silently weaken a global `MUST`.
4. Conflicts are blockers until a `DEC-*` decision resolves them.
5. The context note must list selected SKILLs and templates and why they apply.
