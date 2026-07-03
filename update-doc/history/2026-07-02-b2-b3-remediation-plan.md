# B-2/B-3 阻断级缺口复核与修订方案

> **起草日期**：2026-07-02
> **目标版本**：v3.8.1（建议编号，紧随当前 v3.8.0）
> **起草人**：Claude Opus 4.8（AI 协作分析）
> **状态**：待用户确认
> **目标读者**：ae-sdd 维护者

---

## 0. TL;DR

`update-doc/2026-06-26-ae-sdd-automation-gap-analysis.md` 列出的 3 项🔴阻断级缺口，复核后结论：

- **B-1**（entry token / gate ra-required 未实装）：**已修复**，`cmd_enter` 和 `check_ra_required` 均为完整实现，不再处理。
- **B-2**（G-DOC-STORAGE 物理拦截缺失）：**核心缺口已在今天（2026-07-02）由 commit `b864f9d` 修复**——`gate_intercept.py` 新增 `_check_product_storage_path`，写入前能真实拦截"CodingPlan 写到 d:\tmp\"这类场景。修复过程复用了 `paths.resolve_doc_workspace`，未产生重复路径规则。但 `gates.py` 的 `check_g_doc_storage`（事后兜底层）仍留 2 处次要技术债，本文档给出修复方案。
- **B-3**（人工审核点格式无强校验）：**原样存在，从诊断报告写下那天到今天未被任何改动触及**。这跟 v3.6 决策1B（废弃 gate 自报交叉验证）是两条平行线，不是因果关系——决策1B处理的是"phase 级宏观门禁的自报是否属实"，B-3 要处理的是"审核点这一动作本身呈现的内容格式是否达标"，两者从未被同一机制覆盖过。本文档给出完整修复方案。

---

## 1. 背景与复核结论

### 1.1 B-1：已修复，不再处理

`tools/bin/ae-sdd:1427 cmd_enter` 和 `tools/lib/gates.py:986 check_ra_required` 均为完整实现，写入 `session.json` / 校验 RA 文档存在性的逻辑齐全，`update-check` UC-01~13 全绿。诊断报告这条已过时。

### 1.2 B-2：核心缺口已修复，遗留 2 处次要技术债

**原诊断（2026-06-26）**：AI 未调 `resolve_path`，CodingPlan 写到 `d:\tmp\`，G-DOC-STORAGE 未能拦截。

**复核发现的时间线**：v3.7.2（2026-07-01）的 document-storage 激活 stage3/4，只做了"给已有 deny 判定追加提示文案"和"gates.py 事后校验加真值比对"，**没有堵上"写入前物理拦截"这个核心环节**——这是原诊断在 2026-07-01 之后依然成立的真实原因。真正的物理拦截由今天下午的 `b864f9d`（"Fix update skill cascade checks"）补上：

```
tools/lib/gate_intercept.py 新增 _check_product_storage_path()
  → 读 .ae-sdd/config.yaml 拿 projectKey
  → 调 paths.resolve_doc_workspace(ade_sdd, project_key) 拿 docWorkspacePath
  → 校验目标文件是否 relative_to {doc_workspace}/ae-sdd-doc/
  → 不满足 → 阻断，提示改用 `ae-sdd doc save`
  → 在 _check_product_landing() 最前置调用（第 264-267 行）
```

复用了 `document_storage.py:resolve_path` 同一套 `paths.resolve_doc_workspace`，两处路径规则已统一，不存在"各写一份逐渐漂移"的问题。`iteration-check` 复跑后 HS-10 已从 warn 降级为 info。**这部分不需要再修。**

**遗留的 2 处次要缺口**（`tools/lib/gates.py:1646-1761 check_g_doc_storage`，事后兜底层，非写入前拦截）：

| # | 缺口 | 现象 | 影响 |
|---|------|------|------|
| 1 | 资产路径查找不兼容新版嵌套结构 | 第 1660-1678 行手写的资产查找逻辑只认旧扁平路径 `assets/{pk}.assets.md`，不认当前 v4.1 的嵌套路径 `assets/{pk}/{pk}.assets.md`；同文件第 363-374 行的 `_doc_search_roots` 已经用 `paths.resolve_doc_workspace` 能正确解析两种路径 | 项目已迁移到新版资产结构时，`real_workspace` 解析失败，真值校验退化为硬编码 `_DOC_COMPLIANT_ROOTS` 回退，兜底层校验力度下降（不影响写入前拦截，因为拦截层走的是另一套已修复的调用链） |
| 2 | 扫描范围仅限 `project_dir` 内部 | `check_g_doc_storage` 用 `project_dir.rglob("*.md")`，文件写到 `project_dir` 之外（真正的 `d:\tmp\`）时 `checked=0`，完全扫不到 | 兜底层对"项目目录树之外"的游离文件失明；但这类场景现在已被写入前拦截层挡住，此项缺口的实际风险已从"唯一防线失效"降为"防御纵深的第二层缺角" |

**修复方案（B-2 残余）**：

```python
# tools/lib/gates.py check_g_doc_storage 内部

