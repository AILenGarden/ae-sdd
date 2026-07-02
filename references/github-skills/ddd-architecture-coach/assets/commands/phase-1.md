---
description: Force entry into Phase 1 (Domain Discovery). Skips state-based phase selection. Use when you explicitly want to (re-)run scenario modeling, event/command extraction, BC delineation, classification, touchpoint map, AI opportunities, or User Stories. With no argument → system-level Steps 1–5. With a BC name → per-BC Steps 6–7 for that BC.
---

請啟動 DDD Architecture Coach（`ddd-architecture-coach` skill）並**強制進入 Phase 1**。

**Argument handling:**
- `$ARGUMENTS` 為空 → 進入 Phase 1 system-level（Steps 1–5）。若 `{coach_output_root}/system/domain-stories.md` 已存在 → 詢問使用者要 revise 既有 system 文件、還是改傳一個 BC 跑 Steps 6–7。
- `$ARGUMENTS` 含 BC 名稱 → 進入該 BC 的 Phase 1 Steps 6–7（per-BC Domain Discovery）。Pre-flight：先 probe `{coach_output_root}/system/` 確認 system-level Steps 1–5 已完成；若未完成 → 報錯「System-level Phase 1（Steps 1–5）必須先完成。請先執行不帶參數的 `/phase-1`。」

執行步驟：
1. 讀取 `.claude/project-context.md`（必要）與 `.claude/arch-state.md`（讀取作為 fallback hint，非權威）
2. 讀取 `references/phase1-domain-discovery.md`，依「Mode Selection」決定 system-full / system-incremental / per-bc
3. 跳過 Phase Selection Logic 的自動判斷，直接執行 Phase 1
4. 進入 phase 時更新 `.claude/arch-state.md` 的 `last_touched: { bc, phase, at: <today> }`（個人 cursor，不寫團隊狀態）

$ARGUMENTS
