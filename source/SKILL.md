---
name: ae-sdd
version: 3.5.15
description: |
  端到端自动化工程 SKILL 体系的主入口（v3.5.15）。从 DR 出发，经过 Story 生成、
  Review、Task 生成、Coding、测试，直到全部通过。当开发者说"启动自动化工程"、
  "从 DR 开始实现"、"端到端实现"、"继续流程"、"继续上次"、"/ae-sdd" 时触发。
  支持流程状态跟踪与中断后恢复。v3.5.15 coding-skill 外科手术式治理——文档瘦身
  2263→1917 行，5 处"已迁出但保留"幽灵章节兑现迁出声明后降级为指针，缺失执行细则
  补齐到 code-review-skill，项目特定经验抽离到独立 lessons-learned.md，修复悬空双轨编号。
  v3.5.15 多入口状态机——单条 PHASE_FLOW 重构为 4 子链 PHASE_FLOWS（大/中/小/微）+ scale 路由，
  修复微任务 next_step 误建议跑 RA 的可观测 bug，BUG/配置类复用微链。
  v3.5.14 UC-13 门禁注册完整性（AA 第 6 维）。v3.5.13 G-09B 升级独立硬门禁。
  v3.5.12 review-loop 编排层 CLI + PRD 子系统补全。
  v3.5.11 AA 全维对齐验证器（UC-08~12）。版本变更日志见 source/CHANGELOG/。
---

<!-- SKILL 元数据（version 权威值已在上 frontmatter，此处 main_entry/triggers 供文档检索） -->
main_entry: true
triggers:
  - "启动自动化工程"
  - "从 DR 开始实现"
  - "端到端实现"
  - "继续流程"
  - "继续上次"
  - "/ae-sdd"
  - "ae-sdd-quick"   # 🆕 v3.1：快速通道触发（参见入口段硬前置声明）
allowed_tools:
  - "ae-sdd"   # 主 CLI（见 §🛠️ 工具 API 速查）
---

> ## 🔴 第一动作（硬前置，v3.4.0 加固 — 2026-06-25，禁止跳过）
>
> 收到 `/ae-sdd` 触发后，**第一动作 = 领入口凭证 + 跑 §🛡️ G-00 项目资产门卫**：
> 1. 🆕 v3.4.0 关卡1：跑 `ae-sdd enter <projectKey> --story <STORY-ID>` 领取 entry token（写 `.auto-engineering/<STORY>/session.json`）。未领凭证的流程产物落地/代码改动将被关卡2/3 物理拦截（HS-9/10/11）。
> 2. 跑 G-00：`ae-sdd gates check --only G-00`（校验项目资产 `assets/<projectKey>/<projectKey>.assets.md` 存在 + 7 层索引齐备）。
>
> 禁止直接读用户问题内容、禁止直接动代码、禁止主观归类为"对话轻量通道"、禁止跳过 §🎯 统一入口路由判定、禁止越层派 sub-agent。
>
> **违规代价**：跳过 G-00 / 跳过 enter 领凭证 = 本次任务失信，下游所有产物需标"事后回溯"。
>
> **快速通道**：用户**显式说** `/ae-sdd-quick` 或 `走快速通道` 时可豁免 G-00 完整 7 步路由，但**仍需落档**（注明快速通道来源 + 项目资产摘要）。快速通道豁免路由，但关卡2/3 物理拦截仍生效（领凭证后产物才能落地）。

# Auto Engineering — 端到端自动化工程 Skill（v3.2 主入口）

> **🆕 v3.1 加固说明（2026-06-22）：**
> - **入口段加"🔴 第一动作（硬前置）"声明**（见上方）—— 解决"root agent 收到 /ae-sdd 触发后跳 G-00 / 跳路由判定 / 直接动手改代码"问题（实测案例：life 项目 STORY-020-BE v3-r2 CodeReview 复盘）
> - 新增 **`source/docs/ae-sdd-conventions.md`**（项目级 SOP 模板）—— 解决"L2 流程纪律承接缺失"
> - 各使用方项目新建 **`ae-sdd-instance.md`**（项目实例化）—— 解决"root agent 没有项目级 ae-sdd 流程可参照"
> - L4 失职检测 hook（`pre-ae-sdd-check.sh`）**延期到 v3.2**（mavis daemon 兼容性未验证）
>
> > **🆕 v3.0 改造说明（2026-06-18）：**
> - 本文件（`SKILL.md`）= **ae-sdd 唯一主入口**。原 `skills/orchestration/ae-sdd-skill.md` 已并入本文件并删除。
> - 强化 **G-00 项目资产门卫**：每个调用必过项目资产完整性检查，缺失时自动触发生成。
> - 新增 **🛠️ 工具 API 速查** 章节：列出 `ae-sdd` CLI 全部 8 个子命令。
> - 新增 **🔧 维护规则与同步机制** 章节：明确"修改 rules → 跑 dev-sync"的工作流（v3.0 起 sync-tools 已废弃，改用 Python CLI）。
> - 角色库/智能路由/Phase 1-3 全流程等核心内容**完整保留**，行数从 1843 → ~2050。
>
> **🟢 兼容性：** v3.0 之前的触发词、状态机、多 Agent 模式、14 条门禁、TR-1~TR-7 全部保留。**没有任何破坏性变更**——只是把规则描述升级为"规则+工具"双轨。

---

## 🛡️ G-00 项目资产门卫（🔴 每次调用必过 — 已有逻辑前置+工具强制）

> **🆕 v3.0 强化：** 本节从原 §路由决策算法 §0（项目资产检查）抽取前置。**逻辑已存在**（步骤 0.1/0.2/0.3），v3.0 把它升级为 G-00 显式门卫并**新增工具强制执行**。

### 强制规则（不可跳过）

| # | 规则 | 工具强制 | 行为 |
|---|------|---------|------|
| 1 | **项目资产文件必须存在** | `ae-sdd gates check --only G-00` | 不存在 → 🔴 阻断 |
| 2 | **7 层索引层必须齐备**（§A-G）| `ae-sdd gates check --only G-00` | 缺失任一层 → 🔴 阻断 |
| 3 | **距 `lastAuditedAt` ≤ 30 天** | `ae-sdd gates check --only G-00` | 过期 → 🟡 警告（不阻断）|
| 4 | **资产不存在时由 AI Agent 触发生成** | 加载 `project-assets-update-skill.md §3` | 调 `project-assets-update-skill.md §3` 生成动作 |

### 执行时机

- **任何 `ae-sdd run` / `state next-step` / `classify` / `gates check` 调用前 → 先跑 G-00**
- **AI Agent 手动调用**：G-00 由 Agent 在路由步骤 0 手动跑 `ae-sdd gates check --only G-00` 验证；G-00 不通过时路由到 `project-assets-update-skill §3` 生成资产
- **AI Agent 手动调用**：进入步骤 1 之前必须先确认 G-00 通过

### 详细 SOP

完整 SOP 见 **§🎯 统一入口与智能路由 → 步骤 0**（原 0.1/0.2/0.3 逻辑保留）。本节只是把"必须做"提到顶部，避免被埋在长文里被忽略。

### 工具命令

```bash
ae-sdd gates check --only G-00
  # G-00 内部校验：项目资产存在 + 7 层索引齐备 + lastAuditedAt ≤ 30 天
  # 返回: { exists, last_audited_at, missing_sections, stale }
  # exists=false → 由 AI Agent 路由到 project-assets-update-skill §3 生成

# 资产生成（无独立 CLI 子命令，由 project-assets-update-skill §3 引导 AI 生成）：
#   加载 project-assets-update-skill.md §3 → 9 步探查 SOP → 产出 .ae-sdd/assets/{workspaceKey}/{workspaceKey}.assets.md
#   （路径模板见 document-storage §2.3，多业务线按 line 分组）

# 资产读取（ES 倒排索引 + BM25，Agent 按需读资产用）：
ae-sdd assets read <stage>          # 按阶段读取资产（基线 KEY × BM25）
ae-sdd assets outline               # 资产大纲（章节列表 + 索引统计）
ae-sdd assets section <name>        # 取整章原文
ae-sdd assets query "<关键词>"      # 精准查（倒排索引命中）
ae-sdd assets stats                 # 索引统计
```

> **🔴 不允许跳过 G-00**：即使用户说"直接开始"，也必须先确认项目资产存在。缺失时由 AI Agent 路由到 `project-assets-update-skill §3` 生成资产，而非放行。

---

## 🛡️ G-RA 需求分析准入门卫（🔴 v3.2 加固 — 解决"RA 被绕过"问题）

> **🆕 v3.2 新增（2026-06-24）：** G-00 项目资产门卫解决"有没有项目资产"问题；本 G-RA 门卫解决"Phase 1 下游节点能不能在无 RA 时启动"问题。
>
> **🆕 v3.2 加固说明：**
> - **入口段扩展为 G-00 + G-RA 双门卫**——解决"路由表说 PRD/Issue → requirement-analysis，但旧 4 类需求 fallback 路径仍会绕过 RA"问题（实测案例：life 项目 6 个 Story 全部无 RA 文档）
> - **新增 §🎯 智能路由表 强制门禁列**——每条路由后显式标注"是否需要 RA 前置"
> - **新增 §路由决策算法 1.8 RA 准入门禁**——硬阻断"无 RA 直接进 dr-generate/story-generate/task-generate"

### 为什么需要 G-RA（背景）

**旧版问题（v3.1 及之前）**：

| # | 路径 | 用户输入示例 | 旧路由结果 | 旧 RA 状态 |
|---|------|-----------|-----------|-----------|
| 1 | 4 类需求类型 2 | "做个用户管理功能" | `story-generate-skill.md` | 🔴 无 RA 文档 |
| 2 | 4 类需求类型 3 | "加个缓存预热" | `task-generate-skill.md` | 🔴 无 RA 文档 |
| 3 | 4 类需求类型 4 | "改个枚举值" | `coding-skill.md` | 🟢 跳过 RA（合理）|
| 4 | 4 维判定 | "生成 DR" / "写 DR" | `dr-generate-skill.md` | 🔴 无 RA 文档 |

**实测证据：** life 项目 STORY-002 / 007 / 009 / 010 / 011 / 020 六个 Story 全部无 RA 文档。根因是 4 类需求 fallback 路径说"中大任务 → story-generate"，**没有强制 RA 前置**。

### G-RA 强制规则（不可跳过）

| # | 规则 | 工具强制 | 行为 |
|---|------|---------|------|
| 1 | **进 dr-generate / story-generate / task-generate 前必须存在 RA 文档** | `ae-sdd gate ra-required --project <projectKey> --story <STORY-ID>` | RA 不存在 → 🔴 阻断 |
| 2 | **RA 文档必须含 8 个核心维度**（角色/场景/流程/数据/规则/设计方向/AC/假设）| `ae-sdd gate ra-required` | 任一维度缺失 → 🔴 阻断 |
| 3 | **RA 文档必须完成 RAModel 12 维 + 需求风险预判** | `ae-sdd gate ra-required` + RA-G02/RA-G03 | 任一维度/命中风险无证据闭环 → 🔴 阻断 |
| 4 | **RA 文档必须通过 RA-G01~RA-G16 全部门禁** | `ae-sdd gate ra-required` | 任一 RA 质量闸未过 → 🔴 阻断 |
| 5 | **RA 文档 5 问自检阻断项必须为 0** | `ae-sdd gate ra-required` | 存在未通过结论或用 90% 逃避阻断项 → 🔴 阻断 |
| 6 | **RA 文档必须解决所有 🔴 阻断型缺口** | `ae-sdd gate ra-required` | 任一 🔴 缺口未解决 → 🔴 阻断 |
| 7 | **RA 距今 ≤ 30 天**（防止用过时 RA）| `ae-sdd gate ra-required` | 超 30 天 → 🟡 警告（不阻断，但提示重审）|
| 8 | **微任务（类型 4）豁免** —— 单文件/单枚举值级改动无需 RA | — | 走 coding-skill 直接编码 |
| 9 | **BUG/配置类豁免** —— BUG 修复/配置调整无需 RA（RA 反向通道 §见 requirement-analysis-skill §反向通道）| — | 走 coding-skill BUG 路径 |
| 10 | 🆕 **v3.5.9 RA 机械派生深度通过** —— 验证 E.5/G.5/H.6/H.5 规定的「每行 R→R'→AC 机械追问」是否真做了，防「形式通过、内容空转」 | `ae-sdd gate ra-required` 内调 `scripts/ra_depth_scan.py`（D1-D5 规则） | 5 条规则任一 BLOCKER → 🔴 阻断 |

### 执行时机

- **任何 `dr-generate-skill` / `story-generate-skill` / `task-generate-skill` 启动前 → 先跑 G-RA**
- **AI Agent 手动调用**：G-RA 不在 PHASE_ENTRY_GATES（不由 CLI 自动触发）；Agent 在路由步骤 1.8 手动跑 `ae-sdd gate ra-required` 验证，不通过时路由到 `requirement-analysis-skill`
- 🆕 v3.4.0：RA 需求分析在 `ra-generated` phase 进行（大/中/小链中 initialized → **ra-generated** → dr-generated/story-generated/task-generated）；离开 ra-generated 前自动校验 memory（`STATE_PHASE_TO_MEMORY_PHASE` 含 ra-generated→ra，修复 B3-6）。🆕 v3.5.15：微链（微任务/BUG/配置类）跳过 ra-generated，initialized → coding 直接编码

### 详细 SOP

完整 SOP 见 **§🎯 统一入口与智能路由 → 步骤 1.8 RA 准入门禁**。本节只是把"必须做"提到顶部，避免被埋在长文里被忽略。

### 工具命令

```bash
ae-sdd gate ra-required --project <projectKey> --story <STORY-ID>
  # 返回: { ra_exists, ra_path, dimensions_complete, self_check_pass_rate, blocking_gaps, ra_age_days, blocked, reason }
  # blocked=true → 🔴 阻断 + 输出路由建议
  #
  # ⚠️ RA 缺失/不完整时，由 AI Agent 加载 requirement-analysis-skill 生成或补全 RA
  #    （CLI 不自动触发；G-RA 不在 PHASE_ENTRY_GATES，由 Agent 在路由步骤 1.8 手动调用本命令验证）
  #    完成后回到原路径，重跑本命令确认通过
```

> **🔴 不允许跳过 G-RA**：即使用户说"直接出 Story"，也必须先确认 RA 文档存在 + 8 维度齐全 + RAModel 12 维完整 + RA-G01~RA-G16 全过 + 🔴 缺口已解决。RA 缺失时由 AI Agent 路由到 `requirement-analysis-skill` 生成 RA，而非放行。

### 与旧版 fallback 的兼容性

| 路径 | v3.1 行为 | v3.2 行为 |
|------|----------|----------|
| 4 类需求类型 2（中大任务）| `story-generate-skill`（无 RA 检查）| `story-generate-skill` + G-RA 前置 |
| 4 类需求类型 3（小任务）| `task-generate-skill`（无 RA 检查）| `task-generate-skill` + G-RA 前置 |
| 4 维判定 → dr-generate | `dr-generate-skill`（无 RA 检查）| `dr-generate-skill` + G-RA 前置 |
| 4 维判定 → story-generate | `story-generate-skill`（无 RA 检查）| `story-generate-skill` + G-RA 前置 |
| 类型 4（微任务）| `coding-skill` | `coding-skill`（豁免 G-RA）|
| BUG/配置类 | `coding-skill` | `coding-skill`（豁免 G-RA）|

**升级原则**：v3.2 不破坏 v3.1 的任何路由能力，只是给"中大/小任务"和"dr-generate"3 个路径加 RA 前置门禁；微任务和 BUG 类保持不变。

---

## 🛡️ G-CODEPLAN-SRC CodingPlan 源码核对门卫（🔴 v3.4.0 加固 — 解决"CodingPlan 凭推测设计类骨架"问题）

> **🆕 v3.4.0 新增（2026-06-25，建议书1）：** G-00 解决"有没有项目资产"，G-RA 解决"Phase 1 下游能不能在无 RA 时启动"，本门卫解决"Phase 2 CodingPlan 阶段类骨架是否核对过现有同类源码"问题。
>
> **背景：** life 项目 STORY-020 实测中，AI 出 CodingPlan 时未读 `LatestSideMessagePOConverter`、现有 PO 建模范式、测试范式，凭推测设计"嵌套 Anchor 值对象 + 新增 Converter 映射"，导致改错文件、漏改真正受影响的 Converter、与现有扁平 PO 范式不符。CodeReview 阶段（事后）有"读源码"要求但时序错位——代码都写完了才读，对 CodingPlan（事前）毫无约束力。本门禁把"读源码"前置到 CodingPlan 阶段。

### 强制规则（不可跳过）

| # | 规则 | 工具强制 | 行为 |
|---|------|---------|------|
| 1 | **CodingPlan 关键类骨架章节每个新增/修改类必须附来源标记** | `ae-sdd gates check --only G-CODEPLAN-SRC` | 无任何标记 → 🔴 阻断 |
| 2 | **标记为【已读源码：{路径}】时该文件必须真实存在** | `ae-sdd gates check --only G-CODEPLAN-SRC` | 标已读但文件不存在 → 🔴 阻断（防伪造标记）|
| 3 | **待核实清单非空 → CodingPlan 视为草案，禁止进 ⑤ Coding** | `ae-sdd gates check --only G-CODEPLAN-SRC` | 有【待核实源码】未闭环 → 🔴 阻断 |
| 4 | **微任务豁免** —— CodingPlan 无关键类骨架章节时跳过 | — | 标记 skipped，不阻断 |

### 执行时机

- **④bis CodingPlan 生成后、审核点 2.5 之前 → 跑 G-CODEPLAN-SRC**
- **AI Agent 手动调用**：`ae-sdd gates check --only G-CODEPLAN-SRC`；不通过时回 ④bis 补读源码、把【待核实源码】改为【已读源码：】
- **详细判定标准**（"现有同类源码"= 同包同类 / 同职责类 / Converter·PO·DO 同类型）+ 待核实清单格式见 [`coding-skill.md` §④bis G-CODEPLAN-SRC](../phase2-coding/coding-skill.md)

### 工具命令

```bash
ae-sdd gates check --only G-CODEPLAN-SRC
  # 扫 {STORY-ID}-CodingPlan.md §2 关键类骨架章节的【已读源码：】/【待核实源码】标记
  # 返回: { n_read, n_pending, pending[], missing_read_files[], skipped }
  # n_pending>0 或 missing_read_files 非空 → 🔴 阻断
```

> **🔴 不允许跳过 G-CODEPLAN-SRC**：CodingPlan 阶段凭推测写类骨架、标【待核实源码】未补读、或伪造【已读源码】标记，均禁止进 ⑤ Coding。

---

## 🛡️ G-DOC-STORAGE 文档落地存放门卫（🔴 v3.4.0 加固 — 解决"文档乱放/绕过 resolve_path"问题）

> **🆕 v3.4.0 新增（2026-06-25，建议书2）：** document-storage-skill §0 声明"落地前必须先调 resolve_path"，但这是文档约定无门禁拦截。life 项目 STORY-020 实测中 AI 未调 resolve_path、自行把 CodingPlan 写到 `d:\tmp\`。本门禁把"路径/命名合规"变成可执行门禁。

### 强制规则（不可跳过）

| # | 规则 | 工具强制 | 行为 |
|---|------|---------|------|
| 1 | **流程产出文档（Story/Task/CodingPlan/报告等）必须落在合规根目录**（`ae-sdd-doc/`/`design/`/`.ae-task/`/`.ae-plan/`/`.auto-engineering/`）| `ae-sdd gates check --only G-DOC-STORAGE` | 落在 `d:\tmp\`/根目录等游离位置 → 🔴 阻断 |
| 2 | **落地前必须调 `document-storage.resolve_path()` 推导路径** | AI Agent 调用 resolve_path API | 硬编码绝对路径 → 🔴 阻断 |
| 3 | **E003（gitPath 不存在）落地前强制触发** | resolve_path step 2 | gitPath 无效 → 🔴 阻断（非仅事后 health）|

### 执行时机

- **任何 SKILL 写入流程产出文档前 → 跑 G-DOC-STORAGE**
- **AI Agent 手动调用**：`ae-sdd gates check --only G-DOC-STORAGE` 扫描游离产物；`ae-sdd gate doc-storage --path <实际路径> --intent <intent> --project <key>` 单点校验路径合规性
- **详细四维路径模型**（项目根/微服务根/Story 根/文档工作区根 docWorkspacePath）+ E003 升级见 [`document-storage-skill.md` §0.5.1/§0.6.5](../cross-cutting/document-storage-skill.md)

### 工具命令

```bash
ae-sdd gates check --only G-DOC-STORAGE
  # 扫 project_dir 下 .md，校验流程产物是否落在合规根目录、无游离位置
  # 返回: { stray_files[], checked }

ae-sdd gate doc-storage --path <实际写入路径> --intent <intent> --project <projectKey>
  # 单点校验：给定路径是否合规（匹配 §2.2 8 类流程目录模板 + 命名规则）
  # 不通过 → exit 1
```

> **🔴 不允许跳过 G-DOC-STORAGE**：流程产物乱放游离位置（`d:\tmp\` 等）即阻断；必须经 resolve_path 推导路径。

---

## 🛡️ G-DOC-CONSISTENCY 项目侧记忆-配置路径一致性门卫（🔴 v3.5.7 加固 — 解决"旧记忆劫持 config 路径"问题）

> **🆕 v3.5.7 新增（2026-06-27）：** G-DOC-STORAGE 管"产物落在哪"，G-PATH 管"母版写了什么"，但两者都不管"**项目侧记忆（AGENTS.md/.harness/memory/MEMORY.md）里的文档根表述是否与 config 一致**"。本门禁补这个盲区。
>
> **背景（实测案例）：** life 项目 `.ae-sdd/config.yaml` 写 `docWorkspacePath=D:\Item\life`，但项目自己的 `AGENTS.md` 和 `MEMORY.md` 残留 2026-06-16 旧约定写 `D:\Item\doc`，主会话信了旧记忆，把 RA 文档写到 `D:\Item\doc\iterations\...` 而非 `D:\Item\life\ae-sdd-doc\iterations\...`。"config 是 SSOT"的声明没有强制力，本门禁把它变成可执行门禁。

### 强制规则（不可跳过）

| # | 规则 | 工具强制 | 行为 |
|---|------|---------|------|
| 1 | **项目侧记忆的"文档工作区/文档根"表述须与 `.ae-sdd/config.yaml` 的 `docWorkspacePath` 一致** | `ae-sdd gates check --only G-DOC-CONSISTENCY` | 不一致 → 🔴 阻断 |
| 2 | **config 的 docWorkspacePath 是唯一权威源（SSOT）** | resolve_doc_workspace() | 项目侧记忆表述冲突时以 config 为准 |
| 3 | **降级：无 config.yaml / 无 projectKey / 无 assets.md → warn 不阻断** | — | 同 G-00 缺失策略 |

### 扫描范围与判定

- **记忆文件**（项目根下，存在才扫）：`AGENTS.md` / `.harness/memory/MEMORY.md` / `.harness/agent.md` / `CLAUDE.md`
- **声明式线索词**：行内含"文档工作区/文档根/文档目录/项目文档工作区/Story 文档位于"等之一才视为候选
- **路径比对**：提取行内 Windows 绝对路径，与 config 权威值比对——相等或互为前缀（容忍 `ae-sdd-doc/` 子目录表述）= 一致；否则 🔴 冲突
- **从严判定**：只拦截"=`/`位于"等**声明式**表述；泛泛提及（如"历史路径 X 已作废"）不拦截，避免误伤

### 执行时机

- **任何 SKILL 落地文档前 + G-00 通过后 → 跑 G-DOC-CONSISTENCY**（与 G-DOC-STORAGE 同阶段，两者正交）
- **AI Agent 手动调用**：`ae-sdd gates check --only G-DOC-CONSISTENCY`；不通过时修正项目侧记忆文件使其与 config 一致
- **降级场景**：项目未 init（无 `.ae-sdd/config.yaml`）→ 自动降级 warn，不阻断

### 工具命令

```bash
ae-sdd gates check --only G-DOC-CONSISTENCY
  # 扫项目根下 AGENTS.md / .harness/memory/MEMORY.md / .harness/agent.md / CLAUDE.md
  # 校验"文档工作区/文档根"表述路径是否与 config.yaml docWorkspacePath 一致
  # 返回: { canonical, scanned, conflicts: [{file, line, path, canonical, snippet}] }
  # conflicts 非空 → 🔴 阻断（提示以 config 为 SSOT 修正项目侧记忆）
