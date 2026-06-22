# StoryForge (草苔) 开发路线图

> 最后更新: 2026-06-22（v0.23.11）

## ✅ v0.23.x 已实施完成

### 🎯 TriShot 三击生成管线 ✅ (v0.23.0)
- [x] GenerationMode::TriShot 三击模式（与 Fast/TimeSliced/Full 并存）
- [x] prompt_synthesis 模块（manifest + synthesizer + refiner）
- [x] GatewayExecutor::select_fastest_profile + generate_with_fastest
- [x] PlanExecutor TriShot 快速路径（跳过计划生成 LLM）
- [x] PlanStep::long_running 跳过 90s 步超时
- [x] execute_trishot 完整管线（Call 1 → Call 2 → Call 3 + 预算守卫）
- [x] BGP-2 auto_rewrite_executor（HIGH 自动改写 / LOW 建议）
- [x] SyncEvent::ContentAutoRevised / RevisionSuggested
- [x] 前端「三击模式」配置选项
- [x] BGP-3 后台 IngestPipeline（补 smart_execute 路径缺口）
- [x] BGP-1/BGP-4 后台审计+洞察链式 spawn
- [x] silent_background 白名单扩展（4 个新标签）

### 🧩 v0.23.4 智能层闭环落地 ✅ (2026-06-21)
- [x] LLM JSON mode 原生支持（`ResponseFormat::JsonObject`）
- [x] OpenAI/Ollama 适配器结构化输出接线
- [x] Review/Refine Pipeline 解析 `refinement_notes`
- [x] `MemoryBudget::for_task_type` 强类型化预算参数
- [x] 拆书存储统一：`reference_characters` / `reference_scenes` 删除，汇入 `narrative_*` 表
- [x] 迁移 `V100__拆书存储统一_删除_reference_表.sql`

### 🎨 v0.23.5 CI 格式化修复 ✅ (2026-06-21)
- [x] Rust nightly `cargo fmt` 格式化差异清零
- [x] 前端 Prettier 格式化差异清零
- [x] GitHub Actions `rust-check` / `frontend-check` 通过

### 🐛 v0.23.6 修复 macOS 启动崩溃 ✅ (2026-06-22)
- [x] 修复 `state() called before manage() for Arc<dyn VectorStore>` 启动 panic
- [x] `LanceVectorStore` 创建与 `app.manage` 提前到依赖组件之前
- [x] 全平台 CI 构建通过，生成 `.dmg` / `.deb` / `.msi`

### 📋 v0.23.7 诊断信息增强 ✅ (2026-06-22)
- [x] 修复诊断卡片版本号硬编码为 `0.16.0`
- [x] 修复前端/后端超时文案硬编码 `200s` / `180s`
- [x] 诊断信息新增 AI 生成模式、当前模型 ID/名称/提供商/端点
- [x] 诊断信息新增最后调用模型与最后发给 LLM 的提示词全文
- [x] 后端 `LlmService` 发射 `llm-prompt-sent` 事件供前端诊断捕获

### 🚀 v0.23.8 AI 进度指示精细化 ✅ (2026-06-22)
- [x] `LlmGeneratingProgress` 新增 `model_id`、`provider`、`prompt_chars`、`prompt_tokens`、`response_tokens`
- [x] 进度文案具体化：连接模型、组合提示词、等待回应、模型回应 token 数、解析结果
- [x] 新增 `diagnostics::DiagnosticStore` 与 `get_last_llm_prompt` 命令
- [x] 解决大提示词事件丢失导致诊断“未捕获”的问题

### 📚 v0.23.9 运行时创作资产能力清单 ✅ (2026-06-22)
- [x] 应用启动时自动生成并刷新全部系统创作资产目录
- [x] `AssetCapabilityManifest` 注入 Tauri State
- [x] TriShot Call 1 prompt 注入【系统可用创作资产目录】
- [x] TriShot Call 3 透传 `selected_asset_ids` / `asset_tags` 给 ModelGateway
- [x] ModelGateway dispatcher 识别 methodology/beat_card/story_engine/pressure_relationship/style_dna/skill 等标签
- [x] 修复 TriShot `request_id` 错误赋值、Call 1 无预算守卫

