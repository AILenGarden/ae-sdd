# Phase 3: Implementation Specification

## Goal

For one Bounded Context selected by the user, produce an implementation-ready specification that bc-developer agents (or human implementers) can use to build the BC without further architectural decisions. Output is a spec document, not code.

---

## Operating Model

**You lead the production; the user reviews and challenges.** You draft the full spec; user reviews in batches and challenges specific decisions. Apply the same review-batching pattern that proved effective in real testing (P0 / P1 / P2 severity layering for review findings).

Phase 3 produces **the longest single artifact** of the four phases (typically 500-1500 lines for a Core BC). Pace output accordingly.

---

## Language Policy

Per SKILL.md: instructions in English, user-facing output in Traditional Chinese, technical terms in English. Chinese phrases inside 「...」 are verbatim templates.

---

## Mode Selection

| Mode | Trigger | Difference |
|------|---------|------------|
| **template-lead** | This is the first BC the user is doing Phase 3 for, OR no prior Phase 3 spec exists in the project | Use Phase 1/2 outputs + project conventions to draft from scratch |
| **template-from-prior** | A prior BC has a completed Phase 3 spec | Use the prior spec as structural template; produce the new BC in matching shape |

**template-from-prior differential rule**: when a prior BC's spec exists, sections that follow established patterns (directory structure, Port→Adapter mapping, stub behavior, reflection test templates, wire format conventions) should state 「沿 {prior BC} pattern」 with a cross-reference, then document ONLY what differs in this BC. Do not reproduce content that is identical to the prior BC. Detail only what is new or specific to this BC.

Always determine mode by scanning `{coach_output_root}/*/spec.md` — if any other BC's `spec.md` exists with frontmatter `status: v1.x` (or, for older specs without the status field, simply exists), use **template-from-prior** with that BC's spec as the structural template. Otherwise, **template-lead**. Tell the user which mode and why before starting.

---

## Pre-flight Checks (do before drafting)

Before producing any spec content, verify:

1. **Phase 1 outputs exist and are stable**: BC list, UL, Domain Events for this BC are finalized
2. **Phase 2 outputs exist and are stable**: Context Map for this BC, architecture style, persistence strategy, AI-ADRs are finalized
3. **User has explicitly named which BC for Phase 3**: do not pick a BC from Phase 2 list silently
4. **Existing project conventions are read**: `CLAUDE.md`, `.claude/rules/`, prior BCs' Phase 3 specs (if any), prior BCs' implementation rules (if any)

Any missing → halt and request from user. Do not draft on incomplete foundation.

---

## Section Structure (the spec's actual content)

The spec for one BC contains these sections in order. Each section's purpose and minimum content listed; full templates in stages below.

### §1 TL;DR + Handoff Summary

**Purpose**: Reader sees within 1 minute what this BC does, what's in MVP-Core, what's deferred, what decisions were inherited from Phase 1/2.

**Required content**:

1. **One-paragraph BC description** (3-5 sentences): role in product, core responsibility, key complexity (state machine? cross-BC events? AI integration?)
2. **MVP-Core scope summary**: 1-2 lines stating what's in vs out (full §2 details below)
3. **Inherited decisions table** (3-5 rows): each row = one decision from Phase 1 or Phase 2 that anchors Phase 3
   - Format: `| Decision | Source | This spec's chapter |`
4. **Phase 3 conclusions table** (one row per resolved P-question or Q-question): summary of major spec-level decisions made in this Phase 3 iteration, with link to full chapter
   - Format: `| 議題 | 決議摘要 | 本 spec 對應章節 |`
5. **Constraints to watch**: 3-5 bullet points of project-wide rules that constrain this BC (e.g., RLS zero tolerance, sensitive data firewall, BC reference rule)

**Format notes**:

- Use a top-of-file metadata block (YAML frontmatter): `bc`, `version`, `phase: 3`, `date`, `status: draft | v1.x`, `input_documents`, `output_documents`, `subscribers`. The `status` field is the source of truth for Phase 3 stability — replaces the previous `arch-state.completed_bcs` mechanism. Set `status: draft` while iterating; flip to `status: v1.x` once the spec is stable enough to serve as a `template-from-prior` template for subsequent BCs.
- The `subscribers` field is a list of touchpoint identifiers (cross-referencing `{coach_output_root}/system/touchpoints.md`) — every UI / channel surface that subscribes to this BC's emitted events, including secondary observers (agent console, supervisor dashboard, audit/compliance log viewers). Phase 4 review checks alignment between this list and the touchpoints file. If `touchpoints.md` is absent (legacy v0.1.0 project), leave `subscribers:` empty and note the gap in Open Questions.
- Mark inherited decisions with their phase source (e.g., "Phase 1 決策 A", "Phase 2 §3.1") for traceability

### §2 MVP-Core Scope Boundary

**Purpose**: Make the MVP-Core boundary unambiguous so implementers know exactly what to build and what to skip. Prevent scope creep and prevent missing critical features.

**Required content**:

1. **Full state machine vs MVP-Core compression** (if applicable):
   - Table mapping `MVP-Core 狀態 | 對映完整狀態 | MVP-Core 職責 | 延後項目`
   - State machine compression rationale (e.g., 9 → 5 states for reducing test surface; document trade-off)
2. **MVP-Core 必做清單**: bullet list of every Aggregate, Port, table, endpoint, test category that MUST be in MVP-Core
3. **MVP-Core 明確延後清單**: bullet list of every feature explicitly excluded, each with target slice name (e.g., `session-nlp-advanced`, `outbox-pipeline`)
4. **Stub Port inventory**: which Ports are MVP-Core stubs, what behavior each stub returns

**Format notes**:

- Both 必做 and 延後 are non-negotiable lists — no maybe / TBD items
- Every 延後 item has a named slice (no "future" or "TBD" labels)
- If a state machine is compressed, the compression decision must appear in §10 Open Questions Log with rationale

### §3 Aggregate Definitions

**Purpose**: Specify each Aggregate completely enough that an implementer can write the Domain layer without architectural decisions.

**Required content per Aggregate**:

1. **業務場景 table**: `Who | When | What | Frequency` — establishes who triggers operations and how often. Catches missing actors or unclear responsibility.
2. **Aggregate structure diagram**: pseudo-code showing Root + nested entities + VOs + state fields. Mark Conformist fields (from other BCs) explicitly.
3. **Value Objects table**: `名稱 | 職責`. Each VO has a single-line definition.
4. **Invariants table**: `ID | Invariant | MVP-Core?`
   - Numbered with prefix (e.g., S-1 for Session, B-1 for Booking) for cross-reference
   - **Each Invariant must concern ONLY this Aggregate's own state** — no references to other Aggregates' internal state
   - Mark MVP-Core ✅ vs delayed ❌
5. **Methods table**: `Method | 前置狀態 | 後置狀態 | 發布事件`
   - Method signature uses concrete types (not pseudo-code)
   - For methods with non-trivial behavior, add inline note explaining what the method does beyond state transition
6. **Sub-aggregates if any**: state machine diagram (text format), with explicit transitions and forbidden transitions
7. **Key Examples (Gherkin)**: per Aggregate, organized under User Stories from Phase 1. Follow the SBE Writing Guide section below. Each Aggregate must have 🟢 happy path + 🟡 edge case + 🔴 rejection coverage. Key Examples are the primary source for Domain Layer tests (§9.1).

**Format notes**:

- One sub-section per Aggregate (`#### Aggregate A`, `#### Aggregate B`)
- If two Aggregates are coupled by a single business operation (e.g., Session + Booking), document the coupling pattern in §6 (Application Handler), not in Aggregate Invariants
- Domain Events emitted listed at method level, then collected in §5

**Common pitfall**: writing "must be called by handler X" as an Invariant. This is an Application-layer concern. Move to §6.

### §4 Ports & Adapters

**Purpose**: Define the BC's boundary with the outside world (other BCs, infrastructure, external services). Domain Ports vs Application Ports distinction must be explicit.

**Required content per Port**:

