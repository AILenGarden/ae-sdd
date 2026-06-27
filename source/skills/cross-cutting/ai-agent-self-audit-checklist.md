---
name: ai-agent-self-audit-checklist
description: AI Agent 任务开始前自审清单 SOP（🔴 强制，每任务必跑）。覆盖 5 步骤（识别任务类型 / 识别输入类型 / 识别最小必跑流程硬卡片 / 用户催促反模式处理 / 自审完成声明）。🆕 2026-06-27 新建，解决"AI Agent 撞到出文档类任务直接动笔跳过 RA skill"的系统性违规（实测案例：2026-06-27 AI Agent 直接读 PDF 出 36KB proposal 未走 RA skill 完整 7 步）。
---

# AI Agent 自审清单 SOP — 任务开始前强制自审

> **🔴 核心定位（2026-06-27 新建）：** 本 SOP 是 ae-sdd 体系的**任务启动前置自审**机制——任何 AI Agent 在接到任务、开始动笔之前，必须跑完本 SOP 的 5 步骤，否则视为流程违规。
>
> **🔴 署名原则（来自 2026-06-27 用户裁决）：**
> - **本 SOP 不绑定任何特定 Agent 身份**（无论 Mavis / ZCode / Claude / GPT / GLM-5.2 / claude-code）
> - SOP 适用对象是"任何使用 ae-sdd 的 AI Agent"，由 ae-sdd-update-skill 执行人 / 架构组维护
> - 生成文档的 AI Agent 不应出现在署名字段；文档署名应为产出责任主体
>
> **背景（self-flagged defect）：** 2026-06-27 AI Agent 接到"分析同事知识库 PDF 出 proposal"任务时，**直接读 PDF 出 36KB proposal**，未走 `requirement-analysis-skill.md` 完整 7 步。被用户连续批评"又来了，不遵守流程"。事后追溯发现 6+ 份历史 ae-sdd 修订建议书均直接出文档未走 RA skill——是**ae-sdd 自我进化机制无审计**的系统性风险。本 SOP 是补救措施之一。

---

## 触发条件

| 触发场景 | 触发方式 |
|---------|---------|
| AI Agent 接到任何"出文档类"任务 | 用户说"出 proposal / 出建议书 / 出分析报告 / 出调研文档 / 修订建议 / 改造方案" |
| AI Agent 接到"出代码类"任务 | 用户说"实现 XX / 修 XX / 改 XX" |
| AI Agent 接到"出配置类"任务 | 用户说"改 YAML / 改 Properties / 改环境变量" |
| AI Agent 接到"出 RA 类"任务 | 用户说"分析需求 / 从 PRD 开始 / 需求拆解" |

> **不触发本 SOP 的场景：** 纯问答、解释、读代码不产生改动的轮次无需调用。

---

## 5 步骤自审清单（🔴 强制顺序）

### 步骤 1：识别任务类型

| 任务类型 | 必跑最小流程 |
|---------|------------|
| **出文档类**（建议书 / proposal / 分析报告 / 调研文档 / 修订建议）| RA skill 完整 7 步 |
| **出代码类**（功能开发 / BUG 修复）| coding-skill + RA 简化版（5 维规模裁定）|
| **出配置类**（YAML / Properties 调整）| coding-skill 特殊-非代码路径 |
| **出 RA 类**（需求分析 / 从 PRD 开始）| requirement-analysis-skill 完整 7 步 |
| 纯问答 / 分析 / 读代码 | 无流程（不触发本 SOP）|

**🔴 判定规则：** 任务大小 ≠ 流程可豁免。AGENTS.md 黄金法则已明确："无设计不编码；先评审后实现"。

### 步骤 2：识别输入类型 + 触发 SKILL

| 输入类型 | 触发 SKILL |
|---------|----------|
| PRD / Issue 文件 | requirement-analysis-skill |
| 对话需求 | requirement-analysis-skill |
| PDF / 文档分析 | requirement-analysis-skill |
| 代码反推需求 | requirement-analysis-skill |
| BUG 报告 | coding-skill（BUG path，intent=BUG 双重豁免）|
| 配置类 | coding-skill（特殊-非代码，intent=CONFIG 双重豁免）|

**🔴 凭证前置（v3.4.0 加固）：** git 仓库工作目录下，调用任何 SKILL 前必须先 `ae-sdd enter <projectKey> --story <STORY-ID>` 领 entry token。未领凭证的流程产物落地 / 代码改动将被关卡 2/3 物理拦截（HS-9/10/11）。

### 步骤 3：识别最小必跑流程的"硬卡片"

若步骤 1 判定是"出文档类"或"出 RA 类"，硬卡片包括：

