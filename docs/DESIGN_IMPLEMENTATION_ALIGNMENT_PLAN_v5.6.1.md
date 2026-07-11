# StoryMoss v5.6.1 设计-实现对齐全面修复计划

> **审计日期**: 2026-05-08
> **审计范围**: 幕前幕后自动关联、后台自动化、智能化创作飞轮全链路
> **审计方法**: 静态代码审计 + 文档对比 + 关键路径追踪 + 多代理并行审查
> **当前版本**: v5.6.0
> **目标版本**: v5.6.1

---

## 一、执行摘要

v5.6.0 在基础设施层面已建立了完整的框架：`state_sync` 模块、`AgentOrchestrator` 闭环、`WorkflowEngine` 持久化、`QueryPipeline` 语义搜索融合、`useSyncStore` 前端 Hook 等核心组件均已落地。经本次全面审计，发现 **8 项关键差距**，其中 **2 项 P0（影响功能正确性）**、**3 项 P1（影响完整度）**、**3 项 P2（优化/精度）**。

最大结构性缺口是：
1. **前端 Cache 对称失效不完整**：`sceneCreated`/`sceneDeleted` 不刷新 chapters 缓存，导致幕前幕后场景-章节关联状态不同步。
2. **AI 学习指示器伪实现**：`FrontstageApp` 的 `learnings` 仍是硬编码 mock，"越写越懂"对用户的感知是虚假的。

---

## 二、差距详情与修复方案

### 🔴 P0 关键差距（功能断裂）

#### 差距 1: `sceneCreated`/`sceneDeleted` 缺少 chapters 缓存失效 ❌

| 项 | 详情 |
|---|---|
| **位置** | `src-frontend/src/hooks/useSyncStore.ts:162-193` |
| **设计目标** | Chapter↔Scene 双向映射，任何一侧变更应同步刷新两侧缓存 |
| **现状** | `sceneCreated` 只刷新 `KEYS.scenes`，`sceneDeleted` 只刷新 `KEYS.scenes` + `KEYS.sceneDetail`。`KEYS.chapters` 从不刷新。 |
| **影响** | 后端 `SceneRepository::create`/`delete` 会修改 `chapters.scene_id` / `chapters.chapter_id`，但前端 chapters 列表中显示的 scene 关联状态滞后，幕前幕后不同步。 |
| **根因** | 修复 `sceneUpdated` 时添加了 chapters 刷新（v5.1.1），但 `sceneCreated`/`sceneDeleted` 被遗漏。 |

**修复方案**:
```typescript
// useSyncStore.ts line 162-167
case 'sceneCreated': {
  if (storyId) {
    queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
    queryClient.invalidateQueries({ queryKey: KEYS.chapters(storyId) }); // 新增
  }
  ...
}

// useSyncStore.ts line 183-193
case 'sceneDeleted': {
  if (storyId) {
    queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
    queryClient.invalidateQueries({ queryKey: KEYS.chapters(storyId) }); // 新增
  }
  ...
}
```

**验收标准**:
- 幕后创建 scene 后，幕前章节列表中的 scene 关联立即更新
- 幕后删除 scene 后，幕前章节列表中的 scene 关联立即清除
- `cargo check` / `npm run build` 通过

**预估工时**: 10 分钟

---

#### 差距 2: `FrontstageApp` learnings 仍是硬编码 mock ❌

| 项 | 详情 |
|---|---|
| **位置** | `src-frontend/src/frontstage/FrontstageApp.tsx:998-1020` |
| **设计目标** | v4.2.0 `AiLearningIndicator` 组件：每次 AI 交互后展示"系统学到了什么"，让"越写越懂"对用户可见 |
| **现状** | `handleAcceptGeneration` 和 `handleRejectGeneration` 中的 `setLearnings` 调用硬编码固定字符串：`{ category: '反馈', observation: '已记录接受偏好', impact: '系统将学习此方向' }`。注释声称"非硬编码 mock"，但实际仍是 mock。 |
| **影响** | 用户看到的"学习记录"永远是同样的三句话，与真实反馈无关。"越写越懂"是虚假感知。 |
| **根因** | `record_feedback` IPC 调用未返回挖掘出的偏好信息，前端无法展示真实学习点。 |

