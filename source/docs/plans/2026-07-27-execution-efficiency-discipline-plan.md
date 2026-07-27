# 执行效率纪律修订（增量测试 / 昂贵命令单次产出 / 前置确认）— Plan

> **起草日期**：2026-07-27
> **目标文件**：`source/L2-DISCIPLINE.md`（中英双语 SSOT）
> **起草依据**：2026-07-27 两轮迭代（clean_target 统一 → CHANGELOG 清理 → 迁移版本号单点派生）中，用户两次反馈"慢、很慢、浪费 token"后的执行复盘
> **状态**：待评审
> **目标读者**：ae-sdd 维护者 + L2 注入的三个 Agent 指令文件的消费方

---

## 0. TL;DR

两轮迭代的墙钟时间里超过一半可省。根因不是单点失误，而是**验证批放在了步骤边界而不是任务边界**，叠加**同一条昂贵命令为换过滤角度反复执行**。

本 Plan 把五条纪律落进 `L2-DISCIPLINE.md`，其中 R1（增量测试）是本轮新增的最大收益项。

| 规则 | 性质 | 预估收益 |
|------|------|---------|
| **R1 增量测试** | 新增 | 最高——单条即可省掉一轮迭代一半以上等待 |
| **R2 昂贵命令单次产出** | 改写现有条目（措辞太宽，已被违反） | 高 |
| **R3 基线对比用 worktree** | 收紧现有条目 | 中（防事故为主，非提速） |
| **R4 前置确认** | 新增 | 中 |
| **R5 爆炸半径前置** | 新增 | 中 |

---

## 1. 问题证据（来自本次会话，非推测）

### 1.1 全量套件重复执行

本轮做了三件互不冲突、最终一起交付的事，每做完一件跑一次全量 Rust + 全量 Python。

- Python 套件单次约 3 分 20 秒，本轮执行 4 次（我的改动 / 基线 / 清理后 / 重构后）≈ 14 分钟
- 真正需要：收口一次 + 基线一次 ≈ 7 分钟
- 中间两次买不到额外信心，最后一次必然覆盖

### 1.2 同一条命令在一次调用里跑两遍

重复至少 6 次，形态统一：

```bash
# 反模式：全量测试跑两遍，只为换过滤角度
cargo test ... | grep "FAILED"; cargo test ... | grep "ok" | awk '{...}'
```

`install.py` 同样中招：先 `grep ✅` 看分发汇总，再重跑一次 `tail -22` 看尾部提示——**带副作用的分发流程执行了两次**。

`build_dist.py` + `compile_skill_runtime.py` 跑了 3 轮，其中一轮起因是先跑 compile 才发现它依赖 build_dist（脚本第 5 行就写着前置关系）。

### 1.3 自证循环与命令摸索

- clippy 显示 `0.22s`，疑为缓存，连续 5 次调用（touch / cargo clean / `-v` 数 Fresh / 删 fingerprint）自证；一次删 fingerprint 即可定论
- 部署阶段 `daemon status`、`sc query`、`runtime start` 缺参数，6 次试错；先 `--help` 一次可避免
- `install_cli.py` 连读 4 次均异常返回；第一次异常就该换已验证路径

### 1.4 耦合常量串行发现

迁移版本号 11→12 靠编译失败逐个撞出（`migration_catalog_0010` → `0011` → `sqlite_contract` 三处字符串），3 轮编译测试循环。

对照：`clean_target` 改动前做了全仓搜，一次抓到 `migrations/0009` 的 SQL CHECK 约束——**同一会话内做对过一次，说明能力在但触发不稳定**。

> 该根因已于本轮修复：`latest_runtime_schema_version()` 单点派生 + 连续性/自描述不变量测试，后续加迁移不需改任何测试字面量。

---

## 2. 规则条文（待写入 L2-DISCIPLINE.md）

### R1 增量测试（新增）

**中文：**

- 测试环境只跑增量：改动落在哪个 crate/模块，就只跑该 crate 的测试与对应测试文件。禁止在开发回路中跑全量套件。
- 全量套件只在两个时点执行：任务收口交付前，以及 release/分发前。
- 基线对比只采集一次并落盘复用，不重复采集。

**English：**

- Test incrementally: run only the tests of the crate/module the change lands in, plus the matching test files. Never run a full suite inside the development loop.
- Run the full suite at exactly two points: before task closure and before release/distribution.
- Collect a baseline once, persist it, and reuse it; never re-collect.

**保留全量的理由（评审确认点）：** 本轮既有失败（11 项 Python + 1 项 Rust）只有全量才暴露，增量跑不到跨 crate 的契约测试。若要更激进（收口也只跑受影响范围，仅 release 门禁跑全量），需评审决定。

### R2 昂贵命令单次产出（改写现有条目）

**中文：**

- 一次 bash 调用内，同一条构建/测试/分发命令不得出现两次。
- 昂贵命令输出一次落盘（`cmd > /tmp/x.log 2>&1`），后续统计一律读该文件。
- 有副作用的命令（分发、安装、构建）尤其禁止为查看不同输出片段而重跑。

**English：**

- Within a single bash call, the same build/test/distribution command must not appear twice.
- Persist an expensive command's output once (`cmd > /tmp/x.log 2>&1`) and read that file for every later tally.
- Commands with side effects (distribution, install, build) must never be re-run just to inspect a different slice of their output.

> 现有条目只写了"禁止为换过滤角度重跑"，未覆盖"一次调用里写两遍"与"有副作用的分发命令"两种形态，故本轮被再次违反。**规则写得不够具体等于没写。**

