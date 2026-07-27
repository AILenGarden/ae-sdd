# ae-sdd Daemon Binary Rename Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** 将 daemon 的 Cargo binary target、可执行文件名、命令显示名、CLI 自动发现、服务描述符、发布校验、当前规范与生成分发物，从 `ae-sddd` 全部统一为 `ae-sdd-daemon`。

**Architecture:** 保留现有 package、crate 目录、IPC、endpoint manifest、协议和平台 service identity，仅做 native daemon executable identity 的硬切换。`ae-sdd` 仍是薄 CLI；`ae-sdd-daemon` 仍是 composition root。升级采用完整 release 的 drain/stop/stage/promote/start/handshake/cleanup 流程，不在新版本中保留 `ae-sddd` alias 或 fallback。

**Tech Stack:** Rust 1.97.1 / Cargo workspace / clap / Tokio / Windows Named Pipe / Unix Domain Socket / `ae-sdd-build` release and service tooling / generated SKILL and Harness artifacts.

**Planning status:** 本文件是实现计划，不是 `state.executionPlan`，也不构成用户对 Coding 的批准。实施前仍须完成 ae-sdd Work Item、Story、验证矩阵、G-CODEPLAN-SRC/G-14/G-08 和 executionPlan 用户批准。

---

## 1. Frozen Naming Contract

| Concern | Current | Target | Treatment |
| --- | --- | --- | --- |
| Cargo package | `ae-sdd-daemon` | `ae-sdd-daemon` | 不变 |
| crate directory | `bins/ae-sdd-daemon/` | `bins/ae-sdd-daemon/` | 不变 |
| Cargo binary target | `ae-sddd` | `ae-sdd-daemon` | 修改 `[[bin]].name` |
| executable | `ae-sddd` / `ae-sddd.exe` | `ae-sdd-daemon` / `ae-sdd-daemon.exe` | 硬切换 |
| clap command name / stderr prefix | `ae-sddd` | `ae-sdd-daemon` | 统一用户可见 identity |
| thin client | `ae-sdd` | `ae-sdd` | 不变 |
| Windows task/service name | `ae-sdd-daemon` | `ae-sdd-daemon` | 已正确，不重复改名 |
| Windows descriptor | `ae-sdd-daemon.xml` | `ae-sdd-daemon.xml` | 已正确，仅更新其中 executable argv |
| macOS service identity | `com.ae-sdd.daemon` | `com.ae-sdd.daemon` | 平台 identity，不等同 executable |
| Linux service identity | `ae-sdd.service` | `ae-sdd.service` | 平台 identity，不等同 executable |
| `daemonBuild` / `daemonVersion` product identity | `ae-sdd-runtime/<version>` 或不一致 fixture | `ae-sdd-daemon/<version>` | 外部 identity 统一；wire 字段不变 |
| protocol / endpoint / SQLite schema | 现有 schema | 现有 schema | 不改 schema、不加 migration |
| compatibility alias | `ae-sddd` | 无 | 新版本不发布 alias、不探测旧名 |

### Historical record policy

- 当前实现、测试、约束、README、RELEASING、source SSOT、活跃计划和生成分发物中不得再出现 `ae-sddd`。
- 本命名迁移计划可以把旧 token 作为迁移对象引用；product-surface scan 不将本文件计为产品引用。
- 不重写 immutable evidence、旧 runtime stats、备份文件或已经记录真实旧路径的 manifest。它们必须继续表达当时实际运行的是 `ae-sddd.exe`。
- 既有 PRD/RA/DR/Story 不做无审计的全文替换；通过新的命名迁移 Story 和 DR supplement 声明 supersede 关系。当前约束、source docs 和所有新文档只使用 `ae-sdd-daemon`。
- `target/**`、`dist/.ae-sdd.*-stage-*`、`.harness/*.bak.*` 属于可再生或备份内容；清理后重建，不手改。

## 2. Current Context And Preconditions

