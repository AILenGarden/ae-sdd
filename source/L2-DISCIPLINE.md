<!-- ae-sdd L2 会话级纪律 SSOT | 渲染由 scripts/l2_inject.py 按 agent 语言切片注入 -->
<!-- 本文件是 ae-sdd L2 会话级纪律的唯一权威源（SSOT）。各 agent 全局指令文件
     （ZCode AGENTS.md / Codex AGENTS.md / Claude CLAUDE.md）中的 ae-sdd 锚点
     区间由注入器从本文件派生。人工请勿直接编辑各 agent 全局文件的 ae-sdd
     锚点区间——改本文件，再跑注入。 -->

> **与 L1 的关系**：L1（CODING ENTRY CONTRACT）定义在 `source/HARNESS.md:28-47`，
> 是 skill 包内的"首次写入前加载"契约，已随 distribute.py 分发。本文件（L2）是 L1
> 契约的**会话级展开版**——触发语义一致（改动级加载），补充顺序红线、四上下文、
> 非仓库判定等执行纪律，注入到各 agent 全局指令文件，确保三家措辞同源。

<!-- ════════════════════════════════════════════════════════════════════════════ -->
<!-- SECTION:zh —— 中文版（ZCode / Claude 注入用）                                -->
<!-- ════════════════════════════════════════════════════════════════════════════ -->

## ae-sdd 工作流强制调用（git 仓库及工程类目录）

凡请求可能新增、修改、删除、重构、优化或生成生产代码、测试代码、配置、Schema、Migration、构建脚本或其他实现制品，**在制定实现计划或首次写入前加载并调用 `ae-sdd`**，由其智能路由判定需求类型（已有 Story / 中大任务 / 小任务 / 微任务）并产出 CodingPlan，经用户确认后再编码。

- **任务大小不构成豁免理由**：单行日志、纯注释、格式化等零逻辑变更，也不得以"任务太小、过度工程"为由绕过流程。
- **适用范围**：git 仓库（`git rev-parse` 成功的目录）及工程类目录（见下文"非仓库目录的判定"）。纯笔记、临时草稿、配置文件散件不受约束。
- **不适用轮次**：纯问答、解释、分析、读代码不产生改动的轮次无需调用。一旦请求转为实施或写入，必须在第一次编辑前进入 `ae-sdd`。
- **路由判定权**：归 ae-sdd 流程与用户，不归 Agent 的临时直觉。

---

## ae-sdd Coding 执行纪律（SDD+TDD 双驱动，适用于所有工程类目录）

> **即使目录不是 git 仓库**（如 `git rev-parse` 失败但存在 Java 工程 / Story 文档体系），只要产生了代码改动，就必须遵守以下纪律。harness 的 `EnterPlanMode`/`ExitPlanMode` 审批 ≠ ae-sdd 的 CodePlan，两者不可互相替代。

### 顺序红线：先设计后编码

代码改动的合法前置链路（medium 路由为例）：

```
Proposal（4 段式问题载体）
  -> Story Update（改接口契约 / 字段表 / AC）
    -> TestCase Update（同步场景清单）
      -> CodePlan（过 G-CODEPLAN-SRC / G-14 / G-08 门禁 + 用户确认）
        -> Coding（按 CodePlan 执行）
          -> Test -> Review
```

**禁止跳链**：
- 🔴 禁止先改代码再补 Story 文档（"先盖楼后画图"）
- 🔴 禁止跳过 Story/TestCase 直接出 CodePlan（CodePlan 必须从 Story 接口契约 + TestCase 场景清单派生骨架）
- 🔴 禁止跳过 CodePlan 门禁直接编码（G-14 校验 CodePlan 与 Story AC 对齐）
- 🔴 禁止用 harness `ExitPlanMode` 批准代替 ae-sdd §A4 CodePlan 用户审核点

### CodingSkill 四上下文强制加载

进入 Coding 前**必须加载 4 个上下文，缺一停止**：

