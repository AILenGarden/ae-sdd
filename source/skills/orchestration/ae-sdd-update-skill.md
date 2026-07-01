---
name: ae-sdd-update
description: 规范各 SKILL 的内容边界与维护规则。ae-sdd-skill 退守"流程编排"（流程怎么走、节点间如何流转），各子 SKILL 负责"环节内具体规则"（每一步具体怎么做、出错怎么处理）。当用户新增/修改任何 AE 相关 SKILL 时，先查阅本 SKILL 确认内容应放在哪个文件，避免在错误位置撰写或重复堆积。
---

# Auto Engineering Update — SKILL 边界维护规范

> **本 SKILL 不是工作流，而是"SKILL 维护的工作流"。** 它定义 auto-engineering 体系内**每个 SKILL 应该承担什么、不应该承担什么**，杜绝内容在错误位置堆积（这是本次重构的核心目标）。

---

## 核心设计哲学

### auto-engineering-skill = 流程编排（不退守就会腐化）

| 性质 | 含义 | 类比 |
|------|------|------|
| ✅ **应当承担的** | 流程怎么走（Phase 1→2→3）、节点间如何流转、门禁是什么、子 SKILL 索引、状态跟踪 | "项目经理" — 只管"下一步该做什么、由谁做" |
| ❌ **不应当承担的** | 每个环节内"具体怎么做"、讲解模板、报告模板、问题排查细节、术语定义 | "工程师" — "这道题怎么解" |

**反例（历史已发生，本次重构已清理）：**
- ❌ AE-skill 中塞了 3 个讲解模板（应归子 SKILL）
- ❌ AE-skill 中塞了 CodeReview 报告 9 章节模板（应归 `templates/coding/be-codereview-template.md`）
- ❌ AE-skill 中塞了 6 维度前端契约审查清单（应归 `story-review-skill.md`）
- ❌ AE-skill 中塞了 CodingPlan 7 章节 + 10 条门禁（应归 `coding-skill.md`）
- ❌ AE-skill 中塞了测试真实性 8 类禁止（应归 `coding-skill.md`）
- ❌ AE-skill 中塞了 Coding 4 层排查流程（应归 `coding-skill.md`）

**为什么会腐化？** 因为"流程编排"与"具体规则"看似紧密耦合（流程的每一步都有规则），容易把规则顺手写在流程 SKILL 里。**这是"路径依赖"陷阱** —— 一开始写 AE-skill 时为了完整，会把所有相关内容都堆进去；时间一长，AE-skill 变成 2540 行的"百科全书"，没人能维护。

## 🔴 极简描述原则（🆕 2026-07-01 — 瘦身是持续动作，不是一次性重构）

> **更新任何 SKILL.md（含 `source/SKILL.md` 主入口）、子 SKILL、`source/docs/*.md` 时，必须同时执行"瘦身"**：只保留 AI 执行任务所需的核心含义，删除不影响执行判断的内容。这条独立于"内容边界判定"（放哪个文件）——本节管"同一段内容该写多长"。

**写入前 5 问自检（任一答"是"→ 删除或改写）：**

| # | 问题 | 判定为"是"时的动作 |
| --- | --- | --- |
| 1 | 这段是"为什么这样设计"的背景叙事？ | 删除，或压缩为 1 行脚注 |
| 2 | 这段是历史版本变更堆积（🆕 vX.Y.Z 逐条罗列）？ | 移入 `CHANGELOG/`，正文只留"当前状态" |
| 3 | 这段能用表格代替长段落？ | 改表格 |
| 4 | 这段重复了另一个文件已有的内容？ | 改成指针引用 |
| 5 | 删掉这句话，AI 还能正确执行任务？ | 删除 |

**已验证案例：** `source/SKILL.md` 2839 行 → 546 行（-81%），核心路由/门禁/执行清单逻辑零损失。

**禁止：**

- ❌ 新增内容时旧内容"顺手"保留不删（导致文件只增不减）
- ❌ 用"完整性/防遗漏"当借口堆砌背景说明
- ❌ 版本变更历史写在正文（应写 CHANGELOG，正文只体现最终状态）

### 各子 SKILL = 环节内具体规则

| 子 SKILL | 职责 | 不应承担 |
|---------|------|----------|
| `story-review-skill.md` | Story 缺陷挖掘 / 合理性判定 / 通过标准 / ①bis 前端契约 6 维度 | 流程编排（AE-skill）、CodeReview 规则 |
| `task-generate-skill.md` | Task 文档生成 / Task 0 公共依赖 / 全局 Task Review / Task 修复流程 | 流程编排、Story 规则、Code 规则 |
| `coding-skill.md` | ⑥ 按 Task 生成代码 / 测试真实性 8 类禁止 / ④bis CodingPlan / ⑥bis 一致性闸 / ⑦bis 对称性闸 / 异常路径 A1-A6 | CodeReview 报告模板（应在 templates/）、流程编排 |
| `testcase-generate-skill.md` | 测试用例生成 / AC 映射 / 合规性校验 | 流程编排、Coding 规则 |
| `dr-update-skill.md` | DR 文档更新 / DR 缺陷修复 | 流程编排、其他文档规则 |
| `story-update-skill.md` | Story 文档更新 / Story 缺陷修复 | 流程编排、Task 规则 |
| `templates/coding/be-codereview-template.md` | CodeReview 报告 9 章节空白模板 | 具体 Story 的填充内容 |
| `templates/coding/be-coding-report-template.md` | Coding 报告空白模板 | 具体 Story 的填充内容 |
| `templates/design/*.md` | DR / Story / Task / Story Review 逻辑汇总等空白模板 | 具体 Story 的填充内容 |
| `templates/testcase/*.md` | 测试用例 / 测试报告空白模板 | 具体 Story 的填充内容 |

---

## 项目结构与设计说明（🆕 v3.2.4 — 维护者的项目地图）

> **定位：** 本节是 ae-sdd **整个项目**的结构地图与子系统设计说明，回答"ae-sdd 不只是 SKILL 文档，还由哪些部分组成、各部分职责、如何协同"。维护者在改动任何文件前，先对照本节确认"我改的是哪个子系统、会连带影响哪些子系统"。
>
> **与上文「核心设计哲学」的区别：** 上文讲"SKILL 文档内容边界"（哪些规则写哪个 .md）；本节讲"项目工程边界"（scripts/tools/harness/实例化等非 SKILL 子系统怎么协同）。

### 6 大子系统总览

ae-sdd 是一个**多子系统协同**的工程，不是单一文档集合：

| # | 子系统 | 物理位置 | 职责一句话 | 维护方式 |
|---|--------|---------|-----------|---------|
| ① | **SKILL 本体（方法论 + 编排）** | `source/SKILL.md` + `source/skills/`(25 个子 SKILL) + `source/standards/` + `source/templates/` + `source/assets/` | ae-sdd 方法论母版 SSOT：流程编排、门禁、模板、约束、项目资产 | 直接编辑 `source/`，本文件「SKILL 边界判定表」管辖 |
| ② | **实例化体系（4 层架构）** | `dist/ae-sdd/`(Layer2) + `~/.claude/skills/ae-sdd/`(Layer3) + `<project>/.ae-sdd/`(Layer4) | 母版→分发包→用户安装→项目实例，引用+override 模式 | 不手工改 Layer2/3/4；由 build/install/init 生成 |
| ③ | **构建与安装脚本** | `scripts/build_dist.py` / `dev_sync.py` / `install.py` / `init.py` + 对应 `.sh`/`.ps1` 薄壳 | 构建分发包、跨平台安装、项目实例化、开发者一键同步 | 直接编辑 `scripts/*.py`（薄壳 `.sh`/`.ps1` 只找 Python 后 exec） |
| ④ | **安装引导 SKILL** | `source/skills/orchestration/ae-sdd-install-skill.md` | 面向 Agent 的安装/重装/升级/卸载引导（10 节流程） | 随子系统①一起维护（属 skills/），但逻辑独立于方法论 |
| ⑤ | **工具链（CLI + lib + tests）** | `tools/bin/ae-sdd`(15 子命令：原 14 + v3.5.1 `plugin` 4 子命令平铺) + `tools/lib/`(14 模块：原 13 + v3.5.0 `plugin_loader.py`) + `tools/tests/`(16 测试：原 12 + v3.5.0/1 plugin 系列 4 个) | 门禁检查、状态管理、记忆层、DB/Git 工具集、hook 拦截、update-check | 直接编辑 `tools/`，独立于 `source/` 但被 update-graph 联动 |
| ⑥ | **Harness 适配层** | `harness/.harness/agent.md` + `.adapter.lock` | 由 ae-sdd-harness-adapter SKILL 自动生成，转译为 Mavis 团队级 agent | ❌ 不手工改；母版升级后重跑 adapter SKILL 重新生成 |

### 子系统协同关系图

```
┌─────────────────────────────────────────────────────────────────┐
│  ① SKILL 本体（SSOT）                                           │
│  source/SKILL.md + skills/ + standards/ + templates/ + assets/  │
└──────────────┬───────────────────────────────────┬──────────────┘
               │                                   │
        ③ build_dist.py                    ⑤ 工具链 tools/
        (构建分发包)                        (CLI + lib + tests)
               │                                   │
               ▼                                   │
┌──────────────────────────┐                       │
│ ② Layer2 实例化分发包     │ ◄── build 时把 tools/  │
│ dist/ae-sdd/             │     scanner 注入 dist  │
└──────────┬───────────────┘                       │
           │ ③ install.py                          │
           ▼                                       │
┌──────────────────────────┐                       │
│ ② Layer3 用户安装         │ ◄─ CLI 直接从 tools/  │
│ ~/.claude/skills/ae-sdd/ │   运行，不进 dist     │
└──────────┬───────────────┘                       │
           │ ③ init.py (ae-sdd init <dir> <key>)   │
           ▼                                       │
┌──────────────────────────┐                       │
│ ② Layer4 项目实例         │ ── 引用 Layer3 ──►    │
│ <project>/.ae-sdd/       │   + overrides 覆盖    │
│   config.yaml            │                       │
│   state.json             │                       │
│   assets/ (引用)          │                       │
│   overrides/ (项目定制)   │                       │
└──────────────────────────┘                       │
                                                   │
┌──────────────────────────────────────────────────┘
│
│  ④ 安装引导 SKILL 串联 ②③④：
│     install-skill.md → 调 install.py → 装到 Layer3 → 调 init.py → 建 Layer4
│
│  ⑥ Harness 适配层（派生，非本体）：
│     source/SKILL.md + HARNESS.md ──adapter SKILL──► harness/.harness/agent.md
│     （Mavis 团队级 agent 入口，由 .adapter.lock 标记来源 commit）
└────────────────────────────────────────────────────────────────────┘
```

