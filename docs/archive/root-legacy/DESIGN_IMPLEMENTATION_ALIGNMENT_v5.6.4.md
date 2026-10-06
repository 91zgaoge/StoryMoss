# StoryMoss (草苔) v5.6.4 设计-实现对齐全面检视与实施计划

> **检视日期**: 2026-05-13
> **项目**: StoryMoss v5.6.4 - AI 导演式小说创作系统
> **检视范围**: 前端 (React/TypeScript)、后端 (Tauri/Rust)、数据库 (SQLite)、状态同步、后台自动化

---

## 执行摘要

经过对项目设计文档、架构规范和当前代码实现的全面深度检视，识别出 **13 项关键设计-实现差距**，主要集中在三个核心维度：

1. **幕前幕后自动关联** (4项差距) — 数据变更未能在双界面间完整实时同步，关键路径存在断点
2. **后台自动化闭环** (5项差距) — 智能化创作飞轮缺乏持续反馈机制和持久化保障
3. **系统整洁度与数据一致性** (4项差距) — 部分表缺失、死胡同功能、级联清理不完整

**影响评估**: 这些差距导致用户体验割裂（KG 更新不刷新、自动化不触发）、AI 学习效果递减（模板不持久、进化不周期）、数据一致性问题（表缺失、幽灵数据），严重影响了"越写越懂"的核心价值主张。

---

## 项目设计目标回顾

### 核心设计理念

StoryMoss 的设计围绕三个核心理念：

