# 需求规格说明书：示例——适用性与条件章节不一致

## 0. 文档与需求身份
| 字段 | 值 |
| --- | --- |
| Schema | ae-sdd-ra-srs/v2 |
| RA ID | RA-NEG-APPL-001 |
| Work Item | WI-NEG-APPL-001 |
| Revision | 1 |
| Analysis state | complete |
| Scale | medium |
| Scale confidence | 55 |

### 0.1 来源与实际使用的上下文
| REF ID | 类型 | 引用/摘要 | Digest/版本 | 用途 |
| --- | --- | --- | --- | --- |
| REF-001 | 对话 | 用户希望新增一个有状态的功能 | chat-002 | 输入 |

## 1. 问题、目标与非目标
反例：§3 声明 `state_lifecycle` applicable 但全文没有 §8.3 章节；`participants` 标 unknown 且为阻断性，却没有对应 GAP-*。适用性未闭合。

## 2. 范围
- In Scope：新增一个有状态的业务能力。
- Out of Scope：未定。

## 3. 适用性判定
| 条件维度 | 状态 | 依据 | 目标章节/处置 |
| --- | --- | --- | --- |
| participants | unknown | 参与方尚未确认，影响范围与验收 | 应创建阻断性 GAP-* |
| scenarios | applicable | 主流程与异常场景 | §8.2 |
| state_lifecycle | applicable | 有状态生命周期 | §8.3 |
| data_semantics | not_applicable | 暂无数据语义变更 | §3 留判定 |
| external_contracts | not_applicable | 无外部契约 | §3 留判定 |
| quality_security_compliance | not_applicable | 无 | §3 留判定 |
| compatibility_migration_operations | not_applicable | 无 | §3 留判定 |

## 4. 需求清单
| REQ ID | 规范性需求 | 优先级 | Source refs | 依赖/冲突 |
| --- | --- | --- | --- | --- |
| REQ-001 | 须提供新的有状态能力 | P0 | REF-001 | 无 |

## 5. 验收与追溯
| AC ID | 覆盖 REQ | 验收类型 | 可执行/可观察判定 |
| --- | --- | --- | --- |
| AC-001 | REQ-001 | operational | 能力可用且状态可见 |

## 6. 约束、假设、冲突、风险与未决
| ID | 类型 | 内容 | 严重度 | 状态/处置 |
| --- | --- | --- | --- | --- |
| A-001 | 假设 | 状态存储可复用既有设施 | 中 | 待确认 |

## 8.2 场景与交互
主流程：用户触发后状态从初始态进入活动态；异常：并发触发需判定顺序。

## 7. 规模裁定
| 需求维度 | 评分 1-4 | 证据 |
| --- | --- | --- |
| 可观察行为与场景广度 | 3 | 主流程与异常 |
| 参与方、权限或业务域广度 | 2 | 参与方待定 |
| 状态、数据语义与不变量复杂度 | 3 | 有状态生命周期 |
| 外部契约与协调范围 | 1 | 无外部契约 |
| 性能、安全、合规、可用性等质量风险 | 1 | 无 |
| 兼容、迁移、回滚和运行影响 | 1 | 无 |

最高分 = 3 -> Scale = medium。
