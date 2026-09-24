# Managed ae-sdd entry guidance

Only the marked region is installed. General ALSDD, Git and user rules remain user-owned.

<!-- ae-sdd:managed:start id="ae-sdd-entry" version="1" -->
## 项目工程入口：ae-sdd

- 项目工程任务统一使用 `ae-sdd`，先读取其入口 SKILL，由它按任务选择需求、规格、编码、知识及数据库能力；用户无需单独调用内部能力。
- 项目理解、业务语义、设计依据、实现惯例、知识维护及工程追溯由 ae-sdd 的内置知识能力处理。已有 `.al-knowledge/` 时按需读取并回源核验，不默认加载全库。
- 通过宿主提供的 ae-sdd 入口定位插件；知识能力从该入口的 `capabilities/al-knowledge/CAPABILITY.md` 加载，不要求独立安装或调用 al-knowledge。源码开发时按当前环境明确登记的 registry.yaml 定位 ae-sdd。
- 缺库时直接回源，不强制建库；查询只读。代码或规格变化影响已有知识时，处理当前授权范围内的知识更新，其他项报告待更新；不自动扩大项目或安装范围。
<!-- ae-sdd:managed:end id="ae-sdd-entry" -->