### 各子系统维护边界判定（扩展原判定表）

> 当你改动文件时，先确认属于哪个子系统，再查"连带项"。

| 改动源（子系统） | 连带影响的子系统 | 必做同步动作 | 权威检查 |
|----------------|----------------|------------|---------|
| **① SKILL 本体**（改 `source/*.md`） | ②③（build+install 重新分发） | 改完跑 `dev-sync.sh`；更新 README:5 版本号 | `update-check` UC-01/05 |
| **③ 构建脚本**（改 `scripts/build_dist.py`） | ②（dist 产出内容变化） | 确认白名单含新 scanner；重跑 build 验证 dist 完整 | `update-check` UC-04 |
| **③ 安装脚本**（改 `scripts/install.py` / `init.py`） | ②Layer3/4 安装行为变化 | ④ install-skill.md 的 §3/§4 流程可能需同步 | 人工核对 |
| **④ 安装引导 SKILL**（改 `install-skill.md`） | 无连带（纯文档） | 确认与 ③ install.py 实际行为一致 | 人工核对 |
| **⑤ 工具链**（改 `tools/lib/*.py` 或 `tools/bin/ae-sdd`） | ①（SKILL 引用的 CLI 命令契约）+ ⑤（测试） | 同步 SKILL.md 命令引用；补/改对应 `tools/tests/test_*.py` | `update-check` UC-02/03 |
| **⑤ 新增 scanner**（`scripts/*_scan.py`） | ③（build 白名单）+ ①（gates.py 注册）+ ⑤（gates _locate） | 加入 `build_dist.py` 白名单；gates.py 注册门禁；SKILL 引用 | `update-check` UC-04 |
| **⑥ Harness 适配层** | ❌ 不手工改 | 母版升级后重跑 `ae-sdd-harness-adapter` SKILL 重新生成 | `.adapter.lock` commit hash 一致性 |
| **任意 source/ 或 tools/** | ①（CHANGELOG）+ README:5 + dev-sync | 写 CHANGELOG；更新 README:5；跑 update-check 全绿才 dev-sync | `update-check` 全量 |

### 维护者 SOP（按子系统）

> **原则：** 本 SOP 是「更新依赖图谱」章节的人读速查版。权威连带项以 `source/standards/update-graph.json` + `ae-sdd update-check --affected` 输出为准，本表不重复 JSON 内容。

**改 ① SKILL 本体（最常见）：**
```
1. 直接编辑 source/*.md
1.5 过一遍 §🔴 极简描述原则 5 问自检（只增不删是禁止项）
2. （涉及门禁/子 SKILL 数变化）同步 README.md 正文计数 + §3 清单
3. 跑 ae-sdd update-check → 全绿
4. 跑 dev-sync.sh 分发
5. 写 CHANGELOG
```

**改 ③ 构建脚本：**
```
1. 编辑 scripts/build_dist.py
2. 若新增 scanner → 确认加入白名单（UC-04 会查）
3. 若 dist 剥离/注入规则变化 → 同步本文件「母版修改后的同步规则」+ README Q3
4. 重跑 build_dist.py 验证 dist/ae-sdd/ 完整
5. 写 CHANGELOG
```

**改 ⑤ 工具链：**
```
1. 编辑 tools/lib/*.py 或 tools/bin/ae-sdd
2. 新增/修改门禁 → 同步 gates.py GATE_REGISTRY + CHECK_FUNCS（UC-02）+ test_gates.py
3. 新增/修改 CLI 子命令 → 同步 SKILL.md 命令引用（UC-03）+ 补 tools/tests/
4. 跑 ae-sdd update-check → 全绿
5. 跑 tools/tests/ 对应测试
6. 写 CHANGELOG
7. 🆕 v3.5.0：如新增 plugin_loader 类的跨 SKILL 加载机制 → 同步 update-graph.json 加 UG-XX 规则（如 UG-12 plugin-registry）+ 新建对应 cross-cutting 加载协议 SKILL（如 ae-sdd-plugin-loader-skill.md）
```

**改 ⑥ Harness 适配层：**
```
❌ 禁止手工编辑 harness/.harness/agent.md
正确流程：
1. 改 source/SKILL.md 或 HARNESS.md（母版）
2. 重跑 ae-sdd-harness-adapter SKILL（convert-ae-sdd-to-harness.ps1）
3. 检查 .adapter.lock 的 commit hash 已更新
```

### 实例化 4 层架构速查（与 SKILL.md §6 互补）

> **📍 权威定义在 [`SKILL.md` §6 实例化机制](../../SKILL.md)，本节只给维护者视角速查。**

| Layer | 名称 | 路径 | 谁生成 | git 跟踪 | 维护者动作 |
|-------|------|------|--------|---------|-----------|
| 1 | 母版 SSOT | `source/` | 开发者编辑 | ✅ | 唯一手工维护点 |
| 2 | 实例化分发包 | `dist/ae-sdd/` | `build_dist.py` | ❌ (gitignored) | 不手工改，构建产物 |
| 3 | 用户安装 | `~/.claude/skills/ae-sdd/` | `install.py` | ❌ | 不手工改，安装产物 |
| 4 | 项目实例 | `<project>/.ae-sdd/` | `init.py`（`ae-sdd init`） | ❌ (项目侧) | 不手工改，项目侧产物 |

**⚠️ 已知缺口（2026-06-25 更新）：** SKILL.md §6 描述的 `ae-sdd init <project-dir> <project-key>` 命令已于 v3.2.5（2026-06-25）挂载到 CLI（`tools/bin/ae-sdd`，通过 subprocess 调 `scripts/init.py`，透传全部参数），UC-03 历史遗留 warn 已清零该项。`ae-sdd fork` 仍为**未来命令**（v3.0 双目录分层 + overrides/ 机制已覆盖 fork 语义，暂不实现），保留在 UC-03 `HISTORICAL_UNIMPLEMENTED` 集合中（warn 状态）。`run`/`skill`/`sync-tools` 同属历史声明，标未来命令。维护者若补全 fork 等，需同步：CLI 注册 + SKILL.md §6 + install-skill.md + 本节 + update_graph.py `HISTORICAL_UNIMPLEMENTED` 集合。

---

## SKILL 边界判定表（新增/修改内容时使用）

> 当你拿到一段内容，问自己"这段应该写在哪？"——用本表判定。

| 内容类型 | 判定依据 | 应写入位置 | 严禁位置 |
|---------|---------|-----------|---------|
| **流程节点定义**（Phase 1 包含什么子步骤） | 这是"流程怎么走" | `SKILL.md` | 各子 SKILL |
| **节点触发条件**（如"② Review 必须在 ① 完成后"） | 编排级门禁 | `SKILL.md` | 各子 SKILL |
| **流程状态机**（state.json 结构 / 流程脱离与再启动） | 全局状态 | `SKILL.md` | 各子 SKILL |
| **多 Agent 角色库**（story-writer / code-reviewer 等） | 编排层"由谁做" | `SKILL.md` | 各子 SKILL |
| **整体执行清单**（10 项节点级 Checklist） | 编排层门禁 | `SKILL.md` | 各子 SKILL |
| **某阶段的具体步骤**（如"第零步：Story 准入检查"） | 这是"Story Review 阶段怎么做" | `story-review-skill.md` | `SKILL.md` |
| **某阶段的讲解模板**（如"📖 Story 讲解模板"） | 阶段内的"讲故事"细节 | `story-review-skill.md`（对应阶段） | `SKILL.md` |
| **某阶段的报告模板**（CodeReview 9 章节） | 阶段内的产出物空白 | `templates/coding/be-codereview-template.md` | `SKILL.md` |
| **Code Review 评审流程**（准入/多维评审/闸门/异常路径/多 Agent） | 阶段内的具体规则 | **`code-review-skill.md`（🆕 2026-06-06 新建）** | `SKILL.md`（仅保留角色 7 指针） |
| **Code Review CodeReviewUpdatePlan 模板** | 阶段内产出物空白 | `code-review-skill.md §第四步 bis` 模板（内嵌，不独立成文件） | `templates/coding/` |
| **某阶段的闸门规则**（⑥bis 一致性闸 / ⑦bis 对称性闸 / 8 类测试真实性禁止 / 全文档回扫 / 禁裸 ✅ / 报告-代码对账 / 产出物对账 / 真实 DB-HTTP 覆盖） | 阶段内的硬约束 | **`code-review-skill.md`（🆕 2026-06-06 新建独立 SKILL）** | `SKILL.md` / `coding-skill.md`（已迁出，coding-skill 改指针） |
| **某阶段的错误排查**（4 层问题排查） | 阶段内的出错处理 | `coding-skill.md` | `SKILL.md` |
| **某阶段的术语定义**（如"DR 是什么"） | 阶段内的概念 | **不放任何文件，靠上下文理解**；如必须定义，写在阶段文件顶部"概念说明" | `SKILL.md` |
| **某阶段的禁止事项**（如"测试真实性 8 类禁止"） | 阶段内的强约束 | `coding-skill.md` | `SKILL.md`（仅保留高层禁止如"跳过 CodeReview"） |
| **跨阶段的回写规则**（如"Story 变更触发 Task 重生成"） | 跨子 SKILL 的联动 | 写入 `SKILL.md` 的"跨阶段联动"章节 + 各子 SKILL 用指针引用 | 单个子 SKILL 写完整联动逻辑 |
| **Proposal-first 编排门禁**（如"有确认缺陷时先生成 Proposal，再修改 Story"） | 这是"节点之间如何流转" | `SKILL.md` 只写门禁与指针 | `story-review-skill.md` / `story-update-skill.md` 写成全局流程编排 |
| **Story Review 的缺陷修复载体**（问题事实/存疑项/修复建议/影响分析） | 阶段产出物模板 | `proposal-skill.md` + `story-review-checklist.md` | `SKILL.md` |
| **Story Review 如何生成 Proposal**（缺陷如何转建议、待讨论如何处理） | Story Review 阶段内规则 | `story-review-skill.md` | `SKILL.md` |
| **Story Update 如何按 Proposal 执行**（只改 Proposal 覆盖章节、禁止计划外修改） | Story Update 阶段内规则 | `story-update-skill.md` | `SKILL.md` |
| **Story 生成 7 阶段挖掘 SOP**（业务/主流程/AC/接口/数据/Task/①bis） | 阶段内流程骨架 + 外提标准/模板 | **`story-generate-skill.md`（引用 `story-generation-standard.md` + `story-generate-plan-template.md` + `story-writer-prompt-template.md`）** | `SKILL.md`（仅角色 1 指针） |
| **Coding 报告产出 9 章节结构 + 7 道闸** | 阶段内具体规则 | **`coding-report-skill.md`（🆕 2026-06-06 新建）** | `SKILL.md`（仅角色 6 指针） |
| **文档存放标准**（路径模板/命名规则/重入处理/版本号/状态码） | AE 体系横向基础设施 | **`document-storage-skill.md`（🆕 2026-06-06 新建）** | 各子 SKILL（每 SKILL 必引用本文件） |
| **建议书（Proposal）的内容结构**（4 段必填 / 7 道闸 / 5 步走流程） | 阶段内产出物模板 | **`proposal-skill.md`（🆕 2026-06-06 新建，重量级）** | `SKILL.md`（仅流程编排指针） |
| **建议书模板** | 阶段内产出物空白 | `templates/proposal/proposal-template.md` | `SKILL.md`（仅指针） |
| **各 SKILL 的"问题处理"路径**（Code Review/Story Review/Coding 异常/Project Assets 漂移） | 触发 Proposal（不直接生成旧版计划载体） | **`proposal-skill.md` §多渠道接入设计** | 各 SKILL 内置的旧版计划载体（已废弃） |
| **AE 体系统一入口 + 智能路由**（分析用户输入属于哪个节点 + 路由到对应 SKILL） | AE 编排层调度 | **`SKILL.md` §🎯 统一入口与智能路由**（🆕 2026-06-06 增强） | 各子 SKILL 各自判断路由 |
| **任务节点内子任务拆分 + 多 Agent 并行 + 故障补救** | 阶段内并行执行规则 | **`agent-orchestration-skill.md`（🆕 2026-06-06 新建）** | `SKILL.md`（仅含角色 1-8 角色库） |
| **Agent 编排的角色库**（8 角色 story-writer / code-reviewer / test-verifier 等） | 阶段内并行执行 | `agent-orchestration-skill.md` §3 角色库（统一） | `SKILL.md` §🤖（已迁出） |
| **完整实现代码**（方法体、条件分支、循环、try-catch） | 这是"怎么实现"的执行细节 | `coding-skill.md`（Coding SKILL 按 Task 骨架"填肉"） | `templates/design/be-task-template.md`（Task 只写骨架） |
| **Task 骨架**（类骨架 + 方法签名 + 伪代码 ≤10 行 + 依赖工具包） | Task 设计产出物 | `templates/design/be-task-template.md §实现方案` | `coding-skill.md`（不在 Coding 里定义骨架格式） |
| **🆕 4 类需求智能路由**（已有 Story / 中大任务 / 小任务 / 微任务）| 智能路由层调度 | `SKILL.md §智能路由表 + §路由决策算法 2.2` | 各子 SKILL 各自判定 |
| **🆕 任务规模判定规则**（套 Story 7 区模板能否填满） | 智能路由层判定 | `SKILL.md §路由决策算法 2.2 套模板判定步骤` | `task-generate-skill.md`（不重复判定）|
| **🆕 工程根目录路径模板**（重任务 `design/` vs 小任务 `Task/` vs 微任务 `Plan/`）| 文档归属 | `document-storage-skill.md §2.6` | 各子 SKILL 自行决定路径 |
| **🆕 无 Story 上下文独立决策**（TaskSkill / CodingSkill 在无 Story 时如何处理）| 阶段内规则 | `task-generate-skill.md §1.B` + `coding-skill.md §4.2 / §6.0` | `SKILL.md`（不重写）|
| **Story 数据模型字段链路标准**（字段链路明细表 + 横向流转对照图；来源→入参/上下文→分层对象→DB/外部依赖→出参） | Story 模板与 Review 检查项 | `templates/design/*story-template.md` + `story-review-skill.md` | `SKILL.md` |

---

## 内容回写到正确位置的 5 步流程

> 当你发现某段内容"放错位置了"（或新增内容不知放哪），按此流程操作。

> **母版声明（🆕 v3.0 改造）：** `source/` 是 AE 体系唯一母版（SSOT，git 跟踪）。所有 SKILL、templates、standards、assets、scripts 的日常维护只改 `source/`；`dist/ae-sdd/`（构建产物，git ignored）、本地 Claude skills 目录（`~/.claude/skills/ae-sdd/`）等均视为发布/安装产物，不手工维护。

### 步骤 1：识别内容类型

> **🔴 前置——设计意图确认（v3.2.5 新增）：** 如果本次是**修改已有功能设定**（而非新增内容归位），在对照 SKILL 边界判定表之前，**必须先查 [`source/docs/ae-sdd-design.md`](../../docs/ae-sdd-design.md) 中对应能力的设计说明**，确认设计意图后再定位修改位置。
>
> 典型错误：改"review 必须多 Agent"→ 直接改各子 reviewSkill 文字（错）→ 先查 ae-sdd-design.md §多 Agent 编排，理解编排层决策后，应改 `SKILL.md §角色库`（对）。
>
> - **能力设计有疑问** → 先读 ae-sdd-design.md 对应能力模块，再回来执行步骤 2-5
> - **新增内容归位** → 跳过此前置，直接看下面的"SKILL 边界判定表"

对照"SKILL 边界判定表"，明确这段属于"流程编排"还是"环节内具体规则"。

- 流程编排 → 候选位置 `SKILL.md`
- 环节内具体规则 → 候选位置 **对应阶段的子 SKILL**

### 步骤 2：定位目标 SKILL

按"阶段→子 SKILL"映射（2026-06-10 全面重组后）：

| 阶段 | 目标子 SKILL（物理路径） |
|------|------------|
| Story 生成 / Review / ①bis 前端契约 / Story 缺陷修复 | `../phase1-design/story-review-skill.md` 或 `../phase1-design/story-update-skill.md` |
| Task 生成 / Review / Task 0 / Task 修复 | `../phase2-task/task-generate-skill.md` |
| ④bis CodingPlan / ⑤ Coding / 测试真实性 / 异常路径 / 一致性闸 / 对称性闸 | `../phase2-coding/coding-skill.md` |
| CodeReview 报告（产出物空白） | `../../templates/coding/be-codereview-template.md` |
| Coding 报告（产出物空白） | `../../templates/coding/be-coding-report-template.md` |
| 测试用例 / 测试报告（产出物空白） | `../../templates/testcase/*.md` |
| Story / Task / DR / 逻辑汇总（产出物空白） | `../../templates/design/*.md` |
| TestCase 生成 | `../phase1-design/testcase-generate-skill.md` |
| DR Update | `../phase1-design/dr-update-skill.md` |
| 文档存放标准 / 横切依赖 | `../cross-cutting/document-storage-skill.md` |
| 统一问题载体 | `../cross-cutting/proposal-skill.md` |
| 三层 SKILL 注册表加载协议（🆕 v3.5.0）| `../cross-cutting/ae-sdd-plugin-loader-skill.md` |
| Agent 编排 / 跨 AI 工具适配 | `../cross-cutting/agent-orchestration-skill.md` |
| 项目资产管理 | `../cross-cutting/project-assets-update-skill.md` |
| 约束 9 个 | `../../standards/constraints/*.md` |
| 编码思维引擎 | `../../standards/thinking/be-coding-thinking-engine.md` |
| 测试策略 | `../../standards/testing/be-testcase-strategy.md` |
| 项目资产 schema | `../../standards/project-assets/project-assets-schema.md` |
| 项目资产实例 | `../../assets/{projectKey}/*.assets.md` |

### 步骤 3：执行回写

- **新增到子 SKILL** → 在子 SKILL 末尾追加 `## 📋 [章节名]` 标题
- **从 AE-skill 移除重复块** → 在 AE-skill 原位置改为指针：
  ```markdown
  > **📍 详细 [内容] 已下沉到 [`[目标 SKILL]` §[章节锚]](./[目标文件])，本 SKILL 不再重复。**
  > **AE 编排层只关注 [N] 个门禁：** ...
  ```
- **从 templates/ 提取报告模板** → 把空白模板移到 `templates/[阶段]/[模板名].md`，AE-skill 改为指针

### 步骤 4：更新交叉引用

- 涉及多个子 SKILL 的联动 → 在 `SKILL.md` 增加一行"跨阶段联动"指针
- 各子 SKILL 之间相互引用 → 用相对路径 `./xxx-skill.md#锚` 跳转
- 新增 Plan-first 类规则 → AE-skill 只增加"必须先有 Plan 才能进入下一节点"的门禁；Plan 内容、生成规则、执行规则分别放到模板和对应子 SKILL
- **改动 `SKILL.md`**（新增/删除流程节点、改路由场景、改角色库、改门禁数量）→ **必须同步更新 `README.md` 以下章节**：
- **🆕 v3.5.0：** 任何 source/skills/ 下的 SKILL 改动（如新增 `ae-sdd-plugin-loader-skill.md`）→ 必须同步 ae-sdd-update-skill.md §项目结构与设计说明 的 SKILL 计数（22 → 23）+ 该 SKILL 在 "各子 SKILL = 环节内具体规则" 表格中的位置
  - §3 SKILL 功能清单（新增/删除/重命名 SKILL 时）
  - §4.2 典型流程示例（流程步骤变更时）
  - §8.5 常见变更场景表（新增变更类型时）
  - README 末行"最后更新"日期
- **🆕 2026-06-10 未来防御：修改任一 SKILL 时，必须同步更新 `README.md:5` 的版本号**
  - **触发条件：** 任何 SKILL .md 改动（不仅是 `SKILL.md`）→ 必须更新 README 第 5 行 `**版本：** YYYY-MM-DD（最新变更：...）`
  - **操作：** `README.md:5` 的 `**版本：**` 行 = 仓库整体"代际标识"；"最新变更"括号内简述本次变更（如 `+ 4 类需求路由` `+ 工程解耦定位器`）
  - **防止：** 出现"内部大量 2026-06-10 改动但 README 还显示 2026-06-06" 的不一致
  - **章节定位：** `README.md:5` 固定格式 `> **版本：** YYYY-MM-DD（最新变更：...）`

### 步骤 4.1：PRD 级状态机同步清单扩展（🆕 v3.3.0）

> **触发条件：** 修改以下任一文件 → 必须按本表逐项检查/同步：
> - `source/SKILL.md` §1.1~1.6 PRD 级章节
> - `source/skills/cross-cutting/document-storage-skill.md` §3.5 schema
> - `source/templates/design/prd-summary-template.md`
> - `source/HARNESS.md` HS-7 / HS-8 / UserPromptSubmit PRD payload

**PRD 级同步清单（5 项必查）：**

| # | 检查项 | 检查命令 / 位置 | 自动 / 人工 |
|---|--------|---------------|-----------|
| 1 | `document-storage-skill.md §3.5` schema 与 `SKILL.md §1.3` 指针字段名 1:1 一致 | grep 比对 | 自动 |
| 2 | `HARNESS.md` HS-7/HS-8 物理阻断实现存在（`tools/lib/gates.py` HS_REGISTRY） | `grep -n "HS-7\|HS-8" tools/lib/gates.py` | 自动 |
| 3 | 4 个新 CLI 子命令存在（`state prd-check-complete` / `state prd-complete` / `state prd-archive` / `runtime compact`） | `ae-sdd --help \| grep prd-` | 自动 |
| 4 | `standards/update-graph.json` UG-09 规则存在 | `grep -n "UG-09" source/standards/update-graph.json` | 自动 |
| 5 | `prd-summary-template.md` 与 `document-storage-skill.md §3.5` 字段职责分离原则一致（state.md 不重复 state.json 字段）| 人工核对 | 人工 |

**操作：** 改完 PRD 级任一文件 → 跑 `python tools/bin/ae-sdd update-check` → UC-01/UC-03/UC-05 全绿 + UG-09 连带项提示全勾。

**禁止：** ❌ 改 `SKILL.md §1.1~1.6` 但不同步 `document-storage-skill.md §3.5` schema（视为违规）。 ❌ 改 `state.json` schema 但不同步 `ae-sdd-conventions.md §2.3` PRD ID 命名行。

---

### 步骤 4.5：写入 CHANGELOG（🆕 2026-06-10 强制）

> **🔴 强制：** 每次修改 SKILL 母版，必须在 `CHANGELOG/` 目录新建一个 `YYYY-MM-DD-{主题}.md` 文件。

**操作：**
1. **文件命名：** `YYYY-MM-DD-{主题}.md`（如 `2026-06-10-AE-4类需求路由.md`）
2. **位置：** `CHANGELOG/` 目录下（与 SKILL 母版平级）
3. **模板：** 参考 `CHANGELOG/_template.md`
4. **必填内容：** 变更摘要 + 详细变更（文件:行号）+ 触发原因 + 影响范围 + 验证方式 + Reviewer
5. **历史回填：** 历史变更按 MEMORY.md 索引补建 1 个 .md 文件（不丢历史）

**为什么需要：**
- 之前 SKILL 母版无变更日志，"为什么改"信息散落在 SKILL.md frontmatter / 章节内 emoji 标签 / README 末行长段
- git commit 信息是"什么时候改"而不是"为什么改"
- 一个 1 个文件集中"一次大变更"，比 git log 易查阅 100 倍

**禁止：**
- ❌ 修改 SKILL 后不写 CHANGELOG
- ❌ 在 CHANGELOG/ 之外的地方记录 SKILL 变更历史（除 README.md 末行日期 + git commit）
- ❌ 多个变更共用一个文件（每次大变更独立文件）
- ❌ 删 CHANGELOG/ 里的历史文件（永久保留，git 跟踪）

### 步骤 5：验证无重复

执行一次全文 grep：

```bash
# 在 AE-skill 中 grep "已下沉到" — 应能列出所有外链指针
grep -nE "已下沉|已统一存放" SKILL.md

# 在子 SKILL 中 grep 关键章节标题 — 应能在目标位置找到
grep -nE "^## 📋 ①bis|^## 📋 ④bis|^## 📋 测试真实性" *.md
```

---

## 母版修改后的同步规则（强制）

> 本节定义"修改完 AE 母版后，如何让本地运行环境拿到最新内容"。它属于 SKILL 维护工作流，不属于 AE 运行流程。

### 适用范围

任一以下内容发生变更，都适用本节：

- `*.md` 子 SKILL
- `templates/`
- `strategies/`
- `scripts/`
- `project-assets/`
- `README.md`

### 默认规则（🆕 v3.0 双目录分层）

| 对象 | 定位 | 维护方式 |
|------|------|---------|
| `source/`（仓库根 `source/`） | **唯一母版（SSOT）** | 直接修改（开发者编辑这里） |
| `dist/ae-sdd/` | **实例化分发包**（构建产物，git ignored） | 不手工改；由 `bash scripts/build-dist.sh` 从 `source/` 构建 |
| `~/.claude/skills/ae-sdd/` | **本地 Claude skills 安装** | 不手工改；由 `bash scripts/install.sh` 从 `dist/ae-sdd/` 装入 |
| **🆕 v3.0 母版根 `SKILL.md`**（`source/SKILL.md`） | ae-sdd 唯一主入口 | **手工编辑（直接修改主入口）；build 时自动包含** |
| `dist/ae-sdd/SKILL.md`（构建产物） | 分发包入口 | 不手工改；由 build-dist.sh 自动从 `source/SKILL.md` 复制 |
| `~/.claude/skills/ae-sdd/SKILL.md`（安装副本） | 本地 Claude 加载入口 | 不手工改；由 install.sh 自动从 `dist/ae-sdd/SKILL.md` 复制 |

> **🆕 v3.0 重大变更（2026-06-18）：**
> 1. **目录结构重组**：仓库根改为 `source/`（母版）+ `dist/ae-sdd/`（分发包）双目录。
> 2. **主入口已就位**：`source/SKILL.md` 即为 ae-sdd 唯一主入口（直接编辑），不再从 `skills/orchestration/ae-sdd-skill.md` 派生（原派生文件已删除）。
> 3. **废弃 `plugins/ae-sdd/`**：v3.0 起 marketplace plugin 副本路径改为 `dist/ae-sdd/`，`plugins/ae-sdd/` 整个废弃。
> 4. **脚本重命名**：`sync-to-plugin.sh` → `build-dist.sh`（构建）+ `install.sh`（安装）+ `dev-sync.sh`（开发者工具）。
> 5. **安装路径简化**：`~/.claude/skills/ae-sdd/skills/ae-sdd/` → `~/.claude/skills/ae-sdd/`（去掉多余中间层）。

### 修改后动作（🆕 v3.0 工作流）

1. 完成母版修改（直接改 `source/SKILL.md`、`source/skills/xxx-skill.md` 等主入口文件）。
2. 执行本文件 §"内容回写到正确位置的 5 步流程" 中的重复性校验。
3. 如本次变更需要在本地 Claude Skill 中立即生效，运行：

   ```bash
   # 开发者推荐：build + install 一步到位
   bash scripts/dev-sync.sh

   # 或显式两步：
   bash scripts/build-dist.sh  # source/ → dist/ae-sdd/
   bash scripts/install.sh     # dist/ae-sdd/ → ~/.claude/skills/ae-sdd/
   ```

4. 确认同步目标目录（**两个**，由 dev-sync 链式调用产出）：

   ```text
   1) <仓库根>/dist/ae-sdd/SKILL.md                      # 实例化分发包（build 产物）
   2) ~/.claude/skills/ae-sdd/SKILL.md                   # 本地 Claude skills 安装（install 产物）
   ```

5. 两个产物下的 `SKILL.md` 应与 `source/SKILL.md` **完全一致**（tar 整树复制保证）。
6. 在最终回复中明确说明：本次是否已执行 dev-sync / build-dist / install；如未执行，说明"仅修改母版，尚未分发/安装"。

### 同步脚本说明（🆕 v3.0 三脚本分工）

| 脚本 | 位置 | 职责 |
|------|------|------|
| `build-dist.sh` | `scripts/build-dist.sh` | 从 `source/` 构建 `dist/ae-sdd/`（注入 VERSION + plugin.json，剥离 CHANGELOG/docs/marketplace.json）|
| `install.sh` / `install.ps1` | `scripts/install.{sh,ps1}` | 从 `dist/ae-sdd/` 装到 `~/.claude/skills/ae-sdd/`（跨平台 + 本地/远程两模式）|
| `dev-sync.sh` | `scripts/dev-sync.sh` | 开发者工具：build + install 组合 + `--watch` 监听模式 + `--uninstall` |

### 🆕 v3.4.0 自动分发闭环（post-commit hook）

> **v3.4.0 之前的债**：母版改完后，**全靠开发者主动跑 `dev-sync.sh`**。12 个 v3.2.x commit 期间没人跑 → `harness/.harness/agent.md` 停在 v3.1.2、已装 SKILL 停在 v3.2.3、`.adapter.lock` 漂移到 3.3.0。
>
> **v3.4.0 解决方案**：母版仓根目录 `.githooks/post-commit` 自动跑分发链，git hooksPath 设为 `.githooks`，让 hook 跟仓库一起分发。

**自动触发链**（详见 HARNESS.md §"分发闭环 v3.4.0+"）：

```
[ae-sdd 母版 commit]
   ↓ .githooks/post-commit (git hooksPath = .githooks)
build_dist.py (source → dist/ae-sdd)
   ↓
install.py --target-path ~/.claude/skills/ae-sdd --quiet
install.py --target-path ~/.zcode/skills/ae-sdd --quiet
   ↓
ae-sdd-harness-adapter/scripts/convert-ae-sdd-to-harness.ps1
   ↓
mavis harness remount
```

**跳过策略**：
- 非母版仓库 → 静默跳过
- 仅修改 `source/CHANGELOG/` 或 `README.md` → 跳过（无功能性变更）
- `SKIP_AE_SDD_HOOK=1` → 跳过（紧急旁路）
- 任一步骤失败 → 不回滚 commit，仅报错

**何时仍需手工 dev-sync**（hook 失效场景）：
1. **开发机全局禁用 hook**（`git config --global core.hooksPath /dev/null`）
2. **Windows 上 Git Bash 路径含空格**导致子进程失败
3. **mavis daemon 没跑**但想强制 build+install（不依赖 mavis mount）
4. **首次安装**：clone 完仓库后还没装 hook → 跑一次 `bash scripts/install-hooks.sh`（自动 git config core.hooksPath = .githooks）

**`ae-sdd health master-freshness` 输出解读**：

| 场景 | 输出 |
|------|------|
| 完全一致 | `✅ 全部一致 (master=3.4.0, project=3.4.0, hook=✅)` |
| CLI ≠ source | `❌ CLI 3.4.0 ≠ source/SKILL.md 3.5.0` |
| CLI ≠ project | `❌ CLI 3.4.0 ≠ .ae-sdd/config.yaml master.version 3.3.0` |
| hook 未装 | `❌ post-commit hook 未装: .githooks/post-commit 不存在` |

**反向兜底**：母版分发链路 4 个版本源（C LI / source / dist / installed）任一漂移 → `ae-sdd health` 会立即报告，**修复建议**显示在 message 字段。

**build-dist.sh 详细职责：**
1. **校验母版 `source/SKILL.md` 存在性**（不存在则终止）。
2. 从 `source/SKILL.md` YAML frontmatter 提取 `version` 字段。
3. `tar` 整树复制 `source/` → `dist/ae-sdd/`，剥离 `CHANGELOG/` `docs/` `.idea/`。
4. 剥离 `.claude-plugin/marketplace.json`（分发包不携带 marketplace 注册表）。
5. 注入 `dist/ae-sdd/VERSION`（含 version + buildDate）。
6. 注入 `dist/ae-sdd/.claude-plugin/plugin.json`（plugin 自描述元数据）。
7. 验证 `dist/ae-sdd/SKILL.md` 存在性。

**install.sh 详细职责：**
1. 检测运行模式（远程 git clone / 远程 zip / 本地 build / 本地 dist）。
2. 自动调 `build-dist.sh`（如果 dist 不存在）。
3. 备份旧版（`${DST}.bak.<时间戳>`）。
4. `cp -r dist/ae-sdd/. ~/.claude/skills/ae-sdd/`。
5. 验证 SKILL.md + VERSION 写入。

### 禁止

| 禁止 | 原因 | 正确做法 |
|------|------|---------|
| 只改 `dist/ae-sdd/` 或本地 Claude skills 目录 | 产物会被下次 build/install 覆盖，母版丢失变更 | 只改 `source/` 母版 |
| 同时手工维护母版和分发包 | 双源漂移，无法判断哪个是权威版本 | 母版单点维护，分发包由 `build-dist.sh` 生成 |
| 修改母版后假设运行环境已自动更新 | 当前没有自动触发同步机制 | 需要即时生效时显式运行 `bash scripts/dev-sync.sh` |
| ~~手工编辑 `SKILL.md`~~ | ❌ v3.0 已废除此规则 | ✅ **v3.0 起，`source/SKILL.md` 是主入口，**直接编辑即可** |
| ~~修改 `ae-sdd-skill.md` 后同步~~ | ❌ v3.0 已废除 | ✅ **v3.0 起，**直接修改 `source/SKILL.md`**，然后跑 `dev-sync.sh`** |
| ~~运行 `sync-to-plugin.sh`~~ | ❌ v3.0 已重命名 | ✅ **v3.0 起，**运行 `build-dist.sh` + `install.sh`（或 `dev-sync.sh` 一步到位）** |
| ~~把构建产物 commit 到 git~~ | ❌ v3.0 已加 gitignore | ✅ **`dist/` 在 .gitignore 内，不应 commit** |

---

## 更新依赖图谱（🆕 v3.2 — 改了 A 要同步 BCDEFG，杜绝漏更新）

> **前置——设计意图确认（v3.2.5 新增）：** 本节管的是"改了 A 之后连带同步哪些 B/C/D"（物理层）。在查本节之前，先阅读 **[`source/docs/ae-sdd-design.md`](../../docs/ae-sdd-design.md)** 中对应能力模块，确认当前修改的是正确的设计层（流程编排层 / 子 SKILL 节点内规则层 / 工具链层 / 模板层），再用本节查连带项。两步缺一不可。
>
> **设计动机：** 原同步规则是线性的"改母版 → 跑 dev-sync"，但**改了 A 之后要同步哪些 B/C/D 无表可查**，靠人记忆必然漏。本节固化"变更触发源 → 连带项"的依赖图谱，配套 `ae-sdd update-check` 自动兜底检查。

### 📍 权威源（机器可读，Agent 必须从这里消费）

> 🔴 **权威源是 `source/standards/update-graph.json`，不是下面的 Markdown 表。** Markdown 表仅供人快速浏览，可能与 JSON 漂移。Agent（含 ae-sdd 自身）查询连带项时，**必须**通过程序化 API 消费 JSON，禁止解析 Markdown 表格。

**图谱数据文件**：`source/standards/update-graph.json`

结构（每条 rule = 一个"改了 trigger → 同步 affected → 跑 checks"的依赖）：
```json
{
  "rules": [
    {
      "id": "UG-02",
      "name": "gates.py 门禁变更",
      "trigger": ["tools/lib/gates.py"],
      "trigger_condition": "新增/修改/删除门禁",
      "affected": [
        {"path": "tools/tests/test_gates.py", "action": "...", "auto_checkable": false},
        ...
      ],
      "checks": ["UC-02", "UC-03"]
    }
  ]
}
```

### 🤖 Agent 程序化消费协议（强制 — Agent 改完文件后必做）

Agent 完成任何 `source/` 或 `tools/` 改动后，**必须**执行以下两步，禁止跳过：

**第 1 步：查询连带项**（改了什么 → 该同步什么）
```bash
ae-sdd update-check --affected tools/lib/gates.py,scripts/ra_authenticity_scan.py
```
或 Python API：
```python
from lib import update_graph
qr = update_graph.query_affected(["tools/lib/gates.py"])
# qr.affected_items  → 连带项清单（path/action/auto_checkable）
# qr.checks_to_run   → 该跑的 UC-XX 检查 ID
```
Agent 拿到 `affected_items` 后，**逐项确认是否已同步**：
- `auto_checkable=true` 的项 → 第 2 步会自动验证
- `auto_checkable=false` 的项 → Agent 必须人工核对并补齐

**第 2 步：跑检查验证**（兜底，防漏）
```bash
ae-sdd update-check          # 全量跑 UC-01~UC-05
# 或只跑第 1 步返回的 checks_to_run
ae-sdd update-check --only UC-02
```
- 全 ✅ → 闭环完整，可 dev-sync
- 有 ❌ → 按 fix 提示补齐，重跑直到全 ✅

> 🔴 **门禁：** dev-sync 前必须 `ae-sdd update-check` 全绿（error 级 0 failed）。Agent 不得在 update-check 有 failed 时跑 dev-sync。

### 人读视图（仅供参考，非权威）

> ⚠️ 下表是 JSON 的简化视图，可能漂移。精确连带项以 `ae-sdd update-check --affected` 输出为准。

| 触发源 | 连带项（简）| 检查 |
|--------|------------|------|
| SKILL.md version | paths.py MASTER_VERSION / README.md:5 / dist VERSION | UC-01 |
| gates.py 门禁 | CHECK_FUNCS 一致 / test_gates 断言 / CLI 注释 / SKILL 命令契约 | UC-02+UC-03 |
| ae-sdd 子命令 | SKILL 引用存在 / 注释 / 测试 | UC-03 |
| *_scan.py 扫描器 | build_dist 白名单 / gates _locate / SKILL 引用 | UC-04 |
| 子 SKILL .md | 健康度清单 / templates / constraints | UC-05 |
| templates .md | 子 SKILL 锚点 / 章节 1:1 | UC-05 |
| update_graph.py/json | 图谱表 / 测试 / CLI / 本章节 | 全量 |
| 任意 source/tools | CHANGELOG / README:5 / dev-sync | UC-01+03+04+05 |

### 图谱使用 SOP（Agent 流程）

```
改动前：
  1. `ae-sdd update-check --affected <改动的文件>`  → 拿到 affected_items + checks_to_run

改动中：
  2. 按 affected_items 逐项同步（auto_checkable=false 的项必须人工核对）

改动后：
  3. `ae-sdd update-check` 兜底验证 → 全绿（0 failed）才继续
  4. 跑 dev-sync 分发
  5. 写 CHANGELOG
```

> 详见上方「🤖 Agent 程序化消费协议」。人工维护者也可直接读 `source/standards/update-graph.json`。

### 检查器 5 项（`tools/lib/update_graph.py`）

| 检查 ID | 检查内容 | 严重度 | 通过条件 |
|---------|---------|--------|---------|
| **UC-01** | 版本号一致性：SKILL.md / paths.py / README.md 三处 | error | 完全一致 |
| **UC-02** | 门禁注册一致性：GATE_REGISTRY 每个 id 在 CHECK_FUNCS 或 check_all 特判 | error | 全覆盖 |
| **UC-03** | 命令契约闭环：SKILL.md 引用的 `ae-sdd <cmd>` 在 CLI 实现 | error（本次新增）/ warn（历史遗留）| 本次新增全实现 |
| **UC-04** | 扫描器分发一致性：scripts/*_scan.py 在 build_dist.py 白名单 | error | 全在白名单 |
| **UC-05** | 健康度清单覆盖：本文件清单含关键组件 | warn | 关键组件齐 |

### 图谱维护规则

- **新增组件类型**（如未来加 `*.validator.py`）→ 在 `source/standards/update-graph.json` 追加一条 rule + update_graph.py 加对应 UC-XX 检查 + 本章节人读视图追加一行
- **权威源是 JSON**：图谱表与 JSON 漂移时，以 JSON 为准并修正 Markdown 表。检查器 UC-XX 验证的是 JSON 描述的依赖，不是 Markdown 表
- **图谱与检查器必须同步**：JSON 每条 rule 的 `checks` 字段指向的 UC-XX，必须在 update_graph.py 有实现
- **历史遗留命令**（assets/fork/init/run/skill/sync-tools）在 UC-03 标 warn，不阻断，待后续迭代实现

---

## 设计-实现一致性迭代检查（🆕 v3.5.3 — 每月/重大变更后跑，补 UC 自动检查的盲区）

> **🆕 v3.5.3 新增（2026-06-27，用户需求"执行一次迭代检查，看看有没有设计不一致的实现，把这个逻辑写进 SKILL"）：** UC-01~07 是**自动化机器检查**（版本号 / 门禁注册 / 命令注册 / 扫描器分发 / 清单覆盖 / HS 声明定位），但有盲区——它查"HS 规则声明在哪个文件"，**不查物理拦截实现是否齐全**；查"SKILL.md 引用的命令注册了没"，**不查幽灵命令整段描述是否清理**；查"F-1 交叉验证函数存在没"，**不查它覆盖了几个 gate**。本节是**人工/Agent 深度交叉核对 SOP**，补这个盲区，防止"文档撒谎"长期潜伏。

### 为什么需要本节（UC 自动检查够不到的 4 类盲区）

| 盲区 | UC 能查到 | UC 查不到（本节补） | 实测案例（2026-06-27 首次迭代） |
|------|----------|-------------------|------------------------|
| **HS 物理拦截实现** | UC-06 查 HS 声明在 gate_intercept.py / stop_check.py 哪个文件 | HS-4/6/7/8 声明"物理阻断/Stop hook 报警"但**零实现** | HS-7 prd-complete、HS-8 compact 失败、HS-4 ⑥bis、HS-6 测试代码 4 条撒谎 |
| **幽灵命令整段描述** | UC-03 查单个命令是否注册 | 一整段过时设计描述（rules.yaml + sync-tools + .mjs）残留多文件 | SKILL.md:2339-2452 的 v3.0 sync-tools/assets 4 子命令残留 |
| **交叉验证覆盖面** | UC-06 查 `_verify_gate_claims` 函数存在 | F-1 只覆盖 G-08，其余 gate 谎报不校验 | stop_check 硬编码 `_G08_CLEAR_RE` |
| **已实现未接入** | 无对应 UC | 文件已写但 untracked + CLI/gates 未真实 import | document_storage.py 实现 resolve_path 但门禁走文本兜底 |

### 检查时机

- **每月一次**（与 §SKILL 健康度自检清单同期）
- **重大变更后**（新增 HS 规则 / 新增 CLI 命令 / 新增门禁 / 大版本发布前）
- **用户显式要求**"迭代检查 / 一致性检查 / 设计不一致"时

### 检查 SOP（4 步，强制顺序）

#### 步骤 1：跑自动化基线（UC + health + gates）

```bash
ae-sdd update-check     # UC-01~07，记录所有 warn（warn 是深挖线索，非直接结论）
ae-sdd health           # 9 项自检，记录 ❌（注意：母版仓库无 .ae-sdd/ 是设计如此，非缺陷）
ae-sdd gates check      # 28 门禁，确认未被破坏
```

**判定：** update-check 全绿 = 自动检查层无阻断；但 UC-06 的 warn 项必须进步骤 2 深挖（warn = "声明但无物理实现"线索）。

#### 步骤 2 + 3 + 4：跑 `ae-sdd iteration-check`（🆕 v3.5.4 接管步骤 2/3/4 机器粗筛）

```bash
ae-sdd iteration-check [--project <仓库根>] [--json]
```

**机器自动接管**（report-only，不阻断 dev-sync）：
- **IC-1**：扫 SKILL.md 的幽灵命令引用 + 过时技术栈关键词（rules.yaml/.mjs/sync-tools）
- **IC-2**：统计 stop_check.py 的 `_G\d+_CLEAR_RE` 覆盖数 vs GATE_REGISTRY 总数，判定 F-1 覆盖面
- **IC-3**：扫 `tools/lib/*.py` 实现了但全树零 import 的模块 + git untracked 的 `tools/lib/*.py`
- **IC-4**：HARNESS.md 声明的 HS-N vs 三 hook 文件关键词粗筛（HS-3/5 自认降级→info；HS-4/6 未自认→warn；HS-7/8/9/10/11/12 关键词存在→info 通过粗筛）

**人工复核（仍需人工，不可 100% 自动化）**：

| 步骤 | 机器可筛 | 需人工 |
|------|---------|--------|
| **2 HS 物理实现** | 关键词粗筛（IC-4） | "声明物理拦截 vs 实际逻辑真接"语义判定（如 HS-7 关键词 `prd-complete` 在 gate_intercept.py，但 CLI cmd_state_prd_complete 是否真调 check_prd_4_layers 需读代码确认）|
| **3 CLI 命令契约** | 幽灵命令+过时关键词（IC-1）+ F-1 覆盖面计数（IC-2） | "机制描述与实际技术栈脱节"整体观感 |
| **4 已实现未接入** | import 解析 + untracked（IC-3） | 每个"已实现未接入"模块的"声明状态 vs 接入状态"判定（WIP / 死代码）|

**机器 100% 自动化的上限**：步骤 2 语义判定（HS-7 案例：关键词在但逻辑没接，机器只能粗筛）+ 步骤 4 "WIP/死代码"判定（需读实现）。iteration-check 的价值是**消除粗筛工作量**，人工仍需复核语义层。

### 输出物：《设计-实现一致性迭代检查报告》

```markdown
# ae-sdd 设计-实现一致性迭代检查报告（{日期}）

## 自动化基线
- update-check: {✅ N/N | warn 项}
- health: {N/9}（注：母版仓库无 .ae-sdd/ 的 ❌ 是设计如此）
- gates check: {全过 | 失败项}

## 不一致清单（按严重度）
### 🔴 阻断级（文档撒谎：声明存在但实际无）
| # | 项 | 文档声明 | 实际实现 | 定位 file:line |
...
### 🟡 一般级（部分一致：实现存在但缩水）
...
### ✅ 已诚实自认降级（不算撒谎）
...

## 根因分析
...

## 修复建议（按优先级 P0/P1/P2）
...
```

### 与 UC 自动检查的关系（不替代，是补充）

| 维度 | UC-01~07（自动） | 本节迭代检查（人工/Agent） |
|------|----------------|------------------------|
| 触发 | dev-sync 前 / 改完文件 | 每月 / 重大变更 / 用户要求 |
| 深度 | 机器可判定的注册/存在性 | 语义级"声明 vs 实现覆盖面" |
| 盲区 | 无（这是它的边界） | 补 UC 的 4 类盲区 |
| 阻断 | error 级阻断 dev-sync | 报告 + 修复建议，不阻断（由用户决定是否本次迭代修） |
| 关系 | **本节步骤 1 调用 UC 作为基线** | 本节是 UC 的"深挖层" |

> **定位：** UC 是"改完文件立即跑"的快速防线（防漏同步）；本节是"周期性深度体检"（防长期潜伏的文档撒谎）。两者互补，不替代。

### 门禁

- 🔴 本节检查出的 🔴 阻断级不一致（文档撒谎），必须在**下次大版本发布前**修复或降级声明
- 🟡 一般级不一致，纳入下次迭代计划
- ✅ 检查报告须归档到 `source/CHANGELOG/` 或 `source/docs/plans/`（留痕，供下次对比）

---

## SKILL 健康度自检清单（每月或重大变更后跑一次）

### AE-skill 健康度

- [ ] AE-skill 主入口（`source/SKILL.md`）总行数 < 2500（🆕 v3.2.4 修正：v3.0 起 AE-skill 已并入 `source/SKILL.md` 主入口，原"AE-skill 1362 行"锚点已失效；维护者以 `source/SKILL.md` 实际行数为准，目标保持"流程编排为主、具体规则下沉子 SKILL"）
- [ ] AE-skill 中不出现以下关键词的实质内容（出现只能是 1 行指针）：
  - `接口契约完整性` / `调用流程` / `状态展示` / `错误码` / `边界场景` / `联调支持`（6 维度前端契约）
  - `文件顺序` / `类骨架` / `Mapper SQL` / `测试对应` / `验证点` / `调试回滚`（CodingPlan 7 章节）
  - `@Disabled` / `assertTrue(true)` / `catch 吞噬` / `Thread.sleep`（8 类禁止）
  - `全切面一致性` / `对称性`（核查闸）
  - `分层排查` / `4 层排查`（Coding 问题排查）
- [ ] AE-skill 中所有引用子 SKILL 的位置都用相对路径 `./xxx-skill.md`
- [ ] AE-skill 中"📍 已下沉"指针数 ≥ 实际子 SKILL 数 - 1（每个子 SKILL 至少被引用一次）

### 子 SKILL 健康度

- [ ] `story-generate-skill.md` 引用 `story-generation-standard.md` + `story-generate-plan-template.md` + `story-writer-prompt-template.md`，并不再内嵌大段示例/讲解/Agent 模板
- [ ] `story-review-skill.md` 引用 `story-review-checklist.md` + `story-frontend-contract-standard.md`，并声明不生成旧版计划载体
- [ ] `story-update-skill.md` 明确按 Proposal 执行，禁止计划外业务语义修改
- [ ] `task-generate-skill.md` 包含 `## 📖 Task 讲解模板`
- [ ] `task-generate-skill.md` 🆕 2026-06-10 包含 `### 1.A 有 Story 上级文档` + `### 1.B 无 Story 上级文档`（小任务场景独立决策分支）
- [ ] `coding-skill.md` 包含 `## 📖 Code 讲解模板` + `## 📋 ④bis CodingPlan` + `## 📋 测试真实性强制规范` + `## 📋 ⑥bis 编码后全切面一致性核查闸` + `## 📋 ⑦bis 全链路对称性核查闸` + `## 📋 Coding 问题分层排查与修改链`
- [ ] `coding-skill.md` 🆕 2026-06-10 包含 `### 6.0 任务规模 × 文档组合` + `§4.2 按任务规模分支读取` + CodingSkill.Plan/Execute 输入参数条件必填 Story 路径
- [ ] `SKILL.md` 🆕 2026-06-10 智能路由表包含 4 类需求（已有 Story / 中大任务 / 小任务 / 微任务）+ 路由决策算法 2.2 套 Story 7 区模板判定
- [ ] `document-storage-skill.md` 🆕 2026-06-10 包含 `§2.6 三类任务规模 × 文档路径`（重任务 `design/` / 小任务 `Task/` / 微任务 `Plan/`）
- [ ] `templates/coding/be-codereview-template.md` 9 章节齐全
- [ ] Story Review 旧版计划模板若保留，则仅作为历史兼容壳，不再作为运行时输入
- [ ] 🆕 v3.2 `requirement-analysis-skill.md` 包含 `## 🔴 RequirementAnalysisModel（12 维需求分析决策模型）` + `## 第七步：16 道 RA 质量闸`
- [ ] 🆕 v3.2 `templates/design/ra-template.md` 包含 §6.5 衍生规则登记表 + §8.5 衍生 AC 登记表 + §8.6 衍生覆盖率 + §9-bis 业务模式匹配表 + §9-ter 跨域级联效应表 + §13 RA-G01~16 质量闸自检
- [ ] 🆕 v3.2 `tools/lib/gates.py` GATE_REGISTRY 包含 G-RA-1~G-RA-4（RA 文档存在 / 8 维度完整 / 衍生章节完整 / 真实性扫描通过）+ CHECK_FUNCS 注册 + check_all G-RA-4 特判
- [ ] 🆕 v3.2 `scripts/ra_authenticity_scan.py` 存在，8 类禁止规则（vague-ellipsis / no-evidence / fabricated-field / hidden-conflict / masked-gap / placeholder-fill / assumed-no-derivative / missing-timeliness）+ JSON 输出契约与 test_authenticity_scan.py 一致
- [ ] 🆕 v3.2 `tools/lib/gates.py` check_g13 接入 RA 层（六层追溯：RA ↔ DR ↔ Story ↔ Task ↔ Coding Report ↔ CodeReview），RA 为可选层不阻断
- [ ] 🆕 v3.2 `SKILL.md` 含 `## 🛡️ G-RA 需求分析准入门卫` 章节 + 智能路由表 G-RA 门禁列
- [ ] 🆕 v3.2 `tools/lib/update_graph.py` 存在，含 UC-01~UC-05 五项检查 + `check_all`/`summarize`
- [ ] 🆕 v3.2 `tools/bin/ae-sdd` 含 `update-check` 子命令（跑 UC-01~UC-05）
- [ ] 🆕 v3.2 `tools/tests/test_update_graph.py` 存在，覆盖 UC-01~UC-05 各场景
- [ ] 🆕 v3.2 本文件含 `## 更新依赖图谱` 章节（图谱表 + 使用 SOP + 5 项检查说明）
- [ ] 🆕 v3.2.1 `tools/lib/gates.py` 含 `G-CODE-1` 门禁（Coding 真实性）+ `scripts/coding_authenticity_scan.py` 存在
- [ ] 🆕 v3.2.2 `source/skills/cross-cutting/` 含 4 个 toolset SKILL（`database-tool-skill.md` / `git-insight-skill.md` / `memory-management-skill.md` / `toolset-orchestration-skill.md`）
- [ ] 🆕 v3.2.2 `source/standards/toolsets/` 含 4 份 toolset 标准（`db-connection-profile.schema.md` / `git-insight.md` / `memory-layering.md` / `toolset-security.md`）
- [ ] 🆕 v3.2.2 `tools/lib/` 含 3 个 toolset 实现模块（`db_tool.py` 只读优先 + 本地 profile / `git_insight.py` 只读 JSON 输出 / `memory_store.py` 阶段感知 JSONL）
- [ ] 🆕 v3.2.2 `tools/bin/ae-sdd` CLI 含 `memory` / `db` / `git` 三组子命令（`memory enter/write/exit/read/search/promote/summarize`、`db profiles/query/explain/audit`、`git status/diff/log/blame/impact`）
- [ ] 🆕 v3.2.2 `tools/tests/test_toolsets.py` 存在，覆盖三组 toolset 子命令
- [ ] 🆕 v3.2.3 `tools/lib/memory_gate.py` 存在，阶段切换强制记忆检查（CLI 与 PreToolUse gate 共用 `check_exit_ready`）
- [ ] 🆕 v3.2.3 `tools/lib/gate_intercept.py` 在 entry-gate 检查前运行 memory gate；`ae-sdd state write --phase <next>` 离开 RA/design/coding-plan/coding/review 关联阶段时阻断（无 `memory enter→write`）
- [ ] 🆕 v3.2.3 `tools/tests/test_memory_gate.py` 存在，覆盖阶段切换记忆门禁各场景
- [ ] 🆕 v3.2.3 `memory_store.py` 含非破坏性 `check_exit_ready()`（gates 可校验 memory 而不写 exit 事件）+ `--allow-empty-memory` 维护 override
- [ ] 🆕 v3.2.4 本文件含 `## 项目结构与设计说明` 章节（6 子系统总览 + 协同关系图 + 子系统维护边界判定表 + 维护者 SOP + 实例化 4 层速查）
- [ ] 🆕 v3.2.4 README.md 正文门禁计数与 `tools/lib/gates.py` GATE_REGISTRY 实际数量一致（🆕 v3.4.0：G-00~14 + G-CODEPLAN-SRC + G-DOC-STORAGE + G-RA-1~4 + G-CODE-1 = 22）
- [ ] 🆕 v3.2.4 README.md 正文子 SKILL 计数与 `source/skills/**/*-skill.md` 实际文件数一致（当前 23；v3.5.0 加 `ae-sdd-plugin-loader-skill.md`）
- [ ] 🆕 v3.2.5 `source/docs/ae-sdd-design.md` 存在，包含 12 个能力模块（端到端流程编排 / 智能路由 / 状态持久化 / 多Agent编排 / 门禁体系 / 项目资产 / 实例化体系 / Harness适配 / 记忆层 / Plan-First / 真实性扫描 / 工具链CLI）
- [ ] 🆕 v3.2.5 本文件 §步骤1 含"设计意图确认"前置块，引用 `ae-sdd-design.md`
- [ ] 🆕 v3.2.5 本文件 `## 更新依赖图谱` 章节含"前置——设计意图确认"引用块
- [ ] 🆕 v3.4.0 `tools/lib/gates.py` GATE_REGISTRY 含 G-14 / G-CODEPLAN-SRC / G-DOC-STORAGE（22 门禁）+ CHECK_FUNCS 注册 + check_g14/check_g_codeplan_src/check_g_doc_storage 实现
- [ ] 🆕 v3.5.7 `tools/lib/gates.py` GATE_REGISTRY 含 G-DOC-CONSISTENCY（25 门禁）+ CHECK_FUNCS 注册 + check_g_doc_consistency 实现（项目侧记忆-配置路径一致性，防旧记忆劫持 config docWorkspacePath）；`source/SKILL.md` 含 §🛡️ G-DOC-CONSISTENCY 门禁章节；`tools/tests/test_gates.py` 含 TestGDocConsistency 4 用例
- [ ] 🆕 v3.5.9 `scripts/ra_depth_scan.py` 存在（5 条机械派生规则 D1-D5：§6.5 主规则机械派生 / §8.5 R'→AC 链接 / §8.6 覆盖率真实重算 / §9-ter 五问覆盖 / §9-bis 业务模式六选一）；`tools/lib/gates.py` GATE_REGISTRY 含 G-RA-5（26 门禁）+ `check_ra_depth` + `check_all` G-RA-5 特判分支；`source/SKILL.md` frontmatter version = v3.5.9 + G-RA 规则表含 G-RA-5 + §🛠️ 工具速查含 `ra-depth-scan` 子命令；`source/skills/phase1-design/requirement-analysis-skill.md` §阶段 E.5/G.5/H.5/H.6 顶部含 G-RA-5 红框 + 7.1 RA-G08/09/12 判定 SOP 升级 + 禁止事项第 22 条 + 执行清单 8.5/10.5/11.6 补 G-RA-5；`tools/tests/test_ra_depth_scan.py` 存在（≥6 用例覆盖 D1-D5）；`tools/tests/test_gates.py` TestCheckAll 断言 26 + TestGRA5 ≥2 用例；`scripts/build_dist.py` runtime_scripts 白名单含 ra_depth_scan.py；`tools/bin/ae-sdd` 含 `cmd_ra_depth_scan` + argparse `ra-depth-scan`；`source/standards/update-graph.json` 含 UG-14 规则；`source/CHANGELOG/` 含 `2026-06-27-v3.5.9-ra-derivation-depth-gate.md`
- [ ] 🆕 v3.6.0 `scripts/ra_implementation_scan.py` 存在（I1-I7：数据源清单 / 数据流链路 / 术语定义不变量 / 现有实现复用证据 / 高成本难实现设计反驳 / 开发者疑问答复 / DR 生成交接包）；`tools/lib/gates.py` GATE_REGISTRY 含 G-RA-6（29 门禁）+ `check_ra_implementation` + `check_all` G-RA-6 特判分支；`tools/bin/ae-sdd` 含 `cmd_ra_implementation_scan` + argparse `ra-implementation-scan` + `gate ra-required` 覆盖 G-RA-1~6 + FLOW；`source/skills/phase1-design/requirement-analysis-skill.md` 含第一步 ter + RAGeneratePlan §2.5 + 禁止事项 #23；`source/templates/design/ra-template.md` 含 §9-quater 七要素表；`tools/tests/test_ra_implementation_scan.py` 与 `tools/tests/test_gates.py::TestGRA6` 存在；`scripts/build_dist.py` runtime_scripts 白名单含 ra_implementation_scan.py；`source/standards/update-graph.json` 含 UG-18；`source/CHANGELOG/` 含 `2026-06-30-v3.6.0-ra-implementation-view-gate.md`
- [ ] 🆕 v3.4.0 `tools/lib/session.py` 存在，entry token 管理（enter/read_session/has_valid_entry_token/confirm_phase/is_phase_confirmed）
- [ ] 🆕 v3.4.0 `tools/bin/ae-sdd` 含 `enter` / `state confirm` / `gate doc-storage` 3 个新子命令
- [ ] 🆕 v3.4.0 `tools/lib/gate_intercept.py` 含关卡2（_PRODUCT_PATTERNS + _PRODUCT_PHASE_MAP + _check_product_landing）+ 关卡3（coding phase 写 src/ 须 task-reviewed 确认）
- [ ] 🆕 v3.4.0 `tools/lib/stop_check.py` 含 _verify_gate_claims（F-1 修复：交叉验证 ◆ GATE 与实际文档）
- [ ] 🆕 v3.4.0 `tools/lib/state.py` PHASE_FLOW 含 ra-generated（11 phase）；`tools/lib/memory_gate.py` STATE_PHASE_TO_MEMORY_PHASE 含 ra-generated→ra（修复 B3-6）
- [ ] 🆕 v3.4.0 `tools/lib/update_graph.py` 含 UC-06 文档-实现一致性检查 + CHECK_FUNCS 注册；`source/standards/update-graph.json` 含 UG-10 规则
- [ ] 🆕 v3.4.0 `source/SKILL.md` 无虚假命令引用（无 `gate ra-required --fix` / `assets check/generate/audit/update` / G-RA "CLI 自动调用"），UC-03/UC-06 兜底
- [ ] 🆕 v3.4.0 `source/skills/cross-cutting/document-storage-skill.md` §0.5.1 四维定位模型（含 docWorkspacePath）+ §0.6.5 E003 落地前强制
- [ ] 🆕 v3.4.0 `source/templates/coding/be-coding-plan-template.md` §5 骨架含来源标记 + §15 门禁 15 条（含 G-CODEPLAN-SRC）+ front-matter 计数修正
- [ ] 🆕 v3.5.2 `source/SKILL.md` 含 `### ⑦ter 流程收尾合规自检` 章节（5 维度自检 + 自愈 SOP + 自检表模板）+ `#### 1.7 PRD 收尾合规自检 SOP`（堵 prd-complete 跳校验漏洞）+ 整体流程图含 ⑦ter 节点 + 执行清单含 10a 行；`source/docs/ae-sdd-design.md` 端到端流程编排模块含 v3.5.2 自检说明
- [ ] 🆕 v3.5.3 本文件含 `## 设计-实现一致性迭代检查` 章节（4 步 SOP：UC 基线 + HS 物理实现核对 + CLI 命令契约深挖 + 已实现未接入扫描 + 报告模板 + 与 UC 关系定位）；`source/docs/ae-sdd-design.md` 工具链 CLI 模块含 v3.5.3 迭代检查说明
- [ ] 🆕 v3.5.4 本文件 `## 设计-实现一致性迭代检查` SOP 步骤 2/3/4 改为调 `ae-sdd iteration-check`（IC-1~4 机器粗筛 + 人工复核语义层）；`source/SKILL.md` 含 `## 🛠️ 工具 API 速查` 重写 + 删除幽灵命令段（v3.0 残留 rules.yaml/.mjs/sync-tools）；`source/HARNESS.md` HS-7/8 升级"已补物理实现"+ HS-4/6 降级自认；`tools/lib/iteration_check.py` + `tools/bin/ae-sdd:cmd_iteration_check` 注册；HS-7 物理拦截复用 `tools/lib/state.py:check_prd_4_layers`
- [ ] 🆕 v3.5.5 `source/SKILL.md` 含 `## ⏱️ 节点级上下文压力软提示` 章节（6 审核点触发表 + 行为约束 + 5 信号表 + 缺省阈值 + config override + SOP + 对话呈现模板 + CLI 速查）+ 整体流程图 6 个审核点后加 ⏱️ 标记 + §🤖 章节含"主会话职责边界"小节；`source/skills/cross-cutting/agent-orchestration-skill.md` 含 §8.5 默认单 sub-agent 模式 + §8.6 节点级派活清单；`tools/lib/context_pressure.py` + `tools/bin/ae-sdd:cmd_context_pressure` + `tools/tests/test_context_pressure.py` 注册并通过；CLI 速查表含 `ae-sdd context-pressure [--story <ID>]`
- [ ] 🆕 v3.5.11 `tools/lib/alignment_audit.py` 存在（AA 全维对齐验证器，5 维 UC-08~12：门禁承诺↔注册双向 / 门禁实现真实性抓 stub-pass / state 字段存活性 / 状态机闭环 / 幽灵命令全捕获）+ `register_to_update_graph()` 注入 update_graph.CHECK_FUNCS；`tools/bin/ae-sdd:cmd_update_check` import alignment_audit 触发注册 + 12 项全量调度；`tools/tests/test_alignment_audit.py` ≥12 用例；`tools/lib/gates.py` 修复 G-RA-FLOW-VIOLATION 假门禁（check_all 特判传 master_source + `_sys`→`sys` NameError）+ `tools/tests/test_gates.py` TestGRAFlowViolation 3 用例；`source/SKILL.md` frontmatter v3.5.11 + AA 描述；5 个核心 review/生成 SKILL（dr-review/story-review/code-review/requirement-analysis/task-generate）顶部含🟠门禁强度声明；`requirement-analysis-skill.md` 删除 `run dr-review-skill`/`run story-update-skill` 幽灵命令引用；`source/standards/update-graph.json` 含 UG-15 规则；`source/docs/plans/2026-06-29-v3.5.11-aa-tracking-list.md` 存在（AA 首跑 gap 留痕）

