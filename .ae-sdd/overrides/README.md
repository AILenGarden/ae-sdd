# 项目覆盖（Overrides）

> **Override 解析规则：** 实例有效规则 = 母版 defaults + overrides/（同名覆盖）

本目录放项目特化规则，**覆盖母版**（`../ae-sdd/source/`）的同名文件。

## 用法

### 1. 约束特化
把母版 `standards/constraints/api.md` 复制到这里，按项目调整字段：
```bash
cp ../ae-sdd/source/standards/constraints/api.md ./api.md
# 编辑 ./api.md 加项目特定内容
```

### 2. 模板特化
把母版 `templates/design/be-story-template.md` 复制到这里，按项目调整字段：
```bash
cp ../ae-sdd/source/templates/design/be-story-template.md ./be-story-template.md
# 编辑，加项目元信息字段
```

### 3. 不要复制 SKILL
SKILL 是节点级通用规则，**不实例化**。如有项目特定流程扩展，单独写 `<project>-SKILL.md` 自定义 SKILL，不放在 overrides/。

## 启动时
`ae-sdd` 会先读母版 defaults，然后读 `overrides/` 同名文件覆盖。
