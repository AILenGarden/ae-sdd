# Phase 4: Review & Iterate

## Goal

Review any prior phase's output for architectural health. Produce a scored checklist with actionable fixes, not opinions. Output drives concrete corrections — either in the reviewed phase's document or as learnings for future BCs.

Phase 2 and Phase 3 have Self-Check Clean Lists (format compliance before presenting output) and Common Review Findings (pre-emptive pattern matching while drafting). Phase 4 is different: it evaluates the **holistic health** of produced artifacts — do the pieces fit together? Are there systemic risks that per-section checks can't catch?

---

## Operating Model

**You lead the review; the user validates your judgment.**

Read the target artifact(s), run all applicable checklists, produce the scored report. The user's role is to agree, disagree with specific scores, or provide context that changes a score.

Do not ask the user to self-assess. You assess; they challenge.

---

## Language Policy

Per SKILL.md: instructions in English, user-facing output in Traditional Chinese, technical terms in English. Chinese phrases inside 「...」 are verbatim templates.

---

## Review Scope Selection

When the user enters Phase 4, determine scope:

| Trigger | Scope |
|---------|-------|
| User says 「review Phase 1」 or `/phase-4` after Phase 1 | Review Phase 1 output only |
| User says 「review Phase 2」 or `/phase-4` after Phase 2 | Review Phase 2 output; cross-check against Phase 1 |
| User says 「review Phase 3 for BC X」 | Review BC X's Phase 3 spec; cross-check against Phase 1 + 2 |
| User says 「full review」 | Review all existing phase outputs; heaviest scope |
| User provides an external artifact (code, spec, diagram) | Review against the checklist categories that apply; skip inapplicable categories |

Tell the user which scope you selected and why before starting the review.

---

## Pre-flight Checks

Before reviewing, verify:

1. **Target artifact exists and is readable**: the Phase output doc or spec file is accessible
2. **Prior phase outputs available for cross-check**: reviewing Phase 3 requires Phase 1 + 2 outputs; reviewing Phase 2 requires Phase 1
3. **arch-learnings.md is current**: learnings from prior reviews should be loaded to avoid re-flagging resolved issues

Any missing → halt and request. Do not review on incomplete foundation.

---

## Scoring Definitions

Every checklist item gets one score:

| Score | Definition | Action |
|-------|-----------|--------|
| ✅ | Meets the criterion; no change needed | None |
| ⚠️ | Functional but carries known risk | Fix within 3 sprints; document in backlog |
| ❌ | Will cause production incidents or architectural decay | Fix in current iteration before proceeding |

**Escalation rule** (severity-weighted, not count-based — see Rollback Rules section for full table):

- Any **P0 ❌** (e.g., cross-tenant leak, sensitive-data firewall failure, distributed monolith risk) → immediate rollback. One P0 alone is enough; counting them misses the point.
- 3+ **P1 ❌** without a single P0 → recommend rollback; user may decide to fix in place but must explicitly accept the compounding cost.
- All ❌ are **P2** → fix in place, document in `arch-learnings.md` for next BC.

---

## Priority Levels (for ⚠️ and ❌ items)

Every non-✅ item gets a priority:

| Priority | Meaning | Timing |
|----------|---------|--------|
| P0 | Blocks all forward progress | Fix immediately; no new work until resolved |
| P1 | Must resolve before current iteration closes | Fix before next phase or next BC |
| P2 | Tracked debt; resolve in next iteration | Add to backlog with target slice/sprint |

---

## Four Health Checklists

### Checklist 1: DDD Health

Evaluates whether the domain model is well-bounded, correctly named, and structurally sound.

