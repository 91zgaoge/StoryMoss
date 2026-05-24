# 统一创作流水线实施计划

## 1. 问题诊断

当前存在两套互不相通的故事初始化系统：

### 1.1 快速创作路径（活跃）
- 前端：`Stories.tsx` → `runCreationWorkflow()`
- 后端：`CreationWorkflowEngine` (`creative_engine/workflow/engine.rs`)
- 阶段：Conception → Outlining → SceneDesign → Writing → Review → Iteration → Ingestion
- **数据写入**：只写 `Chapter.content`，**不创建 Scene/World/Character/Style**
- **冲突**：直接违反 `SCENE_COMMIT` 为唯一提交粒度的设计原则

### 1.2 向导路径（被禁用/零散）
- `NovelCreationWizard.tsx`：Dashboard 中活跃使用的组件级向导
- `CreationWizard.tsx`：页面级向导，导入但未注册路由（死代码）
- 后端：`create_story_with_wizard` (`story_commands.rs:1407`)
- 数据写入：Story + WorldBuilding + Characters + WritingStyle + Scene（正确）
- **缺陷**：无事务保护、无工作流引擎、200+ 行硬编码事务脚本

### 1.3 核心矛盾
```
快速创作：User Input → [WorkflowEngine] → Chapter（物理存储）
                           ↓
                      不创建 Scene/World/Character/Style

向导模式：User Input → [Wizard Steps] → Scene + World + Character + Style
                           ↓
                      不经过 WorkflowEngine，无审校/迭代
```

**两套系统数据模型不兼容，执行引擎不共享。**

## 2. 统一架构方案

### 2.1 核心原则

所有故事初始化，无论前端交互形态如何，都必须经过同一后端流水线，以 Scene 为唯一提交粒度，产出完整的故事要素（World + Characters + Style + Scene）。

**"快"的本质：用户不参与、不等待、立即可编辑。后台 5 阶段 Pipeline 作为异步任务执行，完成后自动 enrich。**

### 2.2 架构图

```
┌──────────────────────────────────────────────────────────────┐
│                     Unified CreationPipeline                  │
├──────────────────────────────────────────────────────────────┤
│  Phase 1: Conception      用户输入 → 故事种子                │
│  Phase 2: WorldBuilding   种子 → 世界观/规则/历史            │
│  Phase 3: Characters      世界观 → 角色档案                  │
│  Phase 4: WritingStyle    世界观+角色 → 风格定义             │
│  Phase 5: SceneDesign     全部要素 → 场景设计                │
│  Phase 6: Writing         场景设计 → 正文                    │
│  Phase 7: Review          正文 vs 要素一致性检查             │
│  Phase 8: Commit          SCENE_COMMIT → DB + 投影 + KG      │
└──────────────────────────────────────────────────────────────┘
              ↑                              ↑
    ┌─────────┘                              └─────────┐
    │                                                  │
快速模式：Phase 1-6 压缩为单个异步任务        向导模式：Phase 1-8 分步阻塞
    │                                                  │
    └─→ 前端立即返回 Scene + 正文                  └─→ 每步返回中间产物
        后台任务继续 Phase 7-8                         用户确认后继续
        完成后如有修正自动更新 Scene
```

### 2.3 双模式对比

| 维度 | 快速创作 | 向导模式 |
|------|----------|----------|
| 用户等待 | 零等待，立即看到正文 | 每步需确认 |
| 用户决策 | 零决策，全自动 | 每步可编辑/选择 |
| 后台工作量 | **完全相同**，都走 8 阶段 | 完全相同 |
| 最终产出 | 完全相同 | 完全相同 |

## 3. 已完成的实施

### Phase 1: 统一后端 Pipeline ✅

#### 3.1.1 `CreationWorkflowEngine` 改造
- **`Ingestion` 阶段从写 Chapter 改为写 Scene** (`engine.rs:304-387`)
  - 查找 story 的已有 scenes，更新最后一个 scene 的 content
  - 无 scene 则创建新 Scene（sequence=1, title="开场场景"）
- **新增占位要素创建** (`engine.rs:378-387`)
  - 若 story 无 WorldBuilding，创建占位记录（concept="待完善的世界观"）
  - 若 story 无 Characters，创建占位主角记录
  - 若 story 无 WritingStyle，创建占位记录（name="默认风格"）
  - 确保快速创作与向导的数据库结构一致

#### 3.1.2 `run_creation_workflow` 返回增强
- 后端返回结果中新增 `scene_id` 字段 (`story_commands.rs:2197-2205`)
- 前端可基于 `scene_id` 直接跳转到 Scene 编辑器

### Phase 2: 前端改造 ✅

#### 3.2.1 快速创作自动跳转
- `Stories.tsx` 中 `handleQuickCreate` 创作成功后立即：
  - `setCurrentStory(story)` + `setCurrentView('scenes')`
  - 用户无需手动寻找新创建的 Scene