### 跨 SKILL 一致性

- [ ] AE-skill 的"子 SKILL 索引"表覆盖目录中所有 `*-skill.md` 文件
- [ ] AE-skill 的"执行清单"中每个步骤都指向唯一一个具体子 SKILL
- [ ] 任何 SKILL 不重复定义"DR 是什么 / Story 是什么"等基础概念（依赖上下文）
- [ ] **README.md §3 SKILL 功能清单与目录中实际 `*-skill.md` 文件数量一致**（不多不少）
- [ ] **README.md 末行"最后更新"日期 ≥ 本次变更日期**（每次修改任一 SKILL 后必须更新）
- [ ] 如本次变更需要即时生效，已运行 `bash scripts/dev-sync.sh`（或显式 `build-dist.sh` + `install.sh`），并确认 `dist/ae-sdd/SKILL.md` 与 `~/.claude/skills/ae-sdd/SKILL.md` 已刷新；如未运行，最终回复已明确说明"仅修改母版"

---

## 禁止的 6 种反模式

| 反模式 | 危害 | 正确做法 |
|--------|------|---------|
| 更新 SKILL/Doc 时只增不删（背景叙事/历史版本堆积） | 文件只增不减 → 重现"2839 行读不动"问题 | 每次更新都过一遍 §🔴 极简描述原则 5 问自检 |
| 在 AE-skill 中塞"具体怎么做" | AE-skill 膨胀 → 维护困难 → 与子 SKILL 内容漂移 | 内容下沉到子 SKILL，AE-skill 只保留指针 |
| 同一规则在 AE-skill 和子 SKILL 各写一份 | 两份内容漂移 → 改一处忘改另一处 | 单点维护：要么在 AE-skill 要么在子 SKILL，用指针互引 |
| 子 SKILL 中写"流程编排" | 子 SKILL 承担不该承担的事 → 跨阶段改动要改多个文件 | 流程编排一律在 AE-skill，子 SKILL 只接收"我这一阶段的指针" |
| templates/ 与 SKILL 中并存报告模板 | 模板改动两份不一致 | 报告空白模板只在 `templates/`，SKILL 中只写"按 templates/xxx 模板生成" |
| 手工修改插件副本或本地安装目录 | 产物与母版漂移，下次同步会丢变更 | 只改 `skills/ae-sdd/` 母版，需要生效时运行同步脚本 |
| 修改母版后不说明是否同步 | 使用者不知道当前运行环境是否已拿到新规则 | 最终回复必须说明"已同步"或"仅修改母版，未同步" |

