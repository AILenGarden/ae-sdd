# life DR Writing Supplement Reference

> Maintainer/provenance reference only. Runtime life DR rules live in
> `skills/life-dr/SKILL.md`; do not require this file when invoking the SKILL.

Apply this supplement after the global DR standard when the repository is the
`icec-cloud-life` project (short name: `life`). It describes life-specific facts
and conversion rules only.

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
