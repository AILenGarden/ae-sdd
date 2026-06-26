"""
assets_index.py — 项目资产 ES 化索引（倒排索引 + 分词 + BM25 评分）

把项目资产 *.assets.md（markdown 单一权威源）解析为结构化文档集，
建立 token → [(doc_id, tf)] 的倒排索引（ES postings list），查询时按
BM25（ES 默认相似度）评分排序后返回 top-N 命中。

设计要点：
- 纯标准库（re + collections + math + json），对齐 tools/lib 风格
- 分词器：camelCase 拆分 + snake_case 拆分 + 中英文 token + 小写归一
  （对标 ES analyzer）。连字符词组同时保留整词 + 拆分双索引（multi-field）
- BM25（k1=1.5, b=0.75）：多 token 查询 OR 合并分数
- 缓存：build_from_file 按 mtime 判断是否复用 .index.json，避免每次重解析

对外 API（把 project-assets-update-skill §G 的"自然语言协议"变成真函数）：
  AssetsIndex.build(md_text)            → 从 markdown 文本建索引
  idx.search(query, top_n)              → 给 key 返回 top-N Hit
  idx.section(name)                     → 取整章（替代 assets.sections）
  idx.modules()                         → §B 模块索引结构化
  idx.table_fields(table_name)          → §C 字段查询
  idx.stats()                           → 索引统计
  AssetsIndex.build_from_file(path,cache_path) → 带缓存的文件入口
"""
from __future__ import annotations

import json
import math
import os
import re
import tempfile
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Optional


# ─── BM25 参数（ES 默认相似度）─────────────────────────────────────────────
BM25_K1 = 1.5
BM25_B = 0.75


# ─── 分词器（对标 ES analyzer）─────────────────────────────────────────────
# camelCase / PascalCase 边界：CsTicketAppService → Cs Ticket App Service
_CAMEL_SPLIT = re.compile(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])")
# snake_case / kebab-case 分隔：boss_user_role / icec-cloud-life → boss user role
_DELIM_SPLIT = re.compile(r"[_\-.]")
# 连续中文字符段（CJK Unified Ideographs + 扩展）
_CJK_RUN = re.compile(r"[\u4e00-\u9fff\u3400-\u4dbf]+")
# 英文/数字 token
_WORD_RUN = re.compile(r"[a-z0-9]+")


def tokenize(text: str) -> list[str]:
    """把文本切分为 token 列表（小写归一）。

    规则（对标 ES analyzer 的 multi-field 思路）：
    1. camelCase / PascalCase 按大小写边界拆分
    2. snake_case / kebab-case 按 _-. 拆分
    3. 中文按连续段作为一个 token（不做字级切分，保留语义）
    4. 英文/数字按连续段作为一个 token
    5. 全部小写归一

    例：
      "CsTicketAppService"     → ["cs", "ticket", "app", "service"]
      "boss_user_role"         → ["boss", "user", "role"]
      "BossUser 脱敏"          → ["boss", "user", "脱敏"]
      "11101-11107"            → ["11101", "11107"]
    """
    if not text:
        return []
    tokens: list[str] = []
    # 先把 camelCase 拆开（在原串上插空格），再统一处理
    spaced = _CAMEL_SPLIT.sub(" ", text)
    # 按分隔符切段
    for chunk in _DELIM_SPLIT.split(spaced):
        if not chunk:
            continue
        chunk_lower = chunk.lower()
        # 抽中文段
        for cjk in _CJK_RUN.findall(chunk_lower):
            tokens.append(cjk)
        # 抽英文/数字段
        for w in _WORD_RUN.findall(chunk_lower):
            tokens.append(w)
    return tokens


# ─── 文档模型 ─────────────────────────────────────────────────────────────
@dataclass
class Doc:
    """单个被索引的文档单元（markdown 的一行或一个表格行）。"""
    doc_id: int
    section: str          # 所属章节锚，如 "§4" / "§6.4" / "§B"
    line: int             # 在原 markdown 中的行号（1-based，单文件局部）
    text: str             # 原始文本（已 strip）
    tokens: list[str] = field(default_factory=list)
    tf: Counter = field(default_factory=Counter)   # token → 该文档内频次
    file_id: int = 0      # 🆕 v4.0：来源文件 id（0=单文件模式/总览；多文件合并时递增）


