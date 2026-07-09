# 创世方法论全面应用 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复创世 background 方法论静默断链，统一 ID/步进/骨架注入，并（门控后）按步骤注入对应方法论 phase，使方法论在创世全链路可验证生效。

**Architecture:** 保持 `build_strategy_notes` / `resolve_methodology_prompt` 为唯一展开入口；外部化 narrative 模板补齐 `strategy_section`/`quartet_section`；新增 `normalize_methodology_id` 与模板变量契约测试；Phase C 用 `build_strategy_notes_for_step` 按创世步骤选择 snowflake/HDWB 子 prompt，并在创世结束写回 `methodology_step`。

**Tech Stack:** Rust Genesis pipeline / PromptRegistry / StrategySelector / WriteTimeBundle / resources/prompts markdown / cargo 契约测试

**Audit:** `docs/audits/2026-07-09-methodology-in-genesis-audit.md`  
**Design:** `docs/plans/2026-07-09-methodology-in-genesis-remediation-design.md`

---

## File map

| File | Phase | Responsibility |
|------|-------|----------------|
| `resources/prompts/creation/narrative_world_building_generate.md` | A | 补 `strategy_section` / `quartet_section` |
| `resources/prompts/creation/narrative_outline_generate.md` | A | 同上 |
| `resources/prompts/creation/narrative_character_generate.md` | A | 同上；变量名与代码 `world_concept`/`outline_summary` 对齐 |
| `resources/prompts/creation/narrative_scene_generate.md` | A | 同上 |
| `resources/prompts/creation/narrative_foreshadowing_generate.md` | A | 同上 |
| `src-tauri/src/narrative/prompts.rs` | A/B/C | fallback 对齐；brief helper；步进 notes；契约测试 |
| `src-tauri/src/narrative/genesis.rs` | B/C | step 落库；骨架用 brief；分步 notes；日志 |
| `src-tauri/src/domain/methodology.rs` 或 `strategy/` | B | `normalize_methodology_id` |
| `src-tauri/src/strategy/selector.rs` | B | 预填 `recommended_methodology_id` |
| `src-tauri/src/strategy/asset_catalog.rs` | B | genre payload 带 recommended_methodology |
| `src-frontend/.../MethodologySettings.tsx` | B | 写入 canonical ID，UI 兼容 alias |
| `src-tauri/src/agents/service.rs` | B | Writer/Inspector 认 alias |
| `src-tauri/src/creative_engine/write_time_bundle.rs` | B | resolve 前 normalize |
| `src-tauri/src/agents/orchestrator.rs` | D | TriShot methodology 去重（可选） |
| `.claude/skills/sf-genesis-campaign/SKILL.md` 等 | D | 文档弱点段更新 |
| 测试 | A–C | 模板变量、normalize、brief、步进映射 |

---

## Phase A — P0 断链修复（优先，可单独发版）

> **状态：已实施（2026-07-09）** — 模板 + 契约测试 + 日志已落地。

### Task A1: 五个 background 生成模板补占位符

**Files:**
- Modify: `resources/prompts/creation/narrative_world_building_generate.md`
- Modify: `resources/prompts/creation/narrative_outline_generate.md`
- Modify: `resources/prompts/creation/narrative_character_generate.md`
- Modify: `resources/prompts/creation/narrative_scene_generate.md`
- Modify: `resources/prompts/creation/narrative_foreshadowing_generate.md`

- [ ] **Step 1: 对照 Rust 传入的变量名**

```bash
rg -n 'strategy_section|quartet_section|world_concept|outline_summary' \
  src-tauri/src/narrative/prompts.rs | head -60
```

确认 Generate 路径传入键（以代码为准）：
- world: `story_title`, `genre`, `story_description`, `strategy_section`, `quartet_section`
- outline: 查 `outline_prompt` 实际 keys
- character: 含 `world_concept`（外部化若写 `outline_summary` 作世界观则改模板或改代码键名，**二选一，优先改模板对齐代码**）
- scene / foreshadowing: 同上核对

