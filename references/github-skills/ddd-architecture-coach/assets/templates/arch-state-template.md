# Architecture State (Personal Cursor — Gitignored)

> **This file is YOUR personal record of where you last left off** in the DDD Architecture Coach workflow. The skill bootstrap adds it to `.gitignore`.
>
> **Do NOT:**
> - Commit this file (it should be gitignored)
> - Treat it as a source of truth for "what the team is working on" — that comes from git branches, open PRs, and the actual artifacts under `{coach_output_root}/{bc}/`
>
> **The skill reads this file ONLY as a fallback hint** when `/arch-coach` is invoked with no arguments. Per-BC commands (`/phase-2 <BC>`, `/phase-3 <BC>`) take BC explicitly and do not read this file for BC selection.
>
> Phase progress is **derived from `{coach_output_root}/`** (and from `spec.md` frontmatter `status` for Phase 3 stability) — not stored here. Phase 4 review history and learnings live in `.claude/arch-learnings.md` (which IS team-shared).
>
> See `SKILL.md` → State Determination for the full algorithm.

---

```yaml
last_touched:
  bc:                    # BC name; blank when working at system level
  phase:                 # phase_1_step_1_5 | phase_1_step_6_7 | phase_2 | phase_3 | phase_4
  at:                    # ISO date, e.g., 2026-05-06
```