- `get_constraints(projectKey=ae-sdd)` 已解析 11 个当前约束文件；与本变更直接相关的是 `technology-stack.md`、`project-structure.md`、`layered-arch.md`、`api.md`、`testing.md`、`security.md` 和 `code-style.md`。
- CodingModel 来源为 `source/standards/thinking/be-coding-thinking-engine.md`，当前 SHA-256 为 `9aa8b72876c82abc9d6db70db5b96bb6c29005e18f09256346a26c5e0bb4ed7f`。
- 当前 active state 是 `DR-AE-SDD-DAEMON-AUTHORITY-001`，phase=`initialized`，没有 active Story，`executionPlan.approved=false`。不得把本计划误当成已批准 executionPlan。
- 工作树已有大量 Plan A/B/C/D 修改，且目标文件 `bins/ae-sdd-cli/src/bootstrap.rs`、`crates/ae-sdd-build/src/release.rs`、`constraints/project-structure.md` 等已经 dirty。实施者必须逐文件读取当前 diff 并做最小合并，禁止覆盖或回滚现有工作。
- `Cargo.lock` 记录的是 package `ae-sdd-daemon`，预计无需语义修改；若 Cargo 自动产生无关 lockfile churn，应停止并定位原因。
- 本机当前还存在 `target/debug|release/ae-sddd.exe` 和 `C:/Users/EDY/AppData/Local/Programs/ae-sdd/ae-sddd.exe`，但没有运行中的旧 daemon 进程或已登记的 `ae-sdd-daemon` Windows task。Cargo target rename 不会自动清理这些旧文件。

## 3. Acceptance Criteria And Verification Matrix

| AC | Contract | Verification |
| --- | --- | --- |
| AC-NAME-01 | Cargo metadata 中 daemon package 的唯一 binary target 为 `ae-sdd-daemon` | `cargo metadata --no-deps --format-version 1` 结构化断言 |
| AC-NAME-02 | clean target 构建只生成 `ae-sdd-daemon[.exe]`，命令帮助和错误前缀均使用新名 | clean `CARGO_TARGET_DIR` build + `ae-sdd-daemon --help` + invalid invocation |
| AC-NAME-03 | `ae-sdd runtime ensure/start` 默认只发现 sibling `ae-sdd-daemon[.exe]`；并发 singleflight、显式 `--daemon` 和 call-first/recover-once 行为不变 | `runtime_autostart` integration tests |
| AC-NAME-04 | 三个平台 service descriptor 均引用新 executable；平台 service identity 保持不变；旧 executable descriptor 被判定为 drifted | `service_lifecycle` tests + descriptor snapshot assertions |
| AC-NAME-05 | release verifier 要求新 binary，拒绝缺失新 binary，也拒绝新旧 binary 同时存在的陈旧 release 目录 | `ae-sdd-build::release` unit tests + `verify-release` smoke |
| AC-NAME-06 | benchmark 默认 daemon、Windows executable permission classification 和协议 fixture 均使用 `ae-sdd-daemon` | focused crate tests |
| AC-NAME-07 | 当前 tracked source、测试、约束、README、RELEASING、source SSOT 和活跃计划无 `ae-sddd`；生成的 `dist/ae-sdd`、Harness 和安装副本也无旧名 | scoped `rg` scans + runtime/harness verification |
| AC-NAME-08 | 旧 service/install 可以 drain、停止并由新 descriptor 拉起新 binary；成功 handshake 后才删除旧 binary；失败回滚完整旧 release | lifecycle upgrade smoke on Windows/macOS/Linux |
| AC-NAME-09 | `daemonBuild`、endpoint manifest `daemonVersion` 对外返回 `ae-sdd-daemon/<version>`；不新增 protocol、endpoint、DB schema 或 migration | protocol/client/runtime/workspace tests |

## 4. Task 0: Establish The Legal ae-sdd Work Item

**Objective:** 在任何实现写入前建立专用、可审计的命名迁移合同。

**Documents / state:**

- Create or bind: `ae-sdd-doc/RA/RA-AE-SDD-DAEMON-NAME-001.md`
- Create: `ae-sdd-doc/DR/DR-AE-SDD-RUST-DAEMON-001-Supplement.md`
- Create or bind: `ae-sdd-doc/Story/STORY-AE-SDD-DAEMON-NAME-001.md`
- Update through typed state operations: `.auto-engineering/<new-work-item>/state.json`

**Steps:**

