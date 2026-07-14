# 改动方案：reset_story_substate 增加"产物作废信号"，让 task-generate 确定性全量重生成

## 一句话目标

`ae-sdd state relocate --story <ID>` 重置 Story 子状态后，下游 task-generate **不再依赖 LLM 文本比对判断"跳过/更新"**，而是看到一个确定性的作废标记 → 强制全量重新生成。消除"改了 Story、重置了状态、但旧 Task 残留"这一唯一真实风险。

## 为什么只改这一处（精确范围论证）

探查证实，三条下游 skill 的"产物已存在"行为不对称：

| skill | 已存在→跳过逻辑 | 重置后风险 | 本方案是否改 |
|---|---|---|---|
| testcase-generate | 无（`save_doc` 无条件覆盖，`document_storage.py:528`） | 无——重跑自动刷新 | ❌ 不改 |
| coding-skill / coding-process | 无（无条件覆盖） | 无——重跑自动刷新 | ❌ 不改 |
| **task-generate** | **有**：第三步判"Story 描述未变→跳过；已变→增量更新受影响章节"（`task-generate-skill.full.md:243-249`），且是 LLM 凭文本相似度判的 prose | **旧 Task 过期章节残留**，流入下游 | ✅ 唯一改点 |

结论：**风险面收敛到 task-generate 一个 prose 判定分支**，无需触碰 Python 的 save_doc / paths / document_storage（它们本就无条件覆盖）。

## 设计：作废信号挂在 state.json（SSOT），不挂产物文件

- 不在产物 `.md` 里打标记——`save_doc` 无条件覆盖会把标记冲掉，且污染交付物。
- 挂在 `state.json` 的 story substate 里：它是 SSOT，gate 能读、skill 能读、有现成 `resetHistory` 审计结构。

### 新增字段：`artifactInvalidated`（story substate 级）

`reset_story_substate` 重置时，往该 Story 子状态写：

```python
sub["artifactInvalidated"] = {
    "at": now,
    "by": by,
    "scopes": ["TASK", "TESTCASE", "CODING_PLAN"],   # 理论失效范围
    "reason": "story-substate-reset",
}
```

- 仅 `relocate`（带 reset）写；兄弟 Story 不写。
- 这是**一次性信号**：task-generate 消费后清除（见下），不累积。

## 改动清单（1 个 Python 函数 + 1 段 skill prose + 测试 + CHANGELOG）

### 改动 1：`tools/lib/state.py` — `reset_story_substate` 写作废信号（~6 行）

在 `state.py:1448-1461` 的重置循环里，`sub["lastUpdated"] = now` 之后追加：

```python
sub["artifactInvalidated"] = {
    "at": now,
    "by": by,
    "scopes": ["TASK", "TESTCASE", "CODING_PLAN"],
    "reason": "story-substate-reset",
}
```

**新增 helper（供改动 3 的 skill / gate 读取）**：

```python
def consume_artifact_invalidation(state: dict, story_id: str) -> Optional[dict]:
    """读取并清除该 Story 的产物作废信号（一次性消费）。
    返回作废记录（dict）或 None（无信号/非 nested/story 不存在）。
    调用方拿到记录即知：该 Story 下游产物需强制全量重生成。"""
    if not is_nested_state(state):
        return None
    subs = _iter_nested_story_substates(state, story_id)
    if not subs:
        return None
    rec = None
    for sub in subs:
        if sub.get("artifactInvalidated"):
            rec = sub["artifactInvalidated"]
            sub["artifactInvalidated"] = None   # 消费即清除，防累积
    if rec:
        state["activeStory"] = story_id
        record_history(state, f"story-{story_id}-invalidation-consumed", "ae-sdd")
    return rec
```

### 改动 2：`source/skill-fallbacks/skills/phase2-task/task-generate-skill.full.md` — 第三步加作废短路（prose，~4 行）

在第三步表格（`:243-249`）**之上**插入一条**最高优先级短路规则**（使其先于"描述未变→跳过"判断）：

