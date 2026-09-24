# life DR Writing Supplement Reference

> Maintainer/provenance reference only. Runtime DR rules live in the global
> `skills/dr/SKILL.md`; the `icec-cloud-life` variation is the project template
> `templates/project/life-dr.md`, selected by project ID. Do not require this
> file when invoking the SKILL.

This supplement records the provenance and review notes for the
`icec-cloud-life` DR template (short name: `life`). It describes life-specific
facts and conversion rules only; it is not a second runtime Skill or a required
context file.

## Project Facts To Check

- Repository root is `D:\Item\life`; source code is under `2c\`.
- The root is not a Git repository and has no root Maven pom; modules are
  independently buildable Maven projects unless repository evidence says
  otherwise.
- Typical Service modules use DDD `interfaces`, `application`, `domain`, and
  `infrastructure` modules. Some life services have a separate BFF or a
  dual-launch `service` + `web` shape; verify the target domain before writing.
- Base packages normally follow `com.casstime.cloud.life.{domain}`.

## Required Life Design Details

- Identify whether each component is a BFF, SPI, Service, Web, API aggregate,
  or infrastructure adapter.
- For Service modules, state which layer owns protocol adaptation, application
  orchestration, domain rules, persistence, Feign, and messaging.
- Use project asset evidence for `ServiceProviderConstants`, Feign service
  names, context paths, ports, and package paths. Do not infer them from a
  similarly named domain.
- Explain cross-module contracts and ownership without turning them into a
  document dependency. A DR may be complete with no Story list.
- Apply the repository's known redlines: domain owns business rules;
  application coordinates; repositories only store/read; interfaces adapt and
  validate; BFFs do not access DB/Redis/Kafka directly.

## Transcription Limits

Keep the useful 18-topic DR content from the source template where relevant,
but do not copy `ae-sdd` Phase steps, RA intake, plan-first generation,
`document-storage`, `save_doc`, `resolve_path`, review-loop, or gate mechanics.
