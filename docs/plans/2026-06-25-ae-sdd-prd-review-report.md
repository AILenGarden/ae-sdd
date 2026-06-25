# Review: PRD 级状态管理 + 流程级 compact 方案 A'

**Reviewer:** general agent (独立 reviewer)
**Review date:** 2026-06-25
**方案版本:** root agent 草案 v0.1（2026-06-25）
**最终判定:** ⚠️ **需修改**（战略方向正确，存在 4 项 🔴 阻断 + 7 项 ⚠️ 必改）

---

## 0. 一句话结论

方案方向正确（PRD 级聚合状态 + 流程级 compact 是 ae-sdd 适配多 runtime 的必要演进），但**违反 ae-sdd 现有 SKILL 边界判定规则**——流程状态机按 `ae-sdd-update-skill.md:188-192` 必须归属 `ae-sdd-skill.md`，方案草案缺这一环；同时把 Story 级 ⑦bis 直接平移到 PRD 级是语义错误；Codex runtime 自承"待调研"在交付前不可接受。

---

## 1. 🔴 阻断项（4 项，必须先解决才能进入实施）

### 🔴 R-1：状态机归属违规，违反 SKILL 边界判定表

**问题：** `ae-sdd-update-skill.md:188-192` 明确规定：

> | **流程状态机**（state.json 结构 / 流程脱离与再启动） | 全局状态 | `ae-sdd-skill.md` | 各子 SKILL |

方案草案第 0-4 步在"ae-sdd 主 SKILL 之外"独立发明了一套"PRD 级状态机"，未落到 `ae-sdd-skill.md §流程状态跟踪与再启动`（现 L1010-1111），导致：

- 各子 SKILL（phase1-design / phase2-coding / phase3-review）不知道 PRD 级状态机存在
- 任何"切换 Story" / "流程脱离"判定只能读到 Story 级 state.json，看不到 PRD 级上下文

**改进：** 新增 §"PRD 级状态跟踪与跨 PRD 流程脱离"章节到 `ae-sdd-skill.md`（建议插在 L1111 之后、§整体流程之前），由 ae-sdd-skill.md 单点持有 PRD 级状态机定义；子 SKILL 通过指针引用。

**来源：** `ae-sdd-update-skill.md:188-192` + `ae-sdd-skill.md:1010-1111`

---

### 🔴 R-2：Story 级 ⑦bis 直接平移为 PRD 级完成条件，语义错误

**问题：** 方案第 2 步"三层 AND（全部 Story 完成 + ⑦bis 通过 + 人工审核点 4 通过）"中的"⑦bis 通过"是 Story 级门禁——按 `coding-skill.md:1884-1921` 定义，⑦bis 是"DR-Story-Task-实现-测试用例 五层一一对应"，本身就是 Story 内部闭环。**PRD 级不存在这个五层对应**——一个 PRD 可能有 5 个 Story，PRD 级要查的是"DR-N × Story-N × Task-N × CodingPlan-N 全 PRD 范围五层贯通 + 跨 Story 依赖闭合 + 跨 Story 残留风险汇总"。

同样，"人工审核点 4"（`SKILL.md:1167`）是 Story 级 CodeReview 完成确认，不是 PRD 级。

**改进：** PRD 级完成判定 SOP 应重定义为：
1. **所有 Story.state == "completed"**（含 CodeReview 报告出具）
2. **每个 Story 自己的 ⑦bis 都通过**（不是 PRD 级 ⑦bis）
3. **新增 🔍 人工审核点 5：PRD 完成确认**（与现有 4 个审核点同级）
4. （可选）**PRD 级对称性闸：跨 Story 残留风险清单已生成并入 state.md**

新审核点 5 的"主动讲解"规范要复用 `SKILL.md:1300-1343` 模板（讲 PRD 业务全貌 + 跨 Story 决策 + 残留风险）。

**来源：** `coding-skill.md:1884-1921` + `SKILL.md:1167` + `SKILL.md:1296-1343`

---

### 🔴 R-3：Codex runtime hook 协议未验证就列为"已知阻断"

**问题：** 方案第 5 步"Codex 需要调研"沿用了 `discipline-hardening-plan.md:207-233` 已知风险，但二者性质不同：
- discipline-hardening 是 L4 失职检测 hook（pre-trigger），是"增强项"
- 方案 A' 的 Codex compact 钩子是"功能路径"，没有 fallback 就跑不通

**改进：** 必须先做 PoC（30 分钟即可）：
1. 查 Codex 官方文档是否有 Stop / PreToolUse hook 等价物（可能叫 `notify` / `event`）
2. 退而求其次：Codex 是否有 session export / context dump API，能让外部脚本在 PRD 完成后写出"状态移交包"
3. PoC 结论必须落在方案里：要么给出 Codex 适配路径，要么明确"Codex 暂不支持 PRD 级 compact，必须用户手动清空对话"

**来源：** `discipline-hardening-plan.md:207-233` + `HARNESS.md:101-128`（只描述 Claude Code 协议）

---

### 🔴 R-4：state.json 路径与 document-storage 命名约束冲突

**问题：** `document-storage-skill.md:494` 规定：
> | 流程状态文件 | 不带版本号 | `.auto-engineering/{STORY-ID}/state.json` | 状态实时变化 |

