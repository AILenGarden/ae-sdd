# 2026-07-03 ae-sdd Monitor desktop app

## Change

- Added `apps/ae-sdd-monitor`, an Electron-based read-only desktop monitor for ae-sdd workspaces.
- The app scans a selected parent directory for child workspaces containing `.ae-sdd/`.
- The left pane lists discovered workspaces with derived status, phase, project key, and last activity.
- The right pane shows selected workspace state, timeline, work items, runtime statistics, and raw `state.json`.
- Added a Windows packaging script that builds an installable zip with `install.ps1` and `uninstall.ps1`.
- Added `source/docs/ae-sdd-monitor-design.md` as the dedicated Monitor design contract.
- Linked Monitor into `source/docs/ae-sdd-design.md` and `source/docs/ae-sdd-implementation-architecture.md` so it remains a tracked ae-sdd read-only projection layer.
- Added update graph rule `UG-22` to cascade ae-sdd state/runtime/design changes into Monitor docs, parser code, tests, and README.
- Updated `UC-14` to read source-slim fallback content when checking `ae-sdd-update-skill.md` graph anchors.
- Improved first-use interaction: directory selection now shows immediate feedback, scanning status, cancel state, and failure state.
- Added persistent UI preferences under Electron `userData/preferences.json` for the last parent directory, selected workspace, and theme.
- Replaced decorative Mac-style dots with real window controls and hid the default Electron menu frame.
- Extended workspace detail with a phase axis that shows the complete scale-specific phase chain and current node explanation.
- Added active work item aggregation across root state fields, `activeAgents[]`, and unfinished `.auto-engineering/{workItemKey}/state.json` entries.
- Aligned Monitor work item projection with the current `workItemId` / `workItemName` / `workItemKey` design, including `{ID}--{name}` directory fallback and `activeStatePath` display.
- Added macOS packaging configuration and `scripts/package-mac.sh` for dmg/zip builds on macOS.
- Added `scripts/package-mac-unsigned.ps1` and `npm run dist:mac:unsigned` to generate unsigned macOS `.app.zip` artifacts from Windows/macOS.

## Verification

- `npm test`
- `npm run pack:dir`
- `npm run dist`
- Expanded `release/ae-sdd-monitor-0.1.0-windows-x64-installable.zip` and verified required files.
- Launched packaged `ae-sdd Monitor.exe` and confirmed the process starts.
- `python scripts/slim_source_skills.py --validate --json`
- `python -m unittest tools.tests.test_update_graph.TestUC14 tools.tests.test_update_graph.TestQueryAffected tools.tests.test_update_graph.TestUpdateCheckCli tools.tests.test_update_graph.TestSyncManifest`
- `node --check src/main.js`
- `node --check src/preload.js`
- `node --check src/renderer.js`