### 🎯 v0.23.10 模型网关优先使用当前活跃模型 ✅ (2026-06-22)
- [x] `select_fastest_profile` 优先使用当前 `active profile`（健康且 TTFB 不比最快模型差太多）
- [x] `select_candidates` 保证活跃模型始终出现在候选链中

### 🛡️ v0.23.11 诊断提示词过滤探测/静默调用 ✅ (2026-06-22)
- [x] 静默/探测调用不再更新 `DiagnosticStore` 和 `llm-prompt-sent` 事件
- [x] 避免 `model_gateway_probe` 的 `Respond with exactly the word OK.` 覆盖诊断提示词

## ✅ v0.22.x 已实施完成

### 🧩 「异星球末世生存」复合题材创作流程优化 ✅ (v0.22.4)
- [x] GenreResolver 题材解析服务
- [x] GenreProfile 中文别名扩展
- [x] StrategySelector / build_selected_strategy / story_concept_prompt 接入 GenreResolver
- [x] AssetNode tags 与资产同步标签注入
- [x] IntentionGraphPlanner 复合题材资产补充发现
- [x] GatewayRequest asset_tags / discovered_asset_ids 透传
- [x] TaskClassifier / GatewayExecutor 资产标签感知调度
- [x] WriteTimeBundle secondary_genre_profile_strategy 复合题材续写补强

### 🔐 钥匙串彻底移除 + 模型健康报告自动刷新 ✅ (v0.22.3)
- [x] 移除 keyring crate（全平台依赖）
- [x] 移除 secure_storage 模块
- [x] API Key 改为直接存 SQLite
- [x] 模型健康报告每 30 秒自动刷新
- [x] AppConfig.load() 热路径冗余调用消除
- [x] Phase A：TimeSliced 路径全资产注入（StyleDNA六维+方法论+体裁画像+写作策略）
- [x] Phase B：Inspector 全资产注入（体裁画像+角色状态+活跃冲突+四元组+方法论）
- [x] Phase C：意图感知调度接线（agent_type→intent 自动推导，activate classify_by_intention）
- [x] Phase D：算力档案消费闭环（CapabilityProfile TTFB/TPS 参与候选排序）
- [x] Phase E：资产→生成参数规则映射（asset_params.rs）
- [x] Phase F：GenreProfile 推荐资产字段（Migration 96 + 4 新列 + 种子数据 7 题材）

### 提示词全量可配置化 ✅
- [x] 79 个提示词全部纳入 PromptRegistry（21 个分类）
- [x] 前端 Monaco 编辑器 + 批量导入/导出
- [x] 40+ 个原硬编码提示词全部接入 registry
- [x] 15 个假接入 key 修复为真实 DB 覆盖

### SING 意图图集成 ✅
- [x] Migration 95：6 张意图图表
- [x] 意图合成流水线（LLM 增强 + 规则回退）
- [x] PPR 分层发现
- [x] 动态 ReAct 执行
- [x] IntentionGraphPlanner × PlanExecutor 集成
- [x] 前端诊断面板（IntentionGraphDiagnostics）

### v0.20.x 基础设施 ✅
- [x] Phase 1-5: SING 数据层/离线合成/分层发现/PlanGenerator重构/动态ReAct
- [x] Phase 6: 模型网关意图感知集成
- [x] Phase 7: 前端意图图诊断面板
- [x] P0 断环修复: 资产同步/意图分类/执行图持久化/LLM合成/PPR传播
- [x] 真实模型测试（Gemma4-e2b, 6/6）
- [x] Multi-Agent Sessions（6种助手类型）

### Phase 4: AI 智能生成 ✅
**状态**: 完整实现

- [x] NovelCreationAgent
- [x] NovelCreationWizard 组件
- [x] 卡片式选择 UI
- [x] 首个场景自动生成

### Phase 5: 工作室配置系统 ✅
**状态**: 完整实现

- [x] StudioConfig 模型
- [x] StudioManager（导入/导出）
- [x] ZIP 格式支持
- [x] 默认主题配置

### Phase 6: 场景版本系统 ✅ (v3.1.0)
**状态**: 完整实现

- [x] SceneVersionRepository（版本CRUD）
- [x] SceneVersionService（比较、恢复、统计）
- [x] VersionTimeline 组件（垂直时间线）
- [x] DiffViewer 组件（差异对比）
- [x] ConfidenceIndicator 组件（置信度可视化）
- [x] 版本链管理（supersession）

