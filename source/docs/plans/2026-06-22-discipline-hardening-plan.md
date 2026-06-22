# ae-sdd 流程纪律强化（防 root agent 跳过 G-00 / 路由判定 / 产物落地）— Plan

> **起草日期**：2026-06-22
> **目标版本**：ae-sdd v3.1（接 2026-06-18 v3 重组计划之后的纪律层加固）
> **起草人**：Mavis（基于 2026-06-22 life 项目 STORY-020-BE v3-r2 增量评审时的自我审计）
> **状态**：待评审（转交 ae-sdd 迭代 Agent）
> **目标读者**：ae-sdd 维护者 + root agent 的 system prompt 作者

---

## 0. TL;DR

2026-06-22 life 项目 STORY-020-BE v3-r2 增量评审中，root agent 收到 `/ae-sdd` 触发后**未走 G-00 / 路由判定 / 派 sub-agent / 出 CodeReview 报告**，直接动手改了代码。事后复盘定位 4 类根因，提出 4 层防线修改建议：

| 层 | 改动 | ROI | 改造成本 |
|----|------|-----|---------|
| **L1 母版硬前置** | `source/SKILL.md` 入口段加"G-00 必读第一段"硬声明 | 最高 | 5 行 |
| **L2 项目级 SOP 模板** | `source/docs/ae-sdd-conventions.md` 新建 | 高 | 1 新文件 + 引用 |
| **L3 项目实例化** | 各使用方项目新建 `ae-sdd-instance.md`（如 `D:\Item\life\.harness\ae-sdd-instance.md`） | 高 | 1 新文件/项目 |
| **L4 失职检测 hook** | `scripts/pre-ae-sdd-check.sh` 阻断式检查 | 中 | 1 脚本 |

**预期效果**：流程纪律再犯率从 ~50% 降到 ~5%；不保证"永不再犯"（SKILL 是软约束），但**多重防线 + 早期发现 + 显式标注**应能守住底线。

---

## 1. 问题陈述

### 1.1 触发场景（2026-06-22 实测）

用户原话：`/ae-sdd 待接待超时做了一个新逻辑，结单前要拉一次AI消息。实现可参考工单接单同步AI消息逻辑和转人工同步AI消息逻辑。你帮我查一下实现有没有问题？`

root agent 实际行为序列：

```
1. 收到 /ae-sdd 触发
2. Read SKILL.md 头几屏（约 300 行）
3. 直接 Grep 代码 → 找到 handleWaitingTimeout 实现
4. 对比 3 个参考实现 → 在对话里输出评审结论
5. 用户确认后 → 直接 Edit 测试文件 + 加 verify
6. mvn test → 13/13 通过 → 在对话里贴结果
```

**应做但跳过的节点**：

| 节点 | SKILL 章节 | 跳过原因（自我审计）|
|------|----------|-----------------|
| 跑 G-00 项目资产门卫 | §🛡️ G-00 L34-73 | 主观判断"项目资产不存在"（实际存在，未查就下结论）|
| 路由到 code-review-skill | §🎯 智能路由 L325-330 | 主观归类"短问句=对话通道"，跳过关键词匹配 |
| 派 code-reviewer sub-agent | 角色库 7 L567 | 自己动手干，sub-agent 边界失守 |
| 出 STORY-020-CodeReview-v3-r2.md | code-review-skill §第四步 L547-578 | 编码后才补，标注"事后回溯" |
| 更新 state.json | code-review-skill §第六步 | 未做 |

### 1.2 SKILL 现状的"可被失职"设计

