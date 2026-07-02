# DDD Architecture Coach

> English: [README.en.md](README.en.md)

協助你做 DDD 專案架構決策的 Claude Code skill。主軸是規劃 — 產出設計文件與決策記錄；附帶 bc-developer subagent，可選擇用來依 spec 把實作落地。

## 它做什麼

引導你走完四個階段的架構規劃：

1. **Domain Discovery** — 結構化情境建模、event/command 萃取、BC 劃分、Touchpoint Map（介面形式 surface × 角色 actor × 同步互動 co-presence）、User Stories
2. **Architecture Design** — Context Map、各 BC 內部架構決策、雲端部署藍圖、AI-ADRs
3. **Implementation Spec** — Aggregate 設計、Key Examples（Gherkin/SBE）、分層責任、測試規格、CI/CD
4. **Review & Iterate** — 五份健康度檢查（DDD / AI / engineering / cloud / SBE）

輸出採 BC-centric 檔案結構。每個 BC 獨立走 discovery → design → spec，支援漸進開發。

**團隊平行開發**：per-BC 指令（`/phase-2 <BC>`、`/phase-3 <BC>`）以 BC 名稱為必填參數，artifacts 都寫在各自 `{coach_output_root}/{bc}/` 路徑下。多位開發者可同時各自負責不同 BC、各跑各的 phase，不會在共享狀態檔上產生衝突。`.claude/arch-state.md` 是個人 cursor（bootstrap 時自動加入 `.gitignore`），團隊知識則寫在 `.claude/arch-learnings.md`（append-only，已適合 commit）。

## Skill 構成

這個 skill 套件包含兩個職責分離、共享同一套 DDD 紀律的元件：

| 元件 | 角色 | 位置 |
|------|------|------|
| **Architecture Coach**（本體） | 規劃、產設計文件與決策 | `SKILL.md` + `references/` |
| **bc-developer subagent** | 把 spec 落地到 code，並守住 DDD 紀律 | `assets/agents/bc-developer.md`（Bootstrap 時複製到 `.claude/agents/`） |

兩者透過 `{coach_output_root}/{bc}/spec.md` 交接：coach 在 Phase 3 把 DDD 紀律編進 spec（Aggregate 邊界、UL、Invariants、Key Examples、跨 BC event 通訊…），**bc-developer 讀 spec 後負責讓這些紀律真的長在 code 上** — 強制分層架構（Domain Layer 零外部依賴）、Aggregate-centric vertical slice、UL naming、跨 BC 只走 Domain Events、tenant isolation、SBE 三色覆蓋、commit 粒度＝一個 Invariant 一個 cycle，spec 有歧義時停下回報而非猜測。

它不只是「跑 TDD 的 subagent」 — TDD 是它的工作流程，DDD 紀律落地才是它的職責。

你也可以略過 bc-developer，把 spec 交給人類團隊或其他工具實作 — coach 本身不依賴特定下游，但跳過 bc-developer 等同於放棄這層紀律守門。

### 不想用 bc-developer？

bc-developer 是被動 subagent — 沒被 invoke 就不會跑；裝在 `.claude/agents/` 本身沒有副作用。完全清掉的方式：`rm .claude/agents/bc-developer.md`。

不用 bc-developer 時，你可以接手實作的成品：

| Artifact | 用途 | 位置 |
|----------|------|------|
| `{coach_output_root}/{bc}/spec.md` | 規範性 Phase 3 spec（Aggregates、Invariants、Key Examples、Ports…） | 例 `docs/ddd/{bc}/spec.md` |
| `.claude/rules/{bc}.md` | UL + Aggregate 頂層 + firewall + dispatch 摘要的 rules card | `.claude/rules/{bc}.md` |
| `CLAUDE.md` | 專案 tech stack 規則 | repo root |

三種下游路徑：

1. **交給人類團隊** — 讀 spec + rules card；DDD 紀律靠 review
2. **餵給其他 code agent**（Cursor / Copilot / 沒裝這 skill 的 Claude Code）— spec.md 是 stack-agnostic，可直接作為 prompt context
3. **把 `assets/agents/bc-developer.md` 當 checklist**（讀它的 Constraints 段，不 invoke）— 由人或自製流程套用其中的 DDD 紀律

