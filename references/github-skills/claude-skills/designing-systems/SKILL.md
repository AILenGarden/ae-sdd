---
name: designing-systems
description: >
  Use when asked to build a feature or service touching service boundaries,
  database schemas, public APIs, or 3+ files — or when the request arrives as
  a solution ("create a new microservice for X", "add a Kafka topic") rather
  than a problem. Also use when tempted to code while requirements are vague,
  when choosing between extending an existing service and creating a new one,
  when picking sync REST vs async events, or when tdd-java's circuit-breaker
  says the design is fighting you.
---

# Designing Systems

Core principle: the most expensive code is code built on an unexamined design — every line written before the decision multiplies the cost of changing it.

```
THE IRON LAW

NO IMPLEMENTATION BEFORE AN APPROVED DESIGN
for any change touching service boundaries, schemas,
public APIs, or 3+ files. "Approved" means the user
said yes to a written design — not silence, not momentum.
```

Below that threshold (single-file fix, internal refactor), state that the change is below the design gate and proceed with `tdd-java`.

## Phase 1: Frame the problem

1. Restate the problem in your own words — the *problem*, not the requested solution. If the request arrived as a solution ("add a microservice"), back up to the need behind it.
2. List constraints: load, latency, consistency requirements, deadlines, team/stack boundaries.
3. **Identify what already exists — before proposing anything new.** Search the codebase, internal libraries, existing services and endpoints, prior decisions. Reuse beats rebuild. **GATE: name what you found and what you ruled out.** "Nothing exists" is a claim that requires a search, not an assumption.

## Phase 2: Propose 2–3 approaches with explicit tradeoffs

For EACH approach state, in one block:

- **What you gain** — concretely, not "cleaner"
- **What you trade away** — every approach trades something; if you can't name it, you haven't understood the approach
- **Failure modes** — what breaks, how you'd notice, blast radius

One approach is not a design; it's a foregone conclusion wearing a costume. Include the boring option (extend what exists) even when you prefer the exciting one.

### Microservice vs monolith framework

**Default: modular monolith or extend the existing service.** Split into a new service only on a *proven* trigger:

| Valid trigger | Test |
|---|---|
| Independent scaling need | Measured load profile differs by an order of magnitude, not "might grow" |
| Independent deploy cadence | The pieces genuinely ship on conflicting schedules today |
| Team ownership boundary | A different team will own, page for, and evolve it |
| Fault isolation requirement | A failure here must not take down the rest, and a process boundary actually achieves that |

"Cleanliness", "separation of concerns", and "best practice" are **never** valid triggers — a module boundary gives you the same separation for free. Every new network boundary buys you, non-negotiably: **latency** on every call, **partial failure** (the other side can be slow, down, or half-done), **distributed transactions** (sagas/outbox where a local commit used to suffice), and **observability cost** (tracing, dashboards, alerts, on-call). Name all four in the tradeoff section of any approach that adds a service.

### Sync vs async heuristics

| Use request/response (REST) | Use events (Kafka) |
|---|---|
| Queries — caller needs the answer now | Fan-out — N consumers care, producer shouldn't know them |
| UX-blocking operations in the request path | Decoupling lifecycles — consumer downtime must not block producer |
| Caller must act on success/failure immediately | Eventual consistency is tolerable for this data |
| Simple point-to-point with strong consistency | Buffering bursts / load leveling |

Async adds: ordering questions, duplicate delivery (consumers must be idempotent), and harder debugging. Sync adds: temporal coupling and cascading failure (needs timeouts/circuit breakers — see `resilience-performance`).

## Phase 3: Decision — record a lightweight ADR

Record the choice as a compact ADR: **Context** (the problem and constraints), **Decision** (which approach and why over the alternatives), **Consequences** (what gets harder, what we're committing to). Template and a filled example (Kafka vs REST for order events): `references/adr-template.md`.

## Phase 4: Plan, then the gate

1. Break the build into small, verifiable steps — each step names its check (`mvn verify`, a specific test, a curl against the endpoint) and what failure looks like.
2. **GATE: present design + ADR + plan and wait for explicit user approval.** Do not write production code, scaffolding, or "just the entities to save time" before the yes. Then implement step by step with `tdd-java`.

**Circuit-breaker: if during implementation the design needs a 3rd structural revision (new entity relationships, changed API contract, moved boundary), STOP.** The design was wrong. Return to Phase 2 with what you learned; don't keep patching the plan mid-flight.

## Rationalization table

| Excuse | Reality |
|---|---|
| "It's cleaner as a separate service" | A module gives identical separation without latency, partial failure, distributed tx, and an on-call rota. "Cleaner" is not a scaling trigger. |
| "We'll need it later" | Then design for it later, with real requirements. Speculative boundaries are the hardest kind to remove. |
| "The user already knows what they want, I'll just build it" | They named a solution. The design phase exists to check it solves their problem — that's the job, not obstruction. |
| "Writing options is slower than just building the obvious one" | An hour of design is cheaper than a week of rework. The obvious option survives the comparison — fine, now it's a decision. |
| "I'll start the entities/scaffolding while we discuss" | Code written before approval anchors the decision. That's the gate being violated with extra steps. |
| "There's nothing reusable here, this is new" | Did you search? Existing service, internal lib, prior ADR. "New" is a conclusion, not a starting point. |
| "An ADR is bureaucracy for a change this size" | If it crosses the Iron Law threshold, the ADR is 10 lines. If it's genuinely below threshold, say so and skip the whole skill. |
| "Kafka is more scalable, so events by default" | Default follows the interaction shape: queries are sync. Scalability you don't need buys complexity you do pay. |

## Red flags — stop if you catch yourself writing

- "Let me start implementing and we can adjust the design as we go"
- "The obvious approach here is X" — with no alternative examined
- "It's faster" / "it's cleaner" / "it's more scalable" — with no named cost
- "We should make this a microservice for better separation of concerns"
- "I'll create the basic structure first" — before approval
- A design section with gains but no failure modes
- "We don't need an ADR for this" — for a change that crosses a boundary

Any of these means: return to the current phase's gate.

## Verification checklist

Before moving to implementation:

- [ ] Problem restated and confirmed — not just the requested solution echoed back
- [ ] Existing code/services/libs searched; reuse considered and ruled in or out with reasons
- [ ] 2–3 approaches, each with gains, costs, AND failure modes
- [ ] Any new service justified by a proven trigger, with all four network-boundary costs named
- [ ] Sync/async choice justified by interaction shape, not fashion
- [ ] ADR recorded (context / decision / consequences)
- [ ] Step-by-step plan where every step has its own check
- [ ] Explicit user approval received — in their words, not inferred

## Related skills

- `tdd-java` — how each approved plan step gets implemented
- `resilience-performance` — timeouts, retries, circuit breakers for any new remote call the design adds
- `kafka-event-patterns` — once the decision lands on async events
- `jpa-database-patterns` — schema and transaction-boundary consequences of the design
- `reviewing-java-code` — reviewing the implementation against the approved design