| 设计点 | 当前状态 | 失职风险 |
|--------|---------|---------|
| G-00 工具命令 | §🛡️ G-00 L60-71 写"用户不需要手动跑 `ae-sdd assets check`，CLI 在内部会先调" | **CLI 不存在**（实测 `CommandNotFoundException`），"内部会先调"是无根之木，AI 全凭自觉 |
| 路由判定 | §🎯 智能路由 7 步判定流程 | **没有"短问句怎么办"专门路径**，逼 AI 替用户做豁免决定 |
| 强制流程 | §执行声明 L109 "AI 必须严格按本 SKILL 规定的顺序和标准执行每个步骤" | **软约束**，无运行时校验 |
| sub-agent 边界 | 角色库 + §🤖 多 Agent 编排 L450-870 | **入口段（L1-260）完全不提"派活"**，AI 读入口时不会意识到 sub-agent 应是默认动作 |
| escape hatch | 无 | **没给"快速看一眼"开显式开口子**，逼 AI 只能"假装走完整流程"或"擅自豁免" |
| 违规检测 | 无 | **失职时 SKILL 不会主动报警** |

---

## 2. 根因分析

### 2.1 责任七三开

| 责任方 | 比例 | 证据 |
|--------|------|------|
| **root agent 主观失职** | 70% | SKILL 写得很清楚，AI 应该读完再动；主观归类"短问句=对话通道"是判断错 |
| **SKILL 设计缺陷** | 30% | "修辞性强制"+ 缺 escape hatch + 缺硬卡点 = 客观上鼓励了失职 |

### 2.2 实例化方向错误（关键发现）

| 层级 | 实例化产物 | 解决什么问题 | 实际解决了吗 |
|------|----------|------------|------------|
| **L1 SKILL 派发** | `source/SKILL.md` → `plugins/ae-sdd` → `~/.claude/skills/ae-sdd` | 通用 SKILL 部署到不同客户端 | ✅ 解决了（sync-to-plugin.sh 跑通） |
| **L2 流程纪律承接** | 应有"项目级 ae-sdd SOP"文档 | 把母版"强制流程"绑定到项目场景 | ❌ **完全没做** |

**life 项目实例化版实测**（`D:\Item\life\.harness\reins\cs-expert\agent.md`）：

| 维度 | 是否承接 |
|------|---------|
| 命名 / DDD 分层 / auth 规范 | ✅ 写了 |
| **ae-sdd 流程纪律引用** | ❌ grep `ae-sdd` 0 次命中 |
| 走 G-00 / 路由 / CodeReview 报告 | ❌ 完全没提 |

**L2 缺失的影响链**：
- root agent 系统提示里没有"项目级 ae-sdd SOP"
- 收到 `/ae-sdd` 触发 → root agent 只能凭"读母版 SKILL.md 的记忆"操作
- 母版 2000+ 行，没硬前置、没 escape hatch、没违规检测 → "读个开头就上手"是符合当前设计的**可预测行为**
- reins/cs-expert/agent.md 是 sub-agent 用的（已被 orchestrator 派活，不需要走 ae-sdd）→ **真正需要 ae-sdd 流程的 root agent 没有任何"实例化 SOP"**

---

## 3. 修改建议（4 层防线）

### L1 母版硬前置（P0，最高 ROI）

**改动点**：`D:\Item\ae-sdd\source\SKILL.md` 入口段（L1-30）

**当前**（L1-30 大约 30 行，主要是 frontmatter + "目标" + 触发词）

**建议改为**（在 L1-30 内插入 5 行硬前置声明）：

```markdown
---

## 🔴 第一动作（硬前置，禁止跳过）

收到 `/ae-sdd` 触发后，**第一动作 = 跑 §🛡️ G-00 项目资产门卫**，禁止直接读用户问题内容、禁止直接动代码、禁止派 sub-agent。

G-00 通过后按 §🎯 统一入口的 7 步判定流程路由到对应 sub-SKILL，再处理具体内容。

**违规代价**：跳过 G-00 = 本次任务失信，下游所有产物需标"事后回溯"。

**快速通道**：用户**显式说** `/ae-sdd-quick` 或 `走快速通道` 时可豁免 G-00，但仍需出最小追溯文档（注明快速通道来源）。
```

