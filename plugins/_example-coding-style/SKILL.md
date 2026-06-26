---
name: example-coding-style
description: |
  示例插件：TDD + DDD 风格的 CodingSKILL（v3.5.0 plugin registry scaffolding）。
  本插件作为 ae-sdd 仓库根层（L3）的官方 scaffolding 示例存在；
  不会被默认启用，**仅供项目 owner 参考如何写外挂 SKILL**。
  如果你希望覆盖内置 coding-skill.md，请把本目录拷贝到 `<project>/.ae-sdd/plugins/` 后
  修改 `../../plugins/registry.yaml` 启用。
---

# Example Coding Style (TDD + DDD)

> **这是 ae-sdd 插件化的 scaffolding 示例，不是生产 CodingSKILL。**
> 仓库根 `plugins/_example-coding-style/` 仅作展示，不会被自动加载（因为没有在 `plugins/registry.yaml` 注册）。

## 为什么需要这个示例

每个项目团队的 Coding 风格都不同。ae-sdd v3.5.0 起支持**三层插件化**：
- L1 项目层（`<project>/.ae-sdd/plugins/registry.yaml`）
- L2 全局层（`~/.ae-sdd/plugins/registry.yaml`）
- L3 仓库根层（本目录，仅 ae-sdd 团队发布用）

本示例展示**如何写一个 CodingSKILL 插件**。流程：

```
1. 拷贝本目录到你的项目：
   cp -r plugins/_example-coding-style/ <your-project>/.ae-sdd/plugins/my-coding/

2. 在 <your-project>/.ae-sdd/plugins/registry.yaml 添加：
   - name: my-coding
     type: skill-override
     version: 0.1.0
     description: my project's TDD + DDD style
     replaces: source/skills/phase2-coding/coding-skill.md
     path: ./my-coding/SKILL.md

3. 修改 my-coding/SKILL.md 内容为你的团队约定

4. 验证：ae-sdd plugin validate
   trace: ae-sdd plugin trace coding-skill.md
```

## 示例内容（参考用）

### §1 Coding 总原则

- **测试先行**（TDD）：先写失败测试 → 写最小实现 → 重构
- **领域驱动**（DDD）：核心业务用 Aggregate / Entity / Value Object 表达
- **可观测**：所有外部调用记录 trace_id + span
- **幂等**：所有写操作幂等（用唯一约束 / 乐观锁）

### §2 实现门禁

| 维度 | 要求 |
|------|------|
| 可用性 | 覆盖 Story 所有 AC |
| 高效性 | 单查询 < 100ms（无 N+1）|
| 可维护性 | 复用项目资产定义的公共组件 |
| 健壮性 | 含重试 + 幂等 + 失败兜底 |
| 可读性 | 命名表达业务语义，注释解释"为什么" |

### §3 流程

1. 读 Story AC → 列出每个 AC 对应测试点
2. 写测试 → 跑测试（确保失败）
3. 写实现 → 跑测试（确保通过）
4. 重构 → 跑测试（确保仍通过）
5. 跑 `ae-sdd gate coding-required` → 通过

### §4 反模式（禁止）

- ❌ 跳过测试直接写代码
- ❌ 业务逻辑写在 Controller 层
- ❌ 用 Thread.sleep 等待异步结果
- ❌ catch (Exception e) {} 吞异常
- ❌ 用 magic number / 硬编码字符串

---

> **完整 schema 见** [`source/standards/constraints/plugin-registry-spec.md`](../../source/standards/constraints/plugin-registry-spec.md)
> **注册流程见** [`source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md` §3](../../source/skills/cross-cutting/ae-sdd-plugin-loader-skill.md)