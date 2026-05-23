# StoryForge 系统性差距修复实施计划

> 基于审计报告: `2026-05-23-systemic-gap-audit.md`
> 计划日期: 2026-05-23
> 总预估工时: 13.5 小时
> 修复策略: 先闭合数据流(P0)，再填补架构空壳(P1)，最后清理死代码(P2)

---

## Phase 1: P0 数据流闭合 — 核心路径修复（2.5h）

### 任务 1.1: `update_chapter` auto_commit 后补发 StateSync 事件

**问题**: `story_system/mod.rs::run_kg_ingest` 完成知识图谱写入后，不发射任何同步事件，导致幕后 KG 视图不刷新。

**修复点**: `src-tauri/src/story_system/mod.rs`
- 在 `run_kg_ingest` 函数成功保存实体/关系后，添加：
  ```rust
  let _ = crate::state_sync::StateSync::emit_data_refresh(
      app_handle, Some(story_id), "knowledgeGraph"
  );
  ```
- 确保 `app_handle` 可从调用链传递下来（`SceneCommitService::auto_commit` → `apply_commit` → `run_kg_ingest`）

**验证方式**:
1. 启动应用，在幕前编辑章节内容并保存
2. 切换到幕后知识图谱页面，确认新实体在 30s 内自动出现（无需手动刷新）

**工时**: 0.5h

---

### 任务 1.2: `update_character` 后触发 Ingest

**问题**: 角色信息变更（状态、关系、背景）不进入知识图谱，Agent 后续创作无法感知。

**修复点**: `src-tauri/src/lib.rs::update_character`
- 在角色更新成功后，发起异步 Ingest：
  ```rust
  if result.is_ok() {
      // ... 现有 emit_character_updated ...
      
      // 新增: 角色变更触发 Ingest
      if let Some(ref story_id) = story_id_opt {
          let pool_clone = pool.clone();
          let character_id = id.clone();
          let story_id = story_id.clone();
          let app_handle = app.clone();
          tauri::async_runtime::spawn(async move {
              let llm_service = LlmService::new(app_handle.clone());
              let pipeline = IngestPipeline::new(llm_service)
                  .with_pool(pool_clone)
                  .with_app_handle(app_handle.clone());
              
              // 从角色最新状态构建 ingest 文本
              let ingest_text = format!("角色: {}\n描述: {}\n状态: {:?}", 
                  name.as_deref().unwrap_or(""),
                  description.as_deref().unwrap_or(""),
                  // ... 其他字段
              );
              
              let content = IngestContent {
                  text: ingest_text,
                  source: format!("character:{}", character_id),
                  story_id,
                  scene_id: None,
              };
              
              if let Err(e) = pipeline.ingest(&content).await {
                  log::warn!("[AutoIngest] Character {} ingest failed: {}", character_id, e);
              }
          });
      }
  }
  ```

**验证方式**:
1. 修改角色描述，保存
2. 检查知识图谱中是否出现与该角色相关的新实体/关系

**工时**: 1h

---

### 任务 1.3: `upgrade_subscription` 发射变更事件

**问题**: 用户升级订阅后，前端功能解锁状态不实时更新。

**修复点**: `src-tauri/src/subscription/mod.rs::upgrade_subscription`
- 在数据库写入成功后添加：
  ```rust
  let _ = crate::state_sync::StateSync::emit_subscription_changed(
      &app_handle, &user_id, &new_tier
  );
  ```
- 若 `emit_subscription_changed` 不存在，需先在 `state_sync/mod.rs` 中定义

**前端配合**: `src-frontend/src/hooks/useSyncStore.ts`
- 添加 `subscription_changed` 事件监听
- 收到事件后使能 `useSubscription` 的 React Query 缓存

**验证方式**:
1. 在设置页点击升级订阅
2. 不刷新页面，确认 Pipeline/拆书等功能立即解锁

**工时**: 0.5h

---

### 任务 1.4: `update_scene` 元数据变更触发 Ingest

**问题**: 仅修改场景标题/戏剧目标/冲突类型时，`updates.content.is_some()` 为 false，不触发 Ingest。

**修复点**: `src-tauri/src/commands_v3.rs::update_scene`
- 将触发条件从 `if updates.content.is_some()` 扩展为：
  ```rust
  let should_ingest = updates.content.is_some() 
      || updates.title.is_some()
      || updates.dramatic_goal.is_some()
      || updates.external_pressure.is_some()
      || updates.conflict_type.is_some();
  
  if should_ingest {
      // ... 现有 ingest 逻辑 ...
  }
  ```

**工时**: 0.5h

---

## Phase 2: P1 架构空壳填补 — 子系统功能修复（6h）

### 任务 2.1: `build_episodic_memory` 实现

**问题**: `memory/orchestrator.rs` 中 `build_episodic_memory` 返回 `Vec::new()`，Episodic 记忆层完全缺失。