1. 新建独立 Work Item，引用 `PRD-AE-SDD-RUST-DAEMON-001` 和 `DR-AE-SDD-RUST-DAEMON-001`，不要复用当前未关联的 daemon-authority state。
2. 在 RA 中盘点 binary path、CLI bootstrap、service descriptor、release package、生成分发物、已安装副本和 upgrade/rollback 影响。
3. 在 DR supplement 中冻结本计划 §1 的 naming contract，并明确“不保留 alias”和“历史 evidence 不重写”。
4. 在 Story 中写入 AC-NAME-01~09 及本计划 §3 的验证矩阵。
5. 生成紧凑 `state.executionPlan`，包含具体 changed paths、verification、risks 和 sourceReads。
6. 运行 G-CODEPLAN-SRC、G-14、G-08；由用户显式批准 executionPlan 后才进入 Task 1。

## 5. Task 1: Write The Failing Naming Tests

**Objective:** 先把新 binary contract 固化为会失败的测试，再修改生产代码。

**Files:**

- Modify: `bins/ae-sdd-cli/tests/runtime_autostart.rs`
- Modify: `crates/ae-sdd-build/src/release.rs` test module
- Modify: `crates/ae-sdd-build/tests/service_lifecycle.rs`
- Modify: `crates/ae-sdd-protocol/tests/protocol_contract.rs`
- Modify or add focused assertions: `crates/ae-sdd-runtime/tests/**`

**Steps:**

1. 把 runtime autostart fixture、candidate path、skip message 和 default sibling 断言改为 `ae-sdd-daemon[.exe]`。
2. 把 service lifecycle fixture executable 和 invalid relative executable 改为新名；新增“旧 executable descriptor 产生 drift”断言。
3. 把 release required fixture 改为新名；新增新旧 daemon binary 同时存在时必须失败的测试。
4. 把 handshake additive-field fixture 的 `daemonBuild` 从 `ae-sddd/0.1.0` 改为 `ae-sdd-daemon/0.1.0`。其余已经使用 `ae-sdd-daemon/test` 的 client fixture 保持不动。
5. 增加 runtime handshake 和 endpoint manifest identity 断言：对外 build/version 必须以 `ae-sdd-daemon/` 开头，不能泄漏 `ae-sdd-runtime/`。
6. 运行 focused tests并记录预期 RED：默认 sibling、required binary、old-binary rejection 或 product identity 尚未满足。

**RED commands:**

```powershell
cargo test -p ae-sdd-cli --test runtime_autostart
cargo test -p ae-sdd-build release::tests --lib
cargo test -p ae-sdd-build --test service_lifecycle
cargo test -p ae-sdd-protocol --test protocol_contract
cargo test -p ae-sdd-runtime
```

## 6. Task 2: Rename The Cargo Target And External Daemon Identity

**Objective:** 让 Cargo artifact、clap display 和 daemon stderr identity 使用同一个正式名称。

**Files:**

- Modify: `bins/ae-sdd-daemon/Cargo.toml`
- Modify: `bins/ae-sdd-daemon/src/main.rs`
- Modify: `crates/ae-sdd-runtime/src/lib.rs`
- Modify: `crates/ae-sdd-runtime/src/config.rs`
- Modify: `crates/ae-sdd-runtime/src/service_protocol.rs`

**Implementation contract:**

```toml
[[bin]]
name = "ae-sdd-daemon"
path = "src/main.rs"
```

```rust
#[command(name = "ae-sdd-daemon", version, about = "ae-sdd per-user Rust daemon")]
```

1. 修改 `[[bin]].name`。
2. 修改 clap command name 和顶层错误输出前缀。
3. 将 daemon product build identity 作为 `RuntimeConfig` 的显式输入；默认值与 composition root 注入值均为 `ae-sdd-daemon/<workspace-version>`。
4. handshake 的 `daemonBuild` 从 config 返回；endpoint manifest 的 `daemonVersion` 使用 daemon package identity。不要继续用 `env!("CARGO_PKG_NAME")` 暴露内部 `ae-sdd-runtime` crate 名。
5. 将现有 `RUNTIME_BUILD` 常量替换为语义清楚的 default daemon product identity，或在无消费者后删除；不得同时维护两个互相漂移的 build identity。
6. 不改 package name、crate path、wire 字段或 endpoint manifest schema。
7. 用隔离 target 目录构建，避免旧 `target/**/ae-sddd.exe` 造成假阳性或假阴性。

**Verification:**

```powershell
$env:CARGO_TARGET_DIR='.tmp/daemon-rename-target'
cargo build -p ae-sdd-daemon --locked
& '.tmp/daemon-rename-target/debug/ae-sdd-daemon.exe' --help
cargo metadata --no-deps --format-version 1
```

