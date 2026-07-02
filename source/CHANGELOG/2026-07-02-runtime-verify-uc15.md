# 2026-07-02 runtime verify and UC-15

## Summary

- Added `tools/lib/runtime_verify.py`, a read-only verifier for compiled ae-sdd runtime packages.
- Added `ae-sdd runtime verify --path <package> [--json]` to validate installed or built runtime packages.
- Added `UC-15` to `update-check`: runtime compile consistency now compiles the same source twice with different build dates, verifies the package, and compares `SKILL.md` plus `runtime/**` byte snapshots.
- Added compiled-only guards to `scripts/distribute.py` and `scripts/distributors/_base.py` so copytree distribution refuses uncompiled `source/` or incomplete runtime packages before install.
- Added tests for runtime verification, UC-15 filtering/counts, Hermes detection, and `document_storage.get_thinking_engine()`.

## Design Impact

- `source/` remains the human-maintained uncompiled source.
- `dist/ae-sdd/` and installed agent packages must be compiled runtime packages with `compiled: true`, `runtime/manifest.json`, stable `runtime_fingerprint`, load-order files, and fallback source preservation.
- `ae-sdd-update-skill.md` and `source/standards/update-graph.json` now include UG-19 / UC-15 anchors so future runtime compiler changes automatically cascade into the correct consistency checks.

## Validation

- `python tools/tests/run.py runtime_verify -v`
- `python tools/tests/run.py update_graph -v`
- `python tools/bin/ae-sdd runtime verify --path dist/ae-sdd`
- `python tools/bin/ae-sdd update-check --only UC-15`