- [ ] **Step 2: 每个 `.md` frontmatter `variables` 增加**

```yaml
  - strategy_section
  - quartet_section
```

- [ ] **Step 3: 正文在题材/简介之后插入**

```markdown
{{strategy_section}}
{{quartet_section}}
```

并在「要求」中加一条（与 fallback 一致）：

```text
必须遵循【创作策略参考】中的体裁画像、方法论等约束（若本节非空）。
```

注意：`strategy_section` 在 Rust 侧已包含 `\n【创作策略参考】\n...\n` 包装；模板不要再包一层标题，避免重复。若希望模板自带标题，则改 Rust 只传裸 notes——**本方案选：Rust 保持包装，模板只插 `{{strategy_section}}`**。

- [ ] **Step 4: character 模板变量对齐**

若代码传 `world_concept` 而 md 只有 `outline_summary`，把 md 改为：

```yaml
variables:
  - story_title
  - genre
  - world_concept
  - outline_summary
  - strategy_section
  - quartet_section
```

正文使用 `世界观：{{world_concept}}`。

- [ ] **Step 5: 冒烟渲染（无 LLM）**

在 `prompts.rs` 测试或临时：

```rust
let p = world_building_prompt(
    PromptMode::Generate,
    "标题",
    "末世",
    "简介",
    Some("应遵循的方法论：hero_journey\n英雄之旅..."),
    Some(r#"{"run_mode":"文戏"}"#),
    None, // 或 Some(pool) 若测试能加载 registry
);
assert!(p.contains("创作策略参考") || p.contains("应遵循的方法论"));
assert!(p.contains("hero_journey") || p.contains("英雄"));
```

用 `pool: None` 时走 fallback（已有占位符）；**必须再测 registry 路径**：`CARGO_MANIFEST_DIR` 下 resources 可加载时 `resolve_prompt_default` 非空。

---

### Task A2: 模板变量契约测试（防回归）

**Files:**
- Create or Modify: `src-tauri/src/narrative/prompts.rs` 的 `#[cfg(test)]` 模块
- 或 `src-tauri/src/prompts/registry.rs` 测试

- [ ] **Step 1: 写失败测试（先跑红）**

```rust
#[test]
fn background_generate_templates_declare_strategy_section() {
    let ids = [
        "narrative_world_building_generate",
        "narrative_outline_generate",
        "narrative_character_generate",
        "narrative_scene_generate",
        "narrative_foreshadowing_generate",
    ];
    for id in ids {
        let body = crate::prompts::registry::resolve_prompt_default(id)
            .unwrap_or_else(|| panic!("missing builtin {id}"));
        assert!(
            body.contains("{{strategy_section}}"),
            "{id} must include {{{{strategy_section}}}}"
        );
        assert!(
            body.contains("{{quartet_section}}"),
            "{id} must include {{{{quartet_section}}}}"
        );
    }
}
```

- [ ] **Step 2: 跑测试确认 A1 前红、A1 后绿**

```bash
cd src-tauri && cargo test --lib background_generate_templates_declare_strategy_section -- --nocapture
```

- [ ] **Step 3: 增加「传入非空 strategy 时渲染结果含关键词」测试**

对 `world_building_prompt` / `outline_prompt` / `character_prompt` / `scene_prompt` / `foreshadowing_prompt` 各测一条（`pool: None` 用 fallback 亦可；优先有 registry）。

- [ ] **Step 4: Commit**

```bash
git add resources/prompts/creation/narrative_*.md src-tauri/src/narrative/prompts.rs
git commit -m "$(cat <<'EOF'
fix: restore strategy_section in Genesis background prompts

Externalized v0.26.28 templates dropped methodology injection;
code still passed notes. Re-add placeholders + contract tests.
EOF
)"
```

---

### Task A3: 可观测日志（轻量）

