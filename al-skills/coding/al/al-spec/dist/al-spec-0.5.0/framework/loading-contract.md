# ALSpec Loading Contract

The calling Agent must load specification context deliberately. The registry
does not choose a document or merge layers on the Agent's behalf.

## Authoritative Sources

1. The selected type `skills/<type>/SKILL.md` is the authoritative writing
   specification for that type. It is self-contained and does not defer its
   normative rules to a separate `*-writing.md` file.
2. The selected type template is the required canonical output context. A
   project template may be selected by exact project ID without creating a
   project SKILL. For DR, Story, and TestCase, the global type SKILL is always
   used; a mapped project template replaces the global template, otherwise the
   global template is the fallback. Other project SKILLs are used only when
   explicitly registered.
3. `framework/registry.yaml` is the only discovery table.
4. A matching `scope: project` registration is a project layer only for types
   that explicitly register a project SKILL. DR, Story, and TestCase currently
   have no project SKILL; their project variations are represented by template
   mappings under the global type entries.

The files under `standards/` are maintainer/reference material. They may help
authors maintain embedded rules, but they are not a runtime prerequisite and
must not be treated as a second normative SKILL.

## Load Order

```text
registry.yaml (global entries + project template mappings + explicit project SKILLs)
  -> resolve project ID and selected type
  -> selected global type SKILL
  -> mapped project template OR global template fallback
  -> explicit project SKILL only when that type registered one
  -> optional related specifications, when provided
  -> context note and draft/review
```

The Agent must record loaded paths and the reason each project entry applies.
Without a confirmed repository and stable project ID, load the global type SKILL
and its global template. With a confirmed project ID, select a mapped project
template when one exists. Never infer a project SKILL from a project template;
an explicit project SKILL, where one exists for another type, must still never
create a prerequisite on another spec type.

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
    metadata:
      template: templates/global/story.md
      project_templates:
        billing: templates/project/billing-story.md
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

If a global entry has a project-template mapping, show the mapping and select
the mapped template for the confirmed `project_id`; do not create or infer a
project SKILL. If an explicit project SKILL is registered for a type, follow its
own contract, but do not silently merge two writing standards. For DR, Story,
and TestCase, project variation is template-only.

The three current types are independent. A DR, Story, or TestCase can be
created and reviewed without any other type being present. Cross-document
references may be recorded when useful, but they are optional context and never
part of the readiness gate.