代價：DDD 紀律由人或自製流程負擔；以上三種路徑都拿不到自動的 spec-ambiguity halt 與 SBE 三色強制。把 bc-developer.md 全文當紀律 checklist 是低成本補強。

## 它不做什麼

- **Coach 本身不產 code**。架構教練 SKILL.md 與四個 phase 流程只產出設計文件與決策記錄。實作職責由 bundle 內建的 bc-developer subagent（或你自己的開發流程）承接，依 Phase 3 的 `spec.md` 守住 DDD 紀律後寫 code。Coach 與 bc-developer 透過 `spec.md` 交接，職責分離但共用一套紀律
- **不是 Domain Storytelling 或 Event Storming workshop**。Phase 1 借用 DS/ES 的詞彙與格式，但 Claude 起草 artifacts 與專家主持的工作坊本質不同
- **不取代團隊討論**。coach 產出的 Key Examples 標記為 AI-drafted baseline，最終簽核需要跨職能 review

## 適用對象

- 想用 Claude Code 做結構化 DDD 架構規劃的開發者與架構師
- 一人或小團隊、需要架構思考夥伴的 domain-driven 系統開發者
- 考慮 AI/LLM 整合、需要系統化介入設計與 fallback 規劃的專案

## 設計重點

**決策優先順序**（trade-off 衝突時依此順序裁決）：

1. Domain correctness > technical elegance
2. Fallback completeness > AI feature richness
3. Verifiability > extensibility
4. Team executability > architectural ideal

**AI 不是預設選項**。每個 AI 提案必須回答：為什麼用 AI？fallback 是什麼？怎麼驗證？採兩級 veto — 一個 Hard 條件（financial/legal 損害且無 human-in-the-loop）無條件否決；三個 Soft 條件（deterministic 替代達 95%+ 準確率、fallback 成本/延遲相當、無 golden dataset）推定否決，除非有書面理由 override。

**Touchpoint Map 補 DDD 探索盲點**。DDD 的探索工具（Domain Storytelling、Event Storming、User Stories）天生 single-driver-biased — 抓得到「誰做了什麼」，抓不到「誰在背後同時看著」。Phase 1 Step 5 強制列舉所有 UI / channel 介面形式（web、admin console、Telegram、SMS、agent console、audit viewer…）× 主要 actor × **secondary observers** × **freshness budget**，並要求至少 1 個跨介面形式的同步互動場景（co-presence scenario）。例：「客人跟 AI 對話」的 User Story 難以自然浮現「客服後台必須在 1 秒內看到對話內容」這種跨介面即時同步規格 — 藉由 Touchpoint Map 可以把這類需求在 Phase 1 就強制浮現並加以討論，不會等到開發、測試階段才發現。原始構想來自 Service Blueprint（Shostack, 1984），保留 touchpoint × actor 部分、捨棄較厚重且難以在 skill 重現的其他元素如 journey / emotion / metrics。

**Specification by Example（SBE）** 內建於 Phase 3。Gherkin 格式的 Key Examples 同時是 spec、測試案例、文件。

## 檔案結構

```
ddd-architecture-coach/
├── SKILL.md                              # 主指令
├── references/                            # coach 自己讀的方法論文件
│   ├── phase1-domain-discovery.md         # 情境建模 + Event/Command 萃取
│   ├── phase2-architecture-design.md      # Context Map + BC 決策 + 部署
│   ├── phase3-implementation-spec.md      # Aggregate 設計 + SBE + 測試規格
│   └── phase4-review-iterate.md           # 健康度檢查
└── assets/                                # Bootstrap 複製到使用者專案的範本
    ├── agents/
    │   └── bc-developer.md                # DDD 實作守門人 subagent（TDD 工作流，stack-agnostic、model 可配置）
    ├── commands/
    │   ├── arch-coach.md                  # 預設入口；讀 state、決定 phase
    │   ├── phase-1.md ... phase-4.md      # 強制進入指定 phase
    │   └── arch-learn.md                  # 寫一條 learning
    └── templates/
        ├── project-context-template.md    # 專案描述、tech stack、model 選擇
        ├── arch-state-template.md         # 個人 cursor（gitignored）：last_touched.{bc, phase, at}（其餘進度從 docs/ddd 推論）
        └── arch-learnings-template.md     # coach 維護：learnings + open questions（append-only）
```

