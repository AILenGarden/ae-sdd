# ALSpec Boundaries

ALSpec owns the current three type SKILL writing specifications, their canonical
templates, layer-selection metadata, and the loading/traceability contract. It
does not own project facts or the content of author-owned specifications.

## Global Layer

Global type SKILLs define stable document types, ID families, normative
language, minimum sections, evidence expectations, and review rules. They apply
to every project and are loaded with their required templates.

## Project Layer

Project type SKILLs define repository facts, constraint references, storage and
naming conventions, stricter checks, and approved deviations for one exact
`project_id`. They are loaded only after the repository identity is confirmed.

## Non-Negotiable Boundaries

- A document must not silently redefine another document's scope when it has an
  optional reference to it.
- A project layer must not silently delete global traceability or weaken a
  global `MUST`.
- A registry operation must not parse, rewrite, merge, or execute a document.
- A missing path is a discovery warning, not permission to invent content.
- A contradiction or design-changing unknown is a blocker until resolved by a
  documented decision.
- Acceptance is based on recorded evidence, not on a status value alone.

When two author-owned entries cover the same type, show both and require an
explicit decision about narrowing, replacement, or coexistence. The registry
never resolves that conflict automatically.
