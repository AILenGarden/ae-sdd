# 需求规格说明书：将日志级别默认值由 INFO 调整为 WARN

## 0. 文档与需求身份
| 字段 | 值 |
| --- | --- |
| Schema | ae-sdd-ra-srs/v2 |
| RA ID | RA-MICRO-CONFIG-001 |
| Work Item | WI-MICRO-CONFIG-001 |
| Revision | 1 |
| Analysis state | complete |
| Scale | micro |
| Scale confidence | 88 |

### 0.1 来源与实际使用的上下文
| REF ID | 类型 | 引用/摘要 | Digest/版本 | 用途 |
| --- | --- | --- | --- | --- |
| REF-001 | 运维工单 | OPS-77：生产 INFO 日志量过大，影响成本 | ticket-77 | 问题陈述 |
| REF-002 | 现有系统事实 | 默认日志级别配置项 `log.level` 当前为 INFO | config/default.yaml:L12 | 现状值 |

## 1. 问题、目标与非目标
当前默认日志级别为 INFO，生产环境产生大量低价值日志，日志存储成本偏高。

目标：将默认级别调整为 WARN，减少生产噪声与存储成本，保留显式覆盖能力。
非目标：不重写日志框架；不改日志格式；不调整各模块自定义级别覆盖。

## 2. 范围
- In Scope：默认 `log.level` 从 INFO 改为 WARN。
- Out of Scope：现有按模块覆盖的级别配置；日志采集与告警阈值。

## 3. 适用性判定
| 条件维度 | 状态 | 依据 | 目标章节/处置 |
| --- | --- | --- | --- |
| participants | not_applicable | 无新参与方 | §3 留判定 |
| scenarios | not_applicable | 行为不变，仅默认值变更 | §3 留判定 |
| state_lifecycle | not_applicable | 无状态变更 | §3 留判定 |
| data_semantics | not_applicable | 无数据语义变更 | §3 留判定 |
| external_contracts | not_applicable | 无对外契约变更 | §3 留判定 |
| quality_security_compliance | applicable | 可观测性：默认级别下调后须保留排查能力 | §8.6 |
| compatibility_migration_operations | not_applicable | 配置项仍存在，仅取值变化 | §3 留判定 |

## 4. 需求清单
| REQ ID | 规范性需求 | 优先级 | Source refs | 依赖/冲突 |
| --- | --- | --- | --- | --- |
| REQ-001 | 默认 `log.level` 须为 WARN | P0 | REF-001, REF-002 | 无 |
| REQ-002 | 须保留按部署环境显式覆盖级别的机制 | P0 | REF-002 | 无 |

## 5. 验收与追溯
| AC ID | 覆盖 REQ | 验收类型 | 可执行/可观察判定 |
| --- | --- | --- | --- |
| AC-001 | REQ-001 | operational | 全新部署且未设置覆盖时，默认级别为 WARN |
| AC-002 | REQ-002 | operational | 设置 `log.level=DEBUG` 覆盖时，运行时级别为 DEBUG |

## 6. 约束、假设、冲突、风险与未决
| ID | 类型 | 内容 | 严重度 | 状态/处置 |
| --- | --- | --- | --- | --- |
| A-001 | 假设 | 关键路径已有结构化错误日志，不依赖 INFO | 中 | 由 §8.6 排查能力验证确认 |
| R-001 | 风险 | 下调后线上排障信息变少 | 中 | 通过保留显式覆盖（REQ-002）缓解 |

## 7. 规模裁定
| 需求维度 | 评分 1-4 | 证据 |
| --- | --- | --- |
| 可观察行为与场景广度 | 1 | 仅默认取值变更 |
| 参与方、权限或业务域广度 | 1 | 无新参与方 |
| 状态、数据语义与不变量复杂度 | 1 | 无状态/数据语义变更 |
| 外部契约与协调范围 | 1 | 无对外契约变更 |
| 性能、安全、合规、可用性等质量风险 | 1 | 可观测性影响有限，且有覆盖机制 |
| 兼容、迁移、回滚和运行影响 | 1 | 配置取值变化，回滚即改回取值 |

最高分 = 1 -> Scale = micro。

## 8.6 质量属性、安全与合规
可观测性：默认级别下调为 WARN 后须保留关键错误排查能力（结构化错误日志、按需显式覆盖为 DEBUG）。下调不应掩盖阻断性故障。