**Files:**
- Modify: `src-tauri/src/narrative/genesis.rs` — `StrategySelectionStep`、`FirstChapterGenerationStep`、`ParallelWorldOutlineCharacterStep` 入口

- [ ] **Step 1: StrategySelection 完成后**

```rust
log::info!(
    "[Genesis] strategy selected: genre_profile_id={:?} methodology_id={:?} notes_preview_len={}",
    strategy.genre_profile_id,
    strategy.methodology_id,
    build_strategy_notes(ctx, &genre).len()
);
```

注意：此时 `ctx.selected_strategy` 已赋值后再算 notes。

- [ ] **Step 2: Background world 调用前**

```rust
log::info!(
    "[Genesis] background world prompt will include strategy_notes_len={}",
    strategy_notes.len()
);
```

- [ ] **Step 3: Commit** `chore: log methodology injection sizes in Genesis`

---

## Phase B — P1 一致性与骨架保真

### Task B1: `normalize_methodology_id`

**Files:**
- Modify: `src-tauri/src/domain/methodology.rs`（推荐）或新建 `strategy/methodology_id.rs`
- Modify: `genesis.rs` `resolve_methodology_prompt`
- Modify: `write_time_bundle.rs` methodology 分支
- Modify: `agents/service.rs` Writer/Inspector 映射
- Test: unit

- [ ] **Step 1: 实现**

```rust
pub fn normalize_methodology_id(id: &str) -> &str {
    match id.trim() {
        "world_building" | "hdwb" | "high_density_world_building" => {
            "high_density_world_building"
        }
        other => other,
    }
}
```

- [ ] **Step 2: 所有 resolve / match 入口先 normalize**

含：`resolve_methodology_prompt`、WriteTimeBundle、`build_writer_prompt`、Inspector、StrategySelection **写入 DB 前**（落库写 canonical）。

- [ ] **Step 3: 测试**

```rust
assert_eq!(normalize_methodology_id("world_building"), "high_density_world_building");
assert!(resolve_methodology_prompt("world_building", None).is_some());
```

（若 `resolve_methodology_prompt` 非 pub，测 `build_strategy_notes` 或把 normalize+resolve 提成 `pub(crate)`。）

- [ ] **Step 4: 前端设置页**

`MethodologySettings.tsx`：选项 value 改为 `high_density_world_building`；读取时若旧值 `world_building` 仍高亮 HDWB。

---

### Task B2: OpeningSkeleton 保方法论 brief

**Files:**
- Modify: `src-tauri/src/narrative/genesis.rs` — 新增 `build_methodology_brief` / 调整 `opening_skeleton_prompt` 调用
- Modify: `src-tauri/src/narrative/prompts.rs` — 骨架模板可改为双字段，或调用侧拼装后再截断

**设计（锁定）：** 调用侧组装，不依赖模板大改：

```rust
fn build_opening_strategy_notes(ctx: &GenesisContext, genre: &str) -> String {
    let method = build_methodology_brief(ctx); // 最多 400 字：标题+resolve 正文截断
    let genre_brief = build_genre_profile_brief(ctx); // 最多 350 字：名+core_tone+anti 前 3 条
    let merged = format!("{}\n{}", method, genre_brief);
    merged.chars().take(800).collect()
}
```

`build_methodology_brief`：若无 methodology，返回空；有则 `应遵循的方法论：{id}\n{content.chars().take(350)}`。

- [ ] **Step 1: 单测** — 人造超长 core_tone 的 profile + 长 methodology，断言结果含「应遵循的方法论」且 len≤800。

- [ ] **Step 2: OpeningSkeletonStep 改用 `build_opening_strategy_notes`**，首章仍用全文 `build_strategy_notes`。

- [ ] **Step 3: Commit** `fix: keep methodology brief in opening skeleton under 800-char cap`

---

### Task B3: Selector 预填 `recommended_methodology_id`

