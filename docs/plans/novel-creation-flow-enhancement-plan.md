# 小说创作流程功能补全计划 — 修复"有设计但未集成"问题

## 上下文

通过全面代码审计发现，StoryMoss 后端设计了大量高级子系统（合同、提交链、追读力、预检、自动化、叙事审计、风格进化、状态管理等），但多数未真正融入小说创作的核心流程。前端 `StorySystem.tsx` 虽有展示界面，但大量功能仅为只读，缺少操作入口。

**当前创作流程**：CreationWizard → create_story → writer_agent_execute → update_chapter → finalize_draft。该流程基本绕过了所有高级子系统。

**本次目标**：将已设计但未集成的功能逐步补入创作流程，使它们从"摆设"变为"有用"。

---

## 发现的问题清单

| # | 功能模块 | 设计状态 | 集成状态 | 前端状态 | 优先级 |
|---|---------|---------|---------|---------|--------|
| 1 | **合同/提交链** (`story_system/`) | 命令、前端展示页齐全 | 提交流未触发；合同为只读 | StorySystem 有查看标签，无创建/提交按钮 | **高** |
| 2 | **追读力** (`reading_power/`) | Evaluator + DebtManager 完整 | 从未自动调用；仅在 StorySystem 只读展示 | 无评估按钮 | **高** |
| 3 | **预检** (`story_system/preflight.rs`) | 接口存在 | 存根实现（永远返回 ready），writer 前未调用 | 无 UI | **高** |
| 4 | **自动化触发** (`automation/`) | Service + 4 默认触发器 + 处理器 | create_story/update_chapter/create_chapter 已触发；create_scene/update_scene/finalize_draft 未触发 | 无自动化规则配置 UI | **中** |
| 5 | **记忆语义检索** (`memory/orchestrator.rs`) | QueryPipeline 实现完整 | build_with_query() 未被调用；标准 build() 不走语义检索 | StorySystem 记忆标签只读 | **中** |
| 6 | **叙事审计** (`narrative/audit.rs`) | StoryStructureAuditor 完整 | 只有单场景 audit 命令；无 story-level 命令 | 无审计面板 | **中** |
| 7 | **风格进化** (`creative_engine/style/evolution.rs`) | StyleEvolutionEngine 完整 | 仅测试使用；未接入审校后反馈 | 无 UI | **低** |
| 8 | **StoryStateManager** (`state/manager.rs`) | 完整实现 + DB 表 | 从未实例化；无命令暴露 | 无 UI | **低/废弃** |
| 9 | **Collab/Chat** (`collab/`, `chat/`) | 完整实现 + DB 表 + WebSocket | 从未实例化 | Collab 有 Chapters.tsx 连接按钮但无功能页；Chat 完全无 UI | **低/废弃** |

---

## 架构决策（Brainstorming 确认）

### 方案选择：事件驱动集成（Event-Driven）

**用户决策**：选择方案 3（事件驱动）。
**运行模式**：B+C 混合（后台自动运行 + 设置中可开关）。
**展示方式**：B+C 混合（分散嵌入 + 智能浮动），幕前界面克制显示以保持沉浸式写作体验。

### 优先级（用户确认）

1. **追读力 + 预检 + 记忆检索**（最先集成）
2. **合同/提交链 + 自动化**（其次）
3. **审计 + 风格进化**（再次）
4. **StoryStateManager + Collab/Chat**（最后或废弃）

### 核心机制

将 `AutomationService` 从"Ghost 系统"升级为**创作增强中枢**。

**事件发布层**：在现有命令的关键节点统一发布事件（不直接调用功能模块）。

**事件处理层**：每个功能模块注册为独立的 `AutomationHandler`，通过开关控制启用/禁用。

**结果展示层**：幕后（StorySystem）展示详细结果，幕前仅通过轻量 toast/inline badge 提示。

---

## 实施计划（分 4 个阶段）

### Phase 1: 核心创作流补漏（高优先级）

#### 1.1 追读力自动评估集成
**目标**：在内容生成/保存/定稿时自动评估追读力。

**后端** `src-tauri/src/commands_v3.rs`：
- 在 `update_scene()` 中，当 `content` 更新后，调用 `ReadingPowerEvaluator::evaluate_chapter()`（或适配为 scene-level 评估）。
- 在 `finalize_draft()`（pipeline/finalize.rs）完成后，调用 `ReadingPowerEvaluator::evaluate_chapter()`。

**后端** `src-tauri/src/reading_power/mod.rs`：
- 检查 `evaluate_chapter` 是否接受 scene-level 输入；如仅 chapter-level，则在 scene finalize 时降级为"仅记录 scene 字数"，在 chapter 级别触发完整评估。

