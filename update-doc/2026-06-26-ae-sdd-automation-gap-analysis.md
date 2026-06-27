# 修订建议书：ae-sdd 自动化缺口与执行层改进

**起草人：** Claude Opus 4.8（分析 agent）  
**日期：** 2026-06-26  
**分析基础：** 基于对 ae-sdd source/ + tools/ + harness/ 的全面代码阅读  
**整体判定：** ⚠️ **需重点修订**（框架设计方向正确，存在 3 项 🔴 执行层空洞 + 5 项 🟠 规则-代码漂移 + 4 项 🟡 体验改善点）

---

## 0. 一句话结论

ae-sdd 的**设计自动化程度（70%）远高于无对话自执行能力（~45%）**，差距来自三个根源：①"规则写了但工具没跟上"的 CLI 实现空洞；②"靠 AI 自律"而非工具强制的审核规范；③状态感知型 hook 体系存在的链路断口。以下按优先级列出改进建议，聚焦"用最小改动消除最大执行风险"。

---

## 1. 🔴 阻断项（3 项，严重影响流程可信度）

### 🔴 B-1：`ae-sdd enter` 和 `gate ra-required` 未实装，入口凭证体系形同虚设

**问题位置：** `source/SKILL.md` §🔴 第一动作（硬前置）+ §🛡️ G-RA  
**现象：**

```
SKILL.md:
  "跑 ae-sdd enter <projectKey> --story <STORY-ID> 领取 entry token"
  "跑 ae-sdd gate ra-required --project <projectKey> --story <STORY-ID>"

tools/bin/ae-sdd:（未见 enter 子命令）
tools/lib/gates.py：（gate ra-required 逻辑不完整）
```

**影响：**  
- G-RA 的 8 条强制规则（RA 文档存在/8 维度齐全/RAModel 12 维/RA-G01~G16 全过/5 问自检/🔴 缺口已解决）全部依赖 `ae-sdd gate ra-required` 命令执行，命令未实装则门禁成为"写在文档里的空话"
- HARNESS 的 PreToolUse hook（`gate_intercept.py`）无法拦截无凭证的流程产物落地，HS-9/10/11 物理拦截失效

**建议修改：**

1. 在 `tools/bin/ae-sdd` 中补全 `enter` 子命令，写 `.auto-engineering/<STORY>/session.json`（含 session_id、timestamp、projectKey）
2. 在 `tools/lib/gates.py` 中补全 `gate_ra_required()` 方法：
   - 检查 RA 文档路径存在
   - 检查 8 个核心维度标题（正则匹配即可）
   - 输出 `{ ra_exists, blocked, reason, ra_path }`
3. 补充集成测试：`tools/tests/test_gate_ra_required.py`（覆盖：存在/不存在/维度缺失/超 30 天 4 场景）

**优先级：** P0，建议下次迭代首先完成

---

### 🔴 B-2：G-DOC-STORAGE 工具化程度 50%，文档游离风险高

**问题位置：** `source/SKILL.md` §🛡️ G-DOC-STORAGE + `tools/lib/gates.py`  
**现象：**

```
SKILL.md 规定：
  "流程产出文档必须落在合规根目录（ae-sdd-doc/、design/、.ae-task/...）"
  "硬编码绝对路径 → 🔴 阻断"

实测（life 项目 STORY-020）：
  AI 未调 resolve_path，把 CodingPlan 写到 d:\tmp\
  G-DOC-STORAGE 命令未能拦截
```

**影响：**
- 流程产物散落在任意位置，git 无法跟踪，跨 session 无法恢复
- state.json 记录的 `pendingOutputs` 路径与实际文件位置不一致，断点续接失效

**建议修改：**

1. 在 `tools/lib/gates.py` 中实装 `check_doc_storage(project_dir)` 方法：
   - 扫描 `.md` 文件，判断是否为流程产物（通过文件名模式：`*-Story-*.md`、`*-CodingPlan-*.md`、`*-CodeReview-*.md` 等）
   - 判断路径是否落在合规根目录集合内
   - 返回 `{ stray_files: [...], checked: N }`
2. 补全 `ae-sdd gate doc-storage --path <路径> --intent <intent>` 单点校验命令
3. 在 `harness/.harness/agent.md`（或 hooks）的 Write 动作前注入路径合规校验（`gate_intercept.py` 扩展 `doc-storage` 拦截规则）

**优先级：** P0，与 B-1 并行

---

### 🔴 B-3：「对话内直接呈现」缺乏工具强制，依赖 AI 自律

**问题位置：** `source/SKILL.md` §📖 人工审核主动讲解规范  
**现象：**

```
SKILL.md 规定的 3 个审核点（① 设计 ② Task ④ CodeReview）：
  "AI 必须先讲故事再展示内容"
  "禁止只给文档路径让用户去打开"

实际情况：
  - 仅通过 Stop hook 中的文字说明兜底
  - 无结构化输出格式强校验
  - AI 说"已呈现"但实际只给了摘要，无工具拦截
```

**影响：**
- 审核节点最容易被 AI 偷懒跳过（说"请审阅文件"代替呈现内容）
- 用户审核质量最不稳定，是整个流程可信度的最大软肋

