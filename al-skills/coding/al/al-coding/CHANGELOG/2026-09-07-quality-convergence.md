# ALCoding 变更历史

## Quality convergence（0.5.0）

- 入口收敛为 Coding design、Code review、Skill discovery 三个能力入口。
- coding-core 压缩为设计主干，并将设计思想与风险机制移入按需 references。
- review-core 改为按变更影响选择评审维度，区分确定缺陷、待验证风险和验证限制。
- framework 契约合并到 `plugin-spec.md`，删除重复的 loading-contract/domain-rules 文档引用。
- 修正适配器中的失效 framework 引用及旧 Review 术语；插件展示元数据移除已删除的 Local UI。
- Review 默认改为严格验证：所有适用规范逐条检查，关键证据缺失时输出不通过或无法判定。
