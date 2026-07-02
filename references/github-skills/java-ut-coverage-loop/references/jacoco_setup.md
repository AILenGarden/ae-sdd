# JaCoCo Setup (when the project has no coverage plugin yet)

The skill needs `target/site/jacoco/jacoco.xml` to read coverage. If
`detect_project.py` reports `has_jacoco_plugin: false` for the relevant module,
the user must add the plugin to that module's `pom.xml` before the loop can run.

## Required: ask the user before editing pom.xml

Adding JaCoCo modifies build configuration — that is **not** test code. The
skill's hard rule is "never change non-test code without explicit consent."
**Stop and ask the user** before editing the pom. Show them the exact diff
you intend to apply.

If the user says no, suggest two fallbacks:
1. They add the plugin themselves and re-invoke the skill.
2. They run the loop in a project that already has JaCoCo configured.

## Drop-in plugin block

Use `assets/jacoco-pom-snippet.xml`. It produces both XML (machine-readable,
required by the skill) and HTML (human-readable, optional) reports.

Place it inside `<project> <build> <plugins> ... </plugins> </build>`. If
`<build>` or `<plugins>` doesn't exist, create them.

## Verification after install

```bash
cd <project-root>          # or the relevant module
mvn test jacoco:report
test -f target/site/jacoco/jacoco.xml && echo OK
```

The first `mvn test jacoco:report` after adding the plugin may need a
`mvn clean` first if the project has cached failsafe state.

## Common surefire conflicts

JaCoCo injects its agent via `argLine`. If the project already sets
`argLine` in surefire (e.g. for `--add-opens` flags), JaCoCo's argLine will
be lost unless the project uses `${argLine}` in its surefire config:

```xml
<plugin>
  <artifactId>maven-surefire-plugin</artifactId>
  <configuration>
    <argLine>@{argLine} --add-opens=java.base/java.lang=ALL-UNNAMED</argLine>
  </configuration>
</plugin>
```

`@{argLine}` is the late-binding form that picks up JaCoCo's contribution.
If you see "JaCoCo agent did not record any data" or coverage of 0.0% on
every class, this is usually the cause.

## JDK version

JaCoCo 0.8.11+ supports up to Java 21. If the project uses Java 22+, bump
the plugin to 0.8.12 or newer.
