# life Story Writing Supplement Reference

> Maintainer/provenance reference only. Runtime Story rules live in the single
> global `skills/story/SKILL.md`; the life variation is the project template
> `templates/project/life-story.md` selected by project ID. There is no
> the global `story` Skill; life variation is the project template
> `templates/project/life-story.md`. There is no project-level Story Skill.
> Do not require this file when invoking Story.

Apply this supplement after the global Story standard for the
`icec-cloud-life` project (short name: `life`).
The Story remains independent; related DR or PRD references are optional.

## Life Boundary Rules

- State the exact domain and module: for example `life-cs`, `life-im`,
  `life-user`, `life-vehicle`, or `life-workticket`.
- Identify the actual module shape before naming paths. Do not assume every
  domain has the same four modules; `life-user` and dual-launch services need
  explicit verification.
- Map the call boundary where applicable: BFF -> SPI/Feign -> Service, or Web
  -> Service. State the boundary without requiring another Spec document.
- Domain logic belongs in the Domain layer; application code orchestrates;
  Repository implementations only store/read; Interfaces adapt protocol and
  validate input; BFF code aggregates and calls through Facade/Feign.

## Life Naming and Contract Rules

- Typical classes are `{Resource}ServiceImpl`, `{Resource}AppService`,
  `{Resource}Repository`, `{Resource}RepositoryImpl`, `{Resource}Client`,
  `{Resource}Converter`, `{Resource}DO`, `{Resource}PO`, `{Resource}DTO`,
  `{Resource}Request`, and `{Resource}VO`; verify actual source before fixing a
  name.
- Use `com.casstime.cloud.life.{domain}` as the normal package root only when
  confirmed for the target module.
- HTTP paths use lowercase hyphenated nouns; pagination commonly uses POST
  with a `PageRequest<T>` body; return wrappers and error code ranges must come
  from the target module's assets/constraints.
- For each field, record source, transform, destination, requiredness, and
  reason. For DB changes, record table/column/index/CRUD and the exact WHERE
  condition for updates/deletes.

## Transcription Limits

Retain the source Story content checks for interface completeness, field-level
mapping, error/boundary behavior, non-functional behavior, AC, and test/task
notes. Remove source Phase labels, upstream-read gates, review loops, user
confirmation steps, `ae-sdd` commands, and automatic handoff claims.
