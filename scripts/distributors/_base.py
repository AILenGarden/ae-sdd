"""分发器抽象基类 + 共享上下文/结果类型。

本模块定义 ae-sdd 自动分发闭环的插件契约：

    Distributor（ABC）
      ├── detect()    —— 该 Agent 是否可用（CLI 存在 / 目标目录存在）
      ├── compile()   —— needs_compile=True 时产出专属产物；否则返回 None（用通用 dist）
      ├── install()   —— 按该 Agent 自己的协议安装
      ├── verify()    —— 安装后校验（可选，默认 True）
      └── cleanup()   —— 收尾（可选，mavis 清 -N 副本）

    CopytreeDistributor（Distributor 子类）
      共享 copytree 类安装逻辑（备份 → 复制 → 校验 → 清旧 .bak），
      claude/codex/zcode/hermes 只需声明 name + target_path + detect()。

────────────────────────────────────────────────────────────────────────
如何新增一个 Agent 分发器（3 步法）：

1. 在本目录新建 `<agent>.py`，继承 Distributor 或 CopytreeDistributor：
       class FooDistributor(CopytreeDistributor):
           name = "foo"
           def target_path(self) -> Path: ...
           def detect(self) -> bool: ...

2. 在 `__init__.py` 的 DISTRIBUTORS 列表里加一行 import + 注册：
       from .foo import FooDistributor
       DISTRIBUTORS = [..., FooDistributor]

3. （可选）若该 Agent 需要专属编译产物，把 needs_compile=True 并实现 compile()。

无需改 distribute.py / install.py / post-commit —— orchestrator 自动发现新分发器。
参考 `distributors/_example.py` 的注释骨架。
────────────────────────────────────────────────────────────────────────
"""
from __future__ import annotations

import shutil
import subprocess
import sys
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional


# ─── 颜色（ANSI） ────────────────────────────────────────────────────────────
def _supports_color() -> bool:
    import sys
    return sys.stdout.isatty()


if _supports_color():
    C_GREEN = "\033[0;32m"
    C_YELLOW = "\033[1;33m"
    C_RED = "\033[0;31m"
    C_BLUE = "\033[0;34m"
    C_RESET = "\033[0m"
else:
    C_GREEN = C_YELLOW = C_RED = C_BLUE = C_RESET = ""


# ─── 共享上下文/结果 ─────────────────────────────────────────────────────────
@dataclass
class DistributeContext:
    """单次分发运行的共享上下文，传给每个 Distributor。"""
    repo_root: Path
    dist_path: Path              # 通用 dist 产物路径（build_dist 产出）
    keep_bak: int = 2            # 每个目标保留的 .bak 备份数（0=全清；负数=不清理）
    quiet: bool = False          # 静默模式，只输出关键状态
    from_commit: bool = False    # 是否由 post-commit hook 触发（影响日志详尽度）
    use_ps1: bool = False        # mavis 专属：是否回退用 PS1 而非 build_harness.py


@dataclass
class InstallResult:
    """单个 Distributor 的安装结果。"""
    name: str
    status: str                  # "ok" / "warn" / "fail" / "skip"
    message: str = ""
    duration_sec: float = 0.0


# ─── 日志助手（受 ctx.quiet 控制） ───────────────────────────────────────────
def log_info(ctx: Optional[DistributeContext], msg: str) -> None:
    if ctx is None or not ctx.quiet:
        print(f"{C_GREEN}[ae-sdd]{C_RESET} {msg}")


def log_warn(ctx: Optional[DistributeContext], msg: str) -> None:
    if ctx is None or not ctx.quiet:
        print(f"{C_YELLOW}[ae-sdd] ⚠{C_RESET}  {msg}")


def log_error(msg: str) -> None:
    import sys
    print(f"{C_RED}[ae-sdd] ✗{C_RESET}  {msg}", file=sys.stderr)


def log_success(ctx: Optional[DistributeContext], msg: str) -> None:
    if ctx is None or not ctx.quiet:
        print(f"{C_GREEN}[ae-sdd] ✅{C_RESET} {msg}")