@dataclass
class Hit:
    """一次查询命中（对标 ES query result hit）。"""
    section: str
    line: int
    snippet: str
    score: float
    matched_tokens: list[str]
    file_id: int = 0      # 🆕 v4.0：命中所在文件 id（多文件溯源）


# ─── markdown 解析 ───────────────────────────────────────────────────────
# 章节锚：## §4 / ## §6.4 / ## §B / ### 4.5.1 / ## 2. 微服务清单
# anchor 不含尾随点（"2." → "2"，"4.5" 保留中间点）
_SECTION_RE = re.compile(r"^#{1,6}\s+(?:§\s*)?([0-9A-Za-z](?:[0-9A-Za-z\-]*[0-9A-Za-z])?(?:\.[0-9A-Za-z]+)*)")
# 表格行：含至少 2 个 | 且非分隔行（|---|）
_TABLE_ROW_RE = re.compile(r"^\|(.+)\|\s*$")
_TABLE_SEP_RE = re.compile(r"^\|[\s:|-]+\|\s*$")


def _parse_sections(lines: list[str]) -> list[tuple[int, str]]:
    """返回 [(line_no, section_anchor)]，每个章节起始行。

    anchor 形如 "§4" / "§6.4" / "§B" / "2"。维护一个"当前章节"栈：
    遇到 # 一级 → 顶层；## → 主章节；### → 子章节（拼到父章节后）。
    """
    sections: list[tuple[int, str]] = []
    cur_main = ""
    for i, line in enumerate(lines, start=1):
        m = _SECTION_RE.match(line)
        if not m:
            continue
        anchor = m.group(1)
        # hash 数量决定层级
        hashes = len(re.match(r"^#{1,6}", line).group(0))
        if hashes <= 2:
            # 主章节：统一加 § 前缀
            cur_main = f"§{anchor}"
            sections.append((i, cur_main))
        else:
            # 子章节：anchor 自带完整编号（如 4.5 / 4.5.1），直接加 § 前缀
            # 不与 cur_main 拼接，避免 §4 + 4.5 → §4.4.5 的重复
            sections.append((i, f"§{anchor}"))
    return sections


def _section_of(line_no: int, sections: list[tuple[int, str]]) -> str:
    """根据行号查所属章节。"""
    cur = ""
    for start, anchor in sections:
        if start <= line_no:
            cur = anchor
        else:
            break
    return cur or "§0"


def parse_markdown(md_text: str) -> list[Doc]:
    """把 markdown 解析为 Doc 列表。

    索引粒度：每个非空行 = 1 个 doc（表格行单独成 doc，便于精确命中）。
    跳过：空行、纯分隔线、表格分隔行（|---|）、代码围栏。
    """
    lines = md_text.splitlines()
    sections = _parse_sections(lines)
    docs: list[Doc] = []
    in_code_fence = False

    for i, raw in enumerate(lines, start=1):
        stripped = raw.strip()

        # 代码围栏开关
        if stripped.startswith("```"):
            in_code_fence = not in_code_fence
            continue
        if in_code_fence:
            # 代码块内也索引（JSON 实例等有价值的结构化数据），但归到当前章节
            if stripped:
                toks = tokenize(stripped)
                if toks:
                    docs.append(Doc(
                        doc_id=len(docs), section=_section_of(i, sections),
                        line=i, text=stripped, tokens=toks,
                        tf=Counter(toks),
                    ))
            continue

        if not stripped:
            continue
        # 表格分隔行跳过
        if _TABLE_SEP_RE.match(stripped):
            continue
        # 纯分隔线
        if stripped in ("---", "***", "___"):
            continue

        toks = tokenize(stripped)
        if not toks:
            # 纯标点/符号行也保留为 doc（便于 section() 返回完整内容），但不入倒排
            docs.append(Doc(
                doc_id=len(docs), section=_section_of(i, sections),
                line=i, text=stripped, tokens=[], tf=Counter(),
            ))
        else:
            docs.append(Doc(
                doc_id=len(docs), section=_section_of(i, sections),
                line=i, text=stripped, tokens=toks,
                tf=Counter(toks),
            ))

    return docs


