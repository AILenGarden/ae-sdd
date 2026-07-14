# CodingPlan: compact 语义锚点校验（借鉴 caveman-compress validate.py 思路）

## 需求拆解与边界

**目标**：给 ae-sdd 编译产物的 verify 阶段补一层「compact 与 fallback 的语义锚点比对」，让 `_CORE_MAX_LINES` 截断 / `_is_core_structural` 漏判导致的锚点丢失**可见**。借鉴 caveman-compress `validate.py` 的「逐字一致性校验」思路，但映射到 ae-sdd 的静态编译场景。

**三个已确认决策**：
1. 落点：扩 `tools/lib/runtime_verify.py`（复用现有子 SKILL 遍历循环 L243），不改编译器，不新增 CLI flag。所有调用方（CLI runtime verify / distribute.py / distributors / UC-15）自动获得新校验。
2. 锚点比对：纯机械比对，fallback 抽出的锚点 token 要求每个在对应 core 里至少出现一次。不做白名单。
3. 强度：**warning 不阻断**。锚点丢失记入 `warnings`（不影响 `ok`），同时修复一个真 bug（L267 四件套漏 `core`）记入 `issues`。

**边界**：
- 只校验**子 SKILL**（有 core.compact.md 的），不校验顶层 6 个主 compact（它们由 render_xxx_compact 函数直接生成，不走截断逻辑，无 core 概念）。
- 校验锚点正则与编译器 `_is_core_structural:302` **完全一致**：`\b(G-[A-Z0-9-]+|RA-G\d+|TR-\d+|TC-\d+|TV-\d+)\b`。单一正则定义，校验侧与编译侧对齐。

## 现状缺口（三个 Explore 已确认）

| # | 缺口 | 位置 | 性质 |
|---|------|------|------|
| A | L267 四件套 `("manifest","boot","outline","fallback")` 漏 `core`，core.compact.md 连存在性都没校验 | `runtime_verify.py:267` | 真 bug（issue） |
| B | compact 与 fallback 无任何内容比对，截断丢失锚点无法检出 | `runtime_verify.py` 全文 | 功能缺失（warning） |
| C | `core_sha256` 字段在 manifest 里已存在，verify 从不读 | `runtime_verify.py` | 顺手补（issue） |

## 实现步骤

### 步骤 1：`tools/lib/runtime_verify.py` 加锚点比对函数（核心）

在文件头部加模块级正则常量与提取函数：

```python
# 与 compile_skill_runtime.py:_is_core_structural:302 对齐，单一锚点定义源
_CORE_ANCHOR_RE = re.compile(r"\b(G-[A-Z0-9-]+|RA-G\d+|TR-\d+|TC-\d+|TV-\d+)\b")

def _extract_anchors(text: str) -> set[str]:
    return set(_CORE_ANCHOR_RE.findall(text))
```

### 步骤 2：子 SKILL 遍历循环内（L243-312）插入校验

在 L273 四件套存在性校验**之后**、L275 子 manifest 校验**之前**，插入：

```python
# (1) 修复缺口 A：四件套加 core
# L267 改为: for key in ("manifest", "boot", "outline", "core", "fallback"):

# (2) core 存在性 + sha256（修复缺口 C，issue 级）
core_rel = record.get("core")
if isinstance(core_rel, str):
    core_path = package / core_rel
    if not core_path.is_file():
        issue(f"child SKILL core.compact.md missing: {core_rel}")
    else:
        # sha256 校验（与 fallback 同模式，L300-310）
        core_text = core_path.read_text(encoding="utf-8", errors="replace")
        expected_core_hash = record.get("core_sha256")
        actual_core_hash = _sha256_text(core_text)
        if expected_core_hash and expected_core_hash != actual_core_hash:
            issue(f"child SKILL core hash mismatch: {entry}")
        
        # (3) 锚点比对（缺口 B，warning 级，不阻断）
        if core_rel:  # core 文件可读才比对
            child_fb_rel = record.get("fallback")
            if isinstance(child_fb_rel, str) and (package / child_fb_rel).is_file():
                child_fb_text = (package / child_fb_rel).read_text(encoding="utf-8", errors="replace")
                fb_anchors = _extract_anchors(child_fb_text)
                core_anchors = _extract_anchors(core_text)
                lost = fb_anchors - core_anchors
                if lost:
                    warn(f"core lost {len(lost)} anchors vs fallback: {entry}: {sorted(lost)[:10]}{' ...' if len(lost)>10 else ''}")
```

