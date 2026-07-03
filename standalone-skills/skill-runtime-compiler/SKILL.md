---
name: skill-runtime-compiler
description: Compile any Codex-style SKILL package into a compact runtime package while preserving the source package. Use when the user asks to compile a SKILL, create a compiled SKILL copy, keep the master/source SKILL unchanged, generate a same-parent compiled package, verify compiler idempotence, or make a SKILL easier for LLMs to load with shorter runtime context.
---

# Skill Runtime Compiler

## Purpose

Compile a source SKILL directory into a sibling runtime package. The source directory is never modified. The default output path is:

```text
<source-parent>/<source-name>-compiled/
```

Use the bundled script for deterministic behavior:

```bash
python scripts/compile_skill_package.py /path/to/source-skill
```

## Compile Workflow

1. Confirm the input path is a directory containing `SKILL.md`.
2. Run `scripts/compile_skill_package.py <source-skill-dir>`.
3. Inspect the generated sibling package:
   - `SKILL.md` is replaced with a compact compiled bootloader.
   - `runtime/manifest.json` records source checksums, load order, generated files, and `runtime_fingerprint`.
   - `runtime/boot.compact.md` contains the minimal runtime contract.
   - `runtime/outline.compact.md` contains deterministic headings and resource indexes.
   - `runtime/fallback/SKILL.full.md` preserves the original source `SKILL.md`.
   - All original source files are copied into the compiled package unless replaced by generated runtime files.
4. Run the same command again when checking idempotence. `SKILL.md` and `runtime/**` must be byte-identical for unchanged input.

## Commands

Default sibling output:

```bash
python scripts/compile_skill_package.py /path/to/my-skill
```

Explicit output:

```bash
python scripts/compile_skill_package.py /path/to/my-skill --output /path/to/my-skill-runtime
```

Machine-readable result:

```bash
python scripts/compile_skill_package.py /path/to/my-skill --json
```

Batch compile every SKILL package under a repository root:

```bash
python scripts/compile_skill_package.py --all-under /path/to/repo --output-root /path/to/repo/dist/compiled-skills
```

Batch mode skips source packages that already declare `compiled: true` unless
`--include-compiled` is supplied. Existing generated outputs are left untouched
when their recorded source checksums are current; use `--no-skip-up-to-date`
to rebuild them.

Replace a non-generated existing output directory:

```bash
python scripts/compile_skill_package.py /path/to/my-skill --output /path/to/out --force
```

## Determinism Rules

- Never write wall-clock time, absolute temporary paths, host names, random values, or file mtimes into runtime outputs.
- Compute `runtime_fingerprint` only from source file bytes, extracted outline data, compiler version, and fixed output contracts.
- Treat an existing generated output package as disposable and rebuild it from the unchanged source.
- Refuse to compile into the source directory or any child of the source directory.
- Refuse to overwrite an unrelated existing output directory unless `--force` is supplied.
- Do not compile a package that already declares `compiled: true` unless the operator explicitly chooses a different source package.
- In batch mode, do not rediscover generated outputs, `dist/`, `node_modules/`, VCS metadata, or already-compiled sources by default.

## Output Contract

The compiled package is still readable Markdown, not encrypted or model-specific machine code. It is compact because the entrypoint routes the agent to short generated runtime slices first, with the original source preserved as fallback.

Compiled packages must contain:

```text
SKILL.md
runtime/manifest.json
runtime/boot.compact.md
runtime/outline.compact.md
runtime/fallback/SKILL.full.md
```

The source package remains the human-maintained master. Distribute the compiled package to agents when runtime context size matters.
