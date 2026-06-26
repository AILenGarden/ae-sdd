# 资产路径治理设计文档（v4.0 重构方案 A）

> **状态：待评审（🔴 未确认，禁止进入编码）**
> **创建：2026-06-26**
> **背景：** 源于"技能加载报 project-assets 路径过时"的排查，逐步揭示 ae-sdd 资产路径体系存在系统性治理缺失。

---

## 一、问题陈述

ae-sdd 的"项目资产存哪里"这件事**没有单一权威源（SSOT）**。代码、schema、5 个 skill、3 个 template 各说各话，且存在四个互相打架的维度。本设计文档基于全量实测调研（非推断），提出完整重构方案。

---

## 二、现状全量盘点（实测，含 file:line 证据）

### 维度 1：`assets/` vs `project-assets/` —— 文档与代码打架

**统计：27 处资产路径引用，分成两派**

| 派系 | 路径 | 出现位置 | 数量 |
|------|------|---------|:----:|
| **代码派（实际生效）** | `assets/{projectKey}/` 或 `.ae-sdd/assets/` | `gates.py:131`、`paths.py:140-158`、`ae-sdd` CLI:1445/1463/1469、`document-storage-skill:172/223/250` | ~10 |
| **文档派（规范目标）** | `skills/ae-sdd/project-assets/{projectKey}/` | `project-assets-schema.md`(8处)、`project-assets-update-skill.md`(11处)、`coding-skill.md:1100`、3个template、`be-coding-plan-template.md:48`、`be-codereview-template.md:426` | ~27 |

**关键事实：代码全部认 `assets/`，规范文档全部写 `project-assets/`。Agent 按规范去 `project-assets/` 找文件永远找不到（404）。这是当前正在制造故障的 BUG。**

### 维度 2：母版示例 vs 业务项目 —— 两套 `assets/` 含义不同

| 哪里的 assets/ | 含义 | 谁放 | 例子 |
|---------------|------|------|------|
| `ae-sdd仓库/source/assets/` | 母版自带的**示例/范本**资产 | ae-sdd 团队 | icec-cloud-boss |
| `<业务项目>/.ae-sdd/assets/` | 业务项目**自己的**资产 | 业务项目方 | life 项目 |

**两者都叫 assets 但完全不同。** 安装态的 `~/.zcode/skills/ae-sdd/assets/` 是前者（母版示例），非业务项目资产。

### 维度 3：集中扁平 vs 跟工程走 —— 当前机制不支持项目真实结构

**当前机制**（`gates.py:131`）：
```python
asset_file = ade_sdd / "assets" / f"{project_key}.assets.md"
```
→ 一个项目只认一个扁平文件，所有微服务塞进一份资产。

**用户主张**：资产分两层，跟项目真实结构走——
- **总览**（工作区级索引/映射）放工作区根
- **各工程细节**（单工程 DDD/核心类/部署/契约）就近存放

**关键纠正**：路径形态由项目实际结构决定，ae-sdd **不预设中间层级**（有无 2c/admin、叫什么，项目自己定）。

**实测 life 项目现状**印证必要性：life 有 2c/admin/common 等类目录，但资产散落在 `document/life-team-ai-standards/skills/ae-sdd/assets/`（还重复两份），既不在门卫认的 `.ae-sdd/assets/`，也不跟工程走。

### 维度 4：document-storage-skill 治理缺口 + 关键事实

**🔴 关键发现（第 4 次调研才确认）：document-storage 的 `get_assets()` / `resolve_path()` 只是 SKILL.md 文字契约，没有 Python 实现。**

实测：
- `grep "def get_assets"` 在整个 `tools/` `scripts/` **零命中**
- `tools/lib/` 下**没有 document-storage 模块**
- document-storage-skill 是**指令式文档**（告诉 Agent 怎么做），不是可执行代码

