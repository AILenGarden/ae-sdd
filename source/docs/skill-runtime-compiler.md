# SKILL Runtime Compiler

This document defines the native build boundary for ae-sdd methodology assets.
`source/` is the human-maintained source of truth. Runtime packages and `dist/`
are generated artifacts and are never edited manually.

## Source Slim Entries

The source-SKILL slimming transform keeps a short routing entry at
`source/SKILL.md` or `source/skills/**/*.md` and preserves the complete semantic
source under `source/skill-fallbacks/**`.

The fallback is the sole complete semantic source for a slim entry. A renderer
must never derive a new entry from an already slimmed entry, because repeated
summarization loses workflow, gate, and artifact obligations.

Each generated entry carries:

- `source_slim_schema: ae-sdd-source-slim/v2`;
- the canonical fallback SHA-256 of UTF-8 content normalized to LF without a BOM;
- canonical source byte/line counts;
- a deterministic semantic-inventory SHA-256;
- load, heading, and inline-reference indexes.

The structure is defined by
[`source-skill-slim-entry-template.md`](../templates/skill/source-skill-slim-entry-template.md)
and the normative rules are in
[`skill-source-slimming-standard.md`](../standards/skill-source-slimming-standard.md).

## Native Renderer

`ae-sdd-build source-slim` is the maintained renderer. It operates on explicit
entries only, validates path containment below the supplied source root, reads
the `source_fallback` metadata, and uses that fallback as its only semantic
input.

Refresh an entry after changing its fallback:

```text
ae-sdd-build source-slim --source source \
  --skill skills/phase1-design/requirement-analysis-skill.md --refresh
```

Validate the canonical rendered bytes without writing:

```text
ae-sdd-build source-slim --source source \
  --skill skills/phase1-design/requirement-analysis-skill.md --validate
```

`--upgrade` remains a compatibility alias for `--refresh` so previously
generated entries remain actionable. New documentation and new invocations must
use `--refresh`.

The command rejects absolute paths, `..` traversal, source entries outside
`SKILL.md` or `skills/**/*.md`, fallback paths outside `skill-fallbacks/**`,
self-referential or already-slimmed fallbacks, missing fallbacks, and canonical
byte drift during validation. A refresh writes only entries whose canonical
content differs from the deterministic result.

## Build Boundary

Native build jobs own package construction and distribution. They consume
version-controlled source assets and produce generated artifacts; neither an
Agent nor a Hook may use source files as a runtime fallback or manually repair
`dist/`.

Use the registered `ae-sdd-build native-job` and `ae-sdd-build post-commit`
entrypoints for their typed build/distribution requests. The exact release gate,
including compatibility and artifact verification, is maintained in
[`RELEASING.md`](../../RELEASING.md).

The required authority order is:

1. User instruction subject to daemon and security constraints.
2. Daemon state, Gate outcome, and native contracts.
3. Generated runtime package and compact routing indexes.
4. Source fallback for the exact methodology wording.
5. Historical explanatory material.

## Determinism And Validation

For the same canonical fallback content and renderer version, a refresh must
produce the same bytes. The renderer normalizes UTF-8 input to LF without a BOM
before hashing, rendering, and validation, so Git line-ending conversion does
not change the result. Wall-clock data, host paths outside the declared source
root, random values, and prior slim-entry prose are not renderer inputs.

Before releasing a methodology change:

```text
cargo test -p ae-sdd-build --test source_slim
ae-sdd-build source-slim --source source --skill <changed-entry> --validate
cargo fmt --all -- --check
cargo clippy -p ae-sdd-build --all-targets -- -D warnings
```

Run the focused source-slim refresh/validate pair for every changed fallback,
then run the package/release verification required by the Work Item. Generated
`dist/` content remains outside ordinary source edits.

## Maintenance Rules

- Change a methodology rule in its full fallback first, then refresh only the
  corresponding slim entry.
- Do not copy a process state machine, Gate table, or released runtime behavior
  into compiler prose; the Rust control plane remains authoritative.
- Do not silently broaden a one-entry refresh into a whole-tree rewrite.
- If a renderer input, output schema, or package behavior changes, update the
  relevant design, standard, template, focused tests, and Work Item verification
  plan before claiming completion.
