# VS Code Markdown 渲染问题解决方案

## 问题说明

VS Code 内置的 Markdown 预览器对**引用块内的表格**支持不完善，这是 VS Code 的限制，不是文件问题。

文件中有多处这样的结构：
```markdown
> 示例：
>
> | 列1 | 列2 |
> | --- | --- |
> | 值1 | 值2 |
```

在 VS Code 默认预览中，引用块内的表格可能显示为纯文本。

---

## 解决方案

### 方案 1：安装增强型 Markdown 预览插件（推荐）

在 VS Code 扩展商店中安装以下任一插件：

#### A. **Markdown Preview Enhanced** (最推荐)
- 扩展 ID: `shd101wyy.markdown-preview-enhanced`
- 安装命令：
  ```bash
  code --install-extension shd101wyy.markdown-preview-enhanced
  ```
- 使用方法：右键点击 .md 文件 → "Markdown Preview Enhanced: Open Preview"
- 优点：完美支持引用块内的表格、Mermaid 图表、数学公式等

#### B. **Markdown All in One**
- 扩展 ID: `yzhang.markdown-all-in-one`
- 安装命令：
  ```bash
  code --install-extension yzhang.markdown-all-in-one
  ```

#### C. **Markdown Preview Mermaid Support**
- 扩展 ID: `bierner.markdown-mermaid`
- 适合需要渲染 Mermaid 图表的情况

---

### 方案 2：使用外部 Markdown 编辑器

#### A. **Typora** (最佳体验)
- 官网：https://typora.io/
- 所见即所得编辑器，完美支持所有 Markdown 语法
- 可以直接编辑表格，无需手写 Markdown

#### B. **MarkText** (开源免费)
- 官网：https://marktext.app/
- 类似 Typora 的开源替代品

#### C. **Obsidian** (知识管理)
- 官网：https://obsidian.md/
- 如果你用它管理文档，也能完美渲染

---

### 方案 3：在浏览器中查看

将 Markdown 文件推送到 GitHub/GitLab，在线查看：
- GitHub 的 Markdown 渲染器完美支持引用块内的表格
- GitLab 同样支持

---

### 方案 4：转换为 HTML 预览

```bash
# 安装 Pandoc (如果没有)
# Windows: choco install pandoc
# macOS: brew install pandoc

# 转换为 HTML
pandoc templates/project/life-story.md -o life-story.html

# 在浏览器中打开
start life-story.html  # Windows
open life-story.html   # macOS
```

---

## 验证文件是否正确

运行我们的检查脚本：
```bash
cd D:\al-agent-workspace\al-skills\coding\auto-engineering\al-spec
python scripts/check_markdown.py
python scripts/fix_blockquote_tables.py
```

如果输出：
```
✓ 所有模板文件检查通过，无 Markdown 渲染问题
✓ 所有模板文件的引用块表格检查通过
```

说明文件本身完全没问题，只是 VS Code 的预览器不支持而已。

---

## 快速测试

打开这个测试文件：`D:\al-agent-workspace\al-skills\coding\auto-engineering\al-spec\TEST-RENDERING.md`

如果使用 **Markdown Preview Enhanced** 插件，你会看到所有 3 个表格都正常显示。

---

## 推荐配置

安装 Markdown Preview Enhanced 后，在 VS Code 的 settings.json 中添加：

```json
{
  "markdown-preview-enhanced.enableTypographer": true,
  "markdown-preview-enhanced.enableExtendedTableSyntax": true,
  "markdown-preview-enhanced.breakOnSingleNewLine": false
}
```

---

## 结论

✅ **你的 Markdown 文件完全正确**  
❌ **VS Code 默认预览器不支持引用块内的表格**  
✅ **安装 Markdown Preview Enhanced 插件即可解决**

推荐立即安装：
```bash
code --install-extension shd101wyy.markdown-preview-enhanced
```
