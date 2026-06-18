#!/usr/bin/env python3
"""
run.py — 跑 ae-sdd 全部单元测试（跨平台薄壳）

用法:
    python tools/tests/run.py            # 跑全部
    python tools/tests/run.py paths      # 只跑 test_paths.py
    python tools/tests/run.py -v         # verbose

零外部依赖（仅标准库 unittest）。
"""
import sys
import unittest
from pathlib import Path

# 让 import lib 找得到
TESTS_DIR = Path(__file__).resolve().parent
TOOLS_DIR = TESTS_DIR.parent
sys.path.insert(0, str(TOOLS_DIR))


def discover(pattern: str = "test_*.py") -> unittest.TestSuite:
    """发现所有匹配 pattern 的测试"""
    loader = unittest.TestLoader()
    return loader.discover(start_dir=str(TESTS_DIR), pattern=pattern, top_level_dir=str(TOOLS_DIR))


def main() -> int:
    args = sys.argv[1:]

    # verbosity
    if "-v" in args or "--verbose" in args:
        verbosity = 2
    else:
        verbosity = 1

    # pattern 过滤
    pattern = "test_*.py"
    if args and not args[0].startswith("-"):
        # 第一个非 flag 参数 = 测试文件名前缀（如 "paths" → "test_paths.py"）
        prefix = args[0]
        pattern = f"test_{prefix}.py"

    suite = discover(pattern)
    runner = unittest.TextTestRunner(verbosity=verbosity)
    result = runner.run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