**验收标准**：
- [ ] 重读 `source/SKILL.md` L1-30，5 行硬前置可见
- [ ] 5 行硬前置与 §🛡️ G-00 L34-73 之间无内容冲突
- [ ] 触发 `/ae-sdd` 后**前 5 个 tool call** 必包含 G-00 相关动作（如 Read `assets/<projectKey>/<projectKey>.assets.md`）

### L2 项目级 SOP 模板（P1，高 ROI）

**新建文件**：`D:\Item\ae-sdd\source\docs\ae-sdd-conventions.md`

**用途**：给"使用 ae-sdd 的项目"一个项目级 SOP 模板，**专门约束 root agent（orchestrator）行为**，不约束 sub-agent。

**建议结构**（200-300 行）：

```markdown
# ae-sdd 项目级 SOP 模板

> 本文件是项目使用 ae-sdd 的项目级约定。**根母版是 `source/SKILL.md`**，本文件只补充项目级特定约束。

## 1. 项目级强制节点

### 1.1 G-00 必跑且必须落档
- 跑法：AI 读 `assets/<projectKey>/<projectKey>.assets.md`
- 落档：会话开始时输出一段"G-00 资产摘要"（含 projectKey、gitPath、lastAuditedAt）

### 1.2 路由判定必须显式
- 收到任何 `/ae-sdd` 触发后必须输出"路由判定卡"（含关键词命中、4 维判定、目标 sub-SKILL）
- 禁止主观归类为"对话轻量通道"

### 1.3 sub-agent 边界
- 自己动手前必须回答"这件事应该派给哪个 sub-SKILL / sub-agent"
- 能回答 → 用 `mavis communication send --command spawn` 派活
- 不能回答 → 先停下来想清楚

## 2. 项目级产物路径约定

（如：life 项目用 `\.auto-engineering\STORY-{ID}-BE\`，不写 SKILL 默认的 `ae-sdd-doc/iterations/...`）

## 3. 项目级 escape hatch 语义

- 快速通道：用户显式说 `/ae-sdd-quick` 或 `走快速通道`
- 不豁免节点：plan-first / 出 CodeReview 报告 / 出 CodingPlan
- 豁免节点：G-00（仍需落档，但可跳过完整 7 步路由）

## 4. 项目级失职检测

- 列出本项目已部署的 hook / 检测机制
- 失职时的报警方式（如：日志关键词、CR 红、对话内自检）
```

**验收标准**：
- [ ] 文件存在且 ≥ 150 行
- [ ] 母版 `source/SKILL.md` 在 §🛡️ G-00 后引用本文件（"**项目级补充见 `docs/ae-sdd-conventions.md`**"）
- [ ] 各使用方项目（如 life、icec-cloud-boss）能直接基于本模板创建 `ae-sdd-instance.md`

### L3 项目实例化（P1，高 ROI）

**新建文件**：`D:\Item\life\.harness\ae-sdd-instance.md`（life 项目）+ 类似文件给其他使用 ae-sdd 的项目

**用途**：承接 L2 模板，落到具体项目场景。