**Files:**
- Modify: `src-tauri/src/strategy/selector.rs` — `exact_genre_match` 或 match 后
- Modify: `src-tauri/src/strategy/asset_catalog.rs` — genre payload 增加字段
- Optional: 扩展 `seed_genre_recommendations` 覆盖更多题材（军事→scene_structure 等）— 单列子任务

- [ ] **Step 1: 在 exact/fuzzy 得到 `genre_profile_id` 后**

```rust
if strategy.methodology_id.is_none() {
    if let Some(repo) = genre_repo {
        if let Ok(Some(p)) = repo.get_by_id(id) {
            if let Some(m) = p.recommended_methodology_id {
                strategy.methodology_id = Some(normalize_methodology_id(&m).to_string());
                strategy.rationale.push_str("; methodology from genre recommendation");
            }
        }
    }
}
```

注意：`exact_genre_match` 当前无 `genre_repo`——在 `select_strategy` 步骤 1–2 之后、LLM 之前插入预填，或给 `exact_genre_match` 增加 repo 参数。

- [ ] **Step 2: LLM merge 仍可覆盖**（现有 `merge_strategies` 已是 LLM 优先）。

- [ ] **Step 3: 测试** — 无 LLM mock 时：构造 profile 带 `recommended_methodology_id=hero_journey`，只跑预填逻辑（可抽 `fn prefills_methodology_from_genre`）。

- [ ] **Step 4: Commit** `feat: prefill methodology from genre profile recommendation`

---

### Task B4: Genesis 落库 `methodology_step = Some(1)`

**Files:**
- Modify: `src-tauri/src/narrative/genesis.rs` StrategySelectionStep `UpdateStoryRequest`

- [ ] **Step 1: 将 `methodology_step: None` 改为 `Some(1)`**（确认 `UpdateStoryRequest` 类型为 `Option<i32>`）。

```bash
rg -n 'methodology_step' src-tauri/src/db/dto.rs src-tauri/src/db/models.rs | head
```

- [ ] **Step 2: 测试或日志断言** — 更新后 story.methodology_step == 1。

- [ ] **Step 3: Commit** `fix: persist methodology_step=1 on Genesis strategy selection`

---

## Phase C — P2 分步注入（需确认设计 §4）

> **Gate：** 用户确认设计文档 §4 步进映射（snowflake 结束→4，HDWB→2）后再开工。

### Task C1: `build_strategy_notes_for_step`

**Files:**
- Modify: `src-tauri/src/narrative/genesis.rs`

```rust
enum GenesisMethodStep {
    OpeningOrFirstChapter,
    World,
    Outline,
    Character,
    Scene,
    Foreshadow,
}

fn methodology_step_hint(id: &str, step: GenesisMethodStep) -> Option<&'static str> {
    let id = normalize_methodology_id(id);
    match (id, step) {
        ("snowflake", GenesisMethodStep::OpeningOrFirstChapter) => Some("1"),
        ("snowflake", GenesisMethodStep::Outline) => Some("2"), // 或组合 2+4 两次 resolve 拼接
        ("snowflake", GenesisMethodStep::Character) => Some("3"),
        ("snowflake", GenesisMethodStep::Scene) => Some("8"),
        ("high_density_world_building", GenesisMethodStep::World) => Some("1"),
        ("high_density_world_building", GenesisMethodStep::Outline)
        | ("high_density_world_building", GenesisMethodStep::Character) => Some("2"),
        _ => None, // 用默认全文 resolve(..., None)
    }
}
```

- [ ] **Step 1: 单测映射表**（纯函数）。

- [ ] **Step 2: `build_strategy_notes` 增加可选 `method_step: Option<&str>`，传给 `resolve_methodology_prompt`。**

- [ ] **Step 3: 各 background / first chapter 调用传入对应 hint。**

- [ ] **Step 4: Commit** `feat: inject step-specific methodology prompts in Genesis`

---

### Task C2: 创世结束写回 `methodology_step`

