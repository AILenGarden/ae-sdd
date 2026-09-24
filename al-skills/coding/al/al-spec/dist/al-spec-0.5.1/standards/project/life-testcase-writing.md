# life TestCase Writing Supplement Reference

> Maintainer/provenance reference only. Runtime TestCase rules live in the
> global `skills/testcase/SKILL.md`; the `icec-cloud-life` variation is the
> project template `templates/project/life-testcase.md`, selected by project
> ID. Do not require this file when invoking the SKILL.

This supplement records the provenance and review notes for the
`icec-cloud-life` TestCase template (short name: `life`). It describes
life-specific facts and conversion rules only; it is not a second runtime Skill
or a required context file.
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
