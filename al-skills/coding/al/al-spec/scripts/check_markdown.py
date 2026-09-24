#!/usr/bin/env python3
"""检查 Markdown 模板文件的渲染问题"""

import re
from pathlib import Path
from typing import List, Tuple

def check_markdown_file(file_path: Path) -> List[Tuple[int, str]]:
    """检查单个 Markdown 文件，返回问题列表 (行号, 问题描述)"""
    issues = []

    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    in_code_block = False
    in_table = False
    prev_line = ""

    for i, line in enumerate(lines, start=1):
        stripped = line.strip()

        # 检查代码块
        if stripped.startswith('```'):
            in_code_block = not in_code_block

        # 检查表格（排除引用块内的表格）
        if '|' in stripped and not in_code_block and not stripped.startswith('>'):
            if not in_table:
                # 表格开始，检查前面是否有空行
                if prev_line.strip() != '' and i > 1:
                    issues.append((i, "表格前缺少空行"))
                in_table = True
        elif in_table and stripped == '':
            in_table = False
        elif in_table and '|' not in stripped:
            # 表格结束但没有空行
            in_table = False
            if stripped != '':
                issues.append((i, "表格后缺少空行"))

        # 检查列表项
        if re.match(r'^[-*+]\s', stripped):
            # 检查列表项是否有内容
            if len(stripped) <= 2:
                issues.append((i, "列表项为空"))

        # 检查标题
        if stripped.startswith('#'):
            # 检查标题前是否有空行（除了文件开头）
            if i > 1 and prev_line.strip() != '' and not prev_line.strip().startswith('>'):
                issues.append((i, "标题前缺少空行"))

        prev_line = line

    # 检查代码块是否闭合
    if in_code_block:
        issues.append((len(lines), "代码块未正确闭合"))

    return issues

def main():
    script_dir = Path(__file__).resolve().parent
    root = script_dir.parent
    templates_dir = root / 'templates'

    all_issues = {}

    for md_file in templates_dir.rglob('*.md'):
        issues = check_markdown_file(md_file)
        if issues:
            rel_path = md_file.relative_to(root)
            all_issues[str(rel_path)] = issues

    if not all_issues:
        print("✓ 所有模板文件检查通过，无 Markdown 渲染问题")
        return 0

    print(f"发现 {len(all_issues)} 个文件存在问题:\n")
    for file_path, issues in all_issues.items():
        print(f"📄 {file_path}")
        for line_num, issue in issues:
            print(f"  行 {line_num}: {issue}")
        print()

    return 1

if __name__ == '__main__':
    import sys
    sys.exit(main())