```

> **🔴 不允许跳过 G-DOC-CONSISTENCY**：项目侧记忆劫持 config 路径（旧表述覆盖新配置）即阻断；config.yaml 的 docWorkspacePath 是唯一权威源。

---

## 🛡️ G-14 CodingPlan-Story 一致性门卫（🔴 v3.4.0 加固 — 对应建议书4 G-08-15）

> **🆕 v3.4.0 新增（2026-06-25，建议书4）：** 入口关卡让 CodingPlan 落对位置，但 Plan 内容是否与 Story 一致仍需检查。本门禁校验 CodingPlan 引用 Story + AC 对齐 + 偏离项有 Proposal，与 G-CODEPLAN-SRC（内容源码核对）正交。

### 强制规则（不可跳过）

| # | 规则 | 工具强制 | 行为 |
|---|------|---------|------|
| 1 | **CodingPlan 须含 Story 文档引用且文件存在** | `ae-sdd gates check --only G-14` | 无 Story 引用或文件不存在 → 🔴 阻断 |
| 2 | **CodingPlan 测试章节 AC ID 与 Story AC 对齐**（Story 含 AC 时至少一个 AC ID 出现在 Plan）| `ae-sdd gates check --only G-14` | Story 有 AC 但 Plan 无 AC ID → 🔴 阻断 |
| 3 | **偏离 Story 设计须有 Proposal 引用** | `ae-sdd gates check --only G-14` | 含"偏离声明"但无 Proposal 引用 → 🔴 阻断 |

### 执行时机

- **④bis CodingPlan 生成后、进 ⑤ Coding 之前 → 跑 G-14**（与 G-CODEPLAN-SRC 同阶段，两者正交）
- **AI Agent 手动调用**：`ae-sdd gates check --only G-14`

### 工具命令

```bash
ae-sdd gates check --only G-14
  # 校验 {STORY-ID}-CodingPlan.md 与 Story 一致性：引用 + AC 对齐 + 偏离 Proposal
  # 返回: { issues[], ac_ids_in_cp[], story_doc_exists }
```

> **🔴 不允许跳过 G-14**：CodingPlan 偏离 Story 设计且无 Proposal 闭环即阻断。

---

## 🔴 输出核心原则（最高优先级，贯穿所有 SKILL 和所有阶段）

> **AI 生成任何内容时必须遵守以下三条，违反即视为输出无效：**

| 原则 | 要求 | 违反示例 |
|------|------|---------|
| **基于事实** | 所有输出必须有明确来源（DR / PRD / 项目资产 / 用户告知 / 代码读取结果）。来源必须可引用、可定位。 | "通常情况下会有一个 status 字段" |
| **禁止猜测** | 对于不确定的信息，不得猜测后输出；必须标注 `{待确认}` 并主动向用户或项目资产提问。 | 自行补全未读取过的接口路径、字段类型、配置值 |
| **禁止杜撰** | 不得编造不存在于输入材料中的业务规则、字段、场景、错误码、类名、配置项。即使内容"看起来合理"，没有来源就不能写进文档或代码。 | 凭经验写一个"应该有的"字段到数据模型里 |

> **遇到信息缺失时的标准动作：**
> 1. 停止生成该部分
> 2. 明确说明"缺少什么信息、应从哪里获取（项目资产 / 用户 / DR / PRD）"
> 3. 等待补充后继续，禁止用占位内容跳过

---

## 🔴 实现方案决策基线（最高优先级，贯穿 Story → Task → Coding 全链路）

> **所有实现方案在落笔前必须完成以下四步，缺任意一步视为实现方案无效，禁止进入下一阶段：**

| 步骤 | 要求 | 适用范围 |
|------|------|---------|
| **① 现有能力复用扫描** | 不只是第三方集成——所有实现点（接口、状态机、领域服务、通知、定时任务、缓存、幂等、外部集成等）都必须先扫描：项目资产已有实现 / 依赖 Story / 历史 Task / 公共组件 / 平台能力 / 团队约定。有则复用，不复用必须写明原因。 | 全部实现点 |
| **② 业内成熟方案参考** | 对非平凡实现点，必须列出业内成熟方案或团队既有成熟实现（如状态机→事件驱动/转移表/领域对象 transition；幂等→唯一约束/乐观锁；通知→通知中台/MQ），并说明采用或不采用的理由。不得凭直觉直接选方案。 | 状态机 / 幂等 / 补偿 / 消息 / 外部集成等非平凡实现 |
| **③ 五维代码质量评估** | 实现方案必须同时满足五维：**可用性**（完整覆盖业务场景/AC）、**高效性**（无重复查询/大事务/阻塞）、**可维护性**（复用团队抽象/能力单一归属）、**健壮性**（覆盖失败/幂等/补偿/可观测）、**可读性**（命名/分层/状态语义清晰）。任一维度不达标 → 🔴 阻断。 | 全部实现点 |
| **④ 核心能力归属唯一** | 每个核心业务能力（如"结单"、"推送"、"状态流转"）只能有一个唯一实现点（owner Task / owner 类 / owner 方法）。多个入口只能调用它，不得各自实现一套。 | 跨 Task / 跨流程触发同一业务逻辑时强制 |

> **不复用 / 新建能力时的举证要求：** 必须说明对可维护性（后续修改要改几处？）、健壮性（团队已有实现的坑是否重蹈？）的影响。"我自己写一个更简单"不是有效理由。

---

> **执行声明：本 SKILL 是强制执行的工作流，不是参考指南。AI 必须严格按本 SKILL 规定的顺序和标准执行每个步骤，不得跳过任何步骤，不得自行决定绕过任何门禁。人工审核节点必须等待用户确认后才能继续，禁止自动决策。**

**门禁定义：门禁（Gate）是强制验证点，每一步完成后必须通过门禁检查才能进入下一步。包括但不限于：读取文件确认、用户确认、编译通过、服务启动成功、测试 Pass。跳过任一门禁属于违规行为。**

## 目标

从 DR 设计文档出发，自动驱动整个研发流程直到代码通过所有测试。串联所有子 SKILL，形成完整的自动化闭环。

---

## 🎯 统一入口与智能路由（🆕 2026-06-06 增强 — AE 体系的"AI 智能调度层"）

> **🔴 核心立场：** 本 SKILL 是 AE 体系的**统一入口 + 智能路由**，**所有用户输入都先经过本 SKILL 分析**再路由到对应节点 SKILL 执行。
>
> **🔴 关键能力：**
> 1. **分析用户输入**属于哪个流程节点（Phase 1 ①/②/③、Phase 2 ④/⑤、Phase 3 ⑦、重入、Proposal、其他）
> 2. **路由到对应 SKILL**（见 §智能路由表）
> 3. **支持流程节点内子任务并行**（同节点内子任务可拆给多 Agent 并行，详见 [`agent-orchestration-skill.md`](../cross-cutting/agent-orchestration-skill.md)）
> 4. **支持重入流程**（state.json 读 + 续接）

### 智能路由表（6 大节点 + 4 重入场景 + 🆕 4 类需求智能路由）

#### 基础路由表（流程节点）

| 用户输入关键词 / 场景 | 路由到 SKILL | 节点 | 🆕 G-RA 门禁（v3.2） |
|---------------------|------------|------|---------------------|
| "分析需求" / "从 PRD 开始" / "需求拆解" / "需求分析" | **`requirement-analysis-skill.md`** | **Phase 1 入口 🆕** | 🟢 不需要（入口本身）|
| "生成 DR" / "写 DR" / "从 RA 生成 DR" / "DR 起草" | **`dr-generate-skill.md`** | **Phase 1 ① 🆕（规模=大时）** | 🔴 **必过 G-RA** |
| "DR 评审" / "DR Review" / "检查 DR" | **`dr-review-skill.md`** | **Phase 1 ② 🆕** | 🔴 **必过 G-RA**（dr-review 要看 RA）|
| "从 DR 开始" / "生成 Story" / "写 Story" / "Story 起草" | `story-generate-skill.md` | Phase 1 ① | 🔴 **必过 G-RA** |
| "Story 评审" / "审 Story" / "Story Review" | `story-review-skill.md` | Phase 1 ② | 🔴 **必过 G-RA** |
| "生成测试用例" / "补测试用例" | `testcase-generate-skill.md` | Phase 1 ③ | 🔴 **必过 G-RA** |
| "生成 Task" / "写 Task 文档" | `task-generate-skill.md` | Phase 2 ④ | 🔴 **必过 G-RA**（规模≥小）|
| "开始 Coding" / "写代码" / "实现 Story" | `coding-skill.md` | Phase 2 ⑤ | 🟢 仅当规模≥中时需要 RA |
| "出 Coding 报告" / "Coding 完成" | `coding-report-skill.md` | Phase 2 ⑤ | 🟢 不需要（事后总结）|
| "Code Review 报告" / "出 CR 报告" / "评审代码" | `code-review-skill.md` | Phase 3 ⑦ | 🟢 不需要（评审对象是代码）|
| "从 X 继续" / "重入 Y 流程" / "续接" | **🔴 先读 state.json** 判定重入点，再路由到对应 SKILL | 任意 | 🟡 看 state.json currentStep（已完成的步骤豁免）|
| "修一下 XX" / "发现 XX 问题" / "生产故障" / "客户反馈" | `proposal-skill.md` | 任意渠道 | 🟢 不需要（渠道入口）|
| "代码写错了" / "编译失败" / "测试失败" | `coding-skill.md §异常路径` → 触发 `proposal-skill.md` | 异常渠道 3 | 🟢 不需要（BUG 类豁免）|
| "审计项目资产" / "双源一致性" / "每月审计" | `project-assets-update-skill.md §5 审计` | 横向 | 🟢 不需要（资产本身）|
| "修改/补 Story" / "Story Update" | `story-update-skill.md` | 任意（携带 Proposal）| 🟡 看修改类型（RA 维度变更 → 需重审）|
| "修改/补 Task" / "Task Update" | `task-generate-skill.md §5bis 全局 Task Review` | 任意（携带 Proposal）| 🟡 看修改类型 |
| "修改/补 Coding" / "改代码" | `coding-skill.md` + 携带 `proposal-skill.md` 输出的 Proposal | 任意（携带 Proposal）| 🟢 不需要（BUG 类豁免）|
| "放文档哪里" / "命名" / "重入新建还是修改" | `document-storage-skill.md`（横切依赖）| 任意 | 🔴 **必过 G-DOC-STORAGE**（文档落地前强制，🆕 v3.4.0）|
| "修改 SKILL" / "更新 SKILL" / "新增 SKILL" / "重构 SKILL" / "SKILL 边界" / "SKILL 维护" / "优化 ae-sdd" / "改 ae-sdd" | **`ae-sdd-update-skill.md`**（自身维护） | 横向（自治） | 🟢 不需要（自治）|
| 🆕 **"安装 ae-sdd" / "装 ae-sdd" / "重装 ae-sdd" / "升级 ae-sdd" / "卸载 ae-sdd" / "给 <项目> 接 ae-sdd"** | **`ae-sdd-install-skill.md`** | **横向（安装引导 🆕）** | 🟢 不需要（安装引导）|

#### 🆕 4 类需求智能路由（2026-06-10 任务规模分级）

> **核心思路：** 把需求分 4 类，不同规模走不同 SKILL 组合。**所有规模 100% 走 CodingModel 11 维决策 + 14 条 CodingPlan 门禁 + TR-1~TR-7**，只是流程深度和文档数量不同。

| 需求类型 | 触发词示例 | 判定方法 | 路由到 | 事务命名 |
|---------|----------|---------|--------|---------|
| **1. 已有 Story** | "审 STORY-001" / "STORY-001 重入" | `state.json` currentStep 判定 | `story-review-skill.md`（按当前步骤）| `{STORY-ID}` |
| **2. 中大任务（重）** | "做个用户管理功能" / "实现融云回调" / "做一个新功能" | **套 Story 7 区模板能套满 4+ 区** | `story-generate-skill.md` → `story-review-skill.md` → ... | `{STORY-ID}` |
| **3. 小任务（轻）** | "加个缓存预热" / "加个重试机制" / "加个 XX" | 套 Story 7 区只能套 2-3 区 | `task-generate-skill.md`（**跳 Story**）| `Task-{服务缩写}-{任务简述}` |
| **4. 微任务** | "改个枚举值" / "改个常量" / "重命名个字段" / "做个微调" | 套 Story 7 区套不出（0-1 区）| `coding-skill.md` 直接调 `CodingSkill.Plan` 出 CodingPlan（**跳 Story + 跳 Task**）| `Plan-{服务缩写}-{任务简述}` |
| 🆕 **5. 文档类任务（2026-06-27）** | "出 proposal / 出建议书" / "出分析报告" / "出调研文档" / "修订建议" / "改造方案" | **触发 RA skill §反模式 8/9** | `requirement-analysis-skill.md` 完整 7 步（必跑最小流程）| `RA-{主题}` |

> **🆕 2026-06-27 文档类任务补充（来自 RA skill §反模式 8/9）：** "出 proposal / 出建议书 / 出分析报告 / 出调研文档" 类任务**必须先走 requirement-analysis-skill.md 完整 7 步**——不允许直接动笔出文档。这是堵"AI Agent 撞到'出文档类任务'就跳过 RA"的系统性漏洞（实测案例：2026-06-27 历史 6+ 份 ae-sdd 修订建议书均直接出文档未走 RA，详见 `D:\al-agent-workspace\ae-sdd-update-doc\history\2026-06-27-RA多轮挖掘流程未执行-自我修订建议书.md`）。


**事务简称命名规则（2026-06-10 用户确认）：**
- 格式：`{服务名缩写}-{任务简述}`
- 服务名缩写：去掉 `icec-cloud-` 前缀和 `-service`/`-bff` 后缀，保留核心
  - `icec-cloud-life-cs-service` → `cssv`
  - `icec-cloud-boss-user-service` → `usv`
  - `icec-cloud-boss-user-bff` → `ubff`
- 任务简述：业务名 / 功能名（2-3 个单词，尽量简短保留语义）
- 完整例子：`cssv-rongcloud-callback`、`usv-cache-preheat`、`ubff-user-export`

#### 🆕 4 维判定智能路由（2026-06-17 增强 — 与 4 类需求并存）

> **核心立场（用户原话："保留原有能力的条件下新增能力"）：** 在 4 类需求（传统路径）之上**叠加** 4 维判定（增强入口）。**新旧并存，不替换**：
> - **4 维判定优先尝试**——拿到 PRD/Issue/对话需求/BUG 等多源输入时优先走 4 维判定
> - **4 维判据不全时 fallback**——来源或规模未知时 → 套用旧 4 类需求的 Story 7 区模板判定
> - **完全保留**旧 4 类需求判定逻辑（标记为"传统路径"），绝不废弃

**4 维判定（增强入口）：**

```
4 维判定（增强入口，优先尝试）：
├─ 维度 1：来源
│   ├─ PRD → requirement-analysis-skill
│   ├─ Issue → requirement-analysis-skill（轻量）
│   ├─ 对话需求 → requirement-analysis-skill（多轮对话）
│   ├─ BUG/配置类 → coding-skill
│   └─ 无输入 → 引导用户
│
├─ 维度 2：规模（来自 requirement-analysis 输出）
│   ├─ 大 → dr-generate-skill
│   ├─ 中 → story-generate-skill
│   ├─ 小 → task-generate-skill
│   ├─ 微 → coding-skill
│   └─ 特殊(BUG/非代码) → coding-skill
│
├─ 维度 3：现有产物
│   ├─ 无 → requirement-analysis-skill
│   ├─ 有 RA + 无 DR → dr-generate-skill
│   ├─ 有 DR 草稿 → dr-review-skill
│   ├─ 有 DR + 无 Story → story-generate-skill
│   └─ 有 Story → story-review-skill
│
└─ 维度 4：项目类型
    ├─ 重任务 → `ae-sdd-doc/iterations/{date}/{DocType}/{STORY-ID}/`（由 documentStorage.resolve_path() 定位）
    ├─ 小任务 → `ae-sdd-doc/iterations/{date}/Task/{事务简称}/`
    └─ 微任务 → `ae-sdd-doc/iterations/{date}/Coding/{事务简称}/`
```

**融合策略：**
- 入口：先尝试 4 维判定（来源 + 规模 + 现有产物 + 项目类型）
- 4 维判据不完整时（未知来源或规模），fallback 到旧的 4 类需求
- 旧的 4 类需求判定逻辑完全保留（标记为"传统路径"）
- 新旧并存，路由表合并展示

#### 🆕 4 维判定 vs 4 类需求对照（2026-06-17）

> **目的：** 让用户清晰理解新旧关系——4 维判定是"增强入口"，4 类需求是"传统 fallback"，**不是替代关系**。

| 维度 | 4 类需求（传统） | 4 维判定（增强） |
|------|----------------|---------------|
| 判定依据 | 套 Story 7 区模板 | 来源 × 规模 × 现有产物 × 项目类型 |
| 起点 | 已有 Story / 手动任务 | 任何输入（PRD/Issue/对话/BUG） |
| 规模裁定 | 隐含（套模板） | 显式（5 维评分） |
| 路由目标 | 4 个固定 | 动态 6 类（requirement-analysis / dr-generate / dr-review / story-generate / task-generate / coding） |
| 适用范围 | 中大/小/微任务 | 任意规模 + 任意来源 |
| 关系 | fallback | 优先入口 |
| 入口 SKILL | 直接套模板 | 必走 `requirement-analysis-skill` 先做来源识别 + 规模裁定 |

**判定算法（路由决策算法第 2.2 步）：**

```
2.2 【🆕 任务规模判定】（在 2.1 关键词匹配之后）

  显式触发词识别：
  ├─ 出现 "Story-XX" / "STORY-ID" / "审 Story" → 类型 1（重入到 Story Review）
  ├─ 出现 "出 Story" / "做一个新功能" / "实现 XX" → 类型 2
  ├─ 出现 "出个 Task" / "加个 XX" / "做个 XX" → 类型 3
  ├─ 出现 "改个 XX" / "修个 XX" / "重命名 XX" → 类型 4
  └─ 无显式触发词 → 【自动判定】套 Story 7 区模板
      ├─ 套满 4+ 区 → 类型 2（中大任务）
      ├─ 套满 2-3 区 → 类型 3（小任务）
      └─ 套不出或只套 1 区 → 类型 4（微任务）

  套模板判定步骤（自动判定时执行）：
  1. 列出任务涉及的 7 个区：① 业务背景 ② 主流程 ③ AC ④ 接口契约 ⑤ 数据模型 ⑥ 实现任务映射 ⑦ ①bis 前端契约
  2. 对每个区，看任务描述能否给出实质性内容（不只是"无"）
  3. 统计能填满的区数 → 套模板判定
```

**路径差异（由 documentStorage.resolve_path() 自动定位）：**

- **类型 1-2（重任务）：** 文档存到 `ae-sdd-doc/iterations/{date}/{DocType}/{STORY-ID}/`（如 `ae-sdd-doc/iterations/2026-06-17/Story/STORY-001-BE/`）
- **类型 3（小任务）：** 文档存到 `ae-sdd-doc/iterations/{date}/Task/{事务简称}/`（由 `documentStorage.resolve_path(intent="TASK_SMALL", ...)` 定位）
- **类型 4（微任务）：** 文档存到 `ae-sdd-doc/iterations/{date}/Coding/{事务简称}/`（由 `documentStorage.resolve_path(intent="PLAN_MICRO", ...)` 定位）

**完整路径模板见 `document-storage-skill.md §2.9`。**

### 路由决策算法（🆕 2026-06-17 扩展为 7 步：保留原 5 步 + 新增 1.6 来源识别 + 1.7 规模识别）

```
0. 【🆕 工作区与项目资产检查】（每次 SKILL 启动时执行，任何后续流程的前置）
   ↓
   0.1 判断是否有明确工作区（projectKey / gitPath 已知？）
       ├─ 未知 → 询问用户"请告知工程目录或项目名（projectKey）"
       └─ 已知 → 进入 0.2
   ↓
   0.2 调用 document-storage-skill.get_assets(projectKey) 检查项目资产
       ├─ 资产存在 → 静默加载，进入步骤 1
       └─ 资产不存在 → 进入 0.3
   ↓
   0.3 【资产缺失：明确告知用户并生成资产】
       AI 输出：
       "⚠️ 未找到项目 {projectKey} 的资产文件（{assetsPath}）。
        项目资产包含微服务清单、分层规则、命名约定、工程约束等，
        是后续所有流程的上下文基础。
        正在为您生成项目资产，这需要扫描工程目录……"
       ↓
       调用 project-assets-update-skill.md §3（生成动作）
       → 9 步探查 SOP（读 CLAUDE.md + AGENTS.md + 扫描工程 + 抽典型类）
       → 输出：.ae-sdd/assets/{workspaceKey}/{workspaceKey}.assets.md（路径模板见 document-storage §2.3）
       ↓
       生成完成后 AI 告知用户：
       "✅ 项目资产已生成：{assetsPath}
        包含：{microservices 数量} 个微服务 / {分层} 层架构 / {技术栈}
        请确认资产内容是否准确，确认后继续流程。"
       ↓
       用户确认 → 进入步骤 1
       用户发现问题 → 先走 project-assets-update-skill.md §4（更新动作）
   ↓
1. 接收用户输入
   ↓
1.5 【🆕 自更新识别】（优先级高于步骤 2，命中即短路）
   ├─ 用户输入涉及 AE SKILL 自身的新增/修改/重构/边界维护/优化
   │   判定关键词："修改 SKILL" / "更新 SKILL" / "新增 SKILL" / "重构 SKILL"
   │              / "SKILL 边界" / "SKILL 维护" / "优化 ae-sdd" / "改 ae-sdd"
   │              / "ae-sdd skill" + 任意变更动词
   ├─ 命中 → 路由到 `ae-sdd-update-skill.md`（自身维护工作流）
   │         跳过步骤 2-5，直接按 update-skill 的 5 步流程执行
   └─ 未命中 → 进入步骤 1.6
   ↓
1.6 【🆕 来源识别】（2026-06-17 新增 — 4 维判定维度 1；2026-06-27 扩展为 9 类输入）
   ├─ 识别输入类型：PRD / Issue / 对话需求 / BUG / 配置类 / 无输入
   │   ├─ PRD / Issue / 对话需求 → 路由到 `requirement-analysis-skill.md`
   │   │     （由 RA SKILL 内部完成规模裁定 → 进一步路由到 dr-generate / story-generate / task-generate / coding）
   │   ├─ 🆕 2026-06-27 修订建议书 / proposal / 分析报告 / 调研文档 → 路由到 `requirement-analysis-skill.md`
   │   │     （文档类任务同样必走 RA skill 完整 7 步，详见 RA skill §反模式 8/9）
   │   │     触发词：出 proposal / 出建议书 / 出分析报告 / 出调研文档 / 修订建议 / 改造方案 / 重构方案 / 优化方案
   │   ├─ BUG / 配置类 → 路由到 `coding-skill.md`（直接走代码，intent=BUG/CONFIG 双重豁免 RA）
   │   └─ 无输入 → 引导用户提供需求
   └─ 完成来源识别后进入步骤 1.7
   ↓