**这意味着**：代码层路径 SSOT 必须是 `paths.py`（真实存在，有 `find_asset_file`/`assets_dir`），不是不存在的 document-storage API。document-storage-skill 作为"文档层 SSOT"（规范 Agent 行为）是对的，但代码层靠 paths.py。

---

## 三、根因

**资产路径治理缺失单一权威源 + document-storage 的治理定位名存实亡（有文字契约无代码、有 API 无人调）。** 三个维度的分歧都是这个根因的症状。

---

## 四、设计目标（用户主张）

1. **路径治理归 document-storage-skill（文档层）+ paths.py（代码层）**：代码层 SSOT 是 paths.py（因 document-storage API 不存在）。
2. **资产"总览 + 各工程细节"两层模型**：总览放工作区根，工程级细节按工程分目录（放 docWorkspacePath 下，跟代码分离，不污染业务仓库）。
3. **路径形态不预设**：ae-sdd 不预设中间层级，由项目实际结构决定。

---

## 五、可行性依据（已有机制）

| 机制 | 现状 | 复用价值 |
|------|------|---------|
| schema §15"主体+工程级子文件"模型 | ✅ 已设计完整（§15.1 拆分判定 + §15.4 分工表）| 模型已就绪，只差存放位置 |
| document-storage §0.5.1 四维定位模型 | ✅ 已有 `docWorkspacePath`（第四维，为 life 这种工程≠文档项目设计）| 支持"资产跟代码分离" |
| paths.py `find_asset_file`/`assets_dir` | ✅ 真实代码，调用点仅 2 处（CLI）| 可作为代码层 SSOT 扩展 |

---

## 六、详细改动清单（基于零未知调研）

### 6.1 阶段 1：路径统一 + 代码 SSOT 归位

| # | 改动 | 文件:行 | 风险 |
|---|------|--------|:----:|
| 1.1 | 27 处文档 `skills/ae-sdd/project-assets/{projectKey}/` → `assets/{projectKey}/` | schema.md、update-skill.md、coding-skill.md:1100、3 template、be-coding-plan:48、be-codereview:426 | 🟢 低 |
| 1.2 | document-storage 内部口径统一为 `assets/` | document-storage-skill.md:172/223/250 | 🟢 低 |
| 1.3 | `gates.py:131` 硬拼路径改调 `paths.find_asset_file()` | gates.py:131 | 🟡 中（动门卫）|
| 1.4 | paths.py 加 `docWorkspacePath` 读取能力（现完全无此能力）| paths.py 新增函数 | 🟡 中 |

**注意**：`standards/project-assets/` 这类**目录名**不改（它是 standards 下的规范目录，不是资产存放路径）。

### 6.2 阶段 2：assets_index 搜索引擎多文件化（🔴 高风险深水区）

**这是独立的技术债，与路径治理是两个问题。** assets_index.py 全程"单文件假设"：

| # | 改动 | 文件:行 | 说明 |
|---|------|--------|------|
| 2.1 | `AssetsIndex.build` 支持多文本合并 | assets_index.py:237-243 | Doc 模型(89-97)加 file_id |
| 2.2 | 新增 `build_from_files(paths: list)` | assets_index.py:245-289 | 多路径入口 |
| 2.3 | `parse_markdown`/`_parse_sections`/`section()` 复合行号 `(file_id, line)` | assets_index.py:119-153/156-209 | **核心改造**：行号跨文件去歧义 |
| 2.4 | BM25 search 支持 file_id | assets_index.py:329-367 | 搜索结果带文件来源 |
| 2.5 | 缓存多文件 mtime + 失效判断 | assets_index.py:293-326 | 单 mtime → 多 mtime |
| 2.6 | `_resolve_asset_file` 返回 list | ae-sdd CLI:1440-1470 | 单 Path → list[Path] |
| 2.7 | `_build_index` 多文件构建 | ae-sdd CLI:1473-1479 | 遍历合并 |
| 2.8 | 5 个 assets 子解析器加 `--doc-workspace` | ae-sdd CLI:2130/2137/2142/2148/2155 | 新维度参数 |