### Phase 7: 混合搜索系统 ✅ (v3.1.0)
**状态**: 完整实现

- [x] BM25 Search（CJK二元组分词）
- [x] Hybrid Search（RRF融合排序）
- [x] Entity Hybrid Search（名称+向量）
- [x] 可配置权重和参数

### Phase 8: 记忆保留系统 ✅ (v3.1.0)
**状态**: 完整实现

- [x] RetentionManager（遗忘曲线计算）
- [x] 五级优先级分类
- [x] 遗忘时间预测
- [x] 保留报告生成
- [x] 上下文窗口优化

### Phase 9: 幕前界面重构与本地模型 ✅ (v3.1.1)
**状态**: 完整实现

- [x] 精简侧边栏（仅保留"幕后"按钮）
- [x] OKLCH 颜色系统重构（去除 AI 感模板色）
- [x] LXGW WenKai 字体替换（去除 Crimson/Inter）
- [x] Blockquote 与微交互重设计（Waza 原则）
- [x] 顶部动态状态栏
- [x] 底部 LLM 对话栏（悬停显示、模型状态灯、去除模式切换图标）
- [x] 流式对话交互（Enter 发送 / Shift+Enter 换行）
- [x] 本地三模型配置（Gemma / Qwen3.5 / bge-m3）
- [x] Tauri Windows 构建与打包（MSI + NSIS）
- [x] GitHub Actions CI 图标修复（macOS / Ubuntu）

---

### Phase 10: 设计-实现对齐修复 ✅ (v5.6.0)
**状态**: 全部完成

- [x] Scene 删除外键清理（chapters.scene_id → NULL）
- [x] Wizard 同步事件（story_created + data_refresh）
- [x] Character relationships 真实查询（character_relationships 表 JOIN）
- [x] Collab 文档 OT 重建（operations apply 重建内容）
- [x] Workflow EdgeCondition 条件求值（8 种运算符）
- [x] Task 心跳超时指数退避重试
- [x] Outline/Foreshadowing/Payoff 修改后同步事件
- [x] Cache 对称失效（sceneUpdated↔chapters、chapterDeleted↔scenes）
- [x] Workflow 节点 300s 超时
- [x] INGEST_COOLDOWN 24h 过期清理
- [x] FrontstageApp 真实 feedback（移除 mock learnings）
- [x] WritingStyle 更新同步事件
- [x] Workflow 并发守卫与重试幂等性
- [x] Pending vector SQLite 持久化
- [x] Task 执行 300s 超时

### Phase 11: 提示词全面可配置化 ✅ (v0.19.0)
**状态**: 全部完成

- [x] 35+ 内置提示词注册表（`prompts/registry.rs`）
- [x] 15 个 `PromptCategory` 分类体系
- [x] 雪花法 10 步提示词注入注册表
- [x] 5 个内置技能提示词映射（`skill_id_to_prompt_id`）
- [x] Memory / Knowledge / MultiAgent 模块接入注册表
- [x] 前端 PromptsPanel 重写（分类 + 搜索 + 批量重置 + 默认值预览）
- [x] GeneralSettings 精简为「提示词注册表」链接卡片
- [x] `reset_all_prompt_overrides` 批量重置 IPC
- [x] 运行时覆盖生效（`resolve_prompt()` 优先查 DB）

---

## 📊 v0.19.0 项目状态