def _verify_compiled_runtime_source(source: Path, ctx: DistributeContext) -> tuple[bool, str]:
    """Reject uncompiled source packages before installation."""
    tools_dir = ctx.repo_root / "tools"
    inserted = False
    if str(tools_dir) not in sys.path:
        sys.path.insert(0, str(tools_dir))
        inserted = True
    try:
        from lib.runtime_verify import verify_runtime_package  # type: ignore
        result = verify_runtime_package(source)
    except Exception as exc:
        return False, f"compiled runtime 校验器不可用: {exc}"
    finally:
        if inserted and str(tools_dir) in sys.path:
            sys.path.remove(str(tools_dir))

    if result.ok:
        return True, ""
    return False, "; ".join(result.issues[:5])


# ─── Distributor ABC ─────────────────────────────────────────────────────────
class Distributor(ABC):
    """Agent 分发器抽象基类。每个子类代表一种 Agent 的安装协议。"""
    name: str = ""               # "claude" / "codex" / "zcode" / "mavis" ...
    protocol: str = ""           # "copytree" / "harness_mount" / ...
    needs_compile: bool = False  # 是否需要专属编译产物（mavis=True，其余=False）

    @abstractmethod
    def detect(self) -> bool:
        """该 Agent 是否可用（CLI 存在 / 目标目录存在）。auto 模式下只跑 detect=True 的。"""
        ...

    def compile(self, repo_root: Path) -> Optional[Path]:
        """needs_compile=True 时产出专属产物并返回路径；否则返回 None（用通用 dist）。"""
        return None

    @abstractmethod
    def install(self, source: Path, ctx: DistributeContext) -> InstallResult:
        """按该 Agent 自己的协议安装。source=通用 dist 或专属产物。"""
        ...

    def verify(self, ctx: DistributeContext) -> bool:
        """安装后校验（可选 hook，默认通过）。"""
        return True

    def cleanup(self, ctx: DistributeContext) -> None:
        """收尾（可选 hook，mavis 清 -N 副本 + sqlite）。"""
        pass


# ─── CopytreeDistributor 共享基类（🆕 2026-07-03 数据驱动，不再需要子类）───────
SKILL_NAME = "ae-sdd"
BAK_KEEP_DEFAULT = 2