**修复方案**（方案 A：最小改动，让后端返回学习点）:
1. 后端 `record_feedback` 命令在执行完成后，调用 `PreferenceMiner::mine_preferences` 获取最近挖掘的偏好
2. 将偏好信息格式化为 `LearningPoint` 数组返回给前端
3. 前端直接使用返回结果设置 `learnings`

```rust
// lib.rs 或对应命令处理
#[command]
async fn record_feedback(...) -> Result<Vec<LearningPoint>, String> {
    // ... 原有记录逻辑 ...
    
    // 异步触发偏好挖掘
    let learnings = preference_miner.mine_recent(story_id, 5).await
        .unwrap_or_default()
        .into_iter()
        .map(|p| LearningPoint {
            category: p.category,
            observation: p.observation,
            impact: p.impact,
        })
        .collect();
    
    Ok(learnings)
}
```

```typescript
// FrontstageApp.tsx
const result = await recordFeedback({...});
if (result && result.length > 0) {
  setLearnings(result);
} else {
  // fallback 到通用提示
  setLearnings([{ category: '反馈', observation: '已记录偏好', impact: '系统将学习此方向' }]);
}
```

**验收标准**:
- 接受/拒绝 AI 生成后，学习指示器展示与真实偏好相关的信息
- 若后端未返回学习点， gracefully fallback 到通用提示
- `cargo check` / `npm run build` 通过

**预估工时**: 1.5 小时

---

### 🟡 P1 重要差距（功能不完整）

#### 差距 3: WritingStyle 更新后前端缓存不刷新 ⚠️

| 项 | 详情 |
|---|---|
| **位置** | `src-frontend/src/hooks/useSyncStore.ts:242-279` |
| **设计目标** | `update_writing_style` 修改后，前端写作风格缓存自动失效 |
| **现状** | 后端 `commands_v3.rs:381-402` 确实发射了 `data-refresh` 事件（`resourceType: "writingStyle"`），但前端 `useSyncStore.ts` 的 `dataRefresh` switch 缺少 `writingStyle` case。 |
| **影响** | 用户在幕后修改写作风格后，幕前编辑器中的风格注入不会更新，直到手动刷新页面。 |

**修复方案**:
```typescript
// useSyncStore.ts dataRefresh switch 中新增
case 'writingStyle':
  if (storyId) {
    queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(storyId) });
    // 或新增独立的 writingStyle key
  }
  break;
```

**验收标准**:
- 幕后修改写作风格后，幕前编辑器立即应用新风格
- `npm run build` 通过

**预估工时**: 10 分钟

---

#### 差距 4: Outline/Foreshadowing/Payoff 修改后前端缓存不刷新 ⚠️

| 项 | 详情 |
|---|---|
| **位置** | `src-frontend/src/hooks/useSyncStore.ts:242-279` |
| **设计目标** | 大纲/伏笔/回收账本修改后，前端对应缓存自动失效 |
| **现状** | 后端发射 `resourceType: "storyOutlines"` / `"foreshadowings"`，但前端 `dataRefresh` switch 中缺少这两个 case。只在 `case 'all'` 中会被刷新。 |
| **影响** | 幕后修改大纲或伏笔状态后，幕前相关面板（如 Execution Panel）不会自动更新。 |

**修复方案**:
```typescript
// useSyncStore.ts dataRefresh switch 中新增
case 'storyOutlines':
  if (storyId) {
    queryClient.invalidateQueries({ queryKey: KEYS.storyOutlines(storyId) });
  }
  break;
case 'foreshadowings':
  if (storyId) {
    queryClient.invalidateQueries({ queryKey: KEYS.foreshadowings(storyId) });
  }
  break;
```

