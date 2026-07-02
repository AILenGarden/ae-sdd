---
description: Force entry into Phase 2 (Architecture Design) for one BC. Requires explicit BC argument. Use when you want to (re-)work on Context Map, per-BC architecture decisions, cloud deployment, or AI-ADRs for a specific BC.
---

請啟動 DDD Architecture Coach（`ddd-architecture-coach` skill）並**強制進入 Phase 2**。

**Argument handling:**

`$ARGUMENTS` 必須包含 BC 名稱。為空 → 報錯並結束：

> Phase 2 需要明確的 BC 參數。用法：`/phase-2 <BC-name>`。要查看目前哪些 BC 在進行中，請執行不帶參數的 `/arch-coach`。

**不要** fallback 到 `arch-state.md` 的 `last_touched.bc` — 顯式命名 BC 可以避免在團隊環境中誤寫到別的 BC 的 `decisions.md`。

執行步驟：
1. 讀取 `.claude/project-context.md` 與 `.claude/arch-state.md`（後者僅作為 personal cursor 紀錄用）
2. 讀取 `references/phase2-architecture-design.md`，依「Mode Selection」決定 greenfield / incremental
3. 執行 Pre-flight Checks，缺資料則停下並請使用者補
4. 以 `$ARGUMENTS` 指定的 BC 為 per-BC 範圍焦點
5. 進入 phase 時更新 `.claude/arch-state.md` 的 `last_touched: { bc: <BC>, phase: phase_2, at: <today> }`

$ARGUMENTS