# ─── 倒排索引 + BM25 ─────────────────────────────────────────────────────
class AssetsIndex:
    """项目资产 ES 化索引。

    构建：parse_markdown → 建 postings（token → [(doc_id, tf)]）+ 全局统计
    查询：tokenize(query) → OR 查 postings → BM25 评分 → top-N
    """

    def __init__(self, docs: list[Doc], sections: list[tuple[int, str]],
                 raw_lines, file_paths: Optional[list] = None):
        """构建索引。

        raw_lines：单文件模式 = list[str]；多文件模式 = {file_id: list[str]}。
        file_paths：🆕 v4.0 多文件模式的 [Path, ...]（file_id 为索引），单文件为 None。
        """
        self.docs = docs
        self.sections = sections
        self.raw_lines = raw_lines
        self.file_paths = file_paths  # 🆕 v4.0：[Path] 或 None（单文件）
        # 倒排索引：token → {doc_id: tf}
        self._postings: dict[str, dict[int, int]] = defaultdict(dict)
        # 文档长度（token 数）用于 BM25 的长度归一
        self._doc_len: list[int] = [len(d.tokens) for d in docs]
        self._avg_doc_len = (sum(self._doc_len) / len(self._doc_len)) if self._doc_len else 0.0
        self._n_docs = len(docs)
        # 建倒排索引
        for d in docs:
            for tok, tf in d.tf.items():
                self._postings[tok][d.doc_id] = tf

    # ── 构建 ────────────────────────────────────────────────────────────
    @classmethod
    def build(cls, md_text: str) -> "AssetsIndex":
        """从 markdown 文本建索引（纯函数式入口，便于测试）。"""
        lines = md_text.splitlines()
        docs = parse_markdown(md_text)
        sections = _parse_sections(lines)
        return cls(docs, sections, lines)

    @classmethod
    def build_from_file(cls, asset_path: Path,
                        cache_path: Optional[Path] = None) -> "AssetsIndex":
        """从资产文件建索引，带 mtime 缓存。

        cache_path 给定时：若缓存存在且 mtime >= 资产文件 mtime，直接反序列化；
        否则重建并原子写缓存（临时文件 + rename）。
        """
        md_text = asset_path.read_text(encoding="utf-8")
        mtime = asset_path.stat().st_mtime

        # 尝试命中缓存
        if cache_path is not None and cache_path.is_file():
            try:
                cache = json.loads(cache_path.read_text(encoding="utf-8"))
                if cache.get("source_mtime", 0) >= mtime and cache.get("version") == cls._CACHE_VERSION:
                    return cls._from_cache(cache, md_text)
            except (json.JSONDecodeError, KeyError, ValueError):
                pass  # 缓存损坏 → 重建

        idx = cls.build(md_text)

        # 原子写缓存
        if cache_path is not None:
            cache = idx._to_cache(mtime)
            cache_path.parent.mkdir(parents=True, exist_ok=True)
            try:
                # Windows 无原子 rename 覆盖，先写临时再替换
                fd, tmp = tempfile.mkstemp(
                    suffix=".tmp", dir=str(cache_path.parent), prefix=".idx_")
                try:
                    with os.fdopen(fd, "w", encoding="utf-8") as f:
                        json.dump(cache, f, ensure_ascii=False)
                    # os.replace 跨平台原子覆盖
                    os.replace(tmp, str(cache_path))
                except Exception:
                    try:
                        os.unlink(tmp)
                    except OSError:
                        pass
                    raise
            except OSError:
                pass  # 缓存写失败不阻断查询

        return idx

    @classmethod
    def build_from_files(cls, asset_paths: list,
                         cache_path: Optional[Path] = None) -> "AssetsIndex":
        """🆕 v4.0：从多个资产文件建合并索引（总览 + 工程级子文件）。

        每个文件分配 file_id（0=总览，1+=子文件），doc_id 全局连续编号。
        行号保持单文件局部（多文件下用 (file_id, line) 溯源）。
        缓存：任一文件 mtime 变动即重建（多 mtime 比对）。

        单文件时退化为 build_from_file 行为（file_id 全 0）。
        空列表 → 空索引。
        """
        if not asset_paths:
            # 空索引：无 docs
            return cls([], [], {}, file_paths=[])

        # 收集每个文件的 mtime（缓存判断用）
        file_mtimes = []
        all_texts = []
        for ap in asset_paths:
            if not Path(ap).is_file():
                continue
            all_texts.append(Path(ap).read_text(encoding="utf-8"))
            file_mtimes.append(Path(ap).stat().st_mtime)

        if not all_texts:
            return cls([], [], {}, file_paths=[])

        # 尝试命中缓存（多 mtime + 版本）
        if cache_path is not None and cache_path.is_file():
            try:
                cache = json.loads(cache_path.read_text(encoding="utf-8"))
                cached_mtimes = cache.get("file_mtimes", [])
                if (cache.get("version") == cls._CACHE_VERSION
                        and len(cached_mtimes) == len(file_mtimes)
                        and all(c >= a for c, a in zip(cached_mtimes, file_mtimes))):
                    return cls._from_cache_multi(cache, all_texts)
            except (json.JSONDecodeError, KeyError, ValueError, IndexError):
                pass  # 缓存损坏 → 重建

        # 合并多文件：每个文件独立 parse_markdown，file_id 递增
        merged_docs: list[Doc] = []
        merged_sections: list[tuple[int, str]] = []
        raw_lines_by_fid: dict = {}
        doc_id_counter = 0
        # sections 需要带 file_id 区分（行号会重叠），用复合 key
        sections_by_fid: dict = {}

        for fid, md_text in enumerate(all_texts):
            lines = md_text.splitlines()
            raw_lines_by_fid[fid] = lines
            file_docs = parse_markdown(md_text)
            file_sections = _parse_sections(lines)
            sections_by_fid[fid] = file_sections
            # 重新编号 doc_id 全局连续，打 file_id
            for d in file_docs:
                d.doc_id = doc_id_counter
                d.file_id = fid
                merged_docs.append(d)
                doc_id_counter += 1

        idx = cls(merged_docs, sections_by_fid, raw_lines_by_fid,
                  file_paths=[Path(ap) for ap in asset_paths])

        # 原子写缓存
        if cache_path is not None:
            cache = idx._to_cache_multi(file_mtimes)
            cache_path.parent.mkdir(parents=True, exist_ok=True)
            try:
                fd, tmp = tempfile.mkstemp(
                    suffix=".tmp", dir=str(cache_path.parent), prefix=".idx_")
                try:
                    with os.fdopen(fd, "w", encoding="utf-8") as f:
                        json.dump(cache, f, ensure_ascii=False)
                    os.replace(tmp, str(cache_path))
                except Exception:
                    try:
                        os.unlink(tmp)
                    except OSError:
                        pass
                    raise
            except OSError:
                pass  # 缓存写失败不阻断

        return idx

    _CACHE_VERSION = "2"  # 🆕 v4.0：1→2（多文件缓存结构变更）

    def _to_cache(self, source_mtime: float) -> dict:
        return {
            "version": self._CACHE_VERSION,
            "source_mtime": source_mtime,
            "n_docs": self._n_docs,
            "avg_doc_len": self._avg_doc_len,
            "doc_len": self._doc_len,
            "postings": {tok: dict(postings) for tok, postings in self._postings.items()},
            "docs": [
                {"doc_id": d.doc_id, "section": d.section, "line": d.line,
                 "text": d.text, "file_id": d.file_id}
                for d in self.docs
            ],
        }

    @classmethod
    def _from_cache(cls, cache: dict, md_text: str) -> "AssetsIndex":
        docs: list[Doc] = []
        for d in cache["docs"]:
            toks = tokenize(d["text"])
            docs.append(Doc(
                doc_id=d["doc_id"], section=d["section"], line=d["line"],
                text=d["text"], tokens=toks, tf=Counter(toks),
                file_id=d.get("file_id", 0),
            ))
        obj = cls.__new__(cls)
        obj.docs = docs
        obj.sections = _parse_sections(md_text.splitlines())
        obj.raw_lines = md_text.splitlines()
        obj.file_paths = None
        obj._postings = defaultdict(dict)
        for tok, postings in cache["postings"].items():
            obj._postings[tok] = {int(k): v for k, v in postings.items()}
        obj._doc_len = cache["doc_len"]
        obj._avg_doc_len = cache["avg_doc_len"]
        obj._n_docs = cache["n_docs"]
        return obj

    # ── 多文件缓存（🆕 v4.0）─────────────────────────────────────────────
    def _to_cache_multi(self, file_mtimes: list) -> dict:
        """多文件缓存序列化。"""
        return {
            "version": self._CACHE_VERSION,
            "multi_file": True,
            "file_mtimes": file_mtimes,
            "n_docs": self._n_docs,
            "avg_doc_len": self._avg_doc_len,
            "doc_len": self._doc_len,
            "postings": {tok: dict(postings) for tok, postings in self._postings.items()},
            "docs": [
                {"doc_id": d.doc_id, "section": d.section, "line": d.line,
                 "text": d.text, "file_id": d.file_id}
                for d in self.docs
            ],
        }

    @classmethod
    def _from_cache_multi(cls, cache: dict, all_texts: list) -> "AssetsIndex":
        """多文件缓存反序列化。all_texts = 各文件的文本列表（用于重建 sections/raw_lines）。"""
        docs: list[Doc] = []
        for d in cache["docs"]:
            toks = tokenize(d["text"])
            docs.append(Doc(
                doc_id=d["doc_id"], section=d["section"], line=d["line"],
                text=d["text"], tokens=toks, tf=Counter(toks),
                file_id=d.get("file_id", 0),
            ))
        # 重建多文件 sections / raw_lines
        sections_by_fid: dict = {}
        raw_lines_by_fid: dict = {}
        for fid, md_text in enumerate(all_texts):
            lines = md_text.splitlines()
            sections_by_fid[fid] = _parse_sections(lines)
            raw_lines_by_fid[fid] = lines
        obj = cls.__new__(cls)
        obj.docs = docs
        obj.sections = sections_by_fid
        obj.raw_lines = raw_lines_by_fid
        obj.file_paths = None  # 缓存恢复时不持久化 Path（重 build 时才有）
        obj._postings = defaultdict(dict)
        for tok, postings in cache["postings"].items():
            obj._postings[tok] = {int(k): v for k, v in postings.items()}
        obj._doc_len = cache["doc_len"]
        obj._avg_doc_len = cache["avg_doc_len"]
        obj._n_docs = cache["n_docs"]
        return obj

    # ── 查询（BM25）────────────────────────────────────────────────────
    def search(self, query: str, top_n: int = 20) -> list[Hit]:
        """给 key，返回 top-N 命中（按 BM25 降序）。

        多 token 查询走 OR：任一 token 命中即纳入候选，分数累加（ES default OR）。
        """
        q_tokens = tokenize(query)
        if not q_tokens:
            return []

        # 收集候选 doc_id → 累加 BM25 分数
        scores: dict[int, float] = defaultdict(float)
        matched: dict[int, set[str]] = defaultdict(set)

        for qtok in q_tokens:
            postings = self._postings.get(qtok)
            if not postings:
                continue
            df = len(postings)
            # IDF（BM25 变体，+1 防 log(0)）
            idf = math.log(1 + (self._n_docs - df + 0.5) / (df + 0.5))
            for doc_id, tf in postings.items():
                dl = self._doc_len[doc_id] or 1
                # BM25 term frequency 饱和
                tf_norm = (tf * (BM25_K1 + 1)) / (tf + BM25_K1 * (1 - BM25_B + BM25_B * dl / self._avg_doc_len))
                scores[doc_id] += idf * tf_norm
                matched[doc_id].add(qtok)

        if not scores:
            return []

        ranked = sorted(scores.items(), key=lambda kv: kv[1], reverse=True)
        hits: list[Hit] = []
        for doc_id, score in ranked[:top_n]:
            d = self.docs[doc_id]
            hits.append(Hit(
                section=d.section, line=d.line, snippet=d.text,
                score=round(score, 4), matched_tokens=sorted(matched[doc_id]),
                file_id=d.file_id,
            ))
        return hits

    # ── 章节提取 ────────────────────────────────────────────────────────
    def _iter_all_sections(self):
        """统一遍历所有 sections（单文件 list / 多文件 dict 均兼容）。

        yield (file_id, line_no, anchor)。单文件模式 file_id 恒 0。
        """
        if isinstance(self.sections, dict):
            # 多文件模式
            for fid, sec_list in self.sections.items():
                for line_no, anchor in sec_list:
                    yield (fid, line_no, anchor)
        else:
            # 单文件模式（list[tuple[int, str]]）
            for line_no, anchor in self.sections:
                yield (0, line_no, anchor)

    def section(self, name: str) -> Optional[str]:
        """取整章原文（替代 assets.sections 协议）。

        name 可为 "§4" / "4" / "§6.4" / "6.4" / "§B" / "B"。
        返回从该章节标题行到下一章节标题行之间的全部原文。
        多文件模式下：在总览（file_id=0）里查找，找不到再遍历子文件。
        """
        target = name if name.startswith("§") else f"§{name}"
        all_secs = list(self._iter_all_sections())

        # 找目标章节
        start_idx = None
        for i, (fid, line_no, anchor) in enumerate(all_secs):
            if anchor == target:
                start_idx = i
                break
        if start_idx is None:
            # 宽松匹配
            for i, (fid, line_no, anchor) in enumerate(all_secs):
                if anchor.startswith(target):
                    start_idx = i
                    break
        if start_idx is None:
            return None

        start_fid, start_line, _ = all_secs[start_idx]
        # 同 file_id 内找下一章节作为结束
        end_line = None
        for j in range(start_idx + 1, len(all_secs)):
            fid, line_no, anchor = all_secs[j]
            if fid == start_fid:
                end_line = line_no
                break
        if end_line is None:
            end_line = len(self._raw_lines_of(start_fid)) + 1

        return "\n".join(self._raw_lines_of(start_fid)[start_line - 1: end_line - 1])

    def _raw_lines_of(self, file_id: int) -> list:
        """取某 file_id 的 raw_lines（单文件/多文件兼容）。"""
        if isinstance(self.raw_lines, dict):
            return self.raw_lines.get(file_id, [])
        # 单文件模式：raw_lines 是 list[str]，只有 file_id=0
        return self.raw_lines if file_id == 0 else []

    # ── 结构化查询：§B 模块索引 ─────────────────────────────────────────
    def modules(self) -> list[dict]:
        """解析 §B 模块索引表，返回结构化行。

        每行 = {columns...}（按表头命名）。表头行用 | 解析。
        """
        return self._parse_table("§B")

    # ── 结构化查询：§C 字段索引 ─────────────────────────────────────────
    def table_fields(self, table_name: str) -> list[dict]:
        """查 §C 中指定表名的字段行。

        匹配规则：行的第一列（表名列）等于或包含 table_name。
        """
        rows = self._parse_table("§C")
        out: list[dict] = []
        for r in rows:
            # 取第一个单元格作为表名
            first_val = next(iter(r.values()), "") if r else ""
            if table_name.lower() in str(first_val).lower():
                out.append(r)
        return out

    def _parse_table(self, section: str) -> list[dict]:
        """解析某章节下的第一个/所有 markdown 表格为 dict 列表。"""
        sec_text = self.section(section)
        if not sec_text:
            return []
        rows: list[dict] = []
        header: Optional[list[str]] = None
        for line in sec_text.splitlines():
            m = _TABLE_ROW_RE.match(line.strip())
            if not m:
                continue
            if _TABLE_SEP_RE.match(line.strip()):
                continue
            cells = [c.strip() for c in m.group(1).split("|")]
            if header is None:
                header = cells
                continue
            if len(cells) == len(header):
                rows.append(dict(zip(header, cells)))
        return rows

    # ── 统计 ────────────────────────────────────────────────────────────
    def stats(self) -> dict:
        """索引统计（供 `ae-sdd assets stats` 与缓存校验用）。"""
        return {
            "n_docs": self._n_docs,
            "n_tokens": len(self._postings),
            "avg_doc_len": round(self._avg_doc_len, 2),
            "n_sections": len(self.sections),
            "sections": [a for _, a in self.sections],
        }


