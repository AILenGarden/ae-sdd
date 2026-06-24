"""
test_assets_index.py — assets_index 模块单测

覆盖：分词、倒排索引构建、BM25 排序、章节提取、表格结构化查询、
缓存命中/失效、边界。标准库 only（unittest + tempfile），零外部依赖。
"""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import assets_index  # noqa: E402
from lib.assets_index import AssetsIndex, Hit, tokenize  # noqa: E402


# ─── 迷你资产 fixture（覆盖 §2/§4/§5/§B/§C/§F 多种结构）──────────────────
MINI_ASSETS = """\
# icec-cloud-test Project Assets

## 0. 摘要

| 维度 | 内容 |
|------|------|
| 何时需要查 | Code Plan / Coding |
| 谁负责写 | 架构组 |

## 1. 项目资产元信息

| 字段 | 值 |
|------|---|
| projectKey | `icec-cloud-test` |
| gitPath | `d:\\Item\\test` |
| lastAuditedAt | `2026-06-24` |

## 2. 微服务清单

| name | responsibility | port | contextPath | type |
|------|---------------|------|-------------|------|
| icec-cloud-life-cs | 客服域 Service（工单/会话/状态机） | 20092 | — | service |
| icec-cloud-life-im | IM 域 Service（会话/消息/融云） | 20093 | — | service |

## 4. DDD 内部分层落点

| 类角色 | 精确包路径 | 典型类名 |
|--------|-----------|---------|
| AppService | `.../application/appservice/` | `CsTicketAppService` / `ImSessionAppService` |
| Domain Object | `.../model/entity/` | `CsTicketDO` / `ImMessageDO` |
| Repository | `.../repository/` | `CsTicketRepository` |

### 4.5 分层职责

业务规则放 Domain，编排放 Application。

## 5. 命名约定

| 对象 | 命名模板 | 例子 |
|------|---------|------|
| AppService | `{Resource}AppService` | `CsTicketAppService` |
| PO | `{Resource}PO` | `BossUserPO` |

## §B 模块索引

| module | 概述 | 入口 Controller | 关键 AppService |
|--------|------|---------------|----------------|
| `icec-cloud-life-cs` | 客服域 | `CsTicketServiceImpl` | `CsTicketAppService` |
| `icec-cloud-life-im` | IM 域 | `ImSessionServiceImpl` | `ImSessionAppService` |

## §C 字段索引

| 表名 | 字段 | 类型 | 业务含义 |
|------|------|------|---------|
| `cs_ticket` | `id` | bigint | 主键 |
| `cs_ticket` | `status` | tinyint | 工单状态 |
| `im_message` | `session_id` | bigint | 会话 ID |

## §F 关键词反向索引

| 关键词 | 出现位置 |
|--------|---------|
| `AppService` | §4 / §B |
| `融云` | §2 |
"""


class TestTokenize(unittest.TestCase):
    """分词器测试（对标 ES analyzer）。"""

    def test_camel_case_split(self):
        self.assertEqual(tokenize("CsTicketAppService"),
                         ["cs", "ticket", "app", "service"])

    def test_pascal_case_split(self):
        self.assertEqual(tokenize("BossUserPO"), ["boss", "user", "po"])

    def test_snake_case_split(self):
        self.assertEqual(tokenize("boss_user_role"),
                         ["boss", "user", "role"])

    def test_kebab_case_split(self):
        self.assertEqual(tokenize("icec-cloud-life-cs"),
                         ["icec", "cloud", "life", "cs"])

    def test_mixed_cn_en(self):
        toks = tokenize("BossUser 脱敏")
        self.assertIn("boss", toks)
        self.assertIn("user", toks)
        self.assertIn("脱敏", toks)

    def test_number_range(self):
        self.assertEqual(tokenize("11101-11107"), ["11101", "11107"])

    def test_empty(self):
        self.assertEqual(tokenize(""), [])
        self.assertEqual(tokenize("---|***"), [])

    def test_lowercase_normalization(self):
        toks = tokenize("AppService vs APPSERVICE")
        # 都归一到小写
        self.assertTrue(all(t == t.lower() for t in toks))


