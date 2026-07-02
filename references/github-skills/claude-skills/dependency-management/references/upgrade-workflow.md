# Safe Spring Boot Upgrade Workflow

Checklist for upgrading Spring Boot (and, with minor adaptation, any major library/BOM). Built
around one principle: **the BOM bump is one commit, behavioral fixes are separate commits**, so
the diff stays reviewable and bisectable.

## Pre-flight (before touching any file)

- [ ] Read the release notes for EVERY minor between current and target (e.g. 3.2 → 3.4 means reading 3.3 *and* 3.4 notes). Spring publishes a dedicated "Upgrading" wiki page per release — note deprecations-turned-removals, property renames, dependency major bumps (Jackson, Hibernate, Kafka clients).
- [ ] Inventory ad-hoc version pins: every property like `jackson-bom.version`, every hardcoded version in `dependencyManagement`/`constraints` that shadows a Boot-managed artifact. Each is a potential silent conflict with the new BOM. Plan to delete the ones whose reason (CVE fix, bug) the new BOM absorbs.
- [ ] Check companion BOM compatibility: Spring Cloud has a strict compatibility matrix with Boot minors — upgrading Boot without moving Spring Cloud to the matching release train is the #1 upgrade-breaks-everything cause. Same check for any internal platform BOM.
- [ ] Confirm CI is green on the current version (you need a trusted baseline).
- [ ] For minor/major jumps: upgrade to the **latest patch of the current minor first**, verify, then jump minors one at a time.

## Step 1 — Run OpenRewrite recipes

Automates the mechanical part (property renames, deprecated API replacements,
javax→jakarta if coming from Boot 2):

```bash
# Maven — no pom changes needed, run directly:
mvn org.openrewrite.maven:rewrite-maven-plugin:run \
  -Drewrite.recipeArtifactCoordinates=org.openrewrite.recipe:rewrite-spring:RELEASE \
  -Drewrite.activeRecipes=org.openrewrite.java.spring.boot3.UpgradeSpringBoot_3_4
```

```kotlin
// Gradle — apply temporarily:
plugins { id("org.openrewrite.rewrite") version "7.4.0" }
rewrite {
    activeRecipe("org.openrewrite.java.spring.boot3.UpgradeSpringBoot_3_4")
}
dependencies { rewrite("org.openrewrite.recipe:rewrite-spring:latest.release") }
```

```bash
./gradlew rewriteRun
```

Review the rewrite diff **line by line** before committing — recipes are good but not infallible,
especially around custom security configuration and property files with profiles. Commit the
rewrite output as its own commit: `chore: openrewrite UpgradeSpringBoot_3_4`.

## Step 2 — Bump the BOM

- Maven parent users: change `spring-boot-starter-parent` version. BOM-only: change the `spring-boot-dependencies` import version. Gradle: bump `spring-boot` in `libs.versions.toml`.
- Bump companion BOMs to their matrix-matching versions (Spring Cloud release train, etc.).
- Delete every ad-hoc pin the new BOM makes obsolete (from pre-flight inventory).
- Gradle with locking: `./gradlew dependencies --write-locks` — the lockfile diff IS your transitive-change review. Read it; jackson/hibernate/tomcat majors hiding in there are tomorrow's runtime surprises.

## Step 3 — Full verification gate

```bash
mvn clean verify                 # Gradle: ./gradlew clean check
```

Must pass before proceeding — and "pass" means with integration tests (Testcontainers) enabled,
not skipped. Then:

- [ ] `mvn dependency:tree -Dverbose | grep -i "omitted for conflict"` (Gradle: review lockfile diff / `dependencyInsight` on anything suspicious) — no NEW conflicts versus baseline.
- [ ] Boot the service locally against real backing services (Docker Compose / Testcontainers dev mode). Startup is where property renames and bean incompatibilities actually fail — unit tests rarely catch them.
- [ ] Grep startup logs for `deprecated`, `WARN`, and Boot's property-migration hints. Add `spring-boot-properties-migrator` as a temporary runtime dependency to get explicit rename reports; **remove it before merging**.
- [ ] Smoke the actuator surface: `/actuator/health/{readiness,liveness}`, `/actuator/prometheus` — metric names occasionally change between minors and silently break dashboards and alerts. Diff the metric-name list against a baseline capture if your alerting is load-bearing.
- [ ] Re-run the CVE scan — upgrades occasionally *reintroduce* vulnerable transitives (and your old suppressions may now be deletable).

## Step 4 — Behavioral review (the non-mechanical part)

Check release notes against your actual usage:

- Default-behavior changes (e.g. trailing-slash matching, graceful shutdown becoming default, observability auto-config changes) — grep the codebase for each affected feature.
- Hibernate minor bumps: review generated SQL for hot queries if the notes mention dialect/sequence changes (route deep issues to jpa-database-patterns).
- Kafka clients major bumps: check default config changes against your explicit producer/consumer configs (route semantics to kafka-event-patterns).
- Anything you fixed with a workaround + "remove when Boot X.Y" comment — search for those comments now: `grep -rn "remove when\|remove at Boot\|TODO.*upgrade" --include=pom.xml --include="*.toml" --include="*.kts" .`

## Step 5 — Rollout

- [ ] One PR per service; never a bulk multi-repo upgrade in one change.
- [ ] Deploy to a staging environment with production-like traffic first; watch p99, error rate, GC, and startup time against the pre-upgrade baseline for at least one traffic cycle.
- [ ] Canary in production where the platform supports it; define rollback criteria BEFORE deploying (e.g. "error rate > 2× baseline for 5 min").
- [ ] Rollback plan = redeploy previous image. If a DB migration shipped in the same release, you've coupled two risks — don't; ship migrations separately.

## Troubleshooting common upgrade failures

| Failure | Likely cause | Fix |
|---|---|---|
| `NoSuchMethodError` / `NoSuchFieldError` at startup | An ad-hoc pin survived the bump and now mismatches a BOM-managed sibling (jackson-databind vs jackson-core is the classic) | `dependency:tree -Dverbose` / `dependencyInsight` on the group; delete the pin |
| Bean creation failure for an auto-configured bean | Auto-configuration condition changed, or a property was renamed | Add `spring-boot-properties-migrator` temporarily; check the release notes' auto-config section |
| Tests pass, app fails in staging only | Profile-specific properties not migrated, or a runtime-scoped dependency dropped by a starter | Diff `mvn dependency:list -Dscope=runtime` (or runtime lockfile) before/after |
| Surefire/Failsafe suddenly skipping tests | Plugin versions inherited from the old parent no longer match JUnit platform | Re-check plugin management; BOM-only builds must pin surefire/failsafe themselves |
| Dashboards empty after deploy | Micrometer metric or tag names changed between minors | Diff `/actuator/prometheus` output against the pre-upgrade capture; update queries/alerts |
| Spring Cloud beans missing | Boot bumped without the matching Cloud release train | Fix the train version per the compatibility matrix; never mix |

## Cadence

Upgrades age like milk: 3.2→3.3 is an afternoon; 2.7→3.4 is a quarter. Policies that keep it
cheap:

- Patch versions: automated PRs (Renovate/Dependabot on the catalog/parent version), merged on green CI without ceremony.
- Minor versions: within a sprint of release, after the `.1` patch lands.
- Track EOL: running a Boot minor past OSS EOL means no CVE patches — that converts every CVE into an emergency upgrade, the most dangerous kind.