#### 3.2.2 死代码清理
- 删除 `CreationWizard.tsx`（~1384 行未注册路由的死代码页面）
- 从 `App.tsx` 中移除 `CreationWizard` 导入
- `Stories.tsx` 中 `handleWizardCreate` 改为导航到 Dashboard（`NovelCreationWizard` 所在位置）

### Phase 3: 回归验证 ✅
- **后端测试**：252 passed, 0 failed
- **前端构建**：成功（无类型错误）
- **前端 type-check**：通过

## 4. 关键决策记录

| # | 决策 | 结论 |
|---|------|------|
| 1 | `NovelCreationWizard` vs `CreationWizard` | 保留 `NovelCreationWizard`（Dashboard 在用），删除 `CreationWizard`（死代码） |
| 2 | 旧快速创作的 Chapter 数据 | 保留兼容，但新快速创作改走 Scene 路径 |
| 3 | 后台 enrich 与用户编辑冲突 | 后台只 enrich 未编辑字段，正文修正仅在用户未修改时自动应用 |

## 5. Phase 1B 已完成

### 5.1 后台 enrich 任务 ✅
- **`CreationWorkflowEngine::enrich_story_elements`** (`engine.rs:793-950`)
  - 在 `AiOnly` 模式返回后 spawn 后台任务执行 (`story_commands.rs:2200-2212`)
  - 使用单次 LLM 调用从 Scene 正文生成完整要素（WorldBuilding + Characters + WritingStyle）
  - 解析 JSON 响应，替换占位记录为真实内容
  - 日志追踪：`[enrich] Updated world_building/characters/writing_style for story_id=...`

### 5.2 自动修正机制 ✅
- **enrich 后一致性检查** (`engine.rs:922-969`)
  - enrich 完成后，读取刚更新的 WorldBuilding/Characters/WritingStyle
  - 构建一致性检查 prompt，让 LLM 检测正文与要素的冲突
  - 如有冲突，自动更新 `Scene.content`
  - 如无冲突，LLM 回复"无需修正"，跳过更新
  - 修正仅在 enrich 完成后执行，不影响用户已编辑的内容

### 5.3 向导前端统一 ✅
- **`Stories.tsx` 直接集成 `NovelCreationWizard`**
  - 导入 `NovelCreationWizard` 组件和 `createStoryWithWizard` API
  - 添加 `isWizardOpen` + `wizardStory` + `isWizardCreating` state
  - `handleWizardCreate` 打开 wizard modal（不再跳转 Dashboard）
  - `handleWizardComplete` 调用 `createStoryWithWizard`，成功后导航到 Scene 编辑器

## 6. 长期改进项（已完成）

### 6.1 事务保护
- 为 6 个 Repository 的关键写入方法添加 `_in_tx` 变体：
  - `StoryRepository::create_in_tx`
  - `WorldBuildingRepository::create_in_tx` / `update_in_tx`
  - `CharacterRepository::create_in_tx`
  - `WritingStyleRepository::create_in_tx` / `update_in_tx`
  - `SceneRepository::create_in_tx` / `update_in_tx`（重构了内部事务，避免嵌套冲突）
  - `KnowledgeGraphRepository::create_entity_in_tx` / `create_relation_in_tx`
- `create_story_with_wizard` 改为双事务模式：
  - **事务 1**：核心要素持久化（Story + WorldBuilding + Characters + WritingStyle + Scene），步骤 1-5 原子提交
  - **异步 Ingest**：LLM 调用在事务外执行（避免同步 SQLite 事务阻塞 I/O）
  - **事务 2**：KG 数据保存（entities + relations），ingest 成功后原子提交
- 原 `create`/`update` 方法复用 `_in_tx` 变体，保持向后兼容

### 6.2 代码重构
- 提取 `persist_wizard_elements_in_tx` 私有函数（`story_commands.rs`）
- 封装 Story/WorldBuilding/Character/WritingStyle/Scene 的完整创建与更新逻辑
- 该函数可直接被 `CreationWorkflowEngine` 复用（传入占位数据即可）

## 7. 当前实施状态

- [x] 架构方案设计与用户确认
- [x] Phase 1A: 后端 Pipeline 核心重构（Ingestion 改 Scene + 占位要素）
- [x] Phase 2: 前端快速创作改造（自动跳转 + 死代码清理）
- [x] Phase 3: 回归验证（252 测试通过 + 前端构建成功）
- [x] Phase 1B-1: 后台 enrich 任务
- [x] Phase 1B-2: Stories.tsx 集成 NovelCreationWizard
- [x] Phase 1B-3: 自动修正机制
- [x] 事务保护（长期改进）
- [x] `create_story_with_wizard` 代码重构（长期改进）

---
*生成时间：2026-05-24*
*最后更新：2026-05-24*
*状态：全部完成*