---

## 与其他 SKILL 的关系

- **本 SKILL 不是工作流**，是 SKILL 维护的工作流。仅在新增/修改 SKILL 内容时参考。
- **执行 SKILL 内容时不要加载本 SKILL**（会污染上下文）。判断方式：
  - 用户说"开始 AE 流程"/"从 DR 开始" → 加载 `SKILL.md`
  - 用户说"重构 SKILL"/"新增 SKILL"/"SKILL 内容分类" → 加载本 SKILL

---

## 本次重构摘要（2026-06-04）

| 移除项（原 AE-skill 行号） | 大小 | 下沉到 |
|----------------------|------|--------|
| ①bis 6 维度前端契约（L825-1043） | 5.7KB | `story-review-skill.md` |
| ④bis CodingPlan 7 章节（L1327-1502） | 6.3KB | `coding-skill.md` |
| 🔴 测试真实性 8 类禁止（L1604-1766） | 4.4KB | `coding-skill.md` |
| ⑥bis 一致性闸（L1791-1837） | 1.9KB | `coding-skill.md` |
| 七、CodeReview 报告模板（L1859-2383） | 12.5KB | `templates/coding/be-codereview-template.md`（已存在，本次确认无副本） |
| ⑦bis 对称性闸（L2268-2303） | 1.5KB | `coding-skill.md` |
| 📖 讲解规范 L18-222 三个节点模板 | 4.8KB | `story-review-skill.md` / `task-generate-skill.md` / `coding-skill.md` |
| 异常处理中"Coding 4 层排查"（L1268-1325） | 2.6KB | `coding-skill.md` |