1.7 【🆕 规模识别】（2026-06-17 新增 — 4 维判定维度 2）
   ├─ 已有规模结果（来自 requirement-analysis 5 维评分）→ 用规模结果
   │   ├─ 大 → dr-generate-skill
   │   ├─ 中 → story-generate-skill
   │   ├─ 小 → task-generate-skill
   │   ├─ 微 → coding-skill
   │   └─ 特殊（BUG/非代码）→ coding-skill
   └─ 无规模结果（fallback 到旧 4 类需求）→ 套 Story 7 区模板判定
       ├─ 套满 4+ 区 → 类型 2（中大任务，story-generate-skill）
       ├─ 套满 2-3 区 → 类型 3（小任务，task-generate-skill）
       └─ 套不出或只套 1 区 → 类型 4（微任务，coding-skill）
   ↓
1.8 【🆕 RA 准入门禁】（🆕 v3.2 加固 — 2026-06-24）
   ├─ 触发场景：步骤 1.7 路由到 dr-generate-skill / story-generate-skill / task-generate-skill 时
   ├─ 检查项（缺一项即阻断）：
   │   ├─ RA 文档存在（ae-sdd-doc/iterations/{date}/RA/{RA-ID}-vN.m.md）
   │   ├─ 8 个核心维度齐全（角色/场景/流程/数据/规则/设计方向/AC/假设）
   │   ├─ RequirementAnalysisModel 12 维决策完整（RA-01~RA-12 均有结论/证据/风险/动作）
   │   ├─ 需求风险预判已闭环（命中风险均落到对应 RA 章节）
   │   ├─ RA-G01~RA-G16 全部通过
   │   ├─ 5 问自检阻断项 = 0（不得用 90% 逃避阻断项）
   │   ├─ 所有 🔴 阻断型缺口已解决
   │   └─ RA 距今 ≤ 30 天（超期 → 🟡 警告但不阻断）
   ├─ 调用：`ae-sdd gate ra-required --project <projectKey> --story <STORY-ID>`
   ├─ 通过 → 进入步骤 2（加载下游 SKILL）
   ├─ 🔴 不通过：
   │   ├─ 阻断原路由，输出 ⚠️ 提示："RA 文档缺失/不完整，无法启动 dr-generate/story-generate/task-generate"
   │   ├─ 自动路由建议：`requirement-analysis-skill.md`（首次生成 RA）或 `requirement-analysis-skill.md §RA 修订`（RA 已存在但需补维度）
   │   ├─ 用户显式确认豁免（如"我就要跳过 RA"）→ 标记本次任务为"事后回溯"+ 继续
   │   └─ 否则按自动路由建议走 RA → 完成后回到本路由
   ├─ 豁免场景（不触发 G-RA）：
   │   ├─ 类型 4（微任务）→ coding-skill 直接编码
   │   ├─ BUG/配置类 → coding-skill BUG 路径
   │   └─ 重入到 state.json 已记录的完成步骤（如 step-4-coding-r2 不需要 RA）
   ↓
2. 关键词匹配（智能路由表 §1）
   ├─ 命中 6 大节点之一 → 路由到对应 SKILL
   ├─ 命中重入场景 → 读 state.json → 判定重入点 → 路由
   ├─ 命中问题场景 → 路由到 proposal-skill.md（带渠道标识）
   ├─ 命中其他场景 → 路由到对应 SKILL
   └─ 多个命中 → 询问用户优先级
   ↓
2.5 【🆕 v3.5.0 🔌 SKILL 注册表加载】(接入点 = UserPromptSubmit hook)
   ├─ 加载目标 SKILL = S（如 coding-skill.md，来自 next_step_suggestion 的 skill 字段）
   ├─ 接入点：tools/lib/prompt_inject.py 的 inject() → _resolve_skill_path(S, ade_sdd, master)
   │   └─ 内部调用 tools/lib/plugin_loader.py 的 resolve_skill()：
   │       ├─ 收集三层注册表（L1 项目层 / L2 全局层 / L3 仓库根层）
   │       ├─ 按 L1 > L2 > L3 > L0 内置 fallback 优先级合成
   │       ├─ 命中某层 → hook 注入 "plugin: 外挂名 @ 命中层 → 外挂路径"
   │       └─ 三层都未命中 → 注入原 skill 裸文件名（行为同 v3.4.x）
   ├─ 多层冲突时按优先级选胜者 + 🟡 警告（不阻断）
   ├─ 详细 SOP 见 [`ae-sdd-plugin-loader-skill.md`](../skills/cross-cutting/ae-sdd-plugin-loader-skill.md)
   └─ 整步对上层透明：路由算法不感知插件存在，仅是"加载路径"被替换
   ↓
3. 加载对应 SKILL（从 .claude/skills 加载，或 Read 文件）
   ↓
4. 触发对应 SKILL 的"§整体流程"第零步（准入检查）
   ↓
5. 用户确认 → SKILL 执行
```

**步骤 0 的 3 条硬规则：**
- 🔴 **不允许跳过 0.2**：即使用户说"直接开始"，也必须先确认项目资产存在
- 🔴 **0.3 生成过程必须明确告知用户**：不能静默生成，用户必须知道发生了什么
- 🟡 **资产存在时静默加载**：不打扰用户，直接进入步骤 1

### 重入流程判定（state.json）

**state.json 位置：** `.auto-engineering/{STORY-ID}/state.json`（详见 `document-storage-skill.md §2.5`）

**state.json 关键字段：**
```json
{
  "storyId": "STORY-001-BE",
  "currentPhase": "Phase 2",
  "currentStep": "step-4-coding-r2",
  "completedSteps": ["step-1-dr2story", "step-2-story-review", "step-3-testcase", "step-4-coding-r1"],
  "codingRound": 2
}
```

**重入判定算法：**
```
用户输入"从 X 继续" / "重入 Y"
    ↓
读 state.json
    ↓
解析 currentPhase + currentStep
    ↓
根据"上次停在哪个步骤"判定重入点
    ↓
路由到该步骤的对应 SKILL
    ↓
加载 SKILL 并跳过已完成步骤（从 completedSteps 中过滤）
```

### 路由示例

**示例 1：用户说"从 STORY-001 继续"**
- AE-skill 读 `.auto-engineering/STORY-001-BE/state.json` → currentPhase: "Phase 2", currentStep: "step-4-coding-r2"
- 判定：重入到 Phase 2 ④ Coding 第 2 轮
- 路由：`coding-skill.md` + 携带 completedSteps = [step-1/2/3, step-4-r1]（跳过已完成）

**示例 2：用户说"修一下 roleId=0 的特殊语义"**
- AE-skill 关键词匹配 → "修一下" → 路由到 `proposal-skill.md`，渠道标识 = 5（用户反馈）
- AE-skill 加载 `proposal-skill.md` → 引导用户填写 4 段 → 生成 Proposal 文档
- 之后用户说"按 Proposal 走流程" → AE-skill 读 Proposal §4 涉及范围 → 触发 5 步流程

**示例 3：用户说"出 Coding 报告"**
- AE-skill 关键词匹配 → "出 Coding 报告" → 路由到 `coding-report-skill.md`
- AE-skill 加载 `coding-report-skill.md` → 引导用户填 9 章节 → 生成 Coding 报告

### 路由与 SKILL 编排的关系

| 层级 | SKILL | 职责 |
|------|-------|------|
| **第 1 层：统一入口 + 智能路由** | `SKILL.md`（本 SKILL）| 分析用户输入，路由到对应节点 SKILL |
| **第 2 层：流程编排** | `SKILL.md §整体流程` | 9 步流程怎么走、门禁是什么 |
| **第 3 层：节点 SKILL** | `story-generate / story-review / coding / code-review` 等 | 每个节点怎么执行 |
| **第 4 层：横切依赖** | `proposal-skill / document-storage-skill / agent-orchestration-skill` | 跨节点的统一标准 |

**🔴 关键：** AE-skill 本章节是"统一入口"层，**所有用户输入都先经过这里**再路由。其他 SKILL 仍可被直接调用（不强制走 AE-skill），但推荐走 AE-skill 入口。

---

## 📖 人工审核主动讲解规范（编排级门禁 — 详细模板在各子 SKILL）

> **设计哲学：** 人工审核不是"丢文档给用户自己看"，而是 AI 主动把内容"讲"给用户听，**并且直接在对话中展示**。本节规定**双支柱强制门禁**：① 讲解（叙述性，用故事讲清背景/意图）+ ② 对话内直接呈现（结构化输出，让用户无需打开任何文档即可完成审核）。

> 🔴 **双支柱缺一不可：** 只讲故事但不输出内容 = 用户无法核对细节。只输出内容但不讲故事 = 用户不知道为什么。两者都必须做。

### 强制门禁（AE 编排层关注）

| 审核节点 | 讲解主体 | 子 SKILL 模板位置 | 触发门禁 |
|---------|---------|------------------|---------|
| ① Story Review | 设计决策 | [`story-review-skill.md` §📖 Story 讲解模板](../phase1-design/story-review-skill.md) | 🔍 人工审核点 1 前必须完成 |
| ② Task Generate/Review | 实现拆解 | [`task-generate-skill.md` §📖 Task 讲解模板](../phase2-task/task-generate-skill.md) | 🔍 人工审核点 2 前必须完成 |
| ③ Code Review | 代码实现 | [`coding-skill.md` §📖 Code 讲解模板](../phase2-coding/coding-skill.md) | 🔍 人工审核点 4 前必须完成 |

**反模式（AE 编排层必须拦截的）：**
- ❌ "Story 文档已生成，请审核"（让用户自己读）
- ❌ "请确认进入 Phase 2"（不解释为什么）
- ❌ "Task 文档生成完毕，请确认"（不解释设计意图）
- ❌ "CodeReview 报告已出具，请审阅"（不主动讲重点）
- ❌ 一次抛出一大坨文档后等用户"整体确认"（用户根本没耐心看）
- ❌ 讲完故事但只给文档路径，让用户去打开文件核对（**讲故事 ≠ 展示内容**）
- ❌ 只输出审核清单（自检用）但不把关键内容呈现在对话中

**正确做法：**
- ✅ 进入审核节点前，AI 先"讲故事"（叙述性）——见各节点 `📖 AI 主动讲解`
- ✅ 讲完后，AI 把关键内容**直接结构化输出在对话中**——见各节点 `📋 对话内直接呈现`
- ✅ 用户可要求"展开讲某个点" → AI 必须现场补充讲解
- ✅ 用户可要求"快速过" → AI **必须先问**"是否接受降低讲解详细度"，得到明确同意后才能简化（**不得默认简化**）
- ✅ 讲解后用户回复模糊（如"好"/"行"/"可以"） → AI 必须按 ⚠️ 处理，**逐项追问确认**，不得当作 ✅ 通过
- ✅ 讲解 ≠ 一次讲完就结束。每个审核节点都可能多轮往复讲解（用户追问 → AI 补充 → 再确认）

**🔴 对话内直接呈现的通用标准：**

| 项目 | 要求 |
|------|------|
| 格式 | 表格 / 有序列表 / 代码块——让用户一眼能扫到关键信息 |
| 完整性 | 不省略、不摘要、不用"..."替代——用户看到的就是完整的 |
| 可操作性 | 每条内容旁边有"请确认/有疑问/需修改"的指引 |
| 行动选项 | 每次输出完必须跟 ✅/⚠️/⏸️ 选项，不允许只展示不询问 |

**门禁：**
- 🔴 任一审核节点未做主动讲解 → 视为跳过人工审核 → 禁止进入下一阶段
- 🔴 任一审核节点讲解后未做"对话内直接呈现" → 与跳过审核等价 → 禁止进入下一阶段
- 🔴 AI 自行简化讲解 → 视为审核造假，按 [[feedback_report-code-reconciliation]] 整改

---

## 🤖 多 Agent 任务分配机制（单兵作战，多 Agent 并行）

> **场景：一个人负责整个项目，单 Agent 串行做所有事效率太低。** 本节定义如何在 auto-engineering 流程中**把任务拆分给多个子 Agent 并行执行**，让一个人也能驱动一整条生产线。
>
> **核心理念：一个人 = 一支队伍。**
> - **root agent（你正在对话的 AI）** = 项目经理 / 调度员 / 决策者
> - **sub-agent（被派活的子 AI）** = 专项工程师 / 审阅者 / 验证者
> - root agent 负责**拆活、派活、汇总、决策**；sub-agent 负责**按交付标准完成专项任务**
> - 拆分原则：**3+ 独立轨道 / 需独立验证 / 跨多源多工具 / 高错误代价** → 拆；否则单 Agent 串行做

### 🆕 v3.5.5 主会话职责边界（默认派活，不再单 Agent 直做）

> **🔴 背景：** v3.5.4 及之前的默认是"单 Agent 串行做所有事"，主会话被迫读 25 个子 SKILL、读源码、写文档、做 walkthrough、跑测试 → 上下文爆炸。**v3.5.5 起主会话职责收口**：默认派 1 个 sub-agent 执行具体节点，主会话只承担编排层职责。

| 类别 | 主会话负责 | 派给 sub-agent |
|---|---|---|
| **SKILL 文档** | 仅读 SKILL.md 编排层内容（哪些节点、哪些门禁）| 读子 SKILL 模板（story-review / task-generate / coding-skill 等）按模板执行 |
| **源码** | 不读源码 | 读源码做分层 walkthrough |
| **文档产出** | 不写流程文档 | 写 Story / Task / CodingPlan / TestCase / CodeReview 报告 |
| **讲解** | ✅ 主笔（审核点 1/1.5/2/2.5/4/5 的 5-7 维度故事由主会话在对话中产出）| 准备讲解素材（汇总 sub-agent 报告，喂给主会话）|
| **CLI 调用** | ✅ `ae-sdd state/gates/iteration-check/update-check/context-pressure` | 跑 `mvn test` / 解析 Surefire XML / `scripts/test_authenticity_scan.py` |
| **状态落盘** | ✅ 写 `state.json` / `session.json`（编排层动作）| 不直接写 |
| **用户对话** | ✅ 输入分析、✅/⚠️/⏸️ 收口、模糊回复追问 | 不直接对话用户 |

**例外（保留不派活的场景）：**
- 🔹 **微任务（类型 4）**：单文件/单枚举值改动，主会话直做（4 类需求 fallback 保留）
- 🔹 **BUG/配置类**：coding-skill BUG 路径直做
- 🔹 **用户明确豁免**：用户说"主会话直做 / 不要派活" → 尊重用户
- 🔹 **⑥.10 test-verifier**：v3.4.0 已强制派 sub-agent 独立验证（即使其他节点主会话直做）

**节点级派活清单（详见 `agent-orchestration-skill.md §8.6`）：**
- 审核点 1 → `story-writer` + `testcase-writer`
- 审核点 1.5 → `task-writer`（起草实现方案）
- 审核点 2 → `task-writer`（写 Task 文档）
- 审核点 2.5 → `task-writer`（汇总统一版 CodingPlan）
- 审核点 4 → `coder` + `code-reviewer`
- 审核点 5 → `summary-writer`（写 PRD summary.md）

> **为什么这是好事：** 把"读源码 / 写文档 / 跑测试"这些**可被独立执行的机械劳动**从主上下文剥离，主会话专注于"用户对话 + 编排 + 汇总 + 讲解"，上下文压力大幅下降。每个审核点边界通过 `ae-sdd context-pressure`（§⏱️ 节点级上下文压力软提示）软提示剩余容量。

### 何时启用多 Agent

| 触发条件 | 典型场景 | 建议拆分粒度 |
|---------|---------|------------|
| **多个 Story 并行** | 用户有 3+ 个独立 Story 要做 | Story 级别（每个 sub-agent 负责一个 Story 端到端） |
| **单 Story 多 Phase 并行** | 设计/实现/验证可解耦（如 Story 已稳定，只需补全测试用例和 CodeReview） | Phase 级别 |
| **单 Phase 多 Task 并行** | Task 之间无强依赖（如 Task-3 SPI 定义和 Task-5 Controller 实现可并行） | Task 级别 |
| **需独立验证** | 关键决策点（状态机设计、事务边界、错误码）需要"第二意见" | 验证 sub-agent 独立审阅 |
| 🆕 **Review 节点默认多 reviewer**（2026-06-25）| DR/Story/Task/Code Review 节点 Tier 2+ **默认**启用多 reviewer 交叉审（RA Review 无独立节点，不触发） | 按 [`agent-orchestration-skill.md §8.4`](../cross-cutting/agent-orchestration-skill.md) Tier 判定选 1/2/3 个 reviewer，对抗 AI 逻辑自洽陷阱 |
| **跨多源/多工具** | 需要同时跑 DB 测试、HTTP 测试、前端组件测试 | 测试 sub-agent 各自负责一层 |
| **高错误代价** | 涉及资金/数据/权限/线上行为的接口 | 实现 sub-agent + 验证 sub-agent 双跑 |

> **不启用的场景：** 1-2 个 Story、单 Phase 内 Task 强依赖、需要大量上下文连续推理的工作（避免拆分后 sub-agent 拿不到完整上下文反生错）。

### 启动方式

**方式 1：用户主动启用**
> 用户说："启用多 Agent 模式" / "用子 Agent 并行做这几个 Story" / "派个 reviewer 审一下这个设计"

**方式 2：AI 自动提议**
> root agent 检测到当前任务符合"何时启用"表中的任一条件时，**主动向用户提议**：
> ```
> 🤖 【多 Agent 拆分提议】
>
> 检测到当前任务符合多 Agent 启用条件：
> - 触发条件：{具体触发条件}
> - 建议拆分：{具体拆分方案}
> - 预计提速：X 倍（粗略估算）
>
> 是否启用多 Agent 模式？✅ 启用 / ❌ 不启用（继续单 Agent 串行）
> ```

**方式 3：直接调用底层 skill**
> 通过 `mavis-team` skill 创建团队计划（适合复杂多轨道场景），或 `mavis communication send --command spawn` 派单点子任务（适合验证/审阅场景）。

### 子 Agent 角色库（auto-engineering 专用）

> root agent 在派活时，从以下角色库中选择或组合。每个角色都对应一个**专项 prompt 模板**，sub-agent 启动后按模板执行。

#### 角色 1：Story 生成 Agent（`story-writer`）

| 项 | 内容 |
|----|------|
| 输入 | DR 路径 + Story ID + Story 模板（`templates/design/story-template.md`） |
| 输出 | 完整的 Story 主文档（含前端接口契约章节） |
| 标准 | 必须覆盖 ①bis 6 维度 + 模板所有必填章节 |
| 报告格式 | `{STORY-ID}-Story-WriterReport.md`：列出生成的章节 + 关键决策 + 待用户确认点 |
| 适用阶段 | Phase 1 ① |

#### 角色 2：Story Review Agent（`story-reviewer`）

| 项 | 内容 |
|----|------|
| 输入 | Story 文档 + DR 文档 + 约束 + 前端契约要求 |
| 输出 | 缺陷清单（已分类）+ 修复建议 + StoryReviewUpdatePlan 草案（由 root agent 汇总定稿） |
| 标准 | 跑完 Story Review SKILL 完整的 A-E 阶段 + F-Stage 前端契约 Review |
| 报告格式 | `{STORY-ID}-StoryReviewReport.md`：缺陷 ID / 严重度 / 位置 / 修复建议 |
| 适用阶段 | Phase 1 ② |
| 数量建议 | 按 [`agent-orchestration-skill.md §8.4`](../cross-cutting/agent-orchestration-skill.md) Tier 判定选 1/2/3 个（Tier 2 = 设计实现 + 前端契约 双审；Tier 3 + 数据模型三审）|

#### 角色 3：测试用例生成 Agent（`testcase-writer`）

| 项 | 内容 |
|----|------|
| 输入 | Story 文档 + 测试策略模板 + 约束 |
| 输出 | 测试用例文档（含 AC 映射） |
| 标准 | 覆盖 Story 所有 AC + 合规性校验通过 |
| 报告格式 | `{STORY-ID}-TestCase-WriterReport.md`：用例数量 / 覆盖 AC / 跳过的用例 + 原因 |
| 适用阶段 | Phase 1 ③ |

#### 角色 4：Task 生成 Agent（`task-writer`）— 含 CodePlan 汇总职责

| 项 | 内容 |
|----|------|
| 输入 | Story 文档 + 测试用例 + 项目资产 + 约束 |
| 输出 | Task 文档集（含 CodingModel 决策记录 + 任务级 CodePlan）+ Task 实现方案 + **统一版 `{STORY-ID}-CodingPlan.md`** |
| 标准 | 全局 Task Review 通过（TR-1~TR-7）+ 统一版 CodingPlan 14 条门禁全过 + 用户明确确认 |
| 报告格式 | `{STORY-ID}-Task-WriterReport.md`：Task 数量 / 依赖关系图 / 风险 Task 标记 / 统一版 CodePlan 摘要 |
| 适用阶段 | Phase 2 ④ + ④ter（调 `CodingSkill.Plan(task-level)` 生成 CodingModel 决策记录 + 任务级 CodePlan）+ ⑥（汇总统一版 CodePlan）|
| 子流程 | 1. ④ 按 Task 顺序生成文档，每个 Task 撰写时调用 **`CodingSkill.Plan(task-level)`**（不是直接引用章节号），将返回的 CodingModel 决策记录 + 任务级 CodePlan 嵌入 Task 文档<br>2. ④bis 单 Task 一致性校验（TC-1~TC-7）<br>3. ⑤ 生成 Task 0 + 全局 Task Review（TR-1~TR-7）<br>4. ⑥ 汇总所有 Task 的任务级 CodePlan 为统一版 `{STORY-ID}-CodingPlan.md`，套用 Coding-SKILL §④bis 16 节模板 + 14 条门禁<br>5. 用户审核统一版 CodePlan → 通过后触发 `CodingSkill.Execute` |

#### 角色 5：原 `plan-writer` 已合并入角色 4

> **变更（2026-06-05）：** Plan 写在 Task 内（任务级 CodePlan），不再有独立的 plan-writer 角色。统一版 CodePlan 汇总也是 task-writer 的职责（角色 4 的子流程 ④ter + ⑥）。
> 原 ④bis "CodingPlan 输出" 节点降级为 task-writer 的子动作（"⑥ 汇总统一版 CodePlan"）。

#### 角色 6：Coding Agent（`coder`）— 吃统一版 CodePlan

| 项 | 内容 |
|----|------|
| 输入 | **统一版 `{STORY-ID}-CodingPlan.md`**（用户已确认）+ Story 文档 + 项目资产 + 约束 + 工作目录 |
| 输出 | 可编译、可测试的代码 + 单元测试 |
| 标准 | 严格按 CodePlan 实施（临时偏离需用户确认）+ 每步可编译 + 测试真实性（见 `🔴 测试真实性强制规范`） |
| 报告格式 | `{STORY-ID}-Coding-CoderReport-r{M}.md`：本轮变更文件清单 + 编译/测试结果 + 已知问题 |
| 适用阶段 | Phase 2 ⑤ |
| 数量建议 | 1 个 Story = 1 个 coder（避免多 coder 写同一文件冲突） |

#### 角色 7：CodeReview Agent（`code-reviewer`）

| 项 | 内容 |
|----|------|
| 输入 | Coding 报告 + 测试报告 + Story + 实际代码 + 项目资产 |
| 输出 | CodeReview 报告（含 §2 六阶段评审 + §3 合理性判定 + §第四步 bis UpdatePlan + §第七步 7 道闸） |
| 标准 | 见 [`code-review-skill.md` §多 Agent 评审编排](../phase3-review/code-review-skill.md)（含 prompt 模板 + 多 Reviewer 交叉对比模式） |
| 报告格式 | `{STORY-ID}-CodeReview-v{N}-r{M}.md`：架构师级审阅报告 |
| 适用阶段 | Phase 3 ⑦ |
| 数量建议 | 按 [`agent-orchestration-skill.md §8.4`](../cross-cutting/agent-orchestration-skill.md) Tier 判定选 1/2/3 个（即 A/B/C 模式：Tier 2 = BE + AR 双审；Tier 3 + QA 三审）|
| **🔴 【2026-06-06 重构】** | **6 大闸门（⑥bis 一致性 / ⑦bis 对称性 / 全文档回扫 / 禁裸 ✅ / 报告-代码对账 / 产出物对账 / 真实 DB-HTTP 覆盖）已迁出到 `code-review-skill.md §第七步`，本角色仅作 AE 编排层指针，详见该 SKILL。** |

#### 角色 8：测试验证 Agent（`test-verifier` 🔴 强制）

| 项 | 内容 |
|----|------|
| 输入 | 测试报告 + 测试代码 + Story AC |
| 输出 | 测试真实性核查报告 |
| 标准 | 见 `🔴 测试真实性强制规范` 8 类禁止手段 + 5 条保障要求 |
| 报告格式 | `{STORY-ID}-TestVerification-Report.md`：8 类手段扫描结果 + 关键测试代码摘录 + AC 覆盖率 + 🆕 v3.4.0 独立 session_id |
| 适用阶段 | Phase 3 ⑥.10（⑥ 完成判定的硬前置） |
| **关键角色** | **这是 ⑥.10 强制要求的独立验证位——sub-agent 不依赖主 agent 的报告，独立跑一遍测试** |
| 🆕 v3.4.0 独立性 | 报告头部须声明 `verifier session_id`（≠ 主 agent session_id，读 `.auto-engineering/<STORY>/session.json` 对比）；G-09 校验无独立 session_id → warn（防 AI 自跑冒充 sub-agent，建议书3 B2-7）|

### 任务分配模式（典型场景）

#### 模式 A：多 Story 并行（最高频的单兵场景）

```
┌─────────────────────────────────────────────────┐
│ root agent                                      │
│                                                 │
│ 检测到：3 个 Story 待做（STORY-001/002/003）      │
│ 决策：每个 Story 派一个 sub-agent 端到端负责       │
│                                                 │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│   │ sub-A    │  │ sub-B    │  │ sub-C    │     │
│   │ STORY-001│  │ STORY-002│  │ STORY-003│     │
│   │ 端到端   │  │ 端到端   │  │ 端到端   │     │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘     │
│        │             │             │            │
│        └─────────────┼─────────────┘            │
│                      ▼                          │
│              root agent 汇总决策                 │
│              （接受 / 重试 / 升级用户）           │
└─────────────────────────────────────────────────┘
```

**使用：** 直接调用 `mavis-team` skill 创建 3 轨团队计划。

**root agent 职责：**
1. 创建 3 个 sub-agent 任务描述
2. 等待全部完成（或超时）
3. 收集 3 份 `*-WriterReport.md` / `*-ReviewerReport.md` 等
4. 跨 Story 一致性检查（共用 DR 约束、字段基线、错误码体系）
5. 接受 / 重试 / 升级用户

#### 模式 B：单 Story 多阶段并行（设计/实现/验证可解耦时）

```
┌─────────────────────────────────────────────────┐
│ root agent                                      │
│                                                 │
│ 检测到：Story 已稳定，需补全测试用例和 CodeReview  │
│ 决策：派 2 个 sub-agent 并行处理                  │
│                                                 │
│   ┌──────────────────┐  ┌──────────────────┐   │
│   │ sub-X            │  │ sub-Y            │   │
│   │ testcase-writer  │  │ code-reviewer    │   │
│   │ 生成测试用例      │  │ 出具 CodeReview  │   │
│   └────────┬─────────┘  └────────┬─────────┘   │
│            │                     │              │
│            └──────────┬──────────┘              │
│                       ▼                         │
│              root agent 汇总                    │
└─────────────────────────────────────────────────┘
```

#### 模式 C：单 Story 双/多 Reviewer 独立审阅（🆕 2026-06-25 — 按 Tier 默认启用，详见 §8.4）

> **🔴 2026-06-25 升级：** 原为"关键决策点才触发"的反应式模式，现升级为**按 Tier 默认启用**（[`agent-orchestration-skill.md §8.4`](../cross-cutting/agent-orchestration-skill.md)）。Tier 2+ Review 节点默认走双/多 reviewer 交叉审，不再等"检测到关键决策点"才触发。对抗 AI 逻辑自洽陷阱。

```
┌─────────────────────────────────────────────────┐
│ root agent                                      │
│                                                 │
│ Review 节点准入通过后判定 Tier（§8.4.1）          │
│ Tier 2/3 → 默认派 2-3 个 reviewer 独立审          │
│ Tier 1 → 单 reviewer（微/小规模 + 无关键决策）    │
│                                                 │
│   ┌──────────────────┐  ┌──────────────────┐   │
│   │ reviewer-视角A    │  │ reviewer-视角B    │   │
│   │ (视角分工见各节点  │  │ (视角分工见各节点  │   │
│   │  SKILL 多 reviewer │  │  SKILL 多 reviewer │   │
│   │  视角切分小节)     │  │  视角切分小节)     │   │
│   └────────┬─────────┘  └────────┬─────────┘   │
│            │                     │              │
│            └──────────┬──────────┘              │
│                       ▼                         │
│      root agent 跑 §8.4.3 交叉对比算法           │
│      + §8.4.4 冲突决策树                          │
│      （一致 = 接受；不一致 = 按决策树处置）       │
└─────────────────────────────────────────────────┘
```

> **各节点视角切分速查（详见 [`agent-orchestration-skill.md §8.4.2`](../cross-cutting/agent-orchestration-skill.md)）：**
> - Story Review：设计实现 + 前端契约（+ 数据模型 for Tier 3）
> - Code Review：BE 业务实现 + AR 架构规范（+ QA 测试真实性 for Tier 3，即 A/B/C 模式）
> - DR Review / Task Review：详见各节点 SKILL 的多 reviewer 视角切分小节（2026-06-25 已落地：dr-review §第一步 bis / task-generate §5bis）


#### 模式 D：测试真实性独立验证（🔴 强制，⑥.10）

```
┌─────────────────────────────────────────────────┐
│ root agent                                      │
│                                                 │
│ ⑥.10 完成判定前必须派 test-verifier sub-agent    │
│ 独立跑一遍测试，不依赖主 agent 的报告               │
│                                                 │
│   ┌────────────────────────────────────┐       │
│   │ sub-V (test-verifier)              │       │
│   │ 1. 拉取测试代码                     │       │
│   │ 2. 扫描 8 类禁止伪造手段              │       │
│   │ 3. 独立跑 mvn test                  │       │
│   │ 4. 对照 Story AC 验证覆盖率          │       │
│   │ 5. 出具测试真实性核查报告             │       │
│   └────────────┬───────────────────────┘       │
│                │                                │
│                ▼                                │
│      root agent 决策                             │
│      （报告作废 + 返工 / 接受）                   │
└─────────────────────────────────────────────────┘
```

> **🔴 这是 ⑥.10 硬门禁的兜底：即使主 agent 自称"测试通过"，test-verifier 独立验证不通过 = 不通过。** 防止"AI 给自己打钩"。

### 任务分配协议（root agent → sub-agent）

#### 任务描述模板

root agent 派活时，必须用以下结构化 prompt（不要模糊说"帮我做一下"）：

```yaml
# 任务分配卡
agent_role: {角色名}  # 如 story-writer / code-reviewer
story_id: STORY-XXX-BE
task_id: {本次任务唯一 ID}
priority: P0 / P1 / P2

