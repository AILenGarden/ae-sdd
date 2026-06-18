# 代码风格规范

## 摘要

本文件定义团队 Java 代码风格约定，包括注释、异常处理、日志、常量定义、ORM、事务、并发等规范。
适用场景：代码评审、新成员入职、AI 生成代码时。

---

## 一、注释规范

- 所有 public 方法必须有 Javadoc 注释
- 注释语言不强制，中英文均可，同一个方法内保持一致
- Javadoc 格式：
  ```java
  /**
   * 方法说明
   *
   * @param paramName 参数说明
   * @return 返回值说明
   */
  ```
- 禁止无意义注释（如 `// 获取用户` 紧跟 `getUser()`）
- 复杂业务逻辑、非显而易见的设计决策需要写行内注释说明原因

---

## 二、异常处理

- 禁止直接抛 `RuntimeException`，必须使用自定义异常类
- 自定义异常类继承 `RuntimeException`，包含错误码字段
- 异常类按领域划分，定义在 domain 层的 `exception` 包下
- 示例：
  ```java
  public class BossDomainException extends RuntimeException {
      private final String code;
      public BossDomainException(String code, String message) { ... }
  }
  ```

---

## 三、日志规范

- 统一使用 Lombok `@Slf4j` 注解，禁止手动声明 Logger
- 日志级别约定：
  - `log.error()`：异常场景，使用 `log.error("描述", e)` 打印完整堆栈，禁止同时用 `e.getMessage()` 和 `e` 导致重复输出
  - `log.warn()`：业务告警，不影响主流程但需要关注
  - `log.info()`：关键业务节点（如订单创建、状态变更）
  - `log.debug()`：调试信息，生产环境不输出
- 禁止在循环内打印 info/error 日志

---

## 四、常量定义

- 常量定义在 `class` 中，禁止使用 `interface` 定义常量
- 使用 `@NoArgsConstructor(access = AccessLevel.PRIVATE)` 防止实例化
- 常量命名：全大写 + 下划线分隔
- 相关常量组织在同一个类中，按业务域命名，如 `BossUserConstants`
- 示例：
  ```java
  @NoArgsConstructor(access = AccessLevel.PRIVATE)
  public class BossUserConstants {
      public static final String DEFAULT_ROLE = "OPERATOR";
      public static final Integer MAX_LOGIN_RETRY = 5;
  }
  ```

---

## 五、枚举定义规范

- 枚举统一使用 `key`（String）+ `value`（String，中文描述）双字段结构
- 类上加 `@Getter`，通过 Lombok 生成 getter
- 不需要定义 `MAP` 和 `getEnum` 静态方法，保持简洁
- 枚举值命名：全大写 + 下划线分隔
- 示例：
  ```java
  @Getter
  public enum PrincipalTypeEnum {

      USER("USER", "用户"),
      CS_AGENT("CS_AGENT", "客服坐席"),
      ;

      private final String key;

      private final String value;

      PrincipalTypeEnum(String key, String value) {
          this.key = key;
          this.value = value;
      }
  }
  ```

---

## 六、Lombok 使用规范

- `@Data`：用于 PO、DTO、Command、Query、DO 等数据对象
- `@Slf4j`：所有需要打日志的类统一使用
- `@RequiredArgsConstructor`：推荐用于 Service 类的依赖注入，替代 `@Autowired`（非强制）
- `@Builder`：用于需要链式构建的对象
- DO 上可以使用 `@Data`，但手动定义的业务方法不能以 `get` / `set` 开头，避免与 Lombok 生成的方法混淆
- 禁止使用 MapStruct 做对象转换，统一使用 Converter 类显式转换，详见 `project-structure.md`

---

## 七、其他约定

- 魔法值（Magic Number / Magic String）必须定义为常量，禁止直接使用字面量
- 禁止在循环中执行数据库操作（查询或写入），必须改为批量操作：先收集参数，再一次批量查询 / 批量插入

---

## 八、ORM 与事务规约

