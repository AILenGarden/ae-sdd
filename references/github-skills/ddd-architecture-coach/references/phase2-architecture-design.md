# Phase 2: Architecture Design

## Goal

Translate Phase 1's Bounded Contexts and Domain Events into an architecture-level design: Context Map, per-BC architecture decisions, cloud deployment blueprint, and AI-ADRs. Output is the architectural skeleton — Phase 3 will fill in implementation details for each BC.

Wrong architecture decisions here propagate to every BC's Phase 3 spec — this phase has the highest leverage per decision.

Phase 2 has two output scopes:
- **System-level**: Context Map (BC relationships) + deployment blueprint → append to `{coach_output_root}/system/context-map.md`
- **Per-BC**: architecture decisions + AI-ADRs → `{coach_output_root}/{bc}/decisions.md`

> Resolve `{coach_output_root}` from `.claude/project-context.md` (default `docs/ddd/`).

System-level sections are produced once (updated incrementally when new BCs are added). Per-BC sections can be produced as each BC reaches Phase 2.

---

## Operating Model

**You lead the production; the user reviews and challenges.** Draft the full architecture design; user reviews and pushes back on specific decisions.

Phase 2 is typically delivered as a single complete document — architecture decisions are interconnected (Context Map shapes BC decisions, BC decisions shape deployment, all three shape AI-ADRs), so splitting forces users to evaluate decisions without seeing the full picture.

**Exception**: when the user explicitly requests staged review, deliver section-by-section with confirmation between sections.

---

## Language Policy

Per SKILL.md: instructions in English, user-facing output in Traditional Chinese, technical terms in English. Chinese phrases inside 「...」 are verbatim templates.

---

## Mode Selection

| Mode | Trigger | Difference |
|------|---------|------------|
| **greenfield** | New project, no prior architecture decisions | Draft from Phase 1 + project-context.md alone |
| **incremental** | Adding to an established system (e.g., adding new BC to existing Context Map) | Read existing Phase 2 doc; produce delta only; flag conflicts with established decisions |

Always determine mode by probing the filesystem (per SKILL.md → State Determination): `{coach_output_root}/system/context-map.md` already has the relationships + deployment sections → `incremental`; otherwise `greenfield`.

---

## Pre-flight Checks

Before drafting, verify:

1. **Phase 1 complete and stable**: BC list, UL, Domain Events finalized; `unresolved_questions` resolved or explicitly carried forward
2. **`project-context.md` complete**: tech_stack, cloud_provider, team_size, budget_sensitivity all filled
3. **Existing project conventions known**: read CLAUDE.md, .claude/rules/ if present
4. **AI intervention candidates identified in Phase 1**: Phase 2 will produce AI-ADRs for each candidate
5. **Touchpoint Map read**: read `{coach_output_root}/system/touchpoints.md` (Phase 1 Step 5) before designing Context Map integration patterns. The touchpoints file already lists co-presence scenarios and derived integration pattern candidates (push subscription / SSE / polling / outbox + pubsub) per cross-touchpoint event flow — Phase 2 should validate / refine those candidates against BC-internal architecture, not re-derive them. If `touchpoints.md` is missing (legacy v0.1.0 project), flag the gap to the user before proceeding (per SKILL.md backward-compat warning).

Any missing → halt and request from user.

---

## Section Structure

Phase 2 spec contains seven sections in order. Each section's purpose, required content, and format rules below.

---

### §1 承接摘要 (Handoff Summary)

**Purpose**: Anchor the document to Phase 1 inputs and project constraints. Reader sees within 1 minute what assumptions Phase 2 is built on.

**Required content**:

1. **Phase 1 帶入的關鍵決策** (3-7 items): each item is a decision letter (e.g., "決策 A", "決策 F") + one-line summary
2. **Phase 2 期間解決的開放問題** (table): `# | 問題 | 解決方案`. Items here are Phase 1's `unresolved_questions` that Phase 2 closes.
3. **本 Phase 約束條件** (3-5 bullets): team size, budget sensitivity, special requirements (e.g., 供應商無鎖定, 產業中性)

**Format notes**:

- Top-of-file metadata block: version, date, phase label, input documents, deliverables, tech stack, cloud provider, team size, budget sensitivity
- Phase 1 decisions referenced by their original letter/number; if Phase 1 used named decisions, preserve names
- Open questions resolved in Phase 2 = "RESOLVED in Phase 2"; questions that remain → §7

---

### §2 Context Map

**Purpose**: Visualize and document inter-BC relationships. This is Phase 2's most-cited artifact in subsequent phases.

**Required content**:

#### §2.1 BC 間關係總覽

ASCII diagram (or equivalent text rendering) showing all BCs + their relationships.

**Diagram conventions**:
- Each BC marked with classification: `(Core)`, `(Supporting)`, `(Generic)`
- Each relationship marked with pattern: `[OHS+PL]`, `Customer-Supplier`, `Conformist`, `ACL`, `Shared Kernel`, `Separate Ways`
- Direction arrows show information flow (or dependency direction; document convention)

For systems with > 6 BCs, the diagram becomes hard to read. In that case, supplement with a table view in §2.2 instead of relying on diagram alone.

#### §2.2 關係明細

Table: `上游 BC | 下游 BC | Pattern | 選擇理由`

- One row per BC-pair relationship
- Pattern uses standard DDD vocabulary
- 選擇理由 is one sentence stating WHY this pattern (not what it does)
- For unidirectional patterns (Customer-Supplier), label which side is which

#### §2.3 關鍵設計備註

Prose section explaining non-obvious choices:

- Why pattern X was chosen over an alternative that might seem natural (e.g., "Session ↔ Catalog 不是 Shared Kernel" with rationale)
- Asymmetric cases (e.g., "Channel Gateway 需要 ACL，Notification 不需要" with reason)
- Default-to-ACL principle: when in doubt between Shared Kernel and ACL, default to ACL; document deviations

**Format notes**:

- Patterns must use DDD vocabulary, not made-up names
- ACL placement: explicitly state which BC builds the ACL (the consuming side typically)
- Conformist relationships: explicitly state what's being conformed to (the upstream's Published Language)

---

### §3 BC 內部架構決策

**Purpose**: Specify per-BC architecture decisions at a level Phase 3 can build on. Not implementation details — those are Phase 3.

**Required content per BC**:

Use the shared Decision Table format:

| 決策項目 | 選擇 | 理由 | 替代方案 | 何時該換 |
|---------|------|------|---------|---------|

Decision rows must cover (minimum):

1. **架構風格**: Hexagonal / Clean / Layered / Simple CRUD — pick one per BC
2. **持久化策略**: which DB technology + write pattern (Event Sourcing? CRUD? CRUD + history table?)
3. **AI 元件** (if applicable): which Port + how it integrates
4. **通訊模式**: how this BC publishes/consumes events; sync vs async
5. **測試策略**: what's tested at unit / integration / contract level

Additional decision rows as relevant per BC (e.g., specific to AI BCs: prompt safety; specific to Integration BCs: ACL granularity).

**Format notes**:

- Each BC gets its own sub-section (`### 3.1 Session BC (Core)`, etc.)
- 何時該換 column not optional — every decision must have a re-evaluation trigger
- Generic BCs (Identity, Audit) often use simpler architecture (CRUD + OHS); don't over-engineer them
- Core BCs typically Hexagonal; Supporting BCs depends on complexity

---

### §4 雲端部署藍圖

**Purpose**: Specify deployment topology + cost model + scaling triggers. This is where budget_sensitivity from project-context.md gets cashed out.

**Required content**:

#### §4.1 部署單元策略

- **Modular Monolith** vs **Microservices** vs **Service-per-BC** — pick one
- Trigger conditions for splitting (e.g., "WebSocket connections > 500" → Channel Gateway extracts)

For team_size: 1, default to Modular Monolith. Only deviate with explicit justification.

#### §4.2 資料庫策略

Tree or bullet list showing:
- DB topology (single instance? primary+replica? sharded?)
- Schema strategy (single schema? per-BC schema? per-tenant DB?)
- Multi-tenant isolation approach (RLS? application-level filtering? both?)

For multi-tenant systems with budget_sensitivity High: default to single-DB single-schema + RLS + application-level double-defense.

#### §4.3 部署拓撲

