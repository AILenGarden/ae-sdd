# 参考文献 SKILL 能力对比分析 — java3d-coding-skill 增强候选清单

**起草人：** Claude Opus 4.8（分析 agent）
**日期：** 2026-07-02
**分析范围：** `references/github-skills/` 下 14 个仓库（约 300 个 SKILL/规则文件，覆盖 Spring Boot 官方生态技能、DDD 方法论教练、经典架构书籍蒸馏规则、单测专项技能矩阵等）与 [`plugins/java3d-coding-skill/SKILL.md`](../plugins/java3d-coding-skill/SKILL.md)（v1.3.0）逐项比对
**判定基准：** 同时核对了 life 项目实际 constraints 九件套（`D:\Item\life\document\life-team-ai-standards\constraints\`）与共有 [`coding-skill.md`](../source/skills/phase2-coding/coding-skill.md)，避免把"已覆盖内容"或"与现有选型冲突的内容"误判为增强项
**结论性质：** 本文档是**分析建议清单**，不直接修改 `java3d-coding-skill.md`。按 ae-sdd Plan-first 原则，需用户确认采纳范围后再落地编辑。

---

## 0. 一句话结论

参考文献中约 70% 的内容因"Java8 不支持的语法特性""与 icec 已选型架构冲突""已被现有文档覆盖""通用常识无具体判定线"被排除；剩下约 12 项具备真实增强价值，其中**聚合边界判定线、messagebus 事件设计规范、`@Transactional` 传播陷阱、静态扫描规则补充**四项价值最高，可直接充实到适配器现有章节；**Context Map/子域划分方法论**价值也很高但超出编码决策层定位，建议单独归档而非塞进本适配器。

---

## 1. 排除标准（先说清楚"为什么没采纳"）

对比时系统性排除了以下五类内容，避免文档看起来"漏analyze"：

| 排除类别 | 具体例子 | 排除理由 |
|---------|---------|---------|
| **Java8/SpringBoot1.5.7 不支持的语法特性** | `record`/`sealed interface`/虚拟线程/`var`/exhaustive switch pattern matching、`@MockitoBean`（Boot 3.4+）、JUnit5 `@ParameterizedTest`、Spring Security 6.x API | icec 技术栈锁定 Java8 + Spring Boot 1.5.7 + JUnit4.12，这些特性编译不过或运行时不存在 |
| **与 icec 已选型架构哲学冲突** | developer-kit-java 建议"先按业务模块再按层分包"（领域优先）；RFC 9457 ProblemDetail 错误响应；OAuth2 Resource Server 标准鉴权 | icec 明确选型"技术分层优先"（5 平级 Maven 模块）+ `code+message` 响应体（HTTP 统一 200）+ 自建 JWT+Cookie 鉴权，属既定选型不建议动摇 |
| **已被现有文档覆盖** | Maven 依赖方向表（domain 不依赖任何模块）、DDD 4 层 object-per-layer 约束、防腐层 Facade 位置（domain 定义/infrastructure 实现）、构造器注入 | java3d §2/§3 或共有 coding-skill 已有等价或更细的规则 |
| **通用常识缺具体判定线** | "单一职责就是一个类只做一件事""要做好异常处理" | 没有可操作的量化标准，不构成"增强"，只是重复正确但空洞的话 |
| **工具链不匹配无法直接套用** | Spring Data JPA 的 `@EntityGraph`、Resilience4j 具体参数名、Flyway 迁移文件语法 | icec 用 MyBatis-Plus 非 JPA、Hystrix 非 Resilience4j、无 Flyway；具体 API 不能直接搬，只有"原理"可能有转移价值（已在下文单独甄别） |

---

## 2. 🔴 高价值候选（建议直接采纳，充实到适配器现有章节）

### 候选 1：聚合边界判定的可操作规则（补强 §2 DDD 落点表）

**现状空白：** java3d §2.2 只给了包路径落点，没有回答"什么时候该拆聚合""方法能不能传别的聚合对象"这类边界判定问题。

**来源与内容：**
- `ddd-architecture-coach/phase3-implementation-spec.md` AB-1~AB-4：Invariant 描述文本只能引用本聚合自己的字段/方法名（"An Aggregate must never query another Aggregate's internal state"）；聚合方法参数不能传其他聚合对象，只能传 ID（在 Handler 层分别加载）；允许"故意违反单聚合单事务"但必须显式 callout 理由+并发风险
- `spring-boot-skills/domain-driven-design`：子实体超过 3-4 个就应该考虑拆分聚合的具体数字启发式
- 经典书籍蒸馏（DDD 蓝皮书 + IDDD）交叉印证：判定依据统一是"必须立即成立的不变量"而非对象图的自然连接关系或数量

**建议落点：** 新增 §2.6「聚合边界判定线」，紧跟 §2.4（DO/PO 判定线）之后，同样是"决策知识"定位（不是纯规则）。

---

### 候选 2：messagebus 事件设计规范（补强 §4 骨架展开 + §8 踩坑库）

**现状空白：** java3d §1.1 只说"底层 Kafka，上层封装 messagebus"，§4 的"发送 MQ"展开规则只有"先写库后发消息"一句话，没有事件包字段规范、幂等消费策略选择、Outbox 落地方式。

**来源与内容：**
- `claude-skills/kafka-event-patterns`：事件包强制字段 `eventId`(UUID)/`eventType`/`schemaVersion`/`occurredAt`；Topic 命名 `<domain>.<entity>.<event>` 小写点分过去式；幂等消费两种策略——①去重表（`ProcessedEvent(event_id unique)` 与业务写入同事务，冲突即 no-op）②自然幂等（按聚合 ID upsert 或状态守卫忽略重放），二选一的判定线是"只要涉及触发邮件/支付/下游事件等**不可重复的副作用**，必须换成去重表，不能只依赖自然幂等"
- `developer-kit-java/spring-boot-event-driven-patterns`：`DomainEvent` 字段补充 `correlationId`（跨服务追踪）
- `ddd-architecture-coach/phase3` §5.3：领域事件 3-Phase 调度契约——Phase1 收集（持久化提交前，同事务）→ Phase2 历史持久化（同事务内原子写 event-log 表）→ Phase3 通知分发（提交后独立事务，订阅者失败不回滚原写入）

**⚠️ 需先核实一点：** icec 的 messagebus 是自建封装，内部是否已经处理了去重/重试/Outbox，文档层面看不出来。**建议行动是"找 messagebus 组件源码或维护者确认"，而不是直接假定业务代码需要自己实现这些**——如果 messagebus 内部已覆盖，这条只需要在文档里说明"已由 messagebus 内置保障"；如果没覆盖，这是真实风险点，应作为新踩坑写入 §8。

**建议落点：** §4 骨架展开规则"发送 MQ"行细化 + §8 踩坑库新增一条（视核实结果调整措辞）。

---

### 候选 3：`@Transactional` 传播机制 / 自调用陷阱 / AFTER_COMMIT 边界（补强 §8 踩坑库）

**现状空白：** icec `code-style.md` §九已有"事务方法中禁止包含远程调用"，但没有传播机制选择、自调用绕过代理的坑、`@TransactionalEventListener` 的执行边界。这批内容**与 Spring Boot 版本无关**，1.5.7 同样适用，是本次比对里"可信度最高、迁移风险最低"的一批。

**来源与内容：**
- `spring-boot-skills/transactional-patterns`：自调用（Self-Invocation）陷阱——`this.processSingle()` 绕过 Spring AOP 代理，`@Transactional(REQUIRES_NEW)` 被**静默忽略**（不报错，事务语义直接失效），修复方式是注入自身代理或把方法抽到独立 bean
- 同上：checked 异常不会自动回滚——`@Transactional` 默认只在 `RuntimeException` 时回滚，checked 异常必须显式 `rollbackFor`
- 同上：`@TransactionalEventListener(phase = AFTER_COMMIT)` 运行在事务提交**之后**但在原事务**之外**——如果 listener 自己要写库，必须新开 `@Transactional(REQUIRES_NEW)`，不能假设还在原事务里（icec 现有"事务提交后 `@TransactionalEventListener` 发 messagebus 事件"这条规则正好会撞到这个边界，值得补一句说明）

**建议落点：** §8 踩坑库新增 1-2 条（自调用陷阱、AFTER_COMMIT 边界），或在 §4 骨架展开"消息发送"行加一句边界说明。

---

### 候选 4：静态扫描规则补充（补强 §7）

**现状空白：** java3d §7 已有 8 条 grep 规则，但没有"扫描 Domain 层是否被框架污染"这条——这恰好是防止"AI 在 domain 层塞 `@Service`/`@Autowired`"的直接检测手段。

**来源：** `developer-kit-java/clean-architecture`

```bash
# 9. Domain 层禁框架注解污染（防 AI 把 Spring 语法写进领域层）
grep -rn "@Service\|@Component\|@Autowired\|@Repository" \
  --include="*.java" src/main/java/**/domain/ 2>/dev/null