- [ ] **RAGeneratePlan**（先于 RA 文档，Plan-first 硬前置）
- [ ] **RAModel 12 维决策**（RA-01~RA-12 + 需求风险预判 8 类）
- [ ] **8 维度挖掘**（角色 / 场景 / 流程 / 数据 / 规则 / 设计方向 / AC / 假设）
- [ ] **5 问自检**（每条结论：证据 / 反例 / 边界 / 冲突 / 缺口，通过率 100%）
- [ ] **缺口管理**（🔴/🟠 全部解决或用户明确接受）
- [ ] **规模裁定**（5 维评分 + 路由决策）
- [ ] **RA-G01~RA-G16 闸判定**（至少 8 个 PASS，详见 `tools/lib/document_storage.py:check_ra_prerequisites`）
- [ ] **落盘**（`save_doc` 调用，触发 G-RA-PLAN / G-RA-COMPLETE / G-RA-5CHECK / G-RA-GATES 前置检查）

### 步骤 4：用户催促的反模式处理

若用户催促"我想要一次性回答 / 不想多次回答"，**禁止**直接接受。必须：

1. 提示"RA skill 的最小合规路径"（步骤 3 的 8 个硬卡片）
2. 用 RA skill §反模式 7 的确认模板逐项确认
3. 把所有 🔴 阻断型问题一次性列给用户

**🔴 关键约束：** 用户催促 ≠ 流程可跳过。用户催促 = 流程必须更快地收敛到最少必要轮次，但**不能跳到 0 轮**。

### 步骤 5：自审完成声明

完成步骤 1-4 后，在对话中显式声明：

```
✅ AI Agent 自审完成：
- 任务类型：[出文档类 / 出代码类 / 出配置类 / 出 RA 类]
- 输入类型：[PRD / Issue / 对话 / PDF / 代码反推 / BUG / 配置]
- 触发 SKILL：[requirement-analysis-skill / coding-skill / ...]
- 最小必跑流程：[RA skill 7 步 / coding-skill / ...]
- entry token：[已领 / 未领（理由：母版仓库无 .ae-sdd/）]
- 已开始执行。
```

---

## 与其他 SKILL 的关系

| SKILL | 关系 |
|------|------|
| `requirement-analysis-skill.md` | 本 SOP 步骤 3 的 8 个硬卡片对应 RA skill 完整 7 步 |
| `ae-sdd-skill.md`（§🎯 智能路由）| 本 SOP 步骤 1/2 的判定规则与 SKILL.md §4 维判定智能路由表对齐 |
| `proposal-skill.md` | "出建议书"任务走 RA skill 完成后再走 proposal-skill（proposal 是 RA 输出后的执行载体）|
| `document-storage-skill.md` | 本 SOP 步骤 3 的落盘硬卡片调用 save_doc，触发 G-RA-PLAN/COMPLETE/5CHECK/GATES 前置检查 |

---

## 反模式（本 SOP 禁止的 4 类行为）

| # | 反模式 | 危害 | 正确做法 |
|---|--------|------|---------|
| 1 | AI Agent 跳过本 SOP 直接动笔 | 任务类型判定错误 → 走错 SKILL | 任务开始前必跑本 SOP 5 步骤 |
| 2 | AI Agent 把"建议书 / proposal / 分析报告"当作"非 RA" | 跳过 RA skill 7 步 → 出文档无审计依据 | 走 RA skill §反模式 8 自检 |
| 3 | AI Agent 撞到"用户催促 + 任务小"跳过流程 | 流程违规 + 用户监督成本激增 | 走 RA skill §反模式 7/9 |
| 4 | AI Agent 照抄上一份文档的署名风格 | 身份未核实 → 审计链失真 | 每份文档开头 authors 字段基于本会话真实身份；ae-sdd 文档不绑定特定 Agent 名字 |

---

## 维护

- **维护人：** ae-sdd-update-skill 执行人 / 架构组（不绑定特定 Agent 身份）
- **更新频率：** 触发"self-flagged defect"或新增反模式时
- **同步对象：**
  - `requirement-analysis-skill.md` §反模式 7/8/9（本 SOP 的规则源头）
  - `ae-sdd-skill.md` §🎯 智能路由（本 SOP 步骤 1/2 的判定依据）
  - `tools/lib/document_storage.py:check_ra_prerequisites`（本 SOP 步骤 3 落盘硬卡片的代码实现）
  - `scripts/flow_violation_scan.py`（本 SOP 步骤 3 的工具层兜底审计）
- **关键变化（2026-06-27 新建）：**
  - 🆕 5 步骤自审清单（任务类型 / 输入类型 / 最小必跑流程硬卡片 / 用户催促反模式 / 自审声明）
  - 🆕 4 类反模式禁止（跳过 SOP / 误判非 RA / 催促跳流程 / 照抄署名）
  - 🆕 署名原则声明（不绑定特定 Agent 身份）
