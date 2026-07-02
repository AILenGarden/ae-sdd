# CLAUDE.md — Spring Boot service template

Copy into a service repo as `CLAUDE.md`, fill the placeholders, delete what doesn't apply. Keep it short: every line must pass "would removing this cause mistakes?" Procedures belong in skills, not here.

---

# <service-name>

<One sentence: what this service does and who calls it.>

## Commands

- Build + all tests: `mvn verify` <!-- or ./gradlew check -->
- Single test class: `mvn test -Dtest=ClassName`
- Run locally: `mvn spring-boot:run -Dspring-boot.run.profiles=local` (deps via `docker compose up -d`)
- Migrations live in `src/main/resources/db/migration` (Flyway; never edit applied migrations)

## Stack facts

- Java 21, Spring Boot 3.x, Maven <!-- or Gradle -->, PostgreSQL, Kafka
- Observability: Micrometer → Prometheus/Grafana; logs are JSON with `correlationId` in MDC

## Rules that differ from defaults

- Constructor injection only; DTOs are records; entities never leave the service layer
- Every remote call has explicit connect+read timeouts
- New endpoints follow `/api/v1` conventions (see spring-boot-standards skill)
- Tests: positive and negative cases required; integration tests use Testcontainers, not H2

## Workflow

- Non-trivial changes (3+ files, schema, API, or service boundary): design first — the designing-systems skill gates this
- Implementation is test-first (tdd-java skill)
- Never mark work done without showing `mvn verify` output

## Gotchas

- <Repo-specific traps Claude can't infer: flaky test, odd module, env quirk>
