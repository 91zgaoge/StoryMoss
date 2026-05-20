# StoryForge (草苔) v0.7.3 项目完成状态

> 最后更新: 2026-05-20（v0.7.3 + 商业模式重构为功能订阅制 + 1:N Chapter↔Scene 架构完成 + SceneDividerNode 预留接口 + scene_commits 重命名）
> GitHub: https://github.com/91zgaoge/StoryForge

---

## ✅ 已完成功能

### v0.7.0 全面重构：AI 三审 Pipeline + 角色动态状态 + 用量统计 + 幕前指令升级（2026-05-15）

#### AI 三审 Pipeline 系统
| 功能 | 状态 | 说明 |
|------|------|------|
| Pipeline 核心架构 | ✅ | Refine/Review/Finalize + 后处理 |
| `run_refine` | ✅ | AI 修稿：语言润色、结构调整、错别字修正 |
| `run_review` | ✅ | AI 审稿：多维度评分 + 问题列表 + 改进建议 |
| `run_finalize` | ✅ | 定稿：后处理步骤自动执行 |
| 后处理步骤追踪 | ✅ | `PostProcessStep` Running/Success/Failed 状态，关键/非关键分类 |
| `run_character_cards` | ✅ | LLM 驱动角色状态解析：Prompt → JSON → 批量更新 |
| `kb_import` | ✅ | 知识库自动更新 |
| `chapter_notes` | ✅ | 章节笔记自动生成 |
| `style_analysis` | ✅ | 风格分析自动执行 |
| 前端 Pipeline 面板 | ✅ | Actions/Drafts/Reviews 三标签页 |
| 场景进度看板 | ✅ | execution_stage 彩色徽章 + 多色进度条 |
| 幕前 `/` 指令打通 | ✅ | 修稿/审稿/定稿 → `pipeline_refine`/`pipeline_review`/`pipeline_finalize` |

#### 角色动态状态系统
| 功能 | 状态 | 说明 |
|------|------|------|
| `cs_location` | ✅ | 当前位置 |
| `cs_power_level` | ✅ | 实力等级 |
| `cs_physical_state` | ✅ | 身体状态 |
| `cs_mental_state` | ✅ | 心理状态 |
| `cs_key_items` | ✅ | 关键物品 |
| `cs_recent_events` | ✅ | 近期事件 |
| `cs_updated_at_chapter` | ✅ | 最后更新章节号 |
| LLM 自动解析 | ✅ | 定稿时自动从章节内容提取状态变化 |
| `CharacterStatePanel` | ✅ | 可折叠状态面板，内联编辑 |

#### 用量统计与可观测性
| 功能 | 状态 | 说明 |
|------|------|------|
| `UsageStats` 页面 | ✅ | 幕后独立页面，Sidebar 导航 |
| 全局统计 | ✅ | 总次数 / 总 token / 平均响应时间 / 成功率 |
| 单故事统计 | ✅ | 按故事维度聚合 |
| 最近调用记录 | ✅ | 最近 20 条明细表 |
| `get_llm_call_stats` | ✅ | IPC 命令 |
| `get_recent_llm_calls` | ✅ | IPC 命令 |

---

### v0.7.1 架构优化：聚合提交 + 导出完整性 + 组件提取（2026-05-17）

#### SceneCommitService 防抖聚合提交
| 功能 | 状态 | 说明 |
|------|------|------|
| `SCENE_COMMIT_DEBOUNCE` | ✅ | 全局防抖状态，30 秒空闲延迟 |
| `auto_ingest_chapter` 移除 | ✅ | 消除与 Projection Writer 的重复索引 |
| `SceneCommitService::auto_commit` | ✅ | 取代独立摄取，驱动 Vector/Memory ProjectionWriter |
| `update_chapter`/`create_chapter` 接入 | ✅ | 统一调用 `auto_commit` |

#### 导出聚合完整性
| 功能 | 状态 | 说明 |
|------|------|------|
| 空章节场景聚合 | ✅ | 按 `sequence_number` 排序填充 |
| Markdown 导出完整 | ✅ | 空章节自动聚合场景内容 |
| HTML 导出完整 | ✅ | 同上 |
| PlainText 导出完整 | ✅ | 同上 |
| JSON 导出含 scenes | ✅ | 全数据便携导出 |

#### 前端组件提取重构
| 功能 | 状态 | 说明 |
|------|------|------|
| `Settings.tsx` 拆分 | ✅ | 8 个子组件提取到 `pages/settings/` |
| `SceneEditor.tsx` 拆分 | ✅ | `SceneAuditPanel` + `SceneAnnotationPanel` 提取到 `scene-editor/` |
| `StoryTimeline.tsx` 徽章 | ✅ | `execution_stage` 彩色徽章 + 叙事阶段双轨可视化 |
| 未使用导入清理 | ✅ | `Image`/`createLogger`/`Clock`/`Eye`/`FileText` 等 |

---

### v0.7.3 架构对齐：商业模式重构 + 1:N 架构完成 + SceneDivider 预留接口（2026-05-20）

#### 商业模式重构：订阅解锁功能，非模型配额
| 功能 | 状态 | 说明 |
|------|------|------|
| `SubscriptionService` 精简 | ✅ | 移除所有配额计量逻辑，仅保留 `has_feature_access` 功能开关 |
| `AppError::SubscriptionRequired` | ✅ | 取代 `QuotaExceeded`，错误码 `SUBSCRIPTION_REQUIRED` |
| `has_feature_access` 细粒度权限 | ✅ | Free 基础功能 / Pro 解锁 Pipeline / 拆书 / 自动续写 / 自动修改 |
| `log_ai_usage` 纯统计 | ✅ | `ai_usage_logs` 仅记录明细，不参与配额控制 |

