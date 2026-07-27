<!-- ae-sdd L2 conversation discipline SSOT. Released injection authority is the
     Rust `ae-sdd-build post-commit` managed-instruction stage, which replaces only
     the `ae-sdd-l2-ssot` anchor range of each host's global instruction file.
     `scripts/l2_inject.py` is migration/manual legacy tooling and a test oracle only;
     it is not part of the released distribution chain. -->

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

## 执行效率与范围纪律

以下规则用于在强制工作流内优化执行效率，绝不豁免权威检查、gate、批准、测试证据
和 Review。

### 快速续接

- 已存在经核验的交接结论与已批准的 executionPlan 时，直接从其续接。只刷新执行所
  必需的易变权威，不重新生成或重新发现已完成的分析。
- 除被 Gate 阻断外，最多经过一次权威刷新、一批定向源码查看和一次聚焦基线测试，
  就要抵达第一个有范围的补丁。

### 最短可验证切片

- 遵循用户声明的优先级，实现能编译并产出聚焦证据的最小端到端切片。顺序为：
  失败的聚焦测试 -> 最小补丁 -> 聚焦测试 -> 下一切片。
- 除已批准的 AC、失败测试、Gate 或用户明确要求外，不扩散到遗留兼容、泛化架构、
  历史恢复或无关加固。非阻断性加固单独登记。

### 有界调查与输出

- 脏 worktree 中禁止整仓 diff。使用 `rg`、scoped diff 和有界行读取；输出被截断
  时立即收窄查询。
- 避免过大的工具输出和不必要的上下文膨胀，二者会抬高延迟、token 消耗和请求超时
  风险。
- 需要与基线对比时（判断某失败是否本次引入），用 `git worktree add <tmp> HEAD`
  在隔离副本里跑，读完即 `git worktree remove`。禁止用 `git stash` 做基线对比：
  它改写当前工作区，pathspec 命中未跟踪路径时会静默失败，随后的 `pop` 可能把无
  关 stash 应用进来并注入冲突标记，有丢失未提交改动的风险。
- 一条昂贵命令的输出只跑一次并落盘（`cmd > /tmp/x.log 2>&1`），后续统计一律读该
  文件。禁止为换一个过滤角度而重跑同一条构建或测试命令。

### Agent 协同

- 复用经核验的交接结论与 Agent 结果。显式分配路径归属，做互不冲突的实现，而不是
  重复相邻分析。

### 进度控制

- 若连续三批调查型工具调用既没产出补丁、也没产出聚焦测试、也没识别出新阻断项，
  则切换到最短可执行验证回路，或报告确切阻断项。
- 用户改向或中止任务时，立即停止 subagent，仅回滚未完成的本地编辑，保持工作区
  可编译。
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

## Execution Efficiency and Scope Discipline

These rules optimize execution within mandatory workflows. They never waive
authoritative checks, gates, approvals, test evidence, or Review.

### Fast resume

- When a verified handoff and approved executionPlan exist, resume from them.
  Refresh only volatile authority required for execution; do not regenerate or
  rediscover completed analysis.
- Unless blocked by a Gate, reach the first scoped patch after at most one
  authority refresh, one targeted source-inspection batch, and one focused
  baseline test.

### Shortest verified slice

- Follow the user's stated priority and implement the smallest end-to-end slice
  that can compile and produce focused evidence. Use: failing focused test ->
  minimal patch -> focused test -> next slice.
- Do not expand into legacy compatibility, generalized architecture, historical
  recovery, or unrelated hardening unless required by the approved AC, a
  failing test, a Gate, or the user. Record non-blocking hardening separately.

### Bounded investigation and output

- In a dirty worktree, avoid repository-wide diffs. Use `rg`, scoped diffs, and
  bounded line reads; if output truncates, narrow the query immediately.
- Avoid oversized tool output and unnecessary context growth because they
  increase latency, token use, and request-timeout risk.
- To compare against a baseline (deciding whether a failure is pre-existing), run
  it in an isolated copy via `git worktree add <tmp> HEAD` and `git worktree
  remove` when done. Never use `git stash` for baseline comparison: it rewrites
  the live worktree, fails silently when a pathspec hits untracked paths, and a
  following `pop` can apply an unrelated stash with conflict markers, risking
  loss of uncommitted work.
- Run an expensive command once and persist its output (`cmd > /tmp/x.log 2>&1`),
  then read that file for every later tally. Never re-run the same build or test
  command just to filter its output differently.

### Agent coordination

- Reuse verified handoff findings and Agent results. Assign explicit path
  ownership and work on non-conflicting implementation instead of duplicating
  adjacent analysis.

### Progress control

- If three investigative tool batches produce no patch, focused test, or newly
  identified blocker, switch to the shortest executable verification loop or
  report the exact blocker.
- When the user redirects or stops the task, stop subagents immediately and
  leave the workspace compilable by reverting only incomplete local edits.
<!-- /SECTION:en -->

<!-- SECTION:redline11 -->
| 11 | 新写 changelog 或把变更历史混入现行规范 | 永久禁止；历史文件只读，当前事实写入权威规范、状态和测试证据 |
| 11 | Writing a changelog or mixing change history into current specifications | Permanently forbidden; keep history read-only and put current truth in authoritative specs, state, and test evidence |
<!-- /SECTION:redline11 -->
