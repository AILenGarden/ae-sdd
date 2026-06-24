# Auto Engineering — 端到端自动化工程 SKILL 体系

> **定位：** ae-sdd（Auto Engineering SKILL-Driven Development）是一个**门卫式**端到端自动化工程方法论 + 配套工具集。从 DR（Design Requirement）出发，经过 Story 生成、Review、Task 生成、Coding、测试，直到全部通过。
>
> **版本：** v3.2.4（🆕 2026-06-24：ae-sdd-update-skill 新增「项目结构与设计说明」章节 — 固化 ae-sdd 项目 6 大子系统（SKILL 本体/实例化体系/构建安装脚本/安装引导 SKILL/工具链/Harness 适配层）的总览、协同关系图与维护边界，让维护者改任一子系统时知道连带影响；同步补齐健康度清单 v3.2.2/v3.2.3 缺失条目、修正 README 正文门禁数 14→19 与子 SKILL 数 15→22；v3.2.3：Memory 强制门禁升级 — `ae-sdd state write --phase ...` 在离开 RA/design/coding-plan/coding/review 关联阶段前自动校验 `memory enter → memory write`；v3.2.2：Toolset Layer P0 — 新增 `ae-sdd memory/db/git` 三组工程工具，DB 采用本地 profile + read-first 策略，Git Insight 只读输出结构化历史/影响证据；v3.2.1：Coding 工具层加固 — G-CODE-1 + coding_authenticity_scan.py + gate coding-required；v3.2.0：需求分析全维度对标 Coding — RAModel 12 维 + 16 道 RA-G 闸 + G-RA-1~4 硬门禁 + ra_authenticity_scan.py + G-13 六层追溯）
>
> **目标用户：** 架构师 / 项目 owner / 开发者 / AI Agent

---

## 📦 仓库结构

ae-sdd v3.0 引入**母版 vs 分发**双目录分层，**用户拿分发包，开发者改母版**：

```
ae-sdd/                                # 仓库根（GitHub 直发）
├── source/                            # 🟢 母版 SSOT（开发者编辑这里）
│   ├── SKILL.md                       #    ae-sdd 唯一主入口
│   ├── skills/                        #    22 个子 SKILL（phase1/phase2/phase3/cross-cutting/orchestration）
│   ├── assets/                        #    项目资产（icec-cloud-boss / icec-cloud-life）
│   ├── standards/                     #    约束 + 思维引擎 + 测试策略 + 资产标准 + toolset 标准（18 份）
│   ├── templates/                     #    模板（17 份：Story/Task/DR/Report/...）
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
│   └── coding_authenticity_scan.py    #    Coding 真实性扫描器（G-CODE-1 运行时依赖）
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

- **[`source/SKILL.md`](source/SKILL.md)** — ae-sdd 主入口（智能路由 / 4 维判定 / 9 步流程 / 19 门禁 / TR-1~TR-7 / 多 Agent / 测试真实性）
- **[`source/skills/`](source/skills/)** — 22 个子 SKILL（按 phase1/phase2/phase3/cross-cutting/orchestration 分类）
- **[`source/templates/`](source/templates/)** — 17 份模板（Story/Task/DR/Report/...）
- **[`source/standards/`](source/standards/)** — 18 份标准（constraints 9 + thinking 2 + testing 1 + project-assets 2 + toolsets 4）
- **[`source/assets/`](source/assets/)** — 2 个项目资产实例（icec-cloud-boss / icec-cloud-life）
- **[`source/docs/`](source/docs/)** — 规划/迁移文档（含 [v3.1 纪律加固建议书](source/docs/plans/2026-06-22-discipline-hardening-plan.md)）
- **[`source/CHANGELOG/`](source/CHANGELOG/)** — 发版历史（含 [v3.1.2 install-skill + 智能引导](source/CHANGELOG/2026-06-24-ae-sdd-install-skill.md) + [v3.1.1 阶段 H 深度强化](source/CHANGELOG/2026-06-23-requirement-analysis-阶段H深度强化.md) + [v3.1 纪律加固](source/CHANGELOG/2026-06-22-v3.1-discipline-hardening.md)）
- 🆕 v3.1：[`source/docs/ae-sdd-conventions.md`](source/docs/ae-sdd-conventions.md) — 项目级 SOP 模板（root agent 必读）

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

### Q5: 想贡献 SKILL 怎么 PR？
1. Fork 仓库
2. 在 `source/skills/<phase>/` 下加你的 SKILL.md
3. 在 `source/SKILL.md` 路由表里注册触发词
4. 跑 `bash scripts/dev-sync.sh` 验证
5. 提 PR 到 `main` 分支

### Q6（🆕 v3.1）：触发 `/ae-sdd` 后被强制走 G-00 / 路由判定 / 派 sub-agent 怎么办？
v3.1 起 SKILL 入口段加「🔴 第一动作（硬前置）」声明：收到 `/ae-sdd` 触发后必须先跑 G-00、跑路由判定、必要时派 sub-agent，禁止直接动手改代码。如确认是小修小补（单文件单改），用户**显式说** `/ae-sdd-quick` 或 `走快速通道` 可豁免完整 7 步路由，但仍需落档。详见 [v3.1 纪律加固 CHANGELOG](source/CHANGELOG/2026-06-22-v3.1-discipline-hardening.md)。

---

## 📜 License

待定
