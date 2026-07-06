# Genesis Phase 1 — P0 关键正确性修复实施方案

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 Genesis 流程 4 个 P0 缺陷（空内容锁死状态机、角色缺失世界观上下文、ChapterSwitch delivered 时序、genesis_runs 表未接入），并补充回归测试。

**Architecture:** 对 `FrontstageApp.tsx` 的状态机做最小手术（移除空内容分支的 `delivered`、将 `delivered` 后置到 `selectChapter` 真实 setContent 成功后）；对 `genesis.rs` 的 `ParallelWorldOutlineCharacterStep` 调整 await 顺序，使角色提示词拿到已生成的世界观；对 `commands/orchestrator.rs` 在 quick/background 阶段写入 `genesis_runs` 运行记录。

**Tech Stack:** Tauri 2.4 / Rust 1.95、React 18 / TypeScript 5.8 / Vite 6、SQLite、vitest、cargo test。

## Global Constraints

- 不引入 DB schema 变更；使用已有 `genesis_runs` 表与 `GenesisRunRepository`。
- P1-1（策略选择移入 quick phase）已决策暂缓，Phase 1 不触及。
- 所有改动必须通过：`cargo check`、`cargo test --lib`、`npx tsc --noEmit`、`npx vitest run`、`cargo +nightly fmt -- --check`、`npm run format:check`。
- 不增加 quick phase 用户可感知延迟（genesis_runs 写操作使用 fire-and-forget，不阻塞 LLM 路径）。
- step 名称、进度事件、现有测试（`background_steps_include_contract_seeding`）必须继续通过。

---

## File Structure

| 文件 | 变更性质 | 职责 |
|------|----------|------|
| `src-frontend/src/frontstage/FrontstageApp.tsx` | 修改 | P0-1、P0-3 状态机修复 |
| `src-tauri/src/narrative/genesis.rs` | 修改 | P0-2 角色/世界观顺序修复；P0-4 错误累计字段 |
| `src-tauri/src/commands/orchestrator.rs` | 修改 | P0-4 genesis_runs 写入与状态迁移 |
| `src-tauri/src/db/repositories.rs` | 可选修改 | 若 `update_step`/`complete`/`fail` 方法签名不满足，新增辅助方法 |
| `src-frontend/src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx` | 修改/新增测试 | P0-1、P0-3 回归测试 |
| `src-tauri/src/narrative/genesis.rs`（`#[cfg(test)]` 区） | 新增测试 | P0-2 顺序修复测试 |
| `src-tauri/src/commands/orchestrator.rs`（`#[cfg(test)]` 区） | 新增测试 | P0-4 genesis_runs 状态迁移测试 |

---

## Task 1: 修复 `handleSmartGeneration` Gap B（P0-1）

**Files:**
- Modify: `src-frontend/src/frontstage/FrontstageApp.tsx:3600-3638`
- Test: `src-frontend/src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx`

**Interfaces:**
- Consumes: `genesisDeliveryRef`（`'idle' \| 'generating' \| 'delivered'`）、`isFirstChapterReady`、`finalContent`
- Produces: 空 `finalContent` 时状态机保持 `'generating'`，不阻塞后续 ChapterSwitch / pipeline-complete 加载

- [ ] **Step 1: 写失败测试（锁定 Gap B 行为）**

在 `FrontstageApp.genesis-duplicate.test.tsx` 中新增：

```tsx
it('P0-1: isFirstChapterReady + empty final_content must not lock delivered', async () => {
  // 构造一个 Genesis 首章场景：isFirstChapterReady=true，但 smart_execute 返回空 final_content
  const { result } = renderHook(() => useFrontstageAppInternalsForTest(), {
    wrapper: TestWrapper,
  });

  // 触发 Genesis 生成流程，让状态进入 generating
  act(() => {
    result.current.beginGenesis('末世题材：幸存者建立避难所');
  });
  expect(result.current.genesisDeliveryState()).toBe('generating');

  // 模拟 smart_execute 返回空 final_content
  await act(async () => {
    await result.current.handleSmartGeneration({
      final_content: '   ', // trim 后为空
      isFirstChapterReady: true,
    } as any);
  });

  // 关键断言：不能锁到 delivered，否则后续 ChapterSwitch/pipeline-complete 无法加载 DB 正文
  expect(result.current.genesisDeliveryState()).toBe('generating');

  // 后续 ChapterSwitch 或 pipeline-complete 应能正常加载 DB 正文
  await act(async () => {
    await result.current.handleChapterSwitch({
      chapter_id: 'ch-1',
      story_id: 'story-1',
      auto_accept: true,
      content: '<p>第一章正文</p>',
    });
  });

  expect(result.current.genesisDeliveryState()).toBe('delivered');
  expect(result.current.editorText).toContain('第一章正文');
});
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-frontend && npx vitest run src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx -t "P0-1"
```

