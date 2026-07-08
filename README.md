# Auto Engineering — 端到端自动化工程 SKILL 体系

> **定位：** ae-sdd（Auto Engineering SKILL-Driven Development）是一个**门卫式**端到端自动化工程方法论 + 配套工具集。从 DR（Design Requirement）出发，经过 Story 生成、Review、Task 生成、Coding、测试，直到全部通过。
>
> **版本：** v3.9.7（🆕 2026-07-08：gate_intercept `_check_memory_entered` 入口惰性创建 `.ae-sdd/memory/` 根目录（best-effort），修复"全新项目从未跑 memory enter 时，目录缺失 = stage 永假"导致的设计阶段死环（life 项目实测触发）；不改变活跃态判定语义。v3.9.6：模板排版规范化。历史变更见 `source/CHANGELOG/`。）
>
> **目标用户：** 架构师 / 项目 owner / 开发者 / AI Agent

---

## 📦 仓库结构

ae-sdd v3.0 引入**母版 vs 分发**双目录分层，**用户拿分发包，开发者改母版**：

```
ae-sdd/                                # 仓库根（GitHub 直发）
├── source/                            # 🟢 母版 SSOT（开发者编辑这里）
│   ├── SKILL.md                       #    ae-sdd 唯一主入口
│   ├── skills/                        #    28 个子 SKILL（phase1/phase2/phase3/cross-cutting/orchestration）
│   ├── assets/                        #    项目资产（icec-cloud-boss / icec-cloud-life）
│   ├── standards/                     #    约束 + 思维引擎 + 测试策略 + 资产标准 + toolset 标准（20 份）
│   ├── templates/                     #    模板（21 份：Story/Task/DR/Report/...）
│   ├── .claude-plugin/                #    marketplace 注册表
│   ├── CHANGELOG/                     #    发版历史
│   └── docs/                          #    规划/迁移文档
│
├── dist/                              # 🔵 实例化分发包（git ignored，构建产物）
│   └── ae-sdd/                        #    bash scripts/build-dist.sh 生成
│       ├── SKILL.md                   #    （与 source/SKILL.md SHA256 一致）
│       ├── skills/ standards/ templates/ assets/
│       ├── .claude-plugin/plugin.json #    自动注入
│       └── VERSION                    #    自动注入
│
├── scripts/                           # 🟣 构建 + 安装脚本
│   ├── build-dist.sh                  #    source/ → dist/ae-sdd/ 构建
│   ├── install.sh                     #    跨平台安装（macOS/Linux/Git Bash）
│   ├── install.ps1                    #    Windows PowerShell 安装
│   ├── dev-sync.sh                    #    开发者工具：build + install + watch
│   ├── test_authenticity_scan.py      #    测试真实性扫描器（G-09 运行时依赖）
│   ├── ra_authenticity_scan.py        #    RA 真实性扫描器（G-RA-4 运行时依赖）
│   ├── ra_depth_scan.py               #    RA 机械派生深度扫描器（G-RA-5 运行时依赖）
│   ├── ra_implementation_scan.py      #    RA 实现视角七要素扫描器（G-RA-6 运行时依赖）
│   └── coding_authenticity_scan.py    #    Coding 真实性扫描器（G-CODE-1 运行时依赖）
│
├── standalone-skills/                 # 🟨 可复制到其它 agent/仓库的独立 SKILL
│   └── skill-runtime-compiler/        #    通用 SKILL 编译器：<source-skill> -> <source-skill>-compiled
│
├── .gitignore                         # 忽略 dist/、IDE 数据、临时文件
└── README.md                          # 📍 你正在看的文件
```

**关键原则：**
- 🟢 **用户/普通开发者**只关心 `dist/ae-sdd/`（装这个）
- 🟣 **ae-sdd 维护者**只编辑 `source/`（改完跑 `bash scripts/dev-sync.sh`）
- 🔵 **`dist/ae-sdd/` 由构建脚本生成**，不要手工改

---

## 🚀 快速安装（用户）

### 远程一行命令（推荐）

