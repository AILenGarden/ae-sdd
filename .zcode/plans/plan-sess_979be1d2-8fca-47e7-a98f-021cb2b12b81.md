# CodingPlan: ae-sdd 门禁改为"按会话 engage 按需启用"

## 问题精确根因

`gate_intercept.check_intercept`(`tools/lib/gate_intercept.py:964-985`)只要找到 `.ae-sdd/` 目录,就调 `resolve_default_state` 按 state 的 phase 锁工具。而 `resolve_default_state`(`work_item_context.py:318-321`)的单候选兜底层**无条件**命中——只要项目里有 1 个 active work-item,就锁死所有会话,不管该会话是否调过 `/ae-sdd`。

## 修复语义(用户确认)

> **没调 /ae-sdd 前:hook 完全不工作。调 /ae-sdd 后:定位 state.json,定位到了才启用校验。**

用"engage 标记文件"实现:`/ae-sdd` 触发时写标记,gate 检查标记——有则锁,无则放行。

## 改动清单(3 个源文件 + 1 个 .gitignore)

### 改动 1:`tools/lib/work_item_context.py` — 新增 engage 标记读写

复用已有的 `_safe_session_file_name`(L123)、`datetime`/`json` import,新增 3 个函数:

```python
def _engaged_dir(ade_sdd: Path) -> Path:
    return ade_sdd / ".session-engaged"

def is_session_engaged(ade_sdd: Path, session_key: str) -> bool:
    """本会话是否已 engage ae-sdd（用户调过 /ae-sdd 触发词）。
    无 session_key → False（无标识的会话视为未 engage，放行）。"""
    if not session_key:
        return False
    return _engaged_dir(ade_sdd).joinpath(_safe_session_file_name(session_key)).is_file()

def mark_session_engaged(ade_sdd: Path, session_key: str) -> None:
    """记录本会话已 engage。持续态，写入后由 disengage 清除。"""
    if not session_key:
        return
    p = _engaged_dir(ade_sdd) / _safe_session_file_name(session_key)
    try:
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps({
            "engagedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        }, ensure_ascii=False) + "\n", encoding="utf-8")
    except OSError:
        pass

def disengage_session(ade_sdd: Path, session_key: str) -> None:
    """退出 engage，清除本会话标记。"""
    if not session_key:
        return
    p = _engaged_dir(ade_sdd) / _safe_session_file_name(session_key)
    try:
        p.unlink(missing_ok=True)
    except OSError:
        pass
```

### 改动 2:`tools/lib/gate_intercept.py` — `check_intercept` 增加短路判定

在 `check_intercept` 的 L965-971 之间(找到 `ade_sdd` 之后、`resolve_default_state` 之前)插入:

```python
ade_sdd = paths.locate_project_ae_sdd(project_dir)
if ade_sdd is None:
    pending = paths.pending_init_marker(project_dir)
    if pending.exists():
        return _check_pending_init_intercept(tool_name, bash_command, file_path, allow_readonly)
    return True, ""  # 非 ae-sdd 项目，不拦截

# 🆕 engage 判定：未 engage 的会话不做门禁校验（用户未调 /ae-sdd）
if not work_item_context.is_session_engaged(ade_sdd, session_key):
    return True, ""
```

后续 `resolve_default_state` 调用(L973)不变。

### 改动 3:`tools/lib/prompt_inject.py` — 触发时写 engage，退出时清标记

**(a)** 在 `inject()` 的 `_is_ae_sdd_triggered` 分支(L262 附近)加写标记:
```python
if _is_ae_sdd_triggered:
    work_item_context.mark_session_engaged(ade_sdd, session_key)  # 🆕
    # 原有 entry token 检查逻辑不变
```

**(b)** 新增退出关键词检测（仿 `QUICK_CHANNEL_MARKERS` 模式）:
```python
AE_SDD_DISENGAGE_MARKERS: tuple[str, ...] = (
    "ae-sdd-exit",
    "退出 ae-sdd",
    "不锁了",
    "解除门禁",
)
```
在 `inject()` 中（`_is_ae_sdd_triggered` 检测之后）加:
```python
_is_disengage = any(m in user_prompt for m in AE_SDD_DISENGAGE_MARKERS)
if _is_disengage:
    work_item_context.disengage_session(ade_sdd, session_key)
```

### 改动 4:`.gitignore` — 排除运行态目录

在 ae-sdd 母版或业务项目的 `.gitignore` 加（与 `.ae-sdd/session-context/` 同级）:
```
.ae-sdd/.session-engaged/
```

## 行为矩阵（验证依据）

| 场景 | engage 标记 | gate 行为 | 预期 |
|---|---|---|---|
| life 项目 + 调了 /ae-sdd | 有 | 按 phase 锁 | ✅ |
| life 项目 + 没调 ae-sdd 的会话/子Agent | 无 | 放行 | ✅ |
| life 项目 + 调 /ae-sdd 后说"退出 ae-sdd" | 已清除 | 放行 | ✅ |
| 非 ae-sdd 项目 | 无 .ae-sdd/ | 放行（原逻辑） | ✅ |
| 同一会话子 Agent | 继承 session_key | 随主会话 | ✅ |

## 编译与分发（改完源码后）

1. `python scripts/distribute.py`（copytree 类分发器）把 3 个改动 lib 同步到 `~/.claude/skills/ae-sdd/tools/lib/`
2. `tools/bin/ae-sdd` 通过 `from lib import ...` 引用，无需重新打包 bin
3. 同步到 `~/.zcode/skills/ae-sdd/`（ZCode 分发副本，用同样脚本或 dev_sync.py）
4. 重启 Claude/Codex 会话让新 lib 生效

## 验证步骤

1. **JSON 合法性**：改动后 `python -c "import ast; ast.parse(open('tools/lib/gate_intercept.py').read())"` 校验 3 个文件语法
2. **单元验证 engage 函数**：在 life 项目目录模拟 — 写一个假 session-key，调 `is_session_engaged` 应 False，`mark_session_engaged` 后应 True，`disengage_session` 后应 False
3. **端到端**：
   - life 项目新开会话，不调 /ae-sdd，直接发 Bash → 应放行（对比改之前被锁）
   - 同会话调 /ae-sdd → 再发写操作 → 应按 phase 锁
   - 说"退出 ae-sdd" → 再发写操作 → 应放行

## 风险与不确定性（诚实标注）

1. **子 Agent session 继承**：假设子 Agent 继承父会话 session_id（主流客户端行为）。若某些客户端给子 Agent 独立 session_id，子 Agent 会被误放行——但这符合"宁放勿锁"取向。
2. **首次调用窗口**：用户发 `/ae-sdd` 当轮，prompt_inject 写标记；但同轮的 tool 调用 gate-intercept 可能先于 prompt_inject 跑（PreToolUse vs UserPromptSubmit 时序）。实际 UserPromptSubmit 在用户发消息时立即跑，早于任何 PreToolUse，所以无窗口问题——但需实测确认。
3. **多客户端 session_id 格式差异**：Claude 用 conversation_id，Codex 可能不同。`_safe_session_file_name` 用 sha256 hash 兼容任意格式，无影响。
4. **不特殊处理母版仓库 NoWorkItemStateError**：母版 `.auto-engineering/` 为空，engage 后会触发该异常被拒。但母版不该跑业务流程，概率极低，不特殊处理。

## 回滚

源码改动通过 git 回滚（`git checkout -- tools/lib/`）。分发副本保留旧版备份。