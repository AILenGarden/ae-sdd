# 工具用法与边界

下文的 `<capability-root>` 指当前 `CAPABILITY.md` 所在目录，即 ae-sdd 入口旁的 `capabilities/al-knowledge`。开发源目录执行时使用 al-knowledge 模块根目录。

要求 Python 3.11+ 与 [requirements.txt](../requirements.txt) 中的 PyYAML。选择已有合适环境；必要时在独立 venv 中执行 `python -m pip install -r <capability-root>/requirements.txt`，不要修改业务项目依赖。Windows 可选 `py -3.13` 等已安装的稳定版本。

```text
python <capability-root>/scripts/knowledge.py check <project>/.al-knowledge
python <capability-root>/scripts/knowledge.py inspect <project>/.al-knowledge --repo main=<project>
python <capability-root>/scripts/knowledge.py impact <project>/.al-knowledge --id API-CANCEL --repo main=<project>
python <capability-root>/scripts/knowledge.py render <project>/.al-knowledge --repo main=<project>
python <capability-root>/scripts/knowledge.py render <project>/.al-knowledge --repo main=<project> --write
```

前四条只读；render 默认 dry-run，只有 --write 写生成产物。路径含空格时按当前 shell 正确引用。--repo 可重复，为登记的来源仓库显式指定可读根，不访问知识文档自称的本机路径。

## 结果解释

- 输出 JSON：基础 `okf_valid`、工程 `profile_valid`、分层 issues、直接/来源依赖变化的 `directly_changed`、`unchecked`、`possibly_affected`。
- `profile_valid` 只表示在识别的工程 profile 范围内无结构 errors；外部 OKF 的 `profile-unavailable` 必须同时披露，不能将它解释为完整 ae-sdd 工程库。
- inspect 额外输出正文及概念原始字节摘要，辅助在真实核验后填写 checked。工具不会写 checked，也不会自动把 unknown 改为 confirmed。
- impact 以来源变化及 --id 为起点，沿已登记关系/来源/调用关联计算可能影响，可能保守过宽；不是语言分析器或全项目覆盖证明。
- 返回码 0：无结构 error（可以有过期、未核查和导航 warning）；1：结构 error；2：操作失败/非法参数/写入冲突。不得把 0 解释为事实正确、已回源或测试通过。
- 普通 broken-link 是 warning；已确认工程关系指向缺失/身份不符对象是 error。未知字段保留；不会重写来源概念。

## 生成所有权与恢复

索引按目录生成，Spec/接口/调用树位于 bundle 同级 `.al-knowledge-views/`。每个生成文件有内容摘要标记，覆盖只允许原有内容完整匹配；人工编辑、无标记、链接逃逸等会拒绝。保留人工索引，将其有价值内容转移到概念或先明确决定如何处理，不自动 force。

渲染先核查全部写入目标，再用排他 `.render.lock` 防同工具并发；写前复查读到的文档集合/字节；单文件临时写再替换。非合作编辑器仍存在检查与写入之间的竞争窗口，应在用户未同时编辑的范围执行。多文件非事务；失败消息含已写清单，可核对后重跑。崩溃遗留锁只有确认无活动写者后才人工处理。

索引不存事实，输出确定且可重建。移动概念需先按稳定 ID 更新入链，重建前检查旧目录索引；工具对不可自动确认的内容保留并提示。

## 验证

```text
python -m unittest discover -s <capability-root>/tests -v
python <capability-root>/scripts/knowledge.py check <capability-root>/tests/fixtures/project/.al-knowledge --repo main=<capability-root>/tests/fixtures/project
```

测试覆盖基础格式、引用与关系、来源变化、只读性、生成幂等、人工内容保护及失败恢复。fixture 是虚构领域，用可执行本地代码验证示例行为；不能代替具体业务项目试用或事实评审。