**macOS / Linux / Windows Git Bash：**
```bash
curl -fsSL https://raw.githubusercontent.com/AILenGarden/ae-sdd/main/scripts/install.sh | bash
```

**Windows PowerShell：**
```powershell
irm https://raw.githubusercontent.com/AILenGarden/ae-sdd/main/scripts/install.ps1 | iex
```

### 本地安装（已 clone 仓库）

```bash
# 先构建分发包（如果 dist/ae-sdd/ 不存在或要更新）
bash scripts/build-dist.sh

# 再装到本地 Claude skills
bash scripts/install.sh
```

或者**一步到位**：
```bash
bash scripts/install.sh --from-build
```

安装路径：
- Claude Code：`~/.claude/skills/ae-sdd/`
- Codex：`~/.codex/skills/ae-sdd/`（当目录已存在或检测到 Codex CLI 时自动同步）
- Hermes：`~/.hermes/skills/ae-sdd/`（当目录已存在或检测到 Hermes CLI 时自动同步）

装完后在 Claude Code 中输入 `/ae-sdd` 即可启动。

---

## 🛠 开发者指南（ae-sdd 维护者）

### 修改 SKILL 后的工作流

```bash
# 1. 编辑 source/ 下的文件（如 source/SKILL.md / source/skills/xxx-skill.md）

# 2. 跑 dev-sync：build + install 一步到位
bash scripts/dev-sync.sh

# 3. （可选）监听模式：source/ 改了自动 build + install
bash scripts/dev-sync.sh --watch
```

### 构建产物管理

| 工具 | 职责 |
|------|------|
| `bash scripts/build-dist.sh` | 从 `source/` 构建 `dist/ae-sdd/`（注入 VERSION + plugin.json，剥离 CHANGELOG/docs/marketplace.json）|
| `bash scripts/dev-sync.sh` | build + install 组合，开发者用；默认同步 Claude + 已存在/可用 Codex |
| `bash scripts/install.sh` | 从 `dist/ae-sdd/` 装到本地 Agent skills（Claude/Codex，用户/测试用）|
| `bash scripts/dev-sync.sh --uninstall` | 卸载本地安装（带备份）|

### 修改 SKILL 自身的 SOP

详细见 [`source/skills/orchestration/ae-sdd-update-skill.md`](source/skills/orchestration/ae-sdd-update-skill.md) 的 5 步流程。

---

## 📖 详细文档

完整的使用指导书、功能说明、SKILL 间调用关系，见：