Expected: target name、artifact file 和 usage 第一行均为 `ae-sdd-daemon`；metadata 中没有 `ae-sddd` target。

## 7. Task 3: Switch CLI Bootstrap To The New Sibling

**Objective:** 所有 daemon 自动启动入口只解析新 binary，保持现有 recovery 语义。

**Files:**

- Modify: `bins/ae-sdd-cli/src/bootstrap.rs`
- Modify: `bins/ae-sdd-cli/src/main.rs`
- Test: `bins/ae-sdd-cli/tests/runtime_autostart.rs`

**Steps:**

1. 将 `sibling_daemon()` 的 Windows/Unix 文件名改为 `ae-sdd-daemon.exe` / `ae-sdd-daemon`。
2. 同步 `BootstrapOptions`、`BootstrapDisposition`、manifest mismatch 和 runtime command 的用户可见文档。
3. 同步 Windows quoting test 中带空格 executable path 和 expected command line。
4. 保留显式 `--daemon <path>` override；不要增加旧名探测、环境 fallback 或双路径轮询。
5. 运行 autostart 的 missing, real bootstrap, pipe capture, concurrent bootstrap 和 default sibling cases。

**GREEN command:**

```powershell
cargo test -p ae-sdd-cli --test runtime_autostart -- --nocapture
```

## 8. Task 4: Update Release, Service And Build Tooling

**Objective:** 发布、benchmark、权限分类和 service descriptor 全部消费新 artifact，并机械拒绝旧 artifact 泄漏。

**Files:**

- Modify: `crates/ae-sdd-build/src/release.rs`
- Modify: `crates/ae-sdd-build/src/benchmark.rs`
- Modify: `crates/ae-sdd-build/src/jobs/filesystem.rs`
- Modify: `crates/ae-sdd-build/src/config.rs`
- Modify: `crates/ae-sdd-build/tests/service_lifecycle.rs`
- Modify only if needed for migration smoke: `crates/ae-sdd-build/src/service/*`

**Steps:**

1. 将 `REQUIRED_BINARIES` 的 daemon 项改为 `ae-sdd-daemon`。
2. 增加 obsolete binary 检查：release tree 中出现 file stem `ae-sddd` 时返回稳定 error；不能只依靠“新 binary 存在”。
3. 将 benchmark 的默认 sibling path 改为新名。
4. 将 Windows 非扩展名 executable allowlist 从 `ae-sddd` 换成 `ae-sdd-daemon`。
5. 更新 systemd/launchd config fixture 中的 executable path 和 expected rendering。
6. 保持 `ServicePlatform::service_name()`、descriptor path 和 manager labels 不变；只改 descriptor 内 command executable。
7. 增加旧 descriptor drift test，证明安装/升级会重写 command，而不是继续拉起旧 binary。

**Verification:**

```powershell
cargo test -p ae-sdd-build --lib
cargo test -p ae-sdd-build --test service_lifecycle -- --nocapture
cargo build --workspace --release --locked
cargo run -p ae-sdd-build --release --locked -- verify-release --artifact-dir target/release --json
```

## 9. Task 5: Align Protocol Fixtures And Current SSOT

**Objective:** 当前规范、开发者文档和方法论 source 只使用正式名称。

**Files:**

- Modify: `README.md`
- Modify: `RELEASING.md`
- Modify: `constraints/technology-stack.md`
- Modify: `constraints/project-structure.md`
- Modify: `constraints/layered-arch.md`
- Modify: `constraints/api.md`
- Modify: `constraints/testing.md`
- Modify: `source/SKILL.md`
- Modify: `source/docs/ae-sdd-design.md`
- Modify: `source/docs/ae-sdd-implementation-architecture.md`
- Modify: `source/skills/cross-cutting/memory-management-skill.md`
- Modify: `source/skill-fallbacks/SKILL.full.md`
- Modify: `source/skill-fallbacks/skills/cross-cutting/memory-management-skill.full.md`
- Modify: `source/skill-fallbacks/runtime/service-lifecycle-contract.md`
- Modify: `source/skill-fallbacks/runtime/legacy-runtime-cutover.v1.json`
- Modify: `.hermes/plans/2026-07-23_233007-ae-sdd-full-daemon-capability-migration.md`

**Steps:**