| ID | Check | ✅ Looks like | ⚠️ / ❌ Looks like |
|----|-------|--------------|-------------------|
| D-1 | BC boundaries align with team boundaries (or intended future team splits) | Each BC could be owned by one team without cross-team coordination on internals | Two teams share one BC's internals; or one BC spans responsibilities that would naturally split |
| D-2 | Ubiquitous Language is consistent within each BC | Same term means one thing everywhere inside the BC; code names match spec names | Same term used with different meanings within one BC; or spec says "Transaction" but code says "Record" |
| D-3 | Aggregate boundaries are minimal | Each Aggregate protects only its own invariants; no God Aggregate controlling unrelated state | One Aggregate holds state for multiple business concepts; or Aggregate methods enforce rules about other Aggregates' state |
| D-4 | Domain Events use past tense + business language | `SessionStarted`, `BookingConfirmed`, `CategoryPredicted` | `StartSession`, `ApiCalled`, `DataSynced` |
| D-5 | Cross-BC communication uses explicit integration patterns | Events or well-defined interfaces; no direct Aggregate-to-Aggregate references across BCs | BC A imports BC B's Aggregate class; or BC A queries BC B's database tables directly |
| D-6 | Core / Supporting / Generic classification drives investment decisions | Core domains get custom implementation; Generic domains use off-the-shelf solutions | Custom-building a Generic domain (e.g., hand-rolling auth); or treating a Core domain as Generic (e.g., outsourcing the differentiator) |
| D-7 | Domain Layer has zero external dependencies | Domain project references no infrastructure, no framework, no other BC | Domain Layer imports DbContext, HttpClient, MediatR, or another BC's namespace |
| D-8 | Event subscribers align with Touchpoint Map | Each emitted event in this BC's spec lists subscribers (`subscribers:` frontmatter); every subscriber is also recorded in `{coach_output_root}/system/touchpoints.md` §1 inventory; co-presence scenarios in touchpoints.md §2 are reachable from at least one BC's events | BC emits an event that has visible UI/audit consumers in production but no entry in `subscribers:`; or `touchpoints.md` lists a secondary observer (e.g., agent console) that no BC actually emits events to |

**Phase-specific focus**:
- Reviewing Phase 1 → D-1, D-2, D-4, D-6 are primary
- Reviewing Phase 2 → D-1, D-5, D-6, D-8 are primary
- Reviewing Phase 3 → D-2, D-3, D-4, D-7, D-8 are primary

**Backward-compat note for D-8**: legacy v0.1.0 projects without `touchpoints.md` cannot pass D-8 mechanically. For these, mark D-8 as ⚠️ "deferred — touchpoints.md missing", append to `arch-learnings.md` `open_questions:`, and skip rather than failing the whole DDD checklist. New BCs created after touchpoints.md is in place must satisfy D-8.

---

### Checklist 2: AI Health

Evaluates whether AI interventions are justified, safe, and degradation-proof.

| ID | Check | ✅ Looks like | ⚠️ / ❌ Looks like |
|----|-------|--------------|-------------------|
| A-1 | Every AI intervention point has a documented fallback | AI-ADR or Phase 1 opportunity table has Fallback column filled for every row | Fallback column empty or says "manual" with no detail |
| A-2 | AI output has a verification mechanism | Golden dataset defined; or human-in-the-loop review step; or automated output validation | "We trust the model" with no verification path |
| A-3 | AI failure does not cause system unavailability | System degrades gracefully: reduced functionality, not downtime | AI service down → entire feature returns 500; or user-facing flow blocks on AI response with no timeout |
| A-4 | No unnecessary AI usage | Every AI-ADR passes the Hard veto unconditionally and addresses each Soft veto (with override rationale if used despite a soft condition); rejected candidates documented | AI used where regex/rules achieve equivalent accuracy without justifying override; or AI introduced without veto check |
| A-5 | Fallback has 3+ degradation levels | Progressive: self-correction → human-in-loop → service degradation → full manual | Single fallback: "if AI fails, alert operator" |
| A-6 | Sensitive data excluded from AI context | Explicit documentation of which fields never enter LLM context; enforcement mechanism stated | Cost, personal data, or credentials could flow into prompts without filtering |
| A-7 | Prompt injection analysis for user-input-facing AI | Input sanitization, output validation, or two-pass architecture documented | User text goes directly into LLM prompt without sanitization consideration |

**Skip conditions**: if the project has zero AI intervention points (all rejected in Phase 1/2), skip this checklist entirely and note 「本專案無 AI 介入點，Checklist 2 不適用。」

---