class TestParseMarkdown(unittest.TestCase):
    """markdown 解析测试。"""

    def test_build_returns_docs(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        self.assertGreater(len(idx.docs), 0)

    def test_section_assignment(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        # §2 微服务清单的行应归到 §2
        sec2_docs = [d for d in idx.docs if d.section == "§2"]
        self.assertTrue(any("客服域" in d.text for d in sec2_docs))

    def test_code_fence_indexed(self):
        md = "## §A\n```json\n{\"projectKey\": \"test\"}\n```\n"
        idx = AssetsIndex.build(md)
        toks = [t for d in idx.docs for t in d.tokens]
        # projectKey 经 camelCase 拆分为 project / key
        self.assertIn("project", toks)
        self.assertIn("key", toks)

    def test_table_separator_skipped(self):
        md = "## §B\n| a | b |\n|---|---|\n| 1 | 2 |\n"
        idx = AssetsIndex.build(md)
        # 分隔行不入 docs
        self.assertFalse(any("---" in d.text and set(d.text) <= set("|-: ") for d in idx.docs))


class TestInvertedIndex(unittest.TestCase):
    """倒排索引构建测试。"""

    def test_postings_built(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        stats = idx.stats()
        self.assertGreater(stats["n_tokens"], 0)
        self.assertGreater(stats["n_docs"], 0)

    def test_appservice_posted(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        # "appservice" 经分词后应出现在 postings
        self.assertIn("appservice", idx._postings)


class TestSearchBM25(unittest.TestCase):
    """BM25 查询测试（对标 ES query scoring）。"""

    def test_single_token_hit(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        hits = idx.search("AppService")
        self.assertGreater(len(hits), 0)
        # 命中行应包含 AppService 或其分词形式
        self.assertTrue(any("AppService" in h.snippet for h in hits))

    def test_camel_case_query_matches_pascal(self):
        """核心场景：查 AppService 能命中 CsTicketAppService。"""
        idx = AssetsIndex.build(MINI_ASSETS)
        hits = idx.search("AppService")
        snippets = [h.snippet for h in hits]
        self.assertTrue(any("CsTicketAppService" in s for s in snippets))
        self.assertTrue(any("ImSessionAppService" in s for s in snippets))

    def test_results_sorted_by_score(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        hits = idx.search("AppService")
        scores = [h.score for h in hits]
        self.assertEqual(scores, sorted(scores, reverse=True))

    def test_multi_token_or(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        hits = idx.search("AppService Repository")
        # 两个 token 任一命中即纳入
        self.assertGreater(len(hits), 0)
        # 命中 token 字段非空
        for h in hits:
            self.assertGreater(len(h.matched_tokens), 0)

    def test_chinese_query(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        hits = idx.search("融云")
        self.assertGreater(len(hits), 0)
        self.assertTrue(any("融云" in h.snippet for h in hits))

    def test_no_match(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        hits = idx.search("不存在的词汇zzz")
        self.assertEqual(len(hits), 0)

    def test_empty_query(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        self.assertEqual(idx.search(""), [])
        self.assertEqual(idx.search("---|"), [])

    def test_top_n_limit(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        all_hits = idx.search("cs", top_n=100)
        limited = idx.search("cs", top_n=2)
        self.assertLessEqual(len(limited), 2)
        if len(all_hits) > 2:
            self.assertEqual(len(limited), 2)

    def test_hit_fields(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        hits = idx.search("AppService")
        for h in hits:
            self.assertIsInstance(h, Hit)
            self.assertTrue(h.section.startswith("§"))
            self.assertGreater(h.line, 0)
            self.assertIsInstance(h.snippet, str)
            self.assertIsInstance(h.score, float)
            self.assertIsInstance(h.matched_tokens, list)


class TestSectionExtract(unittest.TestCase):
    """章节提取测试（替代 assets.sections 协议）。"""

    def test_get_section_by_name(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        sec = idx.section("§2")
        self.assertIsNotNone(sec)
        self.assertIn("微服务清单", sec)
        self.assertIn("客服域", sec)

    def test_get_section_without_prefix(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        sec = idx.section("2")
        self.assertIsNotNone(sec)
        self.assertIn("微服务清单", sec)

    def test_get_index_section(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        sec = idx.section("§B")
        self.assertIsNotNone(sec)
        self.assertIn("模块索引", sec)

    def test_subsection(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        sec = idx.section("§4.5")
        self.assertIsNotNone(sec)
        self.assertIn("分层职责", sec)

    def test_nonexistent_section(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        self.assertIsNone(idx.section("§ZZZ"))


class TestStructuredQuery(unittest.TestCase):
    """§B/§C 结构化表格查询测试。"""

    def test_modules_returns_rows(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        mods = idx.modules()
        self.assertEqual(len(mods), 2)
        names = [m.get("module", "") for m in mods]
        self.assertIn("icec-cloud-life-cs", str(names))

    def test_table_fields_filter(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        fields = idx.table_fields("cs_ticket")
        self.assertEqual(len(fields), 2)  # id + status
        field_names = [f.get("字段", "") for f in fields]
        self.assertIn("id", str(field_names))

    def test_table_fields_no_match(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        self.assertEqual(idx.table_fields("nonexistent_table"), [])

    def test_modules_empty_when_no_section(self):
        md = "# only title\nno tables here\n"
        idx = AssetsIndex.build(md)
        self.assertEqual(idx.modules(), [])


class TestStats(unittest.TestCase):
    """统计接口测试。"""

    def test_stats_shape(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        s = idx.stats()
        self.assertIn("n_docs", s)
        self.assertIn("n_tokens", s)
        self.assertIn("n_sections", s)
        self.assertIn("sections", s)
        self.assertGreater(s["n_docs"], 0)


class TestCache(unittest.TestCase):
    """缓存命中/失效测试。"""

    def test_cache_hit_skips_rebuild(self):
        with tempfile.TemporaryDirectory() as tmp:
            asset = Path(tmp) / "test.assets.md"
            asset.write_text(MINI_ASSETS, encoding="utf-8")
            cache = Path(tmp) / "test.index.json"

            idx1 = AssetsIndex.build_from_file(asset, cache_path=cache)
            self.assertTrue(cache.is_file())
            n_docs_1 = idx1.stats()["n_docs"]

            # 二次构建：mtime 不变，应命中缓存
            idx2 = AssetsIndex.build_from_file(asset, cache_path=cache)
            self.assertEqual(idx2.stats()["n_docs"], n_docs_1)
            # 查询结果一致
            self.assertEqual(
                len(idx1.search("AppService")),
                len(idx2.search("AppService")),
            )

    def test_cache_invalidated_on_mtime_change(self):
        with tempfile.TemporaryDirectory() as tmp:
            asset = Path(tmp) / "test.assets.md"
            asset.write_text(MINI_ASSETS, encoding="utf-8")
            cache = Path(tmp) / "test.index.json"

            idx1 = AssetsIndex.build_from_file(asset, cache_path=cache)
            n_before = idx1.stats()["n_docs"]

            # 修改文件（追加内容 + 主动改 mtime）
            asset.write_text(MINI_ASSETS + "\n## §Z\n新增章节\n", encoding="utf-8")
            import os as _os
            _os.utime(asset, (asset.stat().st_atime, asset.stat().st_mtime + 10))

            idx2 = AssetsIndex.build_from_file(asset, cache_path=cache)
            self.assertGreater(idx2.stats()["n_docs"], n_before)

    def test_cache_without_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            asset = Path(tmp) / "test.assets.md"
            asset.write_text(MINI_ASSETS, encoding="utf-8")
            # cache_path=None：不写缓存，但能正常建索引
            idx = AssetsIndex.build_from_file(asset, cache_path=None)
            self.assertGreater(idx.stats()["n_docs"], 0)

    def test_corrupt_cache_falls_back(self):
        with tempfile.TemporaryDirectory() as tmp:
            asset = Path(tmp) / "test.assets.md"
            asset.write_text(MINI_ASSETS, encoding="utf-8")
            cache = Path(tmp) / "test.index.json"
            cache.write_text("{not valid json", encoding="utf-8")

            # 损坏缓存应回退重建，不抛异常
            idx = AssetsIndex.build_from_file(asset, cache_path=cache)
            self.assertGreater(idx.stats()["n_docs"], 0)


class TestHitsJsonable(unittest.TestCase):
    """输出序列化测试（供 CLI --json 用）。"""

    def test_to_jsonable(self):
        idx = AssetsIndex.build(MINI_ASSETS)
        hits = idx.search("AppService", top_n=3)
        data = assets_index.hits_to_jsonable(hits)
        self.assertEqual(len(data), len(hits))
        for d in data:
            self.assertIn("section", d)
            self.assertIn("line", d)
            self.assertIn("score", d)


class TestReadAssets(unittest.TestCase):
    """资产 SKILL 核心调度函数 read_assets 测试（对标 §G 场景化 API 真实实现）。

    验证用户设计：LLM 调资产 SKILL → 映射表转 KEY → 跑脚本 → 返回简约 v。
    """

    def setUp(self):
        self.idx = AssetsIndex.build(MINI_ASSETS)

    def test_stage_coding_returns_baseline_hits(self):
        """coding 阶段应返回固定 KEY 集的基线命中。"""
        result = assets_index.read_assets(self.idx, stage="coding")
        self.assertEqual(result.stage, "coding")
        self.assertTrue(result.index_ready)
        # coding 映射含 AppService，应命中
        self.assertIn("AppService", result.baseline_hits)

    def test_extra_keys_precise_query(self):
        """LLM 追加自定义 KEY 做精准查。"""
        result = assets_index.read_assets(
            self.idx, stage="coding", extra_keys=["CsTicketAppService", "融云"],
        )
        self.assertIn("CsTicketAppService", result.extra_hits)
        self.assertIn("融云", result.extra_hits)

    def test_sections_returned_by_stage(self):
        """coding 阶段映射要求返回 §4/§5/§6 整章。"""
        result = assets_index.read_assets(self.idx, stage="coding")
        self.assertIn("§4", result.sections)
        self.assertIn("§5", result.sections)
        self.assertIn("DDD", result.sections["§4"])

    def test_unknown_stage_returns_empty_baseline(self):
        """未知阶段不报错，基线命中为空。"""
        result = assets_index.read_assets(self.idx, stage="unknown-stage")
        self.assertEqual(result.baseline_hits, {})

    def test_no_extra_keys(self):
        """不传 extra_keys 时 extra_hits 为空。"""
        result = assets_index.read_assets(self.idx, stage="story-generate")
        self.assertEqual(result.extra_hits, {})

    def test_baseline_hits_are_minimal_v(self):
        """返回的 v 是简约定位（section/line/snippet/score），非整章原文。"""
        result = assets_index.read_assets(self.idx, stage="coding")
        for key, hits in result.baseline_hits.items():
            for h in hits:
                self.assertIn("section", h)
                self.assertIn("line", h)
                self.assertIn("snippet", h)
                self.assertIn("score", h)
                self.assertIn("matched_tokens", h)

    def test_read_result_jsonable(self):
        """ReadResult 可序列化为 JSON（供 CLI --json）。"""
        result = assets_index.read_assets(
            self.idx, stage="code-review", extra_keys=["cellphone"],
        )
        data = assets_index.read_result_to_jsonable(result)
        self.assertEqual(data["stage"], "code-review")
        self.assertIn("baseline_hits", data)
        self.assertIn("extra_hits", data)
        self.assertIn("stats", data)

    def test_stage_key_map_covers_all_stages(self):
        """映射表覆盖全部 8 个阶段。"""
        expected = {"requirement-analysis", "dr-generate", "story-generate",
                    "story-review", "task-generate", "coding",
                    "code-review", "testcase"}
        self.assertEqual(set(assets_index.STAGE_KEY_MAP.keys()), expected)

    def test_stage_section_map_has_entries(self):
        """每个阶段都有整章映射（testcase 可以为空列表）。"""
        for stage, sections in assets_index.STAGE_SECTION_MAP.items():
            self.assertIsInstance(sections, list)


if __name__ == "__main__":
    unittest.main(verbosity=2)
