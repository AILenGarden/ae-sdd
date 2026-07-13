# 2026-07-13 | ae-sdd v3.10.2 - G-PATH project input scope

## Summary

G-PATH project scanning now covers declared memory inputs only. Review and generation drafts under `.ae-sdd/drafts/` are process artifacts and are excluded from this path-boundary gate; canonical Story/TestCase documents remain governed by document-storage and their phase gates.

## Changes

| Area | Change |
|---|---|
| `tools/lib/gates.py` | Restrict project scan targets to `.ae-sdd/memory/**/*.md`, top-level `AGENTS.md`/`CLAUDE.md`/`MEMORY.md`, and `.harness/memory/**/*.md`. |
| `tools/tests/test_gates.py` | Cover draft exclusion and all retained project memory input classes, while retaining strict master and wrong-parent regressions. |
| `source/docs/ae-sdd-design.md` | Document the project-side G-PATH input boundary and why drafts are process artifacts. |
| `source/docs/ae-sdd-implementation-architecture.md` | Record the implementation scope and current_story non-filtering rule. |

## Trigger evidence

The existing `D:/Item/life/.ae-sdd/drafts/cs-ai-STORY-003-BE-TestCaseReview-r1.md` is a failed Review r1 process artifact. Its Story/TestCase references on lines 6-7 triggered G-PATH even though the canonical documents exist under `document/life-team-project-docs/.../design/`.

## Impact

- Master source strict-path matching is unchanged.
- Declared memory/config inputs remain blocking when they contain forbidden output paths.
- Drafts are not silently accepted as canonical documents; Review/context gates remain responsible for process artifacts.
- `current_story` is intentionally not used as a project-path bypass.

## Verification

- `python -m pytest tools/tests/test_gates.py::TestGPath -q`
- `python -m pytest tools/tests/test_gates.py::TestCheckAll -q`
- Pure Python `check_g_path(master, project, current_story)` against the project workspace.
- Affected/full `update-check` after implementation.

## N/A

- No gate ID, CLI command, update-graph rule, runtime compiler, or installed package contract changed.
- No Story-003 draft was edited.
- No generated `dist/` or installed runtime file was edited directly.

## Reviewer

陈聆