**影响测试**：`test_assets_index.py` 几十个断言需重写（行号语义变化）。

### 6.3 阶段 3：门卫升级 + 子文件就近规则 + 迁移

| # | 改动 | 文件 | 风险 |
|---|------|------|:----:|
| 3.1 | G-00 升级：校验总览 + 发现工程级子文件 | gates.py | 🟡 中 |
| 3.2 | 子文件就近存放规则：`docWorkspacePath/assets/{key}/{module}/{module}.assets.md` | schema §15.2 + document-storage + update-skill | 🟢 低 |
| 3.3 | 资产生成 SOP 支持就近生成 | project-assets-update-skill §3/§4 | 🟢 低 |
| 3.4 | 迁移 icec-cloud-boss 子文件 + 修复 4 处链接 | assets/icec-cloud-boss/*（主体 L82/874/875/972）| 🟢 低 |
| 3.5 | 全量回归 + 端到端验证 | 全部 | — |

**迁移链接影响**（已调研）：
- 主体文件 4 处 href 指向子文件（L82/874/875/972），子文件移动后需改相对路径
- 子文件之间无互引，子文件内部无 schema/template 链接 → 迁移友好
- 主体文件不动（schema/template 链接不受影响）

### 6.4 测试同步

| 测试文件 | 改动点 |
|---------|--------|
| test_paths.py:221 | `assets_dir` 断言（若路径结构变）|
| test_gates.py:46/66 | G-00 夹具（若门卫逻辑变）|
| test_gate_intercept_v11.py:253 | assets 目录夹具 |
| test_assets_index.py | 几十个断言（行号语义变化，阶段 2）|

---

## 七、风险评估

| 风险 | 等级 | 应对 |
|------|:----:|------|
| 阶段 2 改搜索引擎核心导致索引全错 | 🔴 高 | 单独评审 + 充分单测；考虑保留单文件模式向后兼容 |
| 阶段 1 改 gates.py 瘫痪流程 | 🟡 中 | 每步跑 test_gates + 真实 G-00；单独 commit 可 revert |
| 迁移破坏现有资产 | 🟢 低 | git 备份；保留扁平兼容 |
| 跨阶段上下文丢失 | 🟢 低 | 每阶段独立 commit + CHANGELOG |

---

## 八、执行建议

**强烈建议分阶段，不要一次性：**
1. **阶段 1**（路径统一 + SSOT 归位）：高价值低风险，立即解决 Agent 404 故障
2. **阶段 3**（门卫 + 规则 + 迁移）：依赖阶段 1，中风险
3. **阶段 2**（搜索引擎多文件化）：独立技术债，单独评审，高风险

阶段 2 与路径治理是**两个独立问题**，不建议捆绑。一次性全做的风险高于分阶段收益。

---

## 九、待评审决策点

1. **口径方向**：以代码派 `assets/` 为准（改 27 处文档）还是文档派 `project-assets/`（改代码）？建议 `assets/`。
2. **阶段 2 是否捆绑**：assets_index 搜索引擎改造是否纳入本次，还是独立任务？建议独立。
3. **子文件存放**：`docWorkspacePath/assets/{key}/{module}/`（跟代码分离）确认？
4. **现有 commit `78077a4`**（icec-cloud-boss 元信息已改成 assets/）：与方案一致，无需回退。

---

## 十、调研过程遗留的教训

本设计文档经历了 4 次"前提有误"的返工，记录如下，供后续执行者参考：
1. 初判"资产文件写错 project-assets" → 实为文档代码打架
2. document-storage 的 `get_assets()` 假设有代码 → 实为纯文字契约
3. assets_index 假设易改多文件 → 实为搜索引擎核心改造
4. paths.py 假设能读 docWorkspacePath → 实为完全无此能力

**教训：动 ae-sdd 核心前，必须把每个假设用 file:line 实测验证，不能凭文档描述推断。**