input:
  - {文件路径 1}
  - {文件路径 2}
  - {约束/模板路径}

output:
  deliverable: {产出物文件路径}
  report: {报告文件路径}

standards:
  - {本任务必须满足的标准 1}
  - {本任务必须满足的标准 2}
  - {门禁/红线}

context:
  - {必要的背景信息，sub-agent 没有 root 的全上下文}

deadline: {最长执行时间}

report_back:
  channel: mavis communication
  target: {root session id}
  format: {报告模板路径}
```

#### 报告回传协议

sub-agent 完成后，必须输出**结构化报告**：

```markdown
# Sub-Agent Report - {task_id}

**Agent 角色：** {role}
**Story：** {STORY-ID}
**执行时间：** {start} ~ {end}
**结果：** ✅ 完成 / ⚠️ 完成但有风险 / 🔴 失败

## 完成情况
- [x] {交付物 1}
- [x] {交付物 2}
- [ ] {未完成项}（原因：{...}）

## 关键决策
- {决策 1}：{原因}
- {决策 2}：{原因}

## 风险点
- 风险 1：{...} → 建议：{...}

## 待 root agent 决策
- {事项 1}
- {事项 2}
```

### 状态共享（state.json 多 Agent 版本）

> 多 Agent 模式下，`.auto-engineering/{STORY-ID}/state.json` 需要扩展。

```json
{
  "storyId": "STORY-010-BE",
  "storyVersion": "v1",
  "codingRound": "r1",
  "currentPhase": "Phase 1",
  "currentStep": "step-3-testcase",
  "completedSteps": ["step-1-dr2story", "step-2-story-review"],
  "activeAgents": [
    {
      "agentId": "sub-A-001",
      "role": "story-writer",
      "sessionId": "mvs_xxx",
      "status": "running",
      "startedAt": "2026-06-03T10:00:00",
      "currentSubTask": "生成 §前端接口契约"
    },
    {
      "agentId": "sub-B-001",
      "role": "story-reviewer",
      "sessionId": "mvs_yyy",
      "status": "completed",
      "startedAt": "...",
      "completedAt": "...",
      "report": "ae-sdd-doc/iterations/{date}/CR/STORY-010-BE/STORY-010-BE-StoryReviewReport.md"
    }
  ],
  "agentReports": [
    {
      "agentId": "sub-A-001",
      "reportPath": "...",
      "summary": "..."
    }
  ],
  "pendingOutputs": {...},
  "lastUpdated": "2026-06-03T11:00:00"
}
```

**root agent 职责：**
- 启动 sub-agent 时 → 写入 `activeAgents[]`
- sub-agent 完成时 → 移入 `agentReports[]` + 更新 `activeAgents[].status`
- 任一 sub-agent 超时或失败 → 决定重试 / 升级用户

### 协调与汇总

#### 汇总流程（root agent 必须执行的步骤）

```
1. 收集所有 sub-agent 报告
   ↓
2. 跨报告一致性检查
   - 多个 Story 是否使用一致的错误码体系？
   - 多个 sub-agent 的设计决策是否冲突？
   - 测试数据来源是否与 Story 一致？
   ↓
3. 门禁扫描
   - sub-agent 是否达成任务卡上的 standards？
   - 是否有 sub-agent 标注 "未完成"？
   ↓
4. 决策
   - 全部达成 + 一致 → 接受，进入下一阶段
   - 部分未达成 → 重试该 sub-agent
   - 冲突 / 不一致 → 升级用户
   ↓
5. 更新 state.json + state.phase
```

#### 冲突处理规则

| 冲突类型 | 处置 |
|---------|------|
| sub-agent 报告间数据不一致 | root agent 必须**自己读双方产出物**判断对错，**不得默认信任何一方** |
| sub-agent 自称完成但门禁未过 | root agent 重新跑门禁，**不轻信 sub-agent 自评** |
| sub-agent 失败 / 超时 | root agent 决定：重试（同一 sub-agent）/ 换 sub-agent / 退回单 Agent / 升级用户 |
| 多 sub-agent 写同一文件 | **禁止！** root agent 派活时必须按文件拆分，**不允许并发写同一文件** |

### 门禁与规则（强制）

| # | 规则 | 违反处置 |
|---|------|---------|
| 1 | sub-agent 必须输出结构化报告，不允许"做完了"这种模糊回复 | 视为任务未完成 |
| 2 | root agent 派活必须用任务卡模板（input/output/standards/deadline 缺一不可） | 视为派活不完整 |
| 3 | root agent 不得默认信任 sub-agent 报告，必须独立交叉验证关键产出物 | 视为汇总失败 |
| 4 | sub-agent 不得直接对话用户（除非任务卡明确授权） | 视为越权 |
| 5 | 多个 sub-agent 不得并发写同一文件 / 同一目录 | 立即终止冲突 sub-agent |
| 6 | sub-agent 失败时不得静默忽略，必须更新 state.json + 通知 root | 视为任务丢失 |
| 7 | root agent 决定"单 Agent 继续"时，必须向用户说明理由（"多 Agent 拆分代价大于收益"） | 视为擅自降级 |
| 8 | ⑥.10 测试真实性必须由 test-verifier sub-agent 独立验证，**主 agent 不得自我验证** | 视为违反 ⑥.10 门禁 |

### 多 Agent 模式 vs 单 Agent 模式对比

| 维度 | 单 Agent 模式（默认） | 多 Agent 模式（启用后） |
|------|---------------------|---------------------|
| 适用场景 | 1-2 Story / 强依赖 / 简单任务 | 3+ Story / 弱依赖 / 高错误代价 / 需独立验证 |
| 速度 | 串行，慢 | 并行，快 2-3 倍 |
| 上下文连续性 | ✅ 强（同一 agent 全程） | ⚠️ 弱（sub-agent 需靠任务卡传递上下文） |
| 决策一致性 | ✅ 强 | ⚠️ 中（root agent 需做交叉验证） |
| 成本 | 低 | 中（每个 sub-agent 独立上下文） |
| 适用阶段 | 全阶段通用 | 各阶段子任务可并行 |
| 推荐使用 | 默认 | **明确场景下启用** |

> **🔴 默认单 Agent，遇到符合"何时启用"表的场景时 AI 必须主动提议。** 但用户可随时说"继续单 Agent 串行做"拒绝。

### 与 mavis-team skill 的衔接

> 本节是 auto-engineering SKILL 内部的"任务分配机制"**规范层**——定义何时派活、派什么活、如何汇总。**具体执行层**通过 `mavis-team` skill（复杂多轨道）或 `mavis communication send --command spawn`（单点验证）实现。
>
> 详细执行流程参考 `mavis-team` skill 文档；本节是 auto-engineering 场景下的"业务规则"层。

### 与现有 SKILL 章节的衔接

| auto-engineering 章节 | 多 Agent 应用 |
|---------------------|-------------|
| Phase 1 ① 生成 Story | 可派 `story-writer` sub-agent |
| Phase 1 ② Story Review | 可派 2 个 `story-reviewer`（BE + FE） |
| Phase 1 ③ 测试用例 | 可派 `testcase-writer` sub-agent |
| Phase 2 ④ Task 生成 | 可派 `task-writer` sub-agent |
| Phase 2 ④bis CodingPlan | 可派 `plan-writer` sub-agent |
| Phase 2 ⑤ Coding | 建议**单 coder**（避免文件冲突） |
| Phase 3 ⑥.10 测试真实性 | **🔴 强制派 `test-verifier` sub-agent 独立验证** |
| Phase 3 ⑦ CodeReview | 派 `code-reviewer` sub-agent（**走 `code-review-skill.md` §多 Agent 评审编排** — 6 阶段 + 7 道闸） |

---

## ⏱️ 节点级上下文压力软提示（🆕 v3.5.5 — 6 个审核点边界必调）

> **🔴 背景：** v3.3.0 引入 PRD 级 compact（`mavis session rotate --handoff-file`，事后收尾）+ v3.5.2 加 ⑦ter 自检 + v3.5.4 加 HS-8 compact 失败检测 — 但这些都是**事后机制**，没有**事前预警**。主会话跑到第 4 个审核点时已被吃满，AI 自己感知不到继续硬扛。v3.5.5 引入"节点级软提示"，在 6 个审核点边界自检压力等级。
>
> **🔴 核心立场（与 v3.3.0 PRD 级 compact 的关系）：**
> - 本机制 = **节点级预警**（提前提示，让用户决定是否进入收尾）
> - v3.3.0 PRD 级 compact = **事后收尾**（实际交接）
> - 两者**不替代**：critical 提示中的"建议 PRD 收尾"由用户决定是否触发，不是强制 compact

### 触发时机（6 个审核点边界必调）

| 审核点 | 章节锚点 | 调用命令 | 软提示后行为 |
|---|---|---|---|
| 1（设计阶段完成） | §Phase 1 末 | `ae-sdd context-pressure --story {STORY-ID}` | 继续进 Phase 2 |
| 1.5（实现方案预确认） | §Phase 2 头部 | 同上 | 继续进 Task 生成 |
| 2（Task 文档完成） | §Phase 2 中段 | 同上 | 继续进 CodingPlan |
| 2.5（CodingPlan 评审） | §Phase 2 中段 | 同上 | 继续进 ⑤ Coding |
| 4（CodeReview 完成） | §Phase 3 末 | 同上 | 继续进 ⑦ter 自检 |
| 5（PRD 完成确认） | §PRD 完成判定 | 同上 | critical 时强烈建议进入 PRD 收尾 + runtime compact |

### 行为约束（🔴 红线）

- 🔴 **仅软提示（report-only），不阻断流程**
- 🔴 **不自动 compact**，不自动派 sub-agent
- 🔴 **不写入 state.json / session.json**（无持久化副作用）
- 🟢 medium / high：对话中输出 ⚠/🟠 提示 + signals 数据
- 🔴 critical：额外输出**推荐动作清单**（运行 `prd-check-complete` / 考虑 PRD 收尾 + compact / 考虑拆分 Story），仍不阻断

### 5 信号采集（全为已有字段，无新 schema 必填）

| 信号 | 来源 | 含义 |
|---|---|---|
| `confirmedPhases` | `session.userConfirmedPhases.length` | 已确认审核点数 |
| `events` | `state.events.length` | 流程操作次数 |
| `historyLen` | `state.history.length` | phase 跳转次数 |
| `docBytes` | 扫 `.ae-sdd/{STORY}/` + `.auto-engineering/{STORY}/` + `design/` + `task/` | 落盘文档总字节 |
| `activeAgents` | `state.activeAgents.length` | 当前并发 sub-agent 数 |

### 缺省阈值表（可被 config.yaml override）

```python
DEFAULT_THRESHOLDS = {
    "medium":   {"docBytes": 500_000,   "events": 100, "historyLen": 5,  "confirmedPhases": 3, "activeAgents": 2},
    "high":     {"docBytes": 2_000_000, "events": 200, "historyLen": 8,  "confirmedPhases": 4, "activeAgents": 3},
    "critical": {"docBytes": 5_000_000, "events": 400, "historyLen": 10, "confirmedPhases": 5, "activeAgents": 4},
}
```

**评级算法**：任一信号达到对应档位 → 该档位；OR 触发取最高档（critical 优先）。

### config.yaml 覆盖配置（可选）

在项目根 `.ae-sdd/config.yaml` 中追加：

```yaml
contextPressure:
  thresholds:
    medium:   {docBytes: 600000,  events: 120, historyLen: 6, confirmedPhases: 3, activeAgents: 2}
    high:     {docBytes: 2500000, events: 250, historyLen: 9, confirmedPhases: 4, activeAgents: 3}
    critical: {docBytes: 6000000, events: 500, historyLen: 11, confirmedPhases: 5, activeAgents: 4}
```

> 字段缺失或非法 → 保留缺省值，不抛异常（`tools/lib/context_pressure.py` 的 `_parse_nested_config` 内置兜底）。

### AE 编排层 SOP（6 个审核点统一）

```
1. 用户 ✅ 确认本审核点
2. AI 调 `ae-sdd context-pressure --story {STORY-ID}`
3. AI 解析返回 JSON 的 `pressure` 字段
4. AI 按评级采取不同行为（仅对话展示，不改 state）
5. AI 继续下一步流程（不阻断）
```

### 对话内呈现模板

```
⏱️  上下文压力：medium（⚠）
   signals: confirmedPhases=3 | events=142 | history=8 | docBytes=3.0MB | activeAgents=2
   触发信号：docBytes=3MB ≥ high(2MB)
   （critical 时额外输出推荐动作清单）
   nextAction: context-pressure is informational only; no action required
