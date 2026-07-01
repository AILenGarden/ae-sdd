# 2026-07-01 | document_storage 激活阶段3+4：hook 增强 + 门禁运行时化（v3.7.2）

## Summary

完成 document_storage 激活的最后两个可选阶段：阶段3 让 PreToolUse hook 的拦截提示指向 `ae-sdd doc save` 命令；阶段4 让 G-DOC-STORAGE 门禁从纯硬编码合规根列表升级为"有 assets 时用 resolve_doc_workspace 真值校验"的渐进增强。两者均采用零回归设计。

## Changes

### 阶段3：gate_intercept.py _check_product_landing 增强

**改动点：** `_check_product_landing` 函数（L190-233）

**设计原则（零误伤）：** 不引入新的拦截点。仅在**已有 deny 判定**（关卡1 entry token 缺失 / 关卡2 产物-Phase 不匹配）时，在 deny_reason 末尾**追加** `ae-sdd doc save` 修复建议。

新增统一修复提示：
```
💡 建议：用 `ae-sdd doc save` 命令落地流程产物，代码自动处理路径/版本/ChangeLog/索引/gitignore，
无需手拼路径（document-storage-skill §4.0 CLI 入口）。
```

效果：LLM 被拦截时看到的修复指引从"先领 token + 切 phase"升级为"先领 token + 切 phase + 用 doc save 落地"。

### 阶段4：gates.py check_g_doc_storage 渐进增强

**改动点：** `check_g_doc_storage` 函数（L1646）

**设计原则（零回归）：** 渐进增强——能从 project_dir 定位 `.ae-sdd/` 并读 docWorkspacePath 时，用它做真值校验（产物是否在真实 workspace 下）；拿不到时回退现有硬编码 `_DOC_COMPLIANT_ROOTS` 列表。

| 判定逻辑 | 旧行为 | 新行为（v3.7.2）|
|---------|--------|----------------|
| 产物合规性 | 硬编码 `_DOC_COMPLIANT_ROOTS` 子串匹配 | 硬编码匹配 + **若拿到 real_workspace 则产物在其下也算合规** |
| assets 缺失 | 硬编码匹配 | 回退硬编码匹配（不变）|
| action 提示 | "调用 resolve_path 推导路径" | "用 `ae-sdd doc save` 命令落地" |
| details | stray_files + checked | + `real_workspace`（调试用）|

真值校验实现：从 `project_dir/.ae-sdd/config.yaml` 读 projectKey → 读 `assets/{pk}.assets.md` §1 docWorkspacePath（缺省=gitPath）→ resolve 后比较产物路径前缀。

## 验证

| 测试套件 | 用例数 | 结果 |
|---------|--------|------|
| test_gates | 106 | ✅ 全过 |
| test_gate_intercept | 99 | ✅ 全过 |
| test_gate_intercept_v11 | 42 | ✅ 全过 |
| test_document_storage | 13 | ✅ 全过 |
| test_cli_doc | 8 | ✅ 全过 |
| test_state | 44 | ✅ 全过 |
| **合计** | **312** | **全过** |

gates.py / gate_intercept.py 语法解析通过。

## Sync

- 本次修改 `tools/lib/gate_intercept.py` + `tools/lib/gates.py`（不含文档改动）。
- `dev-sync` 需在 update-check 通过后执行。
