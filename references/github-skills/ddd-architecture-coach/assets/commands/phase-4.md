---
description: Force entry into Phase 4 (Review & Iterate). Reviews any phase or BC artifact against five health checklists (DDD / AI / Engineering / Cloud / SBE) and produces scored findings. Optional scope argument (Phase 1 / Phase 2 / Phase 3:<BC> / <BC> / system / full); no argument runs full review.
---

請啟動 DDD Architecture Coach（`ddd-architecture-coach` skill）並**強制進入 Phase 4**。

**Argument handling:**

`$ARGUMENTS` 為 review 範圍（可選）：
- 空 → full review（system + 所有 BC）
- `Phase 1` / `Phase 2` → 對應 phase 的 system-level review
- `Phase 3:<BC>` 或 `<BC>` → 該 BC 的 Phase 3 review
- `system` → 僅 system 層 artifacts
- `full` → 等同於空（明確版）

執行步驟：
1. 讀取 `.claude/arch-learnings.md`（避免重複 flag 已解決的問題）；`.claude/arch-state.md` 僅在 review 結論為 rollback retarget 時更新
2. 讀取 `references/phase4-review-iterate.md`
3. 依 $ARGUMENTS 判斷 review scope
4. 執行 Pre-flight Checks，依適用的 checklist 產出評分報告
5. 若 review 結論為 rollback：更新 `.claude/arch-state.md` 的 `last_touched: { bc: <retarget BC 或空>, phase: <retarget phase>, at: <today> }`；否則不動 `arch-state.md`

$ARGUMENTS
