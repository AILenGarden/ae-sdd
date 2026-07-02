# 2026-07-02 Mavis harness source-input idempotency

## Change

- Bump `ae-sdd-harness-adapter` to `v0.3.0`.
- Add `.harness/.adapter.lock.source_input_sha256`.
- Treat `source_input_sha256` as the primary idempotency key for Mavis harness generation.
- Keep `source_commit` as diagnostic metadata only.
- Extend UC-07 distribution closure to verify `.harness/.adapter.lock` against current source inputs.

## Reason

Using the repository `HEAD` commit as a generated-file idempotency key is not stable: committing the
generated harness artifact changes `HEAD`, which makes the committed artifact appear stale again.
The real generator inputs are `source/SKILL.md`, `source/HARNESS.md`, the harness templates, and the
adapter version. Hashing those inputs keeps generation deterministic and avoids a permanent
one-commit lag.

## Verification

- `python scripts/build_harness.py --source D:\Item\ae-sdd --no-mount`
- `python scripts/build_harness.py --source D:\Item\ae-sdd --dry-run`
- `python -m pytest tools/tests/test_build_harness.py -q`
- `python tools/bin/ae-sdd update-check --json`