ASCII diagram (or equivalent) showing:
- Compute units (VMs, containers, serverless)
- Data stores (PG, Redis, etc.)
- External services (LLM provider, SMS, DNS, etc.)
- Geographic placement

Diagram should fit in one screen. Don't overload.

#### §4.4 成本模型

Two cost tables:

1. **MVP 初期** (table: `元件 | 服務 | 月費`)
2. **成長期** (table: same columns)
3. **每新增 tenant 邊際成本** (table: `成本項 | 增量 | 說明`)

For each cost figure: state currency, billing model (flat / pay-as-you-go), and source (real quote vs estimate).

#### §4.5 擴展觸發點

Table: `觸發條件 | 行動 | 預估時機`

- Conditions in concrete terms (e.g., "WebSocket connections > 500", not "high traffic")
- Actions specific (e.g., "VM 升級到 4vCPU/8GB", not "add capacity")

#### §4.6 可觀測性設計

Table: `層級 | 監控項 | 工具`

Layers: 基礎設施 / 應用層 / per-tenant 業務指標 / AI inference (if AI present) / 通知

Plus: 「MVP 最少告警規則」table with conditions and channels.

#### §4.7 供應商無鎖定策略

Only required if "供應商無鎖定" is in §1's constraints. List specific measures (Docker, DB-portable backups, DNS independence, etc.).

**Format notes**:

- Cost figures are estimates — explicitly mark `[需驗證]` if not from a real quote
- Geographic placement matters for latency; document if relevant (e.g., Tokyo region for Asia-Pacific users)
- Vendor-neutral defaults preferred for Generic Domain components (DB, Redis); vendor-specific OK for differentiated services (LLM, payments)

---

### §5 AI-ADR

**Purpose**: One AI-ADR per AI intervention point identified in Phase 1. Document why AI is being used (or rejected) for each candidate.

**Required content per AI-ADR**:

```
### AI-ADR #N: <intervention name>

**Context**: <why is AI being considered? what problem does it solve?>

**Decision**: <prompting / RAG / agent / fine-tuning / not using AI>

| 替代方案 | 為什麼不選 |
|---------|----------|
| <option> | <one-line rationale> |

**Consequences**:
| 面向 | 內容 |
|------|------|
| 預期效益 | <what AI achieves that non-AI can't> |
| 已知風險 | <list of risks: hallucination, injection, latency, etc.> |
| 監控指標 | <metrics that indicate AI is working / failing> |

**Validation**:
| 方式 | 內容 |
|------|------|
| Golden Dataset | <size + acceptance criteria> |
| Automated Checks | <pre/post conditions enforced in code> |
| Human-in-the-loop | <when humans intervene> |

**Fallback**:
| 層級 | 觸發條件 | 行為 |
|------|---------|------|
| 1 | <when level 1 fallback triggers> | <what happens> |
```

Plus a section listing **否決的 AI 介入點** (rejected AI candidates):

| # | 介入點 | 否決理由 | 替代方案 |

**Mandatory veto check** — Hard veto + Soft veto (per SKILL.md):

- **Hard veto** (any → reject AI; no override allowed):
  1. AI errors directly cause financial / legal harm AND there is no human-in-the-loop.

- **Soft veto** (any → presume reject; using AI requires written justification overriding the presumption):
  2. Deterministic algorithm achieves 95%+ accuracy.
  3. Fallback cost / latency within 30% of AI.
  4. Cannot define a golden dataset or validation criteria.

If Hard veto holds → AI-ADR moves to 「否決的 AI 介入點」 section unconditionally.

If only Soft vetos hold → AI-ADR may still be written **as an active decision**, but the Decision section must include an `Override Rationale` block explaining why the soft veto is overridden (e.g., 「條件 2 成立但 deterministic 算法在跨語系輸入降到 78%，AI 在多語系上仍勝出 — 接受 maintenance 成本」). No override block → reject and move to rejection list.

**Format notes**:

- Veto check explicit: each AI-ADR's Decision section answers all 4 conditions
- Validation must include concrete numbers (e.g., "F1 ≥ 0.90 on 100-180 test cases", not "high accuracy")
- Fallback must have 3+ levels (graceful degradation), not just "fail and alert"
- Security-critical AI (e.g., handling PII, financial decisions): explicit prompt injection / data leakage analysis required