#### 1. 剧院式双界面架构
- **幕前 (Frontstage)**: 沉浸式写作界面，暖色调 (#f5f4ed) Claude 阅读体验，AI 萤火随行
- **幕后 (Backstage)**: 专业工作室，故事/场景/角色管理，知识图谱可视化，技能系统
- **设计目标**: 双窗口实时数据同步，任何变更零延迟对齐

#### 2. 场景化叙事系统 + 增强记忆系统
- 以场景为单位的戏剧冲突驱动
- 四层记忆: Multi-Agent Sessions → Knowledge Graph → Vector Store → Raw Sources
- 设计目标: IngestPipeline、向量索引、知识图谱形成自维持反馈环

#### 3. 智能化创作飞轮
- 自适应学习系统记录反馈、挖掘偏好、动态调节生成参数
- 能力进化引擎持续优化 Agent 描述
- 设计目标: "越写越懂"的 AI 创作助手

---

## 当前实现状态分析

### 已实现的部分

| 模块 | 完成度 | 说明 |
|------|--------|------|
| 双界面基础架构 | 90% | 窗口管理、切换、事件通信已就绪 |
| StateSync 事件系统 | 85% | 17 种同步事件已定义，大部分 CRUD 已发射 |
| 后台 Automation Service | 70% | 服务框架、触发器、处理器、事件队列已就绪 |
| 能力进化引擎 | 75% | JSON 持久化、LLM 分析、启动时触发已就绪 |
| Workflow 引擎 | 80% | DAG 执行、节点并行、自动 drain、持久化已就绪 |
| 数据层外键约束 | 85% | PRAGMA foreign_keys = ON 已启用，大部分表已定义级联 |

### 识别出的关键差距

---

## 差距详细分析

### 维度一: 幕前幕后自动关联

#### 差距 1: Ingest 完成后不触发知识图谱同步事件 [P0]

**问题描述**:
- `lib.rs::auto_ingest_chapter` (L972-1092): 章节保存后自动 Ingest，保存实体/关系到 KG，但**不发射任何同步事件**
- `commands_v3.rs::update_scene` (L118-248): 场景内容更新后自动 Ingest，同样**不发射 KG 刷新事件**
- 后果: 幕后知识图谱页面不会自动刷新新抽取的实体，用户需手动刷新

**代码位置**:
- `src-tauri/src/lib.rs:972-1092` — `auto_ingest_chapter` 函数末尾无 StateSync 调用
- `src-tauri/src/commands_v3.rs:118-248` — `update_scene` 中 auto ingest 异步块内无 StateSync 调用

**预期行为**: Ingest 完成后自动发射 `sync-event` (type=ingestionCompleted 或 dataRefresh/knowledgeGraph)，前端自动刷新 KG 缓存

---

#### 差距 2: Automation Service 与数据变更事件集成不完整 [P1]

**问题描述**:
- 当前只有 `create_chapter` (lib.rs L1095) 调用了 `automation_service.trigger_event(ChapterCreated)`
- `create_story`、`create_character`、`update_chapter`、`update_scene`、`delete_*` 等命令**均未触发 automation 事件**
- 后果: 后台自动化触发器只在章节创建时工作，故事创建后自动初始化结构、角色创建后自动分析关系等触发器永远不会被激活

**代码位置**:
- `src-tauri/src/lib.rs:824-916` — create_story/update_story/delete_story/create_character/update_character/delete_character 均无 automation_service 调用
- `src-tauri/src/commands_v3.rs:16-269` — create_scene/update_scene/delete_scene 均无 automation_service 调用

**预期行为**: 所有核心 CRUD 操作完成后，都应将对应事件推入 Automation Service 的事件队列

---

#### 差距 3: 部分同步事件缺失前端独立响应 [P2]

**问题描述**:
- `state_sync/service.rs` 已定义: `emit_character_relationships_updated`、`emit_payoff_ledger_updated`、`emit_ingestion_completed`
- 但 `useSyncStore.ts` 中**没有这些事件的独立 case 处理**，只有 `dataRefresh` 批量刷新可以间接覆盖
- 后果: 如果后端直接发射这些特定事件（而非 dataRefresh），前端不会响应

**代码位置**:
- `src-frontend/src/hooks/useSyncStore.ts:104-347` — switch 语句中缺少上述 case

---

### 维度二: 后台自动化闭环

#### 差距 4: PlanTemplateLibrary 仅内存存储，无持久化 [P1]

**问题描述**:
- `planner/template_learning.rs` (L17-47): `PlanTemplateLibrary` 只有 `Vec<PlanTemplate>` 内存存储
- 重启后所有学习成果丢失，每次都需要重新学习
- 后果: "越写越懂"效果递减，重复 LLM 调用浪费配额

**代码位置**:
- `src-tauri/src/planner/template_learning.rs:17-47`

**预期行为**: PlanTemplateLibrary 应在 SQLite 中持久化（如 `plan_templates` 表），启动时加载，变更时保存

---

#### 差距 5: 能力进化仅启动时触发，无周期触发机制 [P1]

**问题描述**:
- `lib.rs` setup (L391-416): 只在应用启动延迟30秒后执行一次 `evolve_capability_descriptions()`
- 没有按记录数阈值（如达到 5/10/20 条新记录）触发的机制
- 没有定时周期触发（如每 24 小时检查一次）
- 后果: 长时间使用后 AI 能力描述不更新，进化引擎形同虚设

**代码位置**:
- `src-tauri/src/lib.rs:391-416`
- `src-tauri/src/capabilities/evolution.rs:166-237`

**预期行为**: 每次记录新执行后检查阈值，达到条件时自动触发进化；或建立定时任务每 24h 检查一次

---

#### 差距 6: WorkflowEngine 恢复实例后未重新入队 Scheduler [P1]

**问题描述**:
- `workflow/mod.rs::WorkflowEngine::with_pool` (L209-219): 从数据库加载 Pending/Running/Paused 实例到内存 HashMap
- 但**这些恢复的实例没有被自动加入 WorkflowScheduler 的队列**
- `workflow/scheduler.rs::start_auto_drain` 只消费队列中的实例，不会主动扫描 engine 中的 pending 实例
- 后果: 应用重启后，中断的工作流实例虽然被加载到内存，但永远不会被调度执行

**代码位置**:
- `src-tauri/src/workflow/mod.rs:209-219` — `with_pool` 加载实例
- `src-tauri/src/workflow/scheduler.rs:52-106` — `start_auto_drain` 只处理队列

**预期行为**: `with_pool` 加载实例后，或将 Pending/Running 实例自动入队 scheduler；或在 scheduler 启动时扫描 engine 中的待执行实例并入队

---

#### 差距 7: story_metadata 表缺失 [P0]

**问题描述**:
- `automation/service.rs` 大量使用了 `story_metadata` 表（L351, 386, 425-428, 465-468, 506-509）
- 但在 `db/connection.rs` 的完整 schema 定义中**没有 `story_metadata` 表的 CREATE TABLE 语句**
- 后果: automation service 执行时必定报错，整个后台自动化系统无法正常工作

**代码位置**:
- `src-tauri/src/automation/service.rs` — 多处 INSERT/REPLACE INTO story_metadata
- `src-tauri/src/db/connection.rs` — 无 story_metadata 表定义

---

### 维度三: 系统整洁度与数据一致性

#### 差距 8: scene_characters / scene_character_actions 表 Schema 定义缺失 [P0]

**问题描述**:
- `db/repositories_v3.rs` (L2983-3124) 中 `SceneCharacterRepository` 大量操作 `scene_characters` 表
- `db/connection.rs` 的 `create_tables` 和 `create_v3_tables` 中**没有这两个表的 CREATE TABLE 定义**
- 虽然测试代码（connection.rs L2113-2126）直接 INSERT 到这些表，但测试可能依赖隐式表创建或早期迁移残留
- 后果: 新安装的应用或清理后的数据库中这些表不存在，导致场景角色关联功能失败

**代码位置**:
- `src-tauri/src/db/repositories_v3.rs:2983-3124` — SceneCharacterRepository
- `src-tauri/src/db/connection.rs` — 无 scene_characters / scene_character_actions 定义

---

#### 差距 9: delete_story / delete_character 过度依赖外键约束，部分关联数据可能残留 [P1]

**问题描述**:
- `repositories.rs::StoryRepository::delete` (L109-132): 仅执行 `DELETE FROM stories WHERE id = ?1`，完全依赖外键级联
- `repositories.rs::CharacterRepository::delete` (L246-269): 同样仅执行 `DELETE FROM characters WHERE id = ?1`
- 问题表:
  - `story_metadata` — 无表定义，更无外键约束，删除故事后数据必定残留
  - `foreshadowing_tracker` — 有 `story_id` 字段但**无外键约束**（见 connection.rs L1052-1063）
  - `user_preferences` — 有 `story_id` 字段但**无外键约束**
  - `scene_characters` — 有 `scene_id`/`character_id` 字段，但 character_id 无外键约束（如果表存在的话）
  - `scene_character_actions` — 同上
- 后果: 删除操作后产生幽灵数据，污染数据库

**代码位置**:
- `src-tauri/src/db/repositories.rs:109-132` — StoryRepository::delete
- `src-tauri/src/db/repositories.rs:246-269` — CharacterRepository::delete

---

#### 差距 10: image Profile UI 死胡同 [P2]

**问题描述**:
- `src-frontend/src/pages/Settings.tsx` (L162-164, 243-253): 提供 "图像生成" Tab，允许用户配置 image 类型的 LLM Profile
- 后端没有图像生成 IPC 命令或图像生成 Agent
- 后果: 用户配置后无法使用，产生困惑

**代码位置**:
- `src-frontend/src/pages/Settings.tsx:162-164, 243-253`

---

#### 差距 11: 知识图谱直接更新路径不统一 [P2]

**问题描述**:
- 知识图谱实体/关系的更新存在多个入口:
  - `commands_v3.rs` 中的 `create_entity` / `update_entity` / `delete_entity` / `create_relation` / `delete_relation`
  - `lib.rs` 中的 `auto_ingest_chapter` 直接调用 `kg_repo.save_entities_batch`
  - `commands_v3.rs::update_scene` 中的 auto ingest 直接调用 `kg_repo.create_entity`/`create_relation`
- 这些直接操作**不经过 StateSync**，导致前端 KG 可视化无法实时感知变更
- 后果: 同步行为不一致，难以调试，用户体验不流畅

**代码位置**:
- `src-tauri/src/commands_v3.rs` — KG CRUD 命令
- `src-tauri/src/lib.rs:1025-1045` — auto_ingest_chapter 直接保存 KG 数据

---

#### 差距 12: useSyncStore 缺少 ingestionCompleted 事件处理 [P2]

**问题描述**:
- `state_sync/events.rs` 已定义 `IngestionCompleted` 事件
- `state_sync/service.rs` 已实现 `emit_ingestion_completed`
- 但 `useSyncStore.ts` 中**没有 `ingestionCompleted` case**
- 后果: 即使后端修复了差距 1 发射了 ingestion 事件，前端也不会刷新 KG 缓存

**代码位置**:
- `src-frontend/src/hooks/useSyncStore.ts`

---

#### 差距 13: 幕前窗口启动逻辑与配置不一致 [P2]

**问题描述**:
- `PLAN_Frontstage_Backstage.md` 设计: frontstage 窗口 1400x900，启动时可见；backstage 窗口 1200x800，启动时隐藏
- 当前 `lib.rs` setup (L363-369): 确实在启动时隐藏 backstage、聚焦 frontstage
- 但需要验证 `tauri.conf.json` 中的窗口配置是否匹配
- 潜在问题: 窗口 Label 可能仍不匹配（代码中查找 "frontstage"/"backstage"，但配置中可能是 "main"）

**代码位置**:
- `src-tauri/tauri.conf.json` — 需要验证
- `src-tauri/src/lib.rs:363-369`

---

## 修复实施计划

### 修复策略

采用**增量式修复**方案，遵循以下原则:
- 对现有 Rust 单测和前端测试零破坏
- 保持 v5.6.4 已验证行为不变
- 优先修复 P0 级别数据正确性和表缺失问题
- 分步骤验证，每步都可独立测试

### 实施步骤

#### Step A: 数据层根因修复 (P0)
**目标**: 补齐缺失的数据库表定义，修复级联删除问题

**具体任务**:
1. **在 `connection.rs` 中添加 `story_metadata` 表定义** (Migration)
   - 表结构: `story_id TEXT, key TEXT, value TEXT, updated_at TEXT`
   - 创建复合索引 `(story_id, key)`
   - 添加外键约束 `REFERENCES stories(id) ON DELETE CASCADE`

2. **在 `connection.rs` 中添加 `scene_characters` 表定义** (Migration)
   - 表结构: `id TEXT PRIMARY KEY, scene_id TEXT NOT NULL, character_id TEXT NOT NULL, created_at TEXT`
   - 外键: `scene_id REFERENCES scenes(id) ON DELETE CASCADE`, `character_id REFERENCES characters(id) ON DELETE CASCADE`
   - 索引: `idx_scene_characters_scene`, `idx_scene_characters_character`

3. **在 `connection.rs` 中添加 `scene_character_actions` 表定义** (Migration)
   - 表结构: `id TEXT PRIMARY KEY, scene_id TEXT NOT NULL, character_id TEXT NOT NULL, action_type TEXT, content TEXT, created_at TEXT`
   - 外键约束同上

4. **加固 `delete_story` 显式清理逻辑**
   - 在事务中显式清理 `story_metadata`、`foreshadowing_tracker`、`user_preferences`、`ai_operations` 等 story_id 关联数据
   - 即使外键约束已启用，也作为防御性编程添加显式 DELETE

5. **加固 `delete_character` 显式清理逻辑**
   - 在事务中显式清理 `scene_characters`、`scene_character_actions`、`character_relationships`、`character_states`

6. **添加数据一致性测试用例**
   - 验证删除故事/角色后所有关联表无残留

**预期结果**: 删除操作正确级联，无幽灵数据残留；automation service 正常运行

---

#### Step B: 同步事件补全 (P0-P1)
**目标**: 补齐缺失的同步事件发射和前端响应

**具体任务**:
1. **修复 `auto_ingest_chapter`**
   - 在 ingest 成功保存实体/关系后，调用 `StateSync::emit_data_refresh(&app, Some(&story_id), "knowledgeGraph")`
   - 同时调用 `StateSync::emit_ingestion_completed(&app, &story_id, "chapter")`

2. **修复 `update_scene` 中的 auto ingest**
   - 在异步 ingest 块完成保存实体/关系后，发射 `dataRefresh(knowledgeGraph)` 和 `ingestionCompleted`
   - 注意: 由于 ingest 在 `tauri::async_runtime::spawn` 中异步执行，需要通过 AppHandle 发射事件

3. **统一 KG 更新路径**
   - 在 `commands_v3.rs` 的 `create_entity`、`update_entity`、`delete_entity`、`create_relation`、`delete_relation` 命令末尾，添加 `StateSync::emit_data_refresh(_, _, "knowledgeGraph")`

4. **前端 `useSyncStore` 补全**
   - 添加 `case 'characterRelationshipsUpdated'` → 刷新 `characterRelationships` 缓存
   - 添加 `case 'payoffLedgerUpdated'` → 刷新 `payoffLedger` 缓存
   - 添加 `case 'ingestionCompleted'` → 刷新 `knowledgeGraph` 缓存

**预期结果**: 所有数据变更都能触发前端缓存刷新，KG 可视化实时更新

---

#### Step C: Automation Service 全面集成 (P1)
**目标**: 将 Automation Service 连接到所有核心数据变更事件

**具体任务**:
1. **lib.rs 命令集成**
   - `create_story`: 触发 `TriggerEvent::StoryCreated`
   - `create_character`: 触发 `TriggerEvent::CharacterCreated`
   - `update_chapter` / `update_scene`: 触发 `TriggerEvent::ChapterContentUpdated`（带字数）
   - `create_chapter` 已集成，保持现状

2. **commands_v3.rs 命令集成**
   - `create_scene`: 触发 `TriggerEvent::ChapterCreated`（或新增 SceneCreated）
   - `update_scene`: 触发内容更新事件

3. **Automation Service 事件处理增强**
   - 当前 `process_single_event` 已能匹配事件并执行 handler
   - 确保所有预定义触发器的 handler 能正确执行（`init_story_structure` 等）
   - 修复 `story_metadata` 表缺失导致的 handler 失败

**预期结果**: 后台自动化对所有核心数据变更事件响应，形成事件驱动的工作流

---

#### Step D: 后台自动化闭环 (P1)
**目标**: 建立持续的自动化反馈机制

**具体任务**:
1. **PlanTemplateLibrary 持久化** (Migration 43)
   - 新建 `plan_templates` 表: `id TEXT, trigger_patterns TEXT, plan_json TEXT, success_count INTEGER, failure_count INTEGER, created_at TEXT`
   - 修改 `PlanTemplateLibrary`:
     - `new()` 时从数据库加载所有模板
     - `record_success()` 时保存到新表
     - `find_match()` 从内存 + 数据库查询

2. **能力进化周期触发机制**
   - 修改 `ExecutionRecordStore::append()`:
     - 每次追加记录后，检查总记录数是否达到阈值（如 5 的倍数）
     - 若达到阈值，触发一次异步进化（通过 AppHandle 发射内部事件或直接进入队列）
   - 保留启动时的延迟进化作为兜底
   - 添加配置项 `capability_evolution_threshold`（默认 5）

3. **WorkflowEngine 恢复实例自动入队**
   - 修改 `WorkflowEngine::with_pool`:
     - 加载实例后，将状态为 `Pending` 或 `Running` 的实例 ID 返回给调用方
   - 修改 `lib.rs` setup 中的 workflow 初始化:
     - `with_pool` 返回待恢复实例列表
     - 遍历列表，调用 `scheduler.schedule_execution(instance_id).await`
   - 确保恢复的实例能被 `start_auto_drain` 正常调度

**预期结果**: 后台自动化形成自维持反馈环，重启后学习成果保持、工作流自动恢复

---

#### Step E: 系统整洁度优化 (P2)
**目标**: 消除死胡同和不一致性

**具体任务**:
1. **image Profile UI 处理**
   - 方案 A: 在 Settings.tsx 的 image Tab 添加 "暂未实现" 标注，禁用添加按钮
   - 方案 B: 隐藏 image Tab，待后端实现后再开放

2. **验证 tauri.conf.json 窗口配置**
   - 确认 frontstage 窗口 label="frontstage"，启动时 visible=true
   - 确认 backstage 窗口 label="backstage"，启动时 visible=false
   - 确认尺寸配置匹配设计文档

3. **添加文档和注释**
   - 在 StateSync 中添加注释，说明所有 KG 更新必须经过 StateSync
   - 在 automation service 中添加触发器注册说明

**预期结果**: 系统行为一致，用户体验清晰，无死胡同功能

---

## 验证计划

### 功能验证
1. **数据一致性验证**: 删除故事/角色后检查所有关联表无残留
2. **同步事件验证**: Ingest 完成后检查前端 KG 缓存自动刷新
3. **Automation 完整性验证**: 创建故事/角色/场景后检查自动化事件被触发
4. **持久化闭环验证**: 重启后检查 PlanTemplate 保持、工作流自动恢复
5. **能力进化验证**: 记录 5 条执行后检查进化是否自动触发

### 兼容性验证
1. **向后兼容**: 现有数据库通过迁移正常升级
2. **API 兼容**: 前端调用接口保持不变
3. **测试兼容**: 现有 Rust 单测全部通过

---

## 实施时间表

| 阶段 | 内容 | 预计时间 | 依赖 |
|------|------|----------|------|
| 第一阶段 | Step A: 数据层修复（表定义、级联删除） | 1 天 | 无 |
| 第二阶段 | Step B: 同步事件补全 | 0.5 天 | Step A |
| 第三阶段 | Step C: Automation Service 全面集成 | 0.5 天 | Step A |
| 第四阶段 | Step D: 后台自动化闭环 | 1 天 | Step B, C |
| 第五阶段 | Step E: 系统整洁度优化 | 0.5 天 | 无 |
| 第六阶段 | 全面回归测试 | 0.5 天 | 全部 |

**总计**: 4 天完成所有修复

---

## 成功标准

### 功能完整性
- [ ] 所有 13 项差距完全修复
- [ ] 幕前幕后数据实时同步 (< 100ms)
- [ ] Ingest 完成后 KG 可视化自动刷新
- [ ] 后台自动化对所有核心事件响应
- [ ] 重启后学习成果保持、工作流自动恢复

### 质量保证
- [ ] 现有 Rust 单测全部通过
- [ ] 新增测试覆盖所有修复点
- [ ] 性能指标不劣化

### 用户体验
- [ ] 删除操作无幽灵数据残留
- [ ] 知识图谱实时刷新
- [ ] 自动化触发无感、可靠
- [ ] 无死胡同功能

---

## 风险评估与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 数据库迁移影响现有数据 | 高 | 新增表结构，不修改现有表；完整备份机制 |
| 同步事件频繁触发影响性能 | 中 | 事件去重；异步发射不阻塞主流程 |
| Automation 集成引入新 bug | 中 | 增量式修复，每步独立验证；灰度测试 |
| Workflow 恢复机制导致重复执行 | 高 | 幂等检查（scheduler 已有）；状态验证 |

---

## 结论

StoryMoss 项目具有清晰的设计愿景和扎实的技术基础，当前实现已达到功能框架的 80% 完成度。通过系统性修复识别的 13 项设计-实现差距，可以将项目从"功能框架"提升到"生产就绪"状态，真正实现:

1. **幕前幕后零延迟自动关联** — 任何数据变更在双界面间实时同步
2. **后台自动化自维持闭环** — 事件触发 → 自动化处理 → 状态更新 → 前端刷新
3. **AI 持续学习不遗忘** — 模板持久化、能力周期进化、工作流自动恢复

修复方案采用增量式策略，风险可控，预期在 4 天内完成所有修复工作。

---

## 下一步行动

1. **获得本计划批准**
2. **立即开始 Step A**: `story_metadata`、`scene_characters`、`scene_character_actions` 表定义添加
3. **并行进行 Step E**: image Profile UI 处理（无依赖，可立即开始）
4. **按阶段推进**，每阶段完成后进行验证测试