```

### CLI 速查

```bash
ae-sdd context-pressure                  # 项目级（无 story）
ae-sdd context-pressure --story STORY-001-BE   # Story 级
ae-sdd context-pressure --json           # JSON 输出（机器消费）
```

> **🟢 与现有机制的关系：** 与 `ae-sdd iteration-check`（v3.5.4 设计-实现一致性检查，report-only）同级 — 都是 report-only 不阻断的"健康度体检"。`iteration-check` 看 SKILL 文档与实现的一致性；`context-pressure` 看主会话剩余容量。

---

## 流程状态跟踪与再启动

### 状态跟踪（强制）

AI 在本 SKILL 运行期间，**必须持续维护以下状态**，每个 Story 独立存储在 `.auto-engineering/{STORY-ID}/state.json`（工程目录根路径）：

```
.auto-engineering/
├── STORY-010-BE/
│   └── state.json
├── STORY-011-BE/
│   └── state.json
└── ...
```

```json
{
  "storyId": "STORY-010-BE",
  "storyVersion": "v1",
  "codingRound": "r1",
  "currentPhase": "Phase 1",
  "currentStep": "step-3-testcase",
  "scale": "大",
  "entryNode": "PRD",
  "completedSteps": ["step-1-dr2story", "step-2-story-review"],
  "pendingOutputs": {
    "storyDoc": "ae-sdd-doc/iterations/{date}/Story/STORY-010-BE.md",
    "testcase": "ae-sdd-doc/iterations/{date}/Test/STORY-010-BE/STORY-010-BE-testcase.md"
  },
  "lastUpdated": "2026-05-26T10:00:00"
}
```

> **🆕 v3.5.15 多入口状态机（4 子链 + scale 路由）：** `scale` 字段决定走哪条子链，`entryNode` 记入口语义。
>
> | scale | 子链（phase 序列） | 适用场景 |
> |-------|------------------|---------|
> | 大（11 phase） | initialized→ra→dr→story→story-rev→task→task-rev→coding→test→cr→completed | PRD/中大需求，完整主干 |
> | 中（10 phase） | initialized→ra→story→story-rev→task→task-rev→coding→test→cr→completed | 中任务，跳过 DR |
> | 小（8 phase） | initialized→ra→task→task-rev→coding→test→cr→completed | 小任务，跳过 DR/Story |
> | 微（4 phase） | initialized→coding→test-running→completed | 微任务/BUG/配置类，跳过 RA/DR/Story/Task |
>
> - **scale 写入时机**：首次 `ae-sdd state write --phase X --scale <大\|中\|小\|微>` 携带；旧 state 无 scale → 按 completedSteps/phase 反推，默认"大"（最保守）
> - **entryNode**：FlowNode.value（BUG/CONFIG/PRD/RA/DR/STORY/TASK/PLAN），仅记入口语义，BUG/配置类复用微链
> - **修复可观测 bug**：微任务停在 initialized 时，`next_step` 建议"进 coding"而非"跑 RA"

> **storyVersion 变更时机：** Story 主文档发生变更（内容修改、补充说明合入）后累加。用于报告文件命名和 DR-Story 一致性追踪。
> **codingRound 变更时机：** 每次开始新一轮 Coding 实现（不论是因为缺陷修复、增量补充还是重构）前累加。每一轮 Coding 独立出具该轮的 CodeReview 报告。

**每次进入新步骤前**：必须读取 `.auto-engineering/{STORY-ID}/state.json`，确认当前步骤和已完成步骤。
**每次完成步骤后**：必须更新 `.auto-engineering/{STORY-ID}/state.json`，记录完成的步骤和产出物路径。
**版本号更新时机**：
  - `storyVersion` 在 Story 主文档发生内容变更后累加
  - `codingRound` 在开始新一轮 Coding（不论任务类型）前累加
**Story ID 来源**：从用户输入或 state.json 中读取，不可混用不同 Story 的状态文件。

---

### 流程脱离与再启动

#### 场景一：用户偏离流程（说其他话题）

**判定：** 用户消息与当前 Story/Task/Coding 无关，且不是流程节点询问。

**AI 动作（强制）：**
1. 简短回应用户话题（不阻塞）
2. 不更新 `.auto-engineering/{STORY-ID}/state.json`（保持当前步骤不变）
3. 下次对话时，若用户未明确说"继续/回到流程"，则询问："当前流程停在 [{当前步骤}]，是否继续？"

#### 场景二：用户明确说要继续/回到流程

**触发条件：** 用户说"继续"、"回到流程"、"继续上次"、"接着来"等。

**AI 动作（强制）：**
1. 读取 `.auto-engineering/{STORY-ID}/state.json`（STORY-ID 从上下文或用户输入获取）
2. 定位当前步骤和已完成步骤
3. 输出状态摘要：
   ```
   【流程已恢复】
   Story ID：{STORY-ID}
   当前阶段：Phase 1 - 设计阶段
   当前步骤：③ 生成测试用例
   已完成：① Story 生成、② Story Review
   待完成：③bis 用例校验 → 🔍 人工审核 → Phase 2...
   ```
4. 从当前步骤继续执行

#### 场景三：用户说要重启/重新开始

**触发条件：** 用户明确说"重新开始"、"从头来过"。

**AI 动作（强制）：**
1. 确认用户意图（询问是否确认放弃当前进度）
2. 用户确认后，删除 `.auto-engineering/{STORY-ID}/state.json`
3. 从 `Phase 1 ①` 重新开始

#### 场景四：用户切换到其他 Story

**触发条件：** 用户提供了新的 Story ID 或 DR 路径。

**AI 动作（强制）：**
1. 读取当前 Story 的 `.auto-engineering/{当前STORY-ID}/state.json`，确认是否有未完成的 Story
2. 询问用户：是否要暂停当前 Story，先处理新的？
3. 用户确认后，切换到新 Story ID，读取或创建 `.auto-engineering/{新STORY-ID}/state.json`
4. 两个 Story 的状态文件互不影响，可随时切换回来

#### 场景五：PRD 完成 / 进入下一个 PRD（🆕 v3.3.0）

**触发条件：** 用户说「PRD 收尾了 / 进入下一个 PRD」或所有 Story 完成后 AI 检测到 PRD 完成时机。

**AI 动作（强制）：**
1. **自动先校验 4 层 AND 闸**（`ae-sdd state prd-check-complete --prd {PRD-ID}`），输出未达成项
2. 若 4 层 AND 未全过 → 列出阻塞项，**不**直接进入 compact
3. 若 4 层 AND 全过 → 提示用户「PRD 完成确认」（🔍 人工审核点 5，参见 §1.5）
4. 用户确认后 → 执行 `ae-sdd state prd-complete --prd {PRD-ID} --runtime {runtime-name}`
5. compact 完成后 → 写 `.auto-engineering/{PRD-ID}/summary.md` + 预生成 `PRD-NEXT` 模板指针

---

### 再启动判定规则

| 用户意图 | AI 动作 |
|---------|---------|
| 继续上次流程（继续/接着做/接着来） | 读取 `.auto-engineering/{STORY-ID}/state.json`，恢复到当前步骤 |
| 从某个步骤继续（从xx开始/重新做xx） | 读取 `.auto-engineering/{STORY-ID}/state.json`，从指定步骤恢复（不可倒退已完成的关键门禁：Phase 1 完成确认、Phase 2 实现方案预确认、Task 文档完成确认、Phase 3 完成判定、⑥bis 全切面一致性核查闸、CodeReview 报告出具、⑦bis 全链路对称性核查闸） |
| 放弃当前，重新开始 | 确认后删除 `.auto-engineering/{STORY-ID}/state.json`，从 Phase 1 ① 重启 |
| 切换到另一个 Story | 保留当前 Story 的 state.json，切换到新 Story 的 `.auto-engineering/{新STORY-ID}/state.json` |
| 偏离流程（其他话题） | 简短回应，不更新 state，保持步骤不变 |
| **PRD 完成 / 进入下一个 PRD**（🆕 v3.3.0） | 读 `.auto-engineering/{PRD-ID}/state.json`，校验 prdStatus=compacted，写 next-prd 指针 | `.auto-engineering/{PRD-ID}/state.json` + `.auto-engineering/PRD-NEXT/state.json` 模板预生成 |

---

### § 流程状态跟踪与再启动（PRD 级）— 🆕 v3.3.0

> **状态机归属（🔴 单点持有）：** 本节由 `SKILL.md` 单点持有 PRD 级状态机定义；子 SKILL（`phase1-design` / `phase2-coding` / `phase3-review`）通过指针引用本节，**不**独立发明 PRD 级状态字段。如发现子 SKILL 写了自己的 PRD 级状态字段 → 视为违规。

#### 1.1 PRD 级状态文件路径

| 文件 | 路径 | 写入方 | 读取方 |
|------|------|--------|--------|
| `state.json` | `.auto-engineering/{PRD-ID}/state.json` | Story 完成 hook、PRD 收尾 CLI | 所有 phase SKILL、CLI |
| `state.md` | `.auto-engineering/{PRD-ID}/state.md` | `ae-sdd state prd-complete`（一次性） | 用户、handoff 包 |
| `summary.md` | `.auto-engineering/{PRD-ID}/summary.md` | `mavis session rotate --handoff-file` | 下一个 session / 下一个 PRD |

> **🔴 与 Story 级 state.json 共存：** PRD 级 `state.json` 与 Story 级 `.auto-engineering/{STORY-ID}/state.json` 互不替换；PRD 级聚合 Story 级数据。详见 §1.3 schema。

#### 1.2 PRD ID 命名规范

格式：`PRD-<业务域>-<序号>`（3 段，kebab-case）

- 业务域：CS / IM / USER / LIFE（与 `dr-review-skill.md:184` DR ID 业务域对齐）
- 序号：3 位数字，从 001 起
- 示例：`PRD-CS-001`、`PRD-IM-002`、`PRD-USER-001`

#### 1.3 PRD 级 `state.json` schema（SSOT）

完整 schema 定义在 `document-storage-skill.md §3.5`（包含 5 核心 + 3 runtime 字段）。本节只列指针：

```json
{
  "prdId": "PRD-CS-001",
  "storyIds": [...],
  "crossStoryDeps": [...],
  "crossStoryResidualRisks": [...],
  "sizeBudget": {...},
  "prdReview": {...},
  "runtimeHooks": {...},
  "gateRegistry": {"G-PRD-1": "pending", ...},
  "prdStatus": "in_progress | prd_complete_pending_user | awaiting_compact | compacted | prd_aborted",
  "compactHistory": []
}
```

字段演进：所有新字段均为 **optional + 默认值**，旧 PRD 级 state.json 缺字段不报错（v3.3.0 兼容策略）。

#### 1.4 流程脱离场景扩展（5 场景）

将 §流程脱离与再启动 章节（4 场景）扩展到 **5 场景**，新增「场景 5：PRD 完成 / 进入下一个 PRD」。详见 `document-storage-skill.md §4.3` 的扩展决策树，本 SKILL 不再重复定义。

---

### § PRD 完成判定 SOP（4 层 AND + 跨 Story 闸）— 🆕 v3.3.0

#### 1.5 🔍 人工审核点 5：PRD 完成确认（新增）

与现有 4 个 Story 级人工审核点（`SKILL.md:1167` 审核点 4、Phase 1 审核、Phase 2 审核、CodingPlan 审核 2.5）同级，编号 **5**。

**触发时机：** 4 层 AND 全过 + 用户说「PRD 收尾了 / 进入下一个 PRD」

**AI 主动讲解模板（基于 §SKILL.md:1300-1343 现有讲解模板扩展，PRD 级视角）：**

1. PRD 业务全貌（PRD 文档 + DR 摘要）
2. 各 Story 完成情况（聚合自 state.json.storyIds）
3. **跨 Story 关键决策**（从 `crossStoryDeps` + `crossStoryResidualRisks` 提取）
4. **sizeBudget 实际 vs 估算**（新增维度）
5. 残留风险清单（owner + dueDate）

**记录字段：**
```json
{
  "prdReview": {
    "confirmedAt": "2026-06-25T...",
    "confirmedBy": "<user-id>",
    "storytoldAt": "2026-06-25T...",
    "openQuestions": []
  }
}
```

#### 1.6 PRD 完成判定闸（4 层 AND）

```
G-PRD-1 (Story 全部完成):
  ∀ STORY-ID ∈ prdState.storyIds:
    story.codeReviewReport 存在
    ∧ story.sevenBisPassed == true
    ∧ story.userConfirmedAt 非空

G-PRD-2 (Story ⑦bis 全通过):
  ∀ STORY-ID: story.sevenBisMatrix 无 🔴 断链
  ∧ ∀ STORY-ID: story.sevenBisMatrix 出闸条件满足

G-PRD-3 (跨 Story 残留风险已闭环):
  crossStoryDeps[].verifiedAt 全部非空
  ∧ crossStoryResidualRisks[].mitigationPlan 全部非空
  ∧ ∀ risk.severity == "🟢" 或 risk.dueDate > now

G-PRD-4 (PRD 级人工审核通过):
  prdReview.confirmedAt 非空
  ∧ prdReview.confirmedBy 存在
```

**CLI 入口：**
- `ae-sdd state prd-check-complete --prd {PRD-ID}` — 只校验 4 层 AND，输出未达成项，**不改状态**
- `ae-sdd state prd-complete --prd {PRD-ID} --runtime {mavis|claude-code|codex}` — 校验通过后执行 compact，更新 prdStatus

#### 1.7 PRD 收尾合规自检 SOP（🔴 v3.5.2 — 堵 prd-complete 跳校验漏洞）

> **🆕 v3.5.2 新增（2026-06-27，用户需求"每次流程结束时先自检合规，不合规就修复"）：** 与 Story 级 ⑦ter 自检配套的 PRD 级收尾自检。
>
> **背景（已探查的实现 gap）：** `cmd_state_prd_complete`（`tools/bin/ae-sdd:268-271`）把 4 层 AND 校验注释为"简化：提示用户先跑"，实际**不复跑校验**直接把 `prdStatus` 写为 `awaiting_compact` → 存在"跳过 `prd-check-complete` 直接 compact"的漏洞路径。本 SOP 在编排层强制补齐，与 Story 级 ⑦ter 同源（防止 AI 跳自检直接收尾）。

**强制 3 步（不可跳过）：**

```
PRD 收尾触发（用户说"PRD 收尾" / 所有 Story 完成）
    │
    ├── 1. 先跑 `ae-sdd state prd-check-complete --prd {PRD-ID}` → 读 JSON `all_pass`
    │
    ├── 2. 判定：
    │     ├─ all_pass == false → 【禁止】跑 prd-complete，进自愈
    │     │     自愈映射：
    │     │     ├─ G-PRD-1 missing（Story 未完成）→ 回该 Story 级流程，跑 Story 级 ⑦ter
    │     │     ├─ G-PRD-2 missing（⑦bis 未过）→ 回该 Story ⑦bis 闸
    │     │     ├─ G-PRD-3 missing（跨 Story 风险未闭环）→ 补 mitigationPlan / 降级风险
    │     │     └─ G-PRD-4 missing（人工审核点 5 未做）→ 触发审核点 5，等用户确认
    │     │     自愈后【重跑 prd-check-complete】确认 all_pass == true
    │     │
    │     └─ all_pass == true + 用户已确认审核点 5 → 进步骤 3
    │
    └── 3. 收尾执行（顺序不可颠倒）：
          a. `ae-sdd state prd-complete --prd {PRD-ID} --runtime {runtime}` → prdStatus = awaiting_compact
          b. `ae-sdd runtime compact` → compact + 写 summary.md
          c. 写 next-prd 指针
```

**AE 编排层门禁（本 SKILL 只关注 3 条）：**
- ✅ `prd-complete` 前必须保留 `prd-check-complete` 的 `all_pass == true` 证据（写入对话 / 报告）
- ✅ 跳过 `prd-check-complete` 直接 `prd-complete` = 违规，后续 compact 视为**事后回溯**
- ✅ PRD 收尾自检表须对话内呈现（同 Story 级 ⑦ter，列 G-PRD-1~4 各层 pass/missing）

> **为何不动 tools/ 代码补强 `prd-complete`？** 按用户选定路线（SKILL 文字 + 复用现有 CLI），`prd-check-complete` 已是现成只读校验命令，编排层 SOP 强制"先跑 check-complete 再跑 complete"即可堵漏洞，无需改 `cmd_state_prd_complete` 实现（避免 UC-02/UC-03 连带 + 测试编写，符合 KISS）。后续若要硬阻断，可独立提需求加 `prd-complete --require-check` 参数。

---

## 整体流程

```
【必须严格执行以下顺序，不可跳过任何步骤】

输入：DR 文档路径 + Story ID + 工作目录
                │
Phase 1 ────────┤ 设计阶段（必须完成）
                │
                ├── ① 执行 DRtoStory SKILL → 生成 Story
                │
                ├── ①bis 🔴 前端视角接口审视（强制）→ 补全前端接口契约章节
                │
                ├── ② 执行 Story Review SKILL → 挖掘/判定 → StoryReviewUpdatePlan → 按 Plan 修复循环 → Story 稳定
                │
                ├── ③ 生成测试用例
                │
                ├── ③bis 用例合规性校验（必须通过）
                │
                └── 🔍 人工审核：确认设计阶段完成（必须通过）
                    ⏱️ v3.5.5：用户 ✅ 后调 `ae-sdd context-pressure`（审核点 1 软提示，不阻断）

Phase 2 ────────┤ 实现阶段（必须完成）
                │
                ├── 🔍 人工审核：实现方案预确认（必须通过）
                │     在生成 Task 文档之前，对核心业务/接口/分层/并发/异常达成共识
                │     ⏱️ v3.5.5：用户 ✅ 后调 `ae-sdd context-pressure`（审核点 1.5 软提示，不阻断）
                │
                ├── ④ 执行 Task Generate SKILL → 生成 Task 文档 → 全局 Task Review（结合约束+Story+测试用例）→ Task 实现方案
                │
                ├── ④bis 🔴 CodingPlan 输出（可执行层）→ 文件级实现顺序 + 关键代码骨架 + 验证点
                │
                ├── 🔍 人工审核 2.5【🆕 2026-06-10】：CodingPlan 评审
                │     复核 16 章节 + 14 条门禁 + CodingModel 决策 + 风险 Task
                │     用户明确确认后 → ⑤ Coding
                │     ⏱️ v3.5.5：用户 ✅ 后调 `ae-sdd context-pressure`（审核点 2 软提示，不阻断）
                │
                └── 🔍 人工审核：确认 Task 文档 + 实现方案 + CodingPlan 完成（必须通过）
                    ⏱️ v3.5.5：用户 ✅ 后调 `ae-sdd context-pressure`（审核点 2.5 软提示，不阻断）

                ├── ⑤ 执行 Coding SKILL
                │     每个 Task 开始前必须呈现实现方案并获用户确认
                │     用户确认后才能开始写代码
                │
                └── ~~🔍 人工审核：确认编码阶段完成~~（🗑️ 2026-06-10 删除，合并到审核点 4）

Phase 3 ────────┤ 验证阶段（必须完成）
                │
                ├── ⑥ 完成判定（全部条件通过）
                │
                ├── ⑥bis 编码后全切面一致性核查闸（🔴 CodeReview 硬前置，每轮强制）
                │
                ├── ⑦ 出具 CodeReview 报告（必须完成）
                │
                ├── ⑦bis 全链路对称性核查闸（🔴 流程收尾强制：DR-Story-Task-实现-测试用例 五层一一对应）
                │
                ├── 🔍 人工审核 4：CodeReview 阶段完成确认（🆕 2026-06-10 改名，原"验证阶段"）
                │     ⏱️ v3.5.5：用户 ✅ 后调 `ae-sdd context-pressure`（审核点 4 软提示，不阻断）
                │
                ├── ⑦ter 流程收尾合规自检（🔴 v3.5.2 — 用户确认后自跑 5 维度自检，不合规就修复，禁止裸 ✅ 收尾）
                │
                └── ⑧ 完成 ✅

PRD 收尾（v3.3.0，可选入口）：
                │
                └── 🔍 人工审核点 5：PRD 完成确认（4 层 AND 全过后）
                    ⏱️ v3.5.5：用户 ✅ 后调 `ae-sdd context-pressure`（审核点 5 软提示；critical 时强烈建议进入 PRD 收尾 + runtime compact）
```

---

## 输入

```
必须提供以下三项信息，缺一不可：
1. DR 文档路径
2. 要实现的 Story ID（或"全部"）
3. 工作目录（各工程所在的磁盘根路径）
```

---

## Phase 1：设计阶段（必须完成，不可跳过）

### ① 生成 Story

**触发：** [DRtoStory SKILL]（已有）

**输入：** DR 文档路径 + Story ID
**输出：** Story 主文档
**必须参考模板：** `templates/design/be-story-template.md`

**跳过条件：** Story 文档已存在且状态非 Draft

---

### ①bis 前端视角接口审视（🔴 强制 — 6 维度审查清单已下沉）

> **📍 详细 6 维度审查清单已下沉：** 原 AE-skill 中 ①bis 的 6 维度（接口契约完整性/调用流程/状态展示/错误码/边界场景/联调支持）+ 产出物模板（嵌入 Story 文档的"前端接口契约"章节）+ 🔴 门禁（共 6 条），已统一存放在 [`story-review-skill.md` §📋 ①bis 前端视角接口审视 — 6 维度审查清单](../phase1-design/story-review-skill.md)，本 SKILL 不再重复。
>
> **AE 编排层只关注 3 个门禁：**
> - ✅ ① 生成 Story 完成后、② Story Review 之前，必须完成 ①bis 6 维度审视
> - ✅ Story 文档"前端接口契约"章节已嵌入（含至少 1 个完整请求+响应示例）
> - ✅ Story Review SKILL 的"零、Story 准入检查"已包含"前端接口契约章节完整性"勾选项
> - 若前端约束规范 `constraints/frontend.md` 不存在，本步骤降级执行但需在 Story 中标注"**前端约束待补充**"

---

### ② Story Review

**触发：** [Story Review SKILL](../phase1-design/story-review-skill.md)

**输入：** Story 文档 + DR 文档 + `templates/design/be-story-template.md` + 约束
**输出：** 稳定的 Story 文档（无新增可修改项）+ `{STORY-ID}-StoryReviewUpdatePlan-r{轮次}.md`（每轮有确认缺陷时必出）

**内部循环：**
```
挖掘 → 判定 → 生成 StoryReviewUpdatePlan → 按 Plan 修复 → 再挖掘 → ... → 退出
```

> **📍 详细 Plan-first 规则已下沉：** `StoryReviewUpdatePlan` 的内容、模板、字段链路修订计划和出闸条件见 [`story-review-skill.md` §Plan-first 更新原则](../phase1-design/story-review-skill.md) 与 [`templates/design/be-story-review-update-plan-template.md`](../../templates/design/be-story-review-update-plan-template.md)。AE 编排层只关注：有确认缺陷时必须先有 Plan；Story Update 必须按 Plan 执行；Plan 外业务语义修改视为无效更新。

**🔴 必须新增 F-Stage（前端契约 Review）：**
> ①bis 已要求 Story 包含"前端接口契约章节"。② Story Review **必须** 把这一章节作为 Review 维度之一，增加 **F-Stage：前端契约 Review**。F-Stage 至少包含以下检查项：
> 1. 6 个维度是否都覆盖了（接口契约完整性 / 调用流程 / 状态展示 / 错误码 / 边界场景 / 联调支持）
> 2. 字段是否同时满足后端实现和前端对接的需求（不偏废任何一方）
> 3. 错误码前端处理建议是否可执行（不是"toast 一下"这种空话）
> 4. 状态流转 UI 展示建议是否可执行（颜色/图标有具体值）
> 5. 时间/金额/ID 字段的格式与项目前端基线是否一致
> 6. 联调信息（mock / 环境 / 时间窗口）是否齐全
>
> F-Stage 未通过 → Story Review 视为不完整，禁止退出循环。

> **🔴 v3.4.3 废弃"每 3 轮暂停问人"：** 之前本节有"每完成 3 轮 A-E 阶段循环自动暂停询问用户"规则，与退出条件矛盾且违反 Loop Engineering 自评估原则，已废弃。Story Review 退出条件统一遵守 [`review-loop-skill.md`](skills/cross-cutting/review-loop-skill.md) 公共协议（连续 3 轮无新增才退出；3 轮仍有 🔴 升级用户）。

---

### ③ 生成测试用例

**触发：** Story Review SKILL 第七步退出后自动触发，或开发者说"生成测试用例"

**触发 SKILL：** [TestCase Generate SKILL](../phase1-design/testcase-generate-skill.md)

**输入：**
- Story 主文档
- DR 文档
- `strategies/be-testcase-strategy.md`（测试策略模板）
- `templates/testcase/be-testcase-template.md`（用例模板）
- 约束通过 `document-storage-skill.get_constraints(projectKey)` 加载（或由 testcase-generate-skill §1.0b assets.forTestCase 返回）

**输出：** 测试用例文档（路径由 `documentStorage.resolve_path(intent="TEST", storyId)` 定位）

**SKILL 内部流程：**
1. 读取 Story + 策略模板 + 用例模板 + 约束
2. 识别 Story 类型，选择覆盖策略（状态机/CRUD/回调/定时任务/集成）
3. 按三层模型生成用例（类型策略 + 通用维度 + 测试分层）
4. 合规性校验（全部检查项通过才可退出）
5. AC 完整性检查（如有缺失需反馈到 Supplement）
6. 输出测试用例文档

**禁止：**
- 不参考策略模板直接生成 ❌
- 跳过合规性校验 ❌
- 只覆盖 AC，不做全场景覆盖 ❌

---

### ③bis 业务逻辑汇总输出

**触发时机：** Story Review 循环退出后，测试用例生成完成后，进入人工审核之前

**前置条件：** Story Review 的 C8 数据视角总览已完成（含 C8-4 字段链路映射）；①bis 前端视角接口审视已完成

**输出模板：** `templates/design/be-story-review-logic-summary-template.md`

**产出物：** `{STORY-ID}-业务逻辑汇总.md`

**填写规则：** AI 必须基于 Story 文档和 C8 数据视角总览（C8-1~C8-4），填写模板中的所有章节。禁止留空或填"待补充"（如有不确定项，必须标注"需人工确认"）。

**🔴 必须新增"前端对接维度"章节：**
> 业务逻辑汇总表除业务逻辑外，必须包含"**前端对接一览**"章节，汇总从 ①bis 前端接口契约中提取的要点：
> 1. 对外接口清单（接口名 / Method / URL / 调用方）
> 2. 关键字段约定（时间格式、ID 类型、金额单位）
> 3. 状态枚举与 UI 展示映射
> 4. 错误码前端处理建议汇总
> 5. 联调关键信息（mock 平台 / 测试环境 / 联系人 / 时间窗口）
>
> 此章节是**人工审核点 1** 时业务方 / 前端负责人快速确认前端对接可行性的依据。

**人工审核前必须完成此汇总：** 审核者基于此表快速判断业务逻辑完整性，跳过此步骤不得进入人工审核。

---

### 🔍 人工审核点 1：设计阶段完成确认

**触发时机：** 测试用例生成完成后，进入 Phase 2 之前

#### 📖 AI 主动讲解（Story 故事，🔴 强制）

> **本节点禁止直接问"请确认"。** AI 必须先用讲故事的笔法，主动向用户讲清楚本 Story 的业务背景、核心流程、关键设计决策、AC 故事和已识别风险点（详细规范见上文 `📖 人工审核主动讲解规范` 章节）。讲完后才能进入"对话内直接呈现"环节。

讲解模板参考上文 `① StoryReview 阶段 —— 讲"业务设计故事"` 的输出模板，**必须覆盖**：
1. 业务背景与典型用户故事
2. 核心业务流程（从前台到后端）
3. 关键设计决策（状态机/接口/数据模型）的"为什么"
4. 每个 AC 背后的用户场景
5. 已识别风险点与应对

#### 📋 对话内直接呈现（🔴 讲解完成后必须执行，禁止以"请查阅Story文档"代替）

> AI 讲解完毕后，必须在对话中直接输出以下内容，用户无需打开任何文档即可在对话中完成审核。

**必须直接输出在对话中的内容：**

```
📋 【{STORY-ID} 设计阶段审核 — 对话内呈现】

