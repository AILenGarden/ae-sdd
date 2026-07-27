# ae-sdd 能力标准测试题 — test-tool 综合能力库

> ⚠️ **本目录是 ae-sdd 能力测试题的落地区域。**
>
> - 需求文档：[`../../ae-sdd-doc/RA/RA-AE-SDD-CAPABILITY-TEST-TEST-TOOL.md`](../../ae-sdd-doc/RA/RA-AE-SDD-CAPABILITY-TEST-TEST-TOOL.md)
> - 评分卡：[`EVALUATION.md`](./EVALUATION.md)
> - 指标 Schema：[`metrics.schema.json`](./metrics.schema.json)
>
> 本 `README.md` / `EVALUATION.md` / `metrics.schema.json` 三份文件是**评估基础设施**，由测试设计者预置，**ae-sdd 在跑流程时不应修改它们**。ae-sdd 只应创建/修改 `Cargo.toml` 与 `src/` 下的实现代码。

## 这道题考什么

`test-tool` 是一道综合能力压测题，以「2D 网格自动寻路」为业务场景，分层考查 5 个能力维度：

### 必修层（所有模型必须通过）

| 维度 | 内容 | 星度 |
| ---- | ---- | ---- |
| **算法实现** | BFS / Dijkstra / A\* 三种最短路 + 四/八连通 + NodeLimitExceeded 上下文 | ★~★★★★ |

涵盖 AC-1 ~ AC-10，包含确定性测试（AC-10）和高级启发式有效性测试（AC-9）。

### 选修层（4 组能力压测，模型按能力上限选择）

| 组别 | 考查能力 | AC | 星度 | 满分 |
| ---- | -------- | -- | ---- | ---- |
| **4.1 trait 抽象 + 多态** | 把寻路器抽象成 trait，三算法 impl；动态分派 vs 静态分派对等 | AC-11, AC-12 | ★★★~★★★★ | +8 |
| **4.2 泛型设计** | `Cost` trait + `GenericGridMap<C>`；泛型与 trait 正交 | AC-13, AC-14 | ★★★★~★★★★★ | +8 |
| **4.3 Builder + IntoIterator** | `PathRequest::builder()` 链式构造；`PathResult: IntoIterator` | AC-15, AC-16 | ★★★ | +7 |
| **4.4 复杂序列化** | `#[serde(flatten)]` + 自定义 `serialize_with`（Position→`[r,c]`） | AC-17, AC-18 | ★★★★~★★★★★ | +7 |

**选修总分**：0-30 分，作为「模型能力上限」单独记录（`capabilityCeiling`）。

> 评分规则：每组选修必须两条 AC 都 PASS 才给分（防半吊子）。放弃某组要在 Story 显式 `deferred: true` + 理由；不声明又不实现 = 倒扣 1 分（测自我认知）。

### 能力分档（基于选修分）

| 选修分 | 标签 | 含义 |
| ------ | ---- | ---- |
| 0-7 | basic | 只能做算法实现 |
| 8-15 | intermediate | 能做 trait 或 builder |
| 16-23 | advanced | trait + 泛型 + serde 基本都能 |
| 24-30 | top | 5 星 AC 全达成 |

## 目录用途

| 文件/目录 | 谁创建 | 谁可以修改 |
| --------- | ------ | ---------- |
| `README.md` | 测试设计者（已预置） | 仅测试设计者 |
| `EVALUATION.md` | 测试设计者（已预置） | 仅测试设计者 |
| `metrics.schema.json` | 测试设计者（已预置） | 仅测试设计者 |
| `Cargo.toml` | ae-sdd（首次跑时创建） | ae-sdd |
| `src/**` | ae-sdd（首次跑时创建） | ae-sdd |
| `metrics-*.json` | 操作员（每次跑完落地） | 操作员 |

## 重置命令

每次重跑前，**只重置 ae-sdd 的产物**，保留评估基础设施：

```bash
git -C "D:/Item/ae-sdd" clean -fdx demo/test-tool/src/
rm -f "D:/Item/ae-sdd/demo/test-tool/Cargo.toml"
rm -f "D:/Item/ae-sdd/demo/test-tool/Cargo.lock"
```

> 注意：不要 checkout 整个 `demo/test-tool/`，否则会覆盖本 README/EVALUATION/schema。

## 跑测试

详见 [`EVALUATION.md` §怎么跑](./EVALUATION.md) 和 [需求文档 §9 操作员手册](../../ae-sdd-doc/RA/RA-AE-SDD-CAPABILITY-TEST-TEST-TOOL.md)。

简述：
1. 在新 session 里 `/ae-sdd`，把需求文档 ID 告诉 ae-sdd。
2. 跟完 Route → RA → DR → Story → CodingPlan → [你批准] → Coding → Test → Review。
3. 按 `EVALUATION.md` 打分，按 `metrics.schema.json` 落地 `metrics-<STORY-ID>-run<N>.json`。

## 设计取舍

- **没预填 `Cargo.toml` 和 `src/`**：预填会污染 ae-sdd 的 Coding 能力测试，让 ae-sdd 自己创建才能真正测出它从零开始的能力。
- **demo 排除出 workspace**：写在需求 §8.2，避免污染主 `cargo test --workspace`，这也是给 ae-sdd 的一个工程纪律考点。
- **分层 AC**：必修 + 选修的组合既能让基础模型跑通主流程（拿到基本分），又能区分中高端模型的能力上限（看选修分）。同一道题既测了 ae-sdd 流程合规性，又测了底层 LLM 的工程能力。
- **半吊子倒扣分**：选修组只完成一条 AC 不给分；不声明放弃又不实现倒扣 1 分。这是测模型的自我认知——能不能判断自己做不到并诚实声明。