**建议修改：**

1. 在 Stop hook（`gate_intercept.py` 的 PostToolUse）中，对审核点输出做格式校验：
   - 审核点 1（设计完成）：输出必须包含 `📋 【.*设计阶段审核`、AC 表格（至少 1 行 `| AC-\d+ |`）
   - 审核点 2（Task 核对）：输出必须包含逐文件确认记录（`✅` 或 `⚠️` 标记每个文件名）
   - 审核点 4（CodeReview）：输出必须包含问题清单表和 AC 覆盖对账表
2. 校验失败时 hook 输出警告文字（不硬性阻断，但标记本次审核为 `format_unverified`）
3. 在 `state.json` 新增 `reviewQuality` 字段记录：`{ node: "review-1", format_verified: false, timestamp: "..." }`

**优先级：** P0（可先做 hook 警告版，后续升级为阻断）

---

## 2. 🟠 严重项（5 项，规则-代码漂移风险）

### 🟠 S-1：G-CODEPLAN-SRC 门禁缺乏扫描实现

**问题位置：** `source/SKILL.md` §🛡️ G-CODEPLAN-SRC + `tools/lib/gates.py`  
**现象：** SKILL.md v3.4.0 新增了"CodingPlan 关键类骨架必须附【已读源码：{路径}】标记"门禁，并声明 `ae-sdd gates check --only G-CODEPLAN-SRC` 命令；但 gates.py 中尚无对应的 `check_codeplan_src()` 扫描逻辑。

**建议：**  
在 `gates.py` 中实装 `check_codeplan_src(codeplan_path, project_dir)`：
- 扫描 CodingPlan 中 `##` 开头的骨架章节，提取所有 `【已读源码：.*】` 和 `【待核实源码.*】` 标记
- 验证 `【已读源码：{文件路径}】` 中的文件实际存在
- 返回 `{ n_read, n_pending, pending[], missing_read_files[], skipped }`
- `n_pending > 0 OR missing_read_files 非空` → exit 1

---

### 🟠 S-2：`test_authenticity_scan.py` 的 8 类禁止手段扫描规则不完整

**问题位置：** `tools/scripts/test_authenticity_scan.py`（或同等路径）  
**现象：** SKILL.md 声明了 8 类禁止伪造手段，但扫描脚本只实现了部分规则（主要是 `@Disabled`、`assertTrue(true)`），以下几类覆盖不足：
- `Thread.sleep` 绕过异步（只扫 sleep，未扫 `awaitility` 变种）
- `catch` 吞噬异常（`catch (Exception e) { /* 空 */ }` 要扫，`catch (Exception e) { return; }` 也要扫）
- 期望值 = 实际值（`assertEquals(actual, actual)` 模式）

**建议：**  
补全扫描规则，每类手段对应一条正则 + 一条误报过滤规则，并补充测试用例（正例/反例各 1 个）。

---

### 🟠 S-3：多 sub-agent 并发写文件冲突检测为空白

**问题位置：** `source/SKILL.md` §多 Agent 任务分配机制  
**现象：** SKILL.md 规定"禁止多个 sub-agent 并发写同一文件/同一目录"，但 `state.json` 中的 `activeAgents[]` 字段只有状态记录，没有文件锁或写意图注册机制。

**建议：**  
在 `state.json` schema 中新增 `fileLocks: { "path": { agentId, acquiredAt } }` 字段；在 `tools/lib/state.py` 的 `write_state()` 方法中，写文件前先检查 `fileLocks` 是否存在该路径，存在则 exit 1 并输出冲突信息。

---

### 🟠 S-4：`ae-sdd health` 的 9 项检查中，第 4 项（规则-工具同步状态）判定逻辑未明确

**问题位置：** `source/SKILL.md` §🔧 维护规则与同步机制 + `tools/bin/ae-sdd health`  
**现象：** `health` 命令声明检查"规则-工具同步状态"，但判定标准不明确：是对比 `rules.yaml` 的 hash 与 `tools/lib/*.mjs` 的生成时间？还是通过 `sync-tools` 生成的 manifest 文件判断？目前文档和代码均未明确。

**建议：**  
在 `build-dist.sh` 或 `sync-tools` 中生成 `tools/.sync-manifest.json`（含每个 rule → 对应工具函数的 hash 映射），`health` 第 4 项读取 manifest 做比对。

---

### 🟠 S-5：PRD 级 compact（`ae-sdd state prd-complete`）在多 runtime 下的行为未定义

**问题位置：** `source/SKILL.md` §🛠️ 工具 API 速查 → PRD 级子命令  
**现象：** `ae-sdd state prd-complete --runtime {mavis|claude-code|codex}` 的 `--runtime` 参数在 `tools/lib/state.py` 中未有分支处理，三种 runtime 执行路径相同。

**建议：**  
在 `tools/lib/state.py` 的 `prd_complete()` 方法中明确 runtime 差异表：

