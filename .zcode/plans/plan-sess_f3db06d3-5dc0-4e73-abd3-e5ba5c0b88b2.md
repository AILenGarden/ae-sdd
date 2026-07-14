# ae-sdd micro 意图分流路由（v3.10.2）

## 背景与诊断

**你的诉求**：`/ae-sdd 请帮我优化这部分实现` 或 `/ae-sdd 帮我 CodeReview 这段` 时，能直接进无文档微任务、调用相应能力（优化→coding；审查→code-review），而不是走完整 设计→评审→实现 流程，也不要误进自更新。

**现状诊断（4 个真实缺口）**：

| # | 缺口 | 证据 |
|---|------|------|
| 1 | **"优化"误路由 self-update** | 路由表里"优化"只匹配 `优化ae-sdd → update-skill`（SKILL.full.md:338）。`/ae-sdd 优化这部分实现` 会被 LLM 判为"优化 ae-sdd"，进自更新流程 |
| 2 | **"审查/CodeReview"不是路由触发词** | 全体系里"CodeReview"只是 Phase 3 下游节点，从未作为路由入口触发词（grep 确认） |
| 3 | **micro 链无意图分流** | micro 进 CodingProcess 走完整 §A1.5 骨架分解→CodeAnalysis→CodePlan→Execute，无"只调单能力"分支（coding-process-skill.full.md:108-119） |
| 4 | **code-review 硬准入拦死无文档审查** | 第零步必读 5 产物（Story/CodingPlan/CodingReport/TestReport/资产），缺了直接禁止继续（code-review-skill.full.md:194-209） |

**路由是文档驱动**（LLM 读 SKILL.md 路由表判定，classify.py 未参与路由），所以改动以**文档层为主、机制层最小补足**。

---

## 设计方案（方案 A：复用微链 + entry_node 分流，不新增 phase 序列）

### 核心思路

micro 内按意图分流成两条子路径，**复用现有微链 phase 序列**，仅在 gate 跨步跳跃处加 entry_node 豁免（复用 v3.5.15 BUG 范式 gate_intercept.py:443）：

```
micro-optimize（优化/重构）：initialized → coding-process(轻量) → coding → completed
  └─ 跳过 test-running/code-reviewed；coding-process 做"给定代码 CodeAnalysis→直接优化"

micro-review（审查/CodeReview）：initialized → code-reviewed → completed
  └─ 跳过 coding-process/coding/test-running；code-review 走"无文档轻量准入"，对话内出结论
```

### 消歧规则（解决缺口#1，最关键）

`优化` 关键词消歧（写进 classify.py + 路由表 + route.compact.md）：

| 用户输入模式 | 路由 |
|-------------|------|
| `优化这部分实现` / `优化代码` / `重构` / `改进` + **代码/实现上下文** | → micro-optimize |
| `优化 ae-sdd` / `优化 SKILL` / `优化流程` + **ae-sdd/SKILL 上下文** | → self-update（原行为不变） |
| `审查` / `CodeReview` / `出 CR 报告` / `评审这段` + **代码上下文** | → micro-review |

判定优先级：**上下文关键词优先**（代码/实现/这段代码 → micro；ae-sdd/SKILL/流程 → self-update）。

---

## 改动清单（8 文件）

### 机制层（代码，2 文件）

**1. `tools/lib/classify.py`**（~25 行）
- L358-371 entry_node 推断链新增两条分支：
  ```python
  elif any(kw in text_lower for kw in ("优化", "重构", "改进")) and _is_code_context(text_lower):
      entry_node = "OPTIMIZE"
      if scale != "微": scale = "微"
  elif any(kw in text_lower for kw in ("codereview", "code review", "审查", "评审代码", "出cr报告")):
      entry_node = "CODE_REVIEW"
      if scale != "微": scale = "微"
  ```
- 新增 `_is_code_context()` 辅助：检测"代码/实现/这段/这个文件/方法/类"等代码语境词，用于消歧（区分"优化代码"vs"优化ae-sdd"）。注意"优化ae-sdd/优化SKILL/优化流程"优先判 self-update。
- 消歧优先级：先判 self-update 上下文（ae-sdd/SKILL/流程），再判 micro-optimize，避免"优化 ae-sdd 的代码实现"误判。

**2. `tools/lib/gate_intercept.py`**（~15 行）
- L726-731 跨步跳跃校验：对 `entry_node in ("CODE_REVIEW", "OPTIMIZE")` 且 `scale=="微"` 的 state，放宽允许的跳跃距离（initialized→code-reviewed 对 micro-review；initialized→coding 对 micro-optimize）。复用 L443 微链 BUG 范式。
- 不动 PHASE_FLOWS（复用现有微链序列），不动 VALID_SCALES。

### 行为层（文档，3 文件）

**3. `source/skill-fallbacks/SKILL.full.md`**
- L324-332 路由表（编码类）：micro 行扩展为两条子行，加"意图列"：
  ```
  | 微-优化 | 优化/重构/改进代码（无文档）| coding-process 轻量→coding | — |
  | 微-审查 | 审查/CodeReview/评审代码（无文档）| code-review 轻量准入 | — |
  ```
