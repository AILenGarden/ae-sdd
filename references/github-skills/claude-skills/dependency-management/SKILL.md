---
name: dependency-management
description: >
  Use when managing Maven or Gradle dependencies in a Java/Spring Boot
  project: NoSuchMethodError or ClassNotFoundException at runtime after a
  dependency change, version conflicts ("omitted for conflict"), deciding
  where a version should live (BOM, dependencyManagement, version catalog,
  properties), spring-boot-starter-parent vs BOM-only import, upgrading
  Spring Boot or any library safely, dependency convergence/enforcer
  failures, unused or undeclared dependencies (dependency:analyze), CVE
  audits of transitive dependencies (OWASP dependency-check), Gradle
  version catalogs (libs.versions.toml), platform vs enforcedPlatform,
  dependency locking, or triaging "which version actually wins" with
  dependency:tree / dependencyInsight. Not for runtime failure handling or
  timeouts — use resilience-performance. Not for Spring code conventions —
  use spring-boot-standards.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---

# Dependency Management Discipline (Maven and Gradle)

## When to use

- Runtime `NoSuchMethodError`, `NoClassDefFoundError`, or `ClassNotFoundException` after adding/upgrading a dependency
- Two different versions of the same library appear on the classpath, or builds differ between modules
- A Spring Boot upgrade is planned, or a CVE report flags a transitive dependency
- Versions are scattered across pom/build files and nobody knows which one wins
- Reviewing a diff that touches `pom.xml`, `build.gradle(.kts)`, or `libs.versions.toml`

## Quick reference

| Problem | Symptom | Solution |
|---|---|---|
| Version conflict | `NoSuchMethodError` at runtime, works in tests | Maven: `dependency:tree -Dverbose`; Gradle: `dependencyInsight` — then manage the version in ONE place |
| Spring-managed lib pinned ad hoc | Jackson/Tomcat/Kafka-clients version hardcoded on a dependency | Remove the version; override via Boot's version property or BOM bump only |
| Undeclared transitive use | Build breaks when an unrelated dependency is removed | `mvn dependency:analyze` → declare every directly-imported artifact |
| Unused declared deps | Bloated classpath, slow builds, wider CVE surface | `dependency:analyze` "Unused declared" → remove (verify runtime-only deps first) |
| Silent divergence | Different modules resolve different versions | Enforcer `dependencyConvergence` (Maven) / dependency locking (Gradle) in CI |
| CVE in transitive | Security scan fails on a lib you never declared | Bump via dependencyManagement/constraint; exclusion only if unused at runtime |
| Snapshot/duplicate/banned deps | Irreproducible builds, license issues | Enforcer `bannedDependencies`; Gradle `resolutionStrategy.failOnVersionConflict()` cases |
| Stale versions | Upgrades pile up until they're dangerous | `versions:display-dependency-updates` / version catalog + Renovate-style PRs, small and often |

## MUST / MUST NOT

**MUST**

- Keep every version in exactly one place: Maven `dependencyManagement` + properties in the parent/root pom; Gradle `libs.versions.toml`.
- Import framework BOMs (`spring-boot-dependencies`, `spring-cloud-dependencies`, `testcontainers-bom`) instead of versioning their artifacts individually.
- Run convergence enforcement in CI (Maven enforcer / Gradle locking) so conflicts fail the build, not production.
- Run a CVE scan (OWASP dependency-check or equivalent) in CI against the **resolved** dependency tree, transitives included.
- Upgrade Spring Boot by bumping the BOM/parent version only, then run the full `verify` build (workflow: references/upgrade-workflow.md).

**MUST NOT**

- MUST NOT hardcode a version on a dependency that a BOM already manages — it silently wins over the BOM and breaks the next upgrade.
- MUST NOT resolve a conflict with an `<exclusion>` when the artifact is still needed — pin the version via dependencyManagement/constraint instead; exclusions are for removing, not selecting.
- MUST NOT mix `spring-boot-starter-parent` and a manually imported `spring-boot-dependencies` BOM in the same project.
- MUST NOT depend on classes from transitive dependencies without declaring them — the next minor upgrade of the direct dependency may drop them.
- MUST NOT use Gradle dynamic versions (`+`, `latest.release`) or Maven version ranges in applications.

