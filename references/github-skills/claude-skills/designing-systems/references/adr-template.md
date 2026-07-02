# Lightweight ADR Template

One ADR per significant decision. Keep it under a page — an ADR nobody reads is worse than none. Store alongside the code (e.g. `docs/adr/NNNN-short-title.md`), numbered sequentially, never edited after acceptance (supersede instead).

## Template

```markdown
# ADR-NNNN: <short, decision-shaped title>

Date: YYYY-MM-DD
Status: Proposed | Accepted | Superseded by ADR-MMMM

## Context

What problem are we solving and under which constraints (load, latency,
consistency, team, deadline)? What already exists that we considered
reusing? 3–6 sentences. Facts, not advocacy.

## Decision

What we chose, in one sentence. Then: which alternatives were considered
and the one or two reasons each was rejected. Name what this decision
trades away, not only what it gains.

## Consequences

What becomes easier. What becomes harder (be honest — every decision has
this section). New operational burden: dashboards, alerts, runbooks,
migrations. What would make us revisit this decision.
```

## Filled example

```markdown
# ADR-0014: Publish order lifecycle changes as Kafka events instead of REST callbacks

Date: 2026-06-10
Status: Accepted

## Context

When an order transitions state (CREATED → PAID → SHIPPED), three
downstream consumers need to know: notifications, the loyalty service,
and the analytics pipeline. Today the order service calls the
notifications REST endpoint synchronously inside the order transaction;
loyalty and analytics are new requirements. Peak load is ~50 order
events/s with bursts to 300/s during campaigns. Notifications going down
currently fails order updates — an outage we had twice last quarter.
A Kafka cluster already exists and is operated by the platform team.

## Decision

Publish `OrderStatusChanged` events to a Kafka topic via the
transactional outbox pattern; all three consumers subscribe
independently.

Alternatives considered:
- **Sequential REST callbacks** — rejected: temporal coupling means any
  consumer outage blocks order processing (the exact incident we had),
  and each new consumer requires a producer code change and deploy.
- **REST + retry queue per consumer** — rejected: rebuilds half a broker
  per consumer; the platform Kafka cluster already provides this.

What we trade away: read-your-write consistency for consumers (loyalty
points may lag a few seconds), exactly-once delivery (consumers must be
idempotent on event ID), and simple request-scoped debugging (we now
need correlation IDs through the topic).

## Consequences

Easier: adding a fourth consumer is a subscription, not a producer
change; order processing no longer depends on notification uptime;
bursts are absorbed by the topic.

Harder: consumers must implement idempotency keyed on event ID; we own
an outbox relay and its lag alert; schema evolution of the event payload
needs a compatibility policy (start with backward-compatible JSON, field
additions only). Grafana dashboard for consumer lag and outbox backlog
required before go-live.

Revisit if: a consumer turns out to need synchronous confirmation in the
order request path, or event volume stays so low that the operational
overhead outweighs the decoupling.
```

## Anti-patterns

- **Decision-free ADR** — pages of context, no committed choice. The D is the point.
- **Consequences-as-marketing** — only upsides listed. If "Harder" is empty, the analysis is.
- **Retroactive ADR theater** — writing the ADR after the code is merged to look disciplined. The ADR exists to be approved *before* implementation (the Phase 4 gate).
- **Editing accepted ADRs** — changes mean a new ADR with `Supersedes: ADR-NNNN`; history is the value.
