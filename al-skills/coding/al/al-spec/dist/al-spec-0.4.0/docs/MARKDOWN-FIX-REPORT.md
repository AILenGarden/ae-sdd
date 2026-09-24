# al-spec 模板渲染问题修复报告

## 问题诊断

### 发现的问题
在 `templates/project/` 目录下的三个模板文件存在 **Markdown 渲染问题**：
- `life-dr.md`
- `life-story.md`
- `life-testcase.md`

### 问题根因
**表格前后缺少空行**，导致某些 Markdown 渲染器（如 GitHub、GitLab、VS Code）无法正确识别表格结构。

### 受影响范围
- life-dr.md: 12 处问题
- life-story.md: 20 处问题
- life-testcase.md: 2 处问题

---

## 修复方案

### 自动化工具
创建了两个 Python 脚本用于检测和修复：

#### 1. `scripts/check_markdown.py`
**功能：** 检测 Markdown 模板文件的渲染问题
- 扫描所有 `templates/**/*.md` 文件
- 检测表格前后是否有空行
- 检测代码块是否正确闭合
- 输出问题清单（文件名 + 行号 + 问题描述）

**使用方法：**
```bash
python scripts/check_markdown.py
```

#### 2. `scripts/fix_markdown.py`
**功能：** 自动修复表格前后空行问题
- 自动在表格前插入空行
- 自动在表格后插入空行
- 保留代码块和其他格式
- 保持原始换行符（LF）

**使用方法：**
```bash
python scripts/fix_markdown.py
```

---

## 修复结果

### 修复前后对比

**修复前：**
```markdown
> 示例：
> | 工程 | 类型 | 说明 |
> | --- | --- | --- |
> | icec-cloud-life-im | Service | 新增接口 |
```
❌ 表格无法正确渲染，显示为普通文本

**修复后：**
```markdown
> 示例：
>
> | 工程 | 类型 | 说明 |
> | --- | --- | --- |
> | icec-cloud-life-im | Service | 新增接口 |
```
✅ 表格正确渲染为表格

### 验证结果
```bash
$ python scripts/check_markdown.py
✓ 所有模板文件检查通过，无 Markdown 渲染问题
```

---

## 遵循的 Markdown 规范

根据 **CommonMark 规范** 和主流渲染器的实践：

### 表格规则
1. **表格前必须有空行**（除非是文件开头）
2. **表格后必须有空行**（除非是文件结尾）
3. 表格不能紧接在普通段落后面
4. 引用块内的表格同样需要遵循此规则

### 示例
```markdown
这是一段文字。
                           ← 必须有空行
| 列1 | 列2 |
| --- | --- |
| 值1 | 值2 |
                           ← 必须有空行
这是另一段文字。
```

---

## 修复涉及的文件

### 已修复
- ✅ `templates/project/life-dr.md`
- ✅ `templates/project/life-story.md`
- ✅ `templates/project/life-testcase.md`

### 无需修复
- ✅ `templates/global/dr.md` (已符合规范)
- ✅ `templates/global/story.md` (已符合规范)
- ✅ `templates/global/testcase.md` (已符合规范)

---

## 提交记录

```
commit 2c08ccf
fix(al-spec): 修复项目级模板 Markdown 渲染问题

- 修复 life-dr.md、life-story.md、life-testcase.md 表格前后缺少空行
- 新增 check_markdown.py 检测脚本
- 新增 fix_markdown.py 自动修复脚本
- 更新 CHANGELOG/2026-09-01.md
```

---

## 后续建议

### 1. CI/CD 集成
将 `check_markdown.py` 集成到 CI 流程：
```yaml
# .github/workflows/check-markdown.yml
- name: Check Markdown Templates
  run: python scripts/check_markdown.py
```

### 2. Pre-commit Hook
在提交前自动检查：
```bash
# .git/hooks/pre-commit
#!/bin/sh
python scripts/check_markdown.py || exit 1
```

### 3. 编辑器配置
推荐使用支持 Markdown 预览的编辑器：
- VS Code + Markdown Preview Enhanced
- Typora
- MarkText

---

## 总结

✅ **问题已完全解决**
- 所有项目级模板文件现在可以在所有主流 Markdown 渲染器中正确显示
- 提供了自动化工具防止未来出现类似问题
- 遵循 CommonMark 规范确保最大兼容性

📅 修复日期：2026-09-01
👤 修复人：Claude Code
🔧 工具版本：check_markdown.py v1.0, fix_markdown.py v1.0
