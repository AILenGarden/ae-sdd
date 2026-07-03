# 2026-07-03 ae-sdd Monitor desktop app

## Change

- Added `apps/ae-sdd-monitor`, an Electron-based read-only desktop monitor for ae-sdd workspaces.
- The app scans a selected parent directory for child workspaces containing `.ae-sdd/`.
- The left pane lists discovered workspaces as a collapsible project/task tree with derived status, phase, Memory status, project key, and last activity.
- The right pane shows selected project/task state, timeline, Memory, work items, runtime statistics, and raw `state.json`.
- Added a Windows packaging script that builds an installable zip with `install.ps1` and `uninstall.ps1`.
- Added `source/docs/ae-sdd-monitor-design.md` as the dedicated Monitor design contract.
- Linked Monitor into `source/docs/ae-sdd-design.md` and `source/docs/ae-sdd-implementation-architecture.md` so it remains a tracked ae-sdd read-only projection layer.
- Added update graph rule `UG-22` to cascade ae-sdd state/memory/runtime/design changes into Monitor docs, parser code, tests, and README.
- Updated `UC-14` to read source-slim fallback content when checking `ae-sdd-update-skill.md` graph anchors.
- Improved first-use interaction: directory selection now shows immediate feedback, scanning status, cancel state, and failure state.
- Added persistent UI preferences under Electron `userData/preferences.json` for the last parent directory, selected workspace, selected task, collapsed project groups, auto-refresh setting, and theme.
- Replaced decorative Mac-style dots with real window controls and hid the default Electron menu frame.
- Extended workspace detail with a phase axis that shows the complete scale-specific phase chain and current node explanation.
- Added active work item aggregation across root state fields, `activeAgents[]`, and unfinished `.auto-engineering/{workItemKey}/state.json` entries.
- Aligned Monitor work item projection with the current `workItemId` / `workItemName` / `workItemKey` design, including `{ID}--{name}` directory fallback and `activeStatePath` display.
- Changed Monitor navigation to a two-level project/task sidebar and a two-level current project/current task board.
- Added per-project collapse/expand behavior in the sidebar and persisted the collapsed groups with the rest of the Monitor preferences.
- Added iOS-style lightweight interaction motion for button presses, project/task collapse, tab/detail transitions, card/list hover states, and reduced-motion fallback.
- Documented the Monitor motion boundary in the Monitor design, ae-sdd main design, and implementation architecture: animation is renderer/CSS-only UI feedback and remains read-only.
- Added read-only Memory projection from `.ae-sdd/memory/**/*.jsonl` and `.ae-sdd/memory/.stage/*.json`, including project/task Memory status, active scopes, and blocked scopes.
- Replaced visible polling refresh with responsive file watching: `.ae-sdd/` and `.auto-engineering/` changes trigger quiet updates, unchanged data is not re-rendered, and low-frequency polling remains only as a fallback.
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
