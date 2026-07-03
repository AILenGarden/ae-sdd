# ae-sdd Monitor 设计文档

> v0.1.0 · ae-sdd Monitor 是 ae-sdd 的本地只读可视化投影层。本文档只描述 Monitor 的产品语义、信息架构、数据投影和同步边界；ae-sdd 主流程能力语义仍以 [`ae-sdd-design.md`](ae-sdd-design.md) 为准，实现分层仍以 [`ae-sdd-implementation-architecture.md`](ae-sdd-implementation-architecture.md) 为准。

## 1. 定位

ae-sdd Monitor 是一个 PC 桌面应用，用来观察本机多个 ae-sdd 工作区的运行状态。它解决的是“我在哪些目录里有 ae-sdd 工作区、每个工作区现在走到哪一步、最近有没有失败或暂停”的可视化问题。

Monitor 不参与 ae-sdd 决策，不执行 gate，不写入 state，不替代 CLI 输出。它读取已有项目侧文件，把状态转换成可浏览的列表、总览、时间线、工作项和运行时统计。

## 2. 权威边界

| 内容 | 权威源 | Monitor 职责 |
| --- | --- | --- |
| ae-sdd 能力语义、阶段含义、门禁语义 | `source/docs/ae-sdd-design.md` | 跟随展示，不重新定义 |
| 实现分层、状态文件、Runtime Stats 存储规则 | `source/docs/ae-sdd-implementation-architecture.md` | 跟随解析，不另建数据契约 |
| 项目当前状态 | `.ae-sdd/state.json` 与 `.auto-engineering/*/state.json` | 只读解析，派生展示状态 |
| 运行时观测数据 | `.ae-sdd/runtime-stats/*.jsonl` | 只读聚合，展示耗时/失败/最近事件 |
| Gate 与硬判断 | `tools/bin/ae-sdd`、`tools/lib/gates.py`、CLI 输出 | 不执行，不裁决，只展示已有结果线索 |

## 3. 入口与扫描

用户选择一个父目录，Monitor 在该目录下递归扫描 ae-sdd 工作区。判定规则是：目录内存在 `.ae-sdd/` 即视为一个工作区；命中后不继续深入该工作区内部，避免把内部产物误识别成子工作区。

默认扫描策略：

- 最大深度：8 层。
- 最大工作区数量：500 个。
- 跳过高噪声目录：`.git`、`.ae-sdd`、`.auto-engineering`、`node_modules`、`dist`、`release`、`target`、`build`、`out`、虚拟环境、IDE 目录等。
- 扫描结果按派生状态优先级排序：`active`、`blocked`、`paused`、`idle`、`completed`、`invalid`、`unknown`。

## 4. 信息架构

左侧是工作区列表，面向“快速定位”。每个条目展示：

- 项目名或 `projectKey`。
- 派生状态。
- 当前 `phase`。
- 最近活动时间。
- 工作区路径。

右侧是选中工作区详情，面向“判断当前发生了什么”。详情页分为：

- 总览：状态、阶段、进度、当前 Story/Task、最近活动、配置和 state 文件位置。
- 时间线：阶段轴 + `state.history` 与 `state.events` 的事件轨迹。阶段轴必须展示当前 scale 下完整 phase 链，并标出已完成、当前、暂停和待执行节点。
- 工作项：活跃任务汇总 + `.auto-engineering/{workItemKey}/state.json` 全量列表。新状态机目录名为 `{ID}--{name}`，详情中必须展示 `workItemId`、`workItemName` 和 `workItemKey`，避免只看到旧 `currentStory` 或不可读目录。
- 性能：Runtime Stats 的命令数、失败数、耗时和最近事件。
- 原始状态：当前 `.ae-sdd/state.json` 的只读 JSON 视图。

交互要求：

- 点击“选择目录”后必须立即显示反馈：打开选择器、扫描中、取消、扫描完成或扫描失败。
- 重新打开应用时自动恢复上次选择的父目录，并优先选中上次打开的工作区。
- 顶部类 Mac 三点必须是真实窗口控制，分别对应关闭、最小化和最大化/还原；不得保留无功能装饰控件。
- 应用窗口可拖拽，但按钮、搜索、Tab、列表项等交互区域不得被拖拽区域吞掉。

## 5. 状态投影

Monitor 的展示状态是派生值，不是 ae-sdd 的新增状态机字段。

| 派生状态 | 条件 |
| --- | --- |
| `invalid` | 缺少 `state.json` 或读取/解析出错 |
| `paused` | `state.phase == "paused"` |
| `completed` | `state.phase == "completed"` |
| `blocked` | 最近 Runtime Stats 事件退出码非 0，且失败仍是最新线索 |
| `active` | `state.activeAgents` 非空，或最近 24 小时有 state/runtime 活动 |
| `idle` | 可读但近期无活动 |
| `unknown` | 兜底状态 |

进度条依据 ae-sdd 当前阶段链派生。Monitor 内置阶段链只是展示投影，必须跟随 `tools/lib/state.py:PHASE_FLOWS` 和 `ae-sdd-design.md` 的状态机语义；当 ae-sdd 新增/删除/重命名 phase、scale 或 entry node 时，Monitor 解析和测试必须同步。

活跃任务汇总是另一个派生投影，不新增 ae-sdd 字段。来源包括：

