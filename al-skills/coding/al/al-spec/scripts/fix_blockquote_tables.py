#!/usr/bin/env python3
"""检测和修复 Markdown 引用块内的表格渲染问题"""

import re
from pathlib import Path
from typing import List, Tuple

def check_blockquote_tables(file_path: Path) -> List[Tuple[int, str, str]]:
    """检查引用块内的表格问题，返回 (行号, 问题类型, 上下文)"""
    issues = []

    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    for i in range(len(lines)):
        line = lines[i]

        # 检测引用块内表格前缺少空引用行
        if i > 0 and line.strip().startswith('> |'):
            prev_line = lines[i - 1].strip()
            # 前一行不是空引用行，也不是表格分隔符
            if prev_line and not prev_line.startswith('> |') and prev_line != '>' and not prev_line.startswith('> ---'):
                # 并且前一行是普通引用文本
                if prev_line.startswith('>') and '|' not in prev_line:
                    context = f"行{i}: '{prev_line[:50]}...' -> 行{i+1}: '{line[:50]}...'"
                    issues.append((i + 1, 'missing_empty_quote_before_table', context))

    return issues

def fix_blockquote_tables(file_path: Path) -> bool:
    """修复引用块内的表格问题"""
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    modified = False
    fixed_lines = []
    i = 0

    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        # 检测并修复：非表格引用行后直接跟表格（需要插入空引用行）
        if i + 1 < len(lines):
            next_line = lines[i + 1].strip()

            # 当前行是引用块内的非空行（不是表格、不是空引用行）
            if stripped.startswith('>') and '|' not in stripped and stripped != '>':
                # 下一行是引用块内的表格
                if next_line.startswith('> |'):
                    # 在它们之间插入空引用行
                    fixed_lines.append(line)
                    fixed_lines.append('>\n')
                    print(f"  修复行 {i+1}: 在引用文本和表格之间插入空引用行")
                    modified = True
                    i += 1
                    continue

        # 检测并修复：空引用行后紧接表格（删除多余的空引用行）
        if stripped == '>' and i + 1 < len(lines):
            next_line = lines[i + 1].strip()
            if next_line.startswith('> |'):
                # 检查前一行是否也是空引用行
                if i > 0 and fixed_lines and fixed_lines[-1].strip() == '>':
                    # 跳过这个多余的空引用行
                    print(f"  修复行 {i+1}: 删除表格前的多余空引用行")
                    modified = True
                    i += 1
                    continue

        fixed_lines.append(line)
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

    print("=== 检查引用块内表格问题 ===\n")

    all_issues = {}
    for md_file in templates_dir.rglob('*.md'):
        issues = check_blockquote_tables(md_file)
        if issues:
            rel_path = md_file.relative_to(root)
            all_issues[str(rel_path)] = issues

    if not all_issues:
        print("✓ 所有模板文件的引用块表格检查通过\n")
    else:
        print(f"发现 {len(all_issues)} 个文件存在问题:\n")
        for file_path, issues in all_issues.items():
            print(f"📄 {file_path}")
            for line_num, issue_type, context in issues:
                print(f"  {context}")
            print()

    print("\n=== 开始修复 ===\n")

    fixed_count = 0
    for md_file in templates_dir.rglob('*.md'):
        rel_path = md_file.relative_to(root)
        if fix_blockquote_tables(md_file):
            print(f"✓ 已修复: {rel_path}\n")
            fixed_count += 1

    if fixed_count == 0:
        print("✓ 无需修复")
    else:
        print(f"\n共修复 {fixed_count} 个文件")

    return 0

if __name__ == '__main__':
    import sys
    sys.exit(main())
