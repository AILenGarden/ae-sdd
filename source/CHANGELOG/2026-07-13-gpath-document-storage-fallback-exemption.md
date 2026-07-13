# 2026-07-13 | ae-sdd v3.10.2 - G-PATH document-storage fallback exemption

## Summary

G-PATH previously exempted only files named `document-storage-skill.md`. The canonical source full fallback and compiled runtime fallback contain the same path migration examples, but their generated filenames differ, so valid SSOT content was reported as a blocker. G-PATH now recognizes only the canonical entry and its two controlled fallback layouts by strict relative path.

## Changes

| Area | Change |
|---|---|
| `tools/lib/gates.py` | Added `_is_document_storage_skill_artifact()` and replaced the global basename exemption with source/package layout path checks near G-PATH. |
| `tools/tests/test_gates.py` | Added G-PATH regression coverage for canonical source/runtime artifacts and negative parent/basename cases. |
| `README.md` | Added the current G-PATH behavior to the existing v3.10.2 summary. |
| `source/docs/ae-sdd-design.md` | Clarified the G-PATH canonical artifact boundary. |
| `source/docs/ae-sdd-implementation-architecture.md` | Documented the strict relative-path implementation rule. |

## 触发原因

- G-PATH reported the canonical `source/skill-fallbacks/.../document-storage-skill.full.md` migration examples as nine path violations.
- Installed compiled packages can expose the same canonical content as `runtime/skills/.../fallback/SKILL.full.md`, which was not equivalent to the basename-only exemption.

## 影响范围

- G-PATH behavior changes only for the canonical document-storage entry and its source/runtime fallback artifacts; violation patterns, scan limits, skip directories, and other files remain unchanged.
- `tools/bin/ae-sdd`, `source/SKILL.md`, `source/standards/update-graph.json`, and `ae-sdd-update-skill` contracts are N/A because no gate ID, CLI, graph rule, or update protocol changed.
- Version mechanics are N/A: the release identifier remains v3.10.2; README only records the current fix.
- No generated `dist/` or installed runtime file is edited directly.

## 验证方式

- `python -m pytest tools/tests/test_gates.py -k GPath -q`
- `python -m pytest tools/tests/test_gates.py -q`
- `python tools/bin/ae-sdd update-check --json --affected tools/lib/gates.py,tools/tests/test_gates.py`
- `python tools/bin/ae-sdd update-check --json`

## Reviewer

陈聪
