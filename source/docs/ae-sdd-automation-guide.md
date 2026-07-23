---
name: ae-sdd-automation-guide
description: ae-sdd 自动化模式使用指南 — 面向终端用户的从 0 开始实操 walkthrough。前置条件 → 开启 → 第一次跑 → 看联审结果 → 失败处理 → 关闭。
---

# ae-sdd 自动化模式使用指南

> **🆕 v3.8.0** · 面向项目开发者 / 项目 owner
>
> 本指南教你如何把 ae-sdd 从"每个审核点都要人工✅"切换到"输入→结果全自动化"。默认关闭，开启后 6 个人工审核点改走 Tier 3 多 reviewer 联审共识。
>
> 配套规范（AI 执行用）：[`source/SKILL.md §🚀 自动化模式`](../SKILL.md) · 设计说明：[`ae-sdd-design.md §19`](ae-sdd-design.md)

---

## 0. 适合我吗？

| 场景 | 是否建议开启自动化 |
|------|------------------|
| 信任 ae-sdd 联审机制、想省去 6 次人工✅ | ✅ 推荐 |
| 需求清晰、输入材料（PRD/DR/Story）齐全 | ✅ 推荐 |
| 需求模糊、需要边做边和 AI 讨论方向 | ❌ 保持默认（人工审核更稳） |
| 涉及资金/权限/核心交易链路、想人工把关关键点 | ⚠️ 可开启但把该审核点移出白名单 |
| 首次用 ae-sdd、还不熟悉流程 | ❌ 先用默认模式跑通 1-2 个 Story 再考虑 |

**核心权衡**：自动化用"3 个独立 AI reviewer 交叉审"换"人工✅"。省人工，但联审仍是 AI 审 AI——不能 100% 等同人工把关。关键决策点可配置仍走人工。

---

## 1. 前置条件

开启前确认以下条件齐备：

- [ ] **ae-sdd 已安装**（`ae-sdd version` 能输出版本号 ≥ 3.8.0）
- [ ] **项目已 init**（项目根有 `.ae-sdd/config.yaml`）；未 init 先跑 `ae-sdd init <项目路径> <projectKey>`
- [ ] **项目资产已生成**（`.ae-sdd/assets/<projectKey>.assets.md` 7 层索引齐备）；`ae-sdd gates check --only G-00` 通过
- [ ] **运行环境支持派物理 sub-agent**（Claude Code / Codex / Harness 等能 spawn 独立 session；单 session 环境见 §6 降级说明）
- [ ] **输入材料已就绪**（PRD/DR/Story 至少有一份；自动化模式不改变"必须有产物才能入链"的规则）

```bash
# 一键自检
ae-sdd version                    # 确认 ≥ 3.8.0
ae-sdd gates check --only G-00    # 确认项目资产齐备
ls .ae-sdd/config.yaml            # 确认已 init
```

---

## 2. 开启自动化

### 2.1 一行命令开启

```bash
ae-sdd automation enable
```

预期输出：
```
✅ 🟢 自动化模式已启用
   审核点将走 Tier 3 联审共识，跳过人工✅
   开工前信息预收集：开
   阻断出口：pause
```

此命令会：
- 把 `.ae-sdd/config.yaml` 的 `automation.enabled` 改为 `true`
- 写入 `enabledAt` 审计时间戳（**AI 不得自行改**，只能你用此命令写）

### 2.2 确认配置

```bash
ae-sdd automation status
```

预期输出：
```
ℹ️  自动化模式：🟢 已启用
   reviewerTier: 3
   preflightInfoCollection: True
   onConsensusStall: pause
   automatedReviewPoints: [1, 1.5, 2, 2.5, 4, 5]
   enabledAt: "2026-07-03T..."
```

### 2.3 配置字段说明