`ae-sdd-conventions.md:128` 实战路径是 `\.auto-engineering\STORY-{ID}-BE\`。方案把目录名改为 `{PRD-ID}`：
- 没有先在 `document-storage-skill.md` 加一行 `| 流程状态文件（PRD 级）| 不带版本号 | .auto-engineering/{PRD-ID}/state.json |` 的规范
- `{PRD-ID}` 与 `{STORY-ID}` 同层目录冲突（如 PRD-CS-001 vs STORY-002-BE），命名空间没隔离规则
- 没有 `{PRD-ID}` 命名规范（document-storage 当前没有 PRD ID 命名规范，PRD 路径只有文件级 `{PRD-ID}.md`）

**改进：**
1. 先在 `document-storage-skill.md §2.5 流程状态文件路径表`加一行 PRD 级规范
2. 在 `ae-sdd-conventions.md §3 路径速查`加一行
3. PRD ID 命名需统一：`PRD-<业务域>-<序号>` 与 DR ID 格式对齐（参考 `dr-review-skill.md:184` `DR-<PRD-ID>-<序号>`）

**来源：** `document-storage-skill.md:494` + `ae-sdd-conventions.md:128` + `dr-review-skill.md:184`

---

## 2. ⚠️ 必改项（7 项，可在 R-1~R-4 解决后批量修）

### ⚠️ R-5：state.json 结构缺字段，破坏 v3.2.3 memory lifecycle gate

`memory-management-skill.md:32-34` + `SKILL.md:2022-2024` v3.2.3 起：
> `ae-sdd state write --phase <next>` checks the current associated node's memory lifecycle before changing phase. The transition is blocked when the current phase has no matching `memory enter` and later `memory write`.

PRD 级 state.json 必须包含：
- `memoryEnterHistory[]`：记录每次进入 PRD 节点前 memory enter
- `memoryWriteHistory[]`：每次输出后 memory write
- `memoryExitHistory[]`：每次离开前 memory exit
- `prdCompleteGate` 字段（不是字符串，而是对象：含 `allStoriesCompleted` / `lastStorySevenBisPassed` / `userConfirmedAt` / `prdSevenBisPassed`）

`prdStatus` 枚举不完整：缺 `prd_complete_pending_user`、`compact_failed`、`compact_retrying`、`prd_aborted`。

### ⚠️ R-6：CLI 命令命名风格与现有不一致

现有 8 个子命令（`SKILL.md:1996-2006`）是单词：`assets` / `state` / `classify` / `gates` / `sync-tools` / `memory` / `db` / `git`。

方案用 `prd complete` / `prd check-complete` 是 git subcommand 嵌套风格。两种选择：
- **A：** 单层命名 `ae-sdd prd-complete` / `ae-sdd prd-check-complete`（贴近现有风格）
- **B：** 走 `state` 子命令加 PRD 维度：`ae-sdd state prd-complete`（最大复用现有 command 树）

建议 B：`ae-sdd state prd-complete` 与 `ae-sdd state next-step` 同级，对 `state` 子命令加 `--scope=prd` 选项最干净。

### ⚠️ R-7：流程脱离 SOP 缺第 5 场景

`SKILL.md:1053-1099` 列了 4 场景（偏离流程 / 继续 / 重启 / 切换 Story）。`SKILL.md:1103-1111` 再启动判定规则表也只对应 4 行。

需要新增：
- **场景五：用户明确说"PRD 收尾 / 进入下一个 PRD"** —— 触发 `ae-sdd state prd-complete` 流程
- 再启动判定规则表新增一行：`PRD 完成 / 进入下一个 PRD | 读 .auto-engineering/{PRD-ID}/state.json，校验 prdStatus 状态，触发 PRD 级 compact`

### ⚠️ R-8：HARD STOPS 缺一条

`HARNESS.md:77-85` 列了 HS-1~HS-6。需要新增：
- **HS-7** 未通过 PRD 级完成判定三重闸就触发 `ae-sdd state prd-complete`（PreToolUse hook 物理拦截）
- **HS-8** PRD 级 compact 失败时未保留旧 PRD state.json（Stop hook 检查残留状态）

### ⚠️ R-9：Harness 适配层必须重生成

`ae-sdd-update-skill.md:65,158-165` 明确：
> ⑥ Harness 适配层 = `harness/.harness/agent.md` + `.adapter.lock`，由 ae-sdd-harness-adapter SKILL 自动生成
> ❌ 不手工改；母版升级后重跑 adapter SKILL 重新生成

方案新增 PRD 级状态机后，必须：
1. 改 `source/SKILL.md` + `source/HARNESS.md`（母版）
2. 重跑 `ae-sdd-harness-adapter`（`convert-ae-sdd-to-harness.ps1`）
3. 检查 `.adapter.lock` 的 commit hash 已更新

否则 Mavis 团队级 agent 完全看不见 PRD 级状态机。

### ⚠️ R-10：CHANGELOG + update-graph.json + README:5 同步清单

按 `ae-sdd-update-skill.md:280-309`：
1. `README.md:5` 的版本号行必须更新（`+ PRD 级状态机 + 流程级 compact`）
2. `CHANGELOG/2026-06-25-PRD-级状态机-流程级compact.md` 必须新建（按 `:296` 命名规范）
3. `source/standards/update-graph.json` 必须新增 PRD 级 state.json / state.md / SOP 的依赖条目，否则 `ae-sdd update-check` UC-01/UC-05 会报"未跟踪文件"
4. SKILL 边界判定表（`ae-sdd-update-skill.md:188`）必须新增一行 PRD 级状态机归属条目

### ⚠️ R-11：state.md 与 state.json 二者职责不清

方案第 4 步同时生成 state.json（机器读）和 state.md（人类读 timeline），但内容有重叠——state.json 的 `storyIds[]` 已包含每个 Story 的状态摘要，state.md 再写"Story×N 完成情况"是冗余。

**建议：** state.md 只做"人类可读的故事叙述"——按 PRD 业务顺序讲清"为什么这样设计 + 跨 Story 关键决策 + 残留风险"，不重复 state.json 的字段；state.json 末尾加 `_summaryPath` 指向 state.md。

---

## 3. 🟢 建议项（5 项，提升可维护性）

### 🟢 R-12：字段命名统一性

state.json 用 `prdId` / `prdTitle` / `prdPath` / `drId` 风格不统一——`prdPath` 是文件路径（snake_case 风格），`prdId` / `prdTitle` 是 camelCase 风格。建议统一 camelCase：`prdId` / `prdTitle` / `prdDocPath` / `drId` / `storyIds`。

### 🟢 R-13：compact 触发 SOP 步骤 4 应拆 check 与 execute

方案步骤 4 把"校验 + 触发 compact"塞在 `ae-sdd prd complete` 一个命令里。建议拆：
- `ae-sdd state prd-check-complete`：只校验三重闸，输出未达成项，**不修改状态**
- `ae-sdd state prd-complete`：校验通过后执行 compact，更新 prdStatus

符合 ae-sdd 现有"check 类与 write 类命令分离"风格（参考 `ae-sdd gates check` vs `state write`）。

### 🟢 R-14：compactHistory 归档策略

第 3 阻断项"compactHistory 默认保留最近 5 次 + 老的历史归档到 state.archive.json"会增加复杂度。建议改为：`compactHistory` 默认保留全部，提供 `ae-sdd state prd-archive --prd {PRD-ID} --keep-last 5` 手动归档。

### 🟢 R-15：multi-runtime 适配层抽象

方案第 5 步把 runtime-specific compact 钩子直接写在 `prd complete` 命令里。建议新增 `ae-sdd runtime compact --runtime <mavis|claude|codex>` 子命令作为适配层，`prd complete` 内部调用 `runtime compact`，未来加新 runtime 不用改 prd-complete 逻辑。

### 🟢 R-16：state.md 模板归属

PRD 级 state.md 没有对应模板（`templates/design/prd-summary-template.md` 不存在）。建议先建模板再实施，否则不同 reviewer 写出的 state.md 结构差异巨大。

---

## 4. 与 ae-sdd 现有规则兼容性检查（逐条 cite）

| 现有规则 | 方案对齐情况 | cite |
|---------|------|------|
| 流程状态机必须归属 `ae-sdd-skill.md` | ❌ 未在 SKILL.md 落档 | `ae-sdd-update-skill.md:188-192` |
| state.json 路径规范 `.auto-engineering/{ID}/state.json` | ⚠️ PRD 级未规范 | `document-storage-skill.md:494` |
| memory enter/write/exit 三件套 mandatory | ❌ state.json 无 memory lifecycle 字段 | `memory-management-skill.md:32-34` + `SKILL.md:2022-2024` |
| ⑦bis 是 Story 级门禁 | 🔴 直接平移为 PRD 级语义错 | `coding-skill.md:1884-1921` |
| 4 个 🔍 人工审核点 | ❌ 新增 PRD 级审核点未编号 | `SKILL.md:1167, 1296-1343` |
| 流程脱离 4 场景 | ⚠️ 缺"PRD 完成"第 5 场景 | `SKILL.md:1053-1099` |
| 再启动判定规则表 4 行 | ⚠️ 缺"PRD 完成"行 | `SKILL.md:1103-1111` |
| 6 HARD STOPS | ⚠️ 缺 PRD complete 拦截 | `HARNESS.md:77-85` |
| 三层 hook (PreToolUse/UserPromptSubmit/Stop) | ⚠️ PRD compact 路径未走 hook | `HARNESS.md:101-128` |
| 改 SKILL.md → 同步 README §3/§4.2/§8.5/§5 | ⚠️ 同步清单未列 | `ae-sdd-update-skill.md:280-289` |
| 改 SKILL → 写 CHANGELOG | ⚠️ 同步清单未列 | `ae-sdd-update-skill.md:291-309` |
| Harness 适配层必须重生成 | ⚠️ 同步清单未列 | `ae-sdd-update-skill.md:65,158-165` |
| update-graph.json 跟踪文件依赖 | ⚠️ 同步清单未列 | `ae-sdd-update-skill.md:117` |
| CLI 命令命名风格（单词/层级） | ⚠️ 命名风格不一致 | `SKILL.md:1996-2006` |
| Codex hook 协议未验证 | 🔴 自承待调研 | `discipline-hardening-plan.md:207-233` |
| DR ID 格式 `DR-<PRD-ID>-<序号>` | ✅ PRD ID 命名需对齐 | `dr-review-skill.md:184` |
| PRD 路径 `{工程根}/ae-sdd-doc/PRD/{PRD-ID}.md` | ✅ state.json 应引用此路径 | `requirement-analysis-skill.md:42` |
| PRD → RA → DR → Story×N → Task×N → CodingPlan×N | ✅ state.json 结构覆盖此树 | `requirement-analysis-skill.md:87` |
| B-PRD 跨 PRD 切片检查 | ✅ 切片 ≤3 是 STORY 级检查，PRD 级不冲突 | `story-review-skill.md:591-601` |

---

## 5. 三 Runtime 适配矩阵评估

| Runtime | 已有能力 | PRD complete 触发 compact 的可行路径 | 风险等级 |
|---------|---------|-------|------|
| **Mavis** | `mavis session rotate --handoff-file`（同 synchronous rotation）<br>`mavis memory append/read/write`<br>`mavis communication send` | ❌ 方案提到 `mavis session compact` 不存在；<br>✅ 应改为 `mavis session rotate --handoff-file .auto-engineering/{PRD-ID}/state.md`（rotate 已有，handoff file 必需，state.md 直接做移交包） | 🟢 可行，但方案描述错误必须改 |
| **Claude Code** | PreToolUse + UserPromptSubmit + Stop hook（`HARNESS.md:101-128`）<br>Stop hook 当前检查 `◆ STATE:` 标记 | ⚠️ 方案"emit stop hook signal"在 PRD 级不写 STATE 头；<br>✅ 可行路径：让 `state prd-complete` 命令触发 Bash 写入 `state.md`，下次 prompt 通过 `prompt-inject` hook 自动注入 PRD 完成提示 + state.md 路径（复用现有 UserPromptSubmit 协议） | 🟡 可行但需改造 STATE 头注入逻辑 |
| **Codex** | 完全未验证 | ❌ 方案自承"待调研"；<br>🛑 阻断，必须先 PoC，否则 PRD 级 compact 在 Codex runtime 下根本无法跑通 | 🔴 阻断 |

**适配矩阵整体结论：** Mavis 路径可行但方案命名错误；Claude Code 可行但需改造；Codex 必须先 PoC 才能确定能否落地。

---

## 6. 最终判定

### ⚠️ 需修改

**战略方向正确，落地路径需重大修订。**

实施前必须解决的 4 项 🔴 阻断：
1. R-1：PRD 级状态机必须先在 `ae-sdd-skill.md §流程状态跟踪与再启动` 落档
2. R-2：重定义 PRD 级完成判定 SOP（不能直接套 Story 级 ⑦bis 和人工审核点 4）
3. R-3：Codex runtime 必须先做 PoC，要么给出适配路径要么明确"不支持"
4. R-4：先在 `document-storage-skill.md §2.5` 加 PRD 级 state.json 路径规范 + PRD ID 命名规范

实施时必须解决的 7 项 ⚠️ 必改：R-5 ~ R-11

可后续优化的 5 项 🟢 建议：R-12 ~ R-16

**关键里程碑建议：**
- **M1（阻断解决）：** 解决 R-1 ~ R-4，预计 4-6 小时（含 Codex PoC）
- **M2（必改落地）：** R-5 ~ R-11，预计 6-8 小时（含 SKILL.md/HARNESS.md/README.md/CHANGELOG/update-graph 同步）
- **M3（建议优化）：** R-12 ~ R-16，预计 2-3 小时
- **M4（验证）：** `ae-sdd update-check` 全绿 + `dev-sync.sh` + harness 重生成 + `ae-sdd health` 9 项

---

## 7. 给 root agent 的下一步建议

不要直接给用户报方案 A'。先回用户三件事：
1. **🔴 阻断项 4 项 + ⚠️ 必改项 7 项**（已写完）
2. **问用户一个边界问题：** Codex runtime 是 PoC-first 还是 fallback 到"用户手动清空"？
3. **Mavis runtime 命名修正：** 方案中的 `mavis session compact` 不存在，应改为 `mavis session rotate --handoff-file <state.md>`

review 双方满意后 → 让用户拍板"是 / 否 / 修改哪条"，再交付实施计划。

---

## 8. 关联文档回链

- 母版主入口：`D:\Item\ae-sdd\source\SKILL.md`（L1010-1111 状态跟踪；L1153-1167 ⑦bis；L1991-2025 CLI；L2074-2122 同步机制）
- 边界判定表：`D:\Item\ae-sdd\source\skills\orchestration\ae-sdd-update-skill.md:188-222`
- 路径规范：`D:\Item\ae-sdd\source\skills\cross-cutting\document-storage-skill.md:494`
- memory 强制：`D:\Item\ae-sdd\source\skills\cross-cutting\memory-management-skill.md`
- ⑦bis 触发：`D:\Item\ae-sdd\source\skills\phase2-coding\coding-skill.md:1884-1921`
- PRD 路径：`D:\Item\ae-sdd\source\skills\phase1-design\requirement-analysis-skill.md:42,87`
- 切片检查：`D:\Item\ae-sdd\source\skills\phase1-design\story-review-skill.md:591-601`
- Codex 风险：`D:\Item\ae-sdd\source\docs\plans\2026-06-22-discipline-hardening-plan.md:207-233`
- Harness：`D:\Item\ae-sdd\source\HARNESS.md:31-128`
- 6 子系统：`D:\Item\ae-sdd\source\skills\orchestration\ae-sdd-update-skill.md:61-65`

---

## 9. 补充章节（按 root agent 强调的 5 个特定维度）

### 9.1 PRD 完成判定的三层 AND 是否合理？

**结论：三层 AND 不合理，需要重构成"四层 AND + 跨 Story 闸"。**

原方案：
```
1. 所有 story.state = "completed"
2. 最后 Story 的 ⑦bis 对称性闸 通过
3. 人工审核点 4 通过
```

**问题逐条分析：**

**(1) "所有 story.state = completed" 过于宽松**
- `state.json` 的 `currentPhase = "completed"` 只是流程节点完成，不等于 CodeReview 通过 + ⑦bis 通过
- 实际应该查 `state.codeReviewReport` 字段非空 + `state.sevenBisPassed = true`（字段需要新增，参考 coding-skill.md:1884-1921）
- 必须每个 Story 的 CodeReview 报告路径都存在（`.auto-engineering/{STORY-ID}/CR-r{n}.md`）

**(2) "最后 Story 的 ⑦bis 通过" 是单 Story 概念，不是 PRD 级**
- coding-skill.md:1884-1921 ⑦bis 是 Story 内的 DR-Story-Task-实现-测试用例 五层
- PRD 级没有"最后 Story"概念（多个 Story 并行完成时谁是"最后"？）
- 应该改成"所有 Story 的 ⑦bis 都通过"——这是一个 O(N) 校验

**(3) "人工审核点 4 通过" 是 Story 级**
- SKILL.md:1167 人工审核点 4 是 Story 级 CodeReview 完成确认
- PRD 级需要新增 **🔍 人工审核点 5：PRD 完成确认**（与现有 4 个审核点同级）

**重构建议（四层 AND + 跨 Story 闸）：**

```
G-PRD-1 (Story 全部完成):
  ∀ STORY-ID ∈ prdState.storyIds:
    story.state.codeReviewReport 存在
    ∧ story.state.sevenBisPassed == true
    ∧ story.state.userConfirmedAt 非空

