# 2026-07-08 | ae-sdd - life 项目 R6 顶层名迁移示例（LIFE R6 migration guide）

## Summary

2026-07-08 life 项目实测：`D:\Item\life\.auto-engineering\STORY-004-BE--车主端预约单操作-BE\`（v3.8.2 双段目录）+ `.ae-sdd/state.json` 含 `stateMachineId=STORY-004-BE--车主端预约单操作-BE`（v3.8.2 双段 ID）+ `stateMachineName` 冗余字段 —— 这三项都是 v3.9.3 R6 顶层名规则的反模式。

本次**只动了 life 项目侧的 state 文件**，**未改 ae-sdd 代码**。目的是把这次迁移做成"v3.9.3 R6 迁移 SOP 案例"，让其它项目遇到同类情况有据可循。

## Why

### v3.9.3 的 R6 顶层名是强制规则

`source/CHANGELOG/2026-07-07-v3.9.3-r6-r2-mandatory.md` 第 5-15 行：

> "🆕 v3.9.3 实施：废除 v3.8.2 双段 `{ID}--{name}` 目录命名 + `--name` 形参，**全部走 R6 顶层名**（`PRD-{特征}` / `DR-{特征}` / `Story-{合并编号}` / `Task-{task_id}`）"
> "⚠️ **BREAKING CHANGE**：
>   - `work_item_dir_name(id, name)` → `work_item_dir_name(top_node, features)` 签名变更
>   - `cmd_state_new --name` 形参废除（required 改 optional）
>   - `cmd_enter --story` 不再支持裸传（需 `--work-item <R6 顶层名>` 或 `--story <STORY-ID>`）
>   - 旧 `STORY-XXX--YYY/` 双段目录不再被自动命中，需手工 `state relocate --story STORY-X` 迁移"

### R6 顶层名算法（`tools/lib/paths.py:537-580`）

```python
def build_state_machine_name(top_node: str, features: dict) -> str:
    """R6: 只以最顶层主体特征命名 state."""
    if top_node == "STORY":
        nums = [_extract_story_number(sid) for sid in features["story_ids"]]
        # _extract_story_number(STORY-004-BE) -> "004"
        return "Story-" + "-".join(nums)
