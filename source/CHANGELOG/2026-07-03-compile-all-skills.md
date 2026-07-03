# 2026-07-03 compile all SKILL runtime artifacts

## Summary

- Extended the standalone SKILL runtime compiler with batch discovery and batch compilation.
- Batch mode skips sources that already declare `compiled: true`, skips generated outputs, and skips up-to-date generated packages by checksum to avoid repeated slimming work.
- Added `scripts/compile_all_skills.py` as the ae-sdd repository-level entrypoint:
  - `source/SKILL.md` and `source/skills/**/*.md` are still compiled through `scripts/build_dist.py` into `dist/ae-sdd`.
  - Directory-style `SKILL.md` packages are compiled through `standalone-skills/skill-runtime-compiler/scripts/compile_skill_package.py`.
  - Batch outputs default to `dist/compiled-skills/`, with a manifest mapping source packages to generated runtime packages.

## Verification

- `python -m py_compile scripts/compile_all_skills.py standalone-skills/skill-runtime-compiler/scripts/compile_skill_package.py`
- `python -m unittest tools.tests.test_standalone_skill_runtime_compiler`