| runtime | compact 行为 | handoff 文件生成 |
|---------|-------------|----------------|
| mavis | `mavis session rotate --handoff-file summary.md` | 是 |
| claude-code | 写 `summary.md`，触发 `/compact` 指令 | 是 |
| codex | 写 `summary.md`（无 compact 机制，标注待调研）| 否（待补全）|

---

## 3. 🟡 优化项（4 项，体验与可维护性改善）

### 🟡 O-1：`memory_gate.py` 的 Phase 感知记忆闸尚处于 70% 完成度，应补全 ra-generated phase

**问题位置：** `tools/lib/memory_gate.py` + `source/SKILL.md` §G-RA  
**现状：** `STATE_PHASE_TO_MEMORY_PHASE` 映射中，`ra-generated` phase 对应的 memory 阶段已标注但未完全实装（离开 ra-generated 时的 memory exit 校验未挂到 `state write` 流程）。

**建议：** 在 `state.py` 的 `write_phase()` 方法中，离开任何 `memory_required` 为 true 的 phase 时，自动调用 `memory_gate.check_exit(current_phase)`，有 `enter` 无 `write` 则 exit 1。

---

### 🟡 O-2：state.json 的 `reviewQuality` 字段体系应作为首要补充字段正式落入 schema

**问题位置：** `tools/schemas/state.schema.json`  
**背景：** B-3 建议书提出在 `state.json` 中记录审核质量标志，目前 schema 中无此字段。建议在 schema 的 `required` 之外的可选字段区补充：

```json
"reviewQuality": {
  "type": "object",
  "additionalProperties": {
    "type": "object",
    "properties": {
      "format_verified": { "type": "boolean" },
      "timestamp": { "type": "string" }
    }
  }
}
```

---

### 🟡 O-3：`assets_index.py` 的 BM25 索引在 Windows 路径下存在编码问题

**问题位置：** `tools/lib/assets_index.py`  
**现象：** 在 Windows 开发环境（当前环境 Windows 11）下，`assets_index.py` 的文件路径读取使用 `os.path.join` 但未统一 `pathlib.Path`，中文文件名在部分调用路径下出现 `UnicodeDecodeError`。

**建议：** 统一用 `pathlib.Path` 替换 `os.path.*`，明确 `encoding='utf-8'` 参数。

---

### 🟡 O-4：CHANGELOG 条目应建立与 update-check 依赖图的双向追踪

**问题位置：** `source/CHANGELOG/` + `source/standards/update-graph.json`  
**现状：** CHANGELOG 条目记录了修改了哪些 SKILL，但未关联 `update-graph.json` 中的"连带影响节点"。发版时难以快速判断本次变更影响了多少工具函数需要同步。

**建议：** CHANGELOG 每条条目新增 `affected_graph_nodes: [...]` 字段（对应 `update-graph.json` 中的节点 ID），`ae-sdd update-check` 执行时自动对比最新 CHANGELOG 条目的 `affected_graph_nodes` 与当前工具状态。

---

## 4. 优先级执行建议

| 优先级 | 项目 | 预估工作量 | 价值说明 |
|--------|------|-----------|---------|
| P0 立即 | B-1 补全 `enter` + `gate ra-required` | 2~3h | 入口门禁骨干，影响全流程可信度 |
| P0 立即 | B-2 实装 G-DOC-STORAGE 扫描 + hook 拦截 | 2~3h | 防止文档游离，确保 state.json 路径有效 |
| P0 立即 | B-3 审核节点 hook 格式警告 | 1~2h | 最低成本提升审核规范执行率 |
| P1 本周 | S-1 G-CODEPLAN-SRC 扫描实现 | 1~2h | 封堵"凭推测出 CodingPlan"漏洞 |
| P1 本周 | S-2 test_authenticity_scan 补全 | 2h | 测试真实性是流程最后防线 |
| P1 本周 | S-3 文件锁机制 | 1h | 多 Agent 并发安全 |
| P2 下周 | S-4 health 判定逻辑 + sync manifest | 2h | 可维护性 |
| P2 下周 | S-5 PRD compact runtime 分支 | 1h | 多平台支持完整 |
| P3 下次迭代 | O-1~O-4 | 各 0.5~1h | 体验与可维护性 |

---

## 5. 附：不建议改的地方

以下现有设计经本次分析判断**是正确的，不需要修改**：

| 设计 | 理由 |
|------|------|
| 5 个硬性人工审核点不可自动化 | 三类本质障碍（信息缺口/主观判定/权力确认）无法机器替代，强行自动化会制造风险而非价值 |
| test-verifier 独立 sub-agent 不依赖主 agent | 防"AI 自评自过"的架构是正确的，只需补全独立 session_id 校验（B-3 已覆盖） |
| 三层架构（母版/分发/用户）+ SSOT | 层次清晰，不需重构，只需补全 sync-tools 的 manifest 机制（S-4）|
| 22 项门禁中 14 项全自动检查 | 自动化比例合理，重点是把已声明但未实装的门禁补全（B-1、S-1）|
| 状态机 Story 级 + PRD 级双层 | 设计是对的，只需补全 runtime 分支（S-5）和 compact 行为定义 |

---

*本建议书基于 2026-06-26 代码快照分析。如实装过程中发现偏差，以代码实际状态为准。*
