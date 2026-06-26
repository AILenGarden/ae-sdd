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
      claude/codex/zcode 只需声明 name + target_path + detect()。

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


# ─── CopytreeDistributor 共享基类 ────────────────────────────────────────────
SKILL_NAME = "ae-sdd"
BAK_KEEP_DEFAULT = 2


class CopytreeDistributor(Distributor):
    """copytree 类分发器共享逻辑：备份 → 复制 → 校验 → 清旧 .bak。

    claude/codex/zcode 只需声明 name + target_path + detect()，复用本类的
    install/verify/cleanup（逻辑迁自 install.py 的 backup_existing /
    install_from_dist / verify / cleanup_old_backups，保持行为一致）。
    """
    protocol = "copytree"
    needs_compile = False

    @abstractmethod
    def target_path(self) -> Path:
        """该 Agent 的 skills 安装目标绝对路径。"""
        ...

    # ── 备份 ────────────────────────────────────────────────────────────────
    def _backup_root(self) -> Path:
        """备份根目录：~/.ae-sdd/backups/<agent>/。

        与 skills 目录隔离，避免加载器把 .bak 误识别为独立技能（根治方案X）。
        agent 维度（self.name）区分 claude/codex/zcode，避免跨 agent 备份混在一起。
        """
        return Path.home() / ".ae-sdd" / "backups" / self.name

    def _backup_existing(self, dst: Path, ctx: DistributeContext) -> None:
        """备份已有安装到 ~/.ae-sdd/backups/<agent>/<skill>.bak.<时间戳>。

        🆕 根治方案X：备份目录从 skills/ 移到 ~/.ae-sdd/backups/，与 skills 隔离，
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
