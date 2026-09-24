#!/usr/bin/env python3
"""自动修复 Markdown 模板文件的渲染问题"""

import re
from pathlib import Path
from typing import List

def fix_markdown_file(file_path: Path) -> bool:
    """修复单个 Markdown 文件，返回是否有修改"""
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    fixed_lines = []
    in_code_block = False
    in_table = False
    prev_line_empty = False
    modified = False

    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        # 检查代码块
        if stripped.startswith('```'):
            in_code_block = not in_code_block
            fixed_lines.append(line)
            prev_line_empty = (stripped == '')
            i += 1
            continue

        # 在代码块内，直接保留
        if in_code_block:
            fixed_lines.append(line)
            prev_line_empty = (stripped == '')
            i += 1
            continue

        # 检查表格行
        is_table_line = '|' in stripped and stripped != ''

        if is_table_line:
            if not in_table:
                # 表格开始
                if not prev_line_empty and len(fixed_lines) > 0:
                    # 表格前需要空行
                    fixed_lines.append('\n')
                    modified = True
                in_table = True
            fixed_lines.append(line)
        else:
            if in_table:
                # 表格刚结束
                in_table = False
                if stripped != '':
                    # 表格后需要空行
                    fixed_lines.append('\n')
                    modified = True
                fixed_lines.append(line)
            else:
                # 普通行
                fixed_lines.append(line)

        prev_line_empty = (stripped == '')
        i += 1

    if not modified:
        return False

    # 写回文件
    with open(file_path, 'w', encoding='utf-8', newline='\n') as f:
        f.writelines(fixed_lines)

    return True

def main():
    script_dir = Path(__file__).resolve().parent
    root = script_dir.parent
    templates_dir = root / 'templates'

    fixed_count = 0
    for md_file in templates_dir.rglob('*.md'):
        if fix_markdown_file(md_file):
            rel_path = md_file.relative_to(root)
            print(f"✓ 已修复: {rel_path}")
            fixed_count += 1

    if fixed_count == 0:
        print("✓ 所有模板文件已符合规范，无需修复")
    else:
        print(f"\n共修复 {fixed_count} 个文件")

    return 0

if __name__ == '__main__':
    import sys
    sys.exit(main())
