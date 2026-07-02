---
description: Launch the DDD Architecture Coach. With no argument, shows an in-flight summary derived from {coach_output_root}/ and asks which BC to focus on. With a BC name, derives the next phase for that BC and enters it. Use this as the default entry point.
---

請啟動 DDD Architecture Coach（`ddd-architecture-coach` skill）。

**Argument handling:**

### `/arch-coach <BC>` — BC 已知

1. 從 `$ARGUMENTS` 取得 BC 名稱
2. Probe `{coach_output_root}/{bc}/` 決定該進入哪個 phase：
   - 沒有 `discovery.md` → Phase 1 Steps 6–7（pre-flight：先確認 system Steps 1–5 已完成；若否 → 引導使用者跑 `/phase-1`）
   - 有 `discovery.md`、沒有 `decisions.md` → Phase 2
   - 有 `decisions.md`、沒有 `spec.md` 或 `spec.md` frontmatter `status: draft` → Phase 3
   - `spec.md` frontmatter `status: v1.x` → 該 BC 已穩定；建議 `/phase-4 Phase 3:<BC>` 做 review
3. 進入 phase 時更新 `.claude/arch-state.md` 的 `last_touched: { bc, phase, at: <today> }`

### `/arch-coach`（無參數）— 顯示 in-flight 摘要

1. 讀取 `.claude/arch-state.md` 取得 `last_touched`（若是舊版 `current_focus` schema → 靜默 migrate，見 SKILL.md → State Determination → Migration）
2. Probe `{coach_output_root}/` 列舉 in-flight 工作：
   - System-level：Steps 1–5 完成度、`touchpoints.md` 是否存在
   - 對 `{coach_output_root}/` 下每個非 `system/` 子目錄：依「BC 已知」分支的邏輯推算 next phase
3. 渲染 in-flight 摘要表給使用者：

   | BC | 上次完成 | 下一步 | 備註 |
   |---|---|---|---|
   | Channel | Phase 2 | Phase 3 (draft) | spec.md status: draft |
   | Operator | Phase 1 | Phase 2 | — |
   | (system) | Steps 1–5 ✓ | — | touchpoints.md 缺 → 見 open_questions |

4. 詢問使用者要 focus 哪個 BC。預設建議 `last_touched.bc`（若該 BC 仍 in flight）；否則建議 `{coach_output_root}/{bc}/` 中 file mtime 最新的 BC
5. 使用者選定後 → 走「BC 已知」分支

執行步驟：
1. 若 `.claude/arch-state.md` 不存在或 `project-context.md` 未填妥 → 執行 Bootstrap 對話流程
2. 依上面的 argument handling 邏輯決定 BC + phase
3. 開始該 phase 的工作

$ARGUMENTS