class CopytreeDistributor(Distributor):
    """copytree 类分发器：备份 → 复制 → 校验 → 清旧 .bak。

    🆕 2026-07-03 注册表模式：不再需要为每个 Agent 写 .py 子类。
    直接用注册表数据构造实例：
        CopytreeDistributor(name="claude", target_path=Path("~/.claude/skills/ae-sdd"),
                            detect_fn=lambda: True)
    install/verify/cleanup 逻辑迁自 install.py，保持行为一致。
    """
    protocol = "copytree"
    needs_compile = False

    def __init__(
        self,
        name: str = "",
        target_path: Optional[Path] = None,
        detect_fn: Optional[callable] = None,
    ) -> None:
        self.name = name
        self._target_path = target_path or Path()
        # detect_fn 返回 bool；None 时永远 True（向后兼容 claude 的 always）
        self._detect_fn = detect_fn or (lambda: True)

    def target_path(self) -> Path:
        return self._target_path

    def detect(self) -> bool:
        """auto 模式：由注册表的 detect 策略决定。"""
        return self._detect_fn()

    # ── 备份 ────────────────────────────────────────────────────────────────
    def _backup_root(self) -> Path:
        """备份根目录：跟着被备份的 agent 安装走 → ~/.<agent>/ae-sdd-backups/。

        语义：备份是"某 agent 安装的回滚副本"，应留在该 agent 域内（skills 同级），
        而不是 skills 目录内（避免加载器把 .bak 误识别为独立技能），
        也不混入 ae-sdd 工具配置区（~/.ae-sdd/）。

        推导：target_path() = ~/.<agent>/skills/ae-sdd
              → parent.parent = ~/.<agent>/  → 拼 ae-sdd-backups/
        """
        return self.target_path().parent.parent / "ae-sdd-backups"

    def _backup_existing(self, dst: Path, ctx: DistributeContext) -> None:
        """备份已有安装到 ~/.<agent>/ae-sdd-backups/<skill>.bak.<时间戳>。

        🆕 根治：备份目录从 skills/ 移到 skills 同级（agent 域内），与 skills 隔离，
        避免技能加载器把 .bak 当独立技能（迁自 install.py:backup_existing）。
        """
        if dst.exists():
            ts = datetime.now().strftime("%Y%m%d%H%M%S")
            backup_root = self._backup_root()
            backup_root.mkdir(parents=True, exist_ok=True)
            bak = backup_root / f"{dst.name}.bak.{ts}"
            log_warn(ctx, f"检测到已有安装版本，备份到：")
            log_warn(ctx, f"  {bak}")
            dst.rename(bak)

    def _cleanup_old_backups(self, keep: int) -> None:
        """清理备份目录下 {skill}.bak.* 旧备份，保留最近 keep 个。

        🆕 根治方案X：扫描 ~/.ae-sdd/backups/<agent>/（不再扫 skills 目录）。
        keep=0 全清；keep<0 不清理。
        """
        if keep < 0:
            return
        backup_root = self._backup_root()
        if not backup_root.is_dir():
            return
        pattern = f"{SKILL_NAME}.bak.*"
        baks = sorted(
            (p for p in backup_root.glob(pattern) if p.is_dir()),
            key=lambda p: p.name,
            reverse=True,
        )
        if len(baks) <= keep:
            return
        for old in baks[keep:]:
            try:
                shutil.rmtree(old)
                log_warn(None, f"清理旧备份: {old.name}")
            except OSError as e:
                log_warn(None, f"清理失败 {old.name}: {e}")

    # ── 安装 ────────────────────────────────────────────────────────────────
    def install(self, source: Path, ctx: DistributeContext) -> InstallResult:
        """从通用 dist 复制到目标 skills 目录（迁自 install.py:install_from_dist）。"""
        import time
        t0 = time.time()
        dst = self.target_path()
        try:
            if not source.is_dir():
                return InstallResult(self.name, "fail",
                                     f"未找到 dist 源: {source}", time.time() - t0)
            skill_md = source / "SKILL.md"
            if not skill_md.is_file():
                return InstallResult(self.name, "fail",
                                     f"未找到 {skill_md}，请先跑 build_dist", time.time() - t0)
            compiled_ok, compiled_msg = _verify_compiled_runtime_source(source, ctx)
            if not compiled_ok:
                return InstallResult(
                    self.name,
                    "fail",
                    f"拒绝安装未编译/不完整 runtime package: {compiled_msg}",
                    time.time() - t0,
                )

            self._backup_existing(dst, ctx)
            self._cleanup_old_backups(ctx.keep_bak)

            if dst.exists():
                shutil.rmtree(dst)
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(source, dst)
            log_info(ctx, f"文件已复制到 {dst}")
            return InstallResult(self.name, "ok", str(dst), time.time() - t0)
        except Exception as e:
            return InstallResult(self.name, "fail", str(e), time.time() - t0)

    # ── 校验 ────────────────────────────────────────────────────────────────
    def verify(self, ctx: DistributeContext) -> bool:
        """验证安装：SKILL.md 存在 + VERSION 可读（迁自 install.py:verify）。"""
        dst = self.target_path()
        skill_md = dst / "SKILL.md"
        if not skill_md.is_file():
            log_error(f"安装验证失败：{skill_md} 不存在")
            return False
        version_file = dst / "VERSION"
        if version_file.is_file():
            ver = version_file.read_text(encoding="utf-8").split("\n")[0]
            log_info(ctx, f"安装版本: {ver} ({dst})")
        return True


# ─── HarnessMountDistributor（🆕 2026-07-03 从 mavis.py 抽象，参数化）─────────
# 协议模板：harness_mount 类 Agent（mavis 及未来同类）。
# 逻辑迁自 distributors/mavis.py，保持行为一致：compile(build_harness) →
# install(mavis harness mount) → verify(harness list) → cleanup(-N 副本 + sqlite)。

_HARNESS_KEEP_DEFAULT = 0   # 清理 -N 副本保留数（0=全清；负数=不清理）


