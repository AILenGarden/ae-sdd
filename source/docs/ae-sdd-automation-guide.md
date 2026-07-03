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
- [ ] **运行环境支持派物理 sub-agent**（Claude Code / Codex / Mavis 等能 spawn 独立 session；单 session 环境见 §6 降级说明）
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

| 字段 | 默认值 | 含义 | 何时调整 |
|------|-------|------|---------|
| `enabled` | `false` | 总开关 | 用 `ae-sdd automation enable/disable` 切换，不手改 |
| `reviewerTier` | `3` | 联审强度（固定三审） | 暂不支持改，自动化模式强制 3 |
| `preflightInfoCollection` | `true` | 开工前信息预收集 | 想跳过预收集直接开工 → 改 `false` |
| `onConsensusStall` | `pause` | 联审 3 轮未决时的出口 | `pause`=暂停等用户 / `fail`=标记失败终止 |
| `automatedReviewPoints` | `[1, 1.5, 2, 2.5, 4, 5]` | 走自动联审的审核点白名单 | 想让某点仍人工 → 从数组移除该编号 |
| `enabledAt` | `""` | 开启时间戳（审计） | 不手改，由 CLI 写 |

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

之后 CodeReview 仍会停下来等你 ✅，其余点继续走联审。

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

**你的动作**：打开 `.ae-sdd/preflight-info.yaml`，把每个待补项填上实际值（或直接在对话里告诉 AI）。补齐后 AI 才进 Step 2 开工，**开工后不再因缺信息打断你**。

> 无待补信息时 AI 直接进 Step 2，不卡你。

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
- 建议：换支持 spawn 的环境（Claude Code / Codex / Mavis），或保持默认模式（人工✅）

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