- **[`source/SKILL.md`](source/SKILL.md)** — ae-sdd 主入口（智能路由 / 4 维判定 / 9 步流程 / 34 门禁 / TR-1~TR-7 / 多 Agent / 测试真实性 / 🆕 v3.8.0 自动化模式 / 🆕 v3.9.1 上下文加载准入门禁）
- **[`source/skills/`](source/skills/)** — 28 个子 SKILL（按 phase1/phase2/phase3/cross-cutting/orchestration 分类）
- **[`source/templates/`](source/templates/)** — 21 份模板（Story/Task/DR/Report/...）
- **[`source/standards/`](source/standards/)** — 20 份标准（constraints 11 + thinking 2 + testing 1 + project-assets 2 + toolsets 4）
- **[`source/assets/`](source/assets/)** — 2 个项目资产实例（icec-cloud-boss / icec-cloud-life）
- **[`source/docs/ae-sdd-design.md`](source/docs/ae-sdd-design.md)** — 系统能力说明书（能力语义、边界、当前实现状态）
- **[`source/docs/ae-sdd-implementation-architecture.md`](source/docs/ae-sdd-implementation-architecture.md)** — 实现架构说明书（CLI / tools/lib / scripts / state/cache/runtime-stats / build/distribution / gate/scanner 边界）
- **[`source/docs/skill-runtime-compiler.md`](source/docs/skill-runtime-compiler.md)** — compiled runtime 与 compact slices 设计
- **[`source/docs/`](source/docs/)** — 规划/迁移文档（含 [v3.1 纪律加固建议书](source/docs/plans/2026-06-22-discipline-hardening-plan.md) 和 [runtime stats 性能方案](source/docs/plans/2026-07-02-runtime-stats-performance-plan.md)）
- **[`source/CHANGELOG/`](source/CHANGELOG/)** — 发版历史（含 [v3.1.2 install-skill + 智能引导](source/CHANGELOG/2026-06-24-ae-sdd-install-skill.md) + [v3.1.1 阶段 H 深度强化](source/CHANGELOG/2026-06-23-requirement-analysis-阶段H深度强化.md) + [v3.1 纪律加固](source/CHANGELOG/2026-06-22-v3.1-discipline-hardening.md)）
- 🆕 v3.1：[`source/docs/ae-sdd-conventions.md`](source/docs/ae-sdd-conventions.md) — 项目级 SOP 模板（root agent 必读）
- 🆕 v3.5.0：**CodingSKILL 外挂机制**（[🔌 CodingSKILL 外挂机制（v3.5.0 — 母版内置能力）](#-codingskill-外挂机制v350--母版内置能力)）— 三层 SKILL 注册表（L1 项目层 / L2 全局层 / L3 仓库根层）+ 内置 fallback 零破坏
  - **加载协议 SKILL**：[`source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md`](source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md) — 加载协议 SOP + 注册流程引导
  - **schema 规范**：[`source/standards/constraints/plugin-registry-spec.md`](source/standards/constraints/plugin-registry-spec.md) — 权威 schema
  - **模板**：[`source/templates/project-assets/plugin-registry-template.yaml`](source/templates/project-assets/plugin-registry-template.yaml) — 三层通用注册表模板
  - **设计文档**：[`source/docs/plans/2026-06-26-plugin-registry-design.md`](source/docs/plans/2026-06-26-plugin-registry-design.md) — 完整设计说明
  - **示例 scaffolding**：[`plugins/_example-coding-style/`](plugins/_example-coding-style/) — 仓库根层（不自动加载）

---

## 🚀 自动化模式（🆕 v3.8.0 — 输入→结果全自动化）

> **默认关闭。** 开启后 6 个人工审核点（1/1.5/2/2.5/4/5）改走 Tier 3 多 reviewer 联审共识，跳过所有人工✅，实现 ae-sdd 输入→结果。联审机制复用 `agent-orchestration-skill §8.4`（Tier 判定 + 视角正交 + 交叉对比）。

### 配置（`.ae-sdd/config.yaml` 的 `automation` 段）

```yaml
automation:
  enabled: false              # 总开关（默认关）
  reviewerTier: 3             # 强制三审
  preflightInfoCollection: true
  onConsensusStall: pause     # pause=paused等用户 / fail=标记失败
  automatedReviewPoints: [1, 1.5, 2, 2.5, 4, 5]
  enabledAt: ""               # 审计时间戳，AI 不得自行改
```

### 用法

```bash
ae-sdd automation status          # 查看自动化配置
ae-sdd automation enable          # 开启全自动化（写 enabledAt）
ae-sdd automation disable         # 关闭（回退人工审核）
ae-sdd preflight collect          # 开工前信息预收集（列待补信息清单）
ae-sdd state register-review-consensus --point 1 --passed true  # 写联审共识
```

### 行为分叉

| 模式 | 审核点行为 |
|------|----------|
| 默认（enabled=false）| AI 讲解 → 等用户 ✅/⚠️/❌ |
| 自动化（enabled=true 且点在白名单）| AI 讲解 → 强制 Tier 3 派 3 独立 session reviewer → §8.4.3 交叉对比 → G-09B+G-REVIEW-LOOP+G-AUTO-CONSENSUS 全过即自动推进 |
| 自动化但点不在白名单 | 回退人工审核 |

### 开工前信息预收集（Step 1.5）

开工前一次性向用户收集所有必需信息（第三方凭证/复用选择/环境配置/命名约定/对接方/数据初始化），开工后不再打断。

### 阻断出口

联审 3 轮矫正未决 → `state.phase=paused`（默认），输出完整问题清单等用户介入，避免 AI 带病狂奔。

📖 **完整使用指南**（前置条件 → 开启 → 第一次跑 → 失败处理 → 关闭）：[`source/docs/ae-sdd-automation-guide.md`](source/docs/ae-sdd-automation-guide.md)

详见 [`source/SKILL.md §🚀 自动化模式`](source/SKILL.md)。

---

## 🔌 CodingSKILL 外挂机制（🆕 v3.5.0 — 母版内置能力）

> **本节讲的是 ae-sdd 母版内置的 CodingSKILL 外挂机制——**
> - **母版维护者**可在 `ae-sdd/plugins/` 挂官方扩展（仓库根层 L3）
> - **项目 owner**可在自己的 `<project>/.ae-sdd/plugins/` 挂项目层定制（L1）
> - **个人开发者**可在 `~/.ae-sdd/plugins/` 挂跨项目偏好（L2）
>
> **本机制不接收社区贡献 PR**——ae-sdd 母版不接受外部 SKILL 提交，所有定制都在本地/项目层完成（零 PR、零污染、零等待）。

### 1. 三层注册表是什么

ae-sdd v3.5.0 起，**任何内置 SKILL / 模板都可以被外挂覆盖**。注册表分三层：

| 层 | 路径 | 适用场景 | git |
|---|------|---------|-----|
| **L1 项目层** | `<project>/.ae-sdd/plugins/registry.yaml` | 项目团队约定（项目 owner 用） | ❌ |
| **L2 用户全局层** | `~/.ae-sdd/plugins/registry.yaml` | 跨项目的个人偏好 | ❌ |
| **L3 仓库根层** | `<ae-sdd-master>/plugins/registry.yaml` | ae-sdd 母版维护者发布官方扩展 | ✅ |

**优先级：** L1 > L2 > L3 > 内置 fallback。三层都未声明 → 行为与 v3.4.x 完全一致（零破坏）。

### 2. 注册流程（5 步）

#### Step 1：选层

- 项目 owner 定制 → **L1**（项目层，推荐）
- 个人跨项目偏好 → **L2**（全局层）
- 你是 ae-sdd **母版维护者** → L3（仓库根层，发布官方扩展）

#### Step 2：生成注册表

拷贝模板到目标位置：

```bash
# 项目层
cp source/templates/project-assets/plugin-registry-template.yaml \
   <your-project>/.ae-sdd/plugins/registry.yaml

# 或全局层
cp source/templates/project-assets/plugin-registry-template.yaml \
   ~/.ae-sdd/plugins/registry.yaml
```

#### Step 3：填字段 + 写外挂 SKILL

示例（L1 项目层）：

```yaml
# <your-project>/.ae-sdd/plugins/registry.yaml
schema_version: 1
description: my project's TDD + DDD coding

plugins:
  - name: my-coding
    type: skill-override               # 覆盖内置 SKILL
    version: 0.1.0
    description: my TDD + DDD style
    replaces: source/skills/phase2-coding/coding-skill.md
    path: ./my-coding/SKILL.md         # 相对 registry 所在目录
    compatibility:
      ae_sdd_version: ">=3.5.0"
    tags: [team-style, tdd, ddd]
```

```markdown
<!-- <your-project>/.ae-sdd/plugins/my-coding/SKILL.md -->
---
name: my-coding
description: my project's TDD + DDD coding
---

# My Coding (TDD + DDD)

（你的团队约定）
```

#### Step 4：验证

```bash
ae-sdd plugin validate
# 跑三层注册表 + 每个 plugin sanity check

ae-sdd plugin trace coding-skill.md
# 看该 SKILL 实际从哪层加载
```

#### Step 5：测试实际流程

- 用户说"开始 Coding" → 触发 coding-skill 加载
- Agent 按 `ae-sdd-plugin-loader-skill.md` 协议加载 → 命中 L1 → 读外挂 SKILL
- 流程继续

### 3. 完整 schema / 设计文档

- **schema 权威**：[`source/standards/constraints/plugin-registry-spec.md`](source/standards/constraints/plugin-registry-spec.md)
- **设计文档**：[`source/docs/plans/2026-06-26-plugin-registry-design.md`](source/docs/plans/2026-06-26-plugin-registry-design.md)
- **加载协议 SKILL**：[`source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md`](source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md)
- **模板**：[`source/templates/project-assets/plugin-registry-template.yaml`](source/templates/project-assets/plugin-registry-template.yaml)
- **示例 scaffolding**：[`plugins/_example-coding-style/`](plugins/_example-coding-style/)（仓库根层，不自动加载）

### 4. CLI 工具（v3.5.0 实现）

```bash
# 查看已注册插件（三层合并 + 冲突检测）
ae-sdd plugin list

# 校验三层注册表
ae-sdd plugin validate

# 查看某 SKILL 的加载路径
ae-sdd plugin trace <skill-key>

# 生成新注册表
ae-sdd plugin init --layer {project|global}
```

> **CLI 状态：** v3.5.1 已完成挂载 — `ae-sdd plugin list/validate/trace/init` 4 个子命令可用，11 个 CLI 单元测试全过（`tools/tests/test_plugin_cli.py`）。Python 模块 `tools/lib/plugin_loader.py` 完成于 v3.5.0（35 个单元测试全过）。

---

## ❓ 常见问题

### Q1: 我装到本地后想卸载怎么办？
```bash
bash scripts/dev-sync.sh --uninstall
# 或者手动：mv ~/.claude/skills/ae-sdd ~/.claude/skills/ae-sdd.uninstalled.<时间戳>
# Codex 目录同理：mv ~/.codex/skills/ae-sdd ~/.codex/skills/ae-sdd.uninstalled.<时间戳>
```

### Q2: 改了 `source/` 里的内容，本地没生效？
跑一次 `bash scripts/dev-sync.sh`（build + install）。

### Q3: `dist/ae-sdd/` 跟 `source/` 应该是字节级一致吗？
**SKILL.md 是字节级一致**（tar 整树复制）。但 dist 会**剥离**母版专有的：
- `CHANGELOG/`（开发回溯用）
- `docs/`（规划文档）
- `.claude-plugin/marketplace.json`（仅母版持有 marketplace 注册）
- `.idea/`（IDE 数据，build 时不复制）

并**注入**：
- `VERSION`（含版本号 + 构建时间戳）
- `.claude-plugin/plugin.json`（plugin 自描述元数据）
- `scripts/test_authenticity_scan.py`、`scripts/ra_authenticity_scan.py` 与 `scripts/coding_authenticity_scan.py`（门禁运行时扫描器）

### Q4: 我是项目 owner，想给项目加 ae-sdd 怎么用？
安装 ae-sdd 后，在你的项目目录下跑 `ae-sdd init <project-dir> <project-key>`（v3.0 实施中，详见 source/SKILL.md §6 实例化机制）。

### Q5（🆕 v3.1）：触发 `/ae-sdd` 后被强制走 G-00 / 路由判定 / 派 sub-agent 怎么办？
v3.1 起 SKILL 入口段加「🔴 第一动作（硬前置）」声明：收到 `/ae-sdd` 触发后必须先跑 G-00、跑路由判定、必要时派 sub-agent，禁止直接动手改代码。如确认是小修小补（单文件单改），用户**显式说** `/ae-sdd-quick` 或 `走快速通道` 可豁免完整 7 步路由，但仍需落档。详见 [v3.1 纪律加固 CHANGELOG](source/CHANGELOG/2026-06-22-v3.1-discipline-hardening.md)。

### Q6（🆕 v3.5.0）：项目团队 Coding 风格不同，能不能给项目定制自己的 CodingSKILL？
**可以。** v3.5.0 起支持三层 SKILL 注册表机制。详见上面的 [🔌 CodingSKILL 外挂机制](#-codingskill-外挂机制v350--母版内置能力) 节。简单三步：

1. 拷贝模板到 `<project>/.ae-sdd/plugins/registry.yaml`
2. 写你的 CodingSKILL 文档
3. 跑 `ae-sdd plugin validate` 验证

**零 PR、零污染、零等待。** ae-sdd 母版不接受外部 SKILL 贡献，所有 CodingSKILL 定制都在本地（项目层 L1 或全局层 L2）或母版自身（仓库根层 L3）完成。

---

## 📜 License

待定