G-PRD-2 (Story ⑦bis 全通过):
  ∀ STORY-ID: story.sevenBisMatrix 无 🔴 断链
  ∧ ∀ STORY-ID: story.sevenBisMatrix 出闸条件满足

G-PRD-3 (跨 Story 残留风险已闭环):
  crossStoryDeps 中每个 dependency 都有 targetStoryCodeReviewReport 引用
  ∧ crossStoryResidualRisks[] 中每条 risk 都有 owner + dueDate

G-PRD-4 (PRD 级人工审核通过):
  prdReviewConfirmedAt 非空（人工审核点 5）
  ∧ prdReviewConfirmedBy 有用户标识
```

**人工审核点 5 的设计（新增）：**
- 触发时机：G-PRD-1/2/3 全过 + 用户说"PRD 收尾了"
- AI 必须先用讲故事笔法讲清 PRD 业务全貌（参考 SKILL.md:1300-1343 模板）
- 讲解必须覆盖：跨 Story 关键决策 + 跨 Story 残留风险 + sizeBudget 实际消耗 vs 估算
- 用户说"确认"后写 `prdReviewConfirmedAt` + `prdReviewConfirmedBy`

**来源：** `SKILL.md:1167` + `SKILL.md:1296-1343` + `coding-skill.md:1884-1921` + `dr-generate-skill.md:466-468`（跨 Story 状态机粒度）

---

### 9.2 state.json schema 字段够不够用？

**结论：草案字段不够，需要新增至少 5 个核心字段 + 3 个 runtime 适配字段。**

**当前草案字段（10 个）：**
```
prdId / prdTitle / prdPath / drId / storyIds[] / prdStatus /
lastUpdated / compactHistory[] / storyIds[].storyId/state/taskIds/
codingPlanIds/codeReviewReport/completedAt
```

**缺的字段（按优先级）：**

#### 🔴 必须新增（5 个核心字段）

**(1) `crossStoryDeps[]` — 跨 Story 依赖图**
- 来源：`dr-generate-skill.md:466-468` 已规定"只写跨 Story 的状态机和业务规则"
- 结构：
```json
{
  "fromStory": "STORY-002-BE",
  "toStory": "STORY-007-FE",
  "depType": "data | api | interface",
  "critical": true,
  "verifiedAt": null,
  "verifiedBy": null
}
```

**(2) `crossStoryResidualRisks[]` — 跨 Story 残留风险清单**
- 来源：`SKILL.md:1153-1167` + `dr-update-skill.md:113-132` 已要求跟踪 Story 间影响
- 结构：
```json
{
  "riskId": "RISK-PRD-CS-001-001",
  "description": "...",
  "owner": "...",
  "severity": "🔴|🟠|🟡|🟢",
  "dueDate": "...",
  "mitigationPlan": "..."
}
```

**(3) `sizeBudget` — PRD 级规模预算与实际消耗**
- PRD 启动时估算 story 数 + 每个 story 的 task 数 + 每个 task 的 estimatedHours
- 实际完成时记录 actualHours
- 用于 PRD 完成判定时讲清"实际 vs 估算"
- 结构：
```json
{
  "estimated": { "storyCount": 5, "taskCount": 18, "hours": 240 },
  "actual": { "storyCount": 6, "taskCount": 22, "hours": 310 },
  "variance": { "storyCountPct": 20, "taskCountPct": 22, "hoursPct": 29 }
}
```

**(4) `prdReview` — PRD 级人工审核点 5 记录**
- 与 Story 级 `userConfirmedAt` 同模式
- 结构：
```json
{
  "confirmedAt": "2026-06-25T...",
  "confirmedBy": "user-id",
  "storytoldAt": "...",
  "openQuestions": []
}
```

**(5) `memoryLifecycle` — memory enter/write/exit 三件套追踪**
- 来源：`memory-management-skill.md:32-34` + `SKILL.md:2022-2024` v3.2.3 起强制
- 结构：
```json
{
  "enterHistory": [{ "at": "...", "phase": "ra|design|coding-plan|coding|review|prd" }],
  "writeHistory": [{ "at": "...", "phase": "...", "kind": "decision|conflict|..." }],
  "exitHistory": [{ "at": "...", "phase": "..." }]
}
```

#### ⚠️ 强烈建议新增（3 个 runtime 适配字段）

**(6) `runtimeHooks` — runtime-specific compact 配置**
- 当前 compactHistory 只有 `runtime` 字段记录触发时 runtime，但没记录每个 runtime 的钩子配置
- 结构：
```json
{
  "mavis": { "compactCmd": "mavis session rotate", "args": ["--handoff-file", "{state.md}"] },
  "claude-code": { "hookType": "UserPromptSubmit", "injectCmd": "..." },
  "codex": { "compactCmd": null, "status": "unsupported", "fallback": "user-manual" }
}
```

**(7) `gateRegistry` — PRD 级闸门注册**
- 与 Story 级 gates check 同模式（`SKILL.md:2050` 14 门禁扫描）
- PRD 级闸门：G-PRD-1 ~ G-PRD-4（见 9.1）+ G-PRD-5（运行时一致性）

**(8) `version` + `schemaVersion` — schema 版本号**
- 与 document-storage-skill.md:494 流程状态文件"不带版本号"原则冲突——但 PRD 级 state.json 比 Story 级复杂得多，需要独立版本号以支持 schema 演进
- 建议：`schemaVersion: "1.0.0"`，每次字段新增需要 CHANGELOG + 迁移脚本

#### 🟢 可选新增（2 个）

**(9) `previousPrdId` — 上一个 PRD 引用**
- PRD 完成 compact 后进入下一个 PRD，需要追溯"上一个 PRD 收尾时遗留了什么"

**(10) `nextPrdId` — 预声明的下一个 PRD ID**
- 让 PRD 级 SOP 可以预热下一个 PRD 的 state.json 模板

---

### 9.3 Codex hook 阻断项：是独立调研、还是 defer 到 v3.3？

**建议：不要 defer 到 v3.3，必须在 v3.2.6（或独立 v3.2.x）做 PoC，再决定。**

**为什么不建议直接 defer：**
1. 方案 A' 的核心价值是"多 runtime 适配"，Codex 缺位 = 适配矩阵不完整
2. defer 到 v3.3 等于 1 个大版本的窗口期（按 v3.1 → v3.2 间隔 6-8 周看，v3.3 至少 1-2 个月后），期间任何 PRD 级 compact 方案在 Codex runtime 下不可用
3. ae-sdd discipline-hardening-plan.md:207-233 已经自承"Codex hook 协议未验证"超过 3 天（2026-06-22 → 2026-06-25），债不能继续滚

**PoC 路径（30 分钟，可独立于方案实施）：**

```bash
# 1. 查 Codex 官方文档 hook API
codex --help 2>&1 | Select-String -Pattern "hook|notify|event|trigger"
# 查 ~/.codex/ 是否有 hooks.json 之类