- 根 state 的 `activeWorkItem`、`currentWorkItem`、`currentStory`、`currentTask`。
- 根 state 的 `activeAgents[]`，按 `txnName`、`workItemId`、`taskId`、`storyId` 或 `agentId` 归并。
- `.auto-engineering/*/state.json` 中未处于 `completed` 或 `invalid` 的工作项；优先使用 state 内的 `workItemKey` 作为展示 ID，并保留 `workItemId`/`workItemName` 作为辅助信息。

同一个任务从多个来源出现时，Monitor 必须合并展示来源，而不是重复多行或只保留第一项。

## 6. 数据读取

| 数据 | 路径 | 读取方式 |
| --- | --- | --- |
| 项目配置 | `.ae-sdd/config.yaml` / `.ae-sdd/config.yml` / `.ae-sdd/config.json` | 轻量 YAML/JSON 读取，只取展示字段 |
| 活跃工作区状态 | `.ae-sdd/state.json` | JSON 读取，缺失时标记 `invalid` |
| Work item 状态 | `.auto-engineering/*/state.json` | JSON 读取，按目录名汇总 |
| Runtime Stats | `.ae-sdd/runtime-stats/*.jsonl` | 倒序读取最近事件，跳过不完整 JSONL 行 |

所有读取都是本地只读。Monitor 不维护单独数据库；窗口刷新或重新扫描时重新读取文件系统。后续如果引入缓存，缓存只能服务 UI 性能，不得成为状态真相。

## 7. UI 风格

Monitor 采用黑白灰、圆角、类 Mac 的本地工具风格。视觉目标是“安静、可扫读、像状态面板”，不是营销页。

设计约束：

- 首屏就是可操作的扫描与状态面板，不做 landing page。
- 左侧列表固定承担导航，右侧承担详情。
- 卡片只用于具体指标、列表项和工具面板，不在页面段落外层套装饰卡。
- 控件优先用清晰的按钮、分段筛选、Tab 和搜索输入。
- 文案只呈现状态和动作，不在界面里解释实现细节。
- 顶部窗口控制采用黑白圆点风格；如果使用类 Mac 三点，必须接入真实窗口行为。

## 8. 安全与副作用

Monitor 当前必须保持零写入：

- 不修改 `.ae-sdd/`。
- 不修改 `.auto-engineering/`。
- 不执行 `ae-sdd gates check` 或其它会改变状态的命令。
- 不自动清理 Runtime Stats。
- “打开目录”只调用系统 shell 打开路径，不改变项目内容。

如果未来新增写入能力，必须先把能力写入本设计文档和实现架构文档，并新增明确的 gate/确认/审计策略；不能把写入能力混进“刷新”或“扫描”动作。

## 9. 与 ae-sdd 文档同步

Monitor 必须时刻跟随 ae-sdd 主设计与实现架构，具体闭环由 `source/standards/update-graph.json` 的 `UG-22` 维护。

需要同步 Monitor 的典型变更：

- `ae-sdd-design.md` 改了状态机、阶段语义、门禁语义或用户可见流程。
- `ae-sdd-implementation-architecture.md` 改了 `.ae-sdd/`、`.auto-engineering/` 或 Runtime Stats 存储约定。
- `tools/lib/state.py` 改了 `PHASE_FLOWS`、`scale`、`entryNode` 或 state 字段。
- `tools/lib/runtime_stats.py` 改了 JSONL 字段、脱敏策略或事件含义。
- `tools/bin/ae-sdd` 改了会影响状态/性能诊断的命令输出契约。
- `apps/ae-sdd-monitor/**` 改了解析、展示、打包或安装行为。

同步动作：

- 更新本文档，说明 Monitor 对新语义的投影方式。
- 更新 `apps/ae-sdd-monitor/src/workspace.js`，保持解析兼容。
- 更新 `apps/ae-sdd-monitor/test/workspace.test.js`，覆盖新字段或新状态。
- 必要时更新 `apps/ae-sdd-monitor/README.md` 与 changelog。
- 运行 `ae-sdd update-check --affected <changed-files>` 和 Monitor 单元测试。

## 10. 当前实现

当前实现位于 `apps/ae-sdd-monitor/`：

- Electron 主进程：`src/main.js`。
- 安全桥接：`src/preload.js`。
- 状态扫描与解析：`src/workspace.js`。
- UI 渲染：`src/renderer.js`、`src/index.html`、`src/styles.css`。
- 用户偏好：Electron `app.getPath("userData")/preferences.json`，保存父目录、选中工作区和主题。
- 单元测试：`test/workspace.test.js`。
- Windows 打包：`scripts/package-win.ps1`，输出 setup exe 与 installable zip。
- macOS 正式打包：`npm run dist:mac` / `scripts/package-mac.sh`，在 macOS 输出 dmg 与 zip。
- macOS 未签名 app zip：`npm run dist:mac:unsigned` / `scripts/package-mac-unsigned.ps1`，可在 Windows/macOS 生成 `*-macos-*-unsigned.zip`，用于未签名试用或交给 macOS runner 后续签名。

当前版本已支持父目录扫描、多工作区列表、详情页、Runtime Stats 汇总、原始状态查看、重启恢复上次目录、真实窗口控制、活跃任务汇总、阶段轴、Windows 安装包、macOS 未签名 zip、macOS 正式打包配置和只读打开目录。