### Checklist 3: Software Engineering Health

Evaluates structural discipline, test coverage, and maintainability.

| ID | Check | ✅ Looks like | ⚠️ / ❌ Looks like |
|----|-------|--------------|-------------------|
| E-1 | Domain Layer zero external dependencies | Domain project has no package references to infrastructure, framework, or other BCs | Domain imports EF Core, MediatR, HttpClient, or cross-BC namespaces |
| E-2 | Test coverage includes all invariants | Every invariant ID (S-1, B-1, etc.) has at least one corresponding test scenario | Invariants listed in spec but no test scenarios reference them |
| E-3 | No Distributed Monolith risk | BCs communicate via events or well-defined interfaces; no synchronous call chains across 3+ BCs | Service A calls B calls C synchronously; failure in C cascades to A |
| E-4 | Technical debt is tracked | Known shortcuts, stubs, and deferred decisions are listed with target slice/sprint | TODOs in code with no tracking; or stubs without replacement plan |
| E-5 | Architectural tests enforce design rules at CI | Reflection tests for sensitive-field exclusion, VO exposure, tenant isolation are specified and mandatory | Design rules exist as documentation only; no automated enforcement |
| E-6 | View DTOs do not expose Domain internals | Views use primitives or projection types; no VOs, no Aggregates, no internal IDs leaked | View property types include Domain VOs; or View exposes TenantId |
| E-7 | Save boundary rules are explicit | One handler = one SaveChangesAsync; dispatch phases not manually composed by handlers | Handlers call SaveChangesAsync multiple times; or manually orchestrate event dispatch |
| E-8 | Open questions have clear resolution status | Every question is RESOLVED (with chapter link), DEFERRED (with target slice), or OBSOLETE | Questions marked "TBD", "future", or left without status |

**Phase-specific focus**:
- Reviewing Phase 2 → E-1, E-3, E-4 are primary
- Reviewing Phase 3 → all items apply
- Reviewing implementation (code review) → E-1, E-5, E-6, E-7 are primary

---

### Checklist 4: Cloud Health

Evaluates deployment, reliability, cost, and observability decisions.

| ID | Check | ✅ Looks like | ⚠️ / ❌ Looks like |
|----|-------|--------------|-------------------|
| C-1 | No single point of failure on critical path | DB has backup strategy; app server can restart without data loss; external dependency has timeout + fallback | Single DB instance with no backup; or critical flow depends on one external API with no timeout |
| C-2 | Cost matches budget sensitivity | High sensitivity → Modular Monolith, single VM, managed DB; Low sensitivity → choices justified by scaling needs | High budget sensitivity but Kubernetes + multi-region; or cost figures unsourced |
| C-3 | Observability covers critical business paths | Per-tenant business metrics (conversion, error rate, AI confidence) alongside infrastructure metrics | Only CPU/RAM/disk monitoring; product failures invisible until user complaints |
| C-4 | Vendor lock-in risk identified and mitigated (if required) | Lock-in items listed; mitigation path documented (Docker portability, DNS independence, data export) | project-context.md says "no vendor lock-in" but deployment uses proprietary services without exit plan |
| C-5 | Multi-tenant isolation has double defense | RLS at DB layer + application-level filtering; integration tests verify both independently | Single isolation layer; or RLS present but no integration tests for cross-tenant scenarios |
| C-6 | Scaling triggers use concrete metrics | Thresholds stated as numbers: "WebSocket connections > 500", "P95 latency > 200ms" | "When traffic increases", "when needed", "at scale" |
| C-7 | MVP alert rules are focused (≤ 7) | 5-7 critical alerts covering: availability, latency, error rate, tenant-level anomaly | 15+ alerts at MVP (alert fatigue); or zero alerts (blind operation) |

**Phase-specific focus**:
- Reviewing Phase 2 → all items apply (Phase 2 §4 is the cloud blueprint)
- Reviewing Phase 3 → C-5 is primary (RLS and isolation specs)

---

### Checklist 5: SBE Health

Evaluates Key Example quality, coverage, and cross-BC consistency.