| 字段 | 默认值 | 含义 | 怎么改 |
|------|-------|------|---------|
| `enabled` | `false` | 总开关 | 🔴 **只能用 `ae-sdd automation enable/disable`**，不手改（手改不会写 `enabledAt` 审计时间戳，且 AI 不得自行 enable）|
| `reviewerTier` | `3` | 联审强度（固定三审） | 暂不支持改，自动化模式强制 3 |
| `preflightInfoCollection` | `true` | 开工前信息预收集 | 直接编辑 config.yaml 改 `false` |
| `onConsensusStall` | `pause` | 联审 3 轮未决时的出口 | 直接编辑 config.yaml 改 `pause`/`fail` |
| `automatedReviewPoints` | `[1, 1.5, 2, 2.5, 4, 5]` | 走自动联审的审核点白名单 | 直接编辑 config.yaml，增删编号 |
| `enabledAt` | `""` | 开启时间戳（审计） | 🔴 **不手改**，由 `automation enable` 自动写 |

**修改方式分两类**：
- **`enabled` / `enabledAt`** → 必须用 CLI 命令（`automation enable/disable`），因为命令会同时写时间戳做审计，防止 AI 偷偷自己开
- **`preflightInfoCollection` / `onConsensusStall` / `automatedReviewPoints`** → 直接编辑 `.ae-sdd/config.yaml`，改完立即生效，**不用重新 enable**（AI 每次审核点都会重读 config）

**审核点编号对照**：

| 编号 | 时机 | 内容 |
|------|------|------|
| 1 | Phase 1 末 | 设计阶段完成确认（Story + TestCase）|
| 1.5 | Phase 2 头 | 实现方案预确认 |
| 2 | Task 文档完成 | Task 逐文件核对 |
| 2.5 | CodingPlan 评审 | 14 条门禁 + CodingModel 11 维 |
| 4 | CodeReview 完成 | 代码实现完成确认 |
| 5 | PRD 完成 | PRD 级 4 层 AND 闸（仅大任务）|

### 2.4 想让某个审核点仍人工把关？

例如：涉及交易链路，想让 CodeReview（点 4）仍人工审。

编辑 `.ae-sdd/config.yaml`，把 4 从数组移除：
```yaml
automation:
  enabled: true
  automatedReviewPoints: [1, 1.5, 2, 2.5, 5]  # 移除了 4
```

之后 CodeReview 仍会停下来等你 ✅，其余点继续走联审。**改完立即生效，无需重新 enable**。

反过来，想把某个点加回自动化：把编号加回数组即可。

---

## 3. 第一次跑：完整流程

### 3.1 启动

在项目根目录对 AI 说：
```
/ae-sdd
```
或「启动自动化工程」「从 DR 开始实现」。

### 3.2 Step 1 自动化检测

AI 会先读 config，看到 `enabled: true` 后输出：
```
【自动化模式已启用 — 审核点将走 Tier 3 联审共识，跳过人工✅】
```

### 3.3 Step 1.5 开工前信息预收集

AI 自动跑 `ae-sdd preflight collect`，扫描你的输入材料 + 项目资产，识别 6 类待补信息：
1. **第三方平台凭证**（极光/融云 AppKey/AppSecret 等）
2. **复用项选择**（AI 找不到时问用哪个已有实现）
3. **环境配置**（DB/Redis/MQ 地址）
4. **命名约定**
5. **已有对接方信息**
6. **数据初始化要求**

输出形如：
```
🔍 开工前信息预收集（扫描 5 个文档）
📋 待补信息清单（共 3 项，请一次性补齐）：
  【第三方平台凭证】(2 项)
    - design/PRD-X.md: …极光推送 AppKey: {待确认}…
    - design/PRD-X.md: …融云 IM Token: {待确认}…
  【复用项选择】(1 项)
    - assets/xxx.assets.md: …消息推送组件 {待复用}…
  清单已写入 .ae-sdd/preflight-info.yaml
```

**你也可以自己手动跑预收集**（不依赖 AI）：
```bash
ae-sdd preflight collect                # 扫描并生成 .ae-sdd/preflight-info.yaml
ae-sdd preflight collect --json         # 输出结构化 JSON（便于脚本处理）
```

