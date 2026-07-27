# ae-sdd Agent Engineering Contract

All code, test, schema, migration, build, configuration, and generated-artifact work in this
repository must follow the global ae-sdd workflow before the first write.

## Mandatory project rules

1. Resolve the active Work Item/Story and obtain an approved `state.executionPlan` through ae-sdd.
2. Load the current project constraints with `get_constraints(projectKey)`; a remembered or copied
   version is not authoritative.
3. Treat [`constraints/README.md`](constraints/README.md) as the index and the files it names as the
   engineering SSOT. Do not restate their rules in prompts, skills, or local helper documents.
4. Freeze shared DTO/port contracts and migration numbers before parallel implementation. Every
   Agent owns only its assigned paths; shared-contract changes require the coordinator.
5. Use strict RED-GREEN-REFACTOR, then run formatting, strict Clippy, focused tests, workspace
   regression, real ae-sdd evidence, and independent Review before claiming completion.
6. Never treat Monitor, generated `dist/`, or prompt prose as released Rust control-plane
   authority.

When this file conflicts with an active ae-sdd gate, state, Story, or a file under `constraints/`,
stop and repair the authoritative asset instead of bypassing it.