| ID | Check | ✅ Looks like | ⚠️ / ❌ Looks like |
|----|-------|--------------|-------------------|
| S-1 | Three-color coverage per Aggregate | Every Aggregate has at least one 🟢 happy path + 🟡 edge case + 🔴 rejection | Missing color for any Aggregate |
| S-2 | Every Scenario has exactly one When | Single business event per Scenario | 🐡 Pufferfish (multiple When) or 🐛 Centipede (When-Then chains) |
| S-3 | Then is verifiable and uses 「應」 | 「預約狀態應為已確認」 | 🐨 Koala: vague Then, missing Then, or Then describes system internals |
| S-4 | Given contains only business preconditions | Removing any Given changes the Then | Given mentions DB, API, framework, or technical setup |
| S-5 | All terms from UL; concrete data in Given | Role names, amounts, dates are specific; every noun in UL table | 「使用者」instead of specific role; 「某個金額」instead of concrete number |
| S-6 | Key Examples trace to User Story + scenario | Each Scenario has traceability comment: `US-{BC}-{N}` + `情境` origin | No traceability; orphaned Key Examples with no origin |
| S-7 | Cross-BC Key Example style consistency | Same Gherkin structure, same KE-ID format, same language conventions across all BCs | BC A uses 「應」in Then, BC B uses 「會」; or inconsistent ID formats |
| S-8 | Key Examples → Test naming discipline | Test method names preserve `KE_{Aggregate}_{ID}_{behavior}` format; Arrange-Act-Assert maps to Given-When-Then | Test names are generic (`TestMethod1`); or test body structure doesn't map to Gherkin |

**Phase-specific focus**:
- Reviewing Phase 1 → skip (SBE not yet applied in Phase 1)
- Reviewing Phase 3 → S-1 through S-6 are primary
- Reviewing implementation (code review) → S-7, S-8 are primary
- Reviewing across BCs → S-7 is primary (style consistency)

---

## Cross-Phase Consistency Checks

These apply whenever reviewing Phase 2+ output. They catch drift between phases — the most common source of spec contradictions.

| ID | Check | ✅ Looks like | ⚠️ / ❌ Looks like |
|----|-------|--------------|-------------------|
| X-1 | BC count and names match across phases | Phase 1, 2, 3 all reference the same BCs with the same names | Phase 2 introduces a BC not in Phase 1; or Phase 3 uses a different BC name |
| X-2 | Domain Events flow consistently | Events listed in Phase 1 appear in Phase 2 relationships and Phase 3 event catalogs | Phase 1 event missing from Phase 2 Context Map; or Phase 3 emits events not in Phase 1 |
| X-3 | project-context.md constraints honored | Budget sensitivity, team size, cloud provider, tech stack all reflected in decisions | High budget sensitivity but decisions ignore cost; or tech stack mismatch |
| X-4 | AI candidates fully accounted for | Every Phase 1 AI opportunity has either an active ADR or explicit rejection in Phase 2 | Phase 1 AI candidate silently dropped without rejection entry |
| X-5 | UL terms consistent across phases | Same term, same definition, same BC assignment across all phase outputs | Phase 1 calls it "Transaction", Phase 3 spec calls it "Record" |

---

## Review Output Format

Structure the review report as:

```
## Phase 4 Review — <scope description>

### Review 對象
- 文件：<artifact path(s)>
- 範圍：<which checklists applied>

### 評分總覽

| Checklist | ✅ | ⚠️ | ❌ |
|-----------|---|---|---|
| DDD Health | N | N | N |
| AI Health | N | N | N |
| Engineering Health | N | N | N |
| Cloud Health | N | N | N |
| SBE Health | N | N | N |
| Cross-Phase | N | N | N |

### 詳細結果

#### DDD Health
- D-1 ✅ <一句說明>
- D-2 ⚠️ <問題描述> → 建議：<具體修正> | 優先級：P1
- D-3 ❌ <問題描述> → 建議：<具體修正> | 優先級：P0
...

（每個 checklist 同格式）

### 總結建議
- ❌ 數量：N — <是否建議回退>
- 最高優先修正：<P0 items 列表>
- 建議下一步：<具體行動>
```

