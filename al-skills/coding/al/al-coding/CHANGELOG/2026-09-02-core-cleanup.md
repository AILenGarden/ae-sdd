# ALCoding 变更历史

## Core 清理（0.4.1）

- 删除 `core/coding-thinking-process.md`，不再维护第二套 Coding 流程入口。
- Coding 基线统一使用 `core/coding-core.md`；该文件承载设计思想、设计输入、现状确认、代码设计成型和风险判定。
- 更新 `SKILL.md` 与 `framework/loading-contract.md` 的 Core 加载契约，消除对已删除流程文件的活动引用。
- 更新 DDD、MVC、Agent 和 Java/Spring 适配器的 Core 语义指针，改为引用 D1-D7、R01-R11 和设计完成判定。
- 更新反模式库与 `review-core.md` 的失效主文件/复盘引用。
- `dist/al-coding-0.3.x` 是历史发布物，本次不改写；下一次正式编译发布时再生成包含 0.4.1 的新分发包。
