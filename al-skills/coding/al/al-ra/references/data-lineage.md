# 数据来源与流转分析

## 目标

明确关键数据的产生者、进入点、可信边界、权威来源、业务语义、所有者、去向和失败行为。数据分析服务于 RA 本身，不预设任何后续文档或实现流程。

## 最小闭合链路

```text
Producer → Entry Point → Trust Boundary → Validation → Transformation → Meaning → Authority/Owner → Destination → Side Effect
```

## 必查问题

- 谁产生或维护数据？
- 数据从哪里进入？
- 客户端是否可以提交或修改？
- 是否需要从认证上下文、数据库或远程系统重新取得？
- 哪个来源是权威来源？多个来源冲突如何处理？
- 类型、值域、必填性、格式、范围和关联约束是什么？
- 是否发生类型、枚举、单位、时区、精度或格式转换？
- 数据由谁拥有，生命周期、保留期和新鲜度是什么？
- 数据保存、返回、发送或触发了哪些副作用？
- 数据来源不可用、过期、重复或不一致时怎么办？
- 是否涉及敏感数据、权限、脱敏、审计或防篡改？

## 推荐表格

```markdown
## Data Lineage

| Field ID | Field | Producer | Entry Point | Client Writable | Trust Boundary | Authoritative Source | Owner |
|---|---|---|---|---|---|---|---|

| Field ID | Type/Value Domain | Requiredness | Validation | Transformation | Destination | Freshness/Consistency | Failure Behavior |
|---|---|---|---|---|---|---|---|
```

## 判断规则

- 客户端输入不等于权威数据；身份、租户、权限、状态、金额和归属必须验证服务端来源。
- 数据库、缓存、事件快照和远程查询同时存在时，必须标出权威来源及可接受的新鲜度。
- 未确认的来源或所有权写为 `Open Question`，不得猜测。
- 需求只要求展示而不要求持久化时，不要擅自增加存储结论。
- 只描述需求事实、业务语义和可验证约束；具体表名、类名、消息队列和技术组件属于设计内容，除非已有证据。