**注意**：sha256 比对用 `record.get("core_sha256")`（父 manifest 已有此字段），与 fallback 的 `fallback_sha256`（L300-310）完全对称。

### 步骤 3：测试 `tools/tests/test_runtime_verify.py` 加 3 个用例

照搬现有 unittest + 临时目录假包模式（`_write_package` / `_add_compiled_child`）。

**用例 1：core 锚点丢失触发 warning**（正例校验逻辑）
- 构造假子 SKILL，fallback 含 `G-DOC-STORAGE`、`TC-2`，core 删掉它们
- 断言 `result.ok is True`（warning 不阻断）+ `any("lost" in w and "G-DOC-STORAGE" in w for w in result.warnings)`

**用例 2：core 文件缺失触发 issue**（反例，修复缺口 A 的验证）
- 删掉 core.compact.md
- 断言 `result.ok is False` + `any("core" in i for i in result.issues)`

**用例 3：core 无锚点丢失时无 warning**（干净通过）
- fallback 与 core 锚点一致
- 断言 `result.ok is True` + `not any("lost" in w for w in result.warnings)`

**前置**：`_write_package` / `_add_compiled_child` 需补 `core` 路径与 `core_sha256` 字段到假 manifest，否则用例 1/2 会先撞别的 issue。

### 步骤 4：跑测试 + update-check

```bash
python tools/tests/run.py                    # 全量 unittest
python tools/bin/ae-sdd update-check --affected scripts/compile_skill_runtime.py --json
```
update-check 连带项（UG-19/UG-24）：runtime_verify.py / test_runtime_verify.py / ae-sdd-update-skill.md / CHANGELOG。核对全部覆盖。

### 步骤 5：验证真实产物 warning 输出

```bash
python tools/bin/ae-sdd runtime verify --path dist/ae-sdd
```
预期：`ok=True`，warnings 含 11 个子 SKILL 的锚点丢失报告（69 token）。确认 warning 内容可读、不刷屏。

## 发布流程（self-update SOP）

1. **版本号三处同步** 3.10.4 -> 3.10.5：
   - `tools/lib/paths.py:18` `MASTER_VERSION`
   - `source/SKILL.md:3` `version:`
   - `README.md:5` 版本行
2. **CHANGELOG**：`source/CHANGELOG/2026-07-13-v3.10.5-compact-anchor-verify.md`，记录「runtime_verify 新增 compact↔fallback 锚点比对（warning）+ 修复 core 四件套遗漏 bug」。正文只留最终状态，不混历史。
3. **重编 dist**：`python scripts/compile_skill_runtime.py`（runtime_verify.py 改动会随 dist 重建，确认 dist/tools/lib/runtime_verify.py 同步）。
4. **update-check 全绿** + **runtime verify 通过**（ok=True）。
5. **提交**：当前分支 `feat/v3.10-simplify-flow-no-task-no-changelog`，commit message `release(v3.10.5): compact 语义锚点校验 + core 四件套 bug 修复`。

## 风险与应对

| 风险 | 应对 |
|------|------|
| warning 刷屏（69 token 11 个子 SKILL） | 每个 warn 只列前 10 个丢失 token + `...`，不逐个列全 |
| 锚点正则与编译器漂移 | 正则定义写在注释里标注 `# 对齐 compile_skill_runtime.py:302`，单一源 |
| `_add_compiled_child` fixture 改动牵连现有测试 | 先跑现有测试确认基线绿，改完再跑回归 |
| dist 重编后 runtime_fingerprint 变化 | 正常现象（source 没变但 tools/lib 变了，fingerprint 含 tools checksum）；确认 boot.compact.md 的 fingerprint 与 manifest 一致即可 |

## 不做的事

- ❌ 不改编译器 `render_core_compact`（max_lines 截断策略）——warning 让丢失可见，后续单独迭代修编译器
- ❌ 不加 `--verify-semantic` CLI flag——复用现有 `runtime verify` 命令
- ❌ 不做锚点白名单——纯机械比对，占位符 G-XX 若被截断属可接受丢失
- ❌ 不校验顶层 6 个主 compact——它们不走截断逻辑
- ❌ 不引入 runtime_verify 对 gates.py 的依赖——白名单方案已否决