#### 1:N Chapter↔Scene 架构完成（Phase 4）
| 功能 | 状态 | 说明 |
|------|------|------|
| 废弃 `chapters.scene_id` | ✅ | Migration 71 移除 `chapters.scene_id` 列 |
| 全链路 `scenes.chapter_id` | ✅ | Repository / Service / IPC 全部改为反向查询 |
| `SceneCommitService` 取代 `ChapterCommitService` | ✅ | 提交粒度对齐 Scene；Migration 70 重命名表 |
| `SceneDividerRepository` | ✅ | `create`/`get_by_chapter`/`set_dividers`/`delete` |
| `scene_divider_nodes` 表 | ✅ | Migration 72，支持章节内场景边界标记 |

---

### v0.7.2 功能增强：存储同构化 + MCP 动态注册 + 聚合编辑 + 场景分隔节点 + LLM 取消（2026-05-19）

#### 拆书分析存储同构化
| 功能 | 状态 | 说明 |
|------|------|------|
| `reference_*` → `narrative_*` 写入 | ✅ | `BookDeconstructionExecutor` 保存到统一表，含 `source`/`status` 标记 |
| `get_analysis` 从 `narrative_*` 读取 | ✅ | 查询后转换回 `BookAnalysisResult`，API 零变动 |
| `delete_book` 级联清理 | ✅ | 同步删除 `narrative_characters`/`narrative_scenes`/`narrative_world_buildings` |
| `convert_to_story` 状态切换 | ✅ | `reference` → `active` 自动更新 |
| Migration 69 历史数据迁移 | ✅ | `reference_characters`/`reference_scenes` 自动汇入 `narrative_*` |

#### MCP 工具动态注册
| 功能 | 状态 | 说明 |
|------|------|------|
| `CapabilityRegistry` 实时同步 | ✅ | 外部 MCP 服务器连接后即时注册工具，无需重启 |
| 前缀命名空间 | ✅ | 内置 `mcp.builtin.*` / 外部 `mcp.{server_id}.*` |
| 动态注销 | ✅ | 断开时从 Registry 移除，防止调用失效工具 |

#### 1:N 聚合编辑数据库 Schema
| 功能 | 状态 | 说明 |
|------|------|------|
| Migration 68 | ✅ | `scene_commits` 新增 `chapter_id` 外键 |
| 全链路对齐 | ✅ | 数据结构 / Repository / Service 全部适配 |

#### TipTap SceneDividerNode
| 功能 | 状态 | 说明 |
|------|------|------|
| 原子块节点 | ✅ | `atom: true`，不可编辑，仅作为结构标记 |
| 可视化分隔 | ✅ | 编辑器内场景边界水平线 + 标题标签 |
| 事件扩展 | ✅ | click / mouseenter / mouseleave 支持 |

#### LLM 调用取消机制
| 功能 | 状态 | 说明 |
|------|------|------|
| `request_id` 级取消 | ✅ | `cancel_senders` HashMap 精确管理每条请求 |
| `cancel_generation` | ✅ | 按 `request_id` 发送取消信号，不影响其他请求 |
| 流式适配 | ✅ | `generate_stream` 每 chunk 前检查取消标志 |
| Orchestrator 透传 | ✅ | `WorkflowResult` 携带 `request_id`，上层可取消 |

#### AppError 结构化 IPC
| 功能 | 状态 | 说明 |
|------|------|------|
| 统一错误格式 | ✅ | `{ code, message, data }` |
| 错误码体系 | ✅ | `NOT_FOUND` / `VALIDATION_ERROR` / `LLM_ERROR` / `CANCELLED` / `TIMEOUT` 等 |
| 前端精准处理 | ✅ | `loggedInvoke` 按 `code` 分支处理 |

---

### v6.0.0 全面重构：质量管控闭环 + 类型安全 + 可观测性 + UX 微优化（2026-05-15）

#### Story System 合同驱动体系
| 功能 | 状态 | 说明 |
|------|------|------|
| MASTER_SETTING 合同 | ✅ | 故事级全局设定合同 |
| Volume 合同 | ✅ | 卷级设定合同 |
| Chapter 合同 | ✅ | 章节级设定与预期合同 |
| Review 合同 | ✅ | 审阅与修订合同 |
| CHAPTER_COMMIT 提交链 | ✅ | 写后真源分离，驱动 5 个 Projection Writer |
| StateProjectionWriter | ✅ | 解析 state_deltas 写入语义记忆 |
| IndexProjectionWriter | ✅ | 解析 entity_deltas 写入实体记忆 |
| SummaryProjectionWriter | ✅ | 自动生成章节摘要并持久化 |
| MemoryProjectionWriter | ✅ | 解析 accepted_events 写入事件记忆 |
| VectorProjectionWriter | ✅ | 章节摘要 embedding 写入 LanceDB |
| ContractTree 查询 | ✅ | 按故事/卷/章节层级查询合同树 |
| RuntimeContract 计算 | ✅ | 动态合并上层合同生成运行时合同 |

#### 三层记忆编排器
| 功能 | 状态 | 说明 |
|------|------|------|
| Working Memory | ✅ | 最近 5 章 + 活跃角色 + 开放伏笔 |
| Episodic Memory | ✅ | state_changes + relationships 时间线 |
| Semantic Memory | ✅ | 长期事实，按优先级和源章节窗口过滤 |
| MemoryPack 组装 | ✅ | 按任务类型（write/plan/review）动态分配预算 |
| 冲突检测与警告 | ✅ | 记忆项间矛盾检测 |