**修复点**: `src-tauri/src/memory/orchestrator.rs`
- 实现基于场景历史的时间线记忆构建：
  ```rust
  fn build_episodic_memory(&self, story_id: &str, scene_id: Option<&str>) -> Vec<MemoryItem> {
      let repo = SceneRepository::new(self.pool.clone());
      let scenes = match scene_id {
          Some(sid) => repo.get_by_id(sid).ok().flatten().into_iter().collect(),
          None => repo.get_by_story(story_id).unwrap_or_default(),
      };
      
      scenes.into_iter()
          .filter(|s| s.content.is_some() && s.content.as_ref().unwrap().len() > 20)
          .map(|s| MemoryItem {
              source: format!("scene:{}", s.id),
              content: format!("第{}章 {}: {}", 
                  s.chapter_number, 
                  s.title.unwrap_or_default(),
                  s.content.as_ref().unwrap().chars().take(200).collect::<String>()
              ),
              relevance_score: 0.7,
              memory_type: MemoryType::Episodic,
          })
          .collect()
  }
  ```

**验证方式**:
1. 调用 `build_memory_pack` API
2. 检查返回结果中 `episodic` 数组非空

**工时**: 2h

---

### 任务 2.2: `ShortTermMemory::summarize_chapter` 智能摘要

**问题**: 当前仅截取前 200 字符，无语义摘要能力。

**修复策略**: 两阶段实现
- **快速修复（本期）**: 改用"首尾提取+关键词密度"的启发式摘要，优于纯截断
- **完整修复（后续迭代）**: 接入 LLM 生成真正的章节摘要

**修复点**: `src-tauri/src/memory/short_term.rs`
```rust
pub fn summarize_chapter(content: &str) -> String {
    if content.len() <= 300 {
        return content.to_string();
    }
    
    // 提取首段（设定/场景）+ 尾段（高潮/转折）
    let first_paragraph = content.split('\n').next().unwrap_or("");
    let last_paragraph = content.rsplit('\n').next().unwrap_or("");
    
    format!("{} ...（中间省略 {} 字）... {}", 
        first_paragraph.chars().take(100).collect::<String>(),
        content.len(),
        last_paragraph.chars().take(100).collect::<String>()
    )
}
```

**工时**: 1h

---

### 任务 2.3: 伏笔过期主动检查与通知

**问题**: `detect_overdue` 仅在 Agent 执行时被被动调用，无主动调度。

**修复策略**: 绑定到章节推进事件
- **修复点 1**: `lib.rs::update_chapter` 和 `create_chapter` 成功后，检查当前故事的逾期伏笔
- **修复点 2**: 若发现逾期伏笔，发射 `payoff_overdue` 事件到前端

```rust
// 在 update_chapter / create_chapter 的保存成功后添加
if let Some(ref story_id) = story_id_opt {
    let pool_clone = pool.clone();
    let story_id = story_id.clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let ledger = PayoffLedger::new(pool_clone);
        match ledger.detect_overdue(&story_id).await {
            Ok(overdue) if !overdue.is_empty() => {
                let _ = StateSync::emit_payoff_overdue(&app_handle, &story_id, overdue);
            }
            _ => {}
        }
    });
}
```

**前端配合**:
- `useSyncStore` 监听 `payoff_overdue` 事件
- 幕前写作界面收到事件后，在侧边栏显示"伏笔提醒"徽标

**验证方式**:
1. 创建一个将在第 3 章回收的伏笔
2. 写到第 5 章，保存
3. 确认 UI 出现"有伏笔逾期未回收"提醒

**工时**: 1h

---

### 任务 2.4: 场景-角色关联前端入口

**问题**: 5 个场景-角色关联 IPC 命令全部悬空，前端无法配置"谁出现在这个场景中"。

**修复策略**: 在 Scene 编辑器中添加角色参与面板
- **前端**: `src-frontend/src/components/scene-editor/SceneCharacterPanel.tsx`（新建）
  - 展示当前场景已关联角色
  - 提供"添加角色"下拉框（调用 `get_story_characters` 获取候选）
  - 调用 `set_scene_characters` 保存关联
- **后端**: 确认 `commands_v3.rs` 中命令实现正确（已有，无需修改）

**验证方式**:
1. 打开场景编辑器
2. 在角色面板添加/移除角色
3. 保存后重新打开，确认关联持久化

**工时**: 2h

---

## Phase 3: P2 代码清理与架构加固（5h）

### 任务 3.1: 悬空 IPC 命令分级清理

**目标**: 处理 38 个无前端调用的 IPC 命令

**分级策略**:

| 类别 | 命令示例 | 处理方式 |
|------|---------|---------|
| 纯占位符、无任何内部调用 | `open_update_settings`, `get_sync_status` | **删除** |
| 有内部调用价值但无需暴露 | `get_scene_commits`, `apply_chapter_commit` | **转为内部函数，从 IPC 移除** |
| 功能完整、需补充前端入口 | `get_scene_characters` 等 | **保留，创建前端接入任务** |
| 自动化系统命令 | `trigger_automation_event` 等 9 个 | **暂缓删除** — 待自动化系统启用后自然激活 |

**具体删除清单**:
- `open_update_settings` → `updater/mod.rs` 移除
- `get_sync_status` / `enable_auto_sync` / `disable_auto_sync` → `canonical_state` 或 `lib.rs` 移除
- `get_state` → 移除（被更细分的命令替代）