# 2. 找 Codex 团队/社区是否有类似 Claude Code PreToolUse/Stop hook
# 推荐: GitHub issue / docs.codex.com 搜 "hook protocol"

# 3. 退而求其次: Codex session 是否支持 export / dump context
codex session export --help 2>&1
codex session dump --help 2>&1

# 4. PoC 结论写入 ae-sdd PoC doc:
#   - 路径 A: Codex 有 hook → 给出 hooks.json 模板
#   - 路径 B: Codex 有 export → 用脚本生成 state.md 移交包
#   - 路径 C: 都没有 → 明确"Codex runtime 不支持 PRD 级 compact，用户手动清空对话"
```

**PoC 决策矩阵：**

| PoC 结论 | 方案 A' 影响 | 决策 |
|---------|------|------|
| Codex 有 hook | 完整 3 runtime 适配 | ✅ 推进 |
| Codex 有 session export | 2.5 runtime（Claude + Mavis 完整，Codex 半自动） | 🟡 推进但文档化降级 |
| 都没有 | 2 runtime + 1 fallback | 🟡 推进，Codex runtime 状态标 "manual" |

**PoC 不通过 → 方案可以发布，但 Codex runtime 必须显式标记 "manual compact required"，并在 install-skill.md §5.2 加 FAQ。**

**来源：** `discipline-hardening-plan.md:207-233` + `ae-sdd-install-skill.md:148-189`（FAQ 模式）

---

### 9.4 与 ae-sdd-update-skill.md L280 同步清单是否覆盖？

**结论：L280 同步清单不够，需要扩展。**

**ae-sdd-update-skill.md:280 当前同步清单：**
```
改 ae-sdd-skill.md（新增/删除流程节点、改路由场景、改角色库、改门禁数量）→ 同步更新 README.md §3/§4.2/§8.5 + README.md:5 版本号
```

**为什么不够：**

**(1) 同步条件不全**
- 现有条件只覆盖"改 ae-sdd-skill.md"
- 没覆盖"新增/修改 state.json schema" → 必须同步 update-graph.json
- 没覆盖"新增 SOP 文件" → 必须同步 SKILL 边界判定表
- 没覆盖"改 HARNESS.md" → 必须同步 harness/.adapter.lock
- 没覆盖"新增模板" → 必须同步 templates/ README

**(2) 同步目标不全**
- README.md:5 版本号格式 `> **版本：** YYYY-MM-DD（最新变更：...）` 容量有限
- PRD 级状态机改动涉及 6 个文件，单行"最新变更"装不下

**(3) update-graph.json 联动**
- `ae-sdd-update-skill.md:117,124` 提到 update-graph.json 是 update-check 的依据
- 任何 source/ 或 tools/ 改动都要先查连带项（`ae-sdd update-check --affected <file>`）
- 但 update-graph.json 自身没有"PRD 级文件"的注册项

**扩展建议（追加到 ae-sdd-update-skill.md:280 之后）：**

```markdown
#### 4.1 同步清单扩展（🆕 2026-06-25 PRD 级状态机引入后）