def hits_to_jsonable(hits: list[Hit]) -> list[dict]:
    """Hit 列表转 JSON 可序列化结构（供 output.emit 用）。"""
    return [asdict(h) for h in hits]


# ─── 阶段→KEY 映射表（资产 SKILL 内部"意图→KEY"调度）──────────────────────
# 对标 project-assets-update-skill §G.3 场景化 API：每个阶段预跑一组固定 KEY
# 拿"基线命中"，LLM 还可追加自定义 KEY 做精准查（两层结合）。
#
# KEY 选取原则：选该阶段高频需要定位的类名/字段/约定，命中后返回简约 v
# （section/line/snippet/score），LLM 拿到定位后按需精读对应章节。
STAGE_KEY_MAP: dict[str, list[str]] = {
    "requirement-analysis": [
        "AppService", "Repository", "DomainService", "Service",
    ],
    "dr-generate": [
        "AppService", "Repository", "Converter", "Facade",
        "FeignClient", "ServiceProviderConstants",
    ],
    "story-generate": [
        "AppService", "Controller", "ServiceImpl", "Repository",
        "Converter", "FeignClient",
    ],
    "story-review": [
        "AppService", "Repository", "@Transactional", "Facade",
        "deleted_flag", "cellphone",
    ],
    "task-generate": [
        "AppService", "Repository", "Mapper", "Converter",
        "Command", "Query",
    ],
    "coding": [
        "AppService", "Repository", "Converter", "PO", "DO",
        "Mapper", "@Transactional", "Facade",
    ],
    "code-review": [
        "@Transactional", "Facade", "FeignClient", "AccessUserInfoContext",
        "LocalDateTime", "cellphone", "deleted_flag", "错误码",
    ],
    "testcase": [
        "TestRestTemplate", "@SpringBootTest", "Mockito", "@Rollback",
        "ApiResult", "PagedModels",
    ],
}