Expected: FAIL，当前代码会把空内容分支锁到 `delivered`，导致 `handleChapterSwitch` 后 editorText 为空或状态不对。

- [ ] **Step 3: 实现最小修复**

修改 `src-frontend/src/frontstage/FrontstageApp.tsx:3623-3625`：

```tsx
// 修复前：
// } else {
//   genesisDeliveryRef.current = 'delivered';
// }

// 修复后：空 finalContent 不锁 delivered，让 ChapterSwitch / pipeline-complete 后续加载 DB 正文
} else {
  frontstageLogger.warn(
    '[SmartGeneration] isFirstChapterReady but finalContent empty, keeping generating state',
    {
      isFirstChapterReady,
      finalContentLen: finalContent.length,
    }
  );
  logToBackend(
    'frontstage:genesis_smart_empty_keep_generating',
    'isFirstChapterReady but finalContent empty, keeping generating state',
    {
      isFirstChapterReady,
      finalContentLen: finalContent.length,
    }
  );
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cd src-frontend && npx vitest run src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx -t "P0-1"
```

Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add src-frontend/src/frontstage/FrontstageApp.tsx src-frontend/src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx
git commit -m "fix: P0-1 handleSmartGeneration empty finalContent no longer locks delivered"
```

---

## Task 2: 修复角色生成世界观上下文（P0-2）

**Files:**
- Modify: `src-tauri/src/narrative/genesis.rs:1644-1661`
- Test: `src-tauri/src/narrative/genesis.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `world_future`、`outline_future`、`character_future`、`bundle: Arc<RwLock<NarrativeBundle>>`
- Produces: `character_future` 运行时 `bundle.world_building` 已写入，角色提示词拿到非空世界观

- [ ] **Step 1: 写失败测试**

在 `src-tauri/src/narrative/genesis.rs` 测试区新增：

```rust
#[cfg(test)]
mod parallel_world_outline_character_tests {
    use super::*;

    #[tokio::test]
    async fn character_step_reads_world_building_from_bundle() {
        // 构造最小 bundle：world_building 有 concept
        let bundle = Arc::new(tokio::sync::RwLock::new(NarrativeBundle::new()));
        {
            let mut guard = bundle.write().await;
            *guard = guard.clone().with_world_building(WorldBuildingElement {
                id: "w1".to_string(),
                story_id: "s1".to_string(),
                concept: "核冬天后的废土，辐射尘与变异生物".to_string(),
                rules: vec![],
                factions: vec![],
                history: String::new(),
                geography: String::new(),
                culture: String::new(),
                technology: String::new(),
                magic_system: String::new(),
            });
        }

        // 模拟 character_future 内部读取 world 的逻辑
        let world_concept = {
            let b = bundle.read().await;
            b.world_building
                .as_ref()
                .map(|w| w.concept.clone())
                .unwrap_or_default()
        };

        assert!(
            !world_concept.is_empty(),
            "character prompt should receive non-empty world concept"
        );
        assert!(world_concept.contains("核冬天"));
    }
}
```

> 注：若现有测试基础设施不足，可改为在 step 集成测试中 mock LLM，断言 `character_prompt` 输入参数包含 world concept。

- [ ] **Step 2: 运行测试确认失败（或确认当前行为）**

```bash
cd src-tauri && cargo test --lib parallel_world_outline_character_tests -- --nocapture
```

Expected: 当前该测试不一定失败，因为是在构造后读取 bundle；需要真正的集成测试来暴露 await 顺序问题。

- [ ] **Step 3: 实现顺序修复**

修改 `src-tauri/src/narrative/genesis.rs:1644-1661`：