**ORM：**
- POJO 类的布尔属性不加 `is_` 前缀，数据库字段必须加，在 resultMap 中做映射
- 禁止用 HashMap / Hashtable 作为查询结果集输出
- 更新记录时必须同时更新 `last_updated_date`
- 不写大而全的更新接口，只更新有变动的字段

**事务：**
- 查询方法不允许开启事务
- 增删改方法必须开启事务，统一在 AppService 层使用 Spring `@Transactional` 注解，不在 interfaces 层开启
- 事务方法中禁止包含远程调用（Feign）、MQ 消息发送、Kafka 消息发送
- catch 异常后需要回滚时，必须手动调用回滚
- 区分本地事务和分布式事务，不混用

---

## 九、Feign 调用规范

- `@FeignClient` 直接继承 SPI 接口，服务名从 SPI 模块的 `ServiceProviderConstants` 常量取，禁止硬编码字符串
  ```java
  @FeignClient(ServiceProviderConstants.LIFE_IM_SERVICE)
  public interface ImServiceClient extends ImSessionService {
  }
  ```
- Feign Client 定义在调用方的 `infrastructure/feign/` 包下
- BFF 层禁止在 AppService 中直接调用 Feign Client，必须通过 Facade 层封装
  - Facade 负责调用 Feign Client、处理异常、解包 `Result<T>`
  - 异常时返回 null / 空集合 / `Result.error`，不向上抛出异常
  ```java
  @Component
  @Slf4j
  public class ImServiceClientFacade {
      @Autowired
      private ImServiceClient imServiceClient;

      public ImSessionDTO getSessionInfo(Long sessionId) {
          try {
              Result<ImSessionDTO> result = imServiceClient.getSessionInfo(sessionId);
              if (!Result.isOk(result)) {
                  log.warn("获取会话详情失败 sessionId:{} result:{}", sessionId, result);
                  return null;
              }
              return result.getData();
          } catch (Exception e) {
              log.error("获取会话详情异常 sessionId:{}", sessionId, e);
              return null;
          }
      }
  }
  ```

---

## 十、并发规约

- 禁止在应用中显式创建线程，线程资源必须通过线程池提供
- 禁止使用 `Executors` 创建线程池，必须通过 `ThreadPoolExecutor` 显式指定参数
  - `Executors.newFixedThreadPool` / `newSingleThreadExecutor`：队列长度为 `Integer.MAX_VALUE`，可能堆积大量请求导致 OOM
  - `Executors.newCachedThreadPool` / `newScheduledThreadPool`：允许创建线程数为 `Integer.MAX_VALUE`，可能创建大量线程导致 OOM
- 创建线程或线程池时必须指定有意义的线程名称，方便出错时回溯
  ```java
  public class TimerTaskThread extends Thread {
      public TimerTaskThread() {
          super.setName("TimerTaskThread");
      }
  }
  ```

---

## 十一、时间类型规范

- 全工程统一使用 `java.util.Date` 表示时间，禁止使用 `LocalDateTime`
- 包括：SPI（Request / DTO）、Domain（DO）、Infrastructure（PO）、Application 层
- 原因：Feign + JSON 跨服务传输时 `Date` 兼容性最好，统一类型避免各层之间反复转换

---

## 十二、JSON 序列化规范

- JSON 序列化 / 反序列化统一使用 `com.casstime.commons.utils.JsonUtils`
- 禁止在业务代码中自行创建 `ObjectMapper` 实例
- `JsonUtils` 已内置 `ObjectMapper` 单例，覆盖 `toJson`、`toBean`、`toList`、`toMap` 等常用场景
- 示例：
  ```java
  // ✅ 正例
  String json = JsonUtils.toJson(extraAttribute);
  CsTicketExtraAttribute attr = JsonUtils.toBean(json, CsTicketExtraAttribute.class);

  // ❌ 反例
  ObjectMapper om = new ObjectMapper();
  String json = om.writeValueAsString(extraAttribute);
  ```
