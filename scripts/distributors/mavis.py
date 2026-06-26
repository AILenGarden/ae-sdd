"""Mavis 分发器：harness_mount 协议。

与其他三个 copytree 分发器不同：
  - needs_compile=True：mavis 需要专属编译产物 agent.md（调 build_harness.py）
  - install 协议是 `mavis harness mount` 而非 copytree
  - cleanup 要清 mavis 端 ae-sdd-N 副本 + 同步 sqlite（迁自 install.py:cleanup_mavis_duplicates）

逻辑对齐 .githooks/post-commit 第 6/7 步 + install.py 的 mavis 相关函数。
"""
from __future__ import annotations

import re
import shutil
import sqlite3
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Optional

from ._base import Distributor, DistributeContext, InstallResult, log_info, log_warn, log_error

SKILL_NAME = "ae-sdd"
MAVIS_KEEP_DEFAULT = 0   # 清理 mavis 端 -N 副本时保留数（0=全清；负数=不清理）
MAVIS_HOME = Path.home() / ".mavis"


class MavisDistributor(Distributor):
    name = "mavis"
    protocol = "harness_mount"
    needs_compile = True

    # ── detect ──────────────────────────────────────────────────────────────
    def detect(self) -> bool:
        """auto 模式：mavis CLI 或 ~/.mavis/bin/mavis.cmd 存在时包含。"""
        # build_harness.py 与本包同级（scripts/），由调用方保证 sys.path 含 scripts/
        from build_harness import find_mavis_cmd
        return find_mavis_cmd() is not None

    # ── compile（专属产物：agent.md） ───────────────────────────────────────
    def compile(self, repo_root: Path) -> Optional[Path]:
        """调 build_harness.py 生成 harness/.harness/agent.md，返回 .harness 目录。"""
        scripts_dir = repo_root / "scripts"
        build_harness = scripts_dir / "build_harness.py"
        if not build_harness.is_file():
            log_error(f"build_harness.py 不存在: {build_harness}")
            return None
        # --no-mount：mount 留给 install 阶段做（compile 只产出文件）
        result = subprocess.run(
            [sys.executable, str(build_harness), "--source", str(repo_root), "--no-mount"],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            log_error(f"build_harness.py 失败 (rc={result.returncode})")
            if result.stderr:
                print(result.stderr, file=sys.stderr)
            return None
        harness_dir = repo_root / "harness" / ".harness"
        if (harness_dir / "agent.md").is_file():
            return harness_dir
        return None

    # ── install（mavis harness mount） ──────────────────────────────────────
    def install(self, source: Path, ctx: DistributeContext) -> InstallResult:
        """source 是 compile 产出的 .harness 目录；执行 mavis harness mount。"""
        t0 = time.time()
        from build_harness import run_mavis, find_mavis_cmd

        if find_mavis_cmd() is None:
            return InstallResult(self.name, "skip",
                                 "mavis 未安装，跳过 mount（产物已写入）", time.time() - t0)

        harness_root = source.parent  # source=.harness，mount 入参是 harness/
        # 先 unmount 旧挂载（对齐 post-commit 第 7 步）
        run_mavis(["harness", "unmount", "d-item-ae-sdd-harness"])
        rc, out = run_mavis(["harness", "mount", str(harness_root)])
        if not ctx.quiet:
            for line in out.splitlines():
                print(f"    {line}")
        if rc == 0:
            return InstallResult(self.name, "ok", "mavis harness mounted", time.time() - t0)
        return InstallResult(self.name, "fail",
                             f"mavis harness mount 失败 (rc={rc})", time.time() - t0)

    # ── verify ──────────────────────────────────────────────────────────────
    def verify(self, ctx: DistributeContext) -> bool:
        """mavis harness list 能列出 ae-sdd 即通过。"""
        from build_harness import run_mavis
        rc, out = run_mavis(["harness", "list"])
        if rc == 0 and "ae-sdd" in out:
            return True
        log_warn(ctx, f"mavis harness list 未确认 ae-sdd（rc={rc}）")
        return rc == 0

    # ── cleanup（清 -N 副本 + sqlite，迁自 install.py:cleanup_mavis_duplicates） ─
    def cleanup(self, ctx: DistributeContext) -> None:
        keep = MAVIS_KEEP_DEFAULT
        skills_dir = MAVIS_HOME / "skills"
        if not skills_dir.is_dir():
            return
        # 只匹配数字后缀副本（ae-sdd-2 / ae-sdd-3），不碰 ae-sdd-harness-adapter
        pattern = re.compile(rf"^{re.escape(SKILL_NAME)}-\d+$")
        dupes = sorted(
            [p for p in skills_dir.iterdir() if p.is_dir() and pattern.match(p.name)],
            key=lambda p: p.name,
        )
        if not dupes:
            return
        if keep > 0 and len(dupes) > keep:
            dupes = dupes[:-keep]

        # 1. 同步 sqlite 记录（带备份）
        db_path = MAVIS_HOME / "sqlite.db"
        db_deleted = 0
        if db_path.is_file():
            try:
                db_backup = db_path.with_suffix(
                    f".db.bak.{datetime.now().strftime('%Y%m%d%H%M%S')}"
                )
                shutil.copy2(db_path, db_backup)
                conn = sqlite3.connect(str(db_path))
                cur = conn.cursor()
                for d in dupes:
                    cur.execute("DELETE FROM skills WHERE name = ?", (d.name,))
                    db_deleted += cur.rowcount
                conn.commit()
                conn.close()
                log_warn(ctx, f"已备份 mavis sqlite.db → {db_backup.name}")
            except Exception as e:
                log_warn(ctx, f"同步清理 mavis sqlite 记录失败（物理目录仍会清理）: {e}")
        else:
            log_warn(ctx, "未找到 mavis sqlite.db，跳过索引同步（仅清物理目录）")

        # 2. 删物理目录
        removed = 0
        for d in dupes:
            try:
                shutil.rmtree(d)
                log_warn(ctx, f"清理 mavis 端 -N 副本: {d.name}")
                removed += 1
            except OSError as e:
                log_warn(ctx, f"删除 {d.name} 失败: {e}")

        if removed:
            log_info(ctx, f"已清理 mavis 端 {removed} 个 {SKILL_NAME}-N 副本"
                          f"（sqlite 同步删 {db_deleted} 条）")
            if db_deleted < removed:
                log_warn(ctx, "注意：mavis daemon 内存中的 skill 缓存可能未同步，")
                log_warn(ctx, "      如有残留请通过 MiniMax 桌面应用重启 daemon 后再 list 一次。")