- L334-342 非编码类路由：给"优化ae-sdd"加消歧注释（"优化代码/实现 → micro-优化"）。
- L344-351 4类规格判定表：同步新增"微-优化/微-审查"两行。
- L353-360 状态机子链表：给微链加注（"微-优化跳 test/code-review；微-审查跳 coding-process/coding/test"）。
- L313 智能路由调用顺序：第①步自更新识别加"消歧（代码上下文 → 不进自更新）"。

**4. `source/skill-fallbacks/skills/phase3-review/code-review-skill.full.md`**
- L194-209 第零步准入：新增"🆕 无文档轻量准入分支"——
  - 触发条件：entry_node=CODE_REVIEW 且无 Story/CodingPlan 产物
  - 准入要求降级：仅需"用户指定的代码范围 + 项目资产 §5/§6（可选）"
  - 输出：对话内直接给 CodeReview 结论（🔴/🟠/🟡/🟢 分级 + file:line 证据）
  - 落文档选项：用户要求落正式 CodeReview 报告时，state 须先到 code-reviewed phase（gate 豁免已放行），否则仅对话输出
- L170-176 触发条件表：加"micro-review 子路径触发"。

**5. `source/skill-fallbacks/skills/phase2-coding/coding-process-skill.full.md`**
- §A1.5 前新增"micro 意图分流前置门"：
  - entry_node=OPTIMIZE：跳过骨架分解（§A1.5），直接 §A2 CodeAnalysis（基于给定代码）→ 轻量 CodePlan → coding。不要求 Story/TestCase 输入。
  - entry_node=CODE_REVIEW：交棒 code-review-skill（本 SKILL 不接管）。
- §A1 加载上下文表：micro-optimize 允许"仅代码 + 项目资产"，免 Story/TestCase。

### 声明层同步（3 文件）

**6. `scripts/compile_skill_runtime.py`** — L36-45 ROUTE_ROWS 常量：
```python
("micro-optimize", "优化/重构代码, no docs", "coding-process(轻量) -> Coding"),
("micro-review", "审查/CodeReview, no docs", "code-review 轻量准入 -> 对话结论"),
```
（这是 route.compact.md 的真正来源，必须手动同步，否则 dist 与源不一致）

**7. `source/SKILL.md`** — frontmatter：version 3.10.1→3.10.2；新增 🆕 v3.10.2 条目描述本变更。

**8. `source/CHANGELOG/2026-07-11-v3.10.2-micro-intent-routing.md`** — 新建，记录本变更（遵循"文档不承载 changelog"红线，正文只引用）。

### 测试（1 文件）

**9. `tools/tests/test_classify_micro_intent.py`**（新建）— 覆盖：
- `优化这部分实现` → entry_node=OPTIMIZE, scale=微
- `优化 ae-sdd` → 不进 micro（保持 self-update 语义）
- `帮我 CodeReview 这段` → entry_node=CODE_REVIEW, scale=微
- 消歧优先级（"优化 ae-sdd 的实现"应判 self-update 而非 micro）

---

## 不改的部分（明确边界）

- ❌ 不新增 phase 序列（复用现有微链，靠 gate 豁免）—— 避免扩散到 state.py PHASE_FLOWS/VALID_SCALES/_infer_scale
- ❌ 不动 PHASE_ENTRY_GATES 注册结构（micro-review 的 code-reviewed 入口靠 gate_intercept 豁免分支处理）
- ❌ 不改 classify.py 的 `classify()` 被调用现状（它仍未参与路由，只是 entry_node 字段供 state/gate 读取）

---

## 验证计划

1. `python -m pytest tools/tests/test_classify_micro_intent.py` — 新测试通过
2. `python -m pytest tools/tests/` — 全量回归（gate_intercept 改动不破坏现有跨步跳跃测试）
3. `ae-sdd health` — 健康检查
4. `ae-sdd update-check --affected tools/lib/classify.py tools/lib/gate_intercept.py source/skill-fallbacks/SKILL.full.md` — 跑 UG-08/09/10/16 连带检查
5. runtime 重建：`python scripts/build_dist.py`（若存在）或等价命令，验证 route.compact.md 含新 micro 子行
6. 手测：`/ae-sdd 请帮我优化这部分实现` → 进 micro-optimize 而非 self-update

---

## 风险点

| 风险 | 应对 |
|------|------|
| 消歧误判（"优化 ae-sdd 的代码"该进哪？） | 优先级文档化：ae-sdd/SKILL 上下文词优先于代码上下文词；测试覆盖边界 |
| gate 豁免过宽导致跳步漏洞 | 豁免严格限定 `entry_node in (...) AND scale=="微"`，加单测防回归 |
| ROUTE_ROWS 与源文档再次漂移 | 同步两处 + update-check UC-15 一致性校验兜底 |

---

## 执行顺序

1. 机制层先行（classify.py + gate_intercept.py + 新测试）→ pytest 验证
2. 行为层文档（SKILL.full.md + code-review/coding-process fallback）
3. 声明层同步（compile_skill_runtime.py ROUTE_ROWS + SKILL.md frontmatter + CHANGELOG）
4. runtime 重建 + 全量验证（health + update-check + 手测）