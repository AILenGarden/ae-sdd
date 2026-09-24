# life Project Spec Transcription Reference

> Maintainer/provenance reference only. Runtime life transcription rules are
> embedded in `skills/life-dr`, `skills/life-story`, and `skills/life-testcase`;
> do not require this file when invoking those SKILLs.

This document defines how the useful document content from the former `ae-sdd`
DR, Story, and TestCase materials is adapted for the `life` project in ALSpec.
It is a content-translation rule, not an `ae-sdd` workflow import.

## Source and Target

- Source project key: `icec-cloud-life` (the repository is commonly called
  `life`).
- Repository root: `D:\Item\life`.
- Source root: `D:\Item\life\2c\`.
- Target ALSpec project ID: `icec-cloud-life` (`life` is the short project name).
- Source materials consulted: `ae-sdd` DR/Story/TestCase templates, the
  TestCase strategy, and the `icec-cloud-life` project asset documents.

## Keep During Transcription

- document purpose and section responsibilities;
- design/content completeness checks;
- explicit boundaries, field mappings, contracts, error behavior, failure
  handling, test levels, fixtures, and evidence requirements;
- project facts that are present in the life project assets;
- examples rewritten so they describe a life module without claiming an
  unverified fact about every life service.

## Exclude During Transcription

Do not copy these `ae-sdd` concerns into a standalone ALSpec SKILL or template:

- Phase 1/2/3 routing or stage transitions;
- RA/PRD hard prerequisites or downstream/upstream dependency gates;
- `ae-sdd` CLI commands, state files, memory lifecycle, iteration selection,
  document-storage APIs, `save_doc`, or `resolve_path` protocols;
- reviewer sub-agent orchestration, user-confirmation loops, plan-first loops,
  gate IDs, stop checks, hook contracts, or cascade update mechanics;
- claims that a document automatically triggers another SKILL.

## Type Mapping

| Source material | ALSpec target | Adaptation |
|-----------------|---------------|------------|
| DR template sections on architecture, constraints, decisions, data, contracts, failures, security, testing, observability, migration, risks | `templates/project/life-dr.md` | retain content checks; remove lifecycle/process instructions and make related documents optional |
| Story template sections on behavior, interfaces, field mapping, errors, data, non-functional behavior, AC, and implementation notes | `templates/project/life-story.md` | retain field-level and life boundary rules; remove DR/PRD prerequisites, Phase labels, and review gates |
| TestCase template + strategy on levels, type dimensions, fixtures, mocks, and evidence | `templates/project/life-testcase.md` | retain coverage model and real HTTP/DB expectations; remove Story/PRD read gates and storage/report orchestration |

## Conversion Rules

1. Replace `ae-sdd` paths with the ALSpec project registry path or a direct
   project document path.
2. Replace “必须先读上游文档” with “相关文档可选；若提供则记录引用”。
3. Replace `ae-sdd` gate language with a content review checklist for the
   current document.
4. Replace project facts marked as “待探查” with an explicit unknown; do not
   generalize an icec-cloud-life-cs or -im example to all domains.
5. Preserve source evidence links and the date/audit state of each fact.
6. Keep `life-*` SKILL names and `scope: project`, `project_id: icec-cloud-life` registry
   entries distinct from global type SKILLs.