```

`STORY-004-BE` → R6 = **`Story-004`**（BE 是子分支后缀，不进顶层名）。

### life 项目迁移前/后对照

| 字段 | 迁移前 | 迁移后 |
|---|---|---|
| `.ae-sdd/state.json` : `stateMachineId` | `STORY-004-BE--车主端预约单操作-BE` | `Story-004` |
| `.ae-sdd/state.json` : `stateMachineName` | `车主端预约单操作-BE` (冗余) | **删除** |
| `.ae-sdd/state.json` : `currentWorkItem` | `STORY-004-BE--车主端预约单操作-BE` | `Story-004` |
| `.ae-sdd/state.json` : `workItemKey` | `STORY-004-BE--车主端预约单操作-BE` | `Story-004` |
| `.ae-sdd/state.json` : `activeWorkItem` | `STORY-004-BE--车主端预约单操作-BE` | `Story-004` |
| `.ae-sdd/state.json` : `activeStatePath` | `D:\Item\life\.auto-engineering\STORY-004-BE--车主端预约单操作-BE\state.json` | `D:\Item\life\.auto-engineering\Story-004\state.json` |
| `.ae-sdd/state.json` : `activeStory` | `STORY-004-BE` | **`STORY-004-BE` (不动 — Story ID，不是 work-item key)** |
| `.ae-sdd/state.json` : `storyStates.STORY-004-BE.phase` | `story-generated` | `story-generated` (不动) |
| `.auto-engineering/` 目录名 | `STORY-004-BE--车主端预约单操作-BE/` | `Story-004/` |

**关键不变量（`activeStory`）** — 是 Story ID (`STORY-004-BE`)，**不是** work-item key。`storyStates[]` 字典查找依赖它，必须保留全名。R6 改的只是 work-item 容器，不改 Story 本体。

## 迁移 SOP（5 步）

> 适用：升级 v3.9.3+ 后，`.auto-engineering/` 仍含 `{ID}--{name}/` 双段目录的项目。

### Step 0: 备份
```bash
cp .ae-sdd/state.json state.json.bak-$(date -u +%Y-%m-%d)-r6-mv
cp .auto-engineering/<OLD_DIR>/state.json state.json.bak-$(date -u +%Y-%m-%d)-r6-mv
cp .ae-sdd/memory/.stage/*.json ...       # 占位 token 一并备份
cp .ae-sdd/.quick_channel ...             # 若用 quick_channel 临时 bypass
```

### Step 1: MV 目录
```bash
# Linux / WSL / MSYS bash
mv ".auto-engineering/<OLD_DIR>" .auto-engineering/<R6_NAME>
# Windows PowerShell
Rename-Item ".auto-engineering\<OLD_DIR>" "<R6_NAME>"
# R6_NAME 公式 = build_state_machine_name(topNode, features)
#   STORY 入口 + STORY-XXX-BE → "Story-XXX"
#   PRD 入口 + PRD-IM-CS     → "PRD-IM-CS"
#   DR 入口  + DR-CS         → "DR-CS"
#   TASK 入口 + BUG-LIFE-001 → "Task-BUG-LIFE-001"
```

### Step 2: 改两份 state.json 的 5 个字段
```python
import json, time
from pathlib import Path
NOW = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())

for p in [
    Path('.ae-sdd/state.json'),
    Path(f'.auto-engineering/<R6_NAME>/state.json'),
]:
    d = json.loads(p.read_text(encoding='utf-8'))
    for k in ['stateMachineId', 'currentWorkItem', 'workItemKey', 'activeWorkItem']:
        d[k] = '<R6_NAME>'
    d['activeStatePath'] = str(p.parent / 'state.json')  # 或绝对路径
    d['lastUpdated'] = NOW
    d.pop('stateMachineName', None)  # v3.9.3 R6 不再需要
    d.setdefault('migrationEvents', []).append({
        'type': 'rename-to-r6',
        'oldStateMachineId': '<OLD_ID>--<OLD_NAME>',
        'newStateMachineId': '<R6_NAME>',
        'changelog': 'source/CHANGELOG/2026-07-08-life-r6-rename-guide.md',
        'timestamp': NOW,
        'by': 'manual r6 migration',
    })
    p.write_text(json.dumps(d, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
```

### Step 3: 校验
```bash
ae-sdd state read --json           # stateMachineId == R6_NAME
ae-sdd state next-step --json      # next = story-reviewed (phase 不动)
ae-sdd state relocate --story <STORY-ID> --no-reset  # 应能重新命中
ae-sdd gates check --only G-STORY-CTX --json  # 通过
```

实测 4 个命令全 exit 0，无 warning。

### Step 4: 后续 Housekeeping
- 旧目录备份文件 `state.json.bak-*-r6-mv`：**保留 7 天后人工删除**
- 旧目录残留空目录（若有冲突时残留）：手动 `rmdir`
- `.quick_channel` 临时 bypass 文件：完成 Story Review 后**手动删除**恢复门禁保护

## 影响范围

### 行为变更
- ae-sdd 代码**未改** —— 这是一份迁移 SOP 文档，不改 runtime 行为
- v3.9.3 设计早已要求手工迁移，本文档是给后续遇到 legacy 双段目录的项目 owner 看的指南
- 不破坏现有 flat state 项目（flat 不依赖 work-item 目录结构）

### 向后兼容性
- v3.8.2 双段目录本身仍被 `find_work_item_state_path` 通过 `prefix` 模糊匹配找到（`tools/lib/paths.py:225-230`），但**不再自动命中**，需要 R6 重命名
- 旧 `state new --name` 调用仍可用（`--name` 变 optional，值被忽略）

### 没改 activeStory 的理由
`activeStory = STORY-004-BE` 是 ae-sdd v3.9.0 R3 子状态容器的 key (`storyStates[activeStory]`)——如果改成 `Story-004` 会破坏：
- `find_nested_state_by_story_id(ade_sdd, story_id)` (line 527)
- `reset_story_substate(state_data, story_id, ...)` (line 547)
- `state write --sub-story STORY-XXX-BE` 的所有命令族

所以**只改 work-item key，不动 Story ID**。

## Verification（life 项目实测）

| 命令 | 结果 |
|---|---|
| `ae-sdd state read --json` | exit 0，stateMachineId = `Story-004` |
| `ae-sdd state next-step --json` | exit 0，current=story-generated, next=story-reviewed |
| `ae-sdd state relocate --story STORY-004-BE --no-reset` | exit 0，已重定位到 `Story-004` |
| `ae-sdd gates check --only G-STORY-CTX --json` | exit 0，scale=小 豁免 |

## 关键不变量（迁移前后必须保留）

1. `activeStory`（Story ID） —— **不改**
2. `storyStates.STORY-XXX-BE.phase` —— **不改**（phase 推进独立于重命名）
3. `history` 字段 —— **不改**（审计轨迹）
4. `createdAt` —— **不改**（生命周期起点）
5. `stateMachineName` —— **删除**（v3.9.3 不再需要）

## Reviewer

陈聪

## 相关 CHANGELOG

- 2026-07-07-v3.9.3-r6-r2-mandatory.md（R6 顶层名 + R2 强制向上归入）—— 本迁移 SOP 依据
- 2026-07-06-v3.9.0-nested-state-model.md（嵌套 state 模型）—— `storyStates[activeStory]` 子容器设计
- 2026-07-08-v3.9.7-gate-intercept-memory-dir-lazy-init.md（fix-life-deadlock） —— 同期修复
