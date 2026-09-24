# ALSpec Three-Document Schema Reference

> Framework/reference metadata. The invoked type SKILL embeds the normative
> writing rules; this schema does not need to be loaded as a runtime prerequisite.

The schema describes the contract shared by the three current specification types. It is
not a parser mandate for every project document; type SKILLs and project layers
may add fields and sections, but they must preserve IDs and traceability.

## Independent Types

The three types are a peer set, not a required pipeline. A document may include
optional references to other Specs, but it remains valid and usable when those
documents do not exist.

## Required Metadata

Every document should declare `spec_id`, `spec_type`, `status`, `version`,
`owner`, `project_id`, and `updated`. Optional related Spec IDs should be named
explicitly rather than inferred from file names.

## Required ID Families

| Family | Meaning | Owner |
|--------|---------|-------|
| `REQ-*` | system requirement | whichever current Spec declares it |
| `DEC-*` | architecture or implementation decision | whichever Spec records it |
| `STORY-*` | independently deliverable behavior | DR/Story |
| `AC-*` | observable acceptance criterion | Story |
| `TC-*` | executable test case | TestCase |
| `Q-*` | open question or unresolved assumption | any stage |

IDs are unique within a project and are not recycled after publication.

## Type Contracts

- **DR**: context, boundaries, responsibilities, decisions, data/state,
  contracts, failure/security/operations, and design acceptance.
- **Story**: context, independent scope, behavior flow, interface/data mapping,
  business rules, edge cases, AC, and implementation notes.
- **TestCase**: coverage objective, environment/data, AC matrix when available,
  executable scenarios, oracle/evidence, failure recovery, and reporting.

## Ready/Blocked States

Recommended statuses are `draft`, `review`, `ready`, `in-progress`, `blocked`,
`accepted`, and `superseded`. A document is `blocked` when an unanswered
question can change scope, architecture, data, security, compatibility, or
verification. It is not acceptable to mark such a document `ready` with a
placeholder decision.