**Files:**
- Modify: `ContractSeedingStep` 末尾或 background 最后一步 / orchestrator genesis complete 钩子

```rust
fn final_methodology_step(methodology_id: &str) -> i32 {
    match normalize_methodology_id(methodology_id) {
        "snowflake" => 4,
        "high_density_world_building" => 2,
        _ => 1,
    }
}
```

- [ ] **Step 1: 在 background 全部成功（或 ContractSeeding 完成）后 update story。**

- [ ] **Step 2: 单测 `final_methodology_step`。**

- [ ] **Step 3: Commit** `feat: advance methodology_step after Genesis background`

---

### Task C3:（可选）Character 步叠加 `character_depth` 摘要

仅当主方法论不是 `character_depth` 时，追加一段 `resolve_methodology_prompt("character_depth", None)` 的 200 字 brief。避免 prompt 爆炸：总 notes 硬顶 4000 字。

- [ ] **Step 1: 实现 + 测试长度上限**
- [ ] **Step 2: Commit**（可与 C1 合并）

---

## Phase D — 文档、去重、清理

### Task D1: 更新技能与 ROADMAP 弱点描述

**Files:**
- Modify: `.claude/skills/sf-architecture-contract/SKILL.md` — 删除「策略仍在后台」；改为 background 模板已修 / 步进 Phase C
- Modify: `.claude/skills/sf-genesis-campaign/SKILL.md` — 标记战役「策略前移」已完成；新战役指向本 plan Phase C
- Modify: `ROADMAP.md` / `CHANGELOG.md`（发版时）
- Modify: `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md` 顶部加 supersede 链接（勿大段改历史）

- [ ] **Step 1: 改技能文件**
- [ ] **Step 2: 发版时写 CHANGELOG 条目引用审计+本 plan**

---

### Task D2: TriShot 方法论双通道去重（可选）

**Files:**
- Modify: `src-tauri/src/agents/orchestrator.rs` Call3 system guidance

- [ ] **Step 1: 若 `task.input` 已含「应遵循的方法论」，跳过 `render_selected_asset_guidance` 的 methodology 段。**

- [ ] **Step 2: 单测或日志验证 system 不再重复。**

---

### Task D3: 死资源处理

- [ ] **Step 1: `methodology_scene_self_check.md` 移至 `resources/prompts/archive/` 或删除。**
- [ ] **Step 2: `methodology_character_analysis.md` — 接线到 Character 步或 archive。**
- [ ] **Step 3: Commit** `chore: archive unused methodology prompts`

---

## 验证清单（每 Phase 结束）

```bash
cd src-tauri && cargo test --lib \
  background_generate_templates_declare_strategy_section \
  normalize_methodology \
  opening_strategy_notes \
  methodology_step_hint \
  final_methodology_step \
  -- --nocapture

cd src-tauri && cargo +nightly fmt -- --check
python3 scripts/architecture_guard.py
```

手工（A/B 后至少 1 次）：

1. 创世「写一部军事谍战的长篇小说」
2. 查日志：`methodology_id=`、`strategy_notes_len=`
3. 查 DB：`methodology_id`、`methodology_step`
4. 若有 prompt 落盘/调试：background world 请求含「创作策略参考」

发版门禁：走 `sf-change-control`；推送后按仓库规则监控 CI 至全绿。

---

## 建议执行顺序

```text
A1 → A2 → A3 →（可发 patch）
→ B1 → B2 → B4 → B3 →（可发 patch）
→ [用户确认 Design §4] → C1 → C2 → C3
→ D1 → D2 → D3
```

**最小「全面应用」定义（A+B 完成即宣称）：** 创世每条 LLM 步骤都能看到方法论约束；ID 一致；骨架不丢方法论；step 有明确初值。  
**「分步应用」定义（+C）：** 不同创世步骤吃到对应 snowflake/HDWB 子阶段，且续写步进接得上。