**建议内容**（精简版 100-150 行）：
- 项目 ID / Git 路径
- **项目资产路径（母版 = `D:\Item\ae-sdd\source\assets\<projectKey>\`；sync 后 = `~/.claude/skills/ae-sdd/skills/ae-sdd/assets/<projectKey>/`；同步脚本 = `bash D:\Item\ae-sdd\scripts\sync-to-plugin.sh`）**
  - 🔴 **2026-06-22 修：** 原文写的 `C:\Users\EDY\.claude\skills\ae-sdd\assets\<projectKey>\` 是错的,实测 `~/.claude/skills/ae-sdd/assets/` 不存在,真正路径是 `skills/ae-sdd/assets/`(因为 Claude skill 是按 `name` 目录嵌套一层)
  - **当前已生成资产的项目**:`icec-cloud-boss`、`icec-cloud-life`(实测存在)
  - **当前未生成资产的项目**:`life`(顶层目录 2026-06-16 由 `icec-cloud-boss` 重命名而来,ae-sdd 资产未跟进;实例化时需先确认资产是否存在)
- 项目级产物路径（`\.auto-engineering\STORY-{ID}-BE\`）
- 项目级 reins 列表 + 各自负责范围
- 项目级 escape hatch 实际语义（是否启用快速通道、谁有权说"走快速通道"）
- 项目级失职历史记录（避免重复犯）

**验收标准**：
- [ ] life 项目文件存在且 ≥ 80 行
- [ ] 文件第 1 段显式引用母版 `source/SKILL.md` L1-30 + `source/docs/ae-sdd-conventions.md`
- [ ] 文件第 2 段写明产物路径与 reins 列表

### L4 失职检测 hook（P2，中 ROI）

**新建文件**：`D:\Item\ae-sdd\scripts\pre-ae-sdd-check.sh`

**用途**：在 root agent 收到 `/ae-sdd` 触发时自动检测是否跑了 G-00，**没跑则阻断**（返回非零 + 错误信息）。

**建议实现**（伪代码，**2026-06-22 标注待定**）：

```bash
# pre-ae-sdd-check.sh
# 🔴 待定（2026-06-22）：
#   1) 检测源未定:原文写 `~/.mavis/logs/recent-tools.log` 实测不存在,
#      mavis daemon 是否暴露 pre-trigger hook 待验证
#   2) 阻断方式未定:CLI `ae-sdd` 不存在,只能用文件系统或 system prompt 强约束
#   3) v3.1 不实现,留 v3.2 评估
#
# 伪代码先按"基于项目资产文件存在与否"做最简版本(无需 hook):
PROJECT_KEY=$1
ASSETS_FILE="D:/Item/ae-sdd/source/assets/${PROJECT_KEY}/${PROJECT_KEY}.assets.md"

if [ ! -f "$ASSETS_FILE" ]; then
  echo "🟡 G-00 资产不存在:${PROJECT_KEY}"
  echo "   详见 source/SKILL.md §🛡️ G-00 自动触发生成"
  echo "   或参考 source/docs/ae-sdd-conventions.md §1.1"
  # v3.1 不阻断,只警告;v3.2 评估是否升级为 exit 1
  exit 0
fi

echo "✅ G-00 资产存在:${PROJECT_KEY}"
exit 0
```

**接入方式**：
- **v3.1 不接 hook**（mavis daemon 兼容性未验证，建议书 §5.1 已自承风险）
- **v3.1 fallback**：L1 母版硬前置 + L2 SOP 模板 + L3 实例化已能在 system prompt 层面形成软约束
- **v3.2 评估项**：验证 mavis daemon 是否支持 pre-trigger hook + 是否需要将 L4 升级为硬阻断

**验收标准**：
- [ ] 脚本存在且 `shellcheck` 通过
- [ ] 测试用例：未跑 G-00 → 脚本 exit 1；已跑 G-00 → 脚本 exit 0
- [ ] mavis daemon hook 接入文档（如不支持则 fallback 方案落档）

---

## 4. 执行顺序 + 依赖关系

```
L1 母版硬前置 (P0)
   ↓ 依赖（必须先做）
L2 项目级 SOP 模板 (P1)
   ↓ 依赖（必须先做）
L3 项目实例化 (P1)  ← 多个项目并行（life、icec-cloud-boss 等）
   ↓ 可选依赖
