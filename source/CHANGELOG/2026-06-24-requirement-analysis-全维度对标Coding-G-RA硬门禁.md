# 2026-06-24 | ae-sdd 需求分析能力全维度对标 Coding — G-RA 硬门禁 + 真实性扫描器 + 六层追溯

> **版本号**：3.2.0（v3.1.2 增量）
> **性质**：🔴 硬约束落地（把 RA 的"软门禁"变成"可执行门禁"，与 Coding G-08/G-09 对等）
> **影响范围**：1 新建扫描器（ra_authenticity_scan.py）+ 1 改工具（gates.py）+ 1 改测试（test_gates.py）+ 1 改路径版本（paths.py）+ 1 改构建脚本（build_dist.py 复制运行时扫描器）+ 1 改健康度清单（ae-sdd-update-skill.md）+ 1 更新 README + 1 新增 CHANGELOG + dev-sync 同步
> **本 CHANGELOG 仅覆盖代码层硬约束部分**；同期的 .md 层强化（RAModel 12 维 / 16 道 RA-G 闸 / ra-template 衍生章节 / SKILL.md G-RA 准入门卫章节）由既有工作区改动完成，本文件不重复记录。

---

## 摘要

用户判断：**需求分析能力应与 Coding 能力同等重要**。对标后确认二者存在本质不对等——Coding 阶段既有 SKILL 描述（14 门禁 + 8 类禁止）又有 `gates.py` 代码强制（G-08 解析 14 门禁关键词 + G-09 调 test_authenticity_scan.py）；而 RA 阶段虽已有 SKILL 描述（16 道 RA-G 闸），却**完全没有代码层硬约束**——RA 质量无法被代码验证，"禁止杜撰/穷举/无证据"停留在纸面规则。

**本次强化补齐这一层**：新增 `ra_authenticity_scan.py`（对标 test_authenticity_scan.py）+ gates.py 4 个 G-RA 硬门禁（对标 G-08/G-09）+ G-13 接入 RA 层（五层→六层追溯）。RA 质量从此与 Coding 一样可被代码验证。

**对标前后对比：**

| 层面 | Coding 现状 | RA 本次前 | RA 本次后 |
|------|------------|----------|----------|
| 决策模型 | CodingModel 11 维 | 无 | RAModel 12 维（既有 .md 改动）|
| SKILL 门禁 | 14 门禁（CodingPlan）| 8 道闸（软）| 16 道 RA-G 闸（软，既有）|
| 代码硬门禁 | gates.py G-08/G-09 | **0 个** | **gates.py G-RA-1~4** ✅ |
| 真实性扫描器 | test_authenticity_scan.py | **0 个** | **ra_authenticity_scan.py** ✅ |
| 全链路追溯 | G-13 五层 | 无 RA 层 | **G-13 六层（接入 RA）** ✅ |

---

## 改动 1：新增 scripts/ra_authenticity_scan.py（核心 — 对标 test_authenticity_scan.py）

### 1.1 改动点

新建 `scripts/ra_authenticity_scan.py`（无第三方依赖），完全对标 `scripts/test_authenticity_scan.py` 的结构：`Finding` dataclass + 规则表 + JSON/markdown 输出 + `--root`/`--format` 参数 + 返回码（BLOCKER>0 → 1）。

### 1.2 RA 真实性 8 类禁止规则

源自 requirement-analysis-skill §总则 4 标尺 + §阶段 E.5/G.5 强制：

| 规则 | 严重度 | 对应标尺 |
|------|--------|---------|
| vague-ellipsis（等等/其他/大概/之类的）| BLOCKER | 标尺1 穷举优于抽样 |
| no-evidence（结论行无 PRD §/Issue #/assets./用户对话 cite）| WARN | 标尺2 证据优于假设 |
| fabricated-field（数据要素字段未 cite assets.table）| BLOCKER | 标尺2 + 阶段D必禁 |
| hidden-conflict（"无冲突"但未列冲突清单）| WARN | 标尺3 冲突显性化 |
| masked-gap（"已解决/已确认"但无证据链）| BLOCKER | 标尺4 缺口不掩盖 |
| placeholder-fill（待补充/TODO/XXX/{待确认}）| WARN | 输出核心原则 |
| assumed-no-derivative（"无衍生规则"但无 H.5 全检声明）| BLOCKER | E.5.1 强制 |
| missing-timeliness（衍生 AC 时效写"尽快/及时/立即"非秒数）| BLOCKER | G.5.2 时效要求 |

