# Genesis 人物卡强制落地 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 创世首章必须落地具体主角姓名，并让读者开篇就知道主角要什么、阻力是什么；消除泛称「主角」与「戏空」；零新增 LLM；quick p95 仍 ≤90s。

**Architecture:** 纯函数 `merge_protagonist_card` 合并骨架∪概念 → 短模板双重注入（first_scene Critical + TriShot Call3 尾注）→ 规则探针（真名 + 欲望信号 + 阻力信号）→ 预算内一次 soft retry。fail-open。

**Tech Stack:** Rust GenesisPipeline / TriShot orchestrator / PromptRegistry `.md` / cargo 契约测试 / `creative_workflow.log`

**Design doc:** `docs/plans/2026-07-09-genesis-protagonist-card-design.md`（含「冲突与目标清晰」修订）

---

## File map

| File | Responsibility |
|------|----------------|
| Create: `src-tauri/src/narrative/protagonist_card.rs` | `ProtagonistCard`、merge/render/probe 纯函数 + 单测 |
| Modify: `src-tauri/src/narrative/mod.rs` | `mod protagonist_card; pub use ...` |
| Modify: `resources/prompts/creation/narrative_first_scene_generate.md` | 新增 `{{protagonist_card}}` 变量与 Critical 段位 |
| Modify: `src-tauri/src/narrative/prompts.rs` | `first_scene_prompt` 增加 `protagonist_card` 参数 |
| Modify: `src-tauri/src/narrative/genesis.rs` | FirstChapter：merge → 注入 prompt + `task.parameters`；生成后 probe/soft-retry |
| Modify: `src-tauri/src/agents/orchestrator.rs` | Call3：若 parameters 有卡，在 `NOVEL_OUTPUT_DISCIPLINE` 前追加 |
| Docs of record + bump | 发布切片（建议 v0.26.45） |

**Out of scope:** 续写 TimeSliced、扩骨架 JSON、quality_gate 热路径、改 delivered 状态机。

---

### Task 1: 纯函数模块 + 失败测试

**Files:**
- Create: `src-tauri/src/narrative/protagonist_card.rs`
- Modify: `src-tauri/src/narrative/mod.rs`

- [ ] **Step 1: 写失败测试（TDD）**

在 `protagonist_card.rs` 的 `#[cfg(test)]` 中写：

```rust
#[test]
fn merge_prefers_skeleton_name_over_concept() {
    // skeleton name "林深", meta name "张三" → "林深"
}

#[test]
fn merge_filters_generic_protagonist_label() {
    // skeleton name "主角", meta name "林深" → "林深"
    // both generic → None
}

#[test]
fn merge_fills_desire_obstacle_scene_goal_from_skeleton() {
    // skeleton.goal / obstacle / scene.dramatic_goal 写入 card
}

#[test]
fn render_omits_empty_optional_lines() {
    // wound empty → 渲染串不含「旧伤」
    // 渲染串含「本场欲望」与「本场阻力」纪律句
}

#[test]
fn probe_detects_name_desire_obstacle_signals() {
    // name in content → name_hit
    // desire keyword in content → desire_hit
    // obstacle keyword in content → obstacle_hit
    // 「主角」→ generic_label_hit
}

#[test]
fn soft_retry_trigger_when_goal_and_obstacle_both_miss() {
    // name_hit 但 desire+obstacle 均非空且双 miss → should_soft_retry == true
    // name_hit 且 desire_hit → should_soft_retry == false
}
```

- [ ] **Step 2: 跑测试确认红**

```bash
cd src-tauri && cargo test --lib narrative::protagonist_card -- --nocapture
```

Expected: compile fail or test fail（模块尚未实现）。

- [ ] **Step 3: 实现最小 API**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtagonistCard {
    pub name: String,
    pub desire: Option<String>,
    pub wound: Option<String>,
    pub obstacle: Option<String>,
    pub scene_goal: Option<String>,
    pub source: &'static str, // "skeleton" | "concept" | "mixed"
}

pub fn merge_protagonist_card(
    meta: &StoryMetaElement,
    skeleton: Option<&OpeningSkeleton>,
) -> Option<ProtagonistCard>;

pub fn render_protagonist_card(card: &ProtagonistCard) -> String;

pub fn probe_protagonist_card(content: &str, card: &ProtagonistCard) -> ProtagonistProbeResult;

pub fn should_soft_retry_protagonist_card(probe: &ProtagonistProbeResult, card: &ProtagonistCard) -> bool;

#[derive(Debug, Clone)]
pub struct ProtagonistProbeResult {
    pub name_hit: bool,
    pub generic_label_hit: bool,
    pub desire_hit: bool,    // 无 desire/scene_goal 时为 true（不计入失败）
    pub obstacle_hit: bool,  // 无 obstacle 时为 true（不计入失败）
}
```

规则：
- `is_generic_name(s)`：trim 后 ∈ {主角, 男主, 女主} 或空
- desire/obstacle/scene_goal 空串 → None
- render：含姓名 + 欲望/目标 + 阻力 + 双纪律（辨识度 + 冲突目标清晰）；wound 空则省略行；总长约 100–160 字
- `desire_hit`：对 `desire.or(scene_goal)` 抽内容词做子串匹配；字段过短则跳过（记为 hit=true）
- `should_soft_retry`：`!name_hit` OR（desire 与 obstacle 均 Some 且双 miss）

- [ ] **Step 4: 跑测试确认绿**

```bash
cd src-tauri && cargo test --lib narrative::protagonist_card -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/narrative/protagonist_card.rs src-tauri/src/narrative/mod.rs
git commit -m "$(cat <<'EOF'
feat: add genesis ProtagonistCard merge/render/probe pure functions