# 修复点1：统一资产路径查找逻辑
# 现状：第1660-1678行自己手写 candidate_ae_sdd / "assets" / f"{pk}.assets.md"
# 改法：直接调用已经正确处理新旧两种路径的 paths.resolve_doc_workspace(ade_sdd, pk)
#       替换手写逻辑，消除同文件内两套不一致实现（DRY）
real_workspace = paths.resolve_doc_workspace(candidate_ae_sdd, pk)  # 替代原手写查找

# 修复点2：扫描范围延伸到 project_dir 之外（可选增强，非必须）
# 现状：只 rglob(project_dir)
# 改法：若 real_workspace 与 project_dir 不同源（不是 project_dir 的子路径），
#       追加扫描 real_workspace 本身；同时可选扫描系统临时目录（tempfile.gettempdir()）
#       作为"高风险游离位置"专项探针，命中即报（不追求完整性，只堵最典型的误放场景）
if real_workspace and not _is_subpath(real_workspace, project_dir):
    extra_roots = [real_workspace]
# tempfile 专项探针：低成本，只需检查该目录下是否存在含 current_story 的 .md
```

**测试计划**：
1. 项目用新版嵌套资产路径 `assets/{pk}/{pk}.assets.md` → `check_g_doc_storage` 能正确解析 `real_workspace`（现状会失败，回归后应通过）
2. 项目用旧版扁平路径 `assets/{pk}.assets.md` → 保持兼容（不能因为改造导致旧项目退化）
3. 文档写在 `real_workspace` 之外但仍在系统 tmp 目录 → 专项探针命中并报 stray

**工作量估算**：1-2h（修复点1 是主要工作；修复点2 作为可选增强，视优先级决定是否本次一并做）。

**连带同步**：`source/standards/update-graph.json` 中 `gates.py` 门禁变更规则（UG-02）覆盖 `test_gates.py` 断言同步；本次改动不涉及新增门禁 ID，无需注册表变更。

### 1.3 B-3：真实缺口，原地未动，需要完整方案

**原诊断**：人工审核点①②④"对话内直接呈现"规范无格式强校验，依赖 AI 自律。

**复核结论：原诊断依然 100% 成立**，且需要纠正一个容易搞错的推理链——B-3 不是"决策1B废弃后遗留的缺口"，两者是完全平行、从未交叉过的两条线：

| | 决策1B处理的对象 | B-3处理的对象 |
|---|---|---|
| 校验目标 | AI 在响应里自报的 `◆ GATE: ✅ CLEAR` 字符串是否属实 | AI 在审核点这一动作里呈现的**内容格式**是否达标（有没有真的把 AC 表格/逐文件清单/问题清单表讲给用户看） |
| 校验层级 | phase 级宏观门禁 | 审核点级微观呈现格式 |
| 现状 | v3.6 前是弱校验（可被谎报绕过），v3.6 后改用 `gates check` 真实核对磁盘产物，是升级不是倒退 | 从诊断报告写下那天到今天，**从未有任何校验**，不存在"被废弃"的历史 |

`stop_check.py` v2.0 的 `check_output()` 现在只做两件事：①判断响应是否为空（防截断）②检测 PRD compact 卡死态。两者都不解析响应文本的实质内容，更不会匹配"AC-\d+ 表格"或"逐文件✅标记"这类结构。`gate_intercept.py`（PreToolUse）的拦截对象是工具调用的文件路径/内容，不是 AI 对话文本本身，天然覆盖不到这个维度。`flow_monitor.py` 的 `detect_drift()` 检查的是"磁盘产物是否存在、内容含不含 14 道门禁关键词"，跟"AI 这轮对话有没有真的把内容讲给用户看"是两个完全不同的校验维度。

**全链路 grep 确认零命中**：`AC-\d+`、`逐文件.*✅`、`问题清单.*表`、`reviewQuality`、`format_verified` 在 `SKILL.md`/`stop_check.py`/`gate_intercept.py`/`flow_monitor.py`/`state.schema` 里均未出现。

**审核点的精确格式要求原文**（`source/SKILL.md`）：

| 审核点 | 位置 | 要求原文 |
|---|---|---|
| ① 设计完成 | SKILL.md:464-465 | "AC验收标准完整列表 + 核心接口一览 + 关键设计决策 + 已识别风险点 + 测试用例数量" |
| ② Task逐文件核对 | SKILL.md:492-493 | "按文件名字典序逐文件：AI完整读出→AI指出重点→用户逐文件✅/⚠️/⏸️" |
| ②.5 CodingPlan评审 | SKILL.md:495-496 | "14条门禁通过状态 + CodingModel 11维决策 + 风险Task + 关键类骨架" |
| ④ CodeReview | （子 SKILL 定义） | 问题清单表 + AC 覆盖对账表 |

**⚠️ 一个需要一并考虑的新变量**：仓库当前版本已到 v3.8.0，新增了"自动化开关"（`.ae-sdd/config.yaml` 的 `automation` 段，默认关闭）。开启后 6 个审核点改走 Tier 3 多 reviewer 联审共识，**跳过所有人工✅**。这意味着"对话内呈现格式"这条规则的适用前提是"当前处于人工审核模式"——设计格式校验时必须先判断 `automation.enabled` 是否为 true，若已开启联审模式则不适用本次校验（那是另一套共识机制的职责边界，不应被格式正则误伤）。

---

## 2. B-3 详细修复方案

### 2.1 设计方案

在 `stop_check.py` 的 `check_output()` 内新增一个格式校验分支，函数签名不变（`check_output(transcript_content, ade_sdd) -> tuple[bool, str]`），复用现有的"返回 False + 注入提示文本"机制（这正是该函数已有的设计，新增分支零改动接口）：

```python
# tools/lib/stop_check.py check_output() 内新增

