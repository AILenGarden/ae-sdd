# Configuration: Properties, Profiles, Secrets

How configuration is structured, validated, and kept secret-free in a Spring Boot 3.x
service.

## `@ConfigurationProperties` records

One record per logical prefix, validated at startup so bad config fails fast instead of at
first use:

```java
@Validated
@ConfigurationProperties(prefix = "app.orders")
public record OrdersProperties(
        @NotNull @Min(1) @Max(500) Integer maxLinesPerOrder,
        @DefaultValue("PT24H") Duration idempotencyKeyTtl,
        @Valid Notification notification) {

    public record Notification(
            @NotBlank String topic,
            @DefaultValue("true") boolean enabled) {
    }
}
```

```yaml
app:
  orders:
    max-lines-per-order: 100
    idempotency-key-ttl: 24h
    notification:
      topic: orders.order.created
```

Registration — pick one:

```java
@SpringBootApplication
@ConfigurationPropertiesScan          // scans the whole base package
public class OrdersApplication { ... }
```

Rules:

- Records only — immutable, constructor-bound, no setters to mutate after startup.
- Nested records for grouped settings; `@Valid` on the nested field cascades validation.
- `Duration`/`DataSize` types over raw numbers — `24h`, `512MB` parse natively.
- A startup failure like `Binding to target ... failed ... maxLinesPerOrder must not be null`
  is the system working: fix the YAML, don't relax the constraint.
- `@Value` is acceptable only for a single one-off value used in exactly one place.

Add `spring-boot-configuration-processor` (annotation processor, `optional`/`provided`)
to get IDE autocompletion in YAML for your properties.

## Profiles

Strategy: **defaults in `application.yml`, deltas per environment, secrets from the
environment everywhere.**

```
src/main/resources/
├── application.yml          # shared defaults — the bulk of config lives here
├── application-local.yml    # developer laptop: docker-compose hosts, relaxed logging
├── application-staging.yml  # staging deltas only
└── application-prod.yml     # prod deltas only
```

```yaml
# application.yml
spring:
  application:
    name: orders-service
  datasource:
    url: jdbc:postgresql://${DB_HOST:localhost}:5432/orders
    username: ${DB_USERNAME:orders}
    password: ${DB_PASSWORD}        # no default for secrets — fail if absent
  jpa:
    open-in-view: false             # always; see jpa-database-patterns

# application-prod.yml — deltas only
logging:
  level:
    root: INFO
spring:
  jpa:
    properties:
      hibernate.generate_statistics: false
```

- Activate with the `SPRING_PROFILES_ACTIVE` env var in the deployment manifest — never
  hardcode `spring.profiles.active` in `application.yml`.
- Profile files contain only what *differs*. If `application-prod.yml` repeats a value from
  `application.yml`, delete it from the profile file.
- `application-local.yml` may carry harmless local defaults (localhost, dev credentials for
  containers you start yourself). It must still never contain real credentials.
- Avoid `@Profile`-switched beans for business logic — profiles select *environments*, not
  *behavior*. Behavior toggles are explicit boolean properties.
- Test config goes in `src/test/resources/application-test.yml` plus
  `@ActiveProfiles("test")`; Testcontainers overrides datasource/broker URLs via
  `@ServiceConnection` or `@DynamicPropertySource`.

## Secrets handling

The one rule: **a secret never appears as a literal in any file that reaches git.**

| Mechanism | How | When |
|---|---|---|
| Env var placeholder | `password: ${DB_PASSWORD}` | Baseline — works with every orchestrator |
| Kubernetes Secret → env | `secretKeyRef` in the deployment manifest | Standard k8s deployments |
| Mounted secret files | `spring.config.import: optional:configtree:/run/secrets/` | Vault agent / CSI driver file mounts |
| Vault / cloud secret manager | `spring-cloud-vault` or AWS/GCP config import | Central rotation requirements |
| Local development | `.env` file (gitignored) or shell exports; compose `env_file:` | Developer laptops |

Configtree example — each file under `/run/secrets/` becomes a property named after its
relative path:

```yaml
spring:
  config:
    import: "optional:configtree:/run/secrets/"
```

Hygiene:

- No default value on secret placeholders. `${DB_PASSWORD:changeme}` ships "changeme" to
  prod the day the env var is mistyped; a startup failure is the better outcome.
- Never log resolved configuration at startup; never include properties in error responses.
- Actuator: keep `env` and `configprops` endpoints off the public surface
  (`management.endpoints.web.exposure.include` allow-list); Boot 3 sanitizes values in
  these endpoints by default (`show-values: never`) — leave it that way.
- CI guard (also in SKILL.md verification):

```bash
grep -rnE "(password|secret|api-key|token)\s*[:=]\s*[^$\s][^\s]*" src/main/resources \
  --include='*.yml' --include='*.properties' && exit 1 || true
```

- If a secret ever lands in git: rotate it immediately. Removing the commit is not
  remediation — the value is burned.

## Configuration precedence (what overrides what)

Highest wins, abbreviated to what matters in services:

1. Command-line args / `SPRING_APPLICATION_JSON`
2. OS environment variables (`DB_PASSWORD` → `db.password` via relaxed binding)
3. Profile-specific files (`application-prod.yml`)
4. `application.yml`

Practical consequence: the deployment environment can override any default without an
image rebuild — which is exactly why secrets and per-env endpoints belong there, and why a
"mystery value" at runtime is usually an env var shadowing your YAML. Check with the
(locally enabled) actuator `env` endpoint, which shows each property's source.
