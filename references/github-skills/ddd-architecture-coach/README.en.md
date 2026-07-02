# DDD Architecture Coach

> 繁體中文版：[README.md](README.md)

A Claude Code skill that helps you make architecture decisions for DDD projects. The core is planning — producing design documents and decision records; bundled is a bc-developer subagent you can optionally use to land implementation from the spec.

## What It Does

Guides you through four phases of architecture planning:

1. **Domain Discovery** — structured scenario modeling, event/command extraction, BC delineation, Touchpoint Map (surface × actor × co-presence), User Stories
2. **Architecture Design** — Context Map, per-BC architecture decisions, cloud deployment blueprint, AI-ADRs
3. **Implementation Spec** — Aggregate design, Key Examples (Gherkin/SBE), layered responsibilities, test specs, CI/CD
4. **Review & Iterate** — five health checklists (DDD / AI / engineering / cloud / SBE)

Output is organized in a BC-centric file structure. Each BC independently progresses through discovery → design → spec, enabling incremental development.

**Parallel team development**: per-BC commands (`/phase-2 <BC>`, `/phase-3 <BC>`) take BC name as a required argument, and artifacts are written under each BC's own `{coach_output_root}/{bc}/` path. Multiple developers can each own a different BC and run their own phases simultaneously without contending on a shared state file. `.claude/arch-state.md` is a personal cursor (added to `.gitignore` at bootstrap); team-shared knowledge lives in `.claude/arch-learnings.md` (append-only, safe to commit).

## Skill Components

This skill ships two responsibility-separated components sharing one DDD discipline:

| Component | Role | Location |
|-----------|------|----------|
| **Architecture Coach** (core) | Plans, drafts design docs and decisions | `SKILL.md` + `references/` |
| **bc-developer subagent** | Lands the spec as code while enforcing DDD discipline | `assets/agents/bc-developer.md` (copied to `.claude/agents/` at Bootstrap) |

The hand-off contract is `{coach_output_root}/{bc}/spec.md`: the coach embeds DDD discipline into the spec in Phase 3 (Aggregate boundaries, UL, Invariants, Key Examples, cross-BC event communication…), and **bc-developer reads the spec and ensures that discipline actually lives in the code** — enforced layered architecture (Domain Layer with zero external dependencies), Aggregate-centric vertical slices, UL-driven naming, cross-BC communication only via Domain Events, tenant isolation, three-color SBE coverage, commit granularity of one Invariant per TDD cycle, and halt-and-report on spec ambiguity rather than guessing.

It is not merely a "TDD subagent" — TDD is its workflow, but landing DDD discipline is its responsibility.

You can skip bc-developer and hand the spec to your human team or other tools — the coach doesn't depend on a specific downstream — but skipping it means giving up this enforcement layer.

### If You Don't Want bc-developer

bc-developer is a passive subagent — it doesn't run unless invoked; sitting in `.claude/agents/` has no side effect. To remove it entirely: `rm .claude/agents/bc-developer.md`.

Without bc-developer, here are the artifacts you can hand to your downstream of choice:

| Artifact | Purpose | Location |
|----------|---------|----------|
| `{coach_output_root}/{bc}/spec.md` | Canonical Phase 3 spec (Aggregates, Invariants, Key Examples, Ports…) | e.g. `docs/ddd/{bc}/spec.md` |
| `.claude/rules/{bc}.md` | Rules card: UL + Aggregate top level + firewall + dispatch summary | `.claude/rules/{bc}.md` |
| `CLAUDE.md` | Project tech-stack rules | repo root |

Three downstream paths:

1. **Hand to a human team** — read spec + rules card; DDD discipline enforced via code review
2. **Feed to another code agent** (Cursor / Copilot / a Claude Code instance without this skill) — `spec.md` is stack-agnostic and works directly as prompt context
3. **Use `assets/agents/bc-developer.md` as a checklist** (read its Constraints section, don't invoke) — apply the DDD discipline manually or through your own workflow

Trade-off: DDD discipline becomes your responsibility. None of these paths give you automated spec-ambiguity halts or three-color SBE enforcement. Reading bc-developer.md as a checklist is a low-cost compensator.

## What It Is Not

- **The coach itself doesn't write code.** SKILL.md and the four-phase workflow only produce design documents and decision records. The bundled **bc-developer** subagent (or your own workflow) takes implementation responsibility, reading the Phase 3 `spec.md` and writing code while enforcing DDD discipline. The coach and bc-developer are joined by `spec.md` — separated by responsibility, unified by discipline.
- **Not a Domain Storytelling or Event Storming workshop.** Phase 1 borrows vocabulary and formats from DS and ES, but Claude drafting artifacts is fundamentally different from a facilitated workshop with domain experts.
- **Not a replacement for team discussion.** Key Examples produced by the coach are labeled as AI-drafted baselines. Final sign-off should involve cross-functional review.

## Who It's For

- Developers and architects who want structured DDD architecture planning with Claude Code
- Solo developers or small teams building domain-driven systems who need an architecture thinking partner
- Projects considering AI/LLM integration and needing systematic intervention design with fallback planning

## Key Design Decisions

**Decision priority** (when tradeoffs conflict):
1. Domain correctness > technical elegance
2. Fallback completeness > AI feature richness
3. Verifiability > extensibility
4. Team executability > architectural ideal

**AI is not the default.** Every AI proposal must answer: why AI? what's the fallback? how to verify? A two-tier veto applies — one Hard condition (financial/legal harm without human-in-the-loop) rejects AI unconditionally; three Soft conditions (deterministic alternative reaches 95%+, fallback cost/latency comparable, no golden dataset) presume rejection unless explicitly overridden with documented rationale.

**Touchpoint Map closes a DDD discovery gap.** DDD's classic toolkit (Domain Storytelling, Event Storming, User Stories) is single-driver-biased — it captures "who did what" but not "who else is watching at the same time". Phase 1 Step 5 enumerates every UI/channel surface (web, admin console, Telegram, SMS, agent console, audit viewer…) × primary actor × **secondary observers** × **freshness budget**, requiring at least one cross-surface co-presence scenario. Example: a User Story like "customer chats with AI" rarely makes "agent console must mirror the conversation within 1 second" naturally visible — Touchpoint Map forces such cross-surface NFRs to surface explicitly during Phase 1 for discussion, instead of letting them slip until development or testing. The original concept comes from Service Blueprint (Shostack, 1984); we keep it lightweight — touchpoint × actor only, dropping heavier elements harder to reproduce in a skill (journey / emotion / metrics swim lanes).

**Specification by Example (SBE)** is built into Phase 3. Key Examples in Gherkin format serve simultaneously as spec, test case, and documentation.

## File Structure

```
ddd-architecture-coach/
├── SKILL.md                              # Master instruction
├── references/                            # Methodology docs the coach reads itself
│   ├── phase1-domain-discovery.md         # Scenario Modeling + Event/Command Extraction
│   ├── phase2-architecture-design.md      # Context Map + BC decisions + deployment
│   ├── phase3-implementation-spec.md      # Aggregate design + SBE + test specs
│   └── phase4-review-iterate.md           # Health checklists
└── assets/                                # Bundled templates Bootstrap copies into your project
    ├── agents/
    │   └── bc-developer.md                # DDD discipline enforcer subagent (TDD workflow, stack-agnostic, model configurable)
    ├── commands/
    │   ├── arch-coach.md                  # Default entry; reads state, picks phase
    │   ├── phase-1.md ... phase-4.md      # Force entry into a specific phase
    │   └── arch-learn.md                  # Append a learning
    └── templates/
        ├── project-context-template.md    # Project description, tech stack, model choice
        ├── arch-state-template.md         # Personal cursor (gitignored): last_touched.{bc, phase, at} (phase progress is derived from docs/ddd)
        └── arch-learnings-template.md     # Coach maintains: learnings + open questions (append-only)
```

`references/` holds the methodology docs the coach reads internally; `assets/` holds the bundled files that Bootstrap copies into your project's `.claude/` directory on first run.

## Project Output Structure

The coach produces artifacts in a BC-centric layout under `coach_output_root` (asked during the first Bootstrap; default `docs/ddd/`; can be changed to `docs/architecture/`, `docs/`, `packages/foo/docs/ddd/`, etc.):

```
{coach_output_root}/
  system/
    domain-stories.md          # Scenarios + event/command timeline (cross-BC)
    context-map.md             # BC classification, relationships, deployment
    touchpoints.md             # All UI/channel surfaces × actors × co-presence + derived integration patterns
  {bc}/
    discovery.md               # BC-local events, User Stories, UL, AI opportunities
    decisions.md               # Architecture decisions, AI-ADRs
    spec.md                    # Implementation specification (canonical contract for bc-developer)
```

Additionally, once the spec stabilizes (v1.x), Phase 3 produces an implementation rules card at the project root:

```
.claude/
  rules/
    {bc}.md                    # UL + Aggregate top level + firewall + dispatch + DEFERRED (condensed from spec)
```

The rules card is a quick-reference of the spec for bc-developer or any other downstream consumer; on conflict, `spec.md` wins.

> This skill only writes to the file system. Teams using Confluence / Notion / wiki must sync separately.

## Installation

This skill follows the [Agent Skills open specification](https://agentskills.io/specification) and works with Claude Code, OpenAI Codex CLI, and any compatible client.

### Claude Code

Pick a scope:

**User-level (available in every project)**:
```bash
mkdir -p ~/.claude/skills
cd ~/.claude/skills
git clone https://github.com/jed1978/ddd-architecture-coach.git
```

**Project-level (only active in that project)**:
```bash
mkdir -p .claude/skills
cd .claude/skills
git clone https://github.com/jed1978/ddd-architecture-coach.git
```

Claude Code loads the skill on its next start.

### OpenAI Codex CLI

```bash
mkdir -p ~/.codex/skills
cd ~/.codex/skills
git clone https://github.com/jed1978/ddd-architecture-coach.git
```

### Other compatible clients

The skill folder must be named `ddd-architecture-coach` (matching the `name` field in SKILL.md). Place the folder in your client's skills directory (consult the client's documentation for the path).

### Verify

Open a new conversation and run `/arch-coach`, or simply describe your project to Claude. The skill activates and runs its Bootstrap flow (four short questions).

### Upgrade

```bash
cd <install-path>/ddd-architecture-coach
git pull
```

## Getting Started

1. Install the skill in your Claude Code project (see Installation above)
2. Run `/arch-coach` (or just describe your project to Claude)
3. **Bootstrap is conversational**: Claude asks four short questions (one-line product description, main tech stack, team size, output root `coach_output_root`), then drafts `.claude/project-context.md` for you to review — no blank YAML to fill
4. Bootstrap also copies `arch-state.md`, `arch-learnings.md`, the bc-developer agent, and the slash command files into `.claude/`
5. Claude asks which model the **bc-developer subagent** (the spec-to-code enforcement layer; see "Skill Components" above) should use (default Sonnet 4.6; Haiku 4.5 for fast routine TDD; Opus 4.7 for deep reasoning)
6. Phase 1 starts: you describe behaviors, Claude produces the initial domain model — you review and challenge, never fill from scratch

## Requirements

- Claude Code
- A product/project description (3-5 sentences minimum)
- Willingness to challenge Claude's architectural decisions

## Origin

Built and validated on a real multi-tenant SaaS project (AI-assisted booking + scheduling). The skill's rules, self-checks, and review findings emerged from iterative development across multiple Bounded Contexts and 1000+ tests.

## License

MIT