### 1.3 输出契约

JSON：`{root, status, raFiles, reportStats:{raFiles,blockerFindings,warnFindings}, findings:[{severity,rule,path,line,message,snippet}]}`，与 test_authenticity_scan.py 一致（G-RA-4 复用 G-09 的 JSON 解析逻辑）。

### 1.4 验收标准

- [x] 8 类规则实现
- [x] 冒烟测试：违规 RA 文档 → status=FAIL + 4 BLOCKER；干净 RA 文档 → status=PASS + 返回码 0
- [x] 无第三方依赖，可独立运行

---

## 改动 2：gates.py 新增 G-RA-1~4 硬门禁（核心 — 对标 G-08/G-09）

### 2.1 改动点

`tools/lib/gates.py` 新增 4 个 G-RA 门禁函数 + GATE_REGISTRY 追加 4 项 + CHECK_FUNCS 注册 + check_all G-RA-4 特判（需 master_source 调子脚本，对标 G-09）。

### 2.2 四个门禁

| 门禁 | 名称 | 检查逻辑 | 对标 |
|------|------|---------|------|
| **G-RA-1** | RA 文档存在 | glob `**/RA-*.md`（兼容 ae-sdd-doc/ 与 design/）；pre-RA phase stub 通过；下游 phase 无 RA → block；RA 超 30 天 → warn 不阻断 | G-01 DR 存在 + G-09 phase 感知 |
| **G-RA-2** | RA 8 维度完整 | 解析 RA 文档，8 维度关键词（角色/场景/流程/数据/规则/设计方向/AC/假设）+ RAModel 12 维（RA-01~RA-12）齐全 | G-08 关键词检查 |
| **G-RA-3** | RA 衍生章节完整 | §6.5/§8.5/§8.6/§9-bis/§9-ter 五衍生章节；状态机类需求（命中状态变更关键词）必填，非状态机可"不适用+理由" | G-08 14 关键词 |
| **G-RA-4** | RA 真实性扫描通过 | 调 ra_authenticity_scan.py；BLOCKER=0 → pass | G-09 调 test_authenticity_scan.py |

### 2.3 验收标准

- [x] 4 个 check_ra_* 函数实现，phase 感知（pre-RA stub，对标 G-09）
- [x] GATE_REGISTRY 14→18，CHECK_FUNCS 注册齐全
- [x] check_all G-RA-4 特判（master_source 定位扫描器）
- [x] test_gates.py 新增 TestGRA1~4（每类多场景）全绿

---

## 改动 3：G-13 接入 RA 层（五层→六层追溯）

### 3.1 改动点

`tools/lib/gates.py` check_g13 新增"0. RA → DR 引用追溯"分支（链路最前端）。RA 为可选层：不存在不阻断（微任务/BUG 豁免），存在则检查 DR 是否引用 RA-ID。

### 3.2 追溯链路升级

- 升级前：DR ↔ Story ↔ Task ↔ Coding Report ↔ CodeReview（五层）
- 升级后：**RA ↔ DR ↔ Story ↔ Task ↔ Coding Report ↔ CodeReview**（六层）

### 3.3 验收标准

- [x] RA 存在但 DR 未引用 RA-ID → issue（阻断）
- [x] RA 不存在 → 不阻断（可选层），details.ra_layer.present=False
- [x] RA 存在且 DR 引用 RA-ID → pass，details.ra_layer.present=True
- [x] test_gates.py 新增 TestG13RaLayer 3 场景全绿

---

## 改动 4：测试 + 健康度清单 + README

### 4.1 测试（tools/tests/test_gates.py）