一、AC 验收标准（完整列表）
| AC 编号 | 场景描述 | 验收条件 | 备注 |
|---------|---------|---------|------|
| AC-001  | {场景}  | {条件}  | {如有}|
...（逐条，不省略）

二、核心接口一览
| 接口名 | Method | URL | 调用方 | 关键入参 | 关键出参 |
|--------|--------|-----|--------|---------|---------|
| {接口} | POST   | /xx | BFF    | {入参}  | {出参}  |
...（每个对外接口一行）

三、关键设计决策（需用户确认）
| 决策项 | 选择方案 | 放弃方案 | 选择原因 |
|--------|---------|---------|---------|
| 状态机实现 | {方案} | {方案} | {原因} |
...（每个非平凡决策一行）

四、已识别风险点
- 风险1：{描述} → 应对：{方案}
- 风险2：{描述} → 应对：{方案}

五、测试用例数量：共 {N} 条，覆盖 AC {X}/{Y} 个
```

**请您重点确认以下问题（逐条回复）：**
1. AC 列表是否完整？有没有遗漏的验收场景？
2. 核心接口的 URL/入参/出参是否符合预期？
3. 哪个设计决策你有疑问或想修改？
4. 风险点有没有没考虑到的？

**用户选项（每条内容逐一确认，不接受"整体通过"）：**
- ✅ 全部通过，进入 Phase 2
- ⚠️ {AC编号/接口名/决策项} 需要修改：{说明}
- ❌ 暂停流程

---

## Phase 2：实现阶段（必须完成，不可跳过）

### 🔍 人工审核点 1.5：实现方案预确认（在生成 Task 之前）

**触发时机：** Phase 1 完成后，进入 Phase 2 之前

**目的：** 在写 Task 文档之前，先和用户对齐实现思路，避免 Task 文档写完后返工。

> 🔴 **本节点是"对话内直接呈现"的典型实现**——以下 5 个维度的内容必须由 AI **在对话中用实质内容填写**，而不是把这 5 个问题抛给用户让用户自己回答。AI 先答，用户确认。

**AI 必须在对话中直接输出以下内容（不允许空着等用户填）：**

```
📋 【{STORY-ID} 实现方案预确认 — 对话内呈现】

一、核心业务理解对齐
- 本次 Story 的核心业务：{AI 基于 Story 文档填写，非提问}
- 核心状态机状态及流转：{状态列表 + 流转规则，或"无状态机"}
- 涉及 DB 操作：{增/删/改/查 × 哪几张表}

二、接口与依赖确认
- 对外暴露 SPI 接口：{接口名 + 方法签名 + 调用方}
- 依赖下游 Story/外部服务：{列表，或"无外部依赖"}
- 拟复用的现有代码：{类名:行号，或"无可复用"}

三、分层实现思路
- Domain 层：{聚合根名 + Repository 接口 + 核心方法}
- Application 层：{AppService 核心方法签名}
- Infrastructure 层：{Repository Impl + Feign Client（如有）}
- Interfaces 层：{Controller 端点 + SPI 接口实现}

四、并发与事务策略
- 并发场景：{有/无；有则说明控制方案：乐观锁/悲观锁/幂等键}
- 事务边界：{@Transactional 覆盖的方法 + 范围说明}
- 幂等保障：{方案，或"无需幂等"}

五、异常处理思路
- 核心异常：{异常类名 + 对应错误码（5位）}
- 事务外操作失败：{补偿方案，或"无事务外操作"}
```

**请您确认以下问题（有疑问逐条说明）：**
1. 分层思路是否符合预期？有无分层理解偏差？
2. 复用方案是否合理？有没有应该复用但没提到的现有代码？
3. 并发/事务边界是否与业务预期一致？
4. 异常场景是否覆盖完整？

**用户选项：**
- ✅ 确认实现思路，无误
- ⚠️ 有修改意见（说明修改内容，AI 记录后继续）
- ❌ 暂停流程

> **强制要求：** AI 必须等待用户确认后才能进入生成 Task 文档环节。禁止跳过此节点直接生成 Task。如果用户未回复，最多重试 3 次（每次发送提醒），3 次后仍无回复则暂停，等待用户明确指示。

---

### 🔍 人工审核点 2：Task 文档 + 实现方案完成确认

**触发时机：** Task 文档生成完成 + Task 实现方案输出后，进入 Coding 之前

#### 📖 AI 主动讲解（Task 故事，🔴 强制）

> **本节点禁止"列完 Task 清单就问请确认"。** AI 必须先讲清楚"为什么这样拆 Task"、"依赖链路长什么样"、"每类 Task 风险点在哪"，再走下面的逐文件核对。详细规范见上文 `📖 人工审核主动讲解规范` 章节。

讲解模板参考上文 `② TaskReview 阶段 —— 讲"实现拆解故事"` 的输出模板，**必须覆盖**：
1. **拆 Task 的故事**：为什么拆成 N 个 Task？拆分依据（DDD 聚合根/分层/依赖）？
2. **依赖链路故事**：Task 之间为什么这么排？谁必须先做？
3. **DB 变更故事**：表结构、索引、跨服务一致性
4. **事务边界故事**：每个 Task 的事务范围、事务外操作
5. **风险 Task 标记**：哪些 Task 风险高？为什么？

**讲解 + 逐文件核对的协作流程：**
```
进入本审核点
    │
    ├── 1. AI 先讲一遍"全 Task 拆解故事"（上面的 5 个维度）
    │
    ├── 2. 然后进入逐文件核对循环（见下文）
    │     每核对一个文件，AI 先讲"本 Task 故事"（用上文 ② 的逐 Task 模板）
    │     再读出文件全文，再请用户审阅
    │
    └── 3. 全部文件核对完，最后核对实现方案
```

**审核内容（AI 自检）：**
- [ ] Task 0（环境准备）已生成
- [ ] 所有实现 Task 已生成且包含核心代码示例
- [ ] Task 依赖关系清晰
- [ ] Task 检查项完整
- [ ] **{STORY-ID}-Task实现方案.md 已生成**（汇总所有 Task 的实现要点、依赖关系、DB 变更、事务边界）
- [ ] **🔴 AI 已完成 Task 故事讲解（拆 Task 故事/依赖链路/DB 变更/事务边界/风险 Task）**
- [ ] **🔴 每个 Task 文件核对前 AI 已讲完"本 Task 故事"**

#### 🔴 强制门禁：人工审核必须逐文件自上而下核对（用户原话："我们从上到下一点一点一个文件一个文件的过"）

> **本节点是整个 AE 流程中"对话内直接呈现"的标准实现**——逐文件完整读出即等同于把文档内容直接展示在对话窗口，用户无需打开任何文件即可在对话中完成所有审核。
>
> **AI 不得把全部 Task 文档一次性抛出后等用户"整体确认"。** 用户作为人工审核者，没有义务把 AI 吐出来的一大坨文档自己通读一遍再给一个总评——这等于把审核责任转嫁给用户，违背"人工审核点"的设计本意。
>
> **正确的做法是：AI 主动带用户一个文件一个文件从上到下过，每个文件单独确认后才进入下一个。**

**核对流程（强制，不可整体确认）：**

```
进入人工审核点 2
    │
    ├── 1. AI 列出 Task 文件清单（按文件名字典序排序，天然自上而下）
    │     例如：task-0-env.md → task-1-xxx.md → task-2-yyy.md → ... → {STORY-ID}-Task实现方案.md
    │     （实现方案放在最后核对，因为它依赖前面所有 Task 的决策）
    │
    ├── 2. 对每个 Task 文件，循环执行（用户必须逐文件走完才算完成）：
    │     │
    │     ├── a. AI 完整读出本文件内容（一次性呈现，不省略不摘要）
    │     │     （用户不需要自己打开文件，AI 必须主动呈现）
    │     │
    │     ├── b. AI 主动指出本文件"请用户重点确认"的位置
    │     │     （不是让用户自己找要点）：
    │     │     - 关键决策点（多方案择一）
    │     │     - 与上一 Task 的依赖点
    │     │     - DB 变更 / 事务边界 / 错误码等高风险设计
    │     │     - 不可逆 / 难回退的设计选择
    │     │     - 与 Story AC 的对应点
    │     │
    │     ├── c. 询问用户对本文件的意见（每个文件独立确认）：
    │     │     - ✅ 通过（进入下一个文件）
    │     │     - ⚠️ 需要修改（AI 记录后本文件重审，不进入下一文件）
    │     │     - ⏸️ 暂停（写入 state.json，下次"继续"从本文件继续）
    │     │     - ❌ 终止（清空本轮审核，回 Task Generate 阶段）
    │     │
    │     └── d. 文件被 ⚠️ 修改后，**重走该文件核对流程**，直到 ✅ 才进入下一文件
    │
    ├── 3. 全部 Task 文件 ✅ 通过后，最后核对 {STORY-ID}-Task实现方案.md
    │     （实现方案是"汇总层"，最后看才能对照前面 Task 的实际决策）
    │
    └── 4. 实现方案 ✅ 通过后，输出最终确认：
          "已逐文件核对完毕：[文件名 ✅/⚠️→✅/⏸️] × N + 实现方案 ✅。
           是否确认开始编码实现？"
```

**AI 与用户对话的强制要求：**

| 场景 | ❌ AI 错误行为 | ✅ AI 正确行为 |
|------|-------------|-------------|
| 进入审核 | "Task 已生成，请审核"（让用户自己找） | "现在开始过 Task 文件。按文件名字典序，第 1 个：`task-0-env.md`。" |
| 呈现单文件 | 只列文件名 / 只给摘要 / 让用户自己打开看 | **完整读出文件全文** |
| 提请确认 | "请确认"（不指明看哪里） | "**请重点确认 §3 事务边界**——这与下游 Task-2 强依赖，如有调整会影响 Task-2" |
| 用户回复模糊 | 用户说"好" / "行" / "OK" 即进入下一文件 | **追问确认**："您对当前文件是 ✅ 通过 / ⚠️ 修改 / ⏸️ 暂停？模糊回复需要明确判定。" |
| 用户中途想跳 | 默认跳到末尾"整体确认" | **拒绝跳过**："按 SKILL 规定必须逐文件核对，您可以 ⏸️ 暂停但不能跳到汇总。如要整体重审，请回 Task Generate 阶段。" |
| 全部完成 | "OK 进入编码" | 输出"已逐文件核对：Task-0 ✅ / Task-1 ✅ / Task-2 ⚠️→修复→✅ / ... / 实现方案 ✅ / CodingPlan ✅。是否确认开始编码？" |

**跳过与暂停规则：**

- ⏸️ **暂停**：AI 必须记录"已审核到第 N 个 / 共 X 个文件"、当前文件路径、当前决策（✅/⚠️/⏸️）→ 写入 `.auto-engineering/{STORY-ID}/state.json` 的 `currentStep` 字段 → 下次用户说"继续"时**从第 N+1 个文件继续**，不重头开始。
- ❌ **终止**：清空本轮审核记录 → 触发 Task Generate SKILL 重新生成（或局部修复）→ 修复后回到本审核点，**从第 1 个文件重新走**。
- 🚫 **不存在"快速模式"** —— SKILL 没有快速模式，必须逐文件。

**门禁（出闸条件）：**

- [ ] 全部 Task 文件均获得用户逐文件 ✅ 确认
- [ ] {STORY-ID}-Task实现方案.md 获得用户 ✅ 确认
- [ ] **{STORY-ID}-CodingPlan.md 获得用户 ✅ 确认（详见 ④bis 步骤）**
- [ ] 任何 ⚠️ 已被修复并重新获得 ✅

> **任一未达成 → 不允许进入 ⑤ Coding。** AI 不得以"用户已口头确认整体"等模糊信号绕过门禁。

**违反本门禁的典型反模式（必须避免）：**

- ❌ "我把所有 Task 文档贴在下面，您看一下，确认了我就开始编码。"
- ❌ "Task 文档生成完毕，共 8 个 Task，是否确认开始编码？"（一锅端问）
- ❌ 用户回"好的" / "可以" / "行" → AI 直接进入 ⑤ Coding。
- ❌ AI 自己说"重点是 X"但用户根本没看 X，AI 就当 ✅ 处理。

---

### 🔍 人工审核点 2.5：CodingPlan 评审（🆕 2026-06-10）

**触发时机：** 统一版 CodingPlan 生成后（TaskSkill 第六步完成），进入 ⑤ Coding 之前

**目的：** 在 Coding 之前把 CodingPlan 确认下来，避免 Coding 完后才发现方案有问题（重做 Coding 的成本远高于修改 CodingPlan）。

#### 📖 AI 主动讲解（CodingPlan 故事，🔴 强制）

> 本节点禁止直接问"请审阅 CodingPlan"。AI 必须先用 walkthrough 的方式，把 CodingPlan 主动讲给用户听。

讲解模板（必须覆盖）：

1. **CodingPlan 结构摘要**：16 章节有哪些、哪几章是核心、哪几章是辅助
2. **14 条门禁通过情况**：每条门禁的当前状态（✅/🔴/🟠），重点标红未通过项
3. **关键决策基线对齐**：与 Story `## 实现方案决策基线` 对齐情况（拆分依据/复用能力/五维质量）
4. **CodingModel 决策摘要**：11 维决策的结论 + 核心链路保护（如涉及）
5. **风险 Task 标记**：哪些 Task 风险较高（高并发/批量/外部依赖/支付等）

#### 📋 对话内直接呈现（🔴 讲解完成后必须执行，禁止以"已生成请审阅"代替）

> AI 讲解完毕后，必须在对话中直接输出以下内容，用户无需打开 CodingPlan 文档即可在对话中审核关键决策。

**必须直接输出在对话中的内容：**

```
📋 【{STORY-ID} CodingPlan 评审 — 对话内呈现】

一、14 条门禁通过状态
| 门禁 # | 门禁内容（简述） | 状态 | 备注（不通过时说明）|
|--------|----------------|------|------------------|
| G-01   | Task 0~N 全部生成 | ✅   | —               |
| G-02   | CodingModel 11 维完整 | ✅ | —              |
...（14 条逐行列出）

二、CodingModel 11 维决策摘要
| 维度 | 决策结论 | 核心理由 |
|------|---------|---------|
| 并发控制 | 乐观锁（@Version）| 低并发写入场景 |
| 幂等策略 | 唯一约束 + 状态前置 | 防重复提交 |
| 事务边界 | AppService 单事务 | 无跨服务写操作 |
...（11 维逐行）

三、风险 Task 清单
| Task | 风险类型 | 风险说明 | 应对方案 |
|------|---------|---------|---------|
| Task-3 | 外部依赖 | 调融云 API，可能超时 | 超时重试+降级 |
...（有则列，无则写"本 Story 无高风险 Task"）

四、关键类骨架预览（每层各1个最核心的类，每条强制附源码核对来源标记 — 🆕 v3.4.0 G-CODEPLAN-SRC）
Domain：
```java
// {文件路径假设}
public class {聚合根}DO {
    public void {核心方法}() { ... }
}
【已读源码：domain/message/model/entity/ImMessageDO.java】   ← 已核对的现有同类源码
   或 【待核实源码：Converter 写法】                          ← 未核对项，须补读后改为【已读源码：】
```
Application：
```java
@Transactional
public void {核心方法}({Command} command) { ... }
【已读源码：application/.../XxxAppService.java】
```
...（其他层类似）

> 🆕 v3.4.0 G-CODEPLAN-SRC：每条骨架强制附"已读源码"或"待核实源码"标记之一；待核实清单非空 → CodingPlan 视为草案，禁止进 ⑤ Coding。

**请您重点确认（逐条回复）：**
1. 14 条门禁全部通过，是否有你认为需要额外关注的？
2. CodingModel 11 维决策中，哪个维度的方案与你预期不符？
3. 风险 Task 的应对方案是否充分？
4. 关键类骨架是否与项目现有风格一致？
5. 🆕 每条骨架的【已读源码：】标记是否真实？有无凭推测设计、应标【待核实源码】却未标的？

**用户选项：**
- ✅ 确认 CodingPlan，无误
- ⚠️ 需要修改（说明修改内容，AI 记录后返回 TaskSkill 修补对应章节，再次过 14 条门禁，再次请用户确认）
- ❌ 暂停流程

> **强制要求：** 必须等待用户确认后才能进入 ⑤ Coding。模糊回复（如"好"/"行"/"可以"）需 AI 追问确认。

---

### ④bis CodingPlan 输出（🔴 强制 — 详细 7 章节已下沉）

> **📍 详细 7 章节 + 14 条门禁已下沉：** 原 AE-skill 中 ④bis 的 7 章节（文件顺序/类骨架/数据/Mapper SQL/测试对应/验证点/调试回滚）+ 14 条门禁 + 节点职责已统一存放在 [`coding-skill.md`](../phase2-coding/coding-skill.md) 和 [`be-coding-plan-template.md`](../../templates/coding/be-coding-plan-template.md)，本 SKILL 不再重复。

**AE 编排层 Phase ④→⑤ 调用协议（9 项前置条件，全部满足后才触发 `CodingSkill.Execute`）：**

| # | 前置条件 | 验证方式 |
|---|---------|---------|
| 1 | Task 0 ~ Task-N 全部已生成 | Task 文档目录中每个 Task 文件存在 |
| 2 | 每个 Task 均包含 `## CodingModel 决策记录`，11 维均有明确结论 | 逐 Task 文档检查该章节无空行 |
| 3 | 每个 Task 均包含 `## 任务级 CodePlan`（含类骨架 + 方法级逻辑 + DB 操作 + 外部依赖 + 测试映射） | 逐 Task 文档检查该章节子节齐全 |
| 4 | 全局 Task Review TR-1~TR-7 全部通过（连续 3 轮无新增问题） | Review 结论输出中 TR-1~TR-7 均显示 ✅ |
| 5 | 统一版 `{STORY-ID}-CodingPlan.md` 已生成，14 条门禁全部通过 | CodingPlan 门禁自检表全 ✅ |
| 5.5 | 统一版 CodingPlan 已通过 `documentStorage` 落地（`resolve_path + save_doc` 调用成功，G-DOC-STORAGE ✅）| `ae-sdd gates check --only G-DOC-STORAGE` ✅ |
| 6 | 统一版 CodingPlan 已获用户明确确认（"确认"/"同意"/"可以开始"） | 用户回复记录中有明确确认词 |
| 7 | 统一版 CodingPlan 中的 CodingModel 决策记录已复核（与各 Task 一致） | 无冲突项 |
| 8 | 🆕 v3.4.0 **G-CODEPLAN-SRC 源码核对通过**：新增/修改类建模范式已核对现有源码（待核实清单为空） | `ae-sdd gates check --only G-CODEPLAN-SRC` ✅ |
| 9 | 🆕 v3.4.0 **G-14 CodingPlan-Story 一致性通过**：Plan 引用 Story + AC 对齐 + 偏离项有 Proposal | `ae-sdd gates check --only G-14` ✅ |

> **任一前置条件未满足 → 禁止触发 `CodingSkill.Execute`。** AI 不得以"用户整体确认"等模糊信号绕过。

**触发：** `CodingSkill.Execute`（见 [coding-skill.md §CodingSkill 对外调用契约](../phase2-coding/coding-skill.md)）

---

### ⑤ Coding

**触发：** [Coding SKILL](../phase2-coding/coding-skill.md)

**输入：** Story + Task 文档 + **{STORY-ID}-CodingPlan.md** + 测试用例 + 约束 + 工作目录
**输出：** 可编译、可测试的代码 + Coding 报告

**一轮 Coding 的定义：**
一轮 Coding = 对一个或多个连续 Task 的完整编码实现（从开始到报告出具）。一轮可能包含多个 Task，也可能只有一个 Task。

**多轮 Coding 的常见场景：**

| 场景 | 说明 | 示例 |
|------|------|------|
| 0-1 实现 | 全新功能实现 | Task 1-3 全部新写 |
| 缺陷修复 | 修复测试/Review 发现的问题 | 第一轮发现 bug，第二轮修复 |
|增量补充 | 补充遗漏的功能点 | 发现漏了通知逻辑，补加 |
| 重构优化 | 不改功能，仅优化结构 | 抽取公共方法、拆解大事务 |

**一轮 Coding 的典型流程：**
```
确认实现方案 → 按 Task 顺序写代码 → 编译 → 测试 → 出 Coding 报告 → 出 CodeReview 报告
    │
    └── 失败 → 异常路径（记录问题 → 分析 → 修复 → 继续）
```

**多轮 Coding 的衔接规则：**
- 每一轮 Coding 完成后，出具该轮的 CodeReview 报告（独立版本）
- 后续轮次的 CodeReview 基于前序轮次累积评估（但每次报告独立存档）
- 多轮之间通过 `state.json` 中的 `codingRound` 字段追踪

**Coding 报告要求：**
编码完成后必须生成 `{story}-CodingReport-v{N}-r{M}.md`（v{N}=Story 版本号，r{M}=第 M 轮 Coding），包含以下内容：

1. **Story 任务概述**
   - Story ID 和标题
   - 核心功能描述
   - 实现的业务价值

2. **分层实现清单**
   - 按调用顺序分层列出，表格列固定为 `类型 / 文件路径 / 变更类型 / 说明`
   - **SPI 层**：跨服务接口、SPI DTO（如有）
   - **Domain 层**：聚合根、实体、值对象、领域服务、Facade 接口、Repository 接口
   - **Application 层**：AppService、Orchestrator、DTO/Command、Converter、事件处理器
   - **Infrastructure 层**：Facade 实现、Repository 实现、PO/DO、Mapper/DAO、外部服务集成
   - **Interfaces/BFF 层**：Controller、Request/Response、JobHandler、BFF 聚合入口
   - **Test 层**：单元测试、集成测试、测试资源
   - **文档/配置**：Nacos、YAML、DDL、Story/Coding 报告等（如有）

3. **关键业务逻辑说明**
   - 每个核心方法的业务逻辑描述
   - 状态机流转逻辑（如有）
   - 事务边界说明
   - 并发控制方案（如有）

