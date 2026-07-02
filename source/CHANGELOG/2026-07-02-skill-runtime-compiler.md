# 2026-07-02 skill runtime compiler

## Summary

- Added the SKILL Runtime Compiler design: `source/` is now the uncompiled human-maintained source, while `dist/ae-sdd/` is the compiled runtime package for agents.
- Added `source/docs/skill-runtime-compiler.md` as the full operator/design manual.
- Extended `source/docs/ae-sdd-design.md` with the new Runtime IR module.
- Added the initial compiler tool and build integration so `build_dist.py` emits runtime compact slices and a compiled bootloader.
- Extended the ae-sdd runtime compiler from root-only compilation to full child-SKILL entry compilation: every `source/skills/**/*.md` is emitted as a compiled bootloader under `dist/ae-sdd/skills/**/*.md`, with full source fallback preserved under `runtime/skills/**/fallback/SKILL.full.md`.
- Made runtime compiler output deterministic: no wall-clock timestamp is written to `SKILL.md` or `runtime/**`, `runtime_fingerprint` records stable inputs, and repeated compilation preserves the fallback source.
- Documented the current implementation status and the remaining backlog. Runtime verification, update-check UC-15, child-SKILL compiled-entry checks, and compiled-only distribution checks are now implemented; full outer dist reproducibility and richer semantic sub-SKILL compact remain future extensions.

## Rules

- Do not install `source/` directly into agent skill directories.
- Do not hand-edit `dist/ae-sdd/SKILL.md` or `runtime/*.compact.md`.
- Do not ship raw child SKILL source as `dist/ae-sdd/skills/**/*.md`; those files must be compiled bootloaders and the raw source must live under `runtime/skills/**/fallback/SKILL.full.md`.
- Regenerate runtime package through `scripts/build_dist.py`.
- Runtime compiler output must be byte-idempotent for identical inputs, including `SKILL.md`, `runtime/**`, and `skills/**/*.md`; build time belongs only to outer distribution metadata, not runtime compact files.
