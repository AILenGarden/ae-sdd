# 2026-07-02 standalone SKILL runtime compiler

- Added `standalone-skills/skill-runtime-compiler/` as a reusable compiler SKILL package.
- The bundled compiler script compiles any directory containing `SKILL.md` into a sibling `<name>-compiled/` package while preserving the source package.
- The compiled package writes a compact bootloader, `runtime/manifest.json`, `runtime/boot.compact.md`, `runtime/outline.compact.md`, and `runtime/fallback/SKILL.full.md`.
- Runtime output is deterministic: repeated compilation of unchanged source must produce byte-identical `SKILL.md` and `runtime/**`.
- Added tests for source preservation, generated package structure, overwrite guards, CLI JSON output, and idempotence.
