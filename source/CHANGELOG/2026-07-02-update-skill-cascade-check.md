# 2026-07-02 update-skill cascade check

## Summary

- Added UC-14, a deterministic update-skill cascade sync check.
- UC-14 verifies `source/standards/update-graph.json`, `tools/lib/update_graph.py:CHECK_FUNCS`, and `source/skills/orchestration/ae-sdd-update-skill.md` stay synchronized.
- Added a machine-readable UG/UC anchor section to `ae-sdd-update-skill.md`; it is a compact index, not a replacement for the JSON authority source.
- Updated `update-graph.json` so changes to update-skill, update-graph, source/tools, distribution closure, and AA alignment now suggest UC-14 through `ae-sdd update-check --affected`.
- Brought UC-13 into the graph checks for UG-15, so every registered UC check is reachable from the cascade graph.

## Why

`ae-sdd-update-skill.md` was listed as an affected item, but the checker only performed a broad health checklist through UC-05. That meant the human-readable update graph view could drift from the authoritative JSON graph or from the actual registered checker set without producing a blocking failure.

UC-14 closes that gap by comparing sets, not prose:

- all UG rule IDs in `update-graph.json` must appear in the update-skill anchor list;
- all UC IDs referenced by `update-graph.json` must be registered in `CHECK_FUNCS`;
- all registered UC checks must be referenced by `update-graph.json`;
- all graph UC IDs must appear in the update-skill anchor list.

This keeps the result idempotent and deterministic: the check is pure read-only set comparison and does not depend on LLM interpretation.

## Verification

- `python tools\bin\ae-sdd update-check --only UC-14` -> pass, 17 UG / 14 UC / 14 registered checks synchronized
- `python -m unittest tools.tests.test_update_graph -v` -> 41 tests pass
- `python tools\bin\ae-sdd update-check` -> 14 checks pass, 0 failed, 3 warn
- `python tools\bin\ae-sdd update-check --affected source/skills/orchestration/ae-sdd-update-skill.md --json` -> `checks_to_run` includes `UC-14`