| 改动源 | 同步目标 | 验证命令 |
|--------|---------|---------|
| 新增/修改 `.auto-engineering/{PRD-ID}/*` schema | `source/standards/update-graph.json` 新增条目 | `ae-sdd update-check --affected <file>` |
| 新增 PRD 级 state.md 模板 | `source/templates/design/` + 同步 `document-storage-skill.md §2.5` 路径表 | `ae-sdd health` 第 5/9 项 |
| 新增 PRD 级 SOP（如 prd-complete） | `source/skills/orchestration/` 或 `source/SKILL.md` §PRD 级状态跟踪 | `ae-sdd gates check --gate G-PRD-*` |
| 改 HARNESS.md（新增 PRD 级 hard stop） | 重跑 `ae-sdd-harness-adapter` 检查 `.adapter.lock` commit hash | `git diff harness/.adapter.lock` |
| 新增 runtime 适配（如 Codex hook） | `ae-sdd-install-skill.md` §5 FAQ + README §6 平台支持矩阵 | `ae-sdd install --runtime codex --check` |
```

**source/standards/update-graph.json 必须新增条目（伪代码）：**

```json
{
  "prd-level-state": {
    "files": [
      "source/SKILL.md §PRD 级状态跟踪（新增）",
      "source/HARNESS.md HS-7/HS-8（新增）",
      "source/skills/orchestration/prd-complete-skill.md（新增，建议）",
      "source/templates/design/prd-summary-template.md（新增）"
    ],
    "siblings": [
      "source/standards/project-assets/prd-schema.json（新增）",
      "tools/lib/prd_state.py（新增）",
      "tools/tests/test_prd_state.py（新增）"
    ],
    "downstream": [
      "harness/.harness/agent.md（重生成）",
      "CHANGELOG/2026-06-25-PRD-级状态机.md（新增）",
      "README.md:5（版本号 + 最新变更）"
    ]
  }
}
```

**来源：** `ae-sdd-update-skill.md:117,124,280-309` + `ae-sdd-install-skill.md:148-189` + `SKILL.md:2110-2122`（health 9 项）

---

### 9.5 与现有 Story 级 state.json 的兼容策略：双写？迁移？

**结论：双写 → 单写迁移方案，不是简单的"双写一直保留"。**

**为什么不建议纯双写：**

`(1)` 双写让两个 state.json 互为镜像，任何字段变更要同步两次，容易漂移
`(2)` Story 级 state.json 已经有 8+ 字段（`SKILL.md:1025-1039`），完全镜像到 PRD 级是 N×8 冗余
`(3)` ae-sdd discipline-hardening-plan.md:148-178 的"项目级产物路径"已经踩过类似坑：实战路径与 SKILL.md 描述不一致导致 6 周才发现

**但也不建议一次性迁移（破坏现有 Story）：**
- life / icec-cloud-boss / icec-cloud-life 三个项目已有 `.auto-engineering/STORY-{ID}-BE/state.json` 几百份
- 一次性迁移要重写所有 state.json，回归风险极大

**推荐方案：三阶段渐进迁移**

**阶段 1（v3.2.x 新增 + 双写，可选）：**
- PRD 级 state.json 新增
- 每个 Story 完成时 Story 级 + PRD 级双写（PRD 级通过聚合生成，不复制源字段）
- Story 级 state.json 加一个 `prdId` 字段（双向锚定）
- 现有流程不变，只是 PRD 级 state.json 作为"视图"存在
- 适用：所有新启动的 PRD

**阶段 2（v3.3.x 单写过渡，可配置）：**
- 通过 `ae-sdd config set prd-level-state-mode single` 切换
- 单写模式：Story 级 state.json 的 `prdId` 字段强校验
- 用户可选保留 PRD 级 state.json 作为"事后查询视图"
- 适用：愿意接受新模式的团队

**阶段 3（v3.4.x 故事级 state.json 简化）：**
- Story 级 state.json 移除可从 PRD 级聚合的字段（仅保留 Story 内私有字段）
- 聚合字段从 PRD 级读取（`ae-sdd state read --story {STORY-ID} --scope prd`）
- 完整 schema 演进路径在 update-graph.json 中追踪

**Story 级 state.json 需要新增的字段（双写阶段）：**

```json
{
  "storyId": "STORY-002-BE",
  "prdId": "PRD-CS-001",        // 🆕 双向锚定
  "drId": "DR-CS-001-01",        // 🆕
  "storyOrderInPrd": 2,          // 🆕 PRD 内序号
  "prdReviewStatus": "pending"   // 🆕 用于 G-PRD-3 校验
}
```

**迁移 SOP（写入 ae-sdd-update-skill.md）：**

```markdown
### 状态文件迁移 SOP（Story 级 → PRD 级聚合）

