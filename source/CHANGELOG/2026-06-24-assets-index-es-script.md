# 2026-06-24 | ae-sdd — 项目资产索引 ES 化 + 脚本化（assets read 闭环）

> **性质**：项目资产能力工具层加固。把 project-assets-update-skill §G 的"自然语言协议"（由调用 SKILL 通过 Read/Grep 组合读取）推进到"可执行脚本：倒排索引 + 分词 + BM25 评分 + CLI 入口 + 资产 SKILL 内部调度闭环"。
>
> **影响范围**：新增 1 个 lib 模块 + 1 个 CLI 子命令组（含 read 核心入口）+ 1 组测试 + 2 份资产文件补全 + update_graph 标记修正 + SKILL §6/§G 真实调度逻辑改写。

## 背景

用户指出"项目资产索引做得差"，要求对标 Elasticsearch 思想（倒排索引 / 分词 / 相关性评分）并脚本化——"将 key 给脚本，脚本直接返回索引对应的结果"。

核查现状，7 层索引（§A-§G）是"概念到位、实现缺位"：

| ES 核心能力 | 现状 | 差距 |
|---|---|---|
| 倒排索引 | §F = 人工维护的 markdown 表格 | life 仅 9 词（门禁要 ≥20）；boss **完全没有** §A-§G 章节 |
| 分词 | 无 | `search("AppService")` 命中不了 `CsTicketAppService` |
| 相关性评分 | 无 | 多命中无排序 |
| 可执行查询 | §G 是"自然语言协议" | `ae-sdd assets` 子命令在 CLI 里**根本未实现**（`update_graph.py` 的 `HISTORICAL_UNIMPLEMENTED` 明确记录） |
| 机器可读 | markdown 表格 | 无法可靠解析 |

根因：缺一个把 markdown 资产解析 → 建倒排索引 → 分词 → BM25 评分 → 查询返回的可执行层。

## 改动 1：新增 `tools/lib/assets_index.py`（核心库）

把 ES 思想用纯 Python 标准库映射到资产文件：

```
*.assets.md (markdown 单一权威源)
   │  parse_markdown()
   ▼
结构化文档集 docs[] —— 每个 doc = {section, line, text, tokens}
   │  tokenize() + build inverted index
   ▼
倒排索引 postings: token → [(doc_id, tf)]   ← ES postings list
   │  search(key) → tokenize(key) → OR 查询 → BM25 评分
   ▼
top-N 命中: [{section, line, snippet, score, matched_tokens}]
```

**三层映射：**

| ES 概念 | 本实现 |
|---|---|
| analyzer（分词器） | `tokenize()`：camelCase 拆分（`CsTicketAppService`→`cs/ticket/app/service`）+ snake_case 拆分 + 小写归一 + 中英文 token |
| inverted index（倒排索引） | `token → {doc_id: tf}`，`collections.defaultdict` |
| scoring（BM25） | 标准 BM25（k1=1.5, b=0.75），多 token 查询 OR 合并分数 |

对外 API（把 §G 协议变成真函数）：

```python
class AssetsIndex:
    @classmethod
    def build(cls, md_text: str) -> "AssetsIndex"           # 从 markdown 建索引
    def search(self, query: str, top_n=20) -> list[Hit]     # 给 key 返回 top-N（核心）
    def section(self, name: str) -> str | None              # 取整章
    def modules(self) -> list[dict]                         # §B 结构化
    def table_fields(self, table_name: str) -> list[dict]   # §C 字段查询
    def stats(self) -> dict                                 # 索引统计
    @classmethod
    def build_from_file(cls, asset_path, cache_path=None)   # 带 mtime 缓存的文件入口
```

缓存：索引构建后序列化为 `.ae-sdd/assets/{key}.index.json`，按资产文件 mtime 判断是否重建（临时文件 + `os.replace` 原子替换）。缓存是衍生产物，不入 git。

