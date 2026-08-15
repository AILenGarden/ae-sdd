# 需求规格说明书：示例——需求不可追溯、不可验收

## 0. 文档与需求身份
| 字段 | 值 |
| --- | --- |
| Schema | ae-sdd-ra-srs/v2 |
| RA ID | RA-NEG-TRACE-001 |
| Work Item | WI-NEG-TRACE-001 |
| Revision | 1 |
| Analysis state | complete |
| Scale | small |
| Scale confidence | 50 |

### 0.1 来源与实际使用的上下文
| REF ID | 类型 | 引用/摘要 | Digest/版本 | 用途 |
| --- | --- | --- | --- | --- |
| REF-001 | 对话 | 用户希望优化某个流程 | chat-003 | 输入 |

## 1. 问题、目标与非目标
反例：REQ-001 无任何 Source refs（无来源追溯）；REQ-002 没有任何 AC 覆盖（不可验收）。

## 2. 范围
- In Scope：流程优化。
- Out of Scope：未定。

## 3. 适用性判定
| 条件维度 | 状态 | 依据 | 目标章节/处置 |
| --- | --- | --- | --- |
| participants | not_applicable | 无新参与方 | §3 留判定 |
| scenarios | not_applicable | 无独立场景 | §3 留判定 |
| state_lifecycle | not_applicable | 无状态变更 | §3 留判定 |
| data_semantics | not_applicable | 无数据语义变更 | §3 留判定 |
| external_contracts | not_applicable | 无外部契约 | §3 留判定 |
| quality_security_compliance | not_applicable | 无 | §3 留判定 |
| compatibility_migration_operations | not_applicable | 无 | §3 留判定 |

## 4. 需求清单
| REQ ID | 规范性需求 | 优先级 | Source refs | 依赖/冲突 |
| --- | --- | --- | --- | --- |
| REQ-001 | 须优化某流程 | P0 |  | 无 |
| REQ-002 | 须提升某指标 | P0 | REF-001 | 无 |

## 5. 验收与追溯
| AC ID | 覆盖 REQ | 验收类型 | 可执行/可观察判定 |
| --- | --- | --- | --- |
| AC-001 | REQ-001 | operational | 流程优化后可观察到改进 |

## 6. 约束、假设、冲突、风险与未决
| ID | 类型 | 内容 | 严重度 | 状态/处置 |
| --- | --- | --- | --- | --- |
| A-001 | 假设 | 优化不引入回归 | 中 | 待确认 |

## 7. 规模裁定
| 需求维度 | 评分 1-4 | 证据 |
| --- | --- | --- |
| 可观察行为与场景广度 | 2 | 流程优化 |
| 参与方、权限或业务域广度 | 1 | 无新参与方 |
| 状态、数据语义与不变量复杂度 | 1 | 无 |
| 外部契约与协调范围 | 1 | 无 |
| 性能、安全、合规、可用性等质量风险 | 1 | 无 |
| 兼容、迁移、回滚和运行影响 | 1 | 无 |

最高分 = 2 -> Scale = small。