> **🔴 作废优先（强制全量重生成）**：执行第三步前，先查 state：
> `ae-sdd state read`（或等价库调用）读取当前 Story 子状态是否有 `artifactInvalidated` 信号。
> 若存在 → **本 Story 所有 Task 文档视为不存在，走"新建"全量重新生成**（不复用、不增量更新），生成完成后由框架消费清除该信号。
> 此规则优先于下表的"描述未变→跳过"。

下表（新建/跳过/更新/废弃）措辞补一句注脚："仅当无 `artifactInvalidated` 信号时适用本表"。

### 改动 3：`tools/bin/ae-sdd` — `state read` 输出补 `artifactInvalidated` 字段（~3 行，可选但推荐）

`state read` 的 JSON/文本输出若已透传 story substate，则天然带上新字段；若做了字段白名单过滤，需补一行让它可见，便于 skill/人工排障时确认信号状态。实施时先查 `cmd_state_read` 是否有白名单再决定改不改。

### 改动 4：测试 `tools/tests/test_nested_state.py` — 补 2 个用例

复用该文件已有的 R5 reset 测试结构（`test_reset_story_substate_*` 风格）：

1. `test_reset_writes_artifact_invalidation`：reset 后 `storyStates[sid]["artifactInvalidated"]` 非空、含正确 scopes/reason，兄弟 Story 为 None。
2. `test_consume_artifact_invalidation_is_one_shot`：consume 第一次返回记录并清除字段，第二次返回 None。

### 改动 5：CHANGELOG（遵循红线 #11，文档不承载 changelog）

新增 `source/CHANGELOG/2026-07-10-v3.9.21-reset-artifact-invalidation.md`（v3.9.21），SKILL 正文若涉及 reset/relocate 描述处加一句"详见 CHANGELOG/..."引用。

## 行为验证矩阵

| 场景 | 预期 |
|---|---|
| relocate --story X（nested，X 在 coding） | X 子状态 phase→story-generated，artifactInvalidated 写入；兄弟 Story 不动 |
| 重跑 task-generate（X） | 读到信号→全量新建所有 Task→消费清除信号；旧 Task 被覆盖 |
| 重跑 testcase-generate / coding（X） | 无条件覆盖（原行为），产物刷新 |
| relocate --no-reset | 不写 artifactInvalidated（未重置=无作废） |
| flat 模型 relocate | warn 跳过（维持现状，按约定不补 flat） |
| 消费后再读 | 第二次 consume 返回 None（不累积误判） |

## 不做（边界，诚实标注）

- ❌ **不补 flat 模型回溯**（已与你确认：只管 nested）。
- ❌ **不改 save_doc / document_storage / paths**（它们无条件覆盖，本就是正确行为）。
- ❌ **不物理删/归档产物文件**（已与你确认：作废标记方案）。
- ❌ **不动 gate_intercept 相位门禁逻辑**——reset 后 phase 已是 story-generated，门禁自然要求重过 review→testcase→task，无需额外改动。
- ⚠️ **task-generate 第三步是 prose 改动，依赖 LLM 遵守短路规则**——这是本方案唯一非确定性环节。缓解：规则放在第三步最显眼位置 + 改动 3 让 `state read` 能看到信号，便于人工核对。若后续要完全机械化，可演进为 gate 层校验（本期不做，避免扩大范围）。

## 风险与回滚

- **风险**：prose 规则可能被 LLM 忽略（非机械）。但相比当前"完全无信号、纯靠文本比对"，已是严格改进；且 testcase/coding 本就强制覆盖，最坏情况只是 Task 残留，下游重跑仍会刷新。
- **回滚**：`git checkout -- tools/lib/state.py tools/tests/test_nested_state.py`；skill prose 单文件回滚。state.json 里已写的 `artifactInvalidated` 字段是自描述的，旧代码读到也会忽略（dict 多一个键不影响读逻辑）。

## 编译与分发（改完后）

1. `python -m py_compile tools/lib/state.py` 语法校验
2. `python tools/tests/test_nested_state.py`（或 pytest）跑新用例
3. `python scripts/compile_skill_runtime.py` 重新编译 SKILL（task-generate prose 变了，runtime 包需同步）
4. `python scripts/distribute.py` 同步到 `~/.claude/skills/ae-sdd/` 和 `~/.zcode/skills/ae-sdd/`