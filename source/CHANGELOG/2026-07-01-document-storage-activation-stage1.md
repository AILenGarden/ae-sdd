# 2026-07-01 | document_storage 激活阶段1：CLI + 代码层 + 测试（v3.7.2）

## Summary

把 `document_storage.py` 从"孤岛"（零调用方）激活为"真正可被运行时调用"。新增 `ae-sdd doc save/resolve/finalize` 三个 CLI 子命令，LLM 可通过命令行真正调用文档存放 API，而非读伪代码手模拟。这是路线B（CLI + hook + 文档规范化）的阶段1：代码层 + 测试。

## 背景

document_storage.py v4.1（2026-06-27）已实现 14 个 API，但：
- `tools/bin/ae-sdd` import 了 15 个 lib 模块，**唯独漏掉 document_storage**
- 276 处 SKILL 文档里的 `save_doc()`/`resolve_path()` 是伪代码契约，LLM 读后自己拼路径，**document_storage 从未被运行时执行**
- `gate doc-storage` 命令自己重写一套字符串匹配，绕过 document_storage

本次（阶段1）打通"代码可用"这一层，为后续阶段2（276 处文档规范化）提供可引用的真实命令。

## Changes

### 1. ae-sdd CLI 新增 `doc` 命令组（tools/bin/ae-sdd）

| 子命令 | 用途 | 复用 |
|--------|------|------|
| `ae-sdd doc save` | 一步到位存文档（resolve→写→版本→ChangeLog→STORING→gitignore→删草稿）| `document_storage.save_doc()` |
| `ae-sdd doc resolve` | 只推路径不写（查会写到哪）| `document_storage.resolve_path()` |
| `ae-sdd doc finalize` | 已手写文件补版本号/ChangeLog/STORING（不覆盖内容）| `document_storage.finalize_doc()` |

- 顶部 import 补 `document_storage`（L76，从 15 模块增至 16）
- 新增 `_resolve_ade_sdd_and_project_key()` 辅助函数（统一解析 ade_sdd + project_key）
- argparse 注册 doc 命令组 + set_defaults 绑定 cmd_doc_*

### 2. document_storage.py 新增 finalize_doc + 修复 2 个 v4.1 遗留 bug

**新增：**
- `finalize_doc()` — 对已手写文件补后处理（ChangeLog/STORING/gitignore），不覆盖内容。适用未实现 intent 的文档手写后补登记。
- `E009` 错误码 — finalize 目标文件不存在

**修复 2 个 v4.1 遗留 bug（因零调用方从未暴露，由本次测试驱动发现）：**

| Bug | 根因 | 修复 |
|-----|------|------|
| STORY 类文档路径为 `Story/.md`（docId 空）| resolve_path 占位符替换时 `{docId}` 只读 doc_id/task_name，不回退 story_id | docId 回退链：`doc_id > task_name > story_id` |
| 事件类报告版本不自增（都写 v1-r1 互相覆盖）| (1) `_VERSION_RE` 正则只匹配点格式 `-v1.0`，漏 dash 格式 `-v1-r1`；(2) glob `-v*.*.md` 同样漏；(3) save_doc 自增版本号后未重 resolve 路径 | 正则改双格式匹配；glob 改 `-v*.md`；自增后重新 resolve 拼路径 |

### 3. 测试（从 3 用例增至 21 用例 + 新增 8 用例 CLI 测试）

| 文件 | 新增 |
|------|------|
| `test_document_storage.py` | 3→13 用例：save_doc 写文件/ChangeLog/STORING/gitignore/版本自增/E000 + finalize_doc 不覆盖/ChangeLog/STORING/E009 |
| `test_cli_doc.py`（新增）| 8 用例：doc save 端到端/keep-draft/E000/版本自增 + doc resolve 不写文件/JSON + doc finalize 不覆盖/不存在文件 |

## 验证

| 测试套件 | 用例数 | 结果 |
|---------|--------|------|
| test_document_storage.py | 13 | ✅ 全过 |
| test_cli_doc.py | 8 | ✅ 全过 |
| test_state.py | 44 | ✅ 全过（未受影响）|
| test_gates.py | 106 | ✅ 全过（未受影响）|

**手动端到端验证**（临时项目）：
```
ae-sdd doc resolve --intent STORY --story-id STORY-001-BE
  → 输出：ae-sdd-doc/Story/STORY-001-BE.md ✓

ae-sdd doc save --intent STORY --story-id STORY-001-BE --content-file 草稿.md --changelog-note "首次创建"
  → 写文件 ✓ / ChangeLog 同级生成 ✓ / STORING 更新 ✓ / .gitignore 维护 ✓ / 草稿删除 ✓
```

## 激活前后的架构变化

```
激活前（孤岛）：
  SKILL 文档 save_doc(...) → LLM 读后手拼路径 → 直接 Write
  document_storage.py → 仅 test 调用

激活后（阶段1，代码可用）：
  ae-sdd doc save --content-file 草稿 → document_storage.save_doc() → resolve+写+版本+ChangeLog+STORING+gitignore
  LLM 只需：Write 草稿 → 调 1 次命令
```

## 后续阶段（待执行，不在本次范围）

- **阶段2**：276 处 SKILL 文档规范化（4 批：高密度核心 → 中密度 → 低密度 → document-storage-skill.md 契约更新），把伪代码改为真实 CLI 命令引用
- **阶段3**（可选）：PreToolUse hook（gate_intercept）升级，产物落地校验指向 `ae-sdd doc save`
- **阶段4**（可选）：gates.py G-DOC-STORAGE 门禁从硬编码合规根改为 `paths.resolve_doc_workspace()` 真值校验

## Sync

- 本次修改 `tools/bin/ae-sdd` + `tools/lib/document_storage.py` + 2 个测试文件。
- `dev-sync` 需在 update-check 通过后执行。