**你的动作**：
- 方式 A：直接在对话里告诉 AI（"极光 AppKey 是 xxx，消息推送用已有的 xxx-sender 组件"）—— AI 会把信息写进 preflight-info.yaml
- 方式 B：自己编辑 `.ae-sdd/preflight-info.yaml`，把每个待补项的值填到对应行下面

`.ae-sdd/preflight-info.yaml` 示例（填好后）：
```yaml
# ae-sdd 开工前信息预收集清单（由 ae-sdd preflight collect 生成）
第三方平台凭证:
  - design/PRD-X.md: …极光 AppKey: {待确认}…   # ← 填实际值：a1b2c3...
  - design/PRD-X.md: …融云 IM Token: {待确认}…  # ← 填实际值或标注"已配置在 nacos"
复用项选择:
  - assets/xxx.assets.md: …消息推送组件 {待复用}…  # ← 填：用 icec-message-sender
```

补齐后 AI 才进 Step 2 开工，**开工后不再因缺信息打断你**。

> 无待补信息时 AI 直接进 Step 2，不卡你。
> 想跳过预收集（确认材料齐全、不想 AI 卡这步）：把 config.yaml 的 `preflightInfoCollection` 改 `false`。

### 3.4 正常流程 + 审核点联审

之后走标准 ae-sdd 流程（RA → DR → Story → TestCase → Task → Coding → Test → CodeReview），区别只在 6 个审核点：

**默认模式**（你之前用的）：
```
[审核点 1] AI 讲解设计 → 等你 ✅/⚠️/❌ → 你回 ✅ → 推进
```

**自动化模式**：
```
[审核点 1] AI 讲解设计（听众变 3 个 reviewer）
         → AI 派 3 个独立 session reviewer（业务/架构/第三方视角）
         → 跑交叉对比算法（§8.4.3）
         → 写 reviewConsensus[1].passed=true
         → G-09B + G-REVIEW-LOOP + G-AUTO-CONSENSUS 三门禁全过
         → 自动推进到下一 phase（不停下来等你）
```

你不需要做任何事，AI 会自己跑完。你可以看到 AI 输出联审过程。

### 3.5 看联审结果

每个审核点联审完成后，AI 会调用：
```bash
ae-sdd state register-review-consensus --point 1 --passed true --rounds 1
```

你可以随时查当前联审状态：
```bash
ae-sdd state read    # 看 state.json，含 reviewConsensus 字段
```

`reviewConsensus` 字段示例：
```json
{
  "1": {
    "point": 1,
    "tier": 3,
    "passed": true,
    "rounds": 1,
    "reviewers": [
      {"agentId": "rev-1", "role": "story-reviewer", "verdict": "pass", "sessionId": "sid-A"},
      {"agentId": "rev-2", "role": "story-reviewer", "verdict": "pass", "sessionId": "sid-B"},
      {"agentId": "rev-3", "role": "story-reviewer", "verdict": "pass", "sessionId": "sid-C"}
    ],
    "stallReason": "",
    "recordedAt": "2026-07-03T..."
  }
}
```

### 3.6 什么时候你要自己手动注册联审结果？

正常情况下 **AI 自己调 `register-review-consensus`**，你不用管。但以下 3 种情况你要手动调：

**情况 1：AI 漏调了（G-AUTO-CONSENSUS 门禁阻断）**

AI 跑完联审但忘了写 state → 下个 phase 切不过去，报：
```
❌ G-AUTO-CONSENSUS 自动化联审共识通过: 自动化模式 review 节点但未写 reviewConsensus
```

你手动补写：
```bash
ae-sdd state register-review-consensus --point 1 --passed true --rounds 1
```

**情况 2：你想覆盖 AI 的判定**

AI 判 passed=true 但你不认可（比如 reviewer 报告里有未解决的 🟠 严重型问题）：
```bash
ae-sdd state register-review-consensus --point 2.5 --passed false --rounds 3 --stall-reason "CodingModel 决策缺理由，用户不认可"
```

写 passed=false 后流程会暂停（按 onConsensusStall 策略），等你处理。

**情况 3：带详细 reviewer 信息写入**（审计追溯用）