1. 将当前架构图、命令示例、binary tables、release inventory、process/IPC test contract 和 cutover check 统一为 `ae-sdd-daemon`。
2. 保留普通描述性短语“ae-sdd daemon”；只替换作为 executable identity 的旧 token。
3. 更新仍在执行中的 full-daemon migration plan，避免后续 Part 按旧命令构建或安装。
4. 不新增 changelog、Proposal、CodingReport、TestReport 或 CodeReview report。
5. 运行 update graph，核对 source 修改引出的 dist/Harness/current docs 闭环。

**Verification:**

```powershell
python tools/bin/ae-sdd update-check --json --affected "bins/ae-sdd-daemon/Cargo.toml,bins/ae-sdd-cli/src/bootstrap.rs,crates/ae-sdd-build/src/release.rs,source/SKILL.md,README.md,RELEASING.md"
```

## 10. Task 6: Rebuild Generated And Installed Artifacts

**Objective:** 从 source 重建所有派生副本，禁止手工修改生成文件。

**Generated outputs:**

- Rebuild: `dist/ae-sdd/**`
- Rebuild: `.harness/agent.md` and `.harness/.adapter.lock`
- Redistribute: `C:/Users/EDY/.codex/skills/ae-sdd/**`
- Redistribute: `C:/Users/EDY/.agents/skills/ae-sdd/**`
- Replace native install: `C:/Users/EDY/AppData/Local/Programs/ae-sdd/ae-sdd-daemon.exe`
- Clean only generated stale stages: `dist/.ae-sdd.*-stage-*`

**Steps:**

1. 从 `source/` 运行 dist build，不能直接编辑 `dist/`。
2. 使用 Rust `ae-sdd-build harness` 从 `source/SKILL.md` + `source/HARNESS.md` 重建 `.harness/agent.md`。
3. 验证 dist runtime fingerprint、manifest 和 Harness input hash。
4. 通过现有 installer/distributor 更新 Codex/Agents 安装副本，禁止逐文件手改用户目录。
5. 通过 native release 安装路径放置 `ae-sdd-daemon.exe`；验证新 binary 后移除同一安装根中的 `ae-sddd.exe`。
6. 只删除确认位于仓库 `dist/` 下的陈旧 stage 目录；保留 immutable evidence 和 `.bak` 审计内容，按既有保留策略轮转。

**Commands:**

```powershell
python scripts/build_dist.py
cargo run -p ae-sdd-build --release -- harness --source "D:/Item/ae-sdd/source/SKILL.md" --source "D:/Item/ae-sdd/source/HARNESS.md" --target "D:/Item/ae-sdd/.harness/agent.md" --title "ae-sdd Agent Harness" --allowed-root "D:/Item/ae-sdd"
python tools/bin/ae-sdd runtime verify --path dist/ae-sdd --json
python scripts/install.py --target auto
```

## 11. Task 7: Exercise Upgrade And Rollback

**Objective:** 证明 hard rename 不会留下旧 service command、双 daemon 或不可回滚安装。

**Upgrade sequence:**

1. 验证包含 `ae-sdd-daemon` 的候选 release。
2. 对运行中的旧 daemon 执行 drain/checkpoint 并停止原 service。
3. 将新 CLI、daemon、worker 和 build helper stage 到同一 complete release。
4. 原子 promote 新 release，重渲染保持原 service identity 的 descriptor。
5. 启动并完成 authenticated handshake/status；确认 PID/boot ID 更新且仍为每用户单例。
6. handshake 成功后删除安装目录中的旧 `ae-sddd[.exe]`；清理 stale endpoint manifest。
7. 任一步失败时恢复上一套完整 release 和 descriptor；不要在新 release 内保留 alias 充当 rollback。

**Required smoke matrix:**

- Windows Task Scheduler + Named Pipe.
- macOS LaunchAgent + UDS.
- Linux systemd user unit + UDS.
- Cold start, healthy reuse, concurrent first call, drain, restart, uninstall.
- Explicit old `--daemon .../ae-sddd[.exe]` 失败时返回清晰 missing-executable error，不静默回退。

## 12. Task 8: Final Regression, Evidence And Review

**Objective:** 用真实构建、跨平台行为和 scoped stale-name scan 关闭 Story AC。

