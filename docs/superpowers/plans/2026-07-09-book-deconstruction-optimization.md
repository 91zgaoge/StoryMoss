# 拆书完善与优化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复拆书故事线/作者/伏笔落库断裂，并用已有 LanceDB 向量提升续写参考 few-shots，使拆书对后续写作质量产生可测增益。

**Architecture:** 主路径仍走 `BookDeconstructionExecutor` → `AnalysisPipeline`；Phase A 只改「解析结果写入 bundle/DB」与前端过滤；Phase B 在 `WriteTimeBundle` 增加向量检索优先、Jaccard 降级；Phase C 移除 legacy `BookAnalyzer` 并加 `deconstruction_runs` 观测。不新增热路径 LLM。

**Tech Stack:** Rust AnalysisPipeline / foreshadowing_tracker / LanceDB VectorStore / WriteTimeBundle / React BookDeconstruction / cargo 契约测试

**Audit:** `docs/audits/2026-07-09-book-deconstruction-audit.md`  
**Design:** `docs/plans/2026-07-09-book-deconstruction-optimization-design.md`

---

## File map

| File | Phase | Responsibility |
|------|-------|----------------|
| `src-tauri/src/narrative/analysis.rs` | A | StoryArc 写入 `bundle.outline`；伏笔写入 tracker |
| `src-tauri/src/domain/narrative_elements.rs` | A | `StoryMetaElement.author: Option<String>`（serde default） |
| `src-tauri/src/narrative/mod.rs` + extract fallbacks | A | 解析 author 字段 |
| `src-tauri/src/book_deconstruction/executor.rs` | A | `convert_bundle` 传 author；story_arc 来自 outline |
| `src-tauri/src/book_deconstruction/service.rs` | A/B | 保存伏笔；embedding 元数据可检索 |
| `src-frontend/src/hooks/useBookDeconstruction.ts` | A | `pipeline_id === bookId` 过滤 |
| `docs/USER_GUIDE.md` | A | 降级 §3.9 承诺 |
| `src-tauri/src/creative_engine/write_time_bundle.rs` | B | 向量 few-shots + Jaccard 降级 |
| `src-tauri/src/vector/*` + ports | B | 按 `record_type=reference_scene` + story/book id 检索 |
| `src-tauri/src/db/migrations/V###__deconstruction_runs.rs` | B | 可选 run 表 |
| `src-tauri/src/book_deconstruction/analyzer.rs` | C | 删除或 `#[cfg]`/编译期禁用 fallback |
| 测试 | A–C | analysis 契约 + few-shots + convert |

---

## Phase A — 持久化闭环（建议 v0.26.46）

### Task A1: StoryArc 写入 OutlineElement（修 D1）

**Files:**
- Modify: `src-tauri/src/narrative/analysis.rs`（`StoryArcExtractionStep`，约 755–777）
- Modify: `src-tauri/src/book_deconstruction/executor.rs`（`convert_bundle_to_analysis_result`）
- Test: `analysis.rs` 或新建 `narrative/analysis_arc_tests`

- [x] **Step 1–5: 完成** — `arc_response_to_outline` + 写入 `bundle.outline`；提示词 schema 对齐 `main_arc`；3 契约测试绿

---

### Task A2: Author 字段贯通（修 D2）

**Files:**
- Modify: `src-tauri/src/domain/narrative_elements.rs` — `StoryMetaElement`
- Modify: `src-tauri/src/narrative/mod.rs` — `extract_story_meta_fallback` / prose
- Modify: `src-tauri/src/narrative/prompts.rs` + `resources/prompts/creation/narrative_story_concept_extract.md`（若缺 author）
- Modify: `executor.rs` `convert_bundle_to_analysis_result` — `author: meta.author.clone()`
- Modify: 前端类型/详情展示（`book-deconstruction` types + Overview）

- [x] **Step 1–5: 完成** — `StoryMetaElement.author` + extract/convert 贯通；UI 已有展示；契约测试绿

---

### Task A3: 伏笔写入 foreshadowing_tracker（修 D6 审计项）

**Files:**
- Modify: `src-tauri/src/book_deconstruction/executor.rs` 或 `service.rs` 保存阶段
- Use: 现有 foreshadowing repository / tracker API（检索 `ForeshadowingTracker` / `create`）
- Test: 保存后 `SELECT COUNT(*) FROM foreshadowing_tracker WHERE story_id = book_id`

- [ ] **Step 1: 定位现有写入 API**

```bash
rg -n "foreshadowing_tracker|ForeshadowingTracker|add_foreshadowing" src-tauri/src --glob '*.rs' | head -40
```

- [ ] **Step 2: 在 Analysis 完成后、status=completed 前**

对 `bundle.foreshadowings` 逐条 upsert：
- `story_id = book_id`
- `source` / 内容 / importance
- 失败单条 warn，不 fail pipeline（与 Genesis errors 风格一致）

- [ ] **Step 3: convert_to_story 时**

要么复制 tracker 行到新 `story_id`，要么文档标明「参考书伏笔仍挂 book_id，转故事后在拆书详情可见」——**推荐复制到新 story**（否则续写看不到）。

- [ ] **Step 4: 契约测试 + commit**

```bash
git commit -m "$(cat <<'EOF'
feat: persist deconstruction foreshadowings into foreshadowing_tracker

EOF
)"
```

---

### Task A4: 进度串扰 + 文档降级（修 D5 / D7）

**Files:**
- Modify: `src-frontend/src/hooks/useBookDeconstruction.ts` L82–96
- Modify: `docs/USER_GUIDE.md` §3.9
- Modify: `AnalysisProgress.tsx` 步名与后端 7+保存 对齐（可选）