EOF
)"
```

---

### Task 2: first_scene 模板与 prompt 签名

**Files:**
- Modify: `resources/prompts/creation/narrative_first_scene_generate.md`
- Modify: `src-tauri/src/narrative/prompts.rs`（`first_scene_prompt`）
- Modify: 所有 `first_scene_prompt(` 调用点（主要 `genesis.rs`）

- [ ] **Step 1: 模板加变量**

在 frontmatter `variables` 增加 `protagonist_card`。  
在「【当前场景的戏剧任务】」**之前**插入：

```markdown
{{protagonist_card}}
```

（空串时无额外空白膨胀——调用方保证无卡时传 `""`。）

- [ ] **Step 2: 扩展 `first_scene_prompt` 签名**

增加参数 `protagonist_card: &str`，加入 `vars` 列表。fallback 字符串可忽略该段。

- [ ] **Step 3: 更新调用点编译通过**

```bash
cd src-tauri && cargo check 2>&1 | tail -30
```

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: inject protagonist_card slot into first_scene prompt

EOF
)"
```

---

### Task 3: Genesis FirstChapter 接线

**Files:**
- Modify: `src-tauri/src/narrative/genesis.rs`（`FirstChapterGenerationStep`）

- [ ] **Step 1: merge + 注入**

在构建 `chapter_prompt` 前：

```rust
let card = merge_protagonist_card(&meta, ctx.opening_skeleton.as_ref());
let card_text = card.as_ref().map(render_protagonist_card).unwrap_or_default();
```

- 传入 `first_scene_prompt(..., &card_text, ...)`
- `parameters.insert("protagonist_card", Value::String(card_text.clone()))`
- 保留现有 `placeholder_protagonist_name/goal`（与卡一致）
- 打日志 `genesis.protagonist_card.merged`

- [ ] **Step 2: 生成后探针（在 8% 自重复闸门之后或并列）**

若 `card` 存在：

```rust
let probe = probe_protagonist_card(&result.content, &card);
// log genesis.protagonist_card.probe — name_hit/desire_hit/obstacle_hit/generic_label_hit
```

若 `should_soft_retry_protagonist_card(&probe, &card)` 且尚未因自重复消耗过额外 Call3：

追加指令示例：
`必须使用姓名「{name}」；开场用行动体现要「{desire}」、被「{obstacle}」所阻；禁止用「主角」指代`

重试一次；采用探针综合更好的版本（优先 name_hit，其次 desire+obstacle 命中数）。

- [ ] **Step 3: 契约测试**

扩展 genesis 测试：有 skeleton+meta 时 `merge` 非空且含 desire/obstacle；无有效名时 `render` 为空不注入；`should_soft_retry` 边界。

- [ ] **Step 4: 跑测试**

```bash
cd src-tauri && cargo test --lib narrative:: -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: wire ProtagonistCard into genesis first chapter

EOF
)"
```

---

### Task 4: TriShot Call3 尾注

**Files:**
- Modify: `src-tauri/src/agents/orchestrator.rs`（`execute_trishot`，`NOVEL_OUTPUT_DISCIPLINE` 追加前）

- [ ] **Step 1: 读取 parameters**

```rust
if let Some(card) = task.parameters.get("protagonist_card").and_then(|v| v.as_str()) {
    if !card.trim().is_empty() {
        final_prompt.push_str("\n\n");
        final_prompt.push_str(card);
    }
}
```

放在 `NOVEL_OUTPUT_DISCIPLINE` **之前**。

- [ ] **Step 2: 确认非 Genesis 路径无卡时行为不变**（parameters 无键 → 无追加）

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: append protagonist_card before TriShot Call3 discipline

EOF
)"
```

---

### Task 5: 验证、文档、发布门

**Files:** docs of record + version bump（建议 **v0.26.45**）

- [ ] **Step 1: 全量验证**

```bash
cd src-tauri && cargo test --lib
cd src-tauri && cargo +nightly fmt -- --check
cd ../src-frontend && npx tsc --noEmit && npx vitest run && npm run format:check
cd .. && python3 scripts/architecture_guard.py
```

- [ ] **Step 2: 更新文档**

`CHANGELOG.md` / `AGENTS.md` / `PROJECT_STATUS.md` / `ROADMAP.md` / `ARCHITECTURE.md` / `TESTING.md` / `README.md` / `docs/USER_GUIDE.md`（一句：创世首章强制落地人物卡——姓名 + 开篇欲望/阻力清晰）。

- [ ] **Step 3: bump 四源版本 → commit → tag → push → 监控 CI 至全绿**

遵循 `sf-change-control` + post-push CI 规则。本地 `cargo tauri build`。

- [ ] **Step 4: 发布后对照**

用「新写一部末世生存的长篇小说」跑 1–3 次，查 `genesis.protagonist_card.probe` 的 `name_hit`。

---

## Risk notes

| Risk | Mitigation |
|------|------------|
| 双重注入仍被模型忽略 | soft retry + 观测；不阻断交付 |
| 与 8% 重试抢预算 | 共享「最多一次额外 Call3」 |
| prompt 膨胀 | 卡 ≤120 字；无卡传空串 |
| GitNexus | 改 `FirstChapterGenerationStep` / `execute_trishot` 前跑 `gitnexus_impact` |

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-09-genesis-protagonist-card.md`.

**两选一：**

1. **Subagent-driven（推荐）** — 按 task 派生子代理，每 task 后 review  
2. **Inline** — 本会话按 task 执行，大批量检查点  

你更想用哪一种？
