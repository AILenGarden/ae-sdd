# ae-sdd install

Install or update ae-sdd with its bundled knowledge capability. A full-suite request additionally installs or reuses the other four capabilities in the catalog. Installing the ae-sdd plugin alone must already provide project knowledge; it must not require an independent al-knowledge install.

## Required context

Read the shared installation contract and [agents-guidance.md](agents-guidance.md) before planning. Both shared documents are beside this procedure in `references/`. Missing either contract means an incomplete package; do not infer ownership or deployment paths.

AGENTS synchronization is required for both installation and update. Use [manage_agents.py](../scripts/manage_agents.py) and the single template above. Only the ae-sdd entry region is plugin-owned; general ALSDD/Git/user rules remain untouched.

Resolve the selected Agent, its actual Skill search root, the ae-sdd source checkout/distribution, and explicit source locations for any missing capabilities. Use the user's choices and current host configuration; ask only for information that cannot be resolved. Do not install into every Agent or pick a duplicate source by directory order.

## Procedure

1. Inventory the target and its installation record. Classify packages as managed, independently installed, absent, or conflicting using the shared contract. For db-operator, the suite is incomplete until its native client, daemon service, and Agent client capability are provisioned and status verification succeeds. On Windows, the LLM must invoke the bundled `scripts/install-runtime.ps1` from a foreground terminal or desktop-automation session when UAC may be required; do not launch it through a detached/background command runner. the script self-elevates through the normal UAC flow, installs or updates the service, issues the current user's client capability, and verifies the runtime. Do not hand the command back to the user. If the host cannot surface foreground UAC, report that host limitation explicitly; do not substitute manual service commands or bypass UAC. Do not leave a partial manual setup.
2. Validate sources, frontmatter, required resource closure and the source-to-target mapping. Check the bundled knowledge payload, its CAPABILITY.md and scripts/templates/references; do not substitute a standalone al-knowledge installation. For a full-suite request, resolve the other four packages or reuse valid independent installations. Report missing requested capabilities before writing; a partial set is not a full-suite installation.
3. Present the dry-run: Agent/root, source and target absolute paths, package version when available and content digest, file additions/updates/removals, reused independent packages, conflicts and backup location. A dry-run-only request ends here without filesystem changes.
4. Apply when existing user authorization covers the concrete plan. Obtain a decision for unresolved targets, taking over independent packages or destructive conflicts; do not ask again for an already authorized unchanged plan. Recheck source/target hashes before applying.
5. Resolve the selected global/project AGENTS.md and explicitly registered mirrors. Run the guidance command below as dry-run before changing the host plugin. Resolve conflicts first. An unmarked section remains unowned unless the user authorized taking this reviewed section into installation management; then pass its exact digest with --adopt-sha256. Do not take over an old aggregate guidance block or general rules by heading similarity.
6. Back up and apply plugin/package changes through the normal host process and shared contract. After plugin install/update succeeds, apply the guidance plan with --apply and an external --backup-dir. The helper rechecks files, preserves outside bytes, synchronizes selected targets and writes guidance.json. A helper failure leaves installation incomplete: report plugin status and remaining guidance work separately; do not claim a fully synchronized install.
7. Verify ae-sdd is the public plugin entry and its private knowledge payload is complete. For a full-suite request also verify the other four packages, exactly one managed marker pair in the selected AGENTS.md when guidance was installed, and db-operator runtime status when that capability is installed. Report managed/reused/missing packages, changed paths, version or digest and backup location.

An identical source and target is a no-op, including the installation record. Updates may remove an obsolete managed file only if its current hash still matches the record. Never mirror-delete unknown files or execute a capability's service/setup scripts as part of Skill deployment.

## Guidance command

```text
python <package-root>/scripts/manage_agents.py install --agents <AGENTS.md> --mirror <mirror-AGENTS.md> --record <state-root>/.ae-sdd-install/guidance.json
```

Omit --mirror if none is registered. Apply the reviewed plan by adding `--apply --backup-dir <external-backup-directory>`; reuse existing scope authorization. Record state outside the removable plugin cache. This procedure does not register a native host installation hook: raw host UI/CLI operations alone do not synchronize AGENTS.md. Use this ae-sdd procedure for the complete lifecycle.
