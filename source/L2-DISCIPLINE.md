<!-- ae-sdd L2 conversation discipline SSOT; injected by scripts/l2_inject.py -->

## Process Artifact Policy

RA, DR, and Story are the only core design documents. Proposal, GeneratePlan,
CodingPlan Markdown, CodingReport, TestReport, CodeReview report, and other
process reports are retired for new writes. Historical files remain read-only.
Execution plans live in `state.executionPlan`, tests live in evidence manifests,
and review results live in `state.review.status/findings`. Never write a changelog.

<!-- SECTION:zh -->
## ae-sdd 强制工作流

凡请求可能新增、修改、删除、重构、优化或生成代码、测试、配置、Schema、
Migration、构建脚本或其他工程制品，必须在实现规划或首次写入前加载并调用
`ae-sdd`。只读解释、检查、分析和状态报告不进入 Coding 流程。

任务大小不是豁免理由。git 仓库，以及包含构建文件、ae-sdd 文档目录或工程约束
的非仓库目录，都受本纪律约束。路由由 ae-sdd 与用户决定。

### 极简合法链

```text
大：Route -> Requirement Analysis -> DR/Story/CodingPlan -> executionPlan(用户确认) -> Coding -> Test evidence -> Review findings
中：Route -> Requirement Analysis -> Story/CodingPlan -> executionPlan(用户确认) -> Coding -> Test evidence -> Review findings
小/微：Route -> Requirement Analysis -> CodingPlan/Story-lite -> executionPlan(用户确认) -> Coding -> Test evidence -> Review findings
```

- RA、DR、Story 是核心设计文档；路由需要时必须存在，不得用过程报告替代。
- Story 必须包含接口/字段/主流程/数据模型、AC 和验证矩阵。
- 仅当验证矩阵复杂到不适合内嵌 Story 时，才允许独立 TestCase。
- Task 仅用于大型并行拆分，不是默认流程节点。
- 编码前必须通过 G-CODEPLAN-SRC、G-14、G-08，并获得用户对紧凑计划的确认。
- 测试只记录真实 evidence；Review 只记录 status/findings。
- 任何时候都不得新写 Proposal、CodingReport、TestReport、CodeReview 报告或 changelog。

### 编码前四上下文

| 上下文 | 来源 | 缺失时 |
| --- | --- | --- |
| 项目约束 | `get_constraints(projectKey)` | 停止并更新项目资产 |
| 技术约束 CodingModel | `get_thinking_engine(projectKey)` | 停止并补齐空维度 |
| 需求说明书 | `doc resolve --intent RA` | 停止并生成/更新 RA |
| Story（按分析路径） | `doc resolve --intent STORY` | 仅当分析选择 Story 时生成/更新 |
| 验证契约 | Story 验证矩阵；复杂时可引用 TestCase | 停止并补齐 AC 到验证映射 |

### 硬约束

- 任一 blocker gate 失败后立即停止写入，报告 gate ID，并执行规定修复路径。
- ae-sdd 不可用时不得降级为自由编码；先修复运行环境。
- harness 的计划批准不能替代 `state.executionPlan` 的用户确认。
- `/ae-sdd-quick` 只能由用户显式指定，且不取消 Story-lite、计划确认和验证证据。
- 路由未到合法终态、验证证据未落地、Review 未通过时不得声明完成。
<!-- /SECTION:zh -->

<!-- SECTION:en -->
## Mandatory ae-sdd Coding Workflow

For every task that may create, modify, delete, refactor, optimize, or generate
code, tests, configuration, schemas, migrations, build scripts, or other
engineering artifacts, load and invoke `ae-sdd` before implementation planning
or the first write. Read-only explanation, inspection, analysis, and status
reporting are out of scope.

Task size is not an exemption. Git repositories and non-repository engineering
directories containing build files, ae-sdd documents, or workspace engineering
constraints are in scope. Routing authority belongs to ae-sdd and the user.

### Minimal legal routes

```text
large: Route -> Requirement Analysis -> DR/Story/CodingPlan -> approved executionPlan -> Coding -> Test evidence -> Review findings
medium: Route -> Requirement Analysis -> Story/CodingPlan -> approved executionPlan -> Coding -> Test evidence -> Review findings
small/micro: Route -> Requirement Analysis -> CodingPlan/Story-lite -> approved executionPlan -> Coding -> Test evidence -> Review findings
```

- RA, DR, and Story are the core design documents. Required upstream documents
  must exist and cannot be replaced by process reports.
- Story contains contracts, fields, main flow, data model, AC, and a verification matrix.
- A standalone TestCase is optional and allowed only for a genuinely complex matrix.
- Task is optional and reserved for large parallel decomposition.
- Before Coding, G-CODEPLAN-SRC, G-14, and G-08 must pass and the user must approve
  the compact `state.executionPlan`.
- Tests record real evidence only. Review records `status/findings` only.
- Never create Proposal, CodingReport, TestReport, CodeReview report, or changelog files.

### Four required contexts before Coding

| Context | Source | If missing |
| --- | --- | --- |
| Project constraints | `get_constraints(projectKey)` | stop and update project assets |
| Technical CodingModel | `get_thinking_engine(projectKey)` | stop and fill every dimension |
| Story | `doc resolve --intent STORY` | stop and generate/update Story |
| Verification contract | Story matrix; optional TestCase for complex cases | stop and map every AC to verification |

### Hard constraints

- Fail closed on every blocker gate. Report the gate ID and follow its remediation.
- If ae-sdd or its state is unavailable, repair the environment before coding.
- Harness plan approval does not replace user approval of `state.executionPlan`.
- Only the user may select `/ae-sdd-quick`; it still requires Story-lite, plan approval,
  and verification evidence.
- Do not claim completion before the legal terminal state, finalized evidence, and
  a passing review.
<!-- /SECTION:en -->

<!-- SECTION:redline11 -->
| 11 | 新写 changelog 或把变更历史混入现行规范 | 永久禁止；历史文件只读，当前事实写入权威规范、状态和测试证据 |
| 11 | Writing a changelog or mixing change history into current specifications | Permanently forbidden; keep history read-only and put current truth in authoritative specs, state, and test evidence |
<!-- /SECTION:redline11 -->
