# 2026-07-03 源 SKILL 瘦身标准化

## 背景

源 SKILL 瘦身已经可以降低入口加载成本，但如果没有统一标准，容易变成随意删减，存在语义丢失风险。用户明确要求源 SKILL 瘦身必须先做语义识别，保持与设计一致，并形成模板和 SOP。

## 变更

- 新增 `source/standards/skill-source-slimming-standard.md`，定义源 SKILL 瘦身的权威关系、语义识别类别、SOP、禁止事项和验收标准。
- 新增 `source/templates/skill/source-skill-slim-entry-template.md`，固定 slim entry frontmatter、加载契约、语义 inventory、SOP、heading 和 inline reference 格式。
- 升级 `scripts/slim_source_skills.py` 到 `ae-sdd-source-slim/v2`：
  - 识别 identity/trigger、workflow/route、gate/constraint、tool/API、state/data、output/document、resource reference、design alignment、fallback-only detail 九类语义。
  - 每个 slim entry 写入 `source_semantic_inventory_sha256`。
  - `--validate` 校验 fallback 哈希、schema、标准/模板路径、必备章节、语义 inventory hash 和模板重渲染一致性。
  - `--upgrade` 只从 `source_fallback` 重渲染旧 schema，禁止从已瘦身文本二次摘要。
- `scripts/compile_all_skills.py` 在源瘦身校验失败时停止编译，避免生成语义不可信 runtime。
- 更新设计文档：
  - `source/docs/ae-sdd-design.md` 记录源瘦身预编译阶段与语义边界。
  - `source/docs/ae-sdd-implementation-architecture.md` 记录 `source/skill-fallbacks/**`、源瘦身模块职责和构建数据流。
  - `source/docs/skill-runtime-compiler.md` 记录源瘦身标准、SOP 和编译器读取 fallback 的约束。
- 新增 `tools/tests/test_source_skill_slimming.py` 覆盖新建瘦身、v1 升级和 fallback hash mismatch。

## 影响范围

- 影响 `source/SKILL.md` 与 `source/skills/**/*.md` 的维护方式。
- 不改变 runtime 编译器的硬门禁优先级：CLI/gate/state 输出仍高于 SKILL 文本。
- 已瘦身源文件不会默认二次瘦身；schema 升级必须显式使用 `--upgrade`，且输入来自 fallback。

## 验证

```bash
python scripts/slim_source_skills.py --upgrade
python scripts/slim_source_skills.py --validate
python scripts/compile_all_skills.py --include-references
python -m unittest tools.tests.test_source_skill_slimming tools.tests.test_skill_runtime_compiler tools.tests.test_standalone_skill_runtime_compiler tools.tests.test_runtime_verify
python tools/bin/ae-sdd runtime verify --path dist/ae-sdd
python tools/bin/ae-sdd update-check --only UC-15 --json
```