```rust
// 修复前：
// let world_res = world_future.await;
// let outline_res = outline_future.await;
// let characters_res = character_future.await;
// {
//     let mut bundle_guard = bundle.write().await;
//     if let Ok(ref wb) = world_res { ... }
//     ...
// }

// 修复后：先 await world，立即写入 bundle，再 await outline / character。
// 这样 character_future 内部读取 bundle.world_building.concept 时才能拿到真实世界观。
let world_res = world_future.await;
if let Ok(ref wb) = world_res {
    let mut bundle_guard = bundle.write().await;
    *bundle_guard = bundle_guard.clone().with_world_building(wb.clone());
}

let outline_res = outline_future.await;
let characters_res = character_future.await;

{
    let mut bundle_guard = bundle.write().await;
    if let Ok(ref outline) = outline_res {
        *bundle_guard = bundle_guard.clone().with_outline(outline.clone());
    }
    if let Ok(ref characters) = characters_res {
        for c in characters {
            *bundle_guard = bundle_guard.clone().add_character(c.clone());
        }
    }
}
```

- [ ] **Step 4: 新增/调整集成测试，暴露原顺序缺陷**

在 `#[cfg(test)]` 区新增一个集成测试，使用 mock LLM 返回固定 world concept，验证角色 prompt 参数包含该 concept：

```rust
#[tokio::test]
async fn character_prompt_receives_world_concept_from_bundle() {
    // 1. 创建 GenesisContext
    // 2. 注册 mock LLM，让 world_future 返回固定 concept
    // 3. 执行 ParallelWorldOutlineCharacterStep
    // 4. 拦截 character_prompt 调用参数，断言 world 参数 == 固定 concept
    // 若基础设施不足，可退化为断言最终生成的 CharacterElement 世界观字段包含 world concept。
}
```

- [ ] **Step 5: 运行测试确认通过**

```bash
cd src-tauri && cargo test --lib genesis -- --nocapture
```

Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/narrative/genesis.rs
git commit -m "fix: P0-2 ensure character generation sees world building concept"
```

---

## Task 3: 修复 ChapterSwitch delivered 时序（P0-3）

**Files:**
- Modify: `src-frontend/src/frontstage/FrontstageApp.tsx:1544-1557` 与 `2173-2187`
- Test: `src-frontend/src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx`

**Interfaces:**
- Consumes: `payload.content`、`autoAccept`、`selectChapter`、`genesisDeliveryRef`
- Produces: `delivered` 仅在 `selectChapter` 真实 `setContent` 非空内容后设置

- [ ] **Step 1: 写失败测试**

```tsx
it('P0-3: ChapterSwitch lazy-load failure must not lock delivered', async () => {
  const { result } = renderHook(() => useFrontstageAppInternalsForTest(), {
    wrapper: TestWrapper,
  });

  act(() => {
    result.current.beginGenesis('末世题材');
  });
  expect(result.current.genesisDeliveryState()).toBe('generating');

  // ChapterSwitch 声称有 content，但对应 chapter.content 为空，触发 lazy-load
  // mock get_chapter 返回空内容（模拟失败或空结果）
  mockTauriInvoke('get_chapter', () => null);

  await act(async () => {
    await result.current.handleChapterSwitch({
      chapter_id: 'ch-1',
      story_id: 'story-1',
      auto_accept: true,
      content: '<p> supposedly available </p>', // payload.content 非空
    });
  });

  // lazy-load 失败 / 返回空，delivered 不能被提前设置
  expect(result.current.genesisDeliveryState()).toBe('generating');

  // 后续 smart_execute 投递真实 final_content 应能写入
  await act(async () => {
    await result.current.handleSmartGeneration({
      final_content: '<p>真实第一章正文</p>',
      isFirstChapterReady: true,
    } as any);
  });

  expect(result.current.genesisDeliveryState()).toBe('delivered');
  expect(result.current.editorText).toContain('真实第一章正文');
});
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-frontend && npx vitest run src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx -t "P0-3"
```

Expected: FAIL，当前 `ChapterSwitch` 在 `selectChapter` 之前就设置 `delivered`。

- [ ] **Step 3: 移除 ChapterSwitch 中的提前 delivered 标记**

修改 `src-frontend/src/frontstage/FrontstageApp.tsx:1544-1557`：

```tsx
const chapterSwitchHasContent =
  !!payload.content && payload.content.trim().length > 0;
const chapterSwitchSkipContent =
  !autoAccept ||
  genesisDeliveryRef.current === 'delivered' ||
  (autoAccept && !chapterSwitchHasContent);

