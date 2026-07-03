# 2026-07-03 source SKILL slimming

## Summary

- Slimmed the ae-sdd source SKILL set itself, not only generated runtime outputs.
- Replaced `source/SKILL.md` and all `source/skills/**/*.md` with source slim entries that contain:
  - original frontmatter metadata,
  - `source_slimmed: true`,
  - `source_fallback`,
  - `source_fallback_sha256`,
  - compact load contract,
  - heading and inline-reference outline.
- Preserved every full pre-slim source under `source/skill-fallbacks/`.
- Updated both runtime compilers to read `source_fallback` when `source_slimmed: true` is present, so generated runtime fallback files retain full semantics.
- Updated `scripts/compile_all_skills.py` to run source slimming before compilation; already-slimmed sources are skipped.

## Safety Rules

- Do not delete `source/skill-fallbacks/`; it is the source-side semantic fallback.
- Do not run ad hoc source slimming over files that already contain `source_slimmed: true`.
- Runtime compilation must use `source_fallback` as fallback content, not the slim entry.

## Verification

- `python scripts/slim_source_skills.py` returned `slimmed=0 skipped=30` on the second run.
- `python scripts/compile_all_skills.py --include-references`
- `python -m unittest tools.tests.test_skill_runtime_compiler tools.tests.test_standalone_skill_runtime_compiler tools.tests.test_runtime_verify`
- `python tools/bin/ae-sdd runtime verify --path dist/ae-sdd`
- `python tools/bin/ae-sdd update-check --only UC-15 --json`