1. **Layer**: Domain or Application (Domain Ports go in `{BC}.Domain/Ports/`, Application Ports go in `{BC}.Application/Interfaces/`)
2. **Interface signature** (concrete C# / TypeScript / language-of-project):
   - Method signatures with full types
   - Return value types (use Result types if applicable; document)
3. **MVP-Core Adapter implementation**:
   - Class name (e.g., `StubNlpParser`, `NoOpNotificationPublisher`)
   - Behavior specification (what the stub returns, what events it publishes)
   - For Stubs that mimic async behavior: specify dispatch mechanism (in-process MediatR? actual delay?)
4. **Future real implementation deferral**: which slice will replace the stub

**Domain Layer vs Application Layer Port boundary**:

- **Domain Port** (e.g., `INlpParser`): the Domain layer needs to express a capability without knowing implementation. Domain depends only on the interface.
- **Application Port** (e.g., `IPriceResolverClient`, `IShopAvailabilityClient`): wraps a cross-BC integration or external system call. Application orchestrates these.

**Cross-BC Ports**: must use ACL (Anti-Corruption Layer) pattern. Adapter translates external schema to local language. Port lives in this BC, adapter in this BC's Infrastructure layer.

**Format notes**:

- Group Ports by layer (Domain Ports first, then Application Ports)
- Each Port gets its own sub-section (`#### IPortName`)
- For complex Ports (e.g., 2-step async dispatch), include a sequence diagram or step-by-step flow

**Common pitfall**: Application Ports leaking into Domain layer (e.g., Domain code importing `Yobuki.Catalog.Application.IPriceResolverClient`). If Domain needs pricing, define a Domain Port in this BC; Application Port wraps the cross-BC call.

### §5 Domain Events + Dispatch Mechanism

**Purpose**: List every Domain Event this BC emits + define the dispatch architecture + enforce sensitive-data exclusion at event-payload level.

**Required content**:

#### §5.1 Event Catalog

Table: `Event Name | Payload | MVP-Core?`

- Event name uses past tense + business language (forbid technical names like `DataSyncedToRedis`)
- Payload column lists fields explicitly; each field is a primitive or VO
- **Payload must NOT contain sensitive data fields** (e.g., Cost, internal pricing). Cross-BC subscribers may persist event payloads — assume worst case.
- MVP-Core ✅ vs delayed ❌

After the table: **「外發事件 payload 規則」note** — explicit policy that all events (intra-BC + cross-BC) exclude sensitive fields. List which fields are sensitive for this BC (e.g., `ProposedCost`, `CostAmount`).

#### §5.2 Cross-BC Integration Events

If any events are intended for cross-BC consumption:

- Sub-list of which events are integration events
- Current dispatch mechanism (MVP-Core: typically MediatR in-process, no subscribers yet)
- Future migration path (e.g., when outbox pipeline ships, these move to `IIntegrationEvent` + outbox persistence)

#### §5.3 Dispatch Mechanism (3-Phase Pattern)

The 3-Phase pattern is described as a **stack-agnostic contract** first, then with concrete examples per stack. The contract is what every BC's spec should specify; the example shows how it maps to a specific stack.

##### Contract (stack-agnostic)

| Phase | Timing | Transaction scope | Goal |
|-------|--------|-------------------|------|
| 1. Collect | Before persistence is committed | Same tx as aggregate writes | Gather domain events from aggregates that changed in this unit-of-work; clear them on the aggregates |
| 2. History persist | Before persistence is committed | Same tx as aggregate writes | Append each collected event to the history / event-log table — atomic with the aggregate change |
| 3. Notification dispatch | After persistence is committed | **Independent scope** | Publish events to in-process or out-of-process subscribers; subscriber failures do NOT roll back the original write |

**Stack-agnostic key rules** (apply regardless of framework):

- All three phases must be triggered from a **single, framework-level commit hook** so application code cannot accidentally split or skip them. Application handlers must NOT compose phases manually.
- One Command Handler = one persistence commit (multiple commits per handler would split history into batches and lose atomicity).
- Cross-Aggregate handlers must share the same unit-of-work / persistence scope so all aggregate changes commit atomically.
- Phase 3 (notification) failure semantics: subscriber exception → subscriber's own scope rolls back; original request's aggregate write + history persisted in Phase 2 are unaffected.

##### Example mapping per stack

The contract maps to different framework hooks. Pick the row matching this project:

| Stack | Phase 1+2 implementation site | Phase 3 implementation site | Notes |
|-------|-------------------------------|----------------------------|-------|
| **.NET / EF Core / MediatR** (default example used in this skill) | `DbContext.SaveChangesAsync` override; collect from `ChangeTracker.Entries<IAggregateRoot>()`, append to history table, then call base `SaveChangesAsync` | Inside the override, after base `SaveChangesAsync` returns, publish `DomainEventNotification<T>` via `IPublisher` | Cross-Aggregate handlers share scoped `DbContext` via DI scope |
| **Java / Spring + JPA** | `@TransactionalEventListener(phase = BEFORE_COMMIT)` to write history, OR Hibernate `Interceptor.preFlush` to collect | `@TransactionalEventListener(phase = AFTER_COMMIT)` for notifications via `ApplicationEventPublisher` | Phase 1+2 can also be done in a `@PreUpdate / @PrePersist` JPA listener |
| **Node.js / TypeORM or Prisma** | Custom unit-of-work wrapping `entityManager.transaction(async (em) => { ... })`; collect domain events from aggregates, write history within callback | After the transaction Promise resolves, dispatch via in-process EventEmitter or a queue | No native ChangeTracker — aggregates expose `pullDomainEvents()` |
| **Python / Django** | `pre_save` / pre-commit signal to collect, write history before `transaction.atomic()` exits | `transaction.on_commit(...)` to dispatch notifications via Celery / in-process pub-sub | Use a unit-of-work helper to scope per-request |
| **Python / SQLAlchemy** | `before_commit` event to collect + write history | `after_commit` event to dispatch | Aggregates store events on a per-session attribute |
| **Go / GORM or sqlc** | Wrap repository writes in a unit-of-work that collects events from aggregates, writes history within the same `*sql.Tx` | After `tx.Commit()` succeeds, fan out via channels or message bus | No framework hooks — implement explicitly in the unit-of-work helper |

If your stack is not listed, identify the equivalent of "before-commit hook" + "after-commit hook" in your framework and document it in this BC's spec. The contract is what matters; the implementation surface is local to the stack.

**Format notes**:

- 3-Phase pattern is one option. If the project uses different dispatch (e.g., outbox-only, or Repository-level publish), document chosen pattern + rationale + trade-offs.
- For stub / sync dispatch in MVP-Core: explicitly note 「in-process synchronous dispatch; tx1/tx2 race window is microseconds, acceptable. Future Channel BC integration will widen the window — must reassess at slice X.」

### §6 Application Layer

**Purpose**: Specify every Command / Query / Event Handler with concrete responsibility chains, transaction boundaries, and race condition handling.

**Granularity boundary — contract not code**:
- §6 specifies WHAT each handler does (responsibility chain, transaction boundary, race handling), not HOW (no full interface signatures, no complete record definitions, no error code strings)
- **Command Handlers**: group handlers that follow the standard Load→Method→Save pattern into one summary row; only detail handlers with non-trivial logic (multi-step, cross-Aggregate, conditional branching)
- **Views**: field name + type table only; do not write full language-specific record/class definitions
- **Race / Failure Matrix**: describe handling layer + behavior; error code strings belong in §7 Wire Format, not here
- **Repository interfaces**: do not list method signatures; implementers derive these from Command/Query specs

**Required content**:

#### §6.1 Command Handlers

Table: `Command | 職責 | 呼叫來源`

- 職責 column lists the step sequence (e.g., "1) Validate via S-1; 2) Aggregate.Method(...); 3) Repository.SaveAsync()")
- Steps must be at the granularity of "what happens" not "how it happens" (no implementation details like "call MediatR.Send")
- 呼叫來源: HTTP endpoint or other trigger

For complex handlers (multi-step, cross-Aggregate): include sub-section with step-by-step pseudo-code or sequence diagram.

#### §6.2 Query Handlers

Table: `Query | 回傳 | 呼叫來源`

- 回傳: View DTO type
- Query handlers project Aggregate state to View DTOs — never expose Aggregate or VOs directly

#### §6.3 Views (DTO Specification)

Per View type, provide a field table: `Field | Type | Notes`

- Exclusion rules: state which fields are forbidden (e.g., "no TenantId — JWT carries it", "no Cost — sensitive firewall", "no Domain VO as property type")
- Do not write full record/class definitions — field table is the contract; implementer writes the code
- Projection method rule: must use explicit field-by-field assignment. Forbid auto-map / JsonIgnore reliance.

#### §6.4 Event Handlers

Table: `Handler | 處理事件 | 職責`

- One row per Event Handler
- Handlers run in their own transaction (per §5.3 Phase 3)
- Failure handling: explicitly state log + throw vs swallow (default: log + throw)

For cross-Aggregate handlers that intentionally violate "single Aggregate per transaction": **document the rationale explicitly** in a callout. Include:
- Why the cross-Aggregate operation is atomic in business semantics
- Concurrency risk assessment
- Future re-evaluation trigger (e.g., when outbox pipeline ships)

#### §6.5 Race / Failure Matrix

Table: `情境 | 處理層 | 行為`

- Lists known race conditions and failure modes
- For each: which layer handles (Aggregate Invariant? Handler? Infrastructure?) and what behavior results (reject, retry, compensate)
- Do not specify error code strings here — those belong in §7 Wire Format
- Example rows: "Handler load Aggregate fails → Infrastructure → 404", "Concurrent state transition → Aggregate Invariant → reject"

#### §6.6 Save Boundary Rules

Explicit rules:

- One Command Handler = one `SaveChangesAsync` call (no multi-save handlers)
- Handler does NOT manually compose dispatch phases (§5.3)
- UoW封裝 vs direct DbContext.SaveChangesAsync: state which approach is chosen and why
- Cross-Aggregate handlers: shared DbContext instance via scoped DI

**Common pitfall**: Multi-step handlers manually calling `SaveChanges` between steps for "intermediate consistency". This breaks dispatch atomicity. Refactor into single transaction or split into multiple handlers.

### §7 HTTP Endpoints

**Purpose**: Specify the BC's REST API surface — paths, auth, status codes, wire formats.

**Required content**:

#### §7.1 Customer-Facing Endpoints (if applicable)

If the BC has end-user-facing endpoints:

- **URL design principle stated explicitly**: customer-scoped (URL never contains internal IDs the customer shouldn't see) OR resource-based (URL contains resource ID)
- Table: `Method | Path | 說明 | Auth | 成功 status`
- Auth column: which JWT / token type is required, what claims are extracted
- Success status column: explicit (e.g., "200 + SessionView", "404 + `{ code: \"no_active_session\" }`")

#### §7.2 Admin / Internal Endpoints (if applicable)

- **URL design principle**: typically resource-based (admins see internal IDs)
- Same table format as §7.1
- Add Role Gate column if RBAC applies

#### §7.3 Wire Format Conventions

Explicit rules for:

- Status string format (snake_case? camelCase? PascalCase?)
- Money / amount representation (`{ amount: N, currency: "TWD" }` vs raw number)
- Time format (ISO 8601 UTC? local? epoch?)
- Error response shape (`{ code: "...", message: "..." }` vs RFC 7807 problem details)
- Enum serialization (string vs integer)

**Format notes**:

- Endpoints that bypass URL ID exposure (e.g., `/me/session` instead of `/sessions/{id}`): document the security reasoning in the URL design principle
- Idempotency rules: which POSTs are idempotent (e.g., "POST /webchat/me/session: already-active → 200 + existing")

**Common pitfall**: exposing internal IDs in customer-facing URLs without ownership checks. Either use `/me/...` style customer-scoped paths, or add explicit per-request ownership validation in the handler.

### §8 Infrastructure

**Purpose**: Specify directory structure, persistence schema (tables + RLS + indexes), and Stub adapter implementations sufficient for an implementer to wire the Infrastructure layer correctly.

**Required content**:

#### §8.1 Directory Structure

**template-lead mode**: provide the full pseudo-tree with actual file names for key files (Aggregate roots, Domain Services, DbContext). Sub-directory categories (Aggregates, ValueObjects, Ports, DomainEvents, Commands, Queries, etc.) follow standard layered convention — list the categories, not every leaf file.

**template-from-prior mode**: state 「沿 {prior BC} 三層 csproj 結構」, then list ONLY files whose placement is BC-specific or non-obvious (e.g., a Domain Service that could be mistaken for an Application Service, an orchestrator that lives in Application not Domain). Typically 3-5 lines.

#### §8.2 Port → Adapter Mapping

**template-lead mode**: full table mapping each Port to its MVP-Core Adapter.

**template-from-prior mode**: state 「沿 {prior BC} pattern」 for shared infrastructure Ports (ITenantScope, IPublisher, IClock, etc.). List only BC-specific Ports and their Adapters.

#### §8.3 Persistence Schema

Per table:

- Columns + types (no migration syntax — high-level schema)
- `tenant_id` column (always present for multi-tenant BCs)
- RLS policy: `USING (tenant_id = current_setting('app.tenant_id', true)::uuid)` or equivalent (state PostgreSQL session variable name explicitly)
- Application-level filter: `.Where(x => x.TenantId == current)` as second defense
- Indexes (specify which are MVP-Core, which are deferred)
- Migration management: which project owns migrations (typically Host project)

For append-only tables (e.g., event log / history): explicitly note append-only nature + payload schema (which fields, sensitive-data exclusions).

For tables with FK to other BCs' tables: state policy (FK at DB level? Application-level reference only? RLS-aware FK?).

#### §8.4 Stub Behavior Specifications

For each Stub adapter, specify:

- What the stub returns / publishes
- Whether it simulates async behavior (e.g., `StubShopAvailability` publishing an event vs returning bool)
- Real-implementation replacement plan (which slice, what changes)

For Stubs that publish real Domain Events (vs returning values): explicitly document the choice. This affects 2-step handler architecture (§6.4).

**template-from-prior mode**: if this BC has no BC-specific stubs (all stubs are shared infrastructure from prior BCs), state this explicitly and cross-reference §4 Ports where stub behavior was already specified. Do not reproduce §4 content.

**Format notes**:

- RLS rules use the EXACT PostgreSQL session variable name the project uses (e.g., `app.tenant_id` vs `app.current_tenant_id` — these are NOT interchangeable across project conventions)
- Migration tooling: state which library (EF Core Migrations, Flyway, custom) and where migrations live
- Init SQL: if any data needs seeding (RLS bootstrap roles, system tenants), document which slice handles it

**Common pitfall**: drift between spec's RLS variable name and project convention. Always cross-check existing rules / `.claude/rules/` before specifying.

### §9 Test Strategy

**Purpose**: Specify test coverage expectations across layers + architectural / reflection tests that enforce design rules at CI time.

**Required content**:

#### §9.1 Domain Layer Tests

- **Key Examples are the primary test source**: every Key Example (KE-{ID}) in §3 maps 1:1 to a test. Test method name format: `KE_{Aggregate}_{ID}_{behavior_description}`. Test body follows Given-When-Then → Arrange-Act-Assert.
- Three-color coverage verification: confirm every Aggregate has 🟢 + 🟡 + 🔴 tests
- State machine transition coverage: 100% of state × command combinations (legal + illegal) — Key Examples should already cover these; fill gaps if any
- Domain Event emission: assert each command emits the expected event type + payload structure
- VO equality / immutability tests where relevant

#### §9.2 Application Layer Tests

- Command handler full-flow tests (real DbContext via testcontainers + stub Ports)
- Concurrency tests for uniqueness invariants (e.g., S-1 race)
- **Architectural / Reflection tests** (CI gate, fail build on violation):
  - **Test A — View DTO sensitive-field exclusion**: scan all `*View` / `*ListItemView` types, fail if any public property name contains forbidden keywords (e.g., `cost`)
  - **Test B — VO not exposed via DTO**: scan all `*View` types, fail if any public property type is a Domain VO (whitelist for explicit exceptions like `Money`)
  - **Test C — Domain Event payload sensitive-field exclusion**: scan all event records, fail if any property name or type matches sensitive keywords
  - **Test D — Tenant isolation**: scan all `*View` types, fail if any contains `TenantId` (caller's JWT carries it)

For each architectural test: provide concrete pseudo-code or assertion logic. Test must be machine-verifiable, not documentation-only.

#### §9.3 Event Handler Tests

- Multi-step handler tests with intermediate state assertions (e.g., 2-step ConfirmProposal: assert state at end of step 1 AND step 2)
- Race condition tests: explicitly construct race scenarios (e.g., manually inject Cancel between phases) and assert correct invariant blocking

#### §9.4 RLS Integration Tests (zero tolerance)

- Standard RLS test pattern (e.g., `RLS-1` write A, query A; `RLS-2` write A, query B → empty; `RLS-3` cross-tenant query returns empty without leaking existence)
- Run for every tenant-scoped table
- Negative tests: cross-tenant access attempts must return 0 rows / 404, never 403 (existence leakage)

#### §9.5 E2E Tests

- Happy path: full state machine traversal end-to-end
- Negative paths: critical security / consistency scenarios (cross-tenant API calls, unauthenticated access)
- Wire format assertions: response JSON does not contain forbidden keys (e.g., `cost`)
- Idempotency tests for all idempotent endpoints

**Format notes**:

- Each test category has explicit MVP-Core ✅ vs delayed ❌ markers
- Architectural tests are MVP-Core mandatory — they cannot defer
- Reflection test scopes: clearly state which assemblies / namespaces the test scans

**Common pitfall**: relying on JsonIgnore attributes or serializer config to enforce sensitive-field exclusion. These are not architectural tests — they fail silently on serializer change. Always pair with reflection-based tests.

### §10 Open Questions Resolution Log

**Purpose**: Track every architectural question raised during Phase 3 drafting and its resolution status. Prevents "decided but forgotten" drift and gives future implementers a record of why specific choices were made.

**Required content**:

#### §10.1 Resolution Status Categories

Use these status labels strictly:

- **RESOLVED**: question is fully answered, decision is locked, implementer follows the decision
- **DEFERRED → `<slice-name>`**: question is intentionally delayed; specific slice will address it. Must name the slice (no "future" / "TBD")
- **OBSOLETE**: question was raised but the underlying assumption changed; record kept for historical context

DO NOT use RESOLVED for decisions that are actually deferred. "We'll decide later" is DEFERRED, not RESOLVED.

#### §10.2 Question Format

Table or list format:

| ID | 議題 | 狀態 | 決議摘要 | 對應章節 / 目標 slice |
|----|------|------|---------|-------------------|
| Q1 | <one-line question> | RESOLVED | <one-line decision summary> | §X.Y |
| Q2 | <one-line question> | DEFERRED → outbox-pipeline | <one-line deferred reason> | — |

Each entry:

- ID stable across versions (Q1 stays Q1 even if deleted/replaced — use Q-v1.1-001 if introducing in version revision)
- 議題: phrased as a question, not a statement
- 決議摘要: 1-2 sentences max; full rationale in the corresponding §X chapter
- 對應章節: link to where the decision is documented (RESOLVED only) or target slice (DEFERRED only)

#### §10.3 Empty Section Forbidden

If genuinely no open questions arose:

```
本 BC 無明顯 open questions。理由：
- Phase 1/2 已涵蓋本 BC 所有跨層議題
- BC 本身複雜度低（無 AI 元件、無跨 BC 異步事件）
- 既有專案慣例覆蓋本 BC 大部分實作決策
```

Forcing this rationale prevents lazy "no questions" from hiding unaddressed issues.

#### §10.4 Cross-Reference Rule

Every RESOLVED entry must point to the chapter where the actual decision lives. If the decision spans multiple chapters, link the primary one.

Every DEFERRED entry must name a specific slice. The slice may not yet exist — if so, propose the slice name and add a TODO entry.

**Common pitfalls**:

- Marking "we'll decide during implementation" as RESOLVED. This is DEFERRED.
- Empty 對應章節 columns. If RESOLVED but no chapter documents the decision, the spec is incomplete — fix the spec, don't leave the column blank.
- Adding new questions in v1.x revisions without renumbering. Use Q-v1.1-001 etc. to preserve original Q numbers.

### §11 Implementation Slice Order

**Purpose**: Translate the spec into a vertical-slice implementation plan that bc-developer agents (or human implementers) execute. Each slice is independently shippable and tested.

**Required content**:

#### §11.1 Slice Sequence

Per-slice table or list:

```
{n}. **{bc}-MVP{n}**: <scope summary>
   - Includes: <bullet list of components: Aggregates, Handlers, Adapters, Migrations, Endpoints, etc.>
   - Tests: <expected test categories + minimum count if predictable>
   - Excludes: <explicit list of what's NOT in this slice but IS in MVP-Core overall>
```

Default 5-slice template (modify as needed):

| Slice | Scope | Tests Expected |
|-------|-------|----------------|
| MVP1 | Domain skeleton (Aggregates + VOs + Invariants + Domain Event types + `_domainEvents` collection) | Domain unit tests only; no MediatR / persistence |
| MVP2 | Application + Infrastructure (Handlers + Ports + DbContext + Stubs + Cost firewall reflection tests) | + Application + Infrastructure tests |
| MVP3 | Persistence (Migrations + RLS + integration tests with testcontainers) | + Integration tests + RLS negative tests |
| MVP4 | HTTP Endpoints + auth wiring + E2E | + E2E tests + cross-tenant API negative tests |
| MVP5 | UI integration (replace mocks with real API) + carryover decisions | + UI tests |

#### §11.2 Per-Slice Acceptance Criteria

Each slice must pass:

- All previous slices' tests still green
- New tests for this slice's scope green
- Static analysis / linting clean
- BC boundary check (e.g., `scripts/check-bc-boundaries.sh`) — verifies this BC didn't reach into another BC's internals
- Commit message convention: `feat({bc}): ... — MVP{n}`

#### §11.3 Pre-MVP1 Actions

Before opening MVP1 slice, these must be complete:

- `.claude/rules/{bc}.md` drafted (see §11.4)
- Phase 3 spec frontmatter set to `status: v1.x` (the spec's own frontmatter — not arch-state — is the source of truth for stability)
- Pre-flight: confirm Phase 1/2 outputs match Phase 3 (no contradictions)

#### §11.4 `.claude/rules/{bc}.md` (Implementation Rules Card)

After spec stabilizes (v1.x), generate the rules card. Content (condensed from spec):

- Ubiquitous Language table (this BC's terms)
- Aggregate definitions (top-level structure + Invariants list)
- Cost / sensitive data firewall rules
- Domain Event dispatch mechanism (3-Phase summary)
- DEFERRED issues reminder list (so implementer knows what NOT to attempt)

Rules card is a quick-reference for bc-developer agents during implementation. Spec is canonical; rules card is condensation. On conflict → spec wins.

#### §11.5 Slice Modification Rules

Default 5-slice template assumes:

- BC has Domain + Application + Infrastructure + HTTP + UI layers
- Tests follow standard categories
- Single repository / monorepo project structure

Deviations require rationale documented in this section:

- BC with no UI → drop MVP5
- BC with no HTTP (pure background worker) → restructure MVP3/MVP4
- BC with cross-BC integration as primary scope → may need MVP6 for integration testing

**Common pitfall**: opening MVP1 without `.claude/rules/{bc}.md` drafted. Implementers without the rules card will diverge from spec on details that "feel obvious" but aren't.

---

## SBE Writing Guide

Key Examples are specs, not scripts. They describe business behavior (What), not software design (How). Coach produces the initial version (AI 開路); final sign-off requires team cross-functional review.

> **Language note**: This guide's metaphors (羊頭/蜂腰/蝎尾) and Then-marker (「應」) are designed for **Traditional Chinese Gherkin output** — they enforce syntactic discipline that Chinese prose lacks (no native modal verb for assertion). If a team writes Gherkin in English, substitute `should` / `must` for 「應」; the rest of the discipline (single When, verifiable Then, business-language Given, three-color coverage) is language-neutral and applies identically.

### Writing Order

**Start from Then** — align on business outcome first, then derive Given and When backwards.

### Structure Rules (羊頭蜂腰蝎尾)

**蝎尾 (Then)**:
- Use 「應（該）」 to describe expected results
- Focus on verifiable business outcomes, not system internals
- Every Given condition must be reflected in a Then consequence (首尾呼應)

**蜂腰 (When)**:
- One Scenario = one When (one business event)
- Describe the event (What), not the steps (How)
- Concise enough to say in one sentence

**羊頭 (Given)**:
- Only business preconditions, not technical preconditions
- Use concrete data (real role names, amounts, dates), not abstract descriptions
- Use passive voice and past tense to separate context from action

### Gherkin Format

```gherkin
Feature: <Aggregate Name>

  # ← US-<BC>-<N>: <User Story title>
  # ← 情境 <X> 第 <N> 步
  Scenario: <KE-ID> <one-line behavior description>
    Given <business precondition, UL terms, concrete data>
    When <business event, maps to one Command>
    Then <verifiable business outcome, using 「應」>
```

**KE-ID format**: `KE-{Aggregate abbreviation}-{sequence}`, e.g., `KE-S-01`

**Scenario Outline**: use when multiple scenarios have identical structure but different parameters. Each Examples row must independently explain why the expected result differs.

### Coverage (Three-Color Labels)

- 🟢 Happy Path (normal success)
- 🟡 Edge Case (abnormal but legitimate)
- 🔴 Rejection (system must refuse)

Every Aggregate must cover all three colors. Do not exhaustively enumerate — find key examples that cover maximum range with minimum count.

### Language Rules

- All terms from UL table; one concept = one word
- No 「使用者」— use specific role or account name from DS
- No UI steps (click, select, navigate)
- No system internals (call API, write to DB, emit Domain Event)
- No HTTP responses (200 OK, 404 — those belong in §7)
- Then uses 「應」: 「結帳金額應為 309 元」
- Given uses past tense / passive: 「會員等級已設定為 VIP」
- When uses active voice: 「結帳時使用運費抵用券」

### Anti-Pattern Detection

| Anti-Pattern | Signal | Fix |
|-------------|--------|-----|
| 🐡 Pufferfish | When contains more than one action | Split into separate Scenarios |
| 🐛 Centipede | Multiple When-Then chains in one Scenario | Each business event gets its own Scenario |
| 🐨 Koala | No Then, or Then has no verifiable acceptance criterion | Add 「應 + verifiable result」 |

### Quality Checks

**Given Layering**: for each Given line, ask — if I remove this line, does the Then result change? Yes → keep (business precondition). No → remove (technical precondition, handled by automation layer).

**Connect-the-Dots**: number Given lines G1, G2… and Then lines T1, T2…. Draw connections. Unconnected G = unnecessary or Then is missing a verification. Unconnected T = result appears from nowhere.

**Self-Narration Test**: anyone reading this Scenario — without the author explaining — can understand what it describes.

**Three-Question Challenge**: continuously ask — Is the assumption correct? Are the results complete? What else?

### Key Examples → Test Code Discipline

- Test method name preserves Gherkin semantics: `KE_{Aggregate}_{ID}_{behavior_description}`
- Test body maps Given-When-Then → Arrange-Act-Assert
- After development begins, test code is the living version; Phase 3 spec is the baseline snapshot
- Requirement changes: add a failing Key Example test FIRST, then modify code. Do not modify Phase 3 spec (unless full re-design triggers a new Phase 3 cycle)

---

## Self-Check Clean List

Before presenting any spec section to the user, verify the draft passes the relevant checks. **Drafting-time discipline**: only run rules marked `⭐ MUST` before showing output. Rules marked `▢ Extended` are deferred to Phase 4 Review for the holistic pass — they're not skipped, just batched. This stops drafting from collapsing under 30+ rule cognitive load while still funneling everything through eventual review.

Each category below has specific Y/N rules. For each rule:
- **Pass condition**: explicit, machine-verifiable description of what passes
- **Fail signal**: concrete patterns in draft text that indicate failure
- **Fix action**: how to correct the failed rule before showing user

---

### Category 1: Aggregate Boundary Checks

Applies to: §3 Aggregate Definitions, §6 Application Layer
⭐ **MUST** at drafting: AB-1, AB-2. ▢ **Extended** (Phase 4): AB-3, AB-4.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| AB-1 | No Invariant references another Aggregate's internal state | Every Invariant phrase contains only the Aggregate's own field names + method names | Invariant text contains "and Booking is in Pending status" or "Session.Booking must be valid" | Move the cross-Aggregate check to §6 Application handler; rephrase Invariant to only this Aggregate's state |
| AB-2 | Aggregate methods do not load other Aggregates | Method signature lists only primitives + this Aggregate's VOs | Method has parameter like `Booking booking` or calls `_bookingRepo.Get(...)` | Pass an ID, not the Aggregate; load both in handler |
| AB-3 | Cross-Aggregate orchestration lives in §6 Handler with explicit rationale | §6 has a callout "intentionally violates single-Aggregate-per-tx; rationale: ..." | Cross-Aggregate handler exists but no rationale callout | Add callout with: business-atomicity reason + concurrency assessment + future re-evaluation trigger |
| AB-4 | "system" / "platform" / "the BC" never appears as actor | Actor names are concrete: User, Customer, Operator, AI Parser, specific service | Phrases like "the system creates a Booking" or "the platform records the event" | Replace with concrete actor: "ConfirmProposalHandler creates Booking" |

---

### Category 2: Invariant Layer Separation

Applies to: §3 Aggregate Invariants, §6 Application Handler responsibilities
⭐ **MUST** at drafting: IL-1, IL-3. ▢ **Extended** (Phase 4): IL-2, IL-4.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| IL-1 | Domain Invariants describe Aggregate-internal rules only | Invariant text mentions only this Aggregate's fields, methods, state transitions | "must be called by handler X", "during transaction Y", "after event Z is dispatched" | Rephrase as Aggregate rule (e.g., "Method M only callable when Status == ProposedToShop"); move caller-identity to §6 |
| IL-2 | Application transaction guarantees not written into Invariants | Invariants do not reference SaveChanges, transactions, dispatch order | "this happens in same transaction as ...", "before Phase 2 commits" | Move to §5.3 Dispatch Mechanism or §6.6 Save Boundary Rules |
| IL-3 | Sensitive-data exclusion expressed at multiple layers | §3 has `[SensitiveValue]` on Domain field, §6.3 Views exclude, §5.1 Events exclude | Sensitive field has only one layer of protection (e.g., only Domain mark, no DTO rule) | Add the missing layer; verify §9.2 reflection test covers it |
| IL-4 | Stub assumption stays out of Domain | Domain model handles full input space (e.g., Confidence = High/Medium/Low all routed) | Domain only has logic for "if Confidence == High" with no Medium/Low branches | Document all branches in Domain even if Stub never produces them |

---

### Category 3: Naming & UL Compliance

Applies to: §1 TL;DR, §3 Aggregates, §5 Events, §7 wire format
⭐ **MUST** at drafting: NU-1, NU-3, NU-6. ▢ **Extended** (Phase 4): NU-2, NU-4, NU-5.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| NU-1 | No doubled words in type names | Type names like `Proposal`, `Snapshot` (not `ProposedProposal`, `BookingBookingSnapshot`) | Type name contains the same root twice | Rename to single-root form |
| NU-2 | Inconsistent suffix variants caught | Same concept uses one consistent name across spec (e.g., always `Cost`, not mixing `Cost` / `CostAmount` / `InternalCost`) | Spec uses 2+ variants for the same concept | Pick one canonical name; replace others; add to UL table with "禁止用語" column |
| NU-3 | Technical terms not leaking into business names | Aggregate / VO names are business vocabulary (Customer, Booking) | Names like `BookingDTO`, `SessionEntity`, `IntentObject` | Strip technical suffix; entity-vs-VO is a structural choice, not a naming choice |
| NU-4 | Cross-BC term reuse explicitly resolved | If Tenant / Customer / Order appears in this BC, it's either Conformist (referenced from owning BC's UL) OR explicitly redefined with rationale | Term appears with no UL entry and no Conformist note | Add UL entry + source BC, OR redefine with "differs from {OtherBC}.{Term} because..." |
| NU-5 | Wire format names align with project convention | Wire format uses project's case convention (snake_case / camelCase) consistently | One field uses different case than others in same response | Pick project convention; apply uniformly; document in §7.3 |
| NU-6 | DEFERRED future behavior named with concrete slice | Every DEFERRED item has slice name (e.g., `outbox-pipeline`, `session-nlp-advanced`) | "Future", "TBD", "later" as target | Name a specific slice; if undecided, propose name + add TODO |

---

### Category 4: Cost / Sensitive Data Firewall

Applies to: §3 Aggregates, §5 Events, §6.3 Views, §9.2 Reflection tests
⭐ **MUST** at drafting: CS-1, CS-2, CS-4, CS-5 (security-critical, never deferred). ▢ **Extended** (Phase 4): CS-3, CS-6.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| CS-1 | Domain layer marks sensitive fields | Field has `[SensitiveValue]` attribute or equivalent project marker | Sensitive field (Cost, Salary, SSN, etc.) has no marker | Add the marker; verify §3 Invariants table notes the sensitivity |
| CS-2 | View DTOs exclude sensitive fields by name | No `*View` type has property whose name contains forbidden keywords | Property `CostAmount`, `cost`, `internal_price` in any View | Remove the property; verify projection in §6.3 doesn't accidentally include it |
| CS-3 | View DTOs exclude Domain VOs that contain sensitive fields | View properties are primitives + whitelisted VOs (e.g., Money) | View property has type `Proposal` (which contains Cost) | Replace VO type with explicit field projection in `new XxxView { ... }` |
| CS-4 | Domain Events exclude sensitive fields from payload | Event records' public properties contain no sensitive field names or types | Event payload has `Cost`, `CostAmount`, or VO containing them | Remove from payload; document in §5.1 "外發事件 payload 規則" note |
| CS-5 | Reflection tests enforce the firewall | §9.2 has explicit tests A/B/C/D scanning for sensitive fields in DTOs, VOs, Events | §9.2 mentions tests in prose but no concrete assertion logic | Add pseudo-code or assertion for each test; ensure CI gate fails on violation |
| CS-6 | Reporting / read-side path documented | If sensitive data is needed for reporting (e.g., profit calculation), document the legal read path | Sensitive data is only "in Domain", with no documented access path for legitimate readers | Document: which BC reads, via what mechanism (direct query? read-only ref?), why event-based path is not used |

---

### Category 5: Dispatch Layer Boundaries

Applies to: §5.3 Dispatch Mechanism, §6.6 Save Boundary Rules
⭐ **MUST** at drafting: DL-1, DL-3. ▢ **Extended** (Phase 4): DL-2, DL-4, DL-5, DL-6.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| DL-1 | Domain Aggregate doesn't depend on dispatch framework | Domain code references no MediatR / IPublisher / framework type | Aggregate imports `IMediator` or has `INotificationHandler` reference | Domain raises events to local `_domainEvents` collection only; dispatch happens in Infrastructure |
| DL-2 | 3-Phase pattern explicitly documented when used | §5.3 has table or diagram showing Phase 1 / 2 / 3 with tx scope and timing | §5.3 mentions "we publish events" without phase ordering | Add 3-Phase table; explicitly mark which phase is in original tx vs independent scope |
| DL-3 | One Application Command Handler = one SaveChangesAsync | §6.6 explicitly states this rule | §6 handlers have multiple `SaveChanges` calls in step lists | Refactor handler into single-save flow; if intermediate consistency needed, split into multiple handlers |
| DL-4 | Cross-Aggregate handlers share scoped DbContext | §6.6 documents shared DbContext via DI scope | Handler description mentions "load via SessionDbContext" + "save via BookingDbContext" | Use single scoped DbContext; one save covers all changes |
| DL-5 | MediatR / framework references confined to Application + Infrastructure | §8.1 directory tree shows MediatR types in Application/EventDispatch/ or Infrastructure | Domain layer has any MediatR import | Move to Application Layer; Domain only knows IPublisher abstraction (or even less) |
| DL-6 | Stub adapter behavior mimics real dispatch contract | Stub publishes real events / returns real types, not shortcuts | Stub returns `bool true` while real adapter would publish event | Stub publishes the actual event; downstream handler runs as in real flow |

---

### Category 6: Stub vs Domain Coupling

Applies to: §3 Aggregate methods, §4 Ports, §6 Handler logic
⭐ **MUST** at drafting: SD-1, SD-3. ▢ **Extended** (Phase 4): SD-2, SD-4.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| SD-1 | Domain handles full input space, not Stub-narrow space | Aggregate methods have branches for all values an enum / VO can take | `if (intent.Confidence == High)` with no else clause for Medium/Low | Add branches; in §6.1 handler, document what each branch does (even if MVP-Core stub never produces it) |
| SD-2 | Stub adapter behavior documented separately from Domain | §8.4 states what Stub returns, §3 states what Domain expects (full range) | Domain spec says "Confidence is always High" because stub returns High | Domain accepts full range; §8.4 documents stub's narrow behavior; future real adapter test will exercise the gap |
| SD-3 | Real-adapter migration plan exists | Each Stub has named slice for replacement | Stub exists with no replacement plan | Add target slice in §4 / §8.4 (e.g., `session-nlp-advanced` will replace StubNlpParser) |
| SD-4 | Stub doesn't shortcut downstream events | Stub adapter triggers same Domain Events / state transitions as real adapter would | Stub returns synchronously while real would publish event later | Stub publishes the real event (sync in MVP-Core); 2-step handler pattern in §6.4 unchanged when real adapter ships |

---

### Category 7: Open Question Discipline

Applies to: §10 Open Questions Resolution Log
⭐ **MUST** at drafting: OQ-1, OQ-2. ▢ **Extended** (Phase 4): OQ-3, OQ-4, OQ-5.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| OQ-1 | RESOLVED status truly resolves | Each RESOLVED row has explicit decision summary + chapter link | RESOLVED with empty 對應章節 OR vague summary like "decided to handle later" | Either move to DEFERRED with target slice OR write the actual decision and link the chapter where it lives |
| OQ-2 | DEFERRED has named target slice | Each DEFERRED entry names a slice (existing or proposed) | DEFERRED with "future" / "TBD" / "later" | Pick a slice name; if undecided, propose one + add TODO in §11 |
| OQ-3 | Empty section forbidden | If no questions, prose explains why none arose | §10 is just an empty table | Add rationale: which Phase 1/2 decisions covered this BC; what about this BC keeps it simple |
| OQ-4 | Question IDs stable across versions | Q1 stays Q1 even if revised; new questions in v1.1 get Q-v1.1-001 | Old questions renumbered after new ones added | Preserve original IDs; use version-prefixed IDs for new questions |
| OQ-5 | Cross-references to spec chapters complete | Every RESOLVED entry has `對應章節` filled in | RESOLVED row with empty chapter column | Fill in chapter where decision lives; if no chapter exists, fix the spec (the decision must be documented somewhere) |

---

### Category 8: SBE Key Example Quality

Applies to: §3 Aggregate Definitions (item 7: Key Examples)
⭐ **MUST** at drafting: KE-1, KE-2, KE-5. ▢ **Extended** (Phase 4): KE-3, KE-4, KE-6, KE-7, KE-8.

| # | Rule | Pass condition | Fail signal | Fix |
|---|------|---------------|-------------|-----|
| KE-1 | Every Scenario has exactly one When | Each Scenario block has one When line | Multiple When lines (🐡 Pufferfish) or When-Then chains (🐛 Centipede) | Split into separate Scenarios |
| KE-2 | Every Then uses 「應」 and is verifiable | Then describes observable business outcome | Then is vague, missing, or describes system internals (🐨 Koala) | Rewrite as 「應 + verifiable business result」 |
| KE-3 | Given contains only business preconditions | Removing any Given line changes the Then result | Given mentions DB, API, cache, framework, or technical setup | Remove technical Given; let automation layer handle it |
| KE-4 | Connect-the-dots passes | Every G has at least one T connection; every T has at least one G connection | Orphan G (unnecessary precondition) or orphan T (result from nowhere) | Remove orphan G; add missing G for orphan T |
| KE-5 | Three-color coverage per Aggregate | Each Aggregate has at least one 🟢 + one 🟡 + one 🔴 | Missing color | Add Key Example for missing color |
| KE-6 | All terms from UL | Every noun in Scenario appears in UL table | Term not in UL; or uses 「使用者」 instead of specific role | Add to UL or replace with UL term |
| KE-7 | Each Key Example traces to User Story + scenario | Comment line shows `US-{BC}-{N}` and `情境 {X} 第 {N} 步` | No traceability comment | Add traceability comment |
| KE-8 | Given uses concrete data | Given lines contain specific values (names, amounts, dates) | Abstract descriptions like 「某個會員」or「一些金額」 | Replace with concrete data |

---

### How to Use This List

When you finish drafting a section, before presenting:

1. **Run the categories that apply to that section** (table above lists "Applies to" per category)
2. **For each rule in those categories**, check pass condition against your draft
3. **For any failed rule**, apply the Fix before showing user
4. **Do not present partial passes** — either all rules in scope pass, or fix and re-check

When user raises a review finding that maps to one of these rules, add the finding's lesson to the rule's Fail signal column for future iterations.

---

## Common Review Findings (abstracted from real testing)

Findings the user is likely to raise during review. Each finding below is something a senior reviewer caught during real Phase 3 spec review of a complex BC. **Pre-empt them in your draft when applicable** — every finding has a mitigation pattern that, if followed during drafting, prevents the review finding from arising.

Each entry contains:
- **Finding**: how the reviewer phrased the problem
- **Why it matters**: the architectural cost of leaving it unfixed
- **Mitigation while drafting**: what to do during spec drafting to pre-empt this
- **Real example**: anonymized example from real BC testing
- **Self-Check rule reference**: which Stage 3 rule catches this

---

### Finding 1: Aggregate 越權 — Invariant 引用其他 Aggregate 內部狀態

**Why it matters**: Invariants written this way cannot be tested in Domain layer (need to load other Aggregate to verify). It also blurs the responsibility line — if Invariant fails, who's at fault, this Aggregate or the other one? Vernon IDDD p.126: "An Aggregate must never query another Aggregate's internal state."

**Mitigation while drafting**:
- For every Invariant, ask: "Does this Aggregate know this fact from its own fields alone?"
- If answer is no → move the rule out of Invariants table
- Express the cross-Aggregate guarantee at §6 Application handler level + each Aggregate's own internal Invariants
- Pass IDs (not Aggregate references) into methods

**Real example** (anonymized from booking domain):

❌ Wrong:
```
S-4: ProposedToShop status requires Booking to be created and in Pending state
```

✅ Right:
```
S-4: ConfirmProposal(bookingId) requires bookingId to be non-null and not previously set;
     post-state ProposedToShop
B-1: Booking creation sets Status = Pending (Booking's own Invariant guarantees this)
§6.1: ConfirmProposalHandler creates Booking and calls ConfirmProposal in same transaction;
      atomicity ensured by Application layer + each Aggregate's own Invariants combined
```

**Self-Check rule**: AB-1, AB-2, IL-1

---

### Finding 2: 狀態機顆粒度 — 合併兩個業務語義到一個狀態

**Why it matters**: A state with two business meanings hides bugs. Future feature requests that affect only one of the meanings cannot be expressed without splitting the state — splitting later costs migration + test rewrite + frontend changes.

**Mitigation while drafting**:
- For each state in the machine, write a one-sentence description of "what's happening from a business perspective"
- If the sentence has two clauses joined by "and" or "or", consider whether it's actually two states
- Ask: "What's the smallest user-observable difference that would force splitting this state?"
- If the answer exists and is plausible → split now (cost is one extra enum value vs future migration)
- Apply Decision Priority #4 (Team executability) only after rejecting #1 (Domain correctness): the architecturally faithful state machine wins over test-surface reduction unless test-surface cost is genuinely catastrophic

**Real example**: an early draft compressed 9 states into 5 by merging "collecting customer intent" and "presenting proposal awaiting confirmation" into one state called Collecting. Reviewer caught this — the merged state has two distinct business meanings (still gathering vs. waiting for confirmation), and frontend needs to show different UI for each. Splitting back into 6 states (Started / Collecting / Proposing / ProposedToShop / Confirmed / Closed) costs ~1 hour now vs ~3-4 hours if added later (migration + handler refactor + frontend).

**Self-Check rule**: NU-2, IL-1 (when the merge hides Application concerns)

---

### Finding 3: VO 命名疊字 / 同義反覆

**Why it matters**: Doubled-word names (ProposedProposal, BookingBookingSnapshot) signal that the modeler couldn't decide between two competing concepts. Readers waste effort distinguishing them. UL drift compounds when downstream code uses inconsistent abbreviations of the doubled name.

**Mitigation while drafting**:
- VO and Aggregate names must read naturally as a noun in business prose
- Test: write a sentence like "The session has a ___" — if the blank with the type name reads like jargon, rename
- Avoid filler suffixes: `Details`, `Info`, `Data`, `Object`, `Record`, `Entry`. These are rarely business vocabulary; they're modeler hesitation

**Real example**: original draft used `ProposedProposal` for the VO that captures pricing+provider+plan at proposal time. Reviewer caught the doubling. Alternatives considered:
- `Proposal` (chosen): natural noun, fits sentence "the session has a proposal"
- `ProposalDetails`: filler suffix, reviewer rejected
- `Offer`: cross-BC collision risk (Catalog/Pricing BCs use Offer for promotions)

Method signature became `PresentProposal(Proposal proposal, by)` — type and parameter share root, but lowercase parameter is fine convention.

**Self-Check rule**: NU-1, NU-3

---

### Finding 4: 跨層責任混淆 — Application 約定寫入 Domain Invariant

**Why it matters**: Mixing Application transaction concerns into Domain Invariants makes the Invariant unverifiable in pure Domain tests. It also locks Domain to a specific dispatch architecture, blocking future changes (e.g., synchronous → outbox).

**Mitigation while drafting**:
- For every Invariant, scan for these phrases: "must be called by", "during transaction", "after event X is dispatched", "in same scope as"
- Any of these indicate Application/Infrastructure leaking in
- Move to §6 Application handler responsibility OR §5.3 Dispatch Mechanism
- The Invariant should rephrase as a pure state-machine rule (e.g., "Method M only callable when Status == X")

**Real example** (anonymized from session domain):

❌ Wrong (original draft):
```
S-5: ProposedToShop → Confirmed must be triggered by ShopAvailabilityConfirmed event handler
```

✅ Right (after review):
```
S-5: ConfirmShopAvailability() callable only when Status == ProposedToShop; transitions to Confirmed
§6.4: ShopAvailabilityConfirmedHandler is the caller of ConfirmShopAvailability (Application convention)
```

**Self-Check rule**: IL-1, IL-2

---

### Finding 5: Stub 假設滲漏到 Domain

**Why it matters**: When MVP-Core uses a Stub adapter that always returns one value (e.g., `Confidence = High`), it's tempting to write Domain logic that handles only that case. When the real adapter ships and returns the full input space (Medium, Low, Failed), Domain is forced to refactor — and the refactor is in the most critical part of the BC.

**Mitigation while drafting**:
- For every Port that has a Stub: list the full input/output space the real adapter will produce (not just what the Stub returns)
- Domain methods must handle the full space, even if some branches "never execute" in MVP-Core
- §6 handler routes inputs based on the full space; document each branch even if one branch is the only one Stub triggers
- §8.4 documents what Stub returns; this narrowness is intentional (for MVP focus) but does not narrow Domain

**Real example**: original `SubmitCustomerUtteranceCommand` description said "if confidence is sufficient, present proposal" — without specifying what happens for Medium or Low. Stub always returned High, so MVP-Core would never hit the gap. Reviewer caught this — the Domain needs to handle:
- High → PresentProposal
- Medium → stay in Collecting, await next utterance
- Low → stay in Collecting + log (future: HumanReviewRequested in PendingReview slice)

All three branches now in §6.1, even though MVP-Core only exercises High.

**Self-Check rule**: SD-1, SD-2, IL-4

---

### Finding 6: DTO Cost 外流防線單薄

**Why it matters**: Relying on JsonIgnore attributes or serializer config to exclude sensitive fields is fragile. Switching serializer, framework upgrade, or accidental property addition silently exposes sensitive data. Audit trails after a leak don't recover the leaked data — prevention is the only effective control.

**Mitigation while drafting**:
- Three layers of defense for every sensitive field, all required:
  1. **Domain layer**: `[SensitiveValue]` attribute or equivalent project marker on the field
  2. **Application layer**: Views use explicit `new XxxView { ... }` projection — no auto-map, no JsonIgnore reliance, no VO-as-property-type
  3. **CI gate**: Reflection tests scan all `*View` types and Domain Event records for forbidden field names + forbidden VO types
- Document the legitimate read path for sensitive data (e.g., Reporting BC reads via direct query to specific tables, never via events)

**Real example**: the BookingSnapshot VO contains both PaidAmount and CostAmount. Original spec said "Views exclude Cost" but had no test enforcing it. Reviewer required:
- Test A: scan `*View` types for property names containing "cost" → fail build
- Test B: scan `*View` types for properties whose type is a Domain VO (whitelist Money) → fail build
- Test C: scan all Domain Event records for cost-related properties → fail build

Three-layer defense: Domain `[SensitiveValue]`, Views explicit projection, CI reflection tests. Any single layer can be bypassed; all three together hold.

**Self-Check rule**: CS-1, CS-2, CS-3, CS-4, CS-5

---

### Finding 7: Event payload 敏感資料

**Why it matters**: Even if the BC's own subscribers never persist event payloads, future cross-BC subscribers may (outbox, audit, external integration). Sensitive fields in event payloads have a long blast radius — any subscriber can leak them, including subscribers that don't yet exist.

**Mitigation while drafting**:
- §5.1 Event Catalog: every event's payload column lists fields explicitly
- For each field, ask: "If this event is persisted forever in an outbox / audit log / external system, is this field's exposure acceptable?"
- Sensitive fields: omit from payload entirely (subscribers fetch from authoritative source if needed)
- Document the omission as a §5.1 note: "外發事件 payload 規則: 所有 event payload 不含 X / Y / Z 欄"

**Real example**: original `ServiceCompleted` event payload included CostAmount. Reviewer flagged it — even though MVP-Core has no subscriber, future Reporting BC subscriber might persist the event. Solution:
- ServiceCompleted payload: `{ SessionId, BookingId, PaidAmount, At }` — Cost stripped
- Reporting BC reads Cost via direct query to Booking.Snapshot (read-only ref to Domain), not via event subscription
- Documented in §5.1 + §6.3 + §13 (Reporting integration note)

**Self-Check rule**: CS-4, CS-6

---

### Finding 8: 跨 BC integration event 無訂閱者時的處理策略

**Why it matters**: MVP-Core often ships before downstream BCs that would subscribe to integration events. The choice of "fire and forget vs defer" affects future migration cost. Wrong choice early → either silent event loss or accumulated outbox debt.

**Mitigation while drafting**:
- §5.2 explicitly identifies which events are integration events (cross-BC) vs intra-BC events
- For each integration event: state current dispatch (typically: MediatR in-process, no subscribers yet) and future migration (e.g., outbox pipeline)
- Default policy when no subscriber exists: dispatch normally, log eventId + eventType, no outbox storage. This keeps the publish-side code identical between MVP-Core and post-outbox state.
- Document the migration trigger: "When outbox pipeline ships at slice X, these events move to IIntegrationEvent + outbox persistence; Domain unchanged, Application Layer publish adapter updated"

**Real example**: 7 events in Session BC are intended for cross-BC consumption (SessionStarted, ProposalPresented, etc.). MVP-Core has zero subscribers. Decision documented in §5.2: "fire normally + log; no outbox until pipeline slice. Migration touches only Application Layer adapter, Domain unchanged."

**Self-Check rule**: DL-2, OQ-2

---

### Finding 9: 2-step handler atomicity — 跨 Aggregate transaction boundary

**Why it matters**: When a synchronous handler needs to update two Aggregates atomically (e.g., create Booking + advance Session state), the choice between "single tx" and "two-step async" affects:
- MVP-Core: stub returns sync, single tx feels natural
- Future: real adapter is async (e.g., webhook), forcing two-step
- Switching from sync to async mid-implementation is expensive — handler refactor + race condition handling

**Mitigation while drafting**:
- Design handlers as if real adapter is already async, even when MVP-Core stub is sync
- Use 2-step pattern: Handler 1 advances state to "waiting" + triggers external request; Handler 2 (event handler) processes external response + advances state to final
- Stub adapter publishes the response event immediately (sync via in-process mediator) — same handler code path runs in both MVP-Core and real adapter scenarios
- Document the tx boundary: handler 1 has its own tx, handler 2 has its own tx; race conditions during the gap (microseconds in MVP-Core, minutes in async) are guarded by Aggregate Invariants (e.g., terminal-state-no-transition rule)

**Real example**: ConfirmProposal is a 2-step handler:
- Step 1 (ConfirmProposalHandler): create Booking, advance Session to ProposedToShop, call IShopAvailabilityClient.RequestConfirmationAsync (does NOT wait for completion)
- Step 2 (ShopAvailabilityConfirmedHandler): runs when ShopAvailabilityConfirmed event fires; advance Session to Confirmed + Booking to Confirmed
- StubShopAvailability publishes the event immediately (in-process MediatR) — Step 2 runs ~microseconds after Step 1 in MVP-Core
- Real Channel BC adapter sends Telegram message + waits for response webhook — Step 2 runs ~minutes/hours after Step 1
- Aggregate S-7 (terminal state immutable) guards against customer Cancel between steps

**Self-Check rule**: DL-2, AB-3 (cross-Aggregate orchestration with rationale)

---

### Finding 10: Naming collision across BCs

**Why it matters**: Cross-BC reference rules (e.g., "Session.Application cannot reference Identity.Application") create naming pressure when multiple BCs need similar infrastructure interfaces. Three options exist: shared interface (often forbidden), parallel same-name interfaces (subtle), differently-named interfaces (verbose).

**Mitigation while drafting**:
- Identify which interfaces this BC needs from infrastructure (multi-tenancy, current user, time, logging, configuration)
- For each: check if other BCs already defined a same-purpose interface
- Three resolution paths:
  - **Shared via SharedKernel**: only when the interface is truly cross-BC reusable AND project rules allow SharedKernel to hold it (rare for BC-Application interfaces)
  - **Parallel same-name in different namespaces**: cleanest for type system, costs IDE/alias overhead at integration points (Host)
  - **Differently-named per BC**: avoids alias overhead, costs minor name divergence
- Document choice + rationale in spec; future BC authors follow the established pattern

**Real example**: Identity BC defined `ICurrentTenantProvider` early. Session BC needed the same capability. Three options considered:
- Shared in SharedKernel: rejected, project rule forbids cross-BC Application reference and SharedKernel should hold primitives only
- Parallel same-name `ICurrentTenantProvider` in both Identity.Application.Interfaces and Session.Application.Interfaces: chosen initially, but caused alias-heavy Host code
- After review, renamed Session's version to `ITenantScope` / `ICustomerScope`: shorter, no alias needed in Host, signals BC-specific intent. Identity's `ICurrentTenantProvider` left unchanged (avoid touching shipped BC) but flagged as future rename candidate.

**Self-Check rule**: NU-4

---

### How to Use This List

When drafting any spec section, scan the relevant findings before producing output:

| Section | Most relevant findings |
|---------|----------------------|
| §3 Aggregates | 1 (越權), 2 (狀態機), 3 (命名), 4 (跨層) |
| §5 Events | 6 (Cost 外流), 7 (event payload), 8 (integration event) |
| §6 Application | 4 (跨層), 5 (Stub 假設), 9 (2-step handler) |
| §6.3 Views | 6 (Cost 外流) |
| §8 Infrastructure | 10 (naming collision) |
| §10 Open Questions | (none specifically — Stage 3 OQ rules cover this) |

When user raises a finding NOT in this list during real review, add it after the session ends. This list grows from real findings, not theoretical ones.

---

## Claude's Proactive Mechanisms

Behaviors Claude executes during Phase 3 drafting (not error checks — positive behaviors).

### UL Carry-over

Read existing UL from prior BCs' rules + CLAUDE.md before drafting. New BC's terminology must be compatible; flag conflicts explicitly. Do not invent terms when existing terms apply.

### Cross-Phase Consistency Check

While drafting Phase 3, periodically verify against Phase 2 decisions (Context Map, AI-ADRs, deployment). If a Phase 3 decision contradicts Phase 2 → pause, flag, ask user whether Phase 2 needs revision or Phase 3 should align.

### Deferred Question Tracking

When encountering a question that's better answered in implementation than in spec, do not block. Tag as Q-N + DEFERRED → suggest slice name where it'll be addressed.

### Implementation Rules Generation

After spec is finalized (v1.x stable), generate `.claude/rules/{bc}.md` as a reference card for bc-developer agents. Content: condensed UL + Aggregates + Invariants + Cost firewall + Dispatch mechanism + DEFERRED reminders.

---

## Slice Decomposition Pattern

Phase 3 spec implies an implementation order. The default 5-slice pattern fits a typical BC with Domain + Application + Infrastructure + HTTP + UI layers. BCs with different characteristics need different decomposition.

This section covers:
1. The default 5-slice pattern (when to use it)
2. Five common variant patterns (when to deviate)
3. Per-slice acceptance criteria
4. Trigger conditions for re-evaluating slice plan mid-implementation

---

### Default 5-Slice Pattern (Type A: Standard Customer-Facing BC)

**Use when**: BC has end-user UI, HTTP endpoints, database persistence, AI/external integrations via Stubs, multi-tenant requirements.

| Slice | Scope | Tests Expected | Done Definition |
|-------|-------|----------------|-----------------|
| MVP1 | Domain skeleton: Aggregates + VOs + Invariants + Domain Event types + `_domainEvents` collection on Aggregate Root | Domain unit tests only; no MediatR / persistence | All Invariants tested with success + failure cases; state machine 100% coverage |
| MVP2 | Application + Infrastructure: Handlers + Ports + DbContext (with 3-Phase override) + Stub Adapters + Cost firewall reflection tests | + Application + Infrastructure tests | Reflection tests A/B/C/D green; all command handlers full-flow tested with stub Ports |
| MVP3 | Persistence: Migrations + RLS rules + integration tests with testcontainers + init SQL | + Integration tests + RLS negative tests | Cross-tenant queries return 0 rows; all RLS-protected tables verified |
| MVP4 | HTTP Endpoints + auth wiring + E2E | + E2E tests + cross-tenant API negative tests | Full happy path E2E green; Cancel race + Cost-not-in-response asserted |
| MVP5 | UI integration: replace UI mocks with real API + carryover decisions | + UI tests | Frontend connects to real backend; previously mocked screens use real data |

---

### Variant Patterns

#### Variant B: Background Worker BC (no UI, no HTTP for end users)

**Use when**: BC primarily processes events / scheduled tasks (e.g., notification delivery, scheduled reports). May have admin endpoints but no end-user UI.

**Differences from default**:

| Slice | Scope changes |
|-------|---------------|
| MVP1 | Same as default |
| MVP2 | Same as default; Stubs typically include external service clients (SMS, email, push) |
| MVP3 | Same as default; integration tests heavier on event handlers |
| MVP4 | **Reduced**: only admin endpoints (no customer-facing). E2E tests focus on event-trigger paths, not HTTP request paths |
| MVP5 | **Skipped or replaced**: instead of UI integration, MVP5 = scheduling/dispatch wiring (cron, hosted service, queue listener) |

**Document the variant**: §11 of spec states "MVP5 substituted with scheduler integration; no UI in this BC."

---

#### Variant C: Pure Integration BC (cross-BC ACL hub)

**Use when**: BC's primary responsibility is translating between other BCs (e.g., Channel Gateway BC translating Telegram messages into Session events). Few or no Aggregates, mostly Adapters and event handlers.

**Differences from default**:

| Slice | Scope changes |
|-------|---------------|
| MVP1 | **Reduced**: minimal Aggregates (often just one or zero). Most logic is in Adapters, deferred to MVP2 |
| MVP2 | **Expanded**: heavy Adapter implementations + ACL translation logic + Event handlers from upstream BCs |
| MVP3 | **Often skipped or merged**: this BC may not own persistent state (translates and forwards). If no own tables, MVP3 = configuration only |
| MVP4 | **Webhook receivers + outbound calls**: not customer-facing endpoints; integration endpoints from external systems (e.g., Telegram webhook). E2E tests use mocked external services |
| MVP5 | **Replaced**: cross-BC integration tests with real downstream BCs (e.g., Channel Gateway → Session BC end-to-end test) |

**Document the variant**: §11 explicitly notes "ACL/Integration BC: minimal Domain, expanded Infrastructure. MVP3 may be skipped if no own persistence."

---

#### Variant D: Reporting / Read-Side BC

**Use when**: BC consumes events from other BCs and produces read models / dashboards. Read-only with respect to source-of-truth state. May have its own denormalized tables for query performance.

**Differences from default**:

| Slice | Scope changes |
|-------|---------------|
| MVP1 | **Substantially reduced**: read-side projections are not Aggregates. May skip or replace with "read model schema design" |
| MVP2 | **Replaced**: event subscribers + projection logic (write to denormalized tables) + query handlers |
| MVP3 | Standard: migrations + RLS + integration tests (read-side tables still need RLS) |
| MVP4 | Endpoints serve dashboards / API for analytics consumers |
| MVP5 | Standard UI integration (or none if BC serves only API) |

**Document the variant**: §11 explicitly notes "Read-side BC: no Aggregates. MVP1 = projection schema; MVP2 = event subscribers + projections."

**Cross-BC reference rule**: Reporting BC may have privileged read access to other BCs' Aggregates (e.g., direct query for sensitive fields like Cost that don't flow through events). Document this access pattern in §13 (or equivalent integration section).

---

#### Variant E: Identity / Foundation BC

**Use when**: BC provides primitives other BCs depend on (authentication, tenant management, user/role resolution). Usually first BC implemented in a project.

**Differences from default**:

| Slice | Scope changes |
|-------|---------------|
| MVP1 | Standard, but Aggregates often simpler (User, Tenant, Role) |
| MVP2 | Standard, but **adds publishing of foundational interfaces** (e.g., `ICurrentTenantProvider`, `ITokenService`) that other BCs will consume |
| MVP3 | Standard; RLS rules must be especially robust as they're the model for other BCs |
| MVP4 | Standard; auth endpoints (login, refresh, logout) plus admin endpoints |
| MVP5 | Standard UI; admin SPA for tenant/user management |

**Special considerations**:
- This BC ships first; subsequent BCs depend on its conventions (RLS variable name, JWT claim format, error code shape)
- Document conventions explicitly in `.claude/rules/` to lock them down for downstream BCs
- Naming choices here propagate (e.g., choosing `ICurrentTenantProvider` here forces the cross-BC naming question for subsequent BCs)

---

#### Variant F: Greenfield with Heavy Domain (BC-first, infrastructure-light)

**Use when**: BC has rich Domain logic but minimal infrastructure needs (e.g., a pricing engine BC with complex rules but simple persistence).

**Differences from default**:

| Slice | Scope changes |
|-------|---------------|
| MVP1 | **Expanded**: Aggregates may have many Invariants and methods; this slice may need 2x time vs default |
| MVP2 | Standard, but Stubs may be simpler (fewer external integrations) |
| MVP3 | **Reduced**: simple persistence; integration tests focus on Domain edge cases not infra concerns |
| MVP4 | **Reduced**: API surface may be small (one or two endpoints exposing the engine) |
| MVP5 | Often skipped or minimal UI |

**Document the variant**: §11 notes "Domain-heavy BC: MVP1 expanded; later slices compressed."

---

### Per-Slice Acceptance Criteria (applies to all variants)

Each slice must pass:

- All previous slices' tests still green
- New tests for this slice's scope green
- Static analysis / linting clean
- BC boundary check (e.g., `scripts/check-bc-boundaries.sh`) — verifies this BC didn't reach into another BC's internals
- Commit message convention: `feat({bc}): ... — MVP{n}`
- Spec Self-Check Clean List rules (§Self-Check) all green for content delivered this slice

---

### Mid-Implementation Slice Re-evaluation Triggers

A slice plan in §11 of the spec is a forecast, not a contract. Re-evaluate the plan when:

| Trigger | Action |
|---------|--------|
| **MVP1 Domain tests fail to express an Invariant cleanly** | Pause; review Aggregate boundaries (may need to split or merge); update §3 spec |
| **MVP2 reflection tests catch broad sensitive-data leakage** | Pause; audit §6.3 and §5.1; may need to redesign View / Event payload schema |
| **MVP3 RLS integration tests fail with non-trivial frequency** | RLS rules may have drift from spec; align spec ↔ migration ↔ test, then resume |
| **MVP4 E2E tests reveal gap in handler logic** | Treat as spec gap; add §6.5 race row; fix handler; do NOT continue MVP4 with known gap |
| **A slice runs significantly longer than estimate** | If 2x estimate: pause and assess whether scope was misjudged. If 3x+: spec may have hidden complexity; break slice into sub-slices |
| **Cross-BC dependency emerges** | If the BC suddenly needs another BC that's not yet built: re-order slices to include dependency-stub layer first |

---

### How to Choose a Variant

When drafting §11 of the spec, ask:

1. **Does this BC have end-user UI?** No → Variant B
2. **Is this BC primarily an ACL/translator between other BCs?** Yes → Variant C
3. **Is this BC read-side with no source-of-truth state?** Yes → Variant D
4. **Is this the first BC in the project providing foundational services?** Yes → Variant E
5. **Is the Domain logic the heaviest part with minimal infra?** Yes → Variant F
6. **None of the above** → Variant A (default)

If multiple apply (e.g., a Reporting BC that's also Variant D *and* read-side), document the combination in §11 with rationale.

---

## Output Pacing

Phase 3 spec is long. Apply these rules:

- **Outline first**: produce §1-§11 headers + 1-line purpose per section, confirm with user before drafting any §
- **Section-by-section**: draft one § at a time, confirm before next
- **Aggregate-by-aggregate within §3**: if multiple Aggregates, draft one fully before next
- **Decision-batching for review**: when user raises review findings, apply P0 / P1 / P2 severity layering (P0 blocks next slice, P1 must resolve before slice opens, P2 clarify-only)

---

## Phase 3 Output Checklist

Phase progress is derived from `{coach_output_root}/{bc}/spec.md` existence; spec stability is derived from the spec's own frontmatter `status` field (see §1 Format notes). Do NOT mirror status, output paths, aggregate/invariant counts, or other summaries to `arch-state.md`.

When this BC's Phase 3 reaches v1.x stable (no more spec-structure changes expected):

- Update `{coach_output_root}/{bc}/spec.md` frontmatter `status: v1.x` (until then, keep `status: draft`). This is the signal that the spec is ready to serve as a `template-from-prior` template for subsequent BCs.
- Update `arch-state.md` `last_touched: { bc: <BC>, phase: phase_3, at: <today> }`. This is the personal cursor only — when the user moves to the next BC or to Phase 4, they will invoke that command explicitly and the cursor will update on entry. Do not pre-write a future intent.

Per-BC summary data (aggregates + invariant counts + events emitted + repository count + integration count + test scenario count) lives in `spec.md` itself, where it is the canonical source — not duplicated to arch-state.

Append to `arch-learnings.md`:

- Any new learnings (review findings, decisions): `source: phase_3` + `applies_to: phase_3` or `specific_bc:<n>`
- §10 DEFERRED items that have cross-BC implications (not ones scoped to this BC's slices) — they belong in cross-phase open questions