---

### §6 系統參數累積決策 (Cross-cutting Settings)

**Purpose**: Capture system-wide parameters that emerged during Phase 2 design. Phase 3 will design the data structure (e.g., TenantSettings VO) to hold them.

**Required content**:

Table: `參數 | 歸屬 | 平台預設 | 租戶可覆蓋 | 限制`

- Parameters that span multiple BCs go here (e.g., `cookieExpiryDays`, `aiSecurityLevel`)
- BC-specific parameters stay in that BC's Phase 3 spec
- 歸屬 column: which BC owns the parameter's storage (typically Identity for tenant settings)

**Format notes**:

- This section may be empty for simple systems; if empty, write 「Phase 2 期間無新增系統參數」
- Parameters identified here flag a Phase 3 task: design the holding structure (e.g., TenantSettings VO)

---

### §7 開放問題（待 Phase 3）

**Purpose**: Track architecture-level questions that surface during Phase 2 but are better answered in Phase 3 with BC-level context.

**Required content**:

Table: `# | 問題 | 影響範圍`

- Questions concrete enough to be answered later (not vague TBDs)
- 影響範圍 names the BC(s) where Phase 3 will resolve

**Distinguish from §1's resolved questions**:
- §1's table = Phase 1 questions Phase 2 RESOLVED
- §7's table = questions Phase 2 surfaces, deferred to Phase 3

**Format notes**:

- Empty section forbidden — if no questions surfaced, document the rationale (e.g., 「Phase 2 完整收斂，Phase 3 直接進 BC 詳細設計」)
- Each entry should have a target Phase 3 deliverable that addresses it

---

## Self-Check Clean List