## 改动 2：`tools/bin/ae-sdd` 注册 `assets` 子命令组

照 `state`/`gates` 二级 subparser 模式，新增 4 个叶子命令：

| 命令 | 作用 |
|---|---|
| `ae-sdd assets query "<key>" [--top N] [--project <key>] [--asset-file <path>]` | 给 key，返回倒排索引命中（BM25 排序）—— 用户要的核心能力 |
| `ae-sdd assets outline [--project <key>]` | §A 大纲 + 索引统计 |
| `ae-sdd assets section <name> [--project <key>]` | 取整章原文 |
| `ae-sdd assets stats [--project <key>]` | 索引统计 + 缓存状态 |

- 复用 `paths.find_asset_file()` + `paths.locate_project_ae_sdd()` + `paths.read_config()` 定位资产
- 输出走 `output.emit()`（支持 `--json`，pipeline 友好）
- `--asset-file` 支持直接指定资产文件（绕过 `.ae-sdd` 定位，便于在 ae-sdd 自身仓库测试）

## 改动 3：`tools/tests/test_assets_index.py`（38 个单测）

照 `test_gates.py` 结构：`sys.path.insert` + `from lib import assets_index` + 模块级 `_build_md()` helper + `unittest.TestCase`。覆盖：

- 分词：camelCase / snake_case / kebab-case / 中英文 / 数字范围 / 空串
- markdown 解析：章节归属 / 代码围栏 / 表格分隔行跳过
- 倒排索引构建：postings 存在性 / token 数 / doc 数
- BM25 查询：单 token 命中 / camelCase 查询命中 PascalCase / 结果排序 / 多 token OR / 中文查询 / 无匹配 / 空查询 / top-N 限制 / Hit 字段
- 章节提取：按名取章 / 无前缀 / 索引章 / 子章节 / 不存在
- 结构化查询：§B 模块 / §C 字段过滤 / 无匹配 / 无表格
- 缓存：命中跳过重建 / mtime 变化失效 / 无 cache_path / 损坏缓存回退
- 输出序列化

## 改动 4：补全两份资产文件薄弱项

### `icec-cloud-boss.assets.md`（此前完全缺 §A-§G）

从现有 §2/§4/§5/§7 机械抽取，补齐完整索引层：

- §A 大纲：7 字段速览 + 19 章目录
- §B 模块索引：9 行（boss-user / boss-user-bff / boss-auth-bff / boss-security / boss-abnormal / life-cs / life-im / life-spi / boss-api）
- §C 字段索引：boss_user 主表 11 行 + 错误码占用值 7 行（11101-11107 真实数据）
- §D 组件索引：12 行（TokenService / @SkipAuth / @RequiresPermissions / ApiResult / PagedModels / PageRequest / JsonUtils / DesensitizeUtils / BCryptUtil / MybatisPlusConfig / KafkaDomainEventPublisher）
- §E API 索引：11 行（11 个 SPI 服务）
- §F 反向索引：25 词（位置精确到 §X.Y）
- §G 读取 API：标注脚本化 + 调用示例
- §10 缺口新增 #11（✅ 已补）

### `icec-cloud-life.assets.md`（§F 仅 9 词，§G 伪 API）

- §F 反向索引：9 词 → 28 词，位置从粗粒度"§4/§6.9"精确到"§X.Y + 行号"
- §G 读取 API：从伪代码协议升级为 `ae-sdd assets` 脚本化说明
- §10 缺口新增 #11（✅ 已补）

两份资产 `lastAuditedAt` 统一更新为 2026-06-24，更新日志各加一条。

## 改动 5：`update_graph.py` 移除 `assets` 未实现标记

`HISTORICAL_UNIMPLEMENTED` 集合移除 `"assets"`（`assets` 组已实现 query/outline/section/stats）。`assets check/generate/update/audit/read` 仍走 SKILL 协议，后续迭代补。

## 改动 6：`project-assets-update-skill.md` §G 脚本化标注