#### 追读力评估系统
| 功能 | 状态 | 说明 |
|------|------|------|
| Hook 检测 | ✅ | 悬念/冲突/转折三类钩子识别 |
| Coolpoint 追踪 | ✅ | 打脸/收获/揭秘爽点计数 |
| Micropayoff 微兑现 | ✅ | 章节内小承诺兑现检测 |
| Debt 债务追踪 | ✅ | 含利息与覆盖合同 |
| 综合评分 | ✅ | 0-100 追读力评分与趋势图 |

#### 43 体裁模板库
| 功能 | 状态 | 说明 |
|------|------|------|
| 内置 43 模板 | ✅ | 玄幻/仙侠/都市/历史/科幻/悬疑/言情/武侠/无限流/系统流/末日拓荒/灵气复苏/规则怪谈/模拟器流/盗墓流/星际机甲等 |
| 模板五要素 | ✅ | 核心基调、节奏策略、反模式、参考数据、典型结构 |
| 前端体裁选择 | ✅ | StorySystem 页面支持按体裁过滤和查看 |
| 模板外部化 | ✅ | genres.json 支持用户自定义编辑 |

#### Anti-AI 五维审查
| 功能 | 状态 | 说明 |
|------|------|------|
| 词汇/语法/叙事/情感/对话 | ✅ | 五维度评分与改进建议 |
| 导出前体检 | ✅ | ExportDialog 4 步流程，可选审查后再导出 |
| StorySystem 卡片 | ✅ | 从 Tab 降级为可展开卡片 |

#### 类型安全基座
| 功能 | 状态 | 说明 |
|------|------|------|
| ts-rs 类型导出 | ✅ | SyncEvent / FrontstageEvent / BackstageEvent 自动生成 TS 绑定 |
| 前端穷尽匹配 | ✅ | useSyncStore 使用 assertUnreachable 穷尽检查 |
| IPC 一致性脚本 | ✅ | verify-ipc-manifest.py 自动检测命令注册差异 |

#### 可靠性与可观测性
| 功能 | 状态 | 说明 |
|------|------|------|
| Ingest 作业追踪 | ✅ | ingest_jobs 表（Migration 55） |
| Ingest 健康指示器 | ✅ | 幕前顶栏 🧠 状态图标 |
| Projection 健康检查 | ✅ | check_projection_health 逐 Writer 展示状态 |
| 功能使用度量 | ✅ | feature_usage_logs 表（Migration 56）+ Settings 统计面板 |
| 技术债务清理 | ✅ | 删除 11 个已修复的 #[ignore] 测试 |

#### UX 微优化
| 功能 | 状态 | 说明 |
|------|------|------|
| 角色悬浮卡片 | ✅ | RichTextEditor hover 角色名显示微型浮卡 |
| 导出出版前体检 | ✅ | ExportDialog 新增健康检查步骤 |

---

### v5.3.0 叙事元素模型重构（2026-05-02）

| 功能 | 状态 | 说明 |
|------|------|------|
| 统一叙事元素模型 | ✅ | `narrative/` 模块 — 正向/逆向共用同一套数据结构 |
| GenesisPipeline | ✅ | 7步正向流程：概念→世界观→大纲→角色→场景→伏笔→知识图谱 |
| AnalysisPipeline | ✅ | 7步逆向流程：元数据→世界观→角色→场景→故事线→伏笔→知识图谱 |
| 统一进度系统 | ✅ | `PipelineProgressEvent` + `usePipelineProgress` Hook |
| 统一存储层 | ✅ | Migration 38 + `NarrativeRepository` |
| StoryHealthAnalyzer | ✅ | 6维度结构健康检查 + `analyze_story_structure` IPC |
| 向后兼容 | ✅ | 同时发射新旧事件，保留旧数据表 |

### v5.2.2 架构级重构（2026-05-02）

| 功能 | 状态 | 说明 |
|------|------|------|
| Bootstrap 两阶段执行 | ✅ | 即时阶段（概念+正文，2-3分钟）+ 后台阶段（世界观/角色/场景，异步） |
| 用户等待时间缩短 | ✅ | 从 10+ 分钟缩短到 2-3 分钟 |
| 前端后台进度感知 | ✅ | 状态栏显示"后台正在完善..."，完成后 toast 提示 |

### v5.2.1 新增修复（2026-05-02）

| 修复项 | 状态 | 说明 |
|--------|------|------|
| Bootstrap 超时延长 600s | ✅ | 匹配本地模型多步 LLM 调用耗时 |
| Bootstrap 进度密度增强 | ✅ | 每个 LLM 调用前后增加细粒度进度事件 |
| LLM 心跳 2s 间隔 | ✅ | 更快反馈，减少用户等待焦虑 |
| 后台窗口白屏修复 v5.2.1 | ✅ | 双重维度微调 + 800ms 延迟 + 前端双重重绘 |

### v5.2.0 新增功能（2026-05-02）

#### 🎯 设计-实现对齐全面修复

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| `WorkflowScheduler::run_instance` | ✅ | 100% | 完整 DAG 节点执行器，支持 8 种节点类型 |
| 通用 Workflow IPC 命令 | ✅ | 100% | register/create/start/get_status 4 个命令 |
| `CapabilityEvolutionEngine` | ✅ | 100% | JSON 持久化 + LLM 分析 + 自动记录集成 |
| 幕前↔场景双向同步 | ✅ | 100% | useSyncStore 刷新 + FrontstageApp 监听 + 3 秒防循环 |
| QueryPipeline 降级感知 | ✅ | 100% | `context-degraded` 事件 + 前端 toast |
| 废弃组件清理 | ✅ | 100% | `FrontstageToolbar` 从索引导出移除 |