## Maven discipline

**Parent vs BOM-only.** Default to `spring-boot-starter-parent` (gives plugin management,
resource filtering, sensible defaults). Use BOM-only (`spring-boot-dependencies` in
`dependencyManagement` with `scope=import`) only when a corporate parent already occupies the
parent slot. Complete snippets: `references/maven-recipes.md`.

**Overriding a Boot-managed version** — the one sanctioned way (parent users):

```xml
<properties>
    <!-- Temporary CVE bump; remove when Boot's BOM catches up. Tracked in JIRA-1234. -->
    <jackson-bom.version>2.17.2</jackson-bom.version>
</properties>
```

**Conflict triage:**

```bash
mvn dependency:tree -Dverbose -Dincludes=com.fasterxml.jackson.core
```

Read the output: `(omitted for conflict with 2.15.4)` shows the loser; Maven picks **nearest
declaration wins**, not highest version — which is why an old version declared directly beats a
newer transitive. Fix by managing the version once in `dependencyManagement`, never by reordering
dependencies.

**Hygiene commands:**

```bash
mvn dependency:analyze                       # "Used undeclared" = declare it; "Unused declared" = remove it
mvn versions:display-dependency-updates      # what's stale (versions-maven-plugin)
mvn versions:display-plugin-updates
mvn org.owasp:dependency-check-maven:check   # CVE scan of resolved tree
```

`dependency:analyze` caveats: runtime-only deps (drivers, logging backends) and annotation-only
deps show as false-positive "unused" — verify before removing, then silence with
`ignoredUnusedDeclaredDependencies`.

**Enforcer** (full config in references/maven-recipes.md): `dependencyConvergence` +
`requireUpperBoundDeps` + `bannedDependencies` (e.g. `commons-logging`, `log4j:log4j`, duplicate
javax/jakarta artifacts), bound to `validate` so it fails fast in CI.

## Gradle discipline

**Version catalog** — the single source of truth (`gradle/libs.versions.toml`):

```toml
[versions]
spring-boot = "3.4.5"
testcontainers = "1.20.6"

[libraries]
testcontainers-postgresql = { module = "org.testcontainers:postgresql", version.ref = "testcontainers" }

[plugins]
spring-boot = { id = "org.springframework.boot", version.ref = "spring-boot" }
```

**platform() vs enforcedPlatform():** `platform(...)` contributes the BOM's versions as
constraints that normal conflict resolution can still override (Gradle picks the **highest**
version, unlike Maven). `enforcedPlatform(...)` makes the BOM's versions win unconditionally —
including downgrading transitives, which can reintroduce CVE-vulnerable versions silently.
Default to `platform()`; reach for `enforcedPlatform()` only with a written reason.

```kotlin
dependencies {
    implementation(platform("org.springframework.boot:spring-boot-dependencies:${libs.versions.spring.boot.get()}"))
    implementation("org.springframework.boot:spring-boot-starter-web")   // no version — BOM-managed
}
```

(The Spring Boot Gradle plugin + dependency-management plugin achieve the same; pick one approach
per repo and stick to it.)

**Locking and triage:**

```bash
./gradlew dependencies --configuration runtimeClasspath          # full resolved tree
./gradlew dependencyInsight --dependency jackson-databind --configuration runtimeClasspath
./gradlew dependencies --write-locks                             # (with locking enabled) regenerate lockfiles
```

`dependencyInsight` shows *why* a version was selected (conflict resolution, constraint, rule) —
it is the Gradle equivalent of `dependency:tree -Dverbose` plus the explanation. Locking setup:
`references/gradle-recipes.md`.

## Core pattern: resolving a version conflict

❌ **BAD** — pinning versions on individual dependencies, scattered and BOM-fighting:

```xml
<dependencies>
    <dependency>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-web</artifactId>
    </dependency>
    <dependency>
        <groupId>com.fasterxml.jackson.core</groupId>
        <artifactId>jackson-databind</artifactId>
        <version>2.16.0</version>   <!-- overrides Boot's BOM silently; half of jackson now mismatched -->
    </dependency>
    <dependency>
        <groupId>org.apache.kafka</groupId>
        <artifactId>kafka-clients</artifactId>
        <version>3.6.1</version>    <!-- Boot manages this too; drift guaranteed on next Boot bump -->
    </dependency>
</dependencies>
```

This "works" today and detonates at the next upgrade: jackson-databind 2.16.0 alongside
jackson-core 2.17.x from the BOM is exactly how runtime `NoSuchMethodError` is manufactured.

✅ **GOOD** — versions live in one managed place, artifacts stay versionless:

```xml
<properties>
    <!-- Boot-managed libs: override ONLY via Boot's documented version properties, with a reason -->
    <kafka.version>3.6.2</kafka.version>   <!-- KAFKA-XXXXX fix; remove at Boot 3.4.x -->
</properties>

<dependencyManagement>
    <dependencies>
        <!-- Libraries Boot does NOT manage get their version here, once, for all modules -->
        <dependency>
            <groupId>org.mapstruct</groupId>
            <artifactId>mapstruct</artifactId>
            <version>${mapstruct.version}</version>
        </dependency>
    </dependencies>
</dependencyManagement>

<dependencies>
    <dependency>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-web</artifactId>
    </dependency>
    <dependency>
        <groupId>org.apache.kafka</groupId>
        <artifactId>kafka-clients</artifactId>   <!-- version comes from kafka.version property -->
    </dependency>
    <dependency>
        <groupId>org.mapstruct</groupId>
        <artifactId>mapstruct</artifactId>       <!-- version comes from dependencyManagement -->
    </dependency>
</dependencies>
```

Gradle equivalent: the version lives in `libs.versions.toml`, the BOM comes in via `platform()`,
and a needed override is a `constraints { }` block with a comment — same one-place rule.

## Verification

After ANY dependency change:

```bash
mvn -q dependency:analyze && mvn verify          # Gradle: ./gradlew dependencies --write-locks check
mvn dependency:tree -Dverbose | grep -i "omitted for conflict" | sort -u
```

What failure looks like and what to do:

- **Enforcer `dependencyConvergence` failure** — output lists the divergent paths. Add one entry to `dependencyManagement` choosing the version, with a comment naming the conflict. Do not disable the rule.
- **`dependency:analyze` "Used undeclared"** — add the listed artifact as a direct dependency. "Unused declared" — confirm it isn't runtime-only, then delete.
- **Tests green but app fails at startup** (`NoSuchMethodError`) — version skew within a library family (jackson, netty, kafka). Run the tree/insight command for that group; align via BOM, remove ad-hoc pins.
- **Lockfile diff you didn't intend** (Gradle) — a transitive moved; inspect with `dependencyInsight` before committing the lock change.

## References

| File | Contents | When to load |
|---|---|---|
| references/maven-recipes.md | Complete pom snippets: parent vs BOM-only import, enforcer plugin config, multi-module version management, OWASP dependency-check, versions-maven-plugin | Writing or fixing Maven build files |
| references/gradle-recipes.md | Complete version catalog, platform usage, dependency locking setup, dependencyInsight triage examples | Writing or fixing Gradle build files |
| references/upgrade-workflow.md | Safe Spring Boot upgrade checklist: pre-flight, BOM bump, OpenRewrite recipes, verification gates, rollback criteria | Planning or executing a Spring Boot / major library upgrade |

## Related skills

- **spring-boot-standards** — code and configuration conventions inside the service; this skill stops at the build file.
- **resilience-performance** — runtime behavior of the libraries you just upgraded (timeouts, pools, breakers).
- **kafka-event-patterns** — Kafka usage patterns; here we only pin kafka-clients versions correctly.
- **reviewing-java-code** — overall review workflow; load this skill when the diff touches build files.