`references/` 是 coach 內部讀的方法論文件；`assets/` 內所有檔案會在 Bootstrap 時複製到你專案的 `.claude/` 目錄。

## 專案輸出結構

coach 在 BC-centric layout 下產出 artifacts，全部放在 `coach_output_root` 之下（Bootstrap 第一次互動時詢問，預設 `docs/ddd/`，可改為 `docs/architecture/`、`docs/`、`packages/foo/docs/ddd/` 等）：

```
{coach_output_root}/
  system/
    domain-stories.md          # 情境 + event/command timeline（跨 BC）
    context-map.md             # BC 分類、關係、部署
    touchpoints.md             # 所有 UI/channel 介面形式 × actor × 同步互動 + 衍生 integration patterns
  {bc}/
    discovery.md               # BC-local events、User Stories、UL、AI 機會
    decisions.md               # 架構決策、AI-ADRs
    spec.md                    # 實作規格（bc-developer 直接讀的 contract）
```

另外，Phase 3 在 spec stable（v1.x）後會在專案根產出 implementation rules card：

```
.claude/
  rules/
    {bc}.md                    # UL + Aggregate 頂層 + firewall + dispatch + DEFERRED（從 spec 摘要）
```

rules card 是 spec 的 quick-reference，給 bc-developer 或其他下游消費者用；衝突時 spec 為準。

> 此 skill 只寫入檔案系統。團隊使用 Confluence / Notion / wiki 需自行同步。

## 安裝

本 skill 遵循 [Agent Skills 開放規格](https://agentskills.io/specification)，與 Claude Code、OpenAI Codex CLI、以及任何相容客戶端可用。

### Claude Code

選一種範圍安裝：

**個人（跨所有專案）**：
```bash
mkdir -p ~/.claude/skills
cd ~/.claude/skills
git clone https://github.com/jed1978/ddd-architecture-coach.git
```

**單一專案（只在該專案啟用）**：
```bash
mkdir -p .claude/skills
cd .claude/skills
git clone https://github.com/jed1978/ddd-architecture-coach.git
```

安裝後 Claude Code 在下次啟動時自動載入。

### OpenAI Codex CLI

```bash
mkdir -p ~/.codex/skills
cd ~/.codex/skills
git clone https://github.com/jed1978/ddd-architecture-coach.git
```

### 其他相容客戶端

skill 資料夾須命名為 `ddd-architecture-coach`（與 SKILL.md `name` 欄位一致）。將整個資料夾放入該客戶端的 skills 載入路徑（依各客戶端文件而定）。

### 驗證安裝

開新對話、執行 `/arch-coach` 或直接向 Claude 描述你的專案。Claude 會偵測 skill 並啟動 Bootstrap 流程（問四個短問題）。

### 升級

```bash
cd <安裝路徑>/ddd-architecture-coach
git pull
```

## 開始使用

1. 在 Claude Code 專案中安裝這個 skill（見上方）
2. 執行 `/arch-coach`（或直接向 Claude 描述專案）
3. **Bootstrap 是對話式的**：Claude 問四個短問題（一句話描述產品、主要 tech stack、團隊規模、輸出根目錄 `coach_output_root`），然後產出 `.claude/project-context.md` 草稿讓你校正 — 不需從零填 YAML
4. Bootstrap 同時複製 `arch-state.md`、`arch-learnings.md`、bc-developer agent、slash command 檔到 `.claude/`
5. Claude 詢問 **bc-developer 子 agent**（spec 落地到 code 的守門人，見上方「Skill 構成」）要用哪個 model（預設 Sonnet 4.6；Haiku 4.5 適合快速 routine TDD；Opus 4.7 適合深度推理）
6. Phase 1 開始：你描述行為，Claude 產出初版 domain model — 你 review 與挑戰，不從零填寫

## 系統需求

- Claude Code
- 一段產品 / 專案描述（至少 3-5 句）
- 願意挑戰 Claude 架構決策的態度

## 起源

在一個多租戶 SaaS 專案（AI 輔助預約 + 排程）上開發並驗證。skill 的規則、self-check、review findings 出自跨多個 Bounded Context 與 1000+ 測試的迭代過程。

## 授權

MIT
