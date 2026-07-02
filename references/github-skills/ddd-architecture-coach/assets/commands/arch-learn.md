---
description: Append a learning to .claude/arch-learnings.md. Pass the learning content as the argument; the coach writes it with source=user_triggered and the appropriate applies_to scope.
---

請啟動 DDD Architecture Coach（`ddd-architecture-coach` skill）並**寫入一條 learning**。

執行步驟：
1. 把 $ARGUMENTS 視為使用者要記錄的 learning 內容
2. 讀取 `.claude/arch-learnings.md`（若不存在，依 SKILL.md Memory/State/Learnings 三層分工原則建立）
3. 判斷 `applies_to` 範圍：
   - 內容明顯為個人偏好（跨專案）→ 提醒使用者應寫入 Claude Code memory，不要寫入 arch-learnings
   - 本專案 phase / BC 級規範 → append 到 arch-learnings.md，source: `user_triggered`
4. 確認後寫入並回報新 learning 的位置

$ARGUMENTS
