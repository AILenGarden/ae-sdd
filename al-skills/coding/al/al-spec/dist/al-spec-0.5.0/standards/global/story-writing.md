# Story Writing Reference

> Maintainer/provenance reference only. Runtime Story rules live in
> `skills/story/SKILL.md`; do not require this file when invoking the SKILL.

The Story describes one coherent, independently understandable behavior. It
may stand alone; related PRD or DR references are optional context and must not
be treated as prerequisites.

## Required Content

- actor/trigger, user value, scope, non-scope, and preconditions;
- main flow and explicit exception/boundary flows;
- interface contract for each REST, SPI, event, or UI interaction in scope;
- field-level source, transform, destination, requiredness, and rationale;
- business rules, state changes, error codes, and non-functional behavior;
- observable acceptance criteria in Given/When/Then form;
- optional test and implementation notes without inventing repository paths.

## Field and Interface Rules

- Each input and output field has a source and semantic reason. Values must not
  appear from nowhere because a type is convenient.
- For each interface state method, path, headers, auth, request, response,
  errors, idempotency, and retry behavior when applicable.
- Name invalid input, unauthorized access, missing data, duplicate requests,
  dependency failure, and recovery behavior when those cases are in scope.
- Keep protocol adaptation in the interface boundary and business rules in the
  domain/application responsibility named by the Story.

## Acceptance Rules

- Each `AC-*` has a single observable result and an evidence target.
- Include the happy path and the highest-risk negative/boundary paths.
- An AC may optionally reference `REQ-*` or `DEC-*`; absent references do not
  invalidate the Story.
- Do not use "works correctly" or "handles gracefully" without the observable
  state, response, or side effect that proves it.

## Review Checklist

- Is the delivery boundary small enough to understand and verify as one unit?
- Can a caller implement the interface from the Story without guessing?
- Are field mappings closed from source to destination?
- Do ACs cover the stated rules and negative paths?
