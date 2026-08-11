# 需求规格说明书：示例——规模裁定与评分不一致

## 0. 文档与需求身份
| 字段 | 值 |
| --- | --- |
| Schema | ae-sdd-ra-srs/v2 |
| RA ID | RA-NEG-SCALE-001 |
| Work Item | WI-NEG-SCALE-001 |
| Revision | 1 |
| Analysis state | complete |
| Scale | micro |
| Scale confidence | 30 |

### 0.1 来源与实际使用的上下文
| REF ID | 类型 | 引用/摘要 | Digest/版本 | 用途 |
| --- | --- | --- | --- | --- |
| REF-001 | 对话 | 用户希望做一个跨多域的能力 | chat-005 | 输入 |

## 1. 问题、目标与非目标
反例：§0 声明 Scale=micro，但 §7 六维评分最高为 4（large），裁定与证据不一致。规模须由评分取最高分得出，不得人为压低。

## 2. 范围
- In Scope：跨多域能力。
- Out of Scope：未定。

## 3. 适用性判定
| 条件维度 | 状态 | 依据 | 目标章节/处置 |
| --- | --- | --- | --- |
| participants | applicable | 多参与方 | §8.1 |
| scenarios | applicable | 多场景 | §8.2 |
| state_lifecycle | applicable | 复杂状态机 | §8.3 |
| data_semantics | applicable | 复杂数据语义 | §8.4 |
| external_contracts | applicable | 多方契约 | §8.5 |
| quality_security_compliance | applicable | 合规需求 | §8.6 |
| compatibility_migration_operations | applicable | 迁移影响 | §8.7 |

## 4. 需求清单
| REQ ID | 规范性需求 | 优先级 | Source refs | 依赖/冲突 |
| --- | --- | --- | --- | --- |
| REQ-001 | 须提供跨多域的能力 | P0 | REF-001 | 无 |

## 5. 验收与追溯
| AC ID | 覆盖 REQ | 验收类型 | 可执行/可观察判定 |
| --- | --- | --- | --- |
| AC-001 | REQ-001 | operational | 能力可用且各域协调一致 |

## 6. 约束、假设、冲突、风险与未决
| ID | 类型 | 内容 | 严重度 | 状态/处置 |
| --- | --- | --- | --- | --- |
| A-001 | 假设 | 各域可协调 | 中 | 待确认 |

## 7. 规模裁定
| 需求维度 | 评分 1-4 | 证据 |
| --- | --- | --- |
| 可观察行为与场景广度 | 4 | 多场景 |
| 参与方、权限或业务域广度 | 4 | 多参与方 |
| 状态、数据语义与不变量复杂度 | 4 | 复杂状态机 |
| 外部契约与协调范围 | 4 | 多方契约 |
| 性能、安全、合规、可用性等质量风险 | 3 | 合规需求 |
| 兼容、迁移、回滚和运行影响 | 4 | 迁移影响 |

最高分 = 4 -> Scale = large（与 §0 声明的 micro 不一致）。
