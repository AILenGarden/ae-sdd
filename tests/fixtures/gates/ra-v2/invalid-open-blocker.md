# 需求规格说明书：示例——存在未关闭的阻断性 GAP

## 0. 文档与需求身份
| 字段 | 值 |
| --- | --- |
| Schema | ae-sdd-ra-srs/v2 |
| RA ID | RA-NEG-GAP-001 |
| Work Item | WI-NEG-GAP-001 |
| Revision | 1 |
| Analysis state | complete |
| Scale | small |
| Scale confidence | 40 |

### 0.1 来源与实际使用的上下文
| REF ID | 类型 | 引用/摘要 | Digest/版本 | 用途 |
| --- | --- | --- | --- | --- |
| REF-001 | 对话 | 用户希望新增一个导出功能 | chat-004 | 输入 |

## 1. 问题、目标与非目标
反例：§6 存在一个标记为阻断、状态为开放的 GAP-001，但 analysisState 却声明 complete，自相矛盾。阻断性 GAP 未关闭时 SRS 不得进入 RequirementAnalyzed。

## 2. 范围
- In Scope：新增导出能力。
- Out of Scope：未定。

## 3. 适用性判定
| 条件维度 | 状态 | 依据 | 目标章节/处置 |
| --- | --- | --- | --- |
| participants | not_applicable | 无新参与方 | §3 留判定 |
| scenarios | not_applicable | 无独立场景 | §3 留判定 |
| state_lifecycle | not_applicable | 无状态变更 | §3 留判定 |
| data_semantics | not_applicable | 无数据语义变更 | §3 留判定 |
| external_contracts | applicable | 导出格式契约 | §8.5 |
| quality_security_compliance | not_applicable | 无 | §3 留判定 |
| compatibility_migration_operations | not_applicable | 无 | §3 留判定 |

## 4. 需求清单
| REQ ID | 规范性需求 | 优先级 | Source refs | 依赖/冲突 |
| --- | --- | --- | --- | --- |
| REQ-001 | 须提供数据导出能力 | P0 | REF-001 | 无 |

## 5. 验收与追溯
| AC ID | 覆盖 REQ | 验收类型 | 可执行/可观察判定 |
| --- | --- | --- | --- |
| AC-001 | REQ-001 | operational | 导出产物可被消费方解析 |

## 6. 约束、假设、冲突、风险与未决
| ID | 类型 | 内容 | 严重度 | 状态/处置 |
| --- | --- | --- | --- | --- |
| GAP-001 | gap | 导出格式尚未与消费方确认，阻断验收 | 🔴 阻断 | 开放 |

## 8.5 外部行为契约与依赖
导出格式待与消费方确认（见 GAP-001）。

## 7. 规模裁定
| 需求维度 | 评分 1-4 | 证据 |
| --- | --- | --- |
| 可观察行为与场景广度 | 2 | 导出能力 |
| 参与方、权限或业务域广度 | 1 | 无新参与方 |
| 状态、数据语义与不变量复杂度 | 1 | 无 |
| 外部契约与协调范围 | 2 | 导出格式契约 |
| 性能、安全、合规、可用性等质量风险 | 1 | 无 |
| 兼容、迁移、回滚和运行影响 | 1 | 无 |

最高分 = 2 -> Scale = small。
