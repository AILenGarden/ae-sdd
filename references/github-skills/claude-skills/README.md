# Java + Spring Skill Set

Nine portable skills for Java/Spring microservice development. Flat Agent Skills layout — drop the directories into `.claude/skills/` (Claude Code) or load with `spring-ai-agent-utils` SkillsTool (any LLM). Built per the principles in `../knowledge/00-skill-authoring-principles.md`.

## Knowledge skills — what good code looks like

| Skill | Use for |
|---|---|
| `spring-boot-standards` | Package layout, DI rules, record DTOs, REST API conventions, error contract, config |
| `jpa-database-patterns` | N+1, lazy loading, transactions, PostgreSQL indexing/EXPLAIN, HikariCP, Flyway expand/contract |
| `kafka-event-patterns` | Producer/consumer conventions, retry+DLT, transactional outbox, idempotent consumers, event testing |
| `resilience-performance` | Timeouts, retries, circuit breakers, pool sizing, degradation, triage workflow, observability |
| `dependency-management` | Maven BOM/enforcer/analyze + Gradle catalogs/locking, conflict triage, safe upgrades |
| `oop-design` | SOLID, immutability, patterns that matter in Spring, refactoring catalog |

## Process skills — how to work

| Skill | Iron law |
|---|---|
| `tdd-java` | No production code without a failing test first |
| `designing-systems` | No implementation before an approved design (boundaries/schemas/APIs/3+ files); includes microservice-vs-monolith framework |
| `reviewing-java-code` | No approval without reading every changed line and running the checks |

`CLAUDE-template.md` — short per-repo CLAUDE.md starter that wires the set together.

## Conventions

Knowledge skills: symptom-triggered description → quick-reference table → MUST/MUST NOT → ❌/✅ pairs → verification commands (Maven + Gradle) → references/ loaded on demand. Process skills: core principle → iron law → gated phases → rationalization table → red flags → checklist. All SKILL.md <400 lines; descriptions route negatively to siblings to avoid trigger overlap.