**工时**: 2h

---

### 任务 3.2: 自动化系统默认规则初始化

**问题**: 自动化引擎运转但无任何规则被加载。

**修复策略**: 在应用启动时注入一组默认自动化规则
- **修复点**: `src-tauri/src/lib.rs` 或 `automation/service.rs` 的初始化逻辑
- **默认规则**:
  1. `ChapterContentUpdated` → 若字数突增 >1000，自动触发 `ReadingPowerEvaluator`
  2. `ChapterCreated` → 自动触发 `SceneCommitService::auto_commit`
  3. `CharacterCreated` → 自动触发该角色的 `IngestPipeline`

```rust
fn init_default_automation_rules(service: &AutomationService) {
    service.register_handler(Handler {
        trigger: TriggerEvent::ChapterContentUpdated,
        action: Action::EvaluateReadingPower,
        condition: Some(Condition::WordCountDeltaGt(1000)),
    });
    // ... 其他默认规则
}
```

**工时**: 1.5h

---

### 任务 3.3: 前端死代码清理

**清理清单**:
- `src-frontend/src/pages/Chapters.tsx` — **删除**（无任何路由引用）
- `useSaveExportTemplate` / `useDeleteExportTemplate` — 确认无使用后 **删除**
- `WorkflowSettings.tsx` — 评估是否保留骨架或改为"功能即将上线"占位状态

**工时**: 0.5h

---

### 任务 3.4: Vector 写入自动化改造

**问题**: 向量存储写入依赖各调用路径显式调用，覆盖不完整。

**修复策略**: 将向量写入内化为 `IngestPipeline::ingest()` 的固定后处理步骤
- **修复点**: `src-tauri/src/memory/ingest.rs`
- 在 `ingest()` 返回结果前，若 `VECTOR_STORE` 已初始化，自动将原始文本和实体写入向量存储
- 移除 `update_scene` 中显式的 `VECTOR_STORE.get()` 调用（改为由 ingest 内部处理）

**注意**: 此改造需确保不引入循环依赖。`IngestPipeline` 不应直接依赖 `VECTOR_STORE` 全局，应通过构造函数注入。

**工时**: 1h

---

## 执行顺序与依赖关系

```
Phase 1 (P0 核心修复)
├── 1.1 update_chapter StateSync [独立]
├── 1.2 update_character Ingest [依赖 1.1 的模式]
├── 1.3 subscription 事件 [独立]
└── 1.4 update_scene 元数据触发 [独立]

Phase 2 (P1 架构填补)
├── 2.1 build_episodic_memory [独立]
├── 2.2 ShortTermMemory 摘要 [独立]
├── 2.3 伏笔过期检查 [依赖 1.1 的事件模式]
└── 2.4 场景-角色前端面板 [独立]

Phase 3 (P2 清理加固)
├── 3.1 悬空 IPC 清理 [建议在所有 P0/P1 完成后执行，避免误删]
├── 3.2 自动化默认规则 [独立]
├── 3.3 前端死代码 [独立]
└── 3.4 Vector 写入内化 [建议在 1.1/1.2 完成后执行]
```

---

## 验证清单

### Phase 1 验证
- [ ] 幕前保存章节 → 30s 内幕后 KG 自动刷新
- [ ] 修改角色描述 → KG 中出现角色相关新实体
- [ ] 订阅升级 → 不刷新页面功能立即解锁
- [ ] 修改场景标题（不改内容）→ 触发 Ingest

### Phase 2 验证
- [ ] `build_memory_pack` 返回的 `episodic` 数组非空
- [ ] 长章节摘要不是纯截断，包含首尾语义
- [ ] 逾期伏笔在保存章节时触发前端提醒
- [ ] 场景编辑器可添加/移除角色并持久化

### Phase 3 验证
- [ ] `cargo check` 通过，无未使用函数警告
- [ ] 前端编译无未使用 import 警告
- [ ] `IngestPipeline::ingest` 后向量存储自动包含新数据

---

## 风险与回退策略

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 1.2 `update_character` 引入异步 Ingest 后，高频编辑导致 LLM 请求过多 | 性能和成本 | 添加防抖（5s debounce），或仅当描述字段变更时触发 |
| 3.4 Vector 写入内化可能引入循环依赖 | 编译失败 | 重构为构造函数注入，而非全局 `VECTOR_STORE.get()` |
| 3.1 删除 IPC 命令可能有隐藏的内部反射调用 | 运行时崩溃 | 删除前先全局搜索命令名字符串，确认无 `invoke()`/`emit()`/`listen()` 引用 |
| 2.3 伏笔检查绑定到保存事件可能拖慢保存响应 | 用户体验 | 将 `detect_overdue` 放入独立 async spawn，不阻塞 save 返回 |

---

## 结论

本计划按 **P0 → P1 → P2** 的顺序执行，优先修复用户可感知的数据流断裂（章节保存后 KG 不刷新、角色变更不进入 KG），再填补架构空壳（Episodic 记忆、伏笔调度），最后清理死代码。全部修复预计 **13.5 小时**，可增量交付，每个 Phase 完成后系统状态均有可验证的改善。