**触发：** 项目启用 v3.2.x PRD 级状态机
**操作：**
1. 跑 `ae-sdd state migrate-to-prd --project {projectKey}`
2. 脚本扫描 `.auto-engineering/*/state.json`
3. 推断 prdId（从 `pendingOutputs.storyDoc` 路径解析）
4. 聚合到 `.auto-engineering/{PRD-ID}/state.json`
5. 每个 Story 级 state.json 加 `prdId` 字段（反向锚定）
6. 验证：`ae-sdd state validate --scope both`
7. 备份：`.auto-engineering/_archive/pre-prd-migration-{date}/`

**回滚：** 删 `.auto-engineering/{PRD-ID}/state.json` + 从备份恢复 Story 级 state.json
```

**来源：** `SKILL.md:1010-1049` + `document-storage-skill.md:494` + `ae-sdd-conventions.md:128` + `discipline-hardening-plan.md:148-178`（路径漂移教训）

---

## 10. 最终结论（v2 — 包含 root agent 强调的 5 个维度）

**最终判定：⚠️ 需修改（重要决策点升级到 6 个）**

**6 个必须用户拍板的关键决策：**

| # | 决策点 | 选项 A | 选项 B | 选项 C | 推荐 |
|---|--------|--------|--------|--------|------|
| D-1 | PRD 完成判定闸数 | 3 层 AND（草案） | **4 层 AND + 跨 Story 闸（建议）** | 5 层 + sizeBudget 闸 | **B** |
| D-2 | state.json 新增字段 | 仅草案 10 字段 | **5 核心 + 3 runtime 字段（建议）** | 全 10 字段 | **B** |
| D-3 | Codex runtime | **PoC-first（建议）** | Defer 到 v3.3 | 直接 fallback "用户手动" | **A** |
| D-4 | Story 级兼容 | 双写一直保留 | **三阶段渐进迁移（建议）** | 一次性迁移 | **B** |
| D-5 | 同步清单扩展 | 不扩 L280 | **扩 L280 加 4.1 子节（建议）** | 重写 L280 | **B** |
| D-6 | Mavis compact 路径 | 草案 `mavis session compact` | **`mavis session rotate --handoff-file`（已存在）** | 新增 `mavis session compact` 命令 | **B** |

**给用户的话术建议（root agent 转达）：**

> 方案 A' 战略方向对，但有 4 个 🔴 阻断 + 7 个 ⚠️ 必改。需要你拍板 6 个关键决策：
> 
> 1. PRD 完成判定用 3 层 AND 还是 4 层 + 跨 Story 闸？（B 推荐）
> 2. state.json 加多少字段？（5+3 推荐）
> 3. Codex 怎么走？PoC-first / v3.3 defer / 直接手动？（PoC-first 推荐，30 分钟可搞定）
> 4. Story 级 state.json 怎么过渡？双写 / 三阶段 / 一次性？（三阶段推荐）
> 5. ae-sdd-update-skill.md L280 同步清单要扩吗？（扩成 L4.1 推荐）
> 6. Mavis runtime 用哪个命令？rotate --handoff-file（已存在）vs 新加 compact（rotate 推荐）
> 
> review 双方满意后再交付实施计划。完整 review 报告在 scratchpad。

---

## 11. 关联文档回链（v2 扩展）

新增 cite：
- 跨 Story 依赖：`source/skills/phase1-design/dr-generate-skill.md:466-468`
- 残留风险：`source/skills/phase1-design/dr-update-skill.md:113-132`
- Story 模板：`source/templates/design/be-story-template.md:45`
- update-graph：`source/standards/update-graph.json`（无具体行号，按需 grep）
- Codex FAQ：`source/skills/orchestration/ae-sdd-install-skill.md:148-189`
- 健康度 9 项：`source/SKILL.md:2110-2122`
- 迁移教训：`source/docs/plans/2026-06-22-discipline-hardening-plan.md:148-178`

