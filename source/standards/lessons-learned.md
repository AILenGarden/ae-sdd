# ae-sdd 历史教训沉淀库

> **定位（2026-06-30 建立）：** 本文件收纳从 `coding-skill.md` / `code-review-skill.md` 等 SKILL 正文抽离的**项目特定经验**与**历史复盘案例**，遵循 ae-sdd-update-skill §SKILL 边界判定表的"项目特定经验下沉项目资产，不进 SKILL 正文"原则。
>
> **为什么单独成文：** 这些案例是"为什么有这条规则"的背书，对理解规则来由有价值，但塞进 SKILL 主 prompt 会违反"通用规则才进 SKILL"原则、稀释模型对核心规则的注意力。本文件作为**可查阅的附录**，SKILL 正文保留通用规则 + 指向本文件的指针。
>
> **使用方式：** 维护者复盘新项目踩坑时，按章节追加到本文件对应小节；SKILL 正文不再新增项目特定案例。

---

## 1. STORY-002 反复改 5 轮的病根（一致性闸 / 禁裸✅ / 全文档回扫的来由）

**来源：** d--Item-document 项目（coding-skill/code-review-skill 多处引用）

### 1.1 一致性漂移积累

STORY-002 历轮 CodeReview 都只核了当轮新增范围就标"三方一致 ✅"，实为虚报——A 类"文档落后于代码"债持续累积。改主流程不联动关联章节又产生新矛盾（如 r12 改双边定位但异常表/索引说明/错误码表未同步）。最终被下游 STORY-009 一致性核查揪出 12 项偏差。

**沉淀的规则：** coding-skill §第九步 ⑥bis 一致性闸 + code-review-skill §闸3 全文档回扫闸。

### 1.2 禁裸✅ 的来由

CodeReview 的"三方一致✅""契约一致✅"都是裸 ✅，没人要求附证据，于是历轮用"只核当轮 diff"假装通过，债越积越多。一个只需要"宣称"就能通过的检查项，在压力下必然被宣称通过。

**沉淀的规则：** code-review-skill §闸4 禁裸✅ 闸。

### 1.3 落库漏 NOT NULL 字段被 mock 掩盖

STORY-002 落库漏 NOT NULL 字段是被 mock 测试掩盖的——mock 的 Repository.save() 不会触发 DB 约束检查。后续真实运行才暴露 → 必须返工。

**沉淀的规则：** test-review-skill 的真实 DB/HTTP/Redis 证据链复核；code-review-skill §闸7 仅做 Test Review 引用核查。

---

## 2. STORY-021-BE 分层错误漏检（一致性闸触发时机的来由）

**来源：** d--Item-document 项目

STORY-021-BE 实施时 AI 只跑了编译 + 测试，**没跑**一致性闸，直接出 Coding Report——结果"在 `ImSessionAppService` 写了纯数据访问封装"的分层错误漏到用户反馈才发现。

**沉淀的规则：** 一致性闸的触发时机不是"出 CodeReview 才跑"，而是"代码写完立即跑"——出 CodeReview 之前应该已经跑过本闸并修复完毕。

**通用规则（coding-skill 保留）：** 每一轮 Coding（含缺陷修复轮）完成后立即强制执行一致性闸，不可因"只改了一点"而跳过。

---

## 3. life 项目 STORY-020 CodePlan 源码核对复盘（G-CODEPLAN-SRC 门禁的来由）

**来源：** icec-cloud-life 项目 STORY-020

### 3.1 错把 Converter 当改动的目标

| # | Plan 中的错误 | 源码实际事实 | 危害 |
|---|---|---|---|
| 1 | 把 application 层 `ImMessageConverter` 当成要改的 | 实际要改的是 infrastructure 层 `LatestSideMessagePOConverter` | 改错文件 |
| 2 | 说"新增 Converter 映射" | `ImMessageConverter.toDTO` 已存在 | 重复造轮子（红线 #10）|
| 3 | 设计嵌套 Anchor 值对象 | 现有 PO/DO 全是扁平字段 | 与现有建模范式不符 |
| 4 | 测试范式标"JUnit4/5 待确认" | 代码里就是 JUnit4 + SpringRunner + H2 | 本可读源码确认却标待确认 |
| 5 | Converter 写法按 AGENTS.md 写 `@UtilityClass` | 实际代码用 `@NoArgsConstructor(PRIVATE)`+static | AGENTS.md 与实际有出入，应以代码为准 |