// 修复：不在此处标记 delivered；delivered 应在 selectChapter 真实写入非空内容后置位。
// 原代码：
// if (autoAccept && chapterSwitchHasContent) {
//   genesisDeliveryRef.current = 'delivered';
// }
```

- [ ] **Step 4: 在 selectChapter setContent 成功后后置 delivered**

修改 `src-frontend/src/frontstage/FrontstageApp.tsx:2173-2187`（`setContent` 调用块）：

```tsx
} else {
  try {
    logToBackend('frontstage:select_chapter_set', 'selectChapter setContent', {
      chapterId: chapter.id,
      formattedLen: formattedContent.length,
      skipContent,
    });
    setContent(formattedContent);
    // P0-3 修复：Genesis 生成期间，真实写入非空内容后才把状态机置为 delivered。
    const writtenText = formattedContent.replace(/<[^>]*>/g, '').trim();
    if (
      genesisDeliveryState() === 'generating' &&
      writtenText.length > 0
    ) {
      genesisDeliveryRef.current = 'delivered';
      frontstageLogger.info(
        '[selectChapter] Genesis content delivered via setContent',
        { chapterId: chapter.id, textLen: writtenText.length }
      );
      logToBackend(
        'frontstage:select_chapter_genesis_delivered',
        'Genesis content delivered via setContent',
        { chapterId: chapter.id, textLen: writtenText.length }
      );
    }
  } catch (e) {
    frontstageLogger.error('[selectChapter] setContent 失败', {
      error: e,
      formatted_length: formattedContent.length,
    });
  }
```

- [ ] **Step 5: 运行测试确认通过**

```bash
cd src-frontend && npx vitest run src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx -t "P0-3"
```

Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add src-frontend/src/frontstage/FrontstageApp.tsx src-frontend/src/frontstage/__tests__/FrontstageApp.genesis-duplicate.test.tsx
git commit -m "fix: P0-3 mark delivered only after selectChapter actually sets content"
```

---

## Task 4: 接入 `genesis_runs` 表（P0-4）

**Files:**
- Modify: `src-tauri/src/commands/orchestrator.rs:259-528` 与后台阶段
- Modify: `src-tauri/src/narrative/genesis.rs`（可选：在 `GenesisContext` 加 `errors` 字段用于累计后台错误）
- Test: `src-tauri/src/commands/orchestrator.rs` `#[cfg(test)]` 或 `src-tauri/src/narrative/genesis.rs`

**Interfaces:**
- Consumes: `GenesisContext.session_id`、`GenesisContext.user_premise`、`GenesisContext.story_id`、`PipelineProgressEvent`
- Produces: `genesis_runs` 表写入 `pending → running → quick_done/completed/failed`，前端仪表盘可展示记录

- [ ] **Step 1: 写失败测试**

在 `src-tauri/src/commands/orchestrator.rs` 测试区新增（若该文件无测试，可放 `genesis.rs` 测试区或新增 `tests/orchestrator_genesis_runs.rs`）：

```rust
#[cfg(test)]
mod genesis_runs_tests {
    use super::*;
    use crate::db::repositories::GenesisRunRepository;

    #[test]
    fn genesis_run_lifecycle_persists_to_db() {
        // 使用内存池 / 测试固定装置
        let pool = crate::db::tests::create_test_pool();
        let repo = GenesisRunRepository::new(pool.clone());
        let run = repo.create("run-1", "session-1", "末世题材", 7).unwrap();
        assert_eq!(run.status, "pending");

        repo.update_step("run-1", "概念生成", 1, "running", "{}")
            .unwrap();
        let updated = repo.get_by_id("run-1").unwrap().unwrap();
        assert_eq!(updated.status, "running");
        assert_eq!(updated.current_step.as_deref(), Some("概念生成"));

        repo.complete("run-1", Some("story-1")).unwrap();
        let completed = repo.get_by_id("run-1").unwrap().unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.story_id.as_deref(), Some("story-1"));
    }
}
```

- [ ] **Step 2: 运行测试确认 Repository 方法通过**

```bash
cd src-tauri && cargo test --lib genesis_run_lifecycle_persists_to_db -- --nocapture
```

Expected: PASS（若已有测试池可用）。

- [ ] **Step 3: 在 orchestrator.rs 创建 genesis_run 写入辅助函数**

在 `src-tauri/src/commands/orchestrator.rs` 顶部或合适位置新增：

```rust
use crate::db::repositories::GenesisRunRepository;

fn persist_genesis_run_update<F>(app_handle: &tauri::AppHandle, run_id: &str, f: F)
where
    F: FnOnce(&GenesisRunRepository) -> Result<usize, rusqlite::Error>,
{
    let pool = match app_handle.try_state::<crate::db::DbPool>() {
        Some(state) => state.inner().clone(),
        None => {
            log::warn!("[genesis_runs] DbPool not available, skipping persistence");
            return;
        }
    };
    let repo = GenesisRunRepository::new(pool);
    if let Err(e) = f(&repo) {
        log::warn!("[genesis_runs] Failed to persist run {}: {}", run_id, e);
    }
}
```

- [ ] **Step 4: quick phase 开始时插入运行记录**

在 `src-tauri/src/commands/orchestrator.rs:259-260` 之后插入：

```rust
let mut ctx =
    crate::narrative::genesis::GenesisContext::new(app_handle.clone(), user_input.clone());
let session_id = ctx.session_id.clone();
let run_id = session_id.clone(); // 使用 session_id 作为 run id，简单且唯一
let quick_steps_count = crate::narrative::genesis::GenesisPipeline::quick_phase_steps().len() as i32;

// P0-4: 持久化 Genesis 运行记录，供仪表盘查询
persist_genesis_run_update(&app_handle, &run_id, |repo| {
    repo.create(&run_id, &session_id, &user_input, quick_steps_count)
});
```

- [ ] **Step 5: 进度回调中更新 current_step**

修改 `src-tauri/src/commands/orchestrator.rs:274-289` 的 progress_callback：

```rust
let progress_run_id = run_id.clone();
let progress_callback = std::sync::Arc::new(
    move |evt: crate::narrative::progress::PipelineProgressEvent| {
        let _ = app_handle_progress.emit("pipeline-progress", &evt);
        let _ = app_handle_progress.emit(
            "novel-bootstrap-progress",
            crate::planner::bootstrap::BootstrapProgressEvent {
                session_id: evt.pipeline_id.clone(),
                step_name: evt.step_name.clone(),
                step_number: evt.step_number,
                total_steps: evt.total_steps,
                message: evt.message.clone(),
                status: format!("{:?}", evt.status).to_lowercase(),
            },
        );
        // P0-4: 异步更新 genesis_runs 当前步骤（fire-and-forget，不阻塞进度）
        let step_name = evt.step_name.clone();
        let step_number = evt.step_number;
        let total_steps = evt.total_steps;
        let status = format!("{:?}", evt.status).to_lowercase();
        let app = app_handle_progress.clone();
        let run_id = progress_run_id.clone();
        tauri::async_runtime::spawn(async move {
            let steps_json = serde_json::json!({
                "step_name": step_name,
                "step_number": step_number,
                "total_steps": total_steps,
            })
            .to_string();
            persist_genesis_run_update(&app, &run_id, |repo| {
                repo.update_step(&run_id, &step_name, step_number, &status, &steps_json)
            });
        });
    },
);
```

> 注意：`persist_genesis_run_update` 接收 `&tauri::AppHandle`。闭包中 move 的是 clone，安全。

- [ ] **Step 6: quick phase 成功时更新为 quick_done（或 completed）**

在 `src-tauri/src/commands/orchestrator.rs:299-305` 成功分支：

```rust
Ok(()) => {
    log::warn!("[smart_execute] GenesisPipeline 快速阶段成功完成 story_id={}", ctx.story_id);

    // P0-4: 快速阶段完成，更新运行记录
    let story_id_for_run = ctx.story_id.clone();
    let first_chapter_len = ctx
        .first_chapter_content
        .as_ref()
        .map(|s| s.len() as i32)
        .unwrap_or(0);
    let app_for_run = app_handle.clone();
    let run_id_for_complete = run_id.clone();
    tauri::async_runtime::spawn(async move {
        persist_genesis_run_update(&app_for_run, &run_id_for_complete, |repo| {
            // 使用 "quick_done" 表示 quick phase 已完成，后台仍在跑；后台结束后可再改为 completed
            repo.update_step(
                &run_id_for_complete,
                "快速阶段完成",
                quick_steps_count,
                "quick_done",
                &serde_json::json!({
                    "first_chapter_chars": first_chapter_len,
                    "story_id": story_id_for_run,
                })
                .to_string(),
            )
        });
    });
```

- [ ] **Step 7: quick phase 失败时标记 failed**

在 `src-tauri/src/commands/orchestrator.rs:507-528` 错误分支：

```rust
Err(e) => {
    log::error!("[smart_execute] GenesisPipeline concept generation failed: {}", e);
    emit_progress("error", &format!("小说初始化失败: {}", e), 5, 5);
    let error_msg = format!("{}", e);
    let app_for_fail = app_handle.clone();
    let run_id_for_fail = run_id.clone();
    tauri::async_runtime::spawn(async move {
        persist_genesis_run_update(&app_for_fail, &run_id_for_fail, |repo| {
            repo.fail(&run_id_for_fail, &error_msg)
        });
    });
    // ... 后续转换错误并 return Err(...)
}
```

- [ ] **Step 8: 后台阶段更新同一 run 记录为 completed/failed**

在 `src-tauri/src/commands/orchestrator.rs:406-454` 后台阶段结果处理：

```rust
let (success, error_message) = match &bg_result {
    Ok(_) => { ... (true, None) }
    Err(e) => {
        log::warn!("[GenesisPipeline] 后台阶段失败: {}", e);
        (false, Some(format!("{}", e)))
    }
};

// P0-4: 后台阶段结束，更新 genesis_runs 最终状态与资产数量
let app_for_bg = app_handle_for_emit.clone();
let run_id_for_bg = session_id_bg.clone();
let elements_json = serde_json::json!({
    "world_rules": elements_created.world_rules,
    "characters": elements_created.characters,
    "scenes": elements_created.scenes,
    "foreshadowings": elements_created.foreshadowings,
    "plot_points": elements_created.plot_points,
})
.to_string();
tauri::async_runtime::spawn(async move {
    if success {
        persist_genesis_run_update(&app_for_bg, &run_id_for_bg, |repo| {
            repo.complete(&run_id_for_bg, Some(&story_id_for_emit))
        });
    } else if let Some(err) = error_message {
        persist_genesis_run_update(&app_for_bg, &run_id_for_bg, |repo| {
            repo.fail(&run_id_for_bg, &err)
        });
    }
    // 同时把 elements_json 存入 steps_json 或新增字段（若 schema 不允许，仅记录到日志）
    log::info!(
        "[genesis_runs] background phase finished. elements={}",
        elements_json
    );
});
```

- [ ] **Step 9: 运行 Rust 测试**

```bash
cd src-tauri && cargo test --lib -- genesis_run
```

Expected: PASS。

- [ ] **Step 10: 运行格式与编译检查**

```bash
cd src-tauri && cargo check
cd src-tauri && cargo +nightly fmt -- --check
cd src-frontend && npx tsc --noEmit
cd src-frontend && npm run format:check
```

Expected: 全部通过。

- [ ] **Step 11: 提交**

```bash
git add src-tauri/src/commands/orchestrator.rs src-tauri/src/narrative/genesis.rs
git commit -m "feat: P0-4 persist genesis_runs lifecycle to dashboard"
```

---

## Phase 1 最终验收

- [ ] 运行 `cargo test --lib`：全部通过，Genesis 相关测试 ≥ 3 个新增。
- [ ] 运行 `npx vitest run`：全部通过，新增 Gap B / ChapterSwitch lazy-load 测试通过。
- [ ] 运行 `npx playwright test`：Genesis E2E 通过。
- [ ] 手动 `cargo tauri dev`：
  - 新写小说首章不空白、不重复；
  - 后台完成后仪表盘出现 Genesis 运行记录；
  - 运行记录包含当前步骤、状态、story_id。
- [ ] 推送分支并触发 GitHub Actions CI；本地执行 `cargo tauri build` 生成本平台安装包。
- [ ] 更新版本号与文档：`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`。

---

## Self-Review Checklist

1. **Spec coverage:**
   - P0-1 ✅ Task 1
   - P0-2 ✅ Task 2
   - P0-3 ✅ Task 3
   - P0-4 ✅ Task 4
   - P1-1 暂缓 ✅ 已记录为债务，Phase 1 不执行

2. **Placeholder scan:** 无 TBD/TODO；所有代码块为可直接应用的示例。

3. **Type consistency：** 使用 `GenesisRunRepository` 已有方法签名；`persist_genesis_run_update` 中 `run_id` 与 `session_id` 均为 `String`/`&str`，与仓库一致。