§G 开头新增"2026-06-24 脚本化落地"声明，列出协议 API ↔ CLI 命令的对应表。协议描述保留作为语义契约，实际执行优先走 CLI 脚本。

## 验证

```bash
# 1. 单测全过
python tools/tests/run.py assets_index   # 38 tests OK

# 2. 全量回归不破坏现有门禁
python tools/tests/run.py                # 207 tests OK (1 skipped)

# 3. 真实查询
ae-sdd assets query "AppService" --asset-file source/assets/icec-cloud-life/icec-cloud-life.assets.md --top 5
# → 精确命中 CsTicketAppService / ImSessionAppService，BM25 排序合理

# 4. 中文查询 + JSON 输出
ae-sdd assets query "融云" --json | python -c "import sys,json; print(json.load(sys.stdin)['n_hits'])"

# 5. update-check 不误报
ae-sdd update-check --only UC-03   # ✅ UC-03 通过，assets 不在未实现列表
```

## 改动 7：资产 SKILL 内部调度闭环（🆕 第二层 — 用户设计落地）

> 第一层（改动 1-2）做了"脚本本身"。第二层把脚本**封装进资产 SKILL 内部**，
> 让 LLM 不直接碰脚本，只调资产 SKILL，由 SKILL 内部做"意图→KEY 映射 + 初始化检查 + 脚本调度"。

### 架构（用户设计）

```
LLM（任何下游 SKILL）
  │  "我要读资产" → 调用 project-assets-update-skill
  ▼
project-assets-update-skill（统一入口）
  │  1. 检查索引是否就绪；没就绪则初始化（build 倒排索引，带 mtime 缓存）
  │  2. STAGE_KEY_MAP 把阶段意图 → 一组固定 KEY（基线）
  │  3. 跑 read_assets() 取每个 KEY 的简约 v（section/line/snippet/score）
  │  4. 同时跑 LLM 追加的 --keys（精准查）
  │  5. 取 STAGE_SECTION_MAP 要求的整章原文
  │  6. 返回 ReadResult 给 LLM
  ▼
LLM 拿到简约定位（行号），按需精读
```

### `tools/lib/assets_index.py` 新增调度层

- `STAGE_KEY_MAP`：8 个阶段 → 固定 KEY 集映射表（requirement-analysis/dr-generate/story-generate/story-review/task-generate/coding/code-review/testcase）
- `STAGE_SECTION_MAP`：8 个阶段 → 必读整章映射
- `read_assets(idx, stage, extra_keys)`：核心调度函数，返回 `ReadResult`
- `ReadResult`：简约 v（baseline_hits + extra_hits + sections + stats），v 含 section/line/snippet/score

### `ae-sdd assets read <stage>` CLI 核心入口

```bash
# LLM 经资产 SKILL 读资产的统一入口：说阶段 + 可选追加 KEY
ae-sdd assets read coding --keys "CsTicketAppService,融云" --project icec-cloud-life
# → baseline_hits（8 个固定 KEY 的 BM25 命中）+ extra_hits（2 个精准 KEY）+ sections（§4/§5/§6）
```

### `project-assets-update-skill.md` §6.2 改写

把旧的伪 API（`assets.forCoding()` / `assets.search()`）替换成真实的 `ae-sdd assets read <stage>` 调用流程（检查+初始化 → 映射表 → 脚本 → 简约 v → 按需精读）。下游 SKILL 的"读取项目资产"步骤现在指向 §6.2 的真实命令。

## 不做的事（范围控制）

- 不改资产文件结构/schema（继续 markdown，不引 yaml/json 权威源）
- 不引三方库（jieba/rapidfuzz 等，纯标准库）
- 不直接扫真实工程代码建索引（以 assets.md 为源）
- 不实现 `assets generate/update/audit` 全套子命令（本次只做 query 查询链路）
- 不动 G-00 门禁逻辑（它继续做存在性+鲜度检查，索引查询是独立能力）
