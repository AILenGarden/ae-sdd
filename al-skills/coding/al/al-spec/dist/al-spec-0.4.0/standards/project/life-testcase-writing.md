# life TestCase Writing Supplement Reference

> Maintainer/provenance reference only. Runtime life TestCase rules live in
> `skills/life-testcase/SKILL.md`; do not require this file when invoking the SKILL.

Apply this supplement after the global TestCase standard for the
`icec-cloud-life` project (short name: `life`).
The TestCase can be created from any available behavior, contract, requirement,
or defect description; no other Spec type is required.

## Default Life Test Stack

- Unit: JUnit 4.12 + Mockito + AssertJ.
- Integration: Spring Boot Test with development/test database and
  `@Transactional` + `@Rollback` where applicable.
- Interface: prefer real HTTP with `SpringBootTest(RANDOM_PORT)` +
  `TestRestTemplate`; use MockMvc only when the target framework cannot provide
  a real embedded HTTP path, and record the reason.
- Quality: include Checkstyle/SpotBugs checks when the target module uses them.

## Required Coverage Decisions

- Service unit tests mock direct external dependencies and assert business
  fields, state changes, and side effects.
- Mapper/persistence tests verify custom SQL/XML, keys, indexes, and rollback
  behavior against a real or explicitly justified database.
- Core INSERT/UPDATE/DELETE, transaction rollback, distributed lock, Feign
  path, Redis invalidation, and callback/idempotency behavior require real
  dependencies when the project environment permits; otherwise record the
  limitation and compensating evidence.
- Cover input boundaries, authentication/authorization, concurrency,
  duplicate delivery, related-data states, multi-step sequences, and state
  transition matrices when applicable.

## Transcription Limits

Retain the source TestCase strategy's coverage dimensions and evidence rules.
Remove `ae-sdd` phase routing, mandatory PRD/Story/asset intake, confirmation
loops, `save_doc`/storage API calls, gate IDs, and automatic downstream SKILL
triggers.
