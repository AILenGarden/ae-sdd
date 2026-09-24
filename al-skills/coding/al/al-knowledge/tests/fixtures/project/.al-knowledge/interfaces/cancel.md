---
type: API Endpoint
title: API
description: Fixture API
al:
  profile: ae-sdd-knowledge/1
  id: API
  interface:
    service: appointments
    protocol: HTTP
    signature: POST /appointments/{id}/cancel
  entry: CANCEL
  nodes:
  - id: CANCEL
    title: cancel(status)
    kind: method
    symbol: fixture.app.cancel(str)
  - id: EVENT
    title: Cancellation event
    kind: boundary
    stop_reason: asynchronous fixture boundary
  calls:
  - id: publish
    type: publishes
    from: CANCEL
    to: EVENT
    condition: only if publishing is introduced
    evidence_state: planned
    source_ids: []
  relations:
  - id: implementation
    type: implements
    target: ../specs/story.md#rule
    target_id: STORY
    evidence_state: confirmed
    coverage: full
    source_ids:
    - spec
    - code
    checked:
      by: process:fixture-author
      at: '2026-09-22T10:00:00+08:00'
      body_sha256: 58bbeece5f9b144a4874c148955e90544a3ff1ce3558792ee711ad428f7e2cdc
      source_sha256:
        spec: e8163fbb03fee2119e82b9209e1fc03b6a70a74e846b2c3a297aba6ed43c9f6c
        code: 197a4bdd4dbe283d20bd13655ccc4ea74d93327d17de6007b942e9101d7b7dd9
sources:
- id: spec
  resource: ../references/spec.md
- id: code
  resource: ../references/app.md
---

# Cancellation endpoint
Accepts only draft. HTTP binding is a fictional fixture, not a deployed API.