**前端** `src-frontend/src/pages/StorySystem.tsx`：
- "追读力"标签页添加"重新评估"按钮，调用 `evaluateReadingPower` API。

**前端** `src-frontend/src/services/tauri.ts`：
- 确认 `evaluateReadingPower` 命令已注册。

---

#### 1.2 合同与提交链前端可操作化
**目标**：让 StorySystem 的合同和提交从"只读"变为"可创建/可提交"。

**后端** `src-tauri/src/story_system/mod.rs` / `contract_builder.rs` / `preflight.rs`：
- `contract_builder.rs`：补充 `build_master_setting_contract(story_id, world_building)` 的实现。
- `preflight.rs`：将存根替换为真实检查：检查必需字段（contract 是否存在、outline 是否非空、角色是否 >0）。

**后端** `src-tauri/src/lib.rs`：
- 确认 `create_master_setting`, `create_chapter_contract`, `init_chapter_commit`, `apply_chapter_commit` 命令已正确注册。

**前端** `src-frontend/src/pages/StorySystem.tsx`：
- "合同"标签页：当 `contract_tree` 为空时，显示"生成世界观合同"按钮，调用 `createMasterSetting`。
- "合同"标签页：添加"生成章节合同"按钮（选择章节后），调用 `createChapterContract`。
- "提交链"标签页：当 `commits` 为空时，显示"初始化提交"按钮，调用 `initChapterCommit`。
- "提交链"标签页：每条 commit 旁添加"应用提交"按钮，调用 `applyChapterCommit`。

**前端** `src-frontend/src/services/tauri.ts`：
- 确认上述 API 已导出。

---

#### 1.3 Writer Agent 预检集成
**目标**：在 AI 生成内容前执行预检，提前发现阻塞问题。

**后端** `src-tauri/src/story_system/preflight.rs`：
- 实现真实预检逻辑：
  - 检查 story 是否有关联 contract（master setting + chapter contract）
  - 检查当前 scene 是否有 outline
  - 检查角色列表是否非空
  - 返回 `PreflightResult { ready, issues, blockers }`

**后端** `src-tauri/src/agents/service.rs`：
- 在 `execute_writer_raw()` 开头（line ~335），在 `build_writer_prompt` 之前调用 `PreflightChecker::check()`。
- 如果 `ready == false`，返回错误 `AppError::PreflightFailed { issues }`，让前端展示阻塞原因。

**前端** `src-frontend/src/pages/`（写作界面）：
- 在触发 writer agent 前，可显示预检状态（可选优化，Phase 1 可只做后端阻断）。

---

#### 1.4 自动化触发补全
**目标**：让自动化事件覆盖所有关键生命周期节点。

**后端** `src-tauri/src/commands_v3.rs`：
- `create_scene()`：在创建成功后，调用 `AutomationService::trigger_event(TriggerEvent::SceneCreated { story_id, scene_id })`。
- `update_scene()`：在更新成功后，调用 `TriggerEvent::SceneContentUpdated { story_id, scene_id }`。

**后端** `src-tauri/src/pipeline/finalize.rs`：
- `finalize_draft()`：在定稿成功后，调用 `TriggerEvent::ChapterFinalized { story_id, chapter_id }`。

**前端**：Phase 1 暂不需要新 UI（自动化规则配置放在 Phase 2）。

---

### Phase 2: 记忆与语义检索增强（中优先级）

#### 2.1 QueryPipeline 集成到标准 Context Build
**目标**：让语义检索成为 Writer Agent 上下文的标准组成部分。

**后端** `src-tauri/src/creative_engine/context_builder.rs`：
- 修改 `build()` 方法（line 61-199）：
  - 在构建完基础 context 后，检查 `current_content` 或 `selected_text` 长度是否 >= 10。
  - 如果是，实例化 `QueryPipeline`，调用 `query()` 进行语义检索。
  - 将检索结果合并到 `context.scene_structure` 或新增 `context.semantic_query_results` 字段。

**后端** `src-tauri/src/agents/mod.rs`（AgentContext）：
- 在 `AgentContext` 中新增 `semantic_results: Option<Vec<String>>` 字段（或复用 `scene_structure`）。

**后端** `src-tauri/src/agents/service.rs`（build_writer_prompt）：
- 在 prompt 中增加 "相关记忆检索" 段落，展示语义检索结果。

**前端** `src-frontend/src/pages/StorySystem.tsx`：
- "记忆"标签页添加"构建记忆包"按钮，调用 `buildMemoryPack`。

---

### Phase 3: 审计与风格进化（中优先级）

#### 3.1 叙事审计 story-level 命令与前端面板
**目标**：让用户能对整部作品进行结构健康检查。