### v3.4.0 新增功能（2026-04-18）

#### 🧠 Phase 1 - 地基重构：真实上下文 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| `StoryContextBuilder` | ✅ | 100% | 从真实数据库构建 Agent 上下文（世界观/角色/场景结构） |
| `QueryPipeline` | ✅ | 100% | 四阶段知识检索：CJK 分词 → 图谱扩展 → 预算控制 → 上下文组装 |
| `ContinuityEngine` | ✅ | 100% | 章节连续性追踪，确保角色/设定前后一致 |
| `ForeshadowingTracker` | ✅ | 100% | 伏笔埋设与回收追踪系统 |
| `IngestPipeline` 自动触发 | ✅ | 100% | 场景保存后自动摄取知识图谱 |
| 单元测试 | ✅ | 100% | 27 tests 全部通过 |

#### 🎭 Phase 2 - 方法论注入 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| `MethodologyEngine` | ✅ | 100% | 自动将方法论约束注入 Writer 系统提示词 |
| 雪花法 (10 步) | ✅ | 100% | 从一句话到完整小说的渐进细化 |
| 场景节拍表 (6 节拍) | ✅ | 100% | 开场→冲突→行动→转折→高潮→结局 |
| 英雄之旅 (12 阶段) | ✅ | 100% | Campbell 经典叙事结构 |
| 人物深度模型 (6 维) | ✅ | 100% | 性格/动机/关系/成长/语言/秘密 |
| `AgentOrchestrator` | ✅ | 100% | Writer→Inspector→Writer 质量反馈循环 |
| 单元测试 | ✅ | 100% | 34 tests 全部通过 |

#### 🎨 Phase 3 - 风格深度化 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| `StyleDNA` 六维模型 | ✅ | 100% | 词汇/句法/修辞/视角/情感/对白 |
| 10 种内置经典作家 DNA | ✅ | 100% | 金庸/张爱玲/海明威/村上春树/莫言/古典散文/现代极简/黑色侦探/武侠诗意/浪漫主义 |
| `StyleAnalyzer` | ✅ | 100% | 从文本提取 StyleDNA 指纹 |
| `StyleChecker` | ✅ | 100% | 对比文本与目标 DNA 的相似度 |
| `StyleDnaRepository` | ✅ | 100% | CRUD 操作 + 数据库迁移 |
| 单元测试 | ✅ | 100% | 45 tests 全部通过 |

#### 🔄 Phase 4 - 自适应学习 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| `FeedbackRecorder` | ✅ | 100% | 记录接受/拒绝/修改行为 |
| `PreferenceMiner` | ✅ | 100% | 五维度启发式偏好挖掘 |
| `AdaptiveGenerator` | ✅ | 100% | 动态调节 temperature/top-p/提示词权重 |
| `PromptPersonalizer` | ✅ | 100% | 将用户偏好注入系统提示词 |
| `AdaptiveLearningEngine` | ✅ | 100% | 统一入口整合反馈→挖掘→生成→个性化 |
| `UserFeedbackRepository` | ✅ | 100% | 数据库 CRUD + 统计查询 |
| `UserPreferenceRepository` | ✅ | 100% | 偏好 upsert + 按故事查询 |
| 单元测试 | ✅ | 100% | 54 tests 全部通过 |

#### 🏭 Phase 5 - 工作流闭环 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| `CreationWorkflowEngine` | ✅ | 100% | 7 阶段全自动工作流 |
| `QualityChecker` | ✅ | 100% | 四维质量评估（结构/人物/风格/情节） |
| 3 种创作模式 | ✅ | 100% | OneClick / AiDraftHumanEdit / HumanDraftAiPolish |
| 单元测试 | ✅ | 100% | 63 tests 全部通过 |

#### 🍃 品牌焕新 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| 全新 Logo | ✅ | 100% | `logo.png` 立方体标志 |
| Tauri 全平台图标 | ✅ | 100% | Windows/macOS/iOS/Android 图标包 |
| 版本号统一 | ✅ | 100% | Cargo.toml / package.json / tauri.conf.json → 5.6.2 |
| 旧图标清理 | ✅ | 100% | 移除 LOGO.jpg / icon.jpg / logo-source.png |

#### 💎 功能订阅制 (v0.7.3 重构完成)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| 数据库表 | ✅ | 100% | `subscriptions` / `ai_usage_logs`（v0.7.3 移除配额表，保留日志表） |
| `SubscriptionService` | ✅ | 100% | 订阅状态 / `has_feature_access` 功能开关 / 日志记录 |
| Tauri IPC 命令 | ✅ | 100% | `get_subscription_status` / `dev_upgrade_subscription` |
| `useSubscription` Hook | ✅ | 100% | 全局状态 + `localStorage` 离线缓存 |
| `SubscriptionStatus` 指示器 | ✅ | 100% | Header 显示当前订阅层级标识 |
| 功能权限中间件 | ✅ | 100% | `has_feature_access` 按 `feature_id` 拦截，Free/Pro/Enterprise 分级 |
| 转化漏斗 UI | ✅ | 100% | 免费提示浮层 / `UpgradePanel` / 订阅功能解锁引导 |
| Agent 质量分层 | ✅ | 100% | 免费版 token 限制 1000 + 简化 prompt |
| `AppError::SubscriptionRequired` | ✅ | 100% | `SUBSCRIPTION_REQUIRED` 错误码，前端按 code 渲染升级 UI |
| `log_ai_usage` 纯统计 | ✅ | 100% | 仅记录调用明细，不参与任何配额控制 |