Every ⚠️ and ❌ must have all three fields: problem description (one sentence), suggested fix (concrete and executable), and priority (P0/P1/P2). No vague findings.

---

## Rollback Rules (severity-weighted)

Counting ❌ items can mislead — one cross-tenant data leak is more serious than five naming inconsistencies. Use severity, not count.

| Condition | Action |
|-----------|--------|
| Any **P0 ❌** | Immediate rollback to the phase that introduced it; do not continue forward. Examples: tenant-isolation failure, AI Hard veto bypassed, distributed-monolith risk in critical path, security regression. |
| 3+ **P1 ❌** without P0 | Recommend rollback to the phase where most P1s originate; user may fix in place but must explicitly accept the compounding cost. P1 examples: missing fallback levels in AI-ADR, RLS specified single-layer, cost figures unsourced. |
| All ❌ are **P2** | Fix in place; document each in `arch-learnings.md` for next BC to avoid repeating. P2 examples: naming inconsistencies, View DTO field ordering, low-impact UL drift. |
| ❌ in Cross-Phase Consistency (X-1 to X-5) | Always fix the earliest affected phase first, then propagate forward — regardless of severity. Cross-phase drift compounds; later-phase patches without root-fix become technical debt. |
| ⚠️ items only (no ❌) | Fix high-priority ⚠️ within current iteration; track remainder as backlog. No rollback. |

---

## Learnings Writeback

After review is accepted by the user:

1. **Every ⚠️ and ❌ finding** → append to `arch-learnings.md` with `source: phase_4`, `applies_to: <phase or bc>` (not `arch-state.md`; learnings are append-only and live in the learnings file per SKILL.md's three-layer separation)
2. **Patterns that recur across BCs** → generalize and append with `applies_to: all`
3. **User's challenges to your scoring** (where you changed a score based on their context) → append as learning with note on why the context changed the judgment

`arch-state.md` is touched only when a review concludes with a rollback retarget (or the user moves to a different BC after review) — and even then, only the personal cursor `last_touched: { bc, phase, at }` is updated. See Phase 4 Output Checklist below. Per-review audit data (date, scope, scores, critical fixes, rollback flag) goes into `arch-learnings.md` `phase_4_reviews:` (which is the team-shared file).

This prevents the next BC's Phase 3 (or next project's Phase 2) from repeating the same mistakes.

---

## Incremental Review

When reviewing a new BC's Phase 3 spec while prior BCs exist:

1. **Load prior BC review learnings** from `arch-learnings.md` before reviewing
2. **Check for regression**: does the new BC repeat mistakes the prior BC already fixed?
3. **Check for convention drift**: does the new BC follow conventions established by the first BC? (naming, directory structure, test patterns, RLS variable names)
4. **Flag improvements**: if the new BC found a better pattern than the prior BC, note it as a learning for future back-port consideration

---

## Phase 4 Output Checklist

Phase 4 review records are append-only audit data and live in `arch-learnings.md`, not `arch-state.md`. After review is accepted:

- Append a new entry to `arch-learnings.md` `phase_4_reviews:` with:
  - `date` — ISO date of this review run
  - `review_scope` — which phases/BCs were reviewed (e.g., `phase_3:Booking`)
  - `scores_summary` — ✅/⚠️/❌ counts per checklist (`ddd_health`, `ai_health`, `eng_health`, `cloud_health`, `sbe_health`)
  - `critical_fixes` — list of P0 ❌ items, each with `severity` and `resolution_status` (`pending` | `done`)
  - `rollback_recommended` — `true` | `false`
  - `target_rollback_phase` — only when `rollback_recommended: true`
- Append individual learnings (the qualitative lessons distilled from this review) to `arch-learnings.md` `learnings:` with `source: phase_4` (per Learnings Writeback section above).
- If the review concluded a rollback retarget (or the user explicitly moves to a different BC after review): update `arch-state.md` `last_touched: { bc: <retarget BC or empty>, phase: <retarget phase>, at: <today> }`. Otherwise leave `arch-state.md` alone — Phase 4 review by itself does not move the personal cursor.