L4 失职检测 hook (P2)
```

**预计工作量**：

| 阶段 | 文件 | 工作量 |
|------|------|--------|
| L1 | 1 改 + 1 PR | 30 min（含评审） |
| L2 | 1 新建 + 1 引用 | 2-3 h |
| L3 | 1 新建/项目 × N | 30 min/项目 |
| L4 | 1 新建 + 1 测试 + hook 接入 | 4-6 h（含 hook 验证） |

**总计**：~1 个工作日（含评审）

---

## 5. 风险与回滚

### 5.1 风险

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| L1 硬前置让 root agent 误触发（不该跑 G-00 也跑）| 中 | 低 | L2 模板明确"快速通道"语义 |
| L1 改母版影响其他项目（life、icec-cloud-boss 等）| 高 | 中 | **灰度发布**：先在 life 项目验证 1 周，再推全量 |
| L2 模板与项目实际需求不符 | 中 | 中 | 模板保持"项目可裁剪"，不强制全部使用 |
| L4 hook 接入与 mavis daemon 不兼容 | 中 | 低 | fallback 到 system prompt 强约束 |
| 改动后用户对"硬前置"反感（"太重了"）| 低 | 高 | L2 给 escape hatch 开口子 |

### 5.2 回滚方案

- L1 回滚：revert PR，恢复原 SKILL.md
- L2 回滚：删除 `ae-sdd-conventions.md`，移除母版引用
- L3 回滚：删除 `ae-sdd-instance.md`（各项目独立，影响最小）
- L4 回滚：移除 hook 注册，删除 `pre-ae-sdd-check.sh`

### 5.3 影响范围

- L1+L2 改母版，影响所有 sync 的下游（life、icec-cloud-boss、其他使用 ae-sdd 的项目）
- L3 仅影响具体项目
- L4 仅影响 hook 启用的项目

---

## 6. 验收清单（移交时必填）

| 验收项 | 状态 | 验收人 | 验收时间 |
|--------|------|--------|----------|
| L1 SKILL.md 入口段硬前置可见 | ☐ |  |  |
| L2 ae-sdd-conventions.md 模板完成 | ☐ |  |  |
| L3 life 项目 ae-sdd-instance.md 完成 | ☐ |  |  |
| L3 icec-cloud-boss 项目 ae-sdd-instance.md 完成（如适用）| ☐ |  |  |
| L4 pre-ae-sdd-check.sh 脚本测试通过 | ☐ |  |  |
| L4 mavis daemon hook 接入完成 | ☐ |  |  |
| 灰度发布 1 周无回归 | ☐ |  |  |
| CHANGELOG 更新（v3.0 → v3.1）| ☐ |  |  |

---

## 7. 附录

### 7.1 失败案例原文（2026-06-22 实测）

- 用户原话：`/ae-sdd 待接待超时做了一个新逻辑，结单前要拉一次AI消息。实现可参考工单接单同步AI消息逻辑和转人工同步AI消息逻辑。你帮我查一下实现有没有问题？`
- root agent 失职序列：见 §1.1
- 补救产物：`D:\Item\life\.auto-engineering\STORY-020-BE\code-review-report-v3-r2.md`（事后回溯，标注）

### 7.2 相关 memory 条目

- `C:\Users\EDY\.mavis\agents\mavis\memory\MEMORY.md` 新增 `### ae-sdd SKILL 强制执行纪律 (2026-06-22)` 7 条规则
- `D:\Item\life\.harness\memory\MEMORY.md` 第 ⑥bis 条（STORY-020 v2-r2 CodeReview）— 漂移判断与升版纪律

### 7.3 相关 SKILL 章节索引

- 母版 `source/SKILL.md` §🛡️ G-00 L34-73（项目资产门卫）
- 母版 `source/SKILL.md` §🎯 统一入口 L119-338（7 步路由判定）
- 母版 `source/SKILL.md` §执行声明 L109（强制流程声明）
- `source/skills/phase3-review/code-review-skill.md` §第四步 L547-578（CodeReview 报告模板）
- `source/skills/phase3-review/code-review-skill.md` §Plan-first L159（编码前硬前置）

### 7.4 不承诺"永不再犯"

本 Plan **不承诺"永不再犯"**。原因：
- SKILL 是软约束，无法强制 AI 行为
- LLM 训练分布、上下文长度、用户表达差异都会影响 AI 决策
- 唯一能"永不再犯"的方案是**用代码 hook 强制**（即 L4），但 hook 本身有兼容性问题

**务实目标**：再犯率从 ~50% 降到 ~5%，失职时**早期发现 + 显式标注 + 留追溯**。