| 模块 | 完成度 | 说明 |
|------|--------|------|
| 场景化叙事系统 | 100% | Scene 模型、StoryTimeline、SceneEditor |
| 增强记忆系统 | 100% | Ingest/Query Pipeline、Knowledge Graph、LanceDB 语义搜索、Pending Vector SQLite 持久化 |
| AI 智能生成 | 100% | NovelCreationAgent、Bootstrap 两阶段、创建向导、真实自适应学习反馈 |
| 工作室配置 | 100% | 导入/导出、主题系统 |
| 混合搜索 | 100% | BM25 + Vector RRF融合 + 语义嵌入 |
| 场景版本 | 100% | 版本历史、对比、恢复 |
| 记忆保留 | 100% | 遗忘曲线、优先级管理 |
| 幕前界面 | 100% | 精简侧边栏、幽灵文本、`/` 菜单 |
| 幕前幕后自动关联 | 100% | Chapter↔Scene 双向映射、state_sync、实时同步、Cache 对称失效完整、writingStyle/storySelected 缓存精确化 |
| 后台自动化 | 100% | Workflow 持久化、能力进化反馈环、向量索引闭环（Chapter + Scene）、Workflow 幂等性 |
| 本地模型配置 | 100% | 三模型集成 |
| 提示词可配置化 | 100% | 35+ 提示词注册表、15 分类、前端完整管理面板、运行时覆盖生效 |
| Tauri 构建 | 100% | MSI + NSIS 安装包 |
| 设计-实现对齐 | 100% | v5.6.4 Tauri IPC rename_all 修复 |
| **整体 v0.19.0** | **100%** | 核心功能全部完成 |

---

## 🚀 编译状态

```bash
$ cd src-frontend && npm run build
    vite v6.4.2 building for production...
    ✓ 2156 modules transformed.
    dist/                     655.75 kB │ gzip: 216.60 kB
```

```bash
$ cd src-tauri && cargo tauri build
    Finished release profile [optimized] target(s) in 8m 04s
       Built application at: target/release/storyforge
    Finished 3 bundles at:
        target/release/bundle/dmg/StoryForge_0.23.6_aarch64.dmg
        target/release/bundle/deb/storyforge_0.23.6_amd64.deb
        target/release/bundle/msi/StoryForge_0.23.6_x64_en-US.msi
```

```bash
$ cd src-tauri && cargo test --lib
    running 538 tests
    test result: ok. 538 passed; 0 failed; 2 ignored
```

✅ **编译成功** | ✅ **测试全绿** | ✅ **打包成功**

---

## 🆕 v3.1.1 新增依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| @tiptap/react | ^3.22.3 | 幕前富文本编辑器 |
| @tiptap/starter-kit | ^3.22.3 | TipTap 基础扩展 |
| @tiptap/extension-placeholder | ^3.22.3 | 占位符扩展 |

---

## 📋 后续路线图

### v3.2.x 进行中

- [x] LLM 真实 SSE 流式输出
- [x] Anthropic 适配器
- [x] Ollama 适配器
- [x] 实体嵌入持久化修复

#### 向量存储增强
- [x] SQLite 向量存储持久化（已替代 JSON-memory fallback）
- [ ] LanceDB 持久化存储（ blocked：Arrow 依赖与当前工具链冲突）
- [x] 实体向量持久化（`kg_entities.embedding` BLOB 读写修复）
- [x] 实体向量自动更新（属性变更时重新生成嵌入）
- [x] 语义搜索优化
- [ ] 向量索引性能优化

#### 知识图谱可视化
- [x] 实体关系图谱可视化
- [x] 交互式图谱浏览（双击聚焦、搜索筛选、类型过滤）
- [x] 实体详情弹窗
- [x] 关系强度可视化

#### 记忆系统增强
- [x] 自动归档系统（一键归档 + 恢复 + 已归档浏览）
- [x] 创建向导自动 Ingest
- [x] 实体嵌入持久化
- [x] 知识蒸馏
- [x] 记忆压缩

#### 协作功能
- [x] 评论和批注系统
- [x] 修订模式
- [x] 变更追踪

### v3.3.0 (中期计划)

#### 云端同步
- [ ] 用户账户系统
- [ ] 云存储集成
- [ ] 多设备同步

#### 协作写作增强
- [ ] 实时协作场景编辑
- [ ] 评论和批注系统
- [ ] 修订模式

#### 插件市场
- [ ] Skills 分享平台
- [ ] 主题市场
- [ ] Agent 模板市场

#### 导出增强
- [ ] 自定义导出模板
- [ ] 批量导出
- [ ] 自动发布集成

### v4.0.0 (长期计划)

#### 技术架构升级
- [ ] WebAssembly 前端 (Leptos)
- [ ] 自研小模型部署
- [ ] 边缘计算支持

#### 多人实时协作
- [ ] OT 算法完整实现
- [ ] 实时光标同步
- [ ] 冲突解决机制

#### 移动端支持
- [ ] iOS 应用
- [ ] Android 应用
- [ ] 响应式 Web 版本

