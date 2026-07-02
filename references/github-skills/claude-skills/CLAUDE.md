# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A portable set of nine **Agent Skills** for Java/Spring microservice development. There is no application code, build, or test runner here — every file is Markdown. Skills are consumed by dropping the skill directories into `.claude/skills/` (Claude Code) or loading them via the `spring-ai-agent-utils` SkillsTool (any LLM). Work in this repo is *authoring and editing skill content*, so the conventions below are the real "architecture."

## Layout

- Flat skill directories, each containing one `SKILL.md` plus an optional `references/` folder of deep-dive docs loaded on demand.
- `README.md` — the catalog (skill → purpose table) and the convention summary. Update it when adding/renaming a skill.
- `CLAUDE-template.md` — a starter `CLAUDE.md` for *consumer* service repos, not for this repo. Don't confuse it with this file.

## Two skill kinds, two templates

The set is split deliberately; match the template of the kind you're editing.

- **Knowledge skills** (`spring-boot-standards`, `jpa-database-patterns`, `kafka-event-patterns`, `resilience-performance`, `dependency-management`, `oop-design`) describe *what good code looks like*. Shape: symptom-triggered `description` → quick-reference table → MUST / MUST NOT rules → ❌/✅ code pairs → verification commands → `references/` for detail. They declare `allowed-tools` in frontmatter.
- **Process skills** (`tdd-java`, `designing-systems`, `reviewing-java-code`) describe *how to work*. Shape: core principle → an **IRON LAW** block → gated phases → a rationalization table → red flags → checklist. They omit `allowed-tools`.

## Authoring rules (these are what make the set work)

- **Every `SKILL.md` stays under 400 lines.** Push detail and long examples into `references/`; the SKILL is the index, not the encyclopedia.
- **`description` routes by symptom, and routes *negatively* to siblings.** Each description lists concrete trigger phrases/errors that should activate it, then explicit "Not for X — use `<sibling-skill>`" lines. This negative routing prevents trigger overlap across the nine skills — preserve it when editing any description, and update siblings' negative routes if you change a skill's scope.
- Knowledge content is taught through paired ❌ (wrong) / ✅ (right) examples and ends with copy-pasteable **verification commands given for both Maven and Gradle**.
- Process skills hinge on a single non-negotiable IRON LAW (e.g. "no production code without a failing test first") with the gates and rationalization table built around enforcing it.
- Skills cross-reference each other by name; when renaming or splitting a skill, grep all `SKILL.md` files for the old name and fix both forward references and negative routes.

## Validating a change

There are no automated checks. Before committing a skill edit, manually confirm:
- `SKILL.md` is < 400 lines (`wc -l */SKILL.md`).
- Frontmatter has `name` + `description`; knowledge skills also have `allowed-tools`.
- The `description`'s negative-routing lines still agree with sibling scopes.
- `README.md`'s table reflects any new/renamed skill.