# 阶段→必读整章映射（某些阶段需要整章原文而非命中行，用 section 名）
STAGE_SECTION_MAP: dict[str, list[str]] = {
    "requirement-analysis": ["§A", "§B"],
    "dr-generate": ["§3", "§5", "§7"],
    "story-generate": ["§3", "§4", "§5"],
    "story-review": ["§4", "§6"],
    "task-generate": ["§3", "§4", "§5"],
    "coding": ["§4", "§5", "§6"],
    "code-review": ["§6"],
    "testcase": [],
}


@dataclass
class ReadResult:
    """资产 SKILL 返回给 LLM 的简约结果（v = 定位信息，非整章原文）。

    设计对标用户要求："kv 的 v 可以是文件内的行号等简约信息"。
    LLM 拿到 hits（行号定位）后，按需再精读对应章节。
    """
    stage: str                          # 阶段名
    project_key: str                    # 项目 key
    index_ready: bool                   # 索引是否本次初始化（True=已就绪复用 / False=本次新建）
    baseline_hits: dict[str, list[dict]]    # 固定 KEY → 命中列表（简约 v）
    extra_hits: dict[str, list[dict]]       # 自定义 KEY → 命中列表（简约 v）
    sections: dict[str, str]                # 整章原文（仅 STAGE_SECTION_MAP 要求的章节）
    stats: dict                             # 索引统计


