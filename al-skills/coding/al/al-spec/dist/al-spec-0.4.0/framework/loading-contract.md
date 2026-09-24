# ALSpec Loading Contract

The calling Agent must load specification context deliberately. The registry
does not choose a document or merge layers on the Agent's behalf.

## Authoritative Sources

1. The selected type `skills/<type>/SKILL.md` is the authoritative writing
   specification for that type. It is self-contained and does not defer its
   normative rules to a separate `*-writing.md` file.
2. `templates/global/<type>.md` is the required canonical output context for
   one of the three current types: `dr`, `story`, `testcase`.
3. `framework/registry.yaml` is the only discovery table.
4. A matching `scope: project` registration is the project layer for an exact
   `project_id`; its project SKILL and required project template remain
   author-owned.

The files under `standards/` are maintainer/reference material. They may help
authors maintain embedded rules, but they are not a runtime prerequisite and
must not be treated as a second normative SKILL.

## Load Order

```text
registry.yaml (global + exact project_id entries)
  -> selected global skills/<type>/SKILL.md
  -> templates/global/<type>.md
  -> selected project skills/<project-type>/SKILL.md, when exact project_id applies
  -> project template(s) required by that project SKILL
  -> optional related specifications, when provided
  -> context note and draft/review
```

The Agent must record loaded paths and the reason each project entry applies.
Without a confirmed repository and stable project ID, load the global type SKILL
and its required template only. A project SKILL may add stricter rules or a
project template, but it never creates a prerequisite on another spec type.

## Registry Shape

```yaml
schema_version: 2
specs:
  - id: story
    name: Story
    type: story
    scope: global
    path: skills/story/SKILL.md
    enabled: true
  - id: billing-story
    name: Billing Story
    type: story
    scope: project
    project_id: billing
    path: skills/billing-story/SKILL.md
    enabled: true
```

Required fields are `id`, `name`, `type`, `scope`, `path`, and `enabled`.
`project_id` is required exactly when `scope: project`. Optional discovery
metadata is `description`, `tags`, and opaque `metadata`.

## Index Semantics

`registry.py list` returns enabled global entries. With
`--project-id <id>`, it returns enabled global entries plus project entries
whose `project_id` exactly matches `<id>`. `--include-disabled` retains
disabled entries. Project entries for other projects are never returned when a
project ID is supplied. The index annotates `reference_kind` and `available`
but never dereferences a URI or parses a document.

If both a global and project entry describe the same type, the Agent must show
both registrations, load the exact project SKILL after the global SKILL and
global template, and state any narrowing or deviation. The project entry is an
optional project-specific adaptation, not a prerequisite for the global type.
ALSpec does not silently merge or establish precedence between two author-owned
documents; the project SKILL must state its additions and deviations itself.

The three current types are independent. A DR, Story, or TestCase can be
created and reviewed without any other type being present. Cross-document
references may be recorded when useful, but they are optional context and never
part of the readiness gate.
