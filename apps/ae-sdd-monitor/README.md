# ae-sdd Monitor

Desktop monitor for local ae-sdd workspaces.

The desktop shell is Electron. The renderer is React + TypeScript built by Vite; Electron keeps the local filesystem and window-control responsibilities, while React owns the project/task UI state.

## What It Does

- Select a parent directory and scan for all child workspaces containing `.ae-sdd/`.
- Show each workspace on the left as a collapsible project/task tree; clicking a project switches the project board, clicking a task switches the task board.
- Show the selected project/task on the right with state, phase axis, event stream, Memory status, active work items, all work items, runtime stats, and raw state.
- Switch projects/tasks through React state and keyed components instead of clearing and rebuilding the sidebar or detail page.
- Read data locally from `.ae-sdd/state.json`, `.auto-engineering/{workItemKey}/state.json`, `.ae-sdd/memory/**/*.jsonl`, `.ae-sdd/memory/.stage/*.json`, and `.ae-sdd/runtime-stats/*.jsonl`.
- Display work item identity as `workItemId`, `workItemName`, and `workItemKey`; state fields win, with `{ID}--{name}` directory names as a compatibility fallback.
- Responsive refresh by default: filesystem changes under `.ae-sdd/` and `.auto-engineering/` trigger quiet updates, with low-frequency polling only as a fallback.
- Restore the last parent directory, selected workspace, selected task, collapsed project groups, auto-refresh setting, and theme when the app opens again.
- Use real window controls in the Mac-style title bar.
- Use iOS-style lightweight interaction motion for presses, collapses, tab/detail transitions, and card/list hover states, while avoiding persistent loading/status flashing.

The app is intentionally read-only. It does not mutate ae-sdd state or run gates.

## Design Contract

The Monitor design is maintained in `source/docs/ae-sdd-monitor-design.md`.

It tracks the ae-sdd capability and implementation docs:

- `source/docs/ae-sdd-design.md` for phase, gate, state, and user-visible workflow semantics.
- `source/docs/ae-sdd-implementation-architecture.md` for `.ae-sdd/`, `.auto-engineering/`, and Runtime Stats storage rules.
- `source/standards/update-graph.json` rule `UG-22` for the machine-readable sync closure.

When ae-sdd state, phase flow, Memory, Runtime Stats, or workspace storage contracts change, update `src/workspace.js`, `test/workspace.test.js`, this README, and the Monitor design doc together.
When renderer behavior changes, update `renderer/src/**`, `src/styles.css`, this README, and the Monitor design doc together.

## Development

```powershell
npm install
npm run typecheck
npm run build:renderer
npm test
npm start
```

## Build Installer

Windows:

```powershell
npm run dist:win
```

macOS:

```bash
npm run dist:mac
# or
bash scripts/package-mac.sh
```

Unsigned macOS app zip from Windows or macOS:

```powershell
npm run dist:mac:unsigned
```

The Windows packages are written to `release/`:

```text
release/ae-sdd-monitor-0.1.0-windows-x64-setup.exe
release/ae-sdd-monitor-0.1.0-windows-x64-installable.zip
```

The macOS packages are written to `release/` by electron-builder:

```text
release/ae-sdd Monitor-0.1.0-mac-x64.dmg
release/ae-sdd Monitor-0.1.0-mac-x64.zip
release/ae-sdd Monitor-0.1.0-mac-arm64.dmg
release/ae-sdd Monitor-0.1.0-mac-arm64.zip
```

The unsigned cross-platform macOS packages are written to `release/`:

```text
release/ae-sdd-monitor-0.1.0-macos-x64-unsigned.zip
release/ae-sdd-monitor-0.1.0-macos-arm64-unsigned.zip
```

`dist:mac` must run on macOS. The Windows build can prepare the Windows exe/zip and the unsigned macOS `.app.zip`; final macOS `.app`/`.dmg` verification and signing require a macOS runner.

The setup executable is a self-extracting installer. Run it directly to install the app under `%LOCALAPPDATA%\Programs\ae-sdd Monitor` and create a Start Menu shortcut.

To install from the zip:

```powershell
Expand-Archive .\ae-sdd-monitor-0.1.0-windows-x64-installable.zip
cd .\ae-sdd-monitor-0.1.0-windows-x64-installable
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

The installer copies the app to `%LOCALAPPDATA%\Programs\ae-sdd Monitor` and creates a Start Menu shortcut. Add `-DesktopShortcut` to create a desktop shortcut.