4. **数据库变更**
   - 新增/修改的表结构
   - 新增/修改的索引
   - 数据迁移脚本（如有）

5. **外部依赖调用**
   - 调用的外部服务及接口
   - 调用时机和参数说明
   - 异常处理策略

6. **单元测试覆盖**
   - 测试类清单
   - 核心场景覆盖情况
   - Mock 策略说明

7. **开发问题记录**
   - 遇到的问题及解决方案（必须记录）
   - 技术债务说明（必须记录）
   - 待优化项（必须记录）

---

### 🔍 人工审核点 3：~~编码阶段完成确认~~（🗑️ 2026-06-10 删除）

> **删除原因：** 编码完成后的确认已合并到 Phase 3 ⑦ 阶段的 CodeReview 报告中（CodeReview 报告本身包含代码 walkthrough + ⑥bis 一致性核查 + ⑦bis 对称性闸），无需单独的"Coding 完成后"审核节点。重复审核浪费用户时间。
> 替代节点：见下面 `§🔍 人工审核点 4：CodeReview 阶段完成确认`（旧 §3 的内容已合并过去）。
---

## Phase 3：验证阶段（必须完成，不可跳过）

### 🔴 测试真实性强制规范（8 类禁止手段 + 证据链硬门禁 — 已下沉到 coding-skill）

> **📍 详细 8 类禁止伪造手段 + 证据链硬门禁已下沉：** 原 AE-skill 中 Phase 3 的"测试真实性强制规范"（包括 8 类禁止手段：@Disabled 隐藏失败 / assertTrue(true) 永真 / catch 吞噬异常 / 全 Mock 替代 / 期望值=实际值 / 无效测试数据 / Thread.sleep 绕过 / 凑覆盖率 + 原始日志 / Surefire-Failsafe XML / `test_authenticity_scan.py` / AC 对账 / 跳测参数扫描）已统一存放在 [`coding-skill.md` §📋 测试真实性强制规范](../phase2-coding/coding-skill.md)，本 SKILL 不再重复。
>
> **AE 编排层只关注 1 个门禁：**
> - ✅ ⑥ 完成判定前必须派 `test-verifier` sub-agent 独立跑一遍测试 + 解析 Surefire/Failsafe XML + 执行 `scripts/test_authenticity_scan.py` + 扫描 8 类禁止手段
> - ✅ 详见 coding-skill.md 对应章节
> - 🔴 **主 agent 自称"测试通过"无效，必须 test-verifier 独立验证不通过 = 不通过**

---

### ⑥ 完成判定（逐维量化验证，不可仅凭感觉）

| # | 条件 | 验证方式 | 通过标准 |
|---|------|---------|---------|
| 6.1 | `mvn compile` 通过 | 执行 `mvn compile` | 返回 BUILD SUCCESS，无 error |
| 6.2 | 服务启动成功 | 执行 `mvn spring-boot:run` | 日志含 `Started XxxApplication in X seconds`；端口实际监听（`curl localhost:port/actuator/health` 返回 UP）；无 `BeanCreationException`、`BeanNotOfRequiredTypeException` |
| 6.3 | 主流程接口 Pass | 执行 L2 真实 HTTP 测试（SpringBootTest RANDOM_PORT + TestRestTemplate）或 L3 集成测试 | 主流程接口 100% Pass（至少覆盖 Story 的 AC-001 场景）；返回 HTTP 200；响应结构与 Story 接口契约一致。🔴 能走真实 HTTP 的接口禁止仅用 MockMvc 验证（MockMvc 不走真实端口/网络），仅框架过老无法启动嵌入式容器时降级，并在测试报告注明 |
| 6.4 | 错误码映射正确 | 执行异常场景 L2 测试 | 业务异常 → 对应错误码（如 2104X）；系统异常 → 500 或对应兜底码；HTTP 状态码与 Story 定义一致 |
| 6.5 | DB 写操作落库 | 执行 L3 集成测试（真实 DB） | 验证：INSERT 后 `SELECT` 能查到；UPDATE 后字段值正确；事务提交后数据可见 |
| 6.6 | 事务边界正确 | 执行 L3 集成测试 | 事务内操作失败 → 全部回滚（数据未污染）；事务外操作（通知/消息）异步执行（不等返回） |
| 6.7 | 所有测试用例 Pass | 执行 `mvn test` | L1/L2/L3/L4 全部 Pass；无跳过（除非 Story 在"可跳过的测试"章节中明确标注了可跳过的测试 ID 和原因，且原因合理） |
| 6.8 | 异常路径无 Open 问题 | 检查开发问题记录 | 所有 Open 问题都有修复方案或已修复 |
| 6.9 | 测试报告已出具 | 检查 `{story}-Report.md` 存在 | 文件已生成且与代码仓库内容一致 |
| 6.10 🔴 | **测试真实性校验** | **执行 `🔴 测试真实性强制规范`：独立复跑测试、归档原始日志、解析 Surefire/Failsafe XML、运行 `scripts/test_authenticity_scan.py`、对账 AC × 测试方法** | **扫描 BLOCKER=0；XML failures/errors/skipped=0（skipped 需 Story 明确批准）；测试报告统计与 XML 一致；无跳测/忽略失败参数；关键测试代码已呈现；测试数据可追溯到 Story/Task；无未授权"修复测试"；AC 覆盖率 100%。任一未达成 → 测试报告作废，⑤ Coding 必须返工** |

**门禁说明：**
- 6.1-6.3：任意一项未通过 → 🔴 强制停止，必须修复才能继续
- 6.4-6.6：任意一项未通过 → 🟠 严重型缺陷，需修复后继续
- 6.7-6.10：任意一项未通过 → 修复后才能进入 CodeReview

**全部通过 → 进入 ⑥bis 全切面一致性核查闸（CodeReview 的硬前置，不可跳过）**
**未全部通过 → 回到 Phase 2 继续**

---

### ⑥bis 编码后全切面一致性核查闸（🔴 CodeReview 硬前置 — 已下沉到 coding-skill）

> **📍 详细 4 步核查 + 漂移判定已下沉：** 原 AE-skill 中 ⑥bis 的"全切面一致性核查表"（以代码为锚反向核查 DR / Story / Task / 测试用例 / 代码 五方一致 + 🔴 漂移判定规则）已统一存放在 [`coding-skill.md` §📋 ⑥bis 编码后全切面一致性核查闸](../phase2-coding/coding-skill.md)，本 SKILL 不再重复。
>
> **AE 编排层只关注 1 个门禁：**
> - ✅ ⑦ CodeReview 出具前必须完成 ⑥bis 闸
> - ✅ 输出《全切面一致性核查表》并嵌入 CodeReview 报告"零、"章节
> - 🔴 任一漂移项 = 阻断型 CodeReview 问题

---

### ⑦ CodeReview 报告出具

**触发时机：** ⑥ 完成判定全部通过后，人工审核之前

**输入：** Coding 报告 + 测试报告 + Story 文档 + DR 文档 + 约束规范 + 实际代码

**输出：** `{story}-CodeReview-v{N}-r{M}.md` — 架构师级审阅报告（多版本）

**多版本规则：**
- 一个 Story 可以有多轮 Coding，每轮出具一份独立的 CodeReview 报告
- 文件命名：`{STORY-ID}-CodeReview-v{N}-r{M}.md`
  - `v{N}` = Story 版本号（Story 文档变更后累加）
  - `r{M}` = Coding 轮次号（第 M 轮 Coding 的报告）
- 多轮之间，后一轮的 CodeReview 应在"八、本次提交 Git 文件清单"中标注"与 v{N}-r{M-1} 相比的变化"，不要求重复描述未变化的内容
- 每一轮报告都是独立的完整版本，可独立审阅；累积问题清单在最后一轮汇总

> **强制要求：AI 必须扫描实际代码仓库逐文件填写，禁止凭记忆或 Story 文档推断填写。每个章节的内容必须与实际代码一致。**

> **扫描范围：不仅扫描本 Story 新增的文件，还必须扫描被修改的现有文件，并检查其所有直接调用方是否受影响。扫描方式：使用 IDE 搜索或 grep 查找所有引用。使用 `grep -r "类名" --include="*.java"` 确认所有调用点。传递依赖不在强制扫描范围内，但若因传递依赖导致编译错误则必须处理。禁止只扫新增文件就下结论。**

---

## 七、CodeReview 报告模板

> **📍 模板已下沉到 `templates/`：** 完整的 9 章节 CodeReview 报告模板（含"零、全切面一致性核查表" / 一-九节 / 分层职责红线核查 / 产出物对账 / Git 文件清单等）已统一存放在 [`templates/coding/be-codereview-template.md`](../../templates/coding/be-codereview-template.md)，本 SKILL 不再重复维护副本。
>
> **使用规则：**
> 1. **生成 CodeReview 报告时** → 直接复制 `templates/coding/be-codereview-template.md`，按 Story 填充各章节
> 2. **流程门禁** → ⑦ CodeReview 出具前必须确认 ⑥bis 全切面一致性核查闸已通过、"零"章节已嵌入报告
> 3. **模板维护** → 模板本身如有更新（如新增核查维度），直接修改 `templates/coding/be-codereview-template.md`，**禁止在 AE-skill 中维护副本**（这是本次重构的核心原则，杜绝重复堆积）
>
> **AE 编排层只关注 4 个门禁：**
> - ✅ ⑥bis 全切面一致性核查闸已通过（详见第六章对应章节）
> - ✅ CodeReview 报告已按模板生成、"零"章节已嵌入
> - ✅ ⑦bis 全链路对称性核查闸已通过（见下）
> - ✅ 报告路径与产出物对账表 100% 一致

---

### ⑦bis 全链路对称性核查闸（🔴 流程收尾强制 — 已下沉到 coding-skill）

> **📍 详细 5 步核查流程已下沉：** 原 AE-skill 中 ⑦bis 的"全链路对称性追溯矩阵"（5 步核查：DR 章节 ↔ Story 章节 ↔ Task 章节 ↔ 代码文件:行号 ↔ 测试用例 ID）已统一存放在 [`coding-skill.md` §📋 ⑦bis 全链路对称性核查闸](../phase2-coding/coding-skill.md)，本 SKILL 不再重复。
>
> **AE 编排层只关注 1 个门禁：**
> - ✅ 人工审核点 4 前必须完成 ⑦bis 闸，5 层双向追溯无 🔴 断链（漏做/多做）
> - ✅ 输出《全链路对称性追溯矩阵》

---

### 🔍 人工审核点 4：CodeReview 阶段完成确认（🆕 2026-06-10 改名，原"验证阶段完成确认"）

**触发时机：** CodeReview 报告生成后，最终完成之前

**审核核心：** 以 CodeReview 报告为介质进行架构师级审阅

#### 📖 AI 主动讲解（Code 故事，🔴 强制）

> **本节点禁止直接问"请审阅 CodeReview 报告"。** AI 必须先用 walkthrough 的方式，把代码实现主动讲给用户听。详细规范见上文 `📖 人工审核主动讲解规范` 章节。

讲解模板参考上文 `③ CodeReview 阶段 —— 讲"代码实现故事"` 的输出模板，**必须覆盖**：
1. **代码实现故事**：本轮 Coding 实现了哪些 Task，调用链怎么走
2. **分层 walkthrough**：Domain/Application/Infrastructure/Interfaces 各层核心类 + 关键方法 + 代码位置（文件:行号）
3. **状态机实现**：canTransition() 实际代码、状态流转的判断条件在哪一行
4. **事务实现**：@Transactional 边界、传播行为、回滚规则
5. **异常处理**：异常类、错误码、HTTP 状态码的映射
6. **测试覆盖**：AC × 测试方法 的对应关系、覆盖率
7. **CodeReview 关键发现**：🔴🟠 问题的整改方案

**讲解形式要求：**
- 必须给具体文件:行号，不能只说"在 XxxService 里"
- 必须用代码片段或伪代码展示关键逻辑，不能只口头描述
- 必须主动指出"如果用户想深挖，建议看 {文件}:{行号}-{行号}"
- 用户追问"这段代码什么意思" → 视为讲解不充分，AI 必须自我反思并补充讲解

#### 📋 对话内直接呈现（🔴 讲解完成后必须执行，禁止以"请审阅报告"代替）

> AI 讲解完毕后，必须在对话中直接输出以下内容，用户无需打开 CodeReview 报告即可在对话中审核所有关键信息。

**必须直接输出在对话中的内容：**

```
📋 【{STORY-ID} CodeReview 审核 — 对话内呈现】

一、问题清单（🔴 阻断型 + 🟠 严重型，全部展开）
| 问题 # | 等级 | 文件:行号 | 问题描述 | 整改方案 | 状态 |
|--------|------|---------|---------|---------|------|
| CR-001 | 🔴   | XxxService.java:42 | {描述} | {方案} | 待整改 |
...（无阻断/严重型则写"本轮 CodeReview 无 🔴🟠 问题"）

二、AC × 测试覆盖对账
| AC 编号 | AC 描述 | 覆盖测试方法 | 覆盖方式 | 通过状态 |
|---------|--------|------------|---------|---------|
| AC-001  | {描述} | testXxx()   | L2 HTTP | ✅ Pass  |
...（每条 AC 逐行，无遗漏）

三、调用链摘要（从 BFF 到 DB）
BFF: {RestImpl}:{行号} → AppService: {方法}:{行号} → Domain: {方法}:{行号} → Repository: {方法}:{行号} → DB: {表名}

四、关键代码片段（每层核心方法，≤10行/片段）
[Domain 核心方法]
// {文件}:{起始行}
{代码片段}

[AppService 事务方法]
// {文件}:{起始行}
{代码片段}

五、测试结果快照
- 编译：{✅ 通过 / 🔴 失败，原因：...}
- 服务启动：{✅ 正常 / 🔴 失败，原因：...}
- 主流程接口：{HTTP 200 / 异常}
- 全量测试：{Pass N / Fail M / Skip K}
```

**请您重点确认（逐条回复）：**
1. 问题清单中的整改方案是否合理？有无需要额外讨论的？
2. AC 覆盖是否完整？有没有 AC 验证方式不够严格的？
3. 调用链是否符合预期？有没有分层违规的地方？
4. 关键代码片段是否符合团队约定？

**用户选项：**
- ✅ 确认工程完成
- ⚠️ 需要补充测试或修复问题（说明修改内容）
- 🔄 重新执行某个阶段
- ❌ 暂停流程

---

### ⑦ter 流程收尾合规自检（🔴 v3.5.2 — 不合规就修复，禁止裸 ✅ 收尾）

> **🆕 v3.5.2 新增（2026-06-27，用户需求"每次流程结束时先自检合规，不合规就修复"）：** 人工审核点 4 用户确认后、⑧完成输出前，AI 必须**自跑一遍流程收尾自检**，确保用户确认的内容与实际落地一致。**禁止"用户已确认就裸 ✅ 收尾"** —— 用户确认的是 CodeReview 内容，但产出物是否齐全、文档是否合规落位、state 是否推进、门禁是否全过，需要独立核对。

**触发时机：** 人工审核点 4 用户明确 ✅ 确认后、⑧完成输出之前。

**设计哲学（与⑥.10 测试真实性同源）：** 防止 AI "给自己打钩" —— 主 agent 声称"流程已完成"无效，必须以现有 CLI 的客观输出为证据。

#### 5 项自检维度（复用现有 CLI，不新建硬门禁）

| # | 维度 | 校验命令 | 通过标准 | 自愈策略 |
|---|------|---------|---------|---------|
| **7t-1** | 全门禁通过 | `ae-sdd gates check`（全量 28 门禁） | `all_pass == true`（JSON `failed==0`） | 🔴 逻辑性失败（如 G-08 内容校验）→ 阻断 + 升级用户；🟢 能补生成的（如 G-01 文档缺失）→ 补跑对应 SKILL 生成后重跑 |
| **7t-2** | 文档合规位置 | `ae-sdd gates check --only G-DOC-STORAGE` | `stray_files == []`（无游离产物） | 🟢 游离产物 → AI 调 `document-storage.resolve_path()` 重定位并移动到合规根目录，重跑本维度 |
| **7t-3** | state.json 完整 | `ae-sdd state read --json` | `phase` 在 PHASE_FLOW 合法值内 + `currentStory` 非空 + `events` 非空数组 | 🟢 phase 未推进到 `completed` → `ae-sdd state write --phase completed`；events 缺失 → 补 `append_event` 后 `state write` 落盘 |
| **7t-4** | 产出物齐全 | 逐项核 §⑧产出物表 8 类路径（Story/Supplement/Task/CodingReport/CodeReview/testcase/TestReport/源码） | 全部文件真实存在（`os.path.exists`，非仅路径字符串） | 🟢 缺失 → 补跑对应 SKILL 生成（如 CodingReport 缺→coding-report-skill）；🔴 同一产物多轮补不出 → 阻断 + 升级用户 |
| **7t-5** | 无遗留 🔴 问题 | 读本轮 CodeReview 报告扫 🔴 阻断型 | 本轮 CodeReview 报告无 Open 态 🔴 问题 | 🔴 有未整改 🔴 → 阻断，回 ⑤ Coding 返工（不得带病收尾） |

#### 自检 SOP（强制 4 步，不可跳过）

```
进入 ⑦ter 自检
    │
    ├── 1. AI 跑 7t-1 ~ 7t-5 全部命令，汇总成《流程收尾自检表》
    │     （必须在对话内直接呈现，表格形式，不省略任何维度）
    │
    ├── 2. 判定：
    │     ├─ 全部 ✅ → 进 ⑧完成输出
    │     ├─ 有 🟢 可自愈项 → 进步骤 3
    │     └─ 有 🔴 阻断项 → 进步骤 4
    │
    ├── 3. 自愈循环：
    │     a. 按"自愈策略"列逐项修复
    │     b. 修复后【重跑该维度自检】（不是重跑全部，只重跑修复项）
    │     c. 重跑通过 → 该维度转 ✅；重跑仍失败 → 升级为 🔴 阻断
    │     d. 全部 🟢 转 ✅ → 进 ⑧；任一升 🔴 → 进步骤 4
    │
    └── 4. 🔴 阻断兜底：
          a. 暂停流程，列出全部 🔴 阻断项清单（维度 + 失败原因 + 证据）
          b. 升级用户决策（返工 / 豁免标注"事后回溯" / 暂停）
          c. 【禁止裸 ✅ 收尾】不得跳过自检直接进 ⑧
```

#### 《流程收尾自检表》对话内呈现模板（🔴 必须输出）

```
📋 【{STORY-ID} 流程收尾自检表 — v3.5.2】

| 维度 | 命令 | 结果 | 状态 | 自愈动作 |
|------|------|------|------|---------|
| 7t-1 全门禁 | gates check | failed={N} | ✅/🔴 | {无 / 已补 G-XX} |
| 7t-2 文档位置 | gates check --only G-DOC-STORAGE | stray={N} | ✅/🟢/🔴 | {无 / 已移位 X 文件} |
| 7t-3 state 完整 | state read --json | phase={X} | ✅/🟢/🔴 | {无 / 已推进 phase} |
| 7t-4 产出物齐全 | 逐项核 §⑧表 | 缺失={N} | ✅/🟢/🔴 | {无 / 已补生成 X} |
| 7t-5 无遗留🔴 | 读 CodeReview | open🔴={N} | ✅/🔴 | {无 / 回⑤返工} |

自检结论：{全部 ✅ 可收尾 / N 项自愈中 / N 项 🔴 阻断已升级用户}
```

#### AE 编排层门禁（本 SKILL 只关注 2 条）

- ✅ ⑧完成输出前必须完成 ⑦ter 自检，自检表已对话呈现
- ✅ 任一 🔴 阻断项未自愈 → 禁止进 ⑧完成输出（违规收尾视为事后回溯）

> **为何不做成独立 CLI 硬门禁？** 按用户选定路线（SKILL 文字 + 复用现有 CLI）+ ae-sdd "规则描述 + 工具执行"双轨设计：7t-1/2/3 直接复用 `gates check` / `state read`（已有硬阻断 exit code），7t-4/5 是编排层"清单齐全性"逻辑（无现成单命令），由 AI 按本编排文字执行。符合 [`ae-sdd-update-skill.md` §SKILL 边界判定表](../orchestration/ae-sdd-update-skill.md)："流程节点定义 / 整体执行清单 → ae-sdd-skill"。

---

### ⑧ 完成输出

工程完成后输出（最终版，即最后一轮的产出物路径）：

| 产出物 | 路径 |
|--------|------|
| Story 文档（稳定版） | `ae-sdd-doc/iterations/{date}/Story/{story}.md`（最终版） |
| Story 补充说明 | `ae-sdd-doc/iterations/{date}/Story/{story}-Supplement.md` |
| Task 文档 | `ae-sdd-doc/iterations/{date}/Task/{story}/` |
| **Coding 报告** | `ae-sdd-doc/iterations/{date}/Coding/{story}/{story}-CodingReport-v{N}-r{M}.md`（最后一轮） |
| **CodeReview 报告** | `ae-sdd-doc/iterations/{date}/CR/{story}/{story}-CodeReview-v{N}-r{M}.md`（最后一轮） |
| 测试用例文档 | `ae-sdd-doc/iterations/{date}/Test/{story}/{story}-testcase.md` |
| **测试报告** | `ae-sdd-doc/iterations/{date}/Test/{story}/{story}-Report-v{N}-r{M}.md`（最后一轮） |
| 源代码 | 工作目录下对应工程 |
| 开发问题记录 | `ae-sdd-doc/iterations/{date}/Coding/{story}/{story}-开发问题记录.md` |

> **路径说明：** 以上路径均通过 `documentStorage.resolve_path(intent, storyId)` 动态定位，`{date}` = 当前迭代日期。实际存储路径以 document-storage-skill 返回值为准，禁止硬编码。

> **多轮存档说明：** 每一轮 Coding 的报告（v{N}-r{1}、v{N}-r{2}...）均独立存档，不覆盖。目录 `ae-sdd-doc/iterations/{date}/Coding/{story}/` 下存放该 Story 所有轮次的报告文件。

---

## 异常处理

### 总原则：DR-Story-Task 设计一致性

```
DR（需求文档）→ Story → Task → Coding
     ↑
     └── 一切以 DR 为准。DR 是设计链路的源头基准，Story 是 DR 的细化，
         Task 是 Story 的实现映射。设计链路必须保持一致。
```

**核心原则（强制，不可违背）：**
- DR 是设计的基准，任何层级的修改最终都要与 DR 对齐
- Coding 发现 DR 有缺陷/不合逻辑 → 必须修改 DR，不迁就
- 修改 DR 后 → 必须触发 Story Review，验证 Story 与 DR 的一致性
- 禁止为了绕过 DR 问题而扭曲 Story 或 Task 的描述
- 问题必须在发生的层级解决，不允许用上层弥补下层缺陷

### Coding 问题分层排查与修改链

> **📍 详细 4 层排查流程已下沉到 coding-skill.md：** 原 AE-skill 中"Coding 问题分层排查与修改链"（含 ASCII 流程图 + 判定标准表 + 修改影响范围表 + 关键原则）已统一存放在 [`coding-skill.md` §📋 Coding 问题分层排查与修改链](../phase2-coding/coding-skill.md)，与 coding-skill.md 自身的"异常路径 A1-A4"整合。
>
> **AE 编排层只关注分层原则：**
> - 严格在问题发生的层级解决，禁止跨越层级处理
> - Task 问题与 Story 无关 → 直接改 Task，不动 Story
> - Story 问题与 DR 无关 → 直接改 Story，不动 DR
> - 只有确认 DR 本身有缺陷时，才走完整链路
> - 详细排查步骤 / 判定标准 / 修改影响范围 → 见 coding-skill.md 对应章节