**AE-skill 削减效果：** 134,656 字节 → 75,323 字节（-44%）

---

## 本次重构摘要（2026-06-10 任务规模分级）

| 新增项 | 位置 | 备注 |
|------|------|------|
| 4 类需求智能路由（已有 Story / 中大任务 / 小任务 / 微任务）| `SKILL.md §智能路由表 + §路由决策算法 2.2` | 套 Story 7 区模板自动判定 |
| 事务简称命名规则（`{服务缩写}-{任务简述}`）| `SKILL.md §4 类需求智能路由` + `document-storage-skill.md §2.6` | 服务名缩写：去前缀 `icec-cloud-` 去后缀 `-service`/`-bff` |
| 三类任务路径模板（重任务 `design/` / 小任务 `Task/` / 微任务 `Plan/`）| `document-storage-skill.md §2.6` | 工程根目录相对路径 |
| TaskSkill 加"无 Story 上级文档"分支 | `task-generate-skill.md §1.A / §1.B` | 小任务场景独立决策 |
| CodingSkill Plan/Execute 输入参数条件必填 Story 路径 | `coding-skill.md §CodingSkill 对外调用契约` | 微任务不传 Story 路径 |
| CodingSkill §4.2 按任务规模分支读取 | `coding-skill.md §4.2` | 微任务跳过 Task 文档读取 |
| CodingSkill §6.0 任务规模 × 文档组合 | `coding-skill.md §6.0` | 三类 100% 全流程，仅文档数量不同 |