- [ ] **Step 1: 过滤**

```ts
if (p.pipeline_type !== 'analysis') return;
if (p.pipeline_id !== bookId) return;
```

- [ ] **Step 2: USER_GUIDE** 改为实际能力：结构摘要、人物/场景列表、LitSeg 强度、转故事；删除未交付的「出场频率/完整高潮曲线图表」或标「规划中」

- [ ] **Step 3: vitest**（若有 hook 测试基建）或手工清单

- [ ] **Step 4: commit**

```bash
git commit -m "$(cat <<'EOF'
fix: filter book analysis progress by book_id; align USER_GUIDE

EOF
)"
```

---

### Task A5: Phase A 发布门

- [ ] `cargo test --lib` + `architecture_guard` + frontend checks
- [ ] bump **v0.26.46** + docs of record + tag + push + **监控 CI 至全绿**
- [ ] 本地 `cargo tauri build`（dmg 失败则重试 bundle）

---

## Phase B — 向量 few-shots + 观测（建议 v0.26.47）

### Task B1: 向量检索 few-shots（修 D3）

**Files:**
- Modify: `src-tauri/src/creative_engine/write_time_bundle.rs`
- Possibly: `StoryContextBuilder` / orchestrator 异步加载路径
- Read: `src-tauri/src/vector/lancedb_store.rs` `search` / `search_with_embedding`
- Test: mock 或集成：有 embedding 时返回向量序；无则 Jaccard

**设计约束：**
- `load_sync` 保持同步 API 时：**优先**在 async 调用方（TimeSliced/TriShot 构建 bundle 处）注入 few-shots；或 `block_in_place`/`Handle::current().block_on` **禁止**在 tokio worker 无条件乱用——优先改 async 加载。
- 检索过滤：`record_type == "reference_scene"` 且 id 前缀/metadata 含 `book_id`
- top-k = 3；相似度阈值过低则降级 Jaccard
- **零额外用户可见延迟目标：** 向量检索 p95 ≤150ms；超时则降级

- [ ] **Step 1: 写契约测试** `select_reference_fewshots(vector_hits, jaccard_fallback) -> Vec`

- [ ] **Step 2: 实现向量路径 + 降级**

- [ ] **Step 3: 日志** `write_time.reference_fewshots` — `{source: "vector"|"jaccard", count}`

- [ ] **Step 4: commit**

```bash
git commit -m "$(cat <<'EOF'
feat: prefer LanceDB vector search for reference scene few-shots

EOF
)"
```

---

### Task B2: deconstruction_runs 观测雏形（修 D8）

**Files:**
- Create: `src-tauri/src/db/migrations/V###__deconstruction_runs.rs`（ASCII 短名）
- Modify: executor 开始/每步/结束写入 steps_json + errors
- Frontend: 可选任务详情展示（可仅 DB + 日志，UI 可延后）

表字段建议：`id, book_id, status, steps_json, error, started_at, finished_at`

- [ ] **Step 1: 迁移 + repository**
- [ ] **Step 2: executor 埋点**
- [ ] **Step 3: 测试迁移幂等**
- [ ] **Step 4: commit + bump v0.26.47 发布门**

---

## Phase C — 统一管线与测试（建议 v0.26.48）

### Task C1: 移除 / 封印 BookAnalyzer（修 D4）

**Files:**
- Modify: `service.rs` fallback 分支
- Modify: `analyzer.rs` — 删除或 `deprecated` + 编译开关 `feature = "legacy-deconstruction"` 默认关

- [ ] **Step 1: 确认主路径覆盖率**（无 task 时直接报错，不再 silent fallback）
- [ ] **Step 2: 删除死代码或移入 `legacy/`**
- [ ] **Step 3: 统一只用 `narrative_*_extract` prompts；`deconstruction_*.md` 标记 deprecated 或删除（需 PromptRegistry 同步）
- [ ] **Step 4: commit**

---

### Task C2: 测试加固（修 D9）

最低集：

| 测试 | 断言 |
|------|------|
| `arc_response_to_outline` | acts 非空 |
| `convert_bundle` author/story_arc | 字段透传 |
| few-shots selector | vector 优先 |
| progress filter | 前端单测 |
| upload duplicate hash | 已有，保持绿 |

- [ ] **Step 1: 补齐至 ≥8 新测试**
- [ ] **Step 2: 全量验证 + v0.26.48 发布**

### Task C3: 可选 — 转故事 KG 继承

- 将 `kg_entities` 中 `story_id=book_id` 复制到新 story（或重建边）
- 明确不做物理合并；失败 fail-open

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| StoryMeta 加 author 影响 Genesis serde | `#[serde(default)]` + 旧 JSON 测试 |
| 向量检索拖慢续写热路径 | 超时降级 Jaccard；top-3；可配置关闭 |
| 删 BookAnalyzer 破坏 fallback | Phase C 先 metrics 确认主路径 100% 走 Executor |
| 伏笔复制重复 | ledger_key / upsert |

---

## 执行手顺（推荐）

```text
Phase A (v0.26.46): A1 → A2 → A3 → A4 → A5 发布
Phase B (v0.26.47): B1 → B2 发布
Phase C (v0.26.48): C1 → C2 → (C3) 发布
```

每 Phase 独立可发布、可回滚；禁止单 PR 吞三 Phase。

---

## Execution handoff

Plan saved to `docs/superpowers/plans/2026-07-09-book-deconstruction-optimization.md`.

**两选一：**

1. **Subagent-driven（推荐）** — 按 Task 派生子代理  
2. **Inline** — 本会话从 Task A1 开始  

默认从 **Phase A / Task A1** 开工。