---

### 人工介入节点

| 节点 | 触发条件 | 询问内容 |
|------|---------|---------|
| **Phase 1 → Phase 2** | 测试用例生成完成 | "设计阶段已完成，是否进入实现阶段？" |
| **实现方案预确认** | Phase 1 完成后，生成 Task 文档之前 | "请确认核心业务理解、接口依赖、分层思路、并发策略、异常处理，无误后生成 Task 文档" |
| **Task 生成后** | Task 文档生成完成 | "Task 文档已生成，是否开始编码？" |
| **Task 实现方案确认** | 每个 Task 开始写代码之前 | "请确认本 Task 的类/方法/核心逻辑/DB 操作，实现方案无误后开始编码" |
| **Phase 2 → Phase 3** | ~~所有 Task 编码完成（2026-06-10 删除）~~ | ~~"编码阶段已完成，是否进入验证阶段？"~~ |
| **🆕 CodingPlan 评审**（2026-06-10） | 统一版 CodingPlan 生成后，Coding 之前 | "CodingPlan 已生成，14 条门禁全过，请审阅并确认是否开始编码" |
| **Phase 3 完成** | 所有测试通过 + CodeReview 报告出具 | "CodeReview 阶段完成（含 ⑥bis 一致性核查 + ⑦bis 对称性闸），是否确认工程完成？" |
| 需人工裁定的缺陷 | 合理性判定为 ⚠️ | "请人工判定此项是否为缺陷？" |
| DR 变更影响多个 Story | DR Update 评估 | "是否同步修改受影响 Story？" |

---

## 子 SKILL 索引

| SKILL | 文件 | 职责 |
|-------|------|------|
| **🆕 Requirement Analysis** | [requirement-analysis-skill.md](../phase1-design/requirement-analysis-skill.md) | **需求分析 SKILL — Phase 1 起点。从 PRD/Issue/对话需求生成 RA 文档 + 规模裁定 + 路由决策** |
| **🆕 DR Generate** | [dr-generate-skill.md](../phase1-design/dr-generate-skill.md) | **DR 生成 SKILL — 从 RA 文档生成 DR 草稿（规模=大 时触发）** |
| **🆕 DR Review** | [dr-review-skill.md](../phase1-design/dr-review-skill.md) | **DR Review SKILL — 对 DR 草稿进行 5 阶段评审** |
| Story Generate | [story-generate-skill.md](../phase1-design/story-generate-skill.md) | DR → Story 生成（7 阶段挖掘 SOP） |
| Story Review | [story-review-skill.md](../phase1-design/story-review-skill.md) | Story 缺陷挖掘循环 |
| TestCase Generate | [testcase-generate-skill.md](../phase1-design/testcase-generate-skill.md) | 测试用例生成（全场景覆盖 + 合规性校验） |
| Story Update | [story-update-skill.md](../phase1-design/story-update-skill.md) | Story 文档更新 |
| Task Generate | [task-generate-skill.md](../phase2-task/task-generate-skill.md) | Task 文档生成与更新 + 全局 Task Review（结合约束+Story+测试用例审查所有 Task，闭环修复）|
| Coding | [coding-skill.md](../phase2-coding/coding-skill.md) | 代码实现 + 问题反馈 |
| DR Update | [dr-update-skill.md](../phase1-design/dr-update-skill.md) | DR 文档更新 |
| **🆕 Project Assets Update** | [project-assets-update-skill.md](../cross-cutting/project-assets-update-skill.md) | **项目资产生成/更新/审计 — G-00 门卫的 SOP** |
| **🆕 ae-sdd Install** | [ae-sdd-install-skill.md](../orchestration/ae-sdd-install-skill.md) | **安装引导 SKILL — 平台检测 → 选模式 → 执行 install → 写 hooks → 验证** |
| **🆕 Document Storage** | [document-storage-skill.md](../cross-cutting/document-storage-skill.md) | **文档存放路径解析、命名规则、重入判定** |
| **🆕 Proposal** | [proposal-skill.md](../cross-cutting/proposal-skill.md) | **跨域改动 / 缺陷修复的 4 段 Proposal SOP** |
| **🆕 Review Loop** | [review-loop-skill.md](../cross-cutting/review-loop-skill.md) | **🆕 v3.4.3 Review Loop 公共协议 — 所有 review 节点的 loop 骨架（退出条件 3 轮 + 循环上限 3 轮 + Plan-first）** |
| **🆕 Agent Orchestration** | [agent-orchestration-skill.md](../cross-cutting/agent-orchestration-skill.md) | **多 Agent 编排、SubAgent 派活协议、报告回传** |
| **🆕 ae-sdd Update** | [ae-sdd-update-skill.md](../orchestration/ae-sdd-update-skill.md) | **ae-sdd 自身维护 SKILL（修改 SKILL/SOP 时的入口）** |

---

## 🛠️ 工具 API 速查（ae-sdd CLI 子命令）

> **🔧 维护说明（🆕 v3.5.4 修订）：** ae-sdd 配套可执行 CLI 工具（Python，源码 `tools/bin/ae-sdd` + `tools/lib/*.py`）。**规则散在 SKILL.md，工具在 `tools/bin/ae-sdd`，一致性由 `ae-sdd update-check`（UC-01~07）+ 周期性 `ae-sdd iteration-check` 保证**（v3.5.4 新增，见 [`ae-sdd-update-skill.md` §设计-实现一致性迭代检查](../orchestration/ae-sdd-update-skill.md)）。

### 命令清单（按功能分组）

| 分组 | 命令 | 用途 |
|------|------|------|
| **资产**（G-00） | `ae-sdd gates check --only G-00` | 项目资产门卫（资产不存在时由 AI Agent 路由到 `project-assets-update-skill §3` 生成）|
| | `ae-sdd assets read/outline/section/query/stats` | 资产索引按需读取（ES 倒排索引 + BM25）|
| **状态机** | `ae-sdd state read / write / next-step / confirm` | 状态读取 / phase 切换 / 下一步判定 / 审核点 token |
| | `ae-sdd state prd-check-complete / prd-complete / prd-archive` | PRD 级 4 层 AND 校验 / compact 触发 / 归档（🆕 v3.3.0）|
| **路由** | `ae-sdd classify` | 4 维判定（来源/规模/产物/项目类型）|
| **门禁** | `ae-sdd gates check [--only <G-XX>]` | 28 门禁扫描（全量或单门禁）|
| | `ae-sdd gate ra-required / coding-required / doc-storage` | RA 准入 / Coding 真实性 / 文档存放单点校验 |
| | `ae-sdd flow-violation-scan [--root <dir>]` | 🆕 v3.5.6 RA 流程违规审计（8 条规则扫描 RA 文档合规性）|
| | `ae-sdd ra-depth-scan [--root <dir>]` | 🆕 v3.5.9 RA 机械派生深度扫描（D1-D5，验证 E.5/G.5/H.6/H.5 是否真做了）|
| | `ae-sdd enter` | 入口凭证（entry token，关卡1）|
| **Toolset Layer**（v3.2.3） | `ae-sdd memory enter/write/exit/read/search/promote/summarize` | Phase-aware 强制 memory gate |
| | `ae-sdd db profiles/query/explain/audit` | 本地 profile DB 证据（read-first）|
| | `ae-sdd git status/diff/log/blame/impact` | 只读 Git 证据 + 影响分析 |
| **维护** | `ae-sdd health` | 9 项健康度自检 |
| | `ae-sdd update-check` | UC-01~07 更新依赖图谱检查（dev-sync 前必跑）|
| | `ae-sdd iteration-check` | 🆕 v3.5.4 设计-实现一致性迭代检查（周期性深度体检）|
| | `ae-sdd context-pressure [--story <ID>]` | 🆕 v3.5.5 节点级上下文压力软提示（6 个审核点边界必调，report-only 不阻断）|
| | `ae-sdd version / bump / init` | 版本号 / 三处同步 / 项目实例化 |
| | `ae-sdd plugin list/validate/trace/init` | 🆕 v3.5.1 三层 SKILL 注册表管理 |
| | `ae-sdd runtime compact` | runtime-specific compact 适配层（v3.3.0）|

> **🔴 命令契约权威源：** 完整命令签名见 `tools/bin/ae-sdd`（argparse 注册表）；`source/standards/update-graph.json` 是 UC-03/UC-06 命令契约一致性检查的权威源。新增/修改 CLI 命令须同步 update-graph.json，否则 `update-check` 报 warn。

### AI 调用约定

- **CLI 命令必须可执行**——不能"我打算跑 ae-sdd classify"但实际不跑
- **G-00 由 AI Agent 手动调用**——Agent 在路由步骤 0 跑 `ae-sdd gates check --only G-00` 验证；G-00 不通过时路由到 `project-assets-update-skill §3` 生成
- **输出必须可解析**——CLI 走 stdout 输出 JSON，stderr 写日志，pipeline 友好

---

## 🔧 维护规则与同步机制

> **🔧 v3.5.4 修订：** ae-sdd 采用**"规则描述（SKILL.md 文字）+ 工具执行（Python CLI）"双轨**设计。规则层（SSOT）是 `source/SKILL.md` + 子 SKILL + `source/standards/`；工具层（派生）是 `tools/bin/ae-sdd` + `tools/lib/*.py`。

### 修改工作流

```
[修改 source/SKILL.md 或子 SKILL 规则]
         │
         ▼
[运行 ae-sdd update-check]  ← UC-01~07，dev-sync 前必跑全绿
         │
         ├─→ 版本号三处一致？/ 门禁注册一致？/ 命令契约闭环？
         │   扫描器分发？/ 健康度清单覆盖？/ 文档-实现一致性？
         │
         ├─ 不绿 → 按 UC 提示修复
         │
         └─ 绿 → 跑 scripts/dev-sync.sh 分发到 ~/.claude/skills/
                  + post-commit hook 自动触发（v3.4.0 分发闭环）
```

### 周期性深度体检（🆕 v3.5.4）

`ae-sdd update-check` 是"改完即跑"的快速防线；**`ae-sdd iteration-check`** 是周期性深度体检，补 UC 查不到的 4 类盲区（HS 物理实现齐全度 / 幽灵命令整段描述 / F-1 覆盖面 / 已实现未接入）。每月或重大变更后跑，详见 [`ae-sdd-update-skill.md` §设计-实现一致性迭代检查](../orchestration/ae-sdd-update-skill.md)。

### ae-sdd 自身维护（改 SKILL 本身）

> **触发词：** "修改 SKILL" / "更新 SKILL" / "新增 SKILL" / "重构 SKILL" / "SKILL 边界" / "SKILL 维护" / "优化 ae-sdd" / "改 ae-sdd"

进入本 SKILL 后会**短路**路由到 [`ae-sdd-update-skill.md`](../orchestration/ae-sdd-update-skill.md)，按其 5 步流程执行（评估范围 → 备份 → 实施 → 验证 → CHANGELOG）。

### 5. 版本管理

- **本文件 (`source/SKILL.md`) = ae-sdd 母版（SSOT）**。任何项目实例（`~/.claude/skills/ae-sdd/`）都从母版构建。
- **母版更新 → 跑 `bash scripts/build-dist.sh` → 生成 `dist/ae-sdd/` 实例化分发包 → 装到本地 Claude 或发布到 GitHub release。**
- **CHANGELOG 每次发版必更新**，格式见 `source/CHANGELOG/` 目录。
- **🆕 v3.2 更新闭环门禁：** 任何 `source/` 或 `tools/` 改动后，**dev-sync / build-dist 前必须跑 `ae-sdd update-check` 全绿**（error 级 0 failed）。改了文件后先查连带项：`ae-sdd update-check --affected <改动文件>`，再跑全量 `ae-sdd update-check` 兜底。详见 [`ae-sdd-update-skill.md` §更新依赖图谱](ae-sdd-update-skill.md)（权威源 `source/standards/update-graph.json`）。

### 6. 实例化机制（🆕 v3.0 三层架构）

> **Layer 1: 母版（SSOT）** = `source/`（开发者编辑这里，git 跟踪）
> **Layer 2: 实例化分发包** = `dist/ae-sdd/`（`bash scripts/build-dist.sh` 构建产物，git ignored）
> **Layer 3: 用户安装** = `~/.claude/skills/ae-sdd/`（由 `bash scripts/install.sh` 从 dist 装入，Claude Code 实际加载）
> **Layer 4: 项目实例** = `<project>/.ae-sdd/`（具体项目，引用 + overrides 模式）

**项目实例化命令：**

```bash
ae-sdd init <project-dir> <project-key>
  # 在 <project-dir>/.ae-sdd/ 创建：
  #   config.yaml       指向母版
  #   state.json        空模板
  #   assets/           引用母版 assets/{projectKey}/
  #   overrides/        空目录（项目特定 rules/ + tools/）
  # 不复制 rules/ tools/，只引用
  # fork（完整复制）是显式 opt-in：ae-sdd fork <project-dir>
```

**Override 解析：实例有效规则 = 母版 defaults + overrides/（同名覆盖）**

---

## 禁止事项（强制，违者必须整改）

| 禁止（违者） | 正确做法（强制） |
|------|------|
| 跳过 Phase 1 直接写代码 | 必须先完成设计阶段 |
| 跳过测试用例生成 | Story 稳定后必须生成测试用例 |
| 测试未全部通过就报告完成 | 完成标准是全部 Pass |
| 跳过测试报告 | 每次测试都要出报告 |
| **跳过 Coding 报告** | **编码完成后必须生成 Coding 报告** |
| **跳过 CodeReview 报告** | **验证阶段必须出具架构师级 CodeReview 报告** |
| **跳过实现方案预确认** | **Phase 2 开始前必须向用户呈现实现思路并获确认** |
| **跳过 Task 实现方案确认** | **每个 Task 开始前必须向用户呈现实现方案并获确认。用户必须明确说"确认"、"同意"、"可以开始"，模糊回复（如"好"、"行"、"看看"）需要 AI 追问确认，未获明确确认前禁止写代码** |
| 人工节点自动决策 | 待讨论项必须询问用户 |
| **跳过阶段审核** | **每个阶段完成后必须经过人工审核确认** |
| **未经确认进入下一阶段** | **必须等待用户确认后才能继续** |
| **Task 审核一锅端（一次性抛出全部 Task 文档让用户"整体确认"）** | **🔴 必须逐文件自上而下核对，每个文件单独获 ✅ 后才进入下一文件** |
| **🔴 跳过人工审核的主动讲解（只丢文档不讲解）** | **🔴 三个审核节点（① 设计阶段 ② Task 文档 ④ CodeReview）必须先讲"故事"再问确认。详见 `📖 人工审核主动讲解规范` 章节** |
| **🔴 跳过 CodingPlan 直接写代码** | **🔴 ⑤ Coding 前必须有 `{STORY-ID}-CodingPlan.md`，含 7 个章节（文件顺序/类骨架/数据/Mapper SQL/测试对应/验证点/调试回滚）。详见 ④bis 章节** |
| **🔴 测试伪造通过** | **🔴 8 类禁止手段任一命中 = 测试无效：@Disabled 隐藏失败 / assertTrue(true) 永真 / catch 吞噬异常 / 全 Mock 替代 / 期望值=实际值 / 无效测试数据 / Thread.sleep 绕过 / 凑覆盖率。详见 `🔴 测试真实性强制规范` 章节** |
| **🔴 "修复测试"代替"修复代码"** | **🔴 AI 自行修改已审核通过的测试代码 = 伪造测试。必须标注修改原因 + 获得用户确认。未确认的修改视为伪造** |

---

## 执行清单（逐项执行，禁止跳过）

> **强制要求：AI 启动本 SKILL 时，必须用 TodoWrite 1:1 映射此表，即：执行清单的每一行对应一个 TodoWrite 项，动作内容 = 该行的"动作"列，状态 = 进行中/已完成。未满足门禁不得继续，不得自行降级处理。**

| # | 动作 | 产出物 | 门禁 |
|---|------|--------|------|
| **0** | **🆕 工作区与项目资产检查**（SKILL 启动时最先执行）| — | **projectKey 已知 + 项目资产已存在（或已完成生成并用户确认）**；任一未满足禁止进入后续步骤 |
| 0a | 确认工作区（projectKey / gitPath）| — | 用户明确告知或当前 session 已知 |
| 0b | 调用 `get_assets(projectKey)` 检查项目资产 | — | 资产存在 → 静默通过；资产不存在 → 进入 0c |
| 0c | **（资产缺失时）** 明确告知用户，调用 `project-assets-update-skill.md §3 生成动作` | `.ae-sdd/assets/{workspaceKey}/{workspaceKey}.assets.md`（见 document-storage §2.3） | 生成完成后 AI 报告摘要（微服务数/分层/技术栈）；用户确认资产内容准确 |
| 1 | 收集输入（DR 路径 + Story ID + 工作目录） | — | 三项信息已确认 |
| 1a | **🤖 多 Agent 模式决策**（在 Step 2 之前，可选） | `state.json` 中 `multiAgentMode: true/false` | 检测到"何时启用"表任一条件时主动提议；用户明确同意后启用；启用后从角色库选 sub-agent；⑥.10 测试真实性强制派 `test-verifier` 独立验证 |
| **1.5** | **🆕 自更新识别**（AE SKILL 自身维护，命中即短路） | — | 命中 `ae-sdd-update-skill.md`；未命中进入 1.6 |
| **1.6** | **🆕 来源识别**（2026-06-17 4 维判定维度 1） | — | 来源=PRD/Issue/对话 → `requirement-analysis-skill.md`；BUG/配置类 → `coding-skill.md`；无输入 → 引导用户 |
| **1.7** | **🆕 规模识别**（2026-06-17 4 维判定维度 2） | — | 已有规模结果 → 按规模路由（dr-generate / story-generate / task-generate / coding）；无规模结果 → fallback 套 Story 7 区模板（保留旧 4 类需求判定）|
| 2 | 生成 Story（DRtoStory SKILL） | `{story}.md` | 文件已生成或已存在 |
| 2b | **🔴 前端视角接口审视** | Story 文档追加"前端接口契约"章节（含 6 个维度：契约完整性/调用流程/状态展示/错误码/边界场景/联调支持） | 6 个维度门禁全部通过；至少 1 个完整请求+响应示例；不确定项已标注"需前端确认"；未通过禁止进入 Story Review |
| 3 | Story Review（Story Review SKILL，含 F-Stage 前端契约 Review） | `{story}-Supplement.md` + `{STORY-ID}-StoryReviewUpdatePlan-r{轮次}.md`（有确认缺陷时） | Review 循环已退出；每轮确认缺陷均先出 Plan 再修改 Story；F-Stage 6 项检查全部通过 |
| 4 | 生成测试用例（TestCase Generate SKILL） | `{story}-testcase.md` | 文件已生成 + 合规性校验通过 |
| 4-📖 | **🔴 AI 主动讲解 Story 故事**（人工审核点 1 之前） | Story 故事讲解输出 | 已讲清"业务背景/核心流程/关键设计决策/AC 故事/已识别风险"5 个维度；未讲解禁止进入 Phase 1 → Phase 2 的人工审核点 1 |
| 4b | **实现方案预确认** | 用户确认记录 | 用户已确认实现方案（核心业务/接口/分层/并发/异常） |
| 5 | 生成 Task（Task Generate SKILL） | `task/{story}-task-*.md` + `{STORY-ID}-Task实现方案.md` | 全部 Task 文件已生成 |
| 5a | **全局 Task Review（结合约束+Story+测试用例）** | Review 结论 | TR-1~TR-7 全部通过；发现问题 → Task 修复 → 重新 Review；连续 3 轮无新增问题才退出（3 轮仍有 🔴 → 升级用户），然后输出实现方案 |
| 5b | 人工审核：Task 文档 + 实现方案完成确认（🔴 强制逐文件自上而下核对，禁止一锅端确认） | 用户逐文件确认记录（每个 Task 文件 + 实现方案 + CodingPlan 各 1 条） | 每个文件单独获得用户 ✅；模糊回复追问后再判定；跳过/整体确认视为违规 |
| 5b-📖 | **🔴 AI 主动讲解 Task 故事**（在 5b 之前） | Task 故事讲解输出 | 已讲清"拆 Task 故事/依赖链路/DB 变更/事务边界/风险 Task"5 个维度；每 Task 文件核对前已讲"本 Task 故事"；未讲解禁止进入 5b |
| 5c | **🔴 CodingPlan 输出**（④bis，⑤ 之前） | `{STORY-ID}-CodingPlan.md` | 7 章节齐全；**14 条门禁全部通过**（含 CodingModel 决策记录完整 + 核心链路保护 + 资源隔离 + 混合压测）；🆕 v3.4.0 G-CODEPLAN-SRC 源码核对通过 + G-14 Story 一致性通过；Phase ④→⑤ 调用协议 9 项前置条件全满足；未通过禁止触发 `CodingSkill.Execute` |
| **5b.5** | **🆕 2026-06-10 人工审核：CodingPlan 评审**（在 5c 之后，⑤ 之前） | 用户确认记录 | 用户已确认 CodingPlan；模糊回复追问后再判定；未确认禁止进入 ⑤ Coding |
| 5c-🚫 | **🔴 测试真实性扫描**（在 6.7 之前，⑥ 完成判定硬前置） | 原始日志 + XML 对账 + `test-authenticity-scan` 报告 | `test_authenticity_scan.py` BLOCKER=0；Surefire/Failsafe XML 与报告统计一致；无跳测/忽略失败参数；关键测试代码已呈现；测试数据可追溯；无未授权"修复测试"；AC 覆盖率 100%。任一未达成 → 测试报告作废，⑤ Coding 返工 |
| 6 | Coding（Coding SKILL） | 源代码 + 开发问题记录 | 编译通过 + 服务启动成功 + 接口测试 Pass + 测试全部 Pass |
| 7 | 出具测试报告 | `{story}-Report.md` | 文件已生成 |
| 8 | 出具 Coding 报告 | `{story}-Coding-Report.md` | 文件已生成 |
| 8a | **⑥bis 编码后全切面一致性核查闸** | 《全切面一致性核查表》（嵌入 CodeReview 报告"零、"章节） | 🔴 以代码为锚反向核查全章节五方一致；无 🔴 漂移；核心落库路径有真实 DB 证据；未达标禁止进入 8b |
| 8b | 出具 CodeReview 报告 | `{story}-CodeReview.md` | 报告结构完整，含"零、全切面一致性核查表"，无阻断型问题 |
| 8c | **⑦bis 全链路对称性核查闸** | 《全链路对称性追溯矩阵》 | 🔴 DR-Story-Task-实现-测试用例 五层双向追溯一一对应；无 🔴 断链（漏做/多做）；未达标禁止进入人工审核 |
| 9 | 完成判定 | — | 全部条件 ✅ |
| 9-📖 | **🔴 AI 主动讲解 Code 故事**（在 10 之前） | Code 故事讲解输出 | 已讲清"调用链 walkthrough/分层实现/状态机/事务/异常/测试覆盖/CodeReview 发现"7 个维度；含具体文件:行号和代码片段；未讲解禁止进入 10 |
| 10 | 人工审核确认 | — | 用户确认工程完成 |
| 10a | **🔴 ⑦ter 流程收尾合规自检**（v3.5.2，在 10 之后、⑧之前） | 《流程收尾自检表》 | 5 维度全 ✅（7t-1 全门禁 / 7t-2 文档位置 / 7t-3 state 完整 / 7t-4 产出物齐全 / 7t-5 无遗留 🔴）；🟢 可自愈项已修复并重跑通过；🔴 阻断项已自愈或已升级用户；未自检 / 裸 ✅ 收尾禁止进 ⑧ |