def _detect_review_point_context(state: dict) -> Optional[str]:
    """判断当前 phase 转换是否处于某个审核点场景，返回审核点编号或 None。
    判定依据：state.json 的 phase 字段处于审核点前置 phase
    （如 phase == 'story-generated' 且即将进 'story-reviewed' → 审核点1）。
    复用 flow_monitor.get_phase_gate_map() 已有的 phase→gate 映射作为参照。
    """
    ...

def _check_review_point_format(response_text: str, review_point: str) -> tuple[bool, str]:
    """结构特征粗筛（非语义理解），按审核点编号匹配对应格式特征。
    审核点1  ：要求命中 r"AC[-\s]?\d+" 且命中 r"接口|interface"
    审核点2  ：要求命中 r"[✅⚠️⏸️]" 且出现次数 ≥ 文件数量参考值
    审核点2.5：要求命中 r"14\s*条门禁|门禁.*通过" 且命中 r"CodingModel|11\s*维"
    审核点4  ：要求命中 r"问题清单|Issue" 且命中 r"AC.*(对账|覆盖)"
    未命中 → (False, 具体缺失项提示)
    """
    ...

# check_output 内，检查1（空响应）和检查2（PRD compact）之后新增：
if ade_sdd is not None:
    cfg = paths.read_config(ade_sdd)
    automation_enabled = cfg.get("automation", {}).get("enabled", False)
    if not automation_enabled:  # 🔴 关键前提：仅人工审核模式下才校验格式
        st = state_mod.read_state(paths.state_path(ade_sdd))
        review_point = _detect_review_point_context(st)
        if review_point:
            ok, missing = _check_review_point_format(last_response, review_point)
            if not ok:
                increment_retry(ade_sdd)
                return False, (
                    f"[ae-sdd harness] Stop hook：检测到审核点{review_point}响应缺少必要呈现内容"
                    f"（缺失：{missing}）。\n"
                    f"请在对话内直接呈现完整内容，不要仅给出文件路径。"
                )