**验收标准**:
- 幕后修改大纲后，幕前 Execution Panel 中的大纲概览自动更新
- 幕后修改伏笔状态后，幕前伏笔提示自动更新
- `npm run build` 通过

**预估工时**: 10 分钟

---

#### 差距 5: Pending vector 持久化使用 JSON 而非 SQLite ⚠️

| 项 | 详情 |
|---|---|
| **位置** | `src-tauri/src/lib.rs:126-134, 1305-1326` |
| **设计目标** | ROADMAP.md / AGENTS.md: "Pending vector SQLite 持久化" |
| **现状** | `PENDING_VECTOR_INDEXES` 使用 `pending_vector_indexes.json` 文件持久化，而非 SQLite 数据库。 |
| **影响** | 功能正确（加载/保存/清理均正常），但与设计文档声明的"SQLite 持久化"不符。JSON 文件在异常断电时可能损坏。 |
| **根因** | v5.5.0 修复时选择了最小改动的 JSON 文件方案，未迁移到 SQLite。 |

**修复方案**:
1. 新增 Migration 42: 创建 `pending_vector_indexes` 表（`id`, `chapter_id`, `story_id`, `created_at`）
2. 替换 `load_pending_vectors` / `save_pending_vectors` / `drain_pending_vectors` 为 SQLite 操作
3. 保持原有 API 不变，对上层透明

```rust
// connection.rs Migration 42
"CREATE TABLE IF NOT EXISTS pending_vector_indexes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chapter_id TEXT NOT NULL,
    story_id TEXT NOT NULL,
    created_at INTEGER NOT NULL
)"
```

**验收标准**:
- 应用重启后 pending vectors 从 SQLite 正确加载
- drain 完成后从 SQLite 正确删除
- `cargo test` 通过

**预估工时**: 1 小时

---

### 🟢 P2 优化差距（锦上添花）

#### 差距 6: WorkflowScheduler 同层节点并行执行与文档描述不一致 ⚠️

| 项 | 详情 |
|---|---|
| **位置** | `src-tauri/src/workflow/scheduler.rs:219-220` |
| **设计目标** | AGENTS.md v5.2.0: "串行拓扑执行" |
| **现状** | 代码使用 `futures::future::join_all(node_futures).await` 并行执行同一轮中所有可执行节点。 |
| **影响** | 功能正确（DAG 语义允许并行），但文档描述与实现不一致。重试时可能存在部分完成节点重复执行的边缘情况。 |

**修复方案**:
两个选择：
- **方案 A（修改文档）**: 将 AGENTS.md / CHANGELOG 中的"串行拓扑执行"改为"拓扑有序执行（同层可并行）"
- **方案 B（修改代码）**: 将 `join_all` 改为 `for node_future in node_futures { node_future.await; }` 实现真正串行

推荐方案 A（并行执行在 DAG 中是正确且更高效的语义）。

**验收标准**:
- 文档与实现一致
- `cargo check` 通过

**预估工时**: 10 分钟

---

#### 差距 7: Workflow 实例级别幂等性不完整 ⚠️

| 项 | 详情 |
|---|---|
| **位置** | `src-tauri/src/workflow/scheduler.rs:24-32` |
| **设计目标** | 同一工作流实例不会被重复入队和执行 |
| **现状** | `schedule_execution` 没有检查 instance_id 是否已在 queue 或 running 中。`start_instance` 对 Running 状态返回错误，但前端网络重试可能导致重复调用。 |
| **影响** | 极端情况下（网络抖动 + 前端重试），同一工作流实例可能被重复执行。 |

**修复方案**:
```rust
pub async fn schedule_execution(&self, instance_id: String) -> Result<(), Box<dyn std::error::Error>> {
    // 幂等检查
    {
        let queue = self.queue.lock().unwrap();
        if queue.contains(&instance_id) {
            log::warn!("[WorkflowScheduler] Instance {} already in queue, skipping", instance_id);
            return Ok(());
        }
    }
    {
        let running = self.running_instances.lock().unwrap();
        if running.contains(&instance_id) {
            log::warn!("[WorkflowScheduler] Instance {} already running, skipping", instance_id);
            return Ok(());
        }
    }
    
    let mut queue = self.queue.lock().unwrap();
    queue.push_back(instance_id);
    Ok(())
}
```