#### 发布平台集成
- [ ] 起点中文网集成
- [ ] 晋江文学城集成
- [ ] 自出版平台 (Amazon KDP)

---

## 📈 历史版本

### v0.23.11 (2026-06-22)
- [x] 诊断提示词过滤探测/静默调用，避免被 probe prompt 覆盖

### v0.23.10 (2026-06-22)
- [x] `select_fastest_profile` 优先使用当前活跃模型，避免连到旧模型
- [x] `select_candidates` 候选链兜底活跃模型

### v0.23.9 (2026-06-22)
- [x] 运行时创作资产能力清单：启动时刷新全部系统资产并注入 TriShot/ModelGateway
- [x] TriShot Call 1 可见全局资产，Call 3 透传选中资产给模型网关
- [x] 修复 TriShot request_id 错误与 Call 1 预算守卫

### v0.23.8 (2026-06-22)
- [x] AI 进度指示精细化：连接模型、组合提示词、等待回应、模型回应、解析结果
- [x] 新增 `DiagnosticStore` 与 `get_last_llm_prompt` 命令，提升提示词诊断可靠性

### v0.23.7 (2026-06-22)
- [x] 诊断卡片版本号改为从 `package.json` 动态读取
- [x] 超时文案去硬编码，读取用户实际设置
- [x] 诊断信息新增 AI 生成模式、当前模型、最后 LLM 提示词

### v0.23.6 (2026-06-22)
- [x] 修复 macOS 启动崩溃（VectorStore State 初始化顺序）
- [x] 全平台 CI 构建通过（`.dmg` / `.deb` / `.msi`）

### v0.23.5 (2026-06-21)
- [x] CI 格式化修复（Rust nightly fmt + 前端 Prettier）
- [x] `rust-check` / `frontend-check` 通过

### v0.23.4 (2026-06-21)
- [x] LLM JSON mode 原生支持（OpenAI/Ollama）
- [x] Review/Refine Pipeline 结构化输出
- [x] MemoryPack 预算参数强类型化
- [x] 拆书存储统一，删除 `reference_characters` / `reference_scenes`

### v0.23.3 (2026-06-21)
- [x] MigrationRunner 交错执行修复
- [x] V092 测试基线 48 个失败清零
- [x] `narrative_*` 表 `status` 列补齐

### v0.23.2 (2026-06-21)
- [x] `SyncEvent::ChapterCommitted`
- [x] 前端编辑器状态收敛到 `frontstageStore`

### v0.23.1 (2026-06-21)
- [x] 全局单例清零（14 个）
- [x] 模块循环依赖斩断

### v0.23.0 (2026-06-21)
- [x] TriShot 三击生成管线
- [x] prompt_synthesis 模块
- [x] BGP-2 智能改写
- [x] 前端「三击模式」配置

### v3.1.1 (2026-04-13)
- [x] 幕前界面重构（Waza 设计原则）
- [x] OKLCH 颜色系统 / LXGW WenKai 字体
- [x] 本地三模型配置
- [x] Tauri Windows 构建打包
- [x] GitHub Actions CI 跨平台修复

### v3.1.0 (2025-04-13)
- [x] 混合搜索
- [x] 场景版本管理
- [x] 记忆保留曲线

### v3.0.0 (2025-04-12)
- [x] 场景化叙事架构
- [x] 增强记忆系统
- [x] AI 智能生成
- [x] 工作室配置

### v2.0.x (已完成)
- [x] 双界面架构 (幕前/幕后)
- [x] 技能系统
- [x] MCP 支持
- [x] 状态管理
- [x] 模型路由
- [x] 进化算法
- [x] 导出功能 (PDF/EPUB)

### v1.x (已完成)
- [x] 基础架构
- [x] LLM 集成
- [x] 数据库设计
- [x] 前端界面

---

## 🎯 优先级说明

| 优先级 | 说明 |
|--------|------|
| P0 | 核心功能，必须完成 |
| P1 | 重要功能，影响体验 |
| P2 | 增强功能，锦上添花 |
| P3 | 未来规划，长期目标 |

---

## 📚 相关文档

- [V3 架构计划](docs/plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计
- [CHANGELOG](CHANGELOG.md) - 版本变更记录
- [PROJECT_STATUS](PROJECT_STATUS.md) - 详细项目状态
