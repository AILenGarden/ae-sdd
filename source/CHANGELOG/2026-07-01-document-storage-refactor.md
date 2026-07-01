# 2026-07-01 | document-storage-skill 结构重构（v3.7.1 配套）

## Summary

对 `document-storage-skill.md`（AE 体系文档存放横切依赖）做结构重构 + 代码同步：消除 §0.x 游离编号、3 处文档/代码矛盾、3 组三重描述冗余，使 SSOT 文档与 `document_storage.py` 实现完全对齐。

## 核心决策

| 决策点 | 选择 |
|--------|------|
| 设计类文档方向 | **跟随代码：原地更新**（不带版本号、不进 iterations/）|
| 本次范围 | **.md 文档重构 + document_storage.py 同步修正** |

## Changes

### 1. document-storage-skill.md 结构重构（主体）

| 类别 | 动作 |
|------|------|
| **消除 §0.x 游离编号** | 原 §0.5 工程解耦 → §3；原 §0.6 API 契约 → §4 |
| **§1 文档分类与目录结构** | 合并原 §1 + §2.1~2.4（含路径模板总表、资产路径、迭代目录、旧路径兼容层）|
| **§2 命名与版本号规则** | 原 §3 剔除 §3.5（state schema 外迁）；🔴 修正：设计类"不带版本号、原地更新" |
| **§3 工程解耦定位原则** | 原 §0.5 提升为正文（五维模型/动态定位算法/资产依赖/硬约束）|
| **§4 动态定位 API 契约** | 原 §0.6 提升为正文（14 个 API 唯一 SSOT）|
| **§5 重入流程与文档演进** | 合并原 §4 + §9 + §10 + §11 |
| **§6 ChangeLog 与迭代关联** | 合并原 §5 + §6（关联性算法单一权威表）|
| **§7 .gitignore** | 原 §7（不变）|
| **§8 存量迁移** | 原 §8（迁移目标表合并到 §1.6）|
| **§9 横切调用规范** | 合并原 §15.1~15.4 + §15.5，调用矩阵合并为单一表（14 行覆盖全部调用方）|
| **附录 A** | 原 §3.5 PRD state.json schema 降级为附录（schema 读写归 state.py，本 SKILL 只管路径）|

### 2. 三处矛盾收敛（跟随代码方向）

| 矛盾 | 修正 |
|------|------|
| **矛盾1：设计类版本号** | §1.1/§2.2/§4.10 统一为"设计类不带版本号、原地更新"；删除 §3.2/§3.4 旧"版本化"描述；迭代目录例子改为事件类报告 |
| **矛盾2：ChangeLog 位置** | §6.1 改为"文档同级目录"（与 save_doc L212/L399 一致）；删除 §0.6.12 "跨迭代合并"承诺 |
| **矛盾3：STORING.md** | §4.4 改为"单一 ae-sdd-doc/STORING.md"（与 update_storing_index L421 一致）|

### 3. 三重描述收敛

| 冗余组 | 收敛 |
|--------|------|
| API 契约（原 3 处）| §4 唯一 SSOT；头部表降为指针；删除 §15.5 代码示例 |
| 关联性算法（原 3 处）| §6.3 唯一权威表；§4.5 仅列 API 签名指向 §6.3；头部表删除 |
| 调用矩阵（原 2 处）| §9.1 合并为单一表（14 行），删除旧 §15.1/§15.5.2 |

### 4. document_storage.py 同步修正

| # | 修正 | 位置 |
|---|------|------|
| C1 | save_doc 补 check_and_update_gitignore 调用（对应 §7.3 承诺）| save_doc() 写文件后 |
| C2 | `TRACEABILITY` → `TRACE_MATRIX`（与 §4.10 intent 枚举表一致）| _PATH_TEMPLATES L46 |
| C3 | 19 处 docstring 锚点 §0.6.x → §4.x（对齐新章节编号）| 全文 |

### 5. intent 枚举表新增"实现状态"列

§4.10 intent 枚举表新增 ✅（已实现）/📝（文档登记代码未实现）标记，27 个 intent 中 15 个 ✅、12 个 📝。

### 6. 引用方锚点同步（7 文件，26 处）

| 文件 | 替换数 |
|------|--------|
| orchestration/ae-sdd-update-skill.md | 16 |
| cross-cutting/project-assets-update-skill.md | 3 |
| phase3-review/code-review-skill.md | 1 |
| cross-cutting/proposal-skill.md | 1 |
| phase1-design/dr-update-skill.md | 1 |
| phase1-design/requirement-analysis-skill.md | 1 |
| phase2-task/task-generate-skill.md | 1 |

映射：§0.5.1→§3.1、§2/§2.3/§2.6→§1.3/§1.4/§1.6、§3.5→附录A、§15.1/§15.5/§15.5.2→§9/§9.1。

## 验证

- ✅ `python tools/tests/test_document_storage.py`（3 用例全过）
- ✅ `python tools/tests/test_state.py`（44 用例全过，state 未受影响）
- ✅ document_storage.py 语法解析 + 导入通过（TRACE_MATRIX 存在、TRACEABILITY 已清除）
- ✅ 引用方旧锚点复查：§0.5/§0.6/§2.6/§3.5/§15.x 残留清零

## 后续待办（不在本次范围）

- 12 个 📝 未实现 intent 的模板补全（STORY_SUPPLEMENT/DR_SUPPLEMENT/ISSUE/TESTCASE_REVIEW 等）
- update_storing_index scope 参数生效（小任务旧路径分支）
- document_storage.py 零测试覆盖区域补测试（save_doc 主体/RA 门禁/关联性 API）

## Sync

- 本次修改 `source/` 母版 + `tools/lib/document_storage.py`。
- `dev-sync` 需在 update-check 通过后执行。