**核心原则：**
- **流程深度不减**：3 类规模 100% 走 CodingModel 11 维决策 + 14 条 CodingPlan 门禁 + TR-1~TR-7
- **文档数量递减**：重任务 6 类 → 小任务 5 类 → 微任务 3 类
- **独立决策**：无 Story 时 CodingModel 决策 + 核心链路保护照样走，但**禁止伪造 Story 引用**
- **多 Agent 不变**：由 agent-orchestration-skill 按"任务可拆性"判定，与规模无关

---

## 本次重构摘要（2026-06-10 SKILL 母版目录全面重组 + 3 项配套整改）

| 新增项 | 位置 | 备注 |
|------|------|------|
| **13 SKILL 拆 7 子目录** | `skills/orchestration/` + `skills/phase1-design/` + `skills/phase2-task/` + `skills/phase2-coding/` + `skills/phase3-review/` + `skills/cross-cutting/` | 按"流程节点 + 横切依赖"分类 |
| `constraints/` + `strategies/` 合并为 `standards/` | `standards/constraints/` + `standards/thinking/` + `standards/testing/` + `standards/project-assets/` | 原 9 约束 + 3 策略 + 2 schema/template 都进 standards/ |
| `project-assets/` 改名 `assets/` | `assets/{projectKey}/` | 实际项目资产 |
| 小任务/微任务 `.ae-task/` `.ae-plan/` 隐藏目录 | `document-storage-skill.md §2.6` | 避免污染 IDE 视图 |
| 人工审核点 4 → 5（加 CodingPlan 评审，删 Coding 完成评审）| `SKILL.md` 整体流程 + 整体执行清单 + 人工节点表 | 节点编号 1 → 1.5 → 2 → 2.5 → 4 |
| 同步 `sync-to-plugin.sh` 后的新目录 | 母版 → `~/.claude/skills/ae-sdd/skills/ae-sdd/` | ❌ v3.0 已废弃此机制，改为 `source/` → `dist/ae-sdd/` → `~/.claude/skills/ae-sdd/` 三层构建 + 安装 |

**Why：**
- 之前按"文件类型"分（`templates/` `constraints/` `strategies/` `project-assets/` 散落），无法一眼看出"哪个 SKILL 用于哪个流程节点"
- 重组成"流程节点 + 横切依赖"后，AE-skill 编排层 → 节点 SKILL → 横切 SKILL 的调用链一目了然

**关键原则（保持不动）：**
- 单一权威源 = 母版，plugins 副本只读
- 物理目录 + 逻辑分层分离：物理按流程节点，便于维护；逻辑仍是 4 层架构
- 4 类需求路由（已有 Story / 中大 / 小 / 微）继续按 §智能路由表 判定

---

**维护原则：当你不确定一段内容放哪时，先看"SKILL 边界判定表"，再问"这是流程编排还是环节内具体规则"。99% 的情况能立即定位。剩下 1% 在本 SKILL 评论区或 issue 中讨论。**