#### 📖 拆书功能 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| 文件解析器 | ✅ | 100% | txt/pdf/epub，编码自动检测 |
| 文本分块器 | ✅ | 100% | 短篇全文/中篇章节/长篇采样(50块) |
| LLM 分析器 | ✅ | 100% | 5步 Pipeline，并发限制3 |
| 数据仓库 | ✅ | 100% | `ReferenceBook`/`Character`/`Scene` CRUD |
| 业务服务 | ✅ | 100% | 上传→分析→保存→一键转故事 |
| Tauri IPC 命令 | ✅ | 100% | 6 个命令: upload/get_status/get_analysis/list/delete/convert |
| 前端页面 | ✅ | 100% | 拆书主页面 + 6 个子组件 + Hooks |
| 进度推送 | ✅ | 100% | `book-analysis-progress` Tauri 事件 |
| 数据库迁移 | ✅ | 100% | 3 张表 + 4 个索引 + Migration 16 |
| 单元测试 | ✅ | 100% | 6 tests 全部通过 |

#### 🔧 TaskService 全局共享修复 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| TaskService 泛型化 | ✅ | 100% | `<R: Runtime>` 默认 `Wry`，兼容 mock 测试 |
| TaskService 手动 Clone | ✅ | 100% | 不依赖 `R: Clone`，`Arc<Mutex<ExecutorRegistry>>` 共享 |
| commands.rs State 获取 | ✅ | 100% | 所有 command 改为 `tauri::State<'_, TaskService>` |
| lib.rs app.manage | ✅ | 100% | setup 阶段注册 executor 后全局共享 |
| 集成测试 | ✅ | 100% | 5 tests 验证端到端流程 |

#### 🧪 测试基础设施 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| 前端 Vitest 环境 | ✅ | 100% | `vitest.config.ts` + `jsdom` + `@testing-library/react` |
| Rust 测试工具 | ✅ | 100% | `tempfile` dev-dep + `test_utils.rs` |
| 设置模块测试 | ✅ | 100% | Rust 16 tests + 前端 14 tests |
| 任务系统测试 | ✅ | 100% | Rust 18 tests（13 单元 + 5 集成） |
| 数据库仓库测试 | ✅ | 100% | Rust 14 tests |
| 工具函数测试 | ✅ | 100% | Rust 20 tests |
| 测试覆盖率 | ✅ | 100% | Rust 139 tests + 前端 21 tests 全部通过 |

### v3.3.0 新增功能（2026-04-15）

#### 🖱️ 幕前右键菜单修复与暖色重构 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| Tailwind utilities 补充 | ✅ | 100% | `frontstage.css` 添加 `@tailwind utilities;`，`fixed`/`z-[9999]` 等类正常生效 |
| 事件捕获修复 | ✅ | 100% | `contextmenu`/`mousedown` 改为捕获阶段监听，兼容 Tauri WebView |
| WebView2 默认菜单禁用 | ✅ | 100% | Rust 后端通过 `webview2-com` 禁用 Windows 系统右键菜单 |
| 暖色 UI 重构 | ✅ | 100% | 背景 ivory / 边框 warm-sand / 文字 charcoal / hover warm-sand |
| 菜单功能完整 | ✅ | 100% | 剪切/复制/粘贴、修订模式、批注、评论、古典评点、全选 |

### v3.2.0 进行中功能

#### 💾 向量存储持久化 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| SQLite 向量表 | ✅ | 100% | `vector_records` 表 + 索引 |
| `LanceVectorStore` SQLite 实现 | ✅ | 100% | 替换 JSON-memory fallback |
| API 向后兼容 | ✅ | 100% | `search_similar` / `embed_chapter` 零改动 |
| 持久化单元测试 | ✅ | 100% | 跨实例数据不丢失 |

#### 🕸️ 知识图谱可视化 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| get_story_graph API | ✅ | 100% | 一次性返回完整图数据 |
| ReactFlow 图谱渲染 | ✅ | 100% | 节点按类型着色，边按强度显示 |
| 实体详情面板 | ✅ | 100% | 点击显示属性与关联关系 |
|  backstage 页面集成 | ✅ | 100% | Sidebar 导航、路由、空状态 |
| 记忆健康面板 | ✅ | 100% | 保留报告、自动归档建议 |
| 图谱交互优化 | ✅ | 100% | 双击聚焦节点、搜索筛选、类型过滤 |

#### 📦 自动归档系统 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| `is_archived` / `archived_at` 字段 | ✅ | 100% | `kg_entities` 表扩展 + 自动迁移 |
| `archive_forgotten_entities` | ✅ | 100% | 一键归档所有遗忘状态实体 |
| `restore_archived_entity` | ✅ | 100% | 从归档状态恢复实体 |
| `get_archived_entities` | ✅ | 100% | 查询已归档实体列表 |
| 前端归档页签 | ✅ | 100% | 知识图谱页面「已归档」标签页 |
| 一键归档按钮 | ✅ | 100% | 记忆健康面板直接触发归档 |

#### 🛠️ 技能工坊 (Skills) (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| 前端类型对齐 | ✅ | 100% | `Skill` 接口扩展为完整 `SkillInfo` |
| 真实数据接入 | ✅ | 100% | `getSkills()` 替代 mock 数据 |
| 分类筛选 | ✅ | 100% | 9 个分类标签筛选 |
| 启用/禁用 | ✅ | 100% | 开关调用 `enable/disable_skill` |
| 执行技能 | ✅ | 100% | ▶️ 按钮运行，自动收集必填参数 |
| 卸载技能 | ✅ | 100% | 非内置技能显示卸载入口 |
| 导入技能 | ✅ | 100% | 文件选择器导入本地技能文件 |

