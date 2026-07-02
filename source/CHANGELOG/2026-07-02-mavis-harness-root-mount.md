# 2026-07-02 Mavis harness root mount fix

## Change

- Move the generated Mavis harness artifact from `harness/.harness/` to root `.harness/`.
- Move the adapter lock to `.harness/.adapter.lock`.
- Make `MavisDistributor.verify()` fail when `mavis harness list` returns success but does not list `ae-sdd`.
- Make Mavis install unmount both current and legacy generated path names before mounting.

## Reason

Current Mavis `HarnessManager` registers local harnesses by scanning `<source>/.harness/agent.md`
and `<source>/<child>/.harness/agent.md`, while generating the actual mounted name from the
source path. The old distributor could report success when `mavis harness list` returned an empty
array, which hid incomplete Mavis instantiation.

## Verification

- `python -m pytest tools/tests/test_build_harness.py -q`
- `python tools/tests/run.py mavis_distributor -v`
- `python scripts/distribute.py --target mavis --no-build`
- `mavis harness list`