| 上下文 | 来源 | 缺失处置 |
|---|---|---|
| ① 项目约束 | `get_constraints(projectKey)` | 空 -> 停止，走 project-assets-update |
| ② 技术约束 CodingModel | `get_thinking_engine(projectKey)` | 任一维度空 -> 停止 |
| **③ Story 文档** | `doc resolve --intent STORY` | 提取接口契约/主流程/数据模型/AC |
| **④ TestCase 文档** | `doc resolve --intent TESTCASE` | 提取场景清单/测试分层/预期输入输出 |

- **③④是 SDD+TDD 双驱动的核心**：Story（设计驱动）定义"做什么"，TestCase（测试驱动）定义"怎么验证"，CodePlan 从两者派生骨架。
- 11 维 CodingModel 决策的"证据列"必须引用 Story AC / TestCase ID，禁止凭空编造。

### 非仓库目录的判定

当目录不是 git 仓库但存在以下特征之一时，**视为工程类目录，受本纪律约束**：
- 存在 `pom.xml` / `build.gradle` / `package.json` 等 build 文件
- 存在 Story / DR / TestCase 等 ae-sdd 文档目录
- workspace `AGENTS.md` 声明了模块结构 / 技术栈 / 工程约束

纯笔记、临时草稿、单个配置文件散件不受约束。

### 强约束补充

- **失败关闭**：任一 blocker gate 失败时立即停止后续写入，报告 gate ID 和失败原因，并执行规定的修复路径；**禁止猜测、伪造或口头声明通过**。
- **环境不可用处置**：`ae-sdd` 未安装、状态不可定位或 CLI 无法运行时，不得降级为自由编码；应报告阻塞并先修复运行环境。
- **唯一显式旁路**：仅用户主动指定 `/ae-sdd-quick` 或"走快速通道"时按快速通道执行；Agent 不得自行选择该旁路，且快速通道不免除落档义务。
- **完成判定**：在所选路由到达合法终态且所需验证证据落地前，禁止声称任务完成。

<!-- /SECTION:zh -->

<!-- ════════════════════════════════════════════════════════════════════════════ -->
<!-- SECTION:en —— 英文版（Codex 注入用）                                          -->
<!-- ════════════════════════════════════════════════════════════════════════════ -->

## Mandatory ae-sdd Coding Workflow

For every task that may create, modify, delete, refactor, optimize, or generate production code, test code, configuration, schemas, migrations, build scripts, or other implementation artifacts, **load and invoke `ae-sdd` before implementation planning or the first write**. Let `ae-sdd` classify the task scale, entry node, and route; produce a CodingPlan; and only begin coding after user confirmation.

- **Task size is not an exemption**: one-line logs, comments, formatting, or other zero-logic changes must not bypass the workflow on grounds of "too small" or "over-engineering."
- **Scope**: git repositories (`git rev-parse` succeeds) and engineering directories (see "Non-repository directory detection" below). Pure notes, scratch drafts, and loose config files are not constrained.
- **Out of scope turns**: read-only explanation, inspection, analysis, or status reporting does not enter the Coding workflow. Once a request turns to implementation or writing, `ae-sdd` must be entered before the first edit.
- **Routing authority**: belongs to the ae-sdd flow and the user, not the agent's ad-hoc intuition.

---

## ae-sdd Coding Discipline (SDD+TDD dual-driven, all engineering directories)

> **Even when the directory is not a git repository** (e.g., `git rev-parse` fails but a Java project / Story document hierarchy exists), any code change must follow the discipline below. The harness `EnterPlanMode`/`ExitPlanMode` approval ≠ ae-sdd's CodePlan; the two are not interchangeable.

### Sequence red line: design before coding

The legal pre-chain for code changes (medium route as example):

