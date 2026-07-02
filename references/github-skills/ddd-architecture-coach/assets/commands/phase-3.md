---
description: Force entry into Phase 3 (Implementation Specification) for one BC. Produces aggregate definitions, key examples (Gherkin), test specs, slice plan. Requires explicit BC argument.
---

請啟動 DDD Architecture Coach（`ddd-architecture-coach` skill）並**強制進入 Phase 3**。

**Argument handling:**

`$ARGUMENTS` 必須包含 BC 名稱。為空 → 報錯並結束：

> Phase 3 需要明確的 BC 參數。用法：`/phase-3 <BC-name>`。要查看目前哪些 BC 在進行中，請執行不帶參數的 `/arch-coach`。

**不要** fallback 到 `arch-state.md` 的 `last_touched.bc` — 顯式命名 BC 可以避免在團隊環境中誤寫到別的 BC 的 `spec.md`。

執行步驟：
1. 讀取 `.claude/project-context.md` 與 `.claude/arch-state.md`（後者僅作為 personal cursor 紀錄用）
2. 讀取 `references/phase3-implementation-spec.md`，依其他穩定 BC 的 `spec.md` 決定 template-lead / template-from-prior
3. 從 `$ARGUMENTS` 取得目標 BC 名稱（必填）
4. 執行 Pre-flight Checks（Phase 1/2 必須完成且穩定）
5. 進入 phase 時更新 `.claude/arch-state.md` 的 `last_touched: { bc: <BC>, phase: phase_3, at: <today> }`

$ARGUMENTS