def read_assets(idx: AssetsIndex, stage: str,
                extra_keys: Optional[list[str]] = None,
                project_key: str = "unknown") -> ReadResult:
    """资产 SKILL 的核心调度函数（对标 §G 场景化 API 的真实实现）。

    流程（用户设计）：
      1. 索引已就绪（idx 由调用方用 build_from_file 带缓存构建，含初始化检查）
      2. 通过 STAGE_KEY_MAP 把阶段意图 → 一组固定 KEY
      3. 跑脚本（idx.search）取每个 KEY 的简约 v（section/line/snippet/score）
      4. 同时跑 extra_keys（LLM 追加的精准查）
      5. 取 STAGE_SECTION_MAP 要求的整章原文
      6. 打包返回 ReadResult

    Args:
        idx: 已构建的 AssetsIndex（调用方负责 build_from_file + 缓存）
        stage: 阶段名（requirement-analysis/dr-generate/story-generate/
               story-review/task-generate/coding/code-review/testcase）
        extra_keys: LLM 追加的自定义 KEY（精准查某个类名/字段/业务词）
        project_key: 项目 key（用于结果标注）
    Returns:
        ReadResult（简约 v，LLM 拿到定位后按需精读）
    """
    baseline_keys = STAGE_KEY_MAP.get(stage, [])
    section_names = STAGE_SECTION_MAP.get(stage, [])

    # 固定 KEY → 命中（基线）
    baseline_hits: dict[str, list[dict]] = {}
    for key in baseline_keys:
        hits = idx.search(key, top_n=5)
        if hits:
            baseline_hits[key] = hits_to_jsonable(hits)

    # 自定义 KEY → 命中（精准查）
    extra_hits: dict[str, list[dict]] = {}
    for key in (extra_keys or []):
        hits = idx.search(key, top_n=5)
        if hits:
            extra_hits[key] = hits_to_jsonable(hits)

    # 整章原文
    sections: dict[str, str] = {}
    for name in section_names:
        sec_text = idx.section(name)
        if sec_text is not None:
            sections[name] = sec_text

    return ReadResult(
        stage=stage,
        project_key=project_key,
        index_ready=True,   # idx 已构建即视为就绪（初始化检查由 build_from_file 的缓存逻辑承担）
        baseline_hits=baseline_hits,
        extra_hits=extra_hits,
        sections=sections,
        stats=idx.stats(),
    )


def read_result_to_jsonable(result: ReadResult) -> dict:
    """ReadResult 转 JSON 可序列化结构（供 CLI --json 输出）。"""
    return asdict(result)