- 原 TestCheckAll 的 14 门禁断言 → 18（14 主 + 4 G-RA）
- 新增 TestGRA1（4 场景）/ TestGRA2（4 场景）/ TestGRA3（4 场景）/ TestGRA4（4 场景）/ TestG13RaLayer（3 场景）
- 全量回归：**330 passed, 1 skipped**（原 310 + 新增 20）

### 4.2 健康度清单（ae-sdd-update-skill.md）

子 SKILL 健康度段追加 7 项 v3.2 RA 自检项（requirement-analysis-skill 含 RAModel+16 闸 / ra-template 含 5 衍生章节+§13 / gates.py 含 G-RA-1~4 / ra_authenticity_scan.py 存在 / check_g13 接入 RA 层 / SKILL.md 含 G-RA 章节）。

### 4.3 README.md:5 版本行

v3.1.2 → v3.2.0，纳入本次 G-RA 硬门禁 + 既有 RA 软门禁强化。

### 4.4 构建/运行时同步

- `tools/lib/paths.py` 的 `MASTER_VERSION` 同步到 `3.2.0`，避免 `ae-sdd version` 仍显示旧版。
- `paths.locate_master_source()` 支持 Codex 安装目录与分发包根目录，确保 Codex Skill 内运行门禁时能定位 `SKILL.md`。
- `scripts/build_dist.py` 复制 `scripts/test_authenticity_scan.py` 与 `scripts/ra_authenticity_scan.py` 到 `dist/ae-sdd/scripts/`，确保 G-09/G-RA-4 在安装后可执行扫描而不是跳过。
- `tools/lib/gates.py` 的扫描器定位兼容开发仓库布局（`source/../scripts`）与分发包布局（`dist/scripts`）。

---

## 不承诺声明（🔴 必读）

> 本次强化做到：
> - ✅ 把 RA 16 道闸中的"存在性/维度完整性/衍生章节/真实性"4 类变成代码可执行门禁
> - ✅ RA 真实性 8 类禁止可被扫描器自动检出
> - ✅ RA 纳入全链路追溯
>
> 做不到 / 仍属软约束：
> - ❌ RA-G13（5 问自检通过率 100%）、RA-G14（缺口闭环）、RA-G15（规模路由置信度）等需语义判断的闸，仍由 SKILL 引导人工/LLM 判定，不在 gates.py 自动化范围
> - ❌ 业务具体阈值/策略（如"5 秒还是 10 秒"）仍需人类决策
> - ❌ RAModel 12 维的"结论质量"由 LLM 自填，gates.py 只检查"维度是否存在"，不检查"结论是否正确"

---

## 验证方式

### 验证 1：单元测试

```bash
python -m pytest tools/tests/ -q
# 预期：330 passed, 1 skipped, 17 subtests passed
```

### 验证 2：扫描器冒烟

```bash
python scripts/ra_authenticity_scan.py --root <含违规 RA 的目录> --format json
# 预期：status=FAIL，blockers>0，返回码 1
python scripts/ra_authenticity_scan.py --root <干净 RA 目录> --format json
# 预期：status=PASS，返回码 0
```

### 验证 3：门禁单跑

```bash
python -c "import sys; sys.path.insert(0,'tools'); from lib import gates; print([g['id'] for g in gates.GATE_REGISTRY])"
# 预期：含 G-RA-1~4
```

### 验证 4：双源一致性

dev-sync 后 `dist/ae-sdd/scripts/ra_authenticity_scan.py` 与 `~/.claude/skills/ae-sdd/scripts/ra_authenticity_scan.py` 与 source 一致；`tools/lib/gates.py` 三处一致。

---

## Reviewer

- **改动设计**：Harness（harness root agent）
- **用户决策**：用户确认"全维度对标（5 层全做）"
- **Reviewer**：待指派

---

## 维护

- **触发条件**：用户反馈"RA 质量无法被代码验证"或"需求分析与 Coding 不对等"时
- **后续迭代**：根据实际项目 RA 文档补充扫描规则；将更多 RA-G 闸（如 RA-G13 5 问自检）纳入自动化
- **同步要求**：任何修改必须 `bash scripts/dev-sync.sh`