#### 🪄 小说创建向导 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| `generate_world_building_options` | ✅ | 100% | AI 生成世界观选项 |
| `generate_character_profiles` | ✅ | 100% | AI 生成角色谱选项 |
| `generate_writing_styles` | ✅ | 100% | AI 生成文风选项 |
| `generate_first_scene` | ✅ | 100% | AI 生成首个场景 |
| `create_story_with_wizard` | ✅ | 100% | 一键保存 + 自动 Ingest |
| 前端向导连通 | ✅ | 100% | `NovelCreationWizard` 调用真实后端 |
| Dashboard 集成 | ✅ | 100% | 首页「AI 创建故事」入口 |
| 自动 Ingest | ✅ | 100% | 完成后自动摄取知识图谱 |
| 图谱交互优化 | ✅ | 100% | 双击聚焦、搜索筛选、类型过滤、实体就地编辑 |

#### 🤖 Agent 模型映射与路由 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| AppConfig agent_mappings | ✅ | 100% | JSON 持久化，默认映射已配置 |
| get/save_settings 集成 | ✅ | 100% | 设置读写完整支持 |
| get/update_agent_mapping | ✅ | 100% | 从硬编码改为真实配置操作 |
| LlmService generate_with_profile | ✅ | 100% | 按模型 ID 调用指定配置 |
| AgentService 模型路由 | ✅ | 100% | 5 种 Agent 均接入映射路由 |
| 前台设置 UI 绑定 | ✅ | 100% | 前端已有完整 UI，后端已连通 |

#### 🧠 意图引擎与 Agent 调度 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| IntentParser (Rust) | ✅ | 100% | 基于 LLM 的 JSON 意图解析，11 种意图类型 |
| IntentExecutor (Rust) | ✅ | 100% | Agent 映射、串行/并行执行 |
| parse_intent 命令 | ✅ | 100% | Tauri 命令已注册 |
| execute_intent 命令 | ✅ | 100% | Tauri 命令已注册 |
| useIntent Hook | ✅ | 100% | 前端意图解析与执行封装 |
| RichTextEditor 集成 | ✅ | 90% | 自动选择流式输出或 Agent 调度路径 |
| 完整 workflow 集成 | ✅ | 100% | 基础框架 + Agent 路由 + 取消机制已完成 |

### v3.3.0 新增功能（2026-04-15）

#### 🍃 品牌 Logo 全面应用 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| Tauri 全平台图标 | ✅ | 100% | `cargo tauri icon` 从 `LOGO.jpg` 生成 Windows/macOS/iOS/Android 图标 |
| 前端 favicon | ✅ | 100% | `favicon.ico` + `apple-touch-icon.png` + `icon-192/512.png` |
| 文档展示图 | ✅ | 100% | `docs/images/logo.png` 用于 README 等品牌展示 |
| 品牌描述更新 | ✅ | 100% | README / CHANGELOG / PROJECT_STATUS 同步更新 |

### v3.1.2 新增功能（2026-04-13）

#### 🎨 品牌视觉升级 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| 应用主图标 | ✅ | 100% | 临时使用 Lucide feather 羽毛笔作为过渡 |
| 前端 favicon | ✅ | 100% | 过渡阶段使用 feather.svg |
| 图标来源 | ✅ | 100% | iconbuddy.com / Lucide Icons (MIT) |

#### 🔧 幕后设置页增强 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| 编辑 API Key 输入框 | ✅ | 100% | custom 提供商编辑时正确显示 API Key 字段 |
| 模型连接状态灯 | ✅ | 100% | 卡片级实时探测，绿/红/加载三种状态 |
| 浏览器 dev fallback | ✅ | 100% | Vite 浏览器模式下硬编码本地模型回退 |

### v3.1.1 新增功能（2026-04-13）

#### 🎭 幕前界面重构 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| 精简侧边栏 | ✅ | 100% | 仅保留"幕后"按钮，120px 极简宽度 |
| OKLCH 颜色系统 | ✅ | 100% | 全站 OKLCH 色值，60-30-10 视觉权重 |
| LXGW WenKai 字体 | ✅ | 100% | 移除 Crimson/Inter，统一霞鹜文楷 |
| Blockquote 重设计 | ✅ | 100% | 背景色块 + 引号装饰，去左边框模板 |
| 微交互规范 | ✅ | 100% | 全按钮 `active:scale-95`，清除 `transition: all` |
| 顶部动态状态栏 | ✅ | 100% | 字数、字号、快捷键、保存状态 |
| 底部 LLM 对话栏 | ✅ | 100% | 悬停显示，集成模型状态灯，去除模式切换图标 |
| 流式对话 | ✅ | 100% | Enter 发送，Shift+Enter 换行 |

#### 🤖 本地模型配置 (100%)

| 模型 | 类型 | 状态 | 备注 |
|------|------|------|------|
| Gemma-4-31B-it-Q6_K | 多模态 | ✅ | 本地 vLLM 服务 |
| Qwen3.5-27B-Uncensored | 语言 | ✅ | 本地 vLLM 服务 |
| bge-m3 | Embedding | ✅ | 本地 Embedding 服务 |

#### 🖥️ Tauri 构建与 CI (100%)