```bash
ae-sdd state register-review-consensus \
  --point 4 \
  --passed true \
  --rounds 2 \
  --reviewers "rev-1|code-reviewer|pass|sid-A,rev-2|code-reviewer|pass|sid-B,rev-3|code-reviewer|pass|sid-C"
```

`--reviewers` 格式：`agentId|role|verdict|sessionId`，多个用逗号分隔。

**命令参数说明**：

| 参数 | 必填 | 说明 |
|------|------|------|
| `--point` | 是 | 审核点编号（1/1.5/2/2.5/4/5）|
| `--passed` | 是 | `true`/`false` |
| `--tier` | 否 | 默认 3（自动化模式固定）|
| `--rounds` | 否 | 矫正轮次，默认 1 |
| `--reviewers` | 否 | reviewer 摘要列表（`agentId\|role\|verdict\|sessionId` 逗号分隔）|
| `--stall-reason` | 否 | passed=false 时的原因 |
| `--story` | 否 | Story ID（多 Story 时指定写哪个 state）|

---

## 4. 联审不通过怎么办？

### 4.1 单轮不通过（正常修复循环）

某 reviewer 发现 🔴 阻断型缺陷 → AI 自动回到该系列步骤 2 修复 → 重新派 reviewer 审。这是正常循环，不需要你介入，**最多 3 轮**。

### 4.2 3 轮未决 → 暂停（默认出口）

连续 3 轮仍有未解决缺陷 → AI 执行：
```
state.phase = paused
pauseReason = "consensus-stall"
```

输出完整问题清单：
```
【自动化模式 — 联审共识停滞】
审核点 2.5（CodingPlan 评审）连续 3 轮未通过
未决问题：
  1. [🔴] Task-2 事务边界未明确（reviewer-业务视角 提出，3 轮未修复）
  2. [🟠] CodingModel 决策"乐观锁"缺理由（reviewer-架构视角 提出）
请处理后说「回归流程」继续，或调整 automation.onConsensusStall 策略
```

**你的动作**：
- 看问题清单，决定怎么处理（给信息 / 改设计 / 接受风险）
- 处理后对 AI 说「回归流程」继续

### 4.3 想改成"失败即终止"？

编辑 `.ae-sdd/config.yaml`：
```yaml
automation:
  onConsensusStall: fail   # pause 改 fail
```

之后 3 轮未决 → AI 直接标记失败终止流程，不暂停。**不推荐**——暂停让你有机会介入修复，失败则要重头开始。

---

## 5. 常见场景

### Q1：我中途想加一个审核点回人工

可以。编辑 `.ae-sdd/config.yaml` 把该点从 `automatedReviewPoints` 移除，下次到该点就回退人工。已通过的联审结果不影响。

### Q2：联审的 3 个 reviewer 是真独立还是 AI 假装的？

**物理独立**。G-09B 门禁机械校验 `state.activeAgents` 有 ≥3 个 `sessionId ≠ root` 的 reviewer。AI 自扮（sessionId=root）或不派够 → 门禁阻断，流程进不去下一步。

### Q3：我的环境不支持派物理 sub-agent（单 session）

自动化模式**禁用逻辑多视角降级**（同一 AI 切换视角跑 3 遍不算真联审，盲区未消除）。环境不支持时：
- 默认 `onConsensusStall: pause` → 到审核点会暂停，提示你"环境不支持物理 sub-agent"
- 建议：换支持 spawn 的环境（Claude Code / Codex / Harness），或保持默认模式（人工✅）

### Q4：预收集漏了信息，开工后发现还要补

罕见但可能（如运行时才暴露的对接方信息）。此时 AI 会：
- 标记 `{待确认}` 并继续（不阻断流程）
- 或在审核点联审时被 reviewer 揪出来（视为缺陷）

不会中途打断你问信息——预收集阶段已经把"开工前能想到的"都问过了。

### Q5：开启自动化后想关掉

```bash
ae-sdd automation disable
```