### R3 基线对比用 worktree（收紧现有条目）

**中文：** 保留现有 worktree 规则，补：调用前判断命令是否改写共享状态，再决定隔离方式；`git stash` 不用于任何对比场景。

**English：** Keep the existing worktree rule; add: decide whether a command mutates shared state before invoking it, then choose the isolation; `git stash` is never used for comparison.

> 依据：本会话用 `git stash push` 做基线对比，pathspec 命中未跟踪的 `dist/` 导致静默失败，随后 `pop` 将一个无关 stash 应用进工作区并注入冲突标记。首次总结的教训"改用 worktree"过窄，真正教训是"先问命令改写什么状态"。

### R4 前置确认（新增）

**中文：**

- 调用不熟悉的环境命令前，先读一次 `--help` 或脚本头部注释。
- 有序流水线（build → compile → install）先确认前置步骤，不靠失败反推顺序。
- 怀疑缓存或状态时用一次决定性检查，不做连续试探。

**English：**

- Read `--help` or the script header once before invoking an unfamiliar environment command.
- For ordered pipelines (build → compile → install), confirm prerequisites first instead of inferring order from failures.
- When suspecting stale cache or state, run one decisive check rather than a probe sequence.

### R5 爆炸半径前置（新增）

**中文：** 修改常量、阈值或版本号前，先搜语义名 + 搜字面值，范围覆盖 `crates/` `tools/` `migrations/` `tests/` `docs/`，一次拿全改动点。禁止靠编译失败逐个撞。

**English：** Before changing a constant, threshold, or version, search both the semantic name and the literal value across `crates/`, `tools/`, `migrations/`, `tests/`, and `docs/` to enumerate every site at once. Never discover them one compile failure at a time.

---

## 3. 执行步骤

1. 读 `source/L2-DISCIPLINE.md` 中英两个 SECTION，确认插入点。

   **已知约束：** `crates/ae-sdd-build/tests/compatibility_routes.rs:467`
   （`l2_discipline_ssot_carries_bilingual_execution_efficiency`）断言每语言**恰好 5 个固定子章节**，且每个 heading 在 SSOT 中出现次数必须为 1。因此五条规则全部作为 bullet 挂入既有子章节「有界调查与输出 / Bounded investigation and output」，**不新建子章节**。该测试只校验子章节存在性，不数 bullet 条目。

2. 中英同步改，逐条对应。

3. 查 `source/standards/update-graph.json` 是否有 UG 规则以 `source/L2-DISCIPLINE.md` 为 trigger；若有，按其 affected 清单同步（此步需实查，不预设结论）。

4. 验证（按 R1 自身规则，增量优先）：

   ```bash
   cargo test -p ae-sdd-build --test compatibility_routes   # L2 双语断言
   python -m pytest tools/tests/test_l2_inject.py -q        # 注入切片
   ```

   收口时再各跑一次全量，与已知基线逐项比对：Rust 1 项既有失败（`post_commit_and_harness_docs_use_rust_typed_argv_only`，`.githooks/post-commit:68` 注释含 "Python"），Python 11 项既有失败。

5. 分发：`build_dist.py` → `compile_skill_runtime.py` → `install.py`，各一次，输出落盘。

6. 重新注入三个用户全局指令文件：`~/.claude/CLAUDE.md`、`~/.codex/AGENTS.md`、`~/.zcode/AGENTS.md`。注入只替换 anchor 界定的托管块（`<!-- BEGIN ae-sdd-l2-ssot ... -->` ~ `<!-- END ae-sdd-l2-ssot -->`）。备份已在 `~/ae-sdd-instructions-backup-20260728-0030`。

---

## 4. 需先解决的挂起状态

上一轮注入未完成，当前环境：

- **daemon 已停** —— 为解锁 `dist/` 而停，未起回
- **`dist/ae-sdd` 被 ZCode 进程占用** —— `ae-sdd-build post-commit` 取不到句柄（os error 32），注入中断
- **三个指令文件仍是旧规则** —— 上一轮加入的 worktree / 落盘两条尚未生效到运行时

**建议解法：** 把 `--package` 指向一份临时副本，避开被占用的 `dist/`。理由是注入内容读自 `source/L2-DISCIPLINE.md`，`--package` 仅用于 skill 分发，而 skill 包上一轮已装好。

---

## 5. 不做的事

- 不动 `source/CHANGELOG/` 下 153 个历史文件（L2 第 11 条：历史只读）
- 不碰 `.githooks/post-commit`（其既有失败与本次无关，单独修）
- 不提交。本轮改完工作区累计 20 个改动文件 + 1 个新增迁移，留待审阅

---

## 6. 评审确认点

1. **R1 全量测试保留时点** —— 按本 Plan 为"收口前 + release 前"两处；是否要更激进（仅 release 门禁一处）？
2. **daemon 是否立即起回** —— 或等注入完成后一并重启？

---

## 7. 自评估说明

本 Plan 的问题清单分两类，可信度不同：

- **可观测事实** —— 命令执行次数、失败项、文件行号，均来自本会话实际记录
- **反事实推断** —— "本可避免"属对自身行为的推断，是本人自省最不可靠的部分

R5 有本会话内的存在性证据（`clean_target` 做对、版本号未做，两个方向的观测），R2 无——本会话未出现一次主动复用已有产物的实例。故 R2 属"计划"而非"已具备能力"。

**验证方式不经由自省：** 观察后续长任务——若重跑输出已落盘的套件，则 R2 未生效；若改常量前未搜字面值，则 R5 未生效。
