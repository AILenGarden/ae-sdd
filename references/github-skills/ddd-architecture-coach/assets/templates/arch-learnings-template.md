# Architecture Learnings

> This file is **append-only**. The DDD Architecture Coach writes here from three sources:
>
> - `session`: preferences corrected 3+ times in current session, user agreed to record as **project-scoped** (class A; cross-project preferences go to Claude Code memory instead)
> - `phase_4`: auto-written from Phase 4 Review ⚠️/❌ findings (class C)
> - `user_triggered`: user executes `/arch-learn <content>` (class D)
>
> Conflict priority (per SKILL.md → Memory / State / Learnings):
> personal Claude Code memory < `arch-state.md` current focus < this file's learnings
>
> Before entering any phase, the coach reads this file alongside `arch-state.md`. Relevant entries are folded into guidance — applied, not quoted back.
>
> The `phase_4_reviews:` block holds per-review audit records (formerly in `arch-state.md`).
> The `slices_shipped:` block is an append-only summary of shipped implementation slices — 3-5 line distillations, not full ship logs (git history is authoritative for commits / metrics / file lists).

---

## Learnings

```yaml
learnings:
  # - source: session | phase_4 | user_triggered
  #   date:                # ISO date, e.g., 2026-04-30
  #   content:             # one-paragraph statement of the rule + reason
  #   applies_to:          # all | phase_1 | phase_2 | phase_3 | phase_4 | specific_bc:<n>
  #   priority:            # P0 | P1 | P2  (only required for source: phase_4)

# Examples:
# - source: phase_4
#   date: 2026-04-15
#   content: Session BC missed the CategoryRule aggregate in the fallback path. Future Phase 1 must run at least 1 happy + 1 exception scenario per main path.
#   applies_to: phase_1
#   priority: P1
#
# - source: session
#   date: 2026-04-18
#   content: User prefers short tables; avoid lists over 10 rows. (Project-scoped — cross-project preference would live in Claude Code memory.)
#   applies_to: all
#
# - source: user_triggered
#   date: 2026-04-20
#   content: BudgetEngine in this project is always synchronous, not async event (performance + consistency requirement).
#   applies_to: specific_bc:Budget
```

---

## Open Questions (cross-phase)

Lingering questions that don't belong to a single phase but influence architecture decisions.

```yaml
open_questions:
  # - raised_in_phase:    # phase_1 | phase_2 | phase_3:<bc> | phase_4
  #   date:
  #   question:
  #   status:             # open | needs_clarification | resolved | obsolete
  #   resolution:         # if resolved, write the conclusion
  #   resolved_at:        # ISO date, only when status: resolved
```

---

## Resolved Questions Archive (optional)

Questions whose resolution is recorded above can be moved here once their referencing phase artifacts have shipped.

```yaml
resolved_archive:
  # - question:
  #   resolution:
  #   raised_in_phase:
  #   resolved_at:
```

---

## Phase 4 Review History

Append-only audit log of Phase 4 reviews. Written by the coach after each review run. Companion to the `learnings:` block above — `phase_4_reviews` records *what was reviewed and how it scored*, while `learnings:` records *what to remember for future phases* (with `source: phase_4`).

```yaml
phase_4_reviews:
  # - date:                           # ISO date, e.g., 2026-05-04
  #   review_scope:                   # phase_1 | phase_2 | phase_3:<BC_name>
  #   scores_summary:
  #     ddd_health:                   # e.g., "2 ✅ / 1 ⚠️ / 0 ❌"
  #     ai_health:
  #     eng_health:
  #     cloud_health:
  #     sbe_health:
  #   critical_fixes:
  #     # - issue:
  #     #   severity: P0 | P1 | P2
  #     #   resolution_status: pending | done
  #   rollback_recommended: false     # true | false
  #   target_rollback_phase:          # only when rollback_recommended: true
```

---

## Slices Shipped

Append-only summary of shipped implementation slices. Each entry is a 3-5 line distillation — what the slice added and why. Full ship details (commits, file lists, metrics, wave-by-wave breakdown) live in git history and are not duplicated here. Future BCs can scan this list to find similar prior patterns to reuse.

```yaml
slices_shipped:
  # - bc:                # specific_bc:<name> | system | cross_bc
  #   name:              # short slice name, e.g. multi-turn-slot-filling
  #   branch:            # feat/...
  #   shipped_date:      # ISO date
  #   summary:           # 3-5 lines plain English: what this slice added + why
  #   key_artifacts:     # 1-3 main files / Aggregates / VOs (optional)
  #   follow_ups:        # bullet list of related defer/spinoff slices (optional)
```