立即生效，下一个审核点回退人工。已写过的 `reviewConsensus` 记录保留（审计用），不影响后续。

### Q6：多个 WorkItem / Story 并行，自动化配置是项目级还是 WorkItem 级？

**项目级**。`.ae-sdd/config.yaml` 是项目实例配置，所有 WorkItem / Story 共享同一开关。想让某 WorkItem 走人工、其他走自动化？目前不支持，建议按时间分段：自动化期间跑一批，关掉后再跑需人工把关的。

---

## 5.7 用户命令速查表

把你能用的命令集中列出来（AI 也会调这些，但你随时可手动调）：

| 命令 | 用途 | 何时用 |
|------|------|--------|
| `ae-sdd automation status` | 查看自动化配置 | 开启前/后确认配置 |
| `ae-sdd automation status --json` | 结构化输出（脚本用）| 脚本读取配置 |
| `ae-sdd automation enable` | 开启全自动化 | 决定走自动化时 |
| `ae-sdd automation disable` | 关闭自动化 | 想回退人工审核 |
| `ae-sdd preflight collect` | 开工前信息预收集 | 开工前自己先跑一遍看缺什么 |
| `ae-sdd preflight collect --json` | 结构化输出 | 脚本处理待补清单 |
| `ae-sdd state read` | 读 state.json | 查当前 phase / reviewConsensus |
| `ae-sdd state register-review-consensus --point {N} --passed {true\|false}` | 手动写联审结果 | AI 漏调/你想覆盖判定（见 §3.6）|
| `ae-sdd gates check --only G-AUTO-CONSENSUS` | 单测联审共识门禁 | 排查"为什么 phase 推不过去" |
| `ae-sdd gates check --only G-09B` | 单测 reviewer 独立性门禁 | 排查"reviewer 不独立"阻断 |
| `ae-sdd gates check --only G-00` | 项目资产门卫 | 开启前自检资产齐备 |

**手动编辑 config.yaml 的字段**（改完立即生效，无需重新 enable）：
- `preflightInfoCollection`（true/false）
- `onConsensusStall`（pause/fail）
- `automatedReviewPoints`（[1, 1.5, 2, 2.5, 4, 5] 增删编号）

**禁止手改的字段**（必须用命令）：
- `enabled` → 用 `automation enable/disable`
- `enabledAt` → 由 `automation enable` 自动写

---

## 6. 关闭自动化

```bash
ae-sdd automation disable
```

预期输出：
```
✅ ⚪ 自动化模式已关闭（回退人工审核）
```

之后 6 个审核点恢复"等用户 ✅/⚠️/❌"行为。`enabledAt` 时间戳保留作审计痕迹。

---

## 7. 验证清单

开启后正式跑第一个任务前，建议自检：

- [ ] `ae-sdd automation status` 显示 🟢 已启用
- [ ] `automatedReviewPoints` 只含你想自动化的点
- [ ] `onConsensusStall` 是你想要的策略（pause / fail）
- [ ] `.ae-sdd/assets/<projectKey>.assets.md` 7 层索引齐备（`ae-sdd gates check --only G-00` 通过）
- [ ] 输入材料（PRD/DR/Story）就绪
- [ ] 运行环境支持派物理 sub-agent

---

## 8. 相关文档

| 文档 | 用途 |
|------|------|
| [`source/SKILL.md §🚀 自动化模式`](../SKILL.md) | AI 执行规范（流程编排层）|
| [`source/docs/ae-sdd-design.md §19`](ae-sdd-design.md) | 设计说明（设计+实现表）|
| [`source/skills/cross-cutting/agent-orchestration-skill.md §8.4`](../skills/cross-cutting/agent-orchestration-skill.md) | 多 reviewer 联审机制（Tier 判定 + 视角正交 + 交叉对比 + 降级规则）|
| [`source/HARNESS.md`](../HARNESS.md) HS-15 | 自动化模式 hook 规则声明 |
| [`source/CHANGELOG/2026-07-02-automation-switch.md`](../CHANGELOG/2026-07-02-automation-switch.md) | 变更记录 |
