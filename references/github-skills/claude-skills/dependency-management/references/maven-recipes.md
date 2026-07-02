# Maven Recipes

Complete, copyable pom configurations. Spring Boot 3.x / Java 21 baseline.

## Recipe 1 — spring-boot-starter-parent (the default)

```xml
<parent>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-parent</artifactId>
    <version>3.4.5</version>
    <relativePath/>
</parent>

<properties>
    <java.version>21</java.version>
    <!-- Boot-managed version overrides go here, ONLY via Boot's documented properties,
         each with a reason + removal condition: -->
    <!-- <jackson-bom.version>2.17.2</jackson-bom.version>  CVE-2024-XXXX, remove at Boot 3.4.6 -->
</properties>
```

You get: managed versions for ~1000 artifacts, plugin management (surefire, failsafe, compiler,
jar), `repackage` goal defaults, resource filtering with `@..@` delimiters.

## Recipe 2 — BOM-only import (corporate parent occupies the parent slot)

```xml
<parent>
    <groupId>com.example.platform</groupId>
    <artifactId>corporate-parent</artifactId>
    <version>7.2.0</version>
</parent>

<dependencyManagement>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-dependencies</artifactId>
            <version>3.4.5</version>
            <type>pom</type>
            <scope>import</scope>
        </dependency>
        <!-- Order matters: first import wins on overlap. Put the BOM you trust most first. -->
        <dependency>
            <groupId>org.testcontainers</groupId>
            <artifactId>testcontainers-bom</artifactId>
            <version>1.20.6</version>
            <type>pom</type>
            <scope>import</scope>
        </dependency>
    </dependencies>
</dependencyManagement>
```

Two things the parent gives that BOM-only does NOT — you must add them yourself:

1. Plugin management (compiler release, surefire/failsafe versions, spring-boot-maven-plugin with `repackage`).
2. Version-property overrides (`jackson-bom.version` etc.) — they do **not** work with BOM import; to override a managed version you must declare the artifact's BOM/artifact in `dependencyManagement` *before* the Boot BOM import.

```xml
<build>
    <plugins>
        <plugin>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-maven-plugin</artifactId>
            <version>3.4.5</version>
            <executions>
                <execution>
                    <goals><goal>repackage</goal></goals>
                </execution>
            </executions>
        </plugin>
    </plugins>
</build>
```

## Recipe 3 — Enforcer plugin (CI gate)

```xml
<plugin>
    <groupId>org.apache.maven.plugins</groupId>
    <artifactId>maven-enforcer-plugin</artifactId>
    <version>3.5.0</version>
    <executions>
        <execution>
            <id>enforce-dependency-discipline</id>
            <phase>validate</phase>
            <goals><goal>enforce</goal></goals>
            <configuration>
                <rules>
                    <requireMavenVersion><version>[3.9,)</version></requireMavenVersion>
                    <requireJavaVersion><version>[21,22)</version></requireJavaVersion>
                    <dependencyConvergence/>
                    <requireUpperBoundDeps/>
                    <banDuplicatePomDependencyVersions/>
                    <bannedDependencies>
                        <excludes>
                            <exclude>commons-logging:commons-logging</exclude>   <!-- Boot uses spring-jcl -->
                            <exclude>log4j:log4j</exclude>                        <!-- log4j 1.x, EOL -->
                            <exclude>org.apache.logging.log4j:log4j-core:(,2.17.1)</exclude>
                            <exclude>javax.servlet:*</exclude>                    <!-- jakarta only on Boot 3 -->
                            <exclude>javax.persistence:*</exclude>
                            <exclude>*:*:*:*:*:compile</exclude>
                        </excludes>
                        <includes>
                            <include>*:*:*:*:*:compile</include>   <!-- adjust ban list to taste; this pattern
                                                                        pair lets you whitelist explicitly -->
                        </includes>
                    </bannedDependencies>
                </rules>
                <fail>true</fail>
            </configuration>
        </execution>
    </executions>
</plugin>
```

Notes:

- `dependencyConvergence` is strict and noisy on first adoption — fix divergences by adding entries to `dependencyManagement`, not by removing the rule. Adopt module-by-module in a large legacy multi-module build.
- The include/exclude pattern trick above is optional; the simple form is just the `<excludes>` list without `<includes>`.

## Recipe 4 — Multi-module version management

Root pom owns ALL versions; child modules contain zero `<version>` elements for dependencies.

```xml
<!-- root pom.xml -->
<properties>
    <mapstruct.version>1.6.3</mapstruct.version>
    <shedlock.version>5.16.0</shedlock.version>
</properties>

<dependencyManagement>
    <dependencies>
        <!-- internal modules, so siblings reference each other versionless -->
        <dependency>
            <groupId>${project.groupId}</groupId>
            <artifactId>orders-domain</artifactId>
            <version>${project.version}</version>
        </dependency>
        <!-- third-party not covered by imported BOMs -->
        <dependency>
            <groupId>org.mapstruct</groupId>
            <artifactId>mapstruct</artifactId>
            <version>${mapstruct.version}</version>
        </dependency>
    </dependencies>
</dependencyManagement>
```

```xml
<!-- child module: versionless everywhere -->
<dependencies>
    <dependency>
        <groupId>${project.groupId}</groupId>
        <artifactId>orders-domain</artifactId>
    </dependency>
    <dependency>
        <groupId>org.mapstruct</groupId>
        <artifactId>mapstruct</artifactId>
    </dependency>
</dependencies>
```

Use the `flatten-maven-plugin` (`flattenMode: resolveCiFriendliesOnly`) if you adopt
`${revision}` CI-friendly versions.

## Recipe 5 — Upgrade and audit tooling

```xml
<!-- versions-maven-plugin: keep rules out of the way of BOM-managed deps -->
<plugin>
    <groupId>org.codehaus.mojo</groupId>
    <artifactId>versions-maven-plugin</artifactId>
    <version>2.18.0</version>
    <configuration>
        <ignoredVersions>.*-(alpha|beta|M\d|RC\d).*</ignoredVersions>
    </configuration>
</plugin>

<!-- OWASP dependency-check: CVE gate in CI -->
<plugin>
    <groupId>org.owasp</groupId>
    <artifactId>dependency-check-maven</artifactId>
    <version>12.1.1</version>
    <configuration>
        <failBuildOnCVSS>7.0</failBuildOnCVSS>
        <suppressionFiles>
            <suppressionFile>owasp-suppressions.xml</suppressionFile>  <!-- every entry needs a reason + expiry -->
        </suppressionFiles>
        <nvdApiKeyEnvironmentVariable>NVD_API_KEY</nvdApiKeyEnvironmentVariable>
    </configuration>
    <executions>
        <execution><goals><goal>check</goal></goals></execution>
    </executions>
</plugin>
```

Daily-driver commands:

```bash
mvn versions:display-dependency-updates     # stale direct deps
mvn versions:display-property-updates       # stale version properties
mvn versions:set -DnewVersion=1.4.0         # bump project version (multi-module aware)
mvn org.owasp:dependency-check-maven:check
```

## Triage cheat-sheet

```bash
# Who pulls in artifact X, and which version won?
mvn dependency:tree -Dverbose -Dincludes='com.fasterxml.jackson.core:*'

# Full tree of one module in a multi-module build
mvn dependency:tree -pl orders-service -am

# Classpath as the JVM will see it
mvn dependency:build-classpath -Dmdep.outputFile=cp.txt

# Effective pom — see what dependencyManagement actually resolved to
mvn help:effective-pom -Doutput=effective.xml
```

Reading `-Dverbose` output: `(omitted for conflict with X)` = lost nearest-wins; `(omitted for
duplicate)` = same version, harmless; `(version managed from Y)` = dependencyManagement/BOM did
its job. The fix for a wrong winner is always a `dependencyManagement` entry, never reordering
`<dependencies>` (order-dependence is a build smell that bites the next refactor).
