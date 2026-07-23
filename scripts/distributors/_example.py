"""新增 Agent 分发器示范（不注册，仅供参考/复制）。

本文件演示如何为一个新的 Agent（例如 Cursor）接入 ae-sdd 自动分发闭环。
它不会被注册到 DISTRIBUTORS（见 __init__.py），仅作为模板。

────────────────────────────────────────────────────────────────────────
新增一个 Agent 分发器的 3 步法（详见 _base.py 顶部 docstring）：

1. 复制本文件为 <agent>.py，改 class 名 + name + 协议实现
2. 在 __init__.py 的 DISTRIBUTORS 列表加一行 import + 注册
3. （可选）若该 Agent 需要专属编译产物，设 needs_compile=True 并实现 compile()

无需改 distribute.py / install.py / post-commit —— orchestrator 自动发现。
────────────────────────────────────────────────────────────────────────

下面以一个假想的 Cursor 分发器为例，展示两种典型协议：
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from ._base import Distributor, CopytreeDistributor, DistributeContext, InstallResult


# ═══ 示例 A：copytree 协议（最常见，跟 claude/codex/zcode/hermes 一样） ═════════════
class CursorDistributorExample(CopytreeDistributor):
    """Cursor 分发器示范：copytree → ~/.cursor/skills/ae-sdd。

    CopytreeDistributor 已实现 install/verify/cleanup（备份+复制+清旧 .bak），
    子类只需声明 name + target_path + detect()。
    """
    name = "cursor"  # ← 改成你的 Agent 名

    def target_path(self) -> Path:
        # ← 改成该 Agent 的 skills 安装目录
        return Path.home() / ".cursor" / "skills" / "ae-sdd"

    def detect(self) -> bool:
        """auto 模式下是否启用。通常判断：目标目录已存在 或 CLI 可用。"""
        import shutil
        dst = self.target_path()
        return dst.exists() or bool(shutil.which("cursor") or shutil.which("cursor.exe"))


# ═══ 示例 B：自定义协议（如需专属编译产物 + 非标准安装方式） ═════════════════
class CustomProtocolExample(Distributor):
    """自定义协议示范：演示 needs_compile=True + 自定义 install。

    适用于：Agent 不吃标准 dist 包，需要专属编译产物 + 独特安装命令。
    （harness.py 就是这种模式：compile 调 build_harness.py，install 调 harness mount）
    """
    name = "custom-agent"
    protocol = "custom_protocol"
    needs_compile = True  # ← 需要专属编译产物

    def detect(self) -> bool:
        import shutil
        return bool(shutil.which("custom-agent"))

    def compile(self, repo_root: Path) -> Optional[Path]:
        """产出该 Agent 专属编译产物，返回产物路径；失败返回 None。

        例如调一个编译脚本：python scripts/build_for_custom_agent.py
        """
        # 这里写编译逻辑...
        # 返回编译产物所在目录
        return repo_root / "dist" / "custom-agent"

    def install(self, source: Path, ctx: DistributeContext) -> InstallResult:
        """按该 Agent 自己的协议安装（source=compile 产物或通用 dist）。"""
        import subprocess
        import sys
        import time
        t0 = time.time()
        try:
            # 这里写该 Agent 特有的安装命令...
            # result = subprocess.run(["custom-agent", "install", str(source)])
            return InstallResult(self.name, "ok", "installed", time.time() - t0)
        except Exception as e:
            return InstallResult(self.name, "fail", str(e), time.time() - t0)

    def verify(self, ctx: DistributeContext) -> bool:
        """安装后校验（可选，默认 True）。"""
        return True

    def cleanup(self, ctx: DistributeContext) -> None:
        """收尾（可选，如清理临时文件/旧副本）。"""
        pass


# ═══ 注册示范（取消注释并改 class 名后生效） ═════════════════════════════════
# 在 __init__.py 的 DISTRIBUTORS 列表加入：
#   from ._example import CursorDistributorExample as CursorDistributor
#   DISTRIBUTORS = [..., CursorDistributor]