| 目标 | 状态 | 说明 |
|------|------|------|
| Release 编译 | ✅ | Rust 后端编译通过（0 warnings） |
| MSI 安装包 | ✅ | `StoryForge_3.1.2_x64_en-US.msi` |
| NSIS 安装包 | ✅ | `StoryForge_3.1.2_x64-setup.exe` |
| `rust-check` (Ubuntu) | ✅ | GitHub Actions 通过 |
| `rust-check` (Windows) | ✅ | GitHub Actions 通过 |
| `rust-check` (macOS) | ✅ | GitHub Actions 通过 |
| `tauri-build` Windows | ✅ | GitHub Actions 通过 |
| `tauri-build` macOS | ✅ | GitHub Actions 通过 |
| `tauri-build` Ubuntu | ✅ | GitHub Actions 通过 |

### v3.1.0 核心功能

#### 📜 场景版本系统 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| SceneVersionRepository | ✅ | 100% | 版本CRUD、版本链管理 |
| SceneVersionService | ✅ | 100% | 比较、恢复、统计 |
| VersionTimeline 组件 | ✅ | 100% | 垂直时间线、版本选择 |
| DiffViewer 组件 | ✅ | 100% | 行级差异对比 |
| ConfidenceIndicator | ✅ | 100% | 圆形/条形置信度指示 |
| useSceneVersions hooks | ✅ | 100% | React Query封装 |
| Tauri 命令 | ✅ | 100% | 7个版本管理命令 |

#### 🔍 混合搜索系统 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| Bm25Search | ✅ | 100% | CJK二元组分词、TF-IDF |
| HybridSearch | ✅ | 100% | RRF融合排序 |
| EntityHybridSearch | ✅ | 100% | 名称+向量混合 |
| LanceVectorStore | ✅ | 100% | LanceDB兼容API |
| 实体嵌入 | ✅ | 100% | 384维嵌入生成 |

#### 🧠 记忆保留系统 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| RetentionManager | ✅ | 100% | 遗忘曲线计算 |
| 优先级分级 | ✅ | 100% | 五级优先级 |
| 遗忘预测 | ✅ | 100% | 遗忘时间预测 |
| 保留报告 | ✅ | 100% | 自动报告生成 |
| 上下文优化 | ✅ | 100% | 预算控制选择 |

### v3.0 核心功能

#### 🎪 场景化叙事系统 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| Scene 数据模型 | ✅ | 100% | 戏剧目标、外部压迫、冲突类型、角色冲突 |
| SceneRepository | ✅ | 100% | CRUD + reorder_scenes 拖拽排序 |
| StoryTimeline 组件 | ✅ | 100% | @dnd-kit 拖拽、场景卡片、冲突标签 |
| SceneEditor 组件 | ✅ | 100% | 三标签页（基础/戏剧/内容） |
| ConflictType 枚举 | ✅ | 100% | 6 种标准冲突类型 |
| 场景页面 | ✅ | 100% | Scenes.tsx 完整实现 |
| Tauri 命令 | ✅ | 100% | 12 个场景相关命令 |

#### 🧠 增强记忆系统 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| CJK Tokenizer | ✅ | 100% | Bigram 分词，中日韩支持 |
| Ingest Pipeline | ✅ | 100% | 两步思维链：分析→生成 |
| Knowledge Graph | ✅ | 90% | Entity/Relation 带强度评分 |
| Query Pipeline | ✅ | 100% | 四阶段检索管线 |
| Multi-Agent Sessions | ✅ | 100% | 6 种助手类型独立会话 |
| 数据库存储 | ✅ | 100% | kg_entities, kg_relations 表 |
| Tauri 命令 | ✅ | 100% | 8 个记忆系统命令 |

#### 🤖 AI 智能生成 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| NovelCreationAgent | ✅ | 100% | 世界观/角色/文风/场景生成 |
| NovelCreationWizard | ✅ | 100% | 4 步引导式创建 |
| 卡片式 UI | ✅ | 100% | 单击选择、双击编辑 |
| 首个场景生成 | ✅ | 100% | 创建完成后自动生成 |
| Tauri 命令 | ✅ | 100% | 4 个创建相关命令 |

#### 📦 工作室配置系统 (100%)

| 功能模块 | 状态 | 完成度 | 备注 |
|---------|------|--------|------|
| StudioConfig 模型 | ✅ | 100% | 每部小说独立配置 |
| StudioManager | ✅ | 100% | ZIP 导入/导出、冲突处理 |
| 默认主题 | ✅ | 100% | 幕前暖色/幕后暗色 |
| Tauri 命令 | ✅ | 100% | 2 个配置管理命令 |

---

### 架构基础 (100%)

- ✅ Tauri + Rust 桌面应用框架
- ✅ 幕前幕后双窗口架构
- ✅ 窗口间通信机制 (Events)
- ✅ SQLite 数据库 (r2d2 连接池)
- ✅ 前端 React 18 + TypeScript 5.8 + Vite 6
- ✅ @dnd-kit 拖拽排序

---

## 📊 v3.1.1 新增文件清单

### 前端 (src-frontend/src/)

- `config/models.ts` - 本地三模型配置
- `hooks/useModel.ts` - 模型状态管理与对话 Hook
- `services/modelService.ts` - 模型 HTTP API 服务层

### 截图 (e2e/screenshots/)

- 幕前界面各状态截图（侧边栏、对话栏、模型状态等）

---

## 📈 整体完成度

### v3.1 模块完成度

