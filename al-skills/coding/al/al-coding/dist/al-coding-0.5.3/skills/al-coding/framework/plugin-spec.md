# ALCoding Registry Contract

## Registry Shape

`framework/registry.yaml` is the registration table. Each record identifies one
SKILL and its applicability:

```yaml
schema_version: 3
skills:
  - id: java-spring-coding
    name: Java Spring 编码适配
    type: language
    scope: global
    path: plugins/language/java-spring-coding/SKILL.md
    enabled: true
    description: Java and Spring implementation guidance
    tags: [java, spring, coding]
```

Required fields are `id`, `name`, `type`, `scope`, `path`, and `enabled`.
`type` is one of `global`, `language`, `architecture`, or `project`.
`scope` is `global` or `project`; a project-scoped record has a stable
`project_id`. `id` and `name` are unique within the table, ignoring case.
Optional fields are `description`, `tags`, and opaque `metadata`.

## Selection

The Agent reads the table, keeps enabled entries, and groups them by type. A
global entry can participate in every project. A project entry participates only
when its `project_id` equals the confirmed project identity. Language and
architecture entries participate when the actual stack or design matches them.
The Agent then reads the selected SKILL bodies and relevant references.

Registered skills supplement the Core. A registered skill does not silently
override a Core rule or another applicable standard. A conflict records the
sources, applicable scopes, affected decision, and explicit resolution before a
review conclusion or design decision relies on it.

Project identity comes from confirmed project documentation, repository
constraints, or an explicit user statement. Similar names are not identity
matches. When identity is unknown, use applicable global skills and mark
project-specific conclusions as limited.

## Path Handling

| Path | Resolution |
|---|---|
| Relative | Resolve from the skill root containing `SKILL.md`, one directory above `framework/`. |
| Absolute | Read the recorded location when it exists; report the entry and path when it is unavailable. |
| Host resource reference | Use the host resource mechanism and report an unavailable required resource. |

The current `java-ddd-naming` entry remains a project-scoped external skill with
its recorded project identity. Deployment to another machine requires updating
that external path to the real location before using the project rule.

## Changes

Registration changes edit only `framework/registry.yaml` and preserve existing
records. Before changing a table, read its current contents. Duplicate identity
or conflicting concurrent edits require resolving the specific record; unrelated
records remain unchanged. A request that already matches the target record is
idempotent.

Core changes update the relevant Core file, its version metadata, and a changelog
entry. Adaptation changes update the owning language, architecture, or project
SKILL and its references.
