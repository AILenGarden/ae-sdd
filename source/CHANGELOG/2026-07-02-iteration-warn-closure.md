# 2026-07-02 iteration-check warn closure

## Summary

- Closed the two remaining `ae-sdd iteration-check` warn findings.
- Added HS-10 physical path ownership validation in `gate_intercept.py` for ae-sdd flow artifacts.
- Updated IC-2 to recognize the v3.6 Stop hook decision that retired `◆ GATE` self-report verification.
- Updated HARNESS wording so HS-12 no longer claims a Stop-hook physical cross-check that was intentionally removed.

## Details

### HS-10

`gate_intercept.py` now checks matched flow artifacts before entry-token/phase checks:

- resolve project `docWorkspacePath`/`gitPath` through `paths.resolve_doc_workspace`;
- normalize the target path;
- require the artifact to live under `{docWorkspace}/ae-sdd-doc/`;
- block detached paths such as `d:\tmp\*-Story.md` with a direct `ae-sdd doc save` / `document_storage.resolve_path` remediation hint.

This makes the HARNESS statement "PreToolUse hook physical intercept + G-DOC-STORAGE" materially true for detached flow-artifact paths.

### HS-12

`stop_check.py` v3.6 intentionally retired `◆ GATE` self-report verification because the hook should not trust self-reported status headers. `iteration_check.py` now treats that explicit retirement as an info finding instead of a stale coverage warning. HARNESS now states the downgrade honestly and points to UserPromptSubmit, flow_monitor, and `ae-sdd gates check` as the enforcement path.

## Verification

- `python tools\bin\ae-sdd iteration-check --json` -> `n_warn: 0`
- Inline HS-10 temporary-project check -> detached `STORY-001-Story.md` path blocked with `HS-10`
- `python -m py_compile tools\lib\gate_intercept.py tools\lib\iteration_check.py tools\tests\test_gate_intercept.py tools\tests\test_iteration_check.py`