| 模块 | 完成度 | 权重 | 加权得分 |
|------|--------|------|----------|
| 场景化叙事系统 | 100% | 15% | 15.0 |
| 增强记忆系统 | 100% | 15% | 15.0 |
| AI 智能生成 | 100% | 10% | 10.0 |
| 工作室配置 | 100% | 5% | 5.0 |
| 幕前界面 | 100% | 10% | 10.0 |
| 本地模型集成 | 100% | 5% | 5.0 |
| 后端架构 | 100% | 5% | 5.0 |
| 桌面构建打包 | 100% | 5% | 5.0 |
| 创作方法论引擎 | 100% | 10% | 10.0 |
| StyleDNA 系统 | 100% | 10% | 10.0 |
| 自适应学习系统 | 100% | 5% | 5.0 |
| 创作工作流引擎 | 100% | 5% | 5.0 |
| **v3.4.0 基础总计** | - | 100% | **100.0%** |

### v0.7.0 新增模块完成度

| 模块 | 状态 | 完成度 |
|------|------|--------|
| AI 三审 Pipeline | ✅ 完成 | 100% |
| 角色动态状态 | ✅ 完成 | 100% |
| 用量统计看板 | ✅ 完成 | 100% |
| 幕前 Pipeline 指令 | ✅ 完成 | 100% |

### v0.7.2 新增模块完成度

| 模块 | 状态 | 完成度 |
|------|------|--------|
| 拆书存储同构化 | ✅ 完成 | 100% |
| MCP 动态注册 | ✅ 完成 | 100% |
| 1:N 聚合编辑 Schema | ✅ 完成 | 100% |
| SceneDividerNode | ✅ 完成 | 100% |
| LLM 取消机制 | ✅ 完成 | 100% |
| AppError 结构化 IPC | ✅ 完成 | 100% |

### v0.7.3 新增模块完成度

| 模块 | 状态 | 完成度 |
|------|------|--------|
| 商业模式重构（订阅制） | ✅ 完成 | 100% |
| 1:N Chapter↔Scene 架构完成 | ✅ 完成 | 100% |
| SceneDivider 预留接口 | ✅ 完成 | 100% |
| scene_commits 重命名 | ✅ 完成 | 100% |

---

## 🎯 待完善功能

### v7.1.x 短期计划

#### P1 - 重要功能
1. **云端同步**
   - 位置: 全局
   - 状态: ⏳ 待开始
   - 说明: 多设备间故事数据同步

2. **协作写作增强**
   - 位置: `src-tauri/src/collab/`
   - 状态: ⏳ 待开始
   - 说明: 多人实时编辑 OT 完整实现

3. **插件市场**
   - 位置: 全局
   - 状态: ⏳ 待开始
   - 说明: 技能/模板/主题的在线分享与安装

#### P2 - 增强功能
4. **WebAssembly 前端**
   - 状态: ⏳ 待开始
   - 说明: 核心逻辑 Rust → WASM，提升前端性能

5. **自研小模型**
   - 状态: ⏳ 待开始
   - 说明: 针对小说创作场景微调的小型 LLM

6. **移动端适配**
   - 状态: ⏳ 待开始
   - 说明: 平板/手机端幕前界面适配

### 已完成（历史归档）
- ✅ 前端 UI 接入新方法引擎（v3.5.x）
- ✅ 导出模板自定义（v6.0.0 ExportDialog 已支持模板选择）
- ✅ Ingest 管线性能优化（join_all 并发）
- ✅ 查询缓存机制（LRU 100 条）
- ✅ 实体嵌入持久化（BLOB 修复）
- ✅ 更多冲突类型（11 种）
- ✅ Agent 上下文增强（真实数据库接入）
- ✅ 记忆压缩（MemoryCompressorAgent）
- ✅ 语义搜索优化（FTS5 + RRF 融合）
- ✅ 修订模式与变更追踪
- ✅ 评论批注系统（v4.1.0 从幕前移除，场景级批注保留）
- ✅ AI 三审 Pipeline（v0.7.0）
- ✅ 角色动态状态自动更新（v0.7.0）
- ✅ 用量统计看板（v0.7.0）
- ✅ 幕前 `/` 指令打通 Pipeline（v0.7.0）
- ✅ 拆书分析存储同构化（v0.7.2）
- ✅ MCP 工具动态注册（v0.7.2）
- ✅ 1:N Chapter↔Scene 聚合编辑 Schema（v0.7.2）
- ✅ TipTap SceneDividerNode（v0.7.2）
- ✅ LLM request_id 级取消机制（v0.7.2）
- ✅ AppError 结构化 IPC（v0.7.2）
- ✅ 商业模式重构为功能订阅制（v0.7.3）
- ✅ 1:N Chapter↔Scene 架构完成（v0.7.3）
- ✅ SceneDivider 预留接口（v0.7.3）

---

## 🐛 已知问题

### v3.3.0 已知问题
1. **编译警告**
   - 描述: 已清零（0 warnings）
   - 影响: 无功能影响
   - 解决: 2026-04-14 通过 `#[allow(dead_code)]` 等标记完成清理，未删除任何代码

### v3.1 已知问题（已解决）

1. ✅ **Windows 下 Tauri beforeBuildCommand 路径问题** - v3.1.1 已修复
2. ✅ **Tauri 文件锁阻塞** - v3.1.1 已解决并构建成功
3. ✅ **GitHub Actions macOS/Ubuntu 缺少 `icon.icns`** - v3.1.1 已修复并推送

---

## 📚 相关文档

- [README.md](../README.md) - 项目简介
- [ARCHITECTURE.md](../ARCHITECTURE.md) - 架构文档
- [ROADMAP.md](../ROADMAP.md) - 开发路线图
- [CHANGELOG.md](../CHANGELOG.md) - 更新日志
- [docs/plans/ARCHITECTURE_V3_PLAN.md](plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计