Before presenting any section, verify the draft passes the relevant checks. **Drafting-time discipline**: only run the rules marked `⭐ MUST` before showing output to the user. Rules marked `▢ Extended` are deferred to Phase 4 Review (where they're run as a holistic pass). This keeps drafting moving while still funneling all rules through eventual review.

---

### Category 1: Context Map Health

Applies to: §2 Context Map
⭐ **MUST** at drafting: CM-1, CM-2, CM-4. ▢ **Extended** (Phase 4): CM-3, CM-5.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| CM-1 | Every BC pair relationship uses standard DDD vocabulary | Pattern column shows OHS+PL / Customer-Supplier / Conformist / ACL / Shared Kernel / Separate Ways only | Made-up patterns like "tightly coupled", "linked via API" | Replace with standard DDD pattern + rationale |
| CM-2 | Every relationship has explicit selection rationale | 選擇理由 column is one sentence stating WHY | Empty 理由 column or "natural fit" / "obvious choice" | Write the actual reason: data ownership? language stability? deployment independence? |
| CM-3 | Default-to-ACL principle applied | When in doubt between Shared Kernel and ACL, ACL is chosen | Shared Kernel used widely without strong justification | Switch to ACL unless Shared Kernel cost is genuinely lower |
| CM-4 | ACL placement specified | When ACL is the pattern, the building side is explicit (typically consuming side) | "Both sides build ACL" or unspecified | Pick one side; document why |
| CM-5 | No circular Customer-Supplier | If A→B and B→A, the dependency is documented as bidirectional with rationale | Two Customer-Supplier rows in opposite directions without explanation | Refactor: split BCs or pick one direction |

---

### Category 2: BC Decision Completeness

Applies to: §3 BC 內部架構決策
⭐ **MUST** at drafting: BD-1, BD-2. ▢ **Extended** (Phase 4): BD-3, BD-4, BD-5.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| BD-1 | Every BC has all 5 mandatory decision rows | 架構風格 / 持久化策略 / AI元件(or N/A) / 通訊模式 / 測試策略 all present | One or more rows missing | Add the missing row; if not applicable for this BC, write "N/A" with rationale |
| BD-2 | Every decision has 何時該換 trigger | 何時該換 column populated | Empty cell | Write a concrete trigger condition (e.g., "WebSocket connections > 500", not "later") |
| BD-3 | Generic Domain BCs not over-engineered | Identity / Audit / similar use simple CRUD + OHS, not Hexagonal | Generic BC drafted with full Hexagonal stack | Simplify; document if Hexagonal genuinely needed |
| BD-4 | Core BC architecture matches Phase 1 complexity | Core BCs use isolation-friendly architecture (Hexagonal / Clean) | Core BC uses Simple CRUD | Upgrade to Hexagonal unless complexity is truly low |
| BD-5 | Persistence strategy aligned with team capacity | Team size 1: avoid Event Sourcing primary store unless strong rationale | Solo team chose Event Sourcing for all BCs | Use CRUD + event log table as compromise; full ES only for highest-value BCs |

---

### Category 3: Cloud Architecture Discipline

Applies to: §4 雲端部署藍圖
⭐ **MUST** at drafting: CA-1, CA-2, CA-4. ▢ **Extended** (Phase 4): CA-3, CA-5, CA-6, CA-7.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| CA-1 | Deployment unit strategy matches team size | team_size 1: Modular Monolith default | Solo team picks Microservices / Service-per-BC without strong justification | Default to Modular Monolith; document deviation rationale |
| CA-2 | Multi-tenant isolation has double defense | RLS at DB layer + application-level filtering | Only one layer of tenant filtering | Add the missing layer; document in §4.2 |
| CA-3 | Cost figures sourced or estimated explicitly | Each cost has source noted (real quote / estimate / vendor list price) | Cost figures appear without source | Add source; mark estimates with `[估算]` |
| CA-4 | Scaling triggers concrete | 觸發條件 uses measurable metrics (connections, latency, CPU%) | "Heavy traffic", "many users", "later" | Replace with concrete metric + threshold |
| CA-5 | Observability covers per-tenant business metrics | §4.6 has per-tenant business metrics row | Only infrastructure / system metrics | Add per-tenant metrics: conversion rate, error rate, AI confidence distribution |
| CA-6 | MVP alert rules ≤ 7 | §4.6 alert table has at most 7 rules for MVP | 15+ alert rules at MVP | Cut to 5-7 critical alerts; defer rest to growth phase |
| CA-7 | Vendor lock-in mitigation if required | If §1 lists "供應商無鎖定", §4.7 details specific measures | §1 mentions but §4.7 missing | Add §4.7 with Docker / portable backup / DNS independence |

---

### Category 4: AI-ADR Discipline

Applies to: §5 AI-ADR
⭐ **MUST** at drafting: AA-1, AA-2, AA-4. ▢ **Extended** (Phase 4): AA-3, AA-5, AA-6.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| AA-1 | Every AI candidate has either an AI-ADR or rejection entry | Phase 1's AI candidates all accounted for | An AI candidate from Phase 1 vanishes silently | Add either active ADR or rejection row in 「否決的 AI 介入點」 |
| AA-2 | Veto check explicit per ADR | Each ADR shows answers to 4 veto conditions | Veto conditions skipped or implicit | Add explicit veto answers; if any condition holds, move to rejection list |
| AA-3 | Validation has concrete numbers | Golden dataset size + acceptance criteria stated as numbers | "High accuracy", "good performance" | Specify: dataset size, F1 / precision / recall thresholds |
| AA-4 | Fallback has 3+ levels | Fallback table has multiple rows with progressive degradation | Single-row fallback ("if fail, alert") | Add levels: AI self-correction → human-in-loop → manual takeover → service degradation |
| AA-5 | Security-critical ADRs have injection analysis | If AI handles user input that could be injection target, prompt injection analysis present | No injection consideration despite user-text input | Add: input filter, two-pass architecture, output validation, system prompt minimization |
| AA-6 | Sensitive data exclusion documented | If AI context could leak sensitive data, exclusion mechanism stated | "We trust the AI not to leak X" | Document: which fields never enter LLM context, how this is enforced |

---

### Category 5: Cross-Phase Consistency

Applies to: All sections
⭐ **MUST** at drafting: XC-1, XC-3. ▢ **Extended** (Phase 4): XC-2, XC-4.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| XC-1 | Phase 1 BC list matches Phase 2 BC list | Same count, same names, same Core/Supporting/Generic classification | New BC appears or BC missing in §3 | Reconcile; if new BC needed, document in §1 with rationale |
| XC-2 | Phase 1 Domain Events appear in §2 relationships | Events flagged in Phase 1 surface as relationship triggers | §2 has no integration relationships despite Phase 1 listing cross-BC events | Add relationships covering each cross-BC event |
| XC-3 | Project-context.md constraints honored | budget_sensitivity, team_size, cloud_provider all respected in §3 / §4 | High budget_sensitivity but Microservices selected | Align decisions with constraints; document deviations |
| XC-4 | Tech stack from project-context.md applied | §3 / §4 use the specified language, framework, DB, cloud | Decisions use different stack | Align; if deviating, update project-context first |

---

### How to Use This List

After drafting a section, run the relevant categories' rules. Do not present output that fails.

---

## Common Review Findings

Findings reviewers commonly raise during Phase 2 review. Pre-empt by following the mitigation patterns.

---

### Finding 1: Shared Kernel 過度使用

**Why it matters**: Shared Kernel forces synchronized release cycles across BCs. In a team-of-1 system, this isn't an immediate problem; in a team-of-10+ system, it creates bottlenecks. Even in solo systems, Shared Kernel makes future BC extraction harder.

**Mitigation while drafting**:
- Default to ACL when in doubt
- Shared Kernel only when: shared concept is truly identical AND change rate is near-zero AND extraction cost is genuinely high
- Document why Customer-Supplier wasn't enough (Customer-Supplier is the next-step-up from Separate Ways and usually sufficient)

**Real example**: Session and Catalog both use ProviderId / PlanId. Drafter considered Shared Kernel. Reviewer pointed out that Catalog should evolve pricing logic independently, and what Session needs is the IDs (ValueObjects in both BCs, defined per-BC) plus a stable PriceResolver interface. Pattern chosen: Customer-Supplier (Catalog = Supplier, Session = Customer). This lets Catalog refactor internal pricing without breaking Session, as long as PriceResolver interface holds.

**Self-Check rule**: CM-3

---

### Finding 2: ACL 缺失導致 Domain 滲漏

**Why it matters**: When BC A consumes events from BC B without ACL, B's internal vocabulary leaks into A. If B refactors event schema, A breaks. Worse, A's Domain becomes coupled to B's modeling decisions.

**Mitigation while drafting**:
- For every cross-BC event consumption, ask: "Whose vocabulary is the event in?"
- If consuming side's vocabulary differs (or might differ in future) → ACL on consuming side
- Notification consuming standardized payloads (SMS sender contract) is OK without ACL
- Channel consuming Session events (Session's state machine vocabulary) needs ACL on Channel side

**Real example**: Channel Gateway consumes Session events to know when to relay messages. Without ACL, Channel code refers to "Proposing", "ProposedToShop" — Session's state machine vocabulary. When Session refactors states, Channel breaks. With ACL: Channel translates Session events into its own vocabulary (e.g., "RelayWindowOpen", "RelayWindowClosed") at adapter boundary. Session refactors freely.

**Self-Check rule**: CM-4

---

### Finding 3: AI Veto Check 跳過

**Why it matters**: Without explicit veto check, drafters justify AI use by what AI can do, not what it must do. Result: AI introduced where deterministic algorithms would suffice, creating maintenance / cost / risk overhead.

**Mitigation while drafting**:
- Every AI-ADR runs the Hard/Soft veto check explicitly (Hard first, then each Soft condition)
- If any condition holds → move to rejection list, document deterministic alternative
- Don't be polite about rejection: clearly stating "AI doesn't fit here" is more useful than soft acceptance

**Real example**: schedule parsing was initially proposed as AI-driven. Veto check: condition 1 (deterministic algorithm 95%+) — regex + per-shop format dictionary achieves ~100% on known formats. Decision: rejected; use regex engine. Saves cost, reduces risk, no fallback needed. Documented in 「否決的 AI 介入點」 with rationale.

**Self-Check rule**: AA-1, AA-2

---

### Finding 4: AI Fallback 單薄

**Why it matters**: Single-level fallback ("AI fails → alert humans") fails when AI fails frequently or at scale. Multi-level fallback distributes load across degradation tiers.

**Mitigation while drafting**:
- Every AI-ADR has 3+ fallback levels
- Level 1: AI self-correction (e.g., "ConfidenceLevel = Low → ask user to clarify")
- Level 2: Human-in-the-loop (e.g., "Two consecutive Lows → escalate to operator review queue")
- Level 3: Service unavailable (e.g., "LLM provider down → service mode notice + alert")
- Level 4 (optional): Quality degradation (e.g., "primary model fail rate > 20% → switch to fallback model")

**Real example**: NLP intent detection ADR had only "low confidence → escalate" initially. Reviewer required 4-level fallback: (1) low confidence → AI self-double-confirm; (2) two consecutive lows → operator review queue; (3) LLM service down → "system busy" + alert; (4) primary model quality degradation > 20%/day → switch to fallback model. Result: AI failures absorbed gracefully; operators only see truly hard cases.

**Self-Check rule**: AA-4

---

### Finding 5: 雲端拓撲過度設計

**Why it matters**: Solo / small teams pick Microservices / Kubernetes / multi-region for "future-proofing", spending capacity on infrastructure complexity instead of product. The cost is hidden in operational time, not just dollars.

**Mitigation while drafting**:
- team_size 1: Modular Monolith default, single VM, single DB instance
- team_size 2-5: Modular Monolith probably still right; consider service-per-BC only if actual scaling pain documented
- team_size 6+: depends on coordination cost, growth velocity
- Avoid Kubernetes for MVP unless team has Kubernetes expertise already

**Real example**: solo founder considered Microservices on Kubernetes "to learn". Reviewer pushed back: Modular Monolith on a single VM costs $18/month and runs everything; switching to K8s costs hundreds of hours of learning + significantly higher infrastructure spend; the product gets none of that investment. Decision: Modular Monolith now, document trigger conditions for later split (e.g., WebSocket connections > 500 → extract Channel Gateway).

**Self-Check rule**: CA-1

---

### Finding 6: 多租戶隔離單層

**Why it matters**: Single-layer isolation (only RLS, or only application-level filtering) fails silently when the layer is bypassed. RLS bypass: forgot to set `app.tenant_id` session variable; application-level bypass: forgot `.Where(TenantId == x)` in a query. Either causes cross-tenant data leak — high-severity security incident.

**Mitigation while drafting**:
- Multi-tenant systems require both layers
- DB layer: RLS policies with `USING (tenant_id = current_setting('app.tenant_id', true)::uuid)` or equivalent
- Application layer: every Repository base query auto-includes `.Where(TenantId == currentTenant)`
- Integration tests with zero tolerance: cross-tenant query returns 0 rows; missing tenant context returns 0 rows

**Real example**: original draft specified RLS only. Reviewer noted that any developer mistake in setting session variable bypasses isolation. Updated to double-defense: RLS at DB + Repository base filter at app layer + integration tests verifying both layers independently. If RLS is misconfigured, app layer catches it; if app layer query is wrong, RLS catches it. Both must fail simultaneously to leak.

**Self-Check rule**: CA-2

---

### Finding 7: 觀測性聚焦在基礎設施

**Why it matters**: Monitoring CPU / RAM / disk tells you the system is up. It doesn't tell you the product is working. Per-tenant business metrics (conversion rate, error rate, AI confidence) reveal product issues before they become outages.

**Mitigation while drafting**:
- §4.6 separates layers: infrastructure / application / per-tenant business / AI
- Per-tenant business metrics are mandatory for multi-tenant SaaS
- AI metrics include confidence distribution, fallback trigger rate, latency P50/P95/P99
- MVP alert rules ≤ 7 — distinguish "page me at 3am" from "review tomorrow"

**Real example**: original observability section had only VM CPU / RAM / DB connections. Reviewer noted: a tenant's product breaks (e.g., their AI confidence drops to 30%) without any infrastructure alarm. Added per-tenant business metrics: Session→Booking conversion rate, AI confidence Low ratio, customer takeover rate. These metrics surface product issues that infrastructure monitoring misses.

**Self-Check rule**: CA-5, CA-6

---

### Finding 8: 跨 Phase 不一致

**Why it matters**: Phase 2 is built on Phase 1; if Phase 2 silently invents new BCs, removes BCs, or contradicts Phase 1 events, Phase 3 implementers face contradictory specs. This is the most common cause of "spec drift" in DDD projects.

**Mitigation while drafting**:
- Open Phase 1 doc + project-context.md before drafting Phase 2
- Cross-check: BC list count matches; BC names match; classifications match
- If Phase 1 needs revision based on Phase 2 insights, do it explicitly: update Phase 1 first, then Phase 2 references the updated Phase 1
- Don't let drift accumulate

**Real example**: Phase 1 listed Conversational Intelligence as separate BC. Phase 2 drafter realized it makes more sense as a Port within Session BC. Two paths: (a) silently merge in Phase 2 — wrong, leaves Phase 1 inconsistent; (b) update Phase 1 to merge first, then Phase 2 reflects merged state — correct. Took option (b); Phase 1 v1.1 reflects merged state, Phase 2 v1.0 builds on Phase 1 v1.1.

**Self-Check rule**: XC-1, XC-2, XC-3, XC-4

---

### How to Use This List

| Section | Most relevant findings |
|---------|----------------------|
| §2 Context Map | 1 (Shared Kernel), 2 (ACL 缺失) |
| §3 BC Decisions | 5 (拓撲過度設計 mirror in BC architecture) |
| §4 Cloud | 5 (拓撲過度設計), 6 (隔離單層), 7 (觀測性) |
| §5 AI-ADR | 3 (veto check), 4 (fallback) |
| All sections | 8 (跨 Phase 不一致) |

---

## Claude's Proactive Mechanisms

Behaviors Claude executes during Phase 2 drafting (not error checks — positive behaviors).

### Pattern Defaulting

When relationship pattern is ambiguous, default to **ACL on consuming side**. Document the choice with one-line rationale. Reviewer pushback can switch to other patterns; defaulting to ACL prevents premature Shared Kernel commitments.

### Cost Reality Check

When drafting §4.4 cost model, mark estimates with `[估算]` and note assumptions. Don't present estimates as quotes. If a real quote is available (vendor pricing page), cite the URL. This prevents Phase 2 cost figures from being treated as commitments.

### Veto-First AI Drafting

When drafting an AI-ADR, run veto check FIRST. If any condition fails, draft the rejection entry, not the ADR. This avoids investing time in an ADR that will be rejected anyway.

### Cross-Phase Sync Audit

Before finalizing Phase 2, run a sync audit against Phase 1:
- BC count + names + classifications match?
- Domain Events appear as cross-BC integration relationships?
- AI candidates all accounted for (active ADR or rejection)?
- Open questions from Phase 1 either resolved or carried forward explicitly?

If sync audit fails, fix Phase 1 first (or document Phase 2's deviation with rationale), then complete Phase 2.

---

## Output Pacing

- **Single delivery default** (per Operating Model above); honor explicit section-by-section requests
- **Decision tables**: if §3 exceeds 15 rows, split by sub-section
- **Diagrams**: §2.1 and §4.3 must fit one screen; supplement with tables if topology is too complex

---

## Phase 2 Output Checklist

Phase progress is derived from filesystem (see SKILL.md → State Determination); do NOT mirror status, output paths, or per-BC summaries into `arch-state.md`. The only `arch-state.md` write is the personal cursor `last_touched: { bc, phase, at }` — gitignored, per-developer state, not team coordination.

When Phase 2 system-level reaches stable v1.x:

- Ensure `{coach_output_root}/system/context-map.md` includes both the relationships and deployment sections (their presence IS the completion signal). The deployment decision (Modular Monolith / Microservices / etc.) lives in the document, not in arch-state.
- Update `arch-state.md` `last_touched: { bc: '', phase: phase_2, at: <today> }` (or whichever BC the user is heading into next).

When Phase 2 per-BC reaches stable:

- Ensure `{coach_output_root}/{bc}/decisions.md` is written (its existence IS the completion signal). Decisions count, AI-ADRs, and rejection count live in that document.
- Update `arch-state.md` `last_touched: { bc: <BC>, phase: phase_2, at: <today> }`. The user can subsequently invoke `/phase-3 <BC>` explicitly — do not pre-update `last_touched.phase` to `phase_3` based on assumed next-step intent.

Append to `arch-learnings.md`:

- §7 unresolved questions (the file's `open_questions` section)
- Any learnings that emerged: `source: phase_2`
