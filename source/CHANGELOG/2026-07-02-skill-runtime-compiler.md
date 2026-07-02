# 2026-07-02 skill runtime compiler

## Summary

- Added the SKILL Runtime Compiler design: `source/` is now the uncompiled human-maintained source, while `dist/ae-sdd/` is the compiled runtime package for agents.
- Added `source/docs/skill-runtime-compiler.md` as the full operator/design manual.
- Extended `source/docs/ae-sdd-design.md` with the new Runtime IR module.
- Added the initial compiler tool and build integration so `build_dist.py` emits runtime compact slices and a compiled bootloader.
- Made runtime compiler output deterministic: no wall-clock timestamp is written to `SKILL.md` or `runtime/**`, `runtime_fingerprint` records stable inputs, and repeated compilation preserves the fallback source.
- Documented the current implementation status and the next implementation backlog: runtime verification, update-check integration, compiled-only distribution checks, and full dist reproducibility.

## Rules

- Do not install `source/` directly into agent skill directories.
- Do not hand-edit `dist/ae-sdd/SKILL.md` or `runtime/*.compact.md`.
- Regenerate runtime package through `scripts/build_dist.py`.
- Runtime compiler output must be byte-idempotent for identical inputs; build time belongs only to outer distribution metadata, not runtime compact files.