```

### 2.2 校验失败时的行为：报警重试，非硬阻断

推荐复用 `check_output` 现有的 `MAX_RETRY`（=2）机制，而非新增一种阻断类型：格式粗筛存在误报空间（AI 确实呈现了内容但措辞恰好不匹配正则），如果做成硬阻断，一次误报就会让流程卡死。现有重试机制的设计——"最多重试 2 次，达上限直接放行"——本身就是为这类"可能误报的格式检查"准备的护栏，直接复用比新增一套机制更一致、更安全。

### 2.3 误伤防范

1. **自动化模式开启时必须跳过**（已在设计里作为前置判断，见 2.1）——否则会跟 Tier 3 联审共识机制打架，误拦截合法的自动化流程。
2. **审核点判定必须基于 phase 转换而非关键词猜测**：不能因为响应里出现"AC"两个字母就误判进入审核点1，必须先确认 `state.json` 的 phase 正处于审核点前置状态，缩小误判面。
3. **正则要宽松匹配，不要求精确格式**：比如审核点2 的"逐文件✅"不要求严格的"每行一个文件+一个✅"格式，只要求✅/⚠️/⏸️符号出现次数达到一个宽松阈值即可，避免因为 AI 用了等价但形式不同的表达（比如用表格而非列表）被误拦。
4. **达到 MAX_RETRY 后必须放行**：这是现有机制已经保证的兜底，新分支复用它即自动获得这层保护。

### 2.4 测试计划

| # | 场景 | 预期 |
|---|------|------|
| 1 | 审核点1场景，响应包含完整 AC 列表+接口清单 | 通过（should_stop=True） |
| 2 | 审核点1场景，响应只给"请查看 xxx.md" | 拦截，提示缺失项，重试 |
| 3 | 审核点4场景，响应包含问题清单表+AC对账表 | 通过 |
| 4 | 非审核点场景的普通响应 | 不触发本检查（`_detect_review_point_context` 返回 None） |
| 5 | `automation.enabled=true` 且处于审核点1场景 | 跳过格式校验（不判断，直接走原有逻辑） |
| 6 | 连续 2 次格式不达标后第 3 次 | 达 MAX_RETRY，强制放行 |

### 2.5 连带同步项

`source/standards/update-graph.json:128-133` 已有现成规则覆盖 `stop_check.py` 改动：

> "HARNESS HS-X 声明的 Stop hook 拦截须在此实现"（`auto_checkable: true`）

本次改动后必须同步：
1. `source/HARNESS.md` 新增或修订对应 HS 条目的声明措辞（当前 HS-3/HS-12 的措辞需要评估是否要合并引用本次新增的格式校验，或作为独立新增 HS 条目）
2. `tools/tests/test_stop_check.py`（若不存在需新建）覆盖上表 6 个场景
3. 跑 `ae-sdd update-check` 确认 UC-06 无新增"声明但无实现"项

### 2.6 工作量估算

2-3h（格式判定规则设计+实现是主要工作量；自动化开关判断逻辑简单，复用现有 `read_config`）。

---

## 3. 实施优先级与工作量汇总

| 项 | 优先级 | 预估工时 | 是否可与另一项并行 |
|---|--------|---------|-----|
| B-2 残余（资产路径统一 + 扫描范围延伸） | P1（次要技术债，非阻断） | 1-2h | 可并行——改动文件（`gates.py`）与 B-3（`stop_check.py`）无交集 |
| B-3（审核点格式校验） | P0（真实阻断级缺口，原地未动） | 2-3h | 可并行 |

两项修改的文件完全不重叠（`gates.py` vs `stop_check.py`），无依赖顺序要求，可同时安排给不同工程师或先后处理，互不阻塞。

---

## 4. 用户确认清单

请确认以下决策点后再进入编码阶段：

1. **B-2 修复点2（扫描范围延伸到 project_dir 之外）是否本次一并做**，还是仅做修复点1（资产路径统一），修复点2 留到下次迭代？（修复点2 是"防御纵深第二层"的增强，非阻断，可以缓做）
2. **B-3 校验失败的行为**是否同意采用"报警重试，复用现有 MAX_RETRY 机制"，而非新增硬阻断？（设计理由见 §2.2，硬阻断在格式粗筛场景下误报代价过高）
3. **B-3 是否需要同时新增独立的 HARNESS.md Hard Stop 条目**（比如 HS-15），还是归并进现有 HS-3 的措辞里作为其"已补物理实现"的升级？两种做法都可行，需要你决定文档层面怎么呈现这次变更。
4. **本文档是否需要写入 CHANGELOG**：按 ae-sdd 自身规范（`ae-sdd-update-skill.md` 步骤4.5），修改母版必须写 CHANGELOG；本文档目前存放于 `update-doc/history/`（非 CHANGELOG 目录），确认后续实施完成后是否需要在 `source/CHANGELOG/` 补一条对应记录。

---

## 附：本次复核过程说明

原计划通过 Workflow 多 agent 编排完成本次调研+设计+评审全流程，实际执行中第一阶段（B-2/B-3 现状核实）顺利完成，但第二阶段起出现异常重试导致效率显著低于预期（耗时 55 分钟仍未产出后续阶段结果），遂终止 workflow，改为在主对话中基于已完成的第一阶段核实结果，结合期间发现的仓库并行变更（用户在其他会话中已提交 3 个相关 commit，其中 `b864f9d` 修复了 B-2 核心缺口），直接完成方案设计与本文档整合。B-3 的现状核实结论未受仓库并行变更影响（三个新 commit 均未触及 `stop_check.py`/`flow_monitor.py`/`session.py`），可直接采信。
