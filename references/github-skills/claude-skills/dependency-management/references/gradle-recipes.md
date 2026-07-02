# Gradle Recipes

Complete, copyable Gradle (Kotlin DSL) configurations. Spring Boot 3.x / Java 21 baseline,
Gradle 8.x.

## Recipe 1 — Version catalog (gradle/libs.versions.toml)

The single source of truth for every version in the build. Checked in, diff-reviewable,
understood by Renovate/Dependabot.

```toml
[versions]
spring-boot = "3.4.5"
spring-cloud = "2024.0.1"
testcontainers = "1.20.6"
mapstruct = "1.6.3"
resilience4j = "2.3.0"

[libraries]
# BOMs imported via platform() in build files
spring-boot-bom = { module = "org.springframework.boot:spring-boot-dependencies", version.ref = "spring-boot" }
spring-cloud-bom = { module = "org.springframework.cloud:spring-cloud-dependencies", version.ref = "spring-cloud" }
testcontainers-bom = { module = "org.testcontainers:testcontainers-bom", version.ref = "testcontainers" }

# Versionless entries — version supplied by a BOM at resolution time
spring-boot-starter-web = { module = "org.springframework.boot:spring-boot-starter-web" }
spring-boot-starter-actuator = { module = "org.springframework.boot:spring-boot-starter-actuator" }
testcontainers-postgresql = { module = "org.testcontainers:postgresql" }
kafka-clients = { module = "org.apache.kafka:kafka-clients" }

# Versioned entries — libraries no BOM manages
mapstruct = { module = "org.mapstruct:mapstruct", version.ref = "mapstruct" }
mapstruct-processor = { module = "org.mapstruct:mapstruct-processor", version.ref = "mapstruct" }
resilience4j-spring-boot3 = { module = "io.github.resilience4j:resilience4j-spring-boot3", version.ref = "resilience4j" }

[bundles]
observability = ["spring-boot-starter-actuator"]

[plugins]
spring-boot = { id = "org.springframework.boot", version.ref = "spring-boot" }
spring-dep-mgmt = { id = "io.spring.dependency-management", version = "1.1.7" }
```

```kotlin
// build.gradle.kts
plugins {
    java
    alias(libs.plugins.spring.boot)
}

dependencies {
    implementation(platform(libs.spring.boot.bom))
    implementation(platform(libs.spring.cloud.bom))
    testImplementation(platform(libs.testcontainers.bom))

    implementation(libs.spring.boot.starter.web)
    implementation(libs.resilience4j.spring.boot3)
    implementation(libs.mapstruct)
    annotationProcessor(libs.mapstruct.processor)
    testImplementation(libs.testcontainers.postgresql)
}
```

Rules:

- No version strings in `build.gradle.kts` files. Ever. The catalog is the one place.
- Dash-separated catalog keys become dot-accessors (`spring-boot-starter-web` → `libs.spring.boot.starter.web`).
- In multi-project builds the catalog is automatically shared from the root — subprojects just use `libs`.

## Recipe 2 — platform() vs enforcedPlatform()

```kotlin
dependencies {
    implementation(platform(libs.spring.boot.bom))          // constraints; conflict resolution can upgrade
    // implementation(enforcedPlatform(libs.spring.boot.bom)) // forces; can DOWNGRADE transitives
}
```

- `platform()` — BOM versions become **constraints**. Gradle's resolution (highest version wins) still applies, so a transitive that needs a newer patch gets it. This is the right default.
- `enforcedPlatform()` — BOM versions are **forced**, overriding everything, including downgrading a security bump a transitive carried in. Only use when you must pin an entire family exactly (and write down why).
- Targeted override without enforcedPlatform:

```kotlin
dependencies {
    constraints {
        implementation("org.apache.kafka:kafka-clients:3.6.2") {
            because("KAFKA-XXXXX corruption fix; remove at Boot 3.4.x")
        }
    }
}
```

## Recipe 3 — Dependency locking

Makes resolution reproducible: CI builds exactly what you reviewed, and any transitive drift
shows up as a lockfile diff in the PR.

```kotlin
// build.gradle.kts (root, applies to all projects via allprojects/convention plugin)
dependencyLocking {
    lockAllConfigurations()
    lockMode.set(LockMode.STRICT)    // resolution fails if a dep isn't in the lock state
}
```

```bash
./gradlew dependencies --write-locks            # generate/refresh gradle.lockfile per project
./gradlew check                                  # now resolves strictly against lockfiles
./gradlew classes --update-locks org.apache.kafka:kafka-clients   # surgical single-dep update
```

Commit `gradle.lockfile` files. Workflow: any intentional version change regenerates locks in the
same PR; an *unexplained* lockfile diff is a red flag — run `dependencyInsight` before approving.

`LockMode.LENIENT` exists for migration periods; move to `STRICT` once lockfiles are stable.

## Recipe 4 — Triage commands

```bash
# Full resolved tree for the runtime classpath
./gradlew dependencies --configuration runtimeClasspath

# WHY did jackson-databind resolve to this version? (constraint? conflict? rule?)
./gradlew dependencyInsight --dependency jackson-databind --configuration runtimeClasspath

# Same for a build with multiple projects
./gradlew :orders-service:dependencyInsight --dependency netty --configuration runtimeClasspath
```

Reading `dependencyInsight` output:

```text
com.fasterxml.jackson.core:jackson-databind:2.17.2
  Variant runtimeElements
  Selection reasons:
    - By constraint: platform org.springframework.boot:spring-boot-dependencies:3.4.5
    - By conflict resolution: between versions 2.17.2 and 2.16.0
```

- `By constraint: platform ...` — the BOM did its job.
- `By conflict resolution` — two requesters disagreed; Gradle picked the highest. Fine if intentional; add a constraint if you need a different version.
- `By ancestor` / `Forced` — someone used `force` or `enforcedPlatform` — find it and justify or remove it.

## Recipe 5 — Failing fast on bad states

```kotlin
configurations.all {
    resolutionStrategy {
        failOnNonReproducibleResolution()      // bans dynamic versions and changing modules
        // failOnVersionConflict()             // Maven-style strictness; pairs poorly with BOM
                                               // constraint upgrades — prefer locking instead
    }
}
```

`failOnVersionConflict()` is the closest analog to Maven's `dependencyConvergence`, but with
BOM-driven builds it fires constantly on benign constraint-vs-transitive differences. Dependency
locking + PR review of lockfile diffs achieves the same audit goal with far less friction —
prefer it.

CVE scanning: apply `org.owasp.dependencycheck` Gradle plugin, or run your platform's scanner
against the lockfiles (a side benefit of locking — scanners see the exact resolved graph).

```kotlin
plugins { id("org.owasp.dependencycheck") version "12.1.1" }
dependencyCheck {
    failBuildOnCVSS = 7.0f
    suppressionFile = "owasp-suppressions.xml"   // each suppression: reason + expiry date
}
```
