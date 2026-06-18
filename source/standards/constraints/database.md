# DDL 约束

## 摘要

本文件定义数据库 Schema 和建表规范，包括库级约束、标准字段、命名规则、字段类型约定、索引规范、SQL 规约和 ORM 规约。建表前必须逐条核对。
适用场景：新建库、新建表、修改表结构、评审数据模型时。

---

## 零、Schema 约束

- 库名与应用服务名保持一致
- 统一使用 utf8mb4 字符集、utf8mb4_0900_ai_ci 排序规则（MySQL 8.0 默认，性能优于 utf8mb4_general_ci）
- 每个 Service 独占自己的数据库，禁止跨 Service 共用数据库
- 单表行数超过 500 万行或容量超过 2GB 才考虑分库分表；未达到此阈值禁止提前分库分表
- 特殊场景需要分库分表时，使用 ShardingSphere，并在 DR 中说明原因

---

## 一、标准建表模板

```sql
CREATE TABLE `业务名_表名` (
  `id`                <类型> NOT NULL COMMENT '主键ID',
  -- 业务字段 --
  `created_by`        varchar(64) DEFAULT NULL COMMENT '创建人',
  `created_date`      datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
  `last_updated_by`   varchar(64) DEFAULT NULL COMMENT '最后更新人',
  `last_updated_date` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '最后更新时间',
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='表注释';
```

**必备字段说明：**

| 字段 | 类型 | 约束 | 说明 |
| --- | --- | --- | --- |
| id | 视场景而定 | NOT NULL | 主键，类型见下方说明 |
| created_by | varchar(64) | DEFAULT NULL | 创建人 |
| created_date | datetime | NOT NULL DEFAULT CURRENT_TIMESTAMP | 创建时间，数据库自动填充 |
| last_updated_by | varchar(64) | DEFAULT NULL | 最后更新人 |
| last_updated_date | datetime | NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP | 最后更新时间，数据库自动更新 |

**主键类型选择：**
- 普通业务表：`bigint AUTO_INCREMENT`，自增，步长为 1
- 需要对用户友好的业务单号：`varchar(32)`，由业务层生成（如时间戳+随机数）
- 其他场景按实际需求决定，在 DR 中说明选择理由

**按需字段：**
- `deleted_flag tinyint(1) NOT NULL DEFAULT 0`：有逻辑删除需求时加，无需求时不加

> 注意：如果代码中对 `created_date` / `last_updated_date` 显式赋值，数据库自动填充会失效。建议统一由数据库自动维护，不在代码中赋值。

---

## 二、命名规则

- 表名、字段名使用小写字母或数字，下划线分隔，禁止驼峰命名
- 表名不使用复数，如 `user` 而非 `users`
- 表名推荐格式：`业务名_表的作用`，如 `abnormal_ticket`、`sms_template`
- 禁用 MySQL 保留字（desc、range、match、delayed 等）
- 表达是否概念的字段，命名格式为 `is_xxx`，类型为 `tinyint(1)`（1=是，0=否）
- 索引命名规范：
  - 主键索引：`pk_字段名`
  - 唯一索引：`uk_字段名`
  - 普通索引：`idx_字段名`

---

## 三、字段类型约定

| 场景 | 类型 | 说明 |
| --- | --- | --- |
| 主键 | bigint | 自增，非负 |
| 金额/小数 | decimal | 禁止使用 float / double，存在精度损失 |
| 定长字符串 | char | 长度几乎相等时使用 |
| 变长字符串 | varchar | 长度不超过 5000 |
| 长文本 | text | 超过 5000 时使用，建议独立成表 |
| 时间 | datetime | 统一使用 datetime，不使用 timestamp |
| 是否标识 | tinyint(1) | 1=是，0=否，命名加 `is_` 前缀；保留显示宽度 `(1)` 是为了兼容 MyBatis-Plus 的 Boolean 映射 |
| 逻辑删除 | tinyint(1) | `deleted_flag`，1=已删除，0=未删除；同上保留 `(1)` |
| 状态/类型枚举 | varchar 或 enum | 枚举值稳定（不频繁新增）且无维护表时用 enum；需要灵活扩展或频繁新增时用 varchar；新增枚举值必须追加到末尾 |
| JSON 数据 | json | 适用于结构不固定的扩展字段 |
| 字符集 | utf8mb4 | 统一使用 utf8mb4，支持 emoji |

---

## 四、索引规范

### 4.1 必建索引

- 业务上具有唯一特性的字段（含组合字段）必须建唯一索引
- `created_date`、`last_updated_date` 建普通索引（用于数据同步、分页查询）
- 高频查询条件字段建普通索引

### 4.2 推荐索引

- 组合索引：区分度最高的字段放最左边
- 有 ORDER BY 场景：排序字段放组合索引最后
- varchar 字段建索引时指定索引长度（一般 20 即可达到 90% 以上区分度）
- 利用覆盖索引避免回表

### 4.3 索引禁忌

- 禁止左模糊或全模糊查询（`LIKE '%xxx'`），需走 ElasticSearch
- 超过三个表禁止 JOIN
- JOIN 字段数据类型必须一致，且被关联字段必须有索引
- 禁止在无索引字段上做 JOIN
- 禁止使用外键与级联，外键约束在应用层实现

---

## 五、SQL 规约

**强制：**
- 禁止使用 `SELECT *`，必须明确列出查询字段
- 禁止使用外键与级联
- 禁止使用存储过程
- 禁止使用 `TRUNCATE TABLE`
- 禁止在 XML 中使用 `1=1` 等常量条件
- 删除语句禁止使用 `<where></where>` 标签（防止条件为空时全表删除）
- 使用 `#{}` 参数占位符，禁止使用 `${}` 防止 SQL 注入
- 使用 `ISNULL()` 判断 NULL 值，不用 `= NULL`
- `in` 集合元素控制在 1000 个以内
- 分页查询先判断 count，为 0 直接返回
- 字符串类型的 `<if>` 判断需同时判断不等于空字符串
- XML 中 `<`、`>`、`&` 等特殊字符使用 `<![CDATA[]]>` 处理

---

## 六、建表前检查清单

- [ ] 库名与服务名一致，字符集为 utf8mb4
- [ ] 表名符合命名规范，使用单数，加业务前缀
- [ ] 包含所有必备字段：id、created_by、created_date、last_updated_by、last_updated_date
- [ ] 主键类型选择有明确理由（自增 bigint / 业务单号 varchar / 其他）
- [ ] 有逻辑删除需求时已加 deleted_flag 字段
- [ ] 无 float / double 字段
- [ ] 无外键约束
- [ ] 是否字段命名加 `is_` 前缀，类型为 tinyint(1)
- [ ] 高频查询字段已建索引
- [ ] 唯一业务字段已建唯一索引
- [ ] 索引命名符合规范（pk_ / uk_ / idx_）
- [ ] 所有字段有注释