class HarnessMountDistributor(Distributor):
    """harness_mount 协议模板：调 build_harness.py 生成 agent.md + mavis harness mount。

    🆕 2026-07-03 注册表模式：不再需要 mavis.py 独立子类。
    直接用注册表数据构造：
        HarnessMountDistributor(name="mavis", agent_home=Path("~/.mavis"),
                                detect_fn=lambda: find_mavis_cmd() is not None)
    """
    protocol = "harness_mount"
    needs_compile = True

    def __init__(
        self,
        name: str = "",
        agent_home: Optional[Path] = None,
        detect_fn: Optional[callable] = None,
    ) -> None:
        self.name = name
        self.agent_home = agent_home or (Path.home() / ".mavis")
        self._detect_fn = detect_fn or (lambda: False)

    def detect(self) -> bool:
        return self._detect_fn()

    def compile(self, repo_root: Path) -> Optional[Path]:
        """调 build_harness.py 生成 .harness/agent.md，返回 .harness 目录。"""
        scripts_dir = repo_root / "scripts"
        build_harness = scripts_dir / "build_harness.py"
        if not build_harness.is_file():
            log_error(f"build_harness.py 不存在: {build_harness}")
            return None
        result = subprocess.run(
            [sys.executable, str(build_harness), "--source", str(repo_root), "--no-mount"],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            log_error(f"build_harness.py 失败 (rc={result.returncode})")
            if result.stderr:
                print(result.stderr, file=sys.stderr)
            return None
        harness_dir = repo_root / ".harness"
        if (harness_dir / "agent.md").is_file():
            return harness_dir
        return None

    def install(self, source: Path, ctx: DistributeContext) -> InstallResult:
        """source 是 compile 产出的 .harness 目录；执行 mavis harness mount。"""
        import time
        t0 = time.time()
        # build_harness 与本包同级（scripts/），由调用方保证 sys.path 含 scripts/
        try:
            from build_harness import run_mavis, find_mavis_cmd, mavis_harness_name_for_path
        except ImportError:
            return InstallResult(self.name, "skip",
                                 "build_harness.py 不可导入，跳过 mount", time.time() - t0)

        if find_mavis_cmd() is None:
            return InstallResult(self.name, "skip",
                                 f"{self.name} 未安装，跳过 mount（产物已写入）", time.time() - t0)

        if self.verify(ctx):
            return InstallResult(self.name, "ok", f"{self.name} harness already mounted", time.time() - t0)

        harness_root = source.parent  # source=.harness，mount 入参是 repo root
        # 先 unmount 旧挂载
        for hname in dict.fromkeys([
            mavis_harness_name_for_path(harness_root),
            mavis_harness_name_for_path(harness_root / "harness"),
            SKILL_NAME,
        ]):
            run_mavis(["harness", "unmount", hname])
        rc, out = run_mavis(["harness", "mount", str(harness_root)])
        if not ctx.quiet:
            for line in out.splitlines():
                print(f"    {line}")
        if rc == 0 and self.verify(ctx):
            return InstallResult(self.name, "ok", f"{self.name} harness mounted", time.time() - t0)
        return InstallResult(self.name, "fail",
                             f"{self.name} harness mount 失败 (rc={rc})", time.time() - t0)

    def verify(self, ctx: DistributeContext) -> bool:
        """harness list 能列出 ae-sdd 即通过。"""
        try:
            from build_harness import run_mavis
        except ImportError:
            return False
        rc, out = run_mavis(["harness", "list"])
        if rc == 0 and SKILL_NAME in out:
            return True
        log_warn(ctx, f"{self.name} harness list 未确认 {SKILL_NAME}（rc={rc}）")
        return False

    def cleanup(self, ctx: DistributeContext) -> None:
        """清 -N 副本 + 同步 sqlite（迁自 mavis.py:cleanup）。"""
        import re
        import sqlite3
        from datetime import datetime
        keep = _HARNESS_KEEP_DEFAULT
        skills_dir = self.agent_home / "skills"
        if not skills_dir.is_dir():
            return
        pattern = re.compile(rf"^{re.escape(SKILL_NAME)}-\d+$")
        dupes = sorted(
            [p for p in skills_dir.iterdir() if p.is_dir() and pattern.match(p.name)],
            key=lambda p: p.name,
        )
        if not dupes:
            return
        if keep > 0 and len(dupes) > keep:
            dupes = dupes[:-keep]

        db_path = self.agent_home / "sqlite.db"
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
                log_warn(ctx, f"已备份 {self.name} sqlite.db → {db_backup.name}")
            except Exception as e:
                log_warn(ctx, f"同步清理 {self.name} sqlite 记录失败（物理目录仍会清理）: {e}")
        else:
            log_warn(ctx, f"未找到 {self.name} sqlite.db，跳过索引同步（仅清物理目录）")

        removed = 0
        for d in dupes:
            try:
                shutil.rmtree(d)
                log_warn(ctx, f"清理 {self.name} 端 -N 副本: {d.name}")
                removed += 1
            except OSError as e:
                log_warn(ctx, f"删除 {d.name} 失败: {e}")

        if removed:
            log_info(ctx, f"已清理 {self.name} 端 {removed} 个 {SKILL_NAME}-N 副本"
                          f"（sqlite 同步删 {db_deleted} 条）")