# 期望空；非空 = 领域层被框架污染（🔴 阻断）
```

**建议落点：** §7 追加为第 9 条 grep 规则。

---

## 3. 🟡 中价值候选（有参考价值，但需要先判断落点是否真的是 java3d 适配器）

这批内容大多触发了 java3d 自己声明的 DRY 红线——**"本文件只承载决策知识层，不复述项目 constraints 的纯规则"**。下表逐条标注该落到 java3d 还是别处：

| 候选内容 | 来源 | 性质判定 | 建议落点 |
|---------|------|---------|---------|
| 分页 size 上限（当前 `PageRequest.size` 只有"不能为空"，无上限，裸传 `size=100000` 可拖垮整表） | `rest-api-conventions` | 纯规则（具体数值） | 建议给 life `api.md`，**非** java3d 职责范围；但这是一个真实的、可预见的性能风险点，值得单独提醒用户核实 |
| URL 嵌套最多 2 层 | `rest-api-conventions` | 纯规则 | 建议给 life `api.md` |
| 测试方法命名格式（`methodName_condition_expectedBehavior`） | developer-kit-java + opencode-skills/java-junit + 多个来源交叉印证 | 纯规则 | 建议给 life `testing.md`（现有"禁止无意义命名"过于模糊） |
| Redis Key 命名模板 `{app}:{domain}:{id}` + 缓存雪崩三层防护（`sync=true` + 分布式锁 + TTL jitter） | `spring-data-redis` | 纯规则+决策知识混合 | life 现有 constraints 九件套完全没有 Redis 专项规范，属真实空白；但**先核实项目里是否已有事实上的 key 命名约定**，避免文档规范和实际代码打架 |
| 测试质量红线 R1-R9（禁 `assertTrue(true)`、禁 mock 被测对象本身、禁 `Thread.sleep`、测试文件超 25 个方法说明 SUT 该拆了） | `java-ut-coverage-loop` | 决策知识（判定标准） | 这批质量很高，但主题是"测试真实性"，更适合给 `test-review-skill.md`（已有"假修复识别规则"，是很好的互补）而非 java3d 适配器 |
| Maven 依赖治理（`nearest wins` 机制、`dependencyManagement` vs `dependencies` 反例、Enforcer `dependencyConvergence`） | `dependency-management` + `multi-module-maven` | 决策知识（踩坑规避） | 与 java3d §8 踩坑库 #1（根 pom 依赖管理）主题高度相关，可以用来**充实 #1 的具体检测手段**，价值中等偏高 |
| 容错超时预算一致性原则（`caller timeout > sum(downstream timeout × retries)` 视为 bug）、连接池经验公式（`cores × 2`） | `resilience-performance` | 决策知识（原则性，工具无关） | icec 用 Hystrix 非 Resilience4j，具体参数不能套，但这条"预算一致性"原则本身语言/工具无关，可以酌情写入 §8，但优先级低于候选 1-4 |
| Domain Port vs Application Port 边界（跨 BC Port 必须走 ACL，adapter 放本 BC 的 infrastructure 层） | `ddd-architecture-coach/phase3` §4 | 决策知识 | 与 icec 现有 Facade/EventPublisher 概念部分重叠，若要吸收需谨慎措辞避免引入新术语造成混乱；优先级中等 |

---

## 4. 🟢 高价值但不建议塞进本适配器（超出定位，需要另开归档）

### Context Map / 子域划分 / Touchpoint Map 方法论

**来源：** `ddd-architecture-coach/phase1-domain-discovery.md` + `phase2-architecture-design.md`

**为什么价值高：** 这是本次调研里信息量最大的部分，提供了 java3d 完全没有覆盖的"更上游"决策——BC（Bounded Context）怎么划、什么时候该建新服务、跨 BC 集成模式怎么选（Shared Kernel / Customer-Supplier / ACL / OHS）。其中 **Touchpoint Map** 概念（识别"共同在场的观察者"，如客服工单场景里 supervisor 监看客户对话、AI 座席console 镜像对话）是标准 DDD 工具箱（Domain Storytelling/Event Storming）都没覆盖的空白，对 life 的 cs/im 工单客服域可能有直接参考价值。

**为什么不建议塞进 java3d：** java3d-coding-skill 的定位是"编码决策知识层"，叠加在共有 coding-skill 之上，服务的是**已经确定要写哪个 Service/BFF 之后**的落点决策；而 Context Map/子域划分是**服务边界还没确定之前**的架构设计决策，层级不同。塞进去会破坏适配器的内容边界（DRY 红线）。

**建议：** 如果需要引入，应该：
1. 归档为 `references/` 下的独立摘记（已有 `references/README.md` 的收录机制），或
2. 提给 `dr-review-skill.md`（DR 阶段本来就有"服务边界清晰"检查项 B1-4，但缺具体判定方法），作为该 SKILL 的方法论补充候选——这是另一个独立的采纳决策，不在本次 java3d 对比范围内

---

## 5. 明确不采纳且不需要进一步核实的类别

| 类别 | 代表来源 | 排除理由 |
|------|---------|---------|
| Spring AI / MCP Server / LangChain4j 相关全部技能 | `spring-ai-integration`/`mcp-server`/developer-kit-java 的 langchain4j 系列 | icec/life 不做 LLM 集成，零相关性 |
| ai-agent-skills-microservices-assistant 仓库（14 个技能） | 全仓库 | 提取阶段已确认内容浅、部分文件内容与目录名错位（如 `audit-trail/skill.md` 实际讲 Feign 客户端配置）、`distributed-tracing` 存在技术性自相矛盾（同时推荐已废弃的 Sleuth 和取代它的 Micrometer Tracing） |
| HATEOAS / OpenAPI-first / RFC 9457 / OAuth2 Resource Server | 对应同名 skill | 均与 icec 现有选型（`ApiResult<T>` 简单包装 / Swagger 手写注解 / code+message 错误体 / 自建 JWT+Cookie）方向不同，属另一套设计哲学，不构成"增强"，只是"另一种可能" |
| Spring Batch / Spring Data JPA / Flyway / Spring Data Neo4j 具体 API | 对应同名 skill | icec 分别用 job-spring-boot-starter / MyBatis-Plus / 无迁移工具 / 无图数据库，工具链不同导致具体 API 不可迁移；原理性内容已在候选表单独甄别 |
| SOLID 量化阈值（类 200 行/7 个 public 方法/接口方法数分级等） | `clean-code-skills` | 内容质量不差，但这是 Java 语言通用 OOP 设计常识的量化版，不是"icec/life 特有事实"，与 java3d 适配器"不复述通用规则"的定位冲突；如果要采纳更适合给共有 `coding-skill.md` §11 经验清单，不在本次比对范围内单独展开 |
| 项目结构"领域优先分包"建议 | developer-kit-java rules/project-structure.md | 与 icec 现有"技术分层优先"（5 平级模块）架构哲学直接冲突，采纳会动摇既定选型，不建议 |

---

## 6. 优先级执行建议

| 优先级 | 候选 | 建议动作 | 前置条件 |
|--------|------|---------|---------|
| P0 | 候选 4：静态扫描补充 | 直接加 1 条 grep 规则到 §7 | 无 |
| P0 | 候选 3：`@Transactional` 三个坑 | 直接加到 §8 踩坑库 | 无 |
| P1 | 候选 1：聚合边界判定线 AB-1~AB-4 | 新增 §2.6 | 无，纯知识补充不影响现有内容 |
| P1 | 候选 2：messagebus 事件设计规范 | 先核实 messagebus 内部是否已覆盖去重/Outbox，再定稿措辞 | **需要一次代码/文档核实** |
| P2 | Maven 依赖治理充实 §8 踩坑 #1 | 补充检测手段说明 | 无 |
| P2 | Redis Key 命名 + 雪崩防护 | 先核实项目现状 | **需要核实实际代码是否已有约定** |
| P3 | 容错原则性认知、Domain/Application Port 边界 | 视精力酌情补充 | 无，优先级最低 |
| 另议 | Context Map/Touchpoint Map | 不进 java3d，提给 dr-review-skill 或单独归档 references | 需要用户另外决策是否启动 |
| 另议 | 测试质量红线 R1-R9、测试命名规则 | 不进 java3d，提给 test-review-skill.md / life testing.md | 需要用户另外决策 |

---

## 7. 附：参考来源清单（本次实际读取并提取的仓库/文件）

- `spring-boot-skills`（19 个官方风格 SKILL，128★）
- `claude-skills`（9 个 SKILL：designing-systems / tdd-java / spring-boot-standards / oop-design / dependency-management / reviewing-java-code / kafka-event-patterns / resilience-performance / jpa-database-patterns）
- `ai-agent-skills-microservices-assistant`（14 个 SKILL，已确认无实质增量）
- `springboot-skills-marketplace`（4 个 SKILL）
- `agent-skills/java-spring-best-practices`（1 SKILL + 6 份 references）
- `opencode-skills`（5 个相关 SKILL：java-junit / java-springboot / java-springboot-testing / database-design / api-design）
- `java-ut-coverage-loop`（1 个高密度单测覆盖率驱动 SKILL）
- `clean-code-skills`（10 个 SOLID/TDD/重构 SKILL，跳过 fp-* 函数式编程系列）
- `developer-kit`（`developer-kit-java` plugin：4 份 rules + 15 个 Spring Boot 主题 SKILL + 18 个 unit-test-* 专项矩阵）
- `ddd-architecture-coach`（4 阶段 DDD 方法论：phase1 领域发现 / phase2 架构设计 / phase3 实现规范 / phase4 评审迭代）
- `moai-adk`（3 个相关 SKILL：moai-workflow-ddd / moai-domain-backend / moai-domain-database / moai-ref-api-patterns，多数判定无实质交集已跳过）
- `agent-rules-books`（4 本经典书籍蒸馏规则：DDD 蓝皮书 / DDD Distilled / Implementing DDD / POEAA）

**未纳入分析：** `modu-ai/moai-adk` 主体之外的 `Nubase`（AI-native 后端框架，非 SKILL 集合）、`agent-rules-books` 中与 DDD/架构无关的其余 6 本书（Clean Code/Clean Architecture/Refactoring 等，已在候选表通过 clean-code-skills 侧面覆盖）。

---

*本文档基于 2026-07-02 对 `references/github-skills/` 的全量读取与 6 个并行子 agent 提取结果交叉核对完成。如后续参考文献目录内容更新，需重新执行本次比对流程。*
