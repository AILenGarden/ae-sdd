---
name: ae-sdd
description: Unified AL engineering entry with built-in OKF project knowledge and routing to requirements, specification, coding and database capabilities. Use ae-sdd for project understanding, knowledge maintenance, traceability and engineering work.
---

# ae-sdd

`ae-sdd` is a thin capability entry point. It does not run a daemon, maintain workflow state, enforce Gates, or impose a fixed phase sequence.

## Built-in project knowledge

Users invoke ae-sdd; `al-knowledge` is an internal capability, not a separate prerequisite or user entry. For project understanding, business meaning, design rationale, implementation patterns, knowledge construction/query/update/checking, or Spec/interface/call-chain impact analysis, read [the bundled knowledge capability](capabilities/al-knowledge/CAPABILITY.md) and execute the relevant operation. Its references, templates and scripts are beside that internal entry.

This path is self-contained in the ae-sdd plugin. Do not require an external registry, canonical checkout or independently installed al-knowledge to use it. Missing internal resources mean an incomplete ae-sdd package; report that defect instead of asking the user to call a standalone skill. Queries stay read-only; maintenance follows the user's authorized project and scope. Missing project knowledge never forces a full build before ordinary work.

## Available capabilities

For the other capabilities, read the system registry at the `al-skills` root. In the central source tree resolve it as `../../../../registry.yaml`; in an installed runtime use the host's `al-skills/registry.yaml` or its explicitly packaged copy. Resolve entries through their explicit paths; never search `_agents`, `dist`, or runtime directories by directory order. If the registry or canonical source is absent or invalid, report an incomplete package rather than guessing from directory names. Then read `../../capability-catalog.md` for routing context and the selected plugin's own `SKILL.md` for its current input and output contract.

- `al-ra`: rigorous, interactive requirements analysis.
- `al-spec`: authoring and reviewing typed Specs such as DR, Story, and TestCase.
- `al-coding`: implementation design, coding, review, and implementation verification.
- `al-knowledge`: built into ae-sdd; use its bundled knowledge and consumption contracts.
- `db-operator`: strictly read-only database inspection and evidence gathering.

## Choose a capability

Classify the user's requested outcome first, then load the smallest package whose responsibility includes that outcome. The table is a routing aid, not a mandatory sequence:

| User need | Load | Why |
| --- | --- | --- |
| The request is ambiguous, incomplete, or needs scope, rules, risks, or acceptance conditions clarified | `al-ra` | Establishes confirmed requirements and open questions |
| The user wants a DR, Story, TestCase, or a review of one | `al-spec` | Authors or reviews a typed Spec; the type does not create a workflow stage |
| The user wants code changed, an implementation designed, or implementation-level review/verification | `al-coding` | Owns implementation and its focused validation |
| The user wants business meaning, design rationale, implementation patterns, project facts or engineering relationships queried, maintained or checked | `al-knowledge` | Owns the project knowledge base and traceability |
| The user needs database rows, schema facts, or a safe read-only query | `db-operator` | Produces database evidence without write or administration access |
| The user asks to install or update this suite | [installation procedure](../../references/install.md) | Changes Skill files and the selected AGENTS.md guidance only |
| The user asks to remove this suite | [uninstall procedure](../../references/uninstall.md) | Removes only files and AGENTS.md regions recorded as ae-sdd-managed |

Use more than one package only when the result of one is genuinely an input to another. Common combinations are `al-ra` → `al-spec` for a new formal requirement, `al-ra` → `al-coding` for an implementation request, and `db-operator` → whichever capability consumes the database facts. A small, well-specified code fix may start directly with `al-coding`; do not manufacture an RA or Spec solely to follow a route.

Before loading a package, check whether the user has already supplied its required input. If not, load `al-ra` for clarification or state the missing input. After loading, follow that package's own contract for context, artifacts, verification, and boundaries. The entry Skill should report the selected package and reason, but should not duplicate its instructions.

## Installation and removal

- To install or update the suite, read [the installation procedure](../../references/install.md).
- To uninstall the suite, read [the uninstall procedure](../../references/uninstall.md).
- Installation/update/uninstall must synchronize the selected AGENTS.md and registered mirrors through the bundled manage_agents.py and agents-guidance.md template. Only the ae-sdd entry region is owned; preserve general rules. A raw host plugin command alone does not execute this procedure, so do not report guidance synchronization without checking its ownership record and actual content.
- Installation and removal are reference procedures, not separately discoverable Skills or plugins. The ae-sdd plugin exposes only this entry Skill.
- Produce a concrete dry-run before changing an Agent runtime. Reuse existing authorization when it covers the exact operation and targets; ask only for unresolved scope or destructive conflicts.
- These operations manage Skill files and their installation record. Knowledge resources belong to the ae-sdd package. The other four capabilities may already be independently installed; discovering one does not make it owned by ae-sdd. Preserve any historical standalone knowledge installation without using it to replace the bundled capability.

## Selection guidance

- For a new or ambiguous request, start with `al-ra` and continue clarifying with the user until the requirements are sufficiently confirmed.
- After RA, choose only the Spec or Coding capabilities that the task actually needs. DR, Story, and TestCase are Spec types, not mandatory workflow stages.
- Use `al-knowledge` when project facts, Spec relationships, interface mappings, or method call chains must be created, updated, or checked.
- Use `db-operator` when database facts or schema evidence are required. It is read-only.
- Small, well-understood changes may use `al-ra` followed by `al-coding`; no unused capability needs to be invoked.

The Agent selects internal capabilities through ae-sdd based on the task, evidence, risk, and existing artifacts; the user need not choose or invoke al-knowledge separately.

## Shared working rules

- Distinguish facts, inferences, decisions, and open questions.
- Prefer source evidence and relevant `.al-knowledge` concepts over assumptions. Consume project knowledge through [the bundled consumption contract](capabilities/al-knowledge/references/consumption-contract.md). Missing knowledge never requires building a full corpus before working.
- Do not invent project context when evidence is missing; request or record the missing context.
- Keep generated Specs and implementation changes traceable to their inputs.
- Report what was done, what was verified, and what remains uncertain.

There is no ae-sdd daemon, Gate, phase state machine, central workflow registry, or required global route in this architecture.