**后端** `src-tauri/src/narrative/audit.rs`：
- 新增 `audit_story(story_id)` 命令，遍历 story 的所有 scene/chapter，调用现有 5 维度审计方法。
- 聚合各维度结果，返回 `StoryAnalysisReport`。

**后端** `src-tauri/src/lib.rs`：
- 注册 `audit_story` Tauri 命令。

**前端** `src-frontend/src/pages/StorySystem.tsx`：
- 新增"审计"标签页，显示：
  - "运行全面审计"按钮
  - 5 维度雷达图/评分卡
  - 发现的问题列表（按严重程度分级）

---

#### 3.2 风格进化接入审校反馈
**目标**：让 Anti-AI Review 和 Pipeline Review 的结果能反馈到 StyleDNA 进化。

**后端** `src-tauri/src/creative_engine/style/evolution.rs`：
- 确认 `StyleEvolutionEngine::evolve_from_reviews()` 接口。

**后端** `src-tauri/src/pipeline/finalize.rs` 或 `src-tauri/src/anti_ai/mod.rs`：
- 在 `run_review()` / `anti_ai_review()` 完成后，如用户确认接受审校结果，调用 `StyleEvolutionEngine::evolve_from_reviews()`。
- 更新 story 的 `style_dna` 记录。

**前端** `src-frontend/src/pages/StorySystem.tsx`：
- "风格 DNA"标签页添加"基于审校进化风格"按钮（或自动提示）。

---

### Phase 4: 废弃系统清理与决策（低优先级）

#### 4.1 StoryStateManager 决策
- 评估是否值得维护。该模块有完整的运行时状态设计（角色弧、情节进展、世界观状态等），但与现有 `CanonicalStateManager` 和 `StateSync` 有功能重叠。
- **建议**：标记为废弃或在 README/PROJECT_STATUS 中注明"暂不维护"，避免误导。如未来需要运行时状态机再启用。

#### 4.2 Collab / Chat 决策
- Collab 的 WebSocket 服务器已在 `lib.rs` 启动，但 CollabManager 从未使用。
- Chat 模块完全未暴露。
- **建议**：在 `lib.rs` 中注释掉 WebSocket 服务器启动（减少资源占用），并在文档中标记为"预留功能"。前端已有的 Chapters.tsx 协作按钮可保留但显示"即将推出"。

---

## 关键文件清单

| 阶段 | 文件路径 | 操作 |
|------|---------|------|
| P1 | `src-tauri/src/reading_power/mod.rs` | 暴露 scene-level 评估接口 |
| P1 | `src-tauri/src/commands_v3.rs` | update_scene / finalize 后调用追读力评估 |
| P1 | `src-tauri/src/story_system/contract_builder.rs` | 补充实现 |
| P1 | `src-tauri/src/story_system/preflight.rs` | 实现真实预检逻辑 |
| P1 | `src-tauri/src/agents/service.rs` | 集成预检到 writer 流程 |
| P1 | `src-tauri/src/commands_v3.rs` | 补全 automation trigger |
| P1 | `src-tauri/src/pipeline/finalize.rs` | finalize 后 automation trigger |
| P1 | `src-frontend/src/pages/StorySystem.tsx` | 添加合同/提交/追读力操作按钮 |
| P2 | `src-tauri/src/creative_engine/context_builder.rs` | build() 中集成 QueryPipeline |
| P2 | `src-tauri/src/agents/mod.rs` | AgentContext 新增语义结果字段 |
| P2 | `src-tauri/src/agents/service.rs` | prompt 中展示语义检索结果 |
| P3 | `src-tauri/src/narrative/audit.rs` | 新增 audit_story 命令 |
| P3 | `src-tauri/src/lib.rs` | 注册 audit_story |
| P3 | `src-frontend/src/pages/StorySystem.tsx` | 新增审计标签页 |
| P3 | `src-tauri/src/creative_engine/style/evolution.rs` | 接入 review 反馈 |
| P4 | `src-tauri/src/lib.rs` | 注释 WebSocket 启动（可选） |

---

## 验证方式

1. **编译验证**：`cargo check`（后端）、`npm run build`（前端）
2. **端到端验证**：
   - Phase 1：在 StorySystem 页面能看到"生成合同"、"初始化提交"、"评估追读力"按钮，点击后成功执行。
   - Phase 1：Writer Agent 在缺少 contract/outline 时返回明确的预检错误。
   - Phase 1：创建 scene / finalize draft 后，检查 automation 事件队列是否有新事件。
   - Phase 2：Writer Agent 执行后，检查 AgentContext 中是否包含语义检索结果。
   - Phase 3：运行"全面审计"后，StorySystem 审计标签页显示 5 维度评分。