**验收标准**:
- 同一 instance_id 重复入队时被优雅跳过
- `cargo test` 通过

**预估工时**: 20 分钟

---

#### 差距 8: SyncEvent 数量文档精度 ⚠️

| 项 | 详情 |
|---|---|
| **位置** | `AGENTS.md` / `CHANGELOG.md` |
| **设计目标** | 文档声称 "16 种 `SyncEvent`" |
| **现状** | 实际 `state_sync/events.rs` 中定义了 **16 种**：Story(4) + Character(3) + Scene(4) + Chapter(3) + WorldBuilding(1) + DataRefresh(1) = 16。部分旧文档（v5.1.0 前）声称 18 种。 |
| **影响** | 文档精度问题，不影响功能。 |

**修复方案**: 统一所有文档中的 SyncEvent 数量为 16 种。

**预估工时**: 10 分钟

---

## 三、实施路线图

```
Phase 1: P0 核心修复（立即执行）
├── 1. sceneCreated/sceneDeleted 刷新 chapters 缓存 — 10min
└── 2. FrontstageApp learnings 真实反馈 — 1.5h

Phase 2: P1 功能补全（同日完成）
├── 3. WritingStyle dataRefresh case — 10min
├── 4. Outline/Foreshadowing dataRefresh case — 10min
└── 5. Pending vector SQLite 持久化 — 1h

Phase 3: P2 优化与文档（次日完成）
├── 6. WorkflowScheduler 文档/代码一致性 — 10min
├── 7. Workflow 幂等性 — 20min
└── 8. SyncEvent 文档统一 — 10min
```

**总预估工时**: 3.5 小时

---

## 四、依赖关系

```
差距 1 (Cache 对称失效) ──┐
差距 3 (WritingStyle) ────┼──→ 均独立，可并行
差距 4 (Outline 同步) ────┤
差距 5 (Pending vector) ──┤
差距 2 (Learnings) ───────┤
差距 6-8 (P2 优化) ───────┘
```

---

## 五、验收清单

### 功能验收
- [ ] 幕后创建 scene → 幕前 chapters 列表 scene 关联实时更新
- [ ] 幕后删除 scene → 幕前 chapters 列表 scene 关联实时清除
- [ ] 接受/拒绝 AI 生成 → 学习指示器展示真实偏好信息
- [ ] 幕后修改写作风格 → 幕前编辑器立即应用新风格
- [ ] 幕后修改大纲/伏笔 → 幕前相关面板自动更新
- [ ] 应用重启后 pending vectors 从 SQLite 正确恢复

### 编译验收
- [ ] `cargo check` 零错误
- [ ] `cargo test` 217/217 通过
- [ ] `npm run build` 通过
- [ ] `cargo tauri build` Windows 安装包生成

### 文档验收
- [ ] `CHANGELOG.md` 更新 v5.6.1 条目
- [ ] `AGENTS.md` 更新最近完成的功能和编译状态
- [ ] `ROADMAP.md` 更新完成度统计
- [ ] `FEATURES.md` 更新（如受影响）

---

## 六、风险与应对

| 风险 | 可能性 | 影响 | 应对 |
|------|--------|------|------|
| Pending vector SQLite 迁移导致数据丢失 | 低 | 启动时 pending vectors 丢失 | 迁移脚本保留 JSON 文件作为 fallback，读取后删除 |
| Learnings 后端返回格式不兼容 | 低 | 前端解析失败 | 添加类型守卫，格式不符时 fallback 到通用提示 |
| 新增 invalidateQueries 导致过度刷新 | 中 | 前端性能轻微下降 | 观察 React Query devtools，必要时改为 `refetchQueries` |

---

*计划由 Kimi Code CLI 根据全面代码审计结果制定*
*待用户审批后实施*