```
Proposal (4-section problem carrier)
  -> Story Update (interface contract / field table / AC)
    -> TestCase Update (sync scenario list)
      -> CodePlan (passes G-CODEPLAN-SRC / G-14 / G-08 gates + user confirmation)
        -> Coding (execute per CodePlan)
          -> Test -> Review
```

**Chain-skipping is forbidden**:
- 🔴 Forbidden to change code first and backfill Story docs later ("build first, draw later")
- 🔴 Forbidden to produce a CodePlan while skipping Story/TestCase (CodePlan must derive its skeleton from the Story interface contract + TestCase scenario list)
- 🔴 Forbidden to skip the CodePlan gate and code directly (G-14 validates CodePlan alignment with Story AC)
- 🔴 Forbidden to use harness `ExitPlanMode` approval as a substitute for the ae-sdd §A4 CodePlan user review point

### CodingSkill four-context mandatory loading

Before entering Coding, **4 contexts must be loaded; stop if any is missing**:

| Context | Source | Action if missing |
|---|---|---|
| ① Project constraints | `get_constraints(projectKey)` | empty -> stop, run project-assets-update |
| ② Technical constraints CodingModel | `get_thinking_engine(projectKey)` | any dimension empty -> stop |
| **③ Story doc** | `doc resolve --intent STORY` | extract interface contract / main flow / data model / AC |
| **④ TestCase doc** | `doc resolve --intent TESTCASE` | extract scenario list / test layering / expected I/O |

- **③④ are the core of SDD+TDD dual-driven**: Story (design-driven) defines "what to do", TestCase (test-driven) defines "how to verify", and CodePlan derives its skeleton from both.
- The "evidence column" of the 11-dimension CodingModel decision must reference Story AC / TestCase ID; fabrication is forbidden.

### Non-repository directory detection

When the directory is not a git repository but has one of the following features, **treat it as an engineering directory, subject to this discipline**:
- Contains build files such as `pom.xml` / `build.gradle` / `package.json`
- Contains ae-sdd document directories such as Story / DR / TestCase
- Workspace `AGENTS.md` declares module structure / tech stack / engineering constraints

Pure notes, scratch drafts, and single loose config files are not constrained.

### Hard constraints

- **Fail closed**: when any blocker gate fails, immediately stop further writes, report the gate ID and failure cause, and perform the prescribed remediation path; **never infer, simulate, or verbally declare a pass**.
- **Environment unavailable**: when `ae-sdd` is not installed, its state cannot be resolved, or the CLI cannot run, do not degrade to free-form coding; report the blocker and repair the environment first.
- **Only explicit bypass**: only when the user explicitly specifies `/ae-sdd-quick` or "走快速通道" does the quick channel execute; the agent must not choose this bypass on its own, and the quick channel does not remove documentation obligations.
- **Completion criterion**: do not claim task completion until the selected route reaches a legal terminal state and all required verification evidence exists.

<!-- /SECTION:en -->

<!-- ════════════════════════════════════════════════════════════════════════════ -->
<!-- SECTION:redline11 —— 红线条款 11（红线补丁用，Claude bootstrap 时注入红线表）  -->
<!-- ════════════════════════════════════════════════════════════════════════════ -->

<!-- redline11:zh -->
| 11 | 文档承载 changelog（设计/架构/模板/标准类正文混写历史变更） | 主题连续性破坏 / 检索劣化 / git blame 失真 |

> 条款 11 与 ae-sdd `source/L2-DISCIPLINE.md` 同源；变更历史必须落到 `CHANGELOG/` 独立文件，文档内仅写"详见 CHANGELOG/..."一句引用。
<!-- /redline11:zh -->

<!-- redline11:en -->
| 11 | Documents carrying changelog (design/architecture/template/standard prose mixing in historical changes) | Topic continuity broken / search degraded / git blame distorted |

> Clause 11 is co-sourced with ae-sdd `source/L2-DISCIPLINE.md`; change history must go into separate `CHANGELOG/` files, with only a single "see CHANGELOG/..." reference line in the document body.
<!-- /redline11:en -->