**Focused verification:**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p ae-sdd-protocol --test protocol_contract
cargo test -p ae-sdd-cli --test runtime_autostart -- --nocapture
cargo test -p ae-sdd-build --lib
cargo test -p ae-sdd-build --test service_lifecycle -- --nocapture
cargo test --workspace --all-features
cargo build --workspace --release --locked
cargo run -p ae-sdd-build --release --locked -- verify-release --artifact-dir target/release --json
python tools/bin/ae-sdd update-check --json
python tools/bin/ae-sdd runtime verify --path dist/ae-sdd --json
```

**Stale-name scans:**

```powershell
rg -n --hidden 'ae-sddd' bins crates constraints source README.md RELEASING.md .harness/agent.md .hermes/plans/2026-07-23_233007-ae-sdd-full-daemon-capability-migration.md
rg -n --hidden --glob '!*.pyc' 'ae-sddd' dist/ae-sdd
rg -n --hidden --glob '!*.pyc' 'ae-sddd' C:/Users/EDY/.codex/skills/ae-sdd C:/Users/EDY/.agents/skills/ae-sdd
Test-Path 'C:/Users/EDY/AppData/Local/Programs/ae-sdd/ae-sddd.exe'
Test-Path 'C:/Users/EDY/AppData/Local/Programs/ae-sdd/ae-sdd-daemon.exe'
```

Expected: 三条 product/generated/skill-installed 扫描均无输出；旧 native path 为 `False`，新 native path 为 `True`。历史 evidence scan 单独记录 allowlisted paths，不计为当前命名失败。

**Terminal evidence:**

1. 为 AC-NAME-01~09 逐项登记真实 command、toolchain/build digest、exit code 和 artifact/descriptor digest。
2. 在真实 Windows/macOS/Linux runner 上保存 service lifecycle run references。
3. 由独立 reviewer 检查：无旧 release binary、无 bootstrap alias、service identity 未误改、历史 evidence 未篡改、dirty worktree 中用户改动未丢失。
4. Review 只登记 `status/findings`；存在 blocker/major finding 时不得完成 Work Item。

## 13. Files Expected Not To Change

- `Cargo.lock`：package name 已经正确；除非 Cargo 证明需要，否则不改。
- `Cargo.toml` workspace member path：已经是 `bins/ae-sdd-daemon`。
- `crates/ae-sdd-build/src/service/model.rs`：三个 service identity 已正确。
- `crates/ae-sdd-build/src/service/render.rs` 的 descriptor filenames：已经正确；只通过 request executable 改变 descriptor command。
- `crates/ae-sdd-integrations/src/command.rs` 的 Windows task name：已经是 `ae-sdd-daemon`。
- `migrations/**`：无 DB/schema change。
- `apps/ae-sdd-monitor/**`：本轮继续排除。
- `target/**`、`dist/**`：不作为手写 source；只隔离构建或重新生成。

## 14. Risks And Controls

| Risk | Control |
| --- | --- |
| incremental Cargo build 留下旧 `ae-sddd.exe` | clean staging / isolated `CARGO_TARGET_DIR`；release verifier 显式拒绝 obsolete binary |
| 新 CLI 与旧 descriptor 指向不同 executable | descriptor drift detection + upgrade lifecycle smoke |
| 误把 service ID 也全部改名，导致系统登记项丢失 | §1 冻结 service identity 不变，只改 executable argv |
| 旧 client 或显式 `--daemon` override 仍依赖旧路径 | 将 binary path change 作为明确 breaking release contract；不隐式 alias；提供完整 release rollback |
| source、dist、Harness、安装副本漂移 | source 单一修改，随后 build_dist/harness/install；三层分别扫描 |
| 全局替换篡改历史真实 evidence | 历史目录 allowlist；只新增 superseding design record，不改 immutable manifest |
| 当前 A/B/C/D dirty changes 被覆盖 | 每个目标文件实施前重读 `git diff`；逐 hunk 合并；禁止 checkout/reset/blanket replacement |
| protocol `daemonBuild` 与 executable name 混淆 | composition root 注入 `ae-sdd-daemon/<version>`；runtime 不泄漏内部 crate 名；wire schema 不变 |

## 15. Rollback

1. 回滚单位是上一套完整 native release，不是单独恢复一个 `ae-sddd` alias。
2. upgrade handshake 失败时恢复旧 binary set、旧 descriptor digest 和旧 service command，再启动并验证旧 release。
3. source rollback 后重新运行 build_dist、Harness 和 installer，避免代码回滚但安装副本仍是新名字。
4. immutable evidence 保留两次尝试及其真实 binary/descriptor digest；不得覆盖失败记录。
