# plugins/_example-coding-style/

> **这是 ae-sdd v3.5.0 插件化的 scaffolding 示例，不是生产插件。**
> 不会被自动加载（因为没有在仓库根 `plugins/registry.yaml` 注册）。

## 用途

展示**如何写一个 CodingSKILL 插件**。给项目 owner 看的样板。

## 如何启用

**L1 项目层（推荐）：**
```bash
# 1. 拷贝到你的项目
cp -r plugins/_example-coding-style/ <your-project>/.ae-sdd/plugins/my-coding/

# 2. 在 <your-project>/.ae-sdd/plugins/registry.yaml 添加：
cat >> <your-project>/.ae-sdd/plugins/registry.yaml <<EOF
plugins:
  - name: my-coding
    type: skill-override
    version: 0.1.0
    description: my project's TDD + DDD style
    replaces: source/skills/phase2-coding/coding-skill.md
    path: ./my-coding/SKILL.md
EOF

# 3. 修改 my-coding/SKILL.md 内容为你的团队约定

# 4. 验证
ae-sdd plugin validate
ae-sdd plugin trace coding-skill.md
```

**L2 全局层：**
```bash
# 1. 拷贝到全局
cp -r plugins/_example-coding-style/ ~/.ae-sdd/plugins/my-coding/

# 2. 在 ~/.ae-sdd/plugins/registry.yaml 添加（格式同上）
```

## 为什么本目录不自动加载

本目录在**仓库根**（L3 层），按设计只有 ae-sdd 团队发布官方扩展时才会注册。
普通项目 owner 应该走 L1（项目层）或 L2（全局层）路径。

## 参见

- 注册表 schema：[`source/standards/constraints/plugin-registry-spec.md`](../source/standards/constraints/plugin-registry-spec.md)
- 注册流程引导：[`source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md §3`](../source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md)
- 设计文档：[`source/docs/plans/2026-06-26-plugin-registry-design.md`](../source/docs/plans/2026-06-26-plugin-registry-design.md)
- 用户文档（README.md）：[`README.md §🔌 SKILL 注册与外挂指南`](../README.md)