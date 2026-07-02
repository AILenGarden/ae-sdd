# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Nature

This repo is a **Claude Code skill**, not a software project. There is no build, no test runner, no lint, no package manifest. All artifacts are Markdown — the value is in the prompt content itself.

- `SKILL.md` is the entry point loaded by Claude Code when the skill activates. Its YAML frontmatter (`name`, `description`) controls when the skill triggers — edits there change activation behaviour, not just docs.
- `references/phase{1..4}-*.md` hold the detailed methods/formats for each phase. SKILL.md deliberately defers to them; phase work must `Read` the relevant reference rather than rely on memory.
- `assets/templates/*.md` and `assets/commands/*.md` are copied verbatim into a user's `.claude/` directory at bootstrap. `assets/agents/bc-developer.md` is copied to `.claude/agents/`. Treat everything under `assets/` as user-facing — not internal coach docs (those live in `references/`).

Most files are `chmod 444` (read-only) on disk. If an edit is needed, `chmod u+w` first; the read-only bit signals "stable surface, change deliberately."

## Editing Rules Specific to This Skill

When modifying skill content, the following invariants must be preserved (violating them will break the skill in production):

1. **SKILL.md frontmatter `description`** is the trigger surface. It enumerates the slash commands (`/arch-coach`, `/phase-1..4`, `/arch-learn`) and the topical keywords (Bounded Context, Aggregate, AI-ADR…) that Claude Code matches against user prompts. Adding capabilities means updating this description, not just the body.
2. **Bilingual policy** — instructions to Claude are written in English; user-facing output is Traditional Chinese with technical terms left in English. Chinese text inside `「...」` quotation marks is verbatim output to reproduce, not paraphrasable example copy.
3. **Decision-priority ordering** in SKILL.md (Domain correctness > fallback > verifiability > team executability) is referenced by name from the phase files. Re-ordering requires updating downstream references.
4. **AI veto conditions** (four listed in SKILL.md) are referenced by Phase 2's AI-ADR workflow. Keep wording aligned across files.
5. **File-structure contract** — `docs/system/{domain-stories,context-map,touchpoints}.md` and `docs/{bc}/{discovery,decisions,spec}.md` are the artifact paths the skill produces in user projects. Phase files assume these exact paths.
6. **Phase-selection state machine** in SKILL.md ("State determination" section) operates on three invocation paths: (a) `/phase-N <BC>` with explicit BC arg, (b) `/arch-coach <BC>`, (c) `/arch-coach` with no arg (renders an in-flight summary derived from `{coach_output_root}/` and asks user to pick). `arch-state.md` is **gitignored at bootstrap** — it is per-developer state holding only `last_touched: { bc, phase, at }`, NOT team coordination. Per-BC commands (`/phase-2 <BC>`, `/phase-3 <BC>`) require BC explicitly and do NOT fall back to `last_touched.bc`; only no-arg `/arch-coach` reads `last_touched` (as a default-suggestion hint). Phase progress is derived from filesystem (artifact existence + `spec.md` frontmatter `status` for Phase 3 stability). Adding a phase or step requires updating the state-determination algorithm in SKILL.md and may require updating `arch-state-template.md` only if the new step changes what `last_touched.phase` can be.

## Content Architecture

The skill operates as a **producer/reviewer** workflow, not a Q&A coach: Claude drafts artifacts (narratives, UL tables, aggregates, AI-ADRs), the user reviews and challenges. This shapes every reference file — phase files instruct Claude to *produce*, never to ask the user to fill blanks.

Two-axis flow:

- **System-level** runs once per project: Phase 1 Steps 1–5 produce `domain-stories.md` + the classification section of `context-map.md` + `touchpoints.md`.
- **Per-BC** runs independently per Bounded Context: Phase 1 Steps 6–7 → Phase 2 → Phase 3, each producing one file under `docs/{bc}/`. BCs can interleave; the spec is explicit that this enables incremental delivery.

Phase 4 is orthogonal — it reviews any artifact at any time using five health checklists.

`arch-state.md` (in user projects, generated from the template here, **gitignored at bootstrap**) holds **only** `last_touched.{bc, phase, at}` — a personal cursor for "where did I leave off." It is read only by `/arch-coach` with no arguments as a default-suggestion hint; per-BC phase commands take BC explicitly and do not consult it for BC selection. This is what enables a team to develop multiple BCs in parallel without contention on the cursor file. Phase completion / output paths / per-BC summaries are derived from `{coach_output_root}/` filesystem (and from `spec.md` frontmatter for Phase 3 stability) — not stored. `arch-learnings.md` (which IS team-shared, append-only) holds the `learnings:` section (fed by three sources: session preferences, Phase 4 ⚠️/❌ findings, `/arch-learn` commands), `open_questions:`, and the `phase_4_reviews:` audit log. Before entering a phase, learnings must be read and applied silently — not quoted back.

## When Working in This Repo

- Do not introduce build tooling, package files, or tests "for completeness" — the skill is content, and tooling adds maintenance cost without benefit.
- Do not auto-translate the verbatim Chinese strings (the `「...」` blocks). They are UX copy.
- When changing one phase file, scan the others for cross-references (terminology, file paths, decision rules) — the four phase files share a tightly coupled vocabulary.
- The README.md describes the skill to external readers; SKILL.md instructs Claude. Keep them consistent on capabilities and non-goals (especially the "What It Is Not" list — the explicit non-goals are part of the product).
