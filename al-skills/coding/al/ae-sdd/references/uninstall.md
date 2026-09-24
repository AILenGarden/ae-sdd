# ae-sdd uninstall

Remove files owned by the ae-sdd installation procedure from the selected Agent runtime, including the entry Skill. Preserve independently installed capabilities and user data.

Bundled al-knowledge resources are part of ae-sdd's owned payload and are removed with it when their recorded hashes still match. Preserve project `.al-knowledge/` data and historical standalone knowledge installations; they are not the bundled capability's storage or ownership scope.

## Required context

Read the shared installation contract and [agents-guidance.md](agents-guidance.md) before planning. Both shared documents are beside this procedure in `references/`. Resolve the Agent/root and selected AGENTS.md scope, then read its installation record. The record is ownership evidence, not permission to trust arbitrary paths.

Read [manage_agents.py](../scripts/manage_agents.py) before removing the plugin containing it. Native host package ownership and the separate guidance.json ownership are distinct. A host-managed plugin is removed through its host CLI; do not invent a manual file manifest or infer filesystem deletion authority for it.

## Procedure

1. For native plugins, confirm the selected host installation; for manually deployed packages, validate manifest.json schema, Agent/root and recorded paths. Validate guidance.json independently with explicit AGENTS/mirror targets. Missing or invalid ownership records never authorize guessed filesystem deletion; already absent recorded files are harmless.
2. Compare current hashes with recorded hashes. List removable files, locally modified files, untracked additions, shared/independent packages and already absent files. Preserve modified or externally owned packages pending a separate user decision; continue with unrelated removable packages.
3. Present a dry-run with exact absolute targets, ownership evidence, preserved paths and an external backup location. A dry-run-only request makes no changes. Apply when existing authorization covers this list; ask only for unresolved scope or destructive conflicts.
4. Before removing the plugin, run the guidance command below as dry-run, then apply the unchanged authorized plan. The helper removes only its matching ae-sdd-entry region and guidance record, preserving other rules and synchronizing recorded mirrors. Modified, malformed or unowned regions are preserved. Resolve guidance conflicts before removing the plugin, unless the user explicitly chooses to retain that guidance.
5. Recheck ownership, paths and hashes, then remove the native plugin through its host CLI, or move manually deployed owned files into the verified backup location. The guidance helper has already backed up and edited AGENTS.md; do not move or delete the remaining file. Never recursively delete a package directory containing unknown files. Load needed instructions before removing the entry and resources.
6. Update the installation record after successful removals. Retain entries for unresolved managed files and the uninstall procedure/resources needed to retry; remove the active record only when no managed files remain. Keep backups outside runtime discovery. On failure restore affected files and the prior record, reporting incomplete recovery.
7. Verify removed entries are absent and the managed marker is absent from the selected AGENTS.md while all unmarked content remains byte-identical. Report preserved independent capabilities and any partial removal.

Repeated uninstall with no record and no suite entries is a no-op. With no record but matching entries, preserve those entries as unowned. Project `.al-knowledge`, documents, source code, database services/registrations/credentials, unrelated Skills and Agent configuration are outside the uninstall scope.

## Guidance command

```text
python <package-root>/scripts/manage_agents.py uninstall --agents <AGENTS.md> --mirror <mirror-AGENTS.md> --record <state-root>/.ae-sdd-install/guidance.json
```

Omit --mirror only if none is recorded. Apply with `--apply --backup-dir <external-backup-directory>`. If native plugin removal fails after guidance removal, report the split state and restore guidance from the still-installed template/helper when the plugin remains in use. This is not a multi-resource transaction. Direct host UI/CLI removal does not trigger this cleanup; reconcile with the source helper and recorded targets, never guessed deletion ranges.