**沉淀的规则：** G-CODEPLAN-SRC 门禁（CodingPlan 关键类骨架每个新增/修改类必须附【已读源码：】标记）。

**通用教训（适用所有项目）：** AGENTS.md / 项目资产描述与实际代码有出入时，**以代码为准**——文档可能滞后。

### 3.2 待核实清单示例（项目特定填充模板）

```markdown
### 待核实源码清单（G-CODEPLAN-SRC）
- [ ] ImMessageConverter 现有 toDTO 方法签名（domain/.../ImMessageConverter.java）
- [ ] LatestSideMessagePO 扁平/嵌套范式（infrastructure/.../LatestSideMessagePO.java）
- [ ] 测试框架版本（src/test/java/.../现有测试类）
```

---

## 4. icec 项目特定工程经验（经验检查清单项目特定项）

> **说明：** 以下来自 coding-skill §工程预检经验检查清单。通用检查项（pom 依赖/lombok/Result.code 类型等）保留在 SKILL；项目特定项（含具体包名/版本）下沉到本节，新项目应在项目资产 §6 工程约束中维护各自的版本与包路径。

| # | 检查项 | 项目特定事实（icec） | 通用检查动作 |
|---|--------|---------------------|------------|
| 3 | 第三方 SDK 实际包路径 | 融云是 `io.rong` 不是 `cn.rongcloud.im` | 从 jar 中确认，不要凭记忆猜测 |
| 4 | @NotBlank 来源包 | Spring Boot 1.5.x + hibernate-validator 5.x 用 `org.hibernate.validator.constraints.NotBlank` | 按 Spring Boot 版本确认注解来源包 |
| 7 | ApiResult 完整 import | 不同工程的 ApiResult 包路径不同（life vs boss） | 从工程现有代码 grep 确认包路径 |
| 10 | Feign 注解版本 | Spring Cloud Dalston 用 `org.springframework.cloud.netflix.feign.FeignClient` | 按 Spring Cloud 版本确认 FeignClient 包 |
| 11 | CurrentUserUtil 返回 String | 不要做 Long.valueOf() 转换（除非确认下游需要 Long） | 确认工具类返回类型，避免盲目转换 |

---

## 5. 包路径映射示例（④bis 实战 SOP 步骤4 示例）

**来源：** icec-cloud-boss 项目（coding-skill §④bis 步骤4 历史示例）

Application 层的 `BossUserAppService` → `icec-cloud-boss-user/icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/appservice/BossUserAppService.java`

**通用规则（coding-skill 保留）：** 步骤4 调用 `ae-sdd assets section §4 --project <projectKey>` 匹配每个类对应的精确包路径，禁止写"包路径待定/TBD/按项目惯例"。具体包路径模板由项目资产 §4 提供。

---

## 6. 交付表示例（闸4 编码后交付表的项目特定填充示例）

**来源：** icec-cloud-life-cs 项目（coding-skill §闸4 历史示例）

```markdown
### SPI 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| SPI 接口 | `icec-cloud-life-spi/icec-cloud-life-im-spi/src/main/java/.../ImSessionService.java` | 修改 | 新增 `getLatestMessageAt` / `batchGetLatestMessageAt` 方法签名 |

### Domain 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| Facade 接口 | `icec-cloud-life-cs/icec-cloud-life-cs-domain/src/main/java/.../ImSessionServiceFacade.java` | 修改 | CS 防腐层新增 2 个方法 |
| Repository 接口 | `icec-cloud-life-cs/icec-cloud-life-cs-domain/src/main/java/.../CsTicketRepository.java` | 修改 | 新增 `syncLastMessageAtFromIm(Long ticketId, Date lastMessageAt)` |

### Application 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| Orchestrator | `icec-cloud-life-cs/icec-cloud-life-cs-application/src/main/java/.../CsTicketCloseOrchestrator.java` | 修改 | CloseContext 加 `lastMessageAt` 字段 |
```

**通用规则（coding-skill 保留）：** 交付表必填列 = 类型 + 文件路径 + 变更类型 + 说明，按项目分层架构调用顺序排列（SPI → Domain → Application → Infrastructure → Interfaces/BFF → Test → 文档/配置）。

---

## 维护说明

- 新项目踩坑复盘时，追加到对应小节（若无对应小节则新建）
- SKILL 正文不再新增项目特定案例；项目特定内容一律进本文件
- 通用规则的反向引用：SKILL 正文保留通用规则 + `lessons-learned.md` 指针，不重复项目细节
- 本文件作为**附录**，不进 ae-sdd 运行时主 prompt（维护者查阅用）
