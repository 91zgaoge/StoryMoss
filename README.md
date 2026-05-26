<p align="center">
  <img src="docs/images/logo.png" alt="StoryForge 草苔" width="120" />
</p>

# StoryForge (草苔) v0.7.8 - AI 导演式小说创作系统

> 🌿 越写越懂的 AI 小说创作系统 — Tauri + Rust + React 驱动的桌面写作软件
>
> 专为小说作者打造的**导演式创作工作台**：知识图谱可视化、伏笔追踪与回收、StyleDNA 风格引擎、AI 三审Pipeline、角色动态状态追踪、7 阶段全自动创作工作流、高密度状态世界构建法。让 AI 成为你的创作搭档，越写越懂你。
>
> **v0.7.8 最新更新（2026-05-26）**：续写功能风格指纹加固 + 自动更新系统增强 + CI artifact 上传。**续写功能风格指纹加固** — 从"软性风格提示"跃迁到"量化指纹约束"：风格指纹引擎从任意参考文本提取句长分布、四字格密度、虚词频率、标志性词汇、锚点片段等量化特征；Writer prompt 自动注入风格约束；Inspector 新增第 6 维「风格一致性评分」（句长/词汇/虚词/四字格/语感五子维度）；Orchestrator 双轨平衡（风格分 + 叙事分分别评分，低于阈值自动调节权重）；3 候选并行生成选优（temperature 0.75/0.90/1.05 产生多样性，指纹打分选最优）；跨段一致性 4 维度漂移检测（句长/四字格/虚词/标志性词汇）；后处理替换层（虚词对齐 + 四字格密度补偿）；前端 WenSiPanel 增加参考文本输入 + 风格-叙事平衡滑块 + 实时风格分数显示。**自动更新系统增强** — 检测间隔 24h → 4h；后台静默下载 + 进度可视化；更新日志结构化分类（新增/修复/注意）；CI 配置增量更新（delta patch）。
>
> **v0.7.7 更新（2026-05-25）**：统一创作流水线 + 级联改写 + 架构归一化 + 后端预检自动补齐 + 统一后台活动提示系统。**统一创作流水线** — 快速创作与向导模式合并为同一后端 8 阶段 Pipeline，Scene 为唯一提交粒度；快速创作后后台自动 enrich 生成完整世界观/角色/风格，并自动修正正文与要素的不一致；向导创作全流程 SQLite 事务保护。**级联改写系统** — 角色/世界观/关系变更后自动检测影响范围，Diff 预览 + 一键改写受影响场景。**架构归一化** — db 模型层（models_v3）、仓库层（repositories_v3 + Trait 层）、命令注册（handlers.rs）全面重构，消除 2 套模型 + 6 套仓库的碎片化问题。**Task System 全局化** — AI 生成与 Pipeline 审校接入任务中心，支持进度追踪和一键重试。**后端预检自动补齐** — `execute_writer_raw` 预检发现缺少世界观合同/章节合同/场景大纲时，不再直接报错，而是自动调用 LLM 补齐后重新预检；所有后端写作入口（`smart_execute`/`auto_write`/`auto_revise`/`generate_scene_draft`/Workflow）全覆盖。**统一后台活动提示系统** — 新增 `backendActivityStore` (Zustand) 聚合所有后台活动状态，`useBackendActivityListener` Hook 统一监听 6 类进度事件；底部状态栏新增心跳脉冲动画 + 进度条 + 多任务计数，大阶段变化时自动 toast 提示，让用户持续感应后台智能平台的"心跳般"互动陪写。
>
> **v0.7.5 更新（2026-05-22）**：事件驱动创作增强中枢 — 6 个先前"有设计但未集成"的高级子系统全面落地。追读力自动评估（update_scene / finalize_draft 后自动触发）、Writer Agent 预检（生成前检查合同/角色/大纲，缺失时精准报错）、语义检索自动注入（Writer Agent 上下文自动执行知识库搜索）、合同/提交链前端可操作化（StorySystem 一键生成世界观合同/章节合同/初始化提交）、叙事审计 story-level（全作品 5 维度结构健康检查）、风格进化接入审校反馈（Anti-AI Review 结果一键进化 StyleDNA）。**新增：合同自动补齐** — 预检发现缺少世界观合同或章节合同时，系统自动根据已有故事内容调用 LLM 生成并保存，全过程实时进度提示，无需用户手动创建。协同编辑/Chat/StoryStateManager 标记为预留功能。
>
> **v0.7.4 更新（2026-05-22）**：幕后世界构建页面（核心概念 / 世界规则 / 历史背景 / 文化体系一站式编辑）+ 世界-场景自动关联（场景 setting 变更自动同步到世界观，遵循"场景增世界增，场景减世界减"双向关联原则）+ 内置经典作家 DNA 扩展至 52 种（新增中国/日本/欧美文学 32 位作家 + 8 种类型文学风格）。
>
> **v0.7.0 重大更新（2026-05-15）**：AI 三审 Pipeline 系统（修稿 → 审稿 → 定稿 → 后处理）+ 角色动态状态面板（LLM 驱动实时更新）+ 用量统计看板 + 幕前 `/` 指令打通 Pipeline + Stories 场景进度看板 — 从"能创作"到"高质量创作"的完整质量管控闭环。
>
> **架构优化（2026-05-17）**：`ChapterCommitService` 防抖聚合提交（30秒空闲延迟）取代旧版独立 `auto_ingest_chapter` 管线，消除重复索引；导出聚合：空章节内容自动按场景序号聚合填充，确保 Markdown/HTML/PlainText 导出完整；`Settings.tsx` / `SceneEditor.tsx` 大型组件提取重构为原子化子组件；`StoryTimeline.tsx` 新增 `execution_stage` 彩色徽章（plan/outline/draft/review/final），场景进度一目了然。
>
> **v0.7.3 架构对齐（2026-05-20）**：商业模式从模型配额制重构为功能订阅制（`SubscriptionService` 移除所有配额计量逻辑，改为 `has_feature_access` 细粒度功能开关；`AppError::SubscriptionRequired` 取代 `QuotaExceeded`）；1:N Chapter↔Scene 架构完成（废弃 `chapters.scene_id`，全链路改为 `scenes.chapter_id` 查询；`SceneCommitService` 取代 `ChapterCommitService`，`scene_commits` 取代 `chapter_commits`，提交粒度彻底对齐 Scene）；`SceneDividerNode` 功能预留接口就绪（`SceneDividerRepository` + `scene_divider_nodes` 表 + Migration 72）；数据库 Schema 清理（Migration 70 重命名 `chapter_commits` → `scene_commits`，Migration 71 移除 `chapters.scene_id`）。
>
> **v0.7.2 功能增强（2026-05-19）**：拆书分析存储同构化（`reference_characters`/`reference_scenes` 数据统一汇聚到 `narrative_*` 表，Migration 69 自动迁移历史数据）；MCP 工具动态注册（`CapabilityRegistry` 实时同步外部 MCP 服务器工具，前端通过 `mcp.{server_id}.*` 前缀区分内置与外部工具）；1:N 聚合编辑数据库 schema（`chapter_commits` 新增 `chapter_id` 外键，支持多场景聚合到单一章节）；TipTap 场景分隔节点（`SceneDividerNode` 原子块节点，可视化区分相邻场景内容）；LLM 调用取消机制（`request_id` 级取消信号，`cancel_generation` 精确中断单条请求）；`AppError` 结构化 IPC（统一 `{ code, message, data }` 错误格式，前端按 code 做精准错误处理）。
>
> **AI 三审 Pipeline 系统**：引入 `Rewrite → Refine → Review → Finalize` 四级创作管线，每章正文经过 AI 修稿（语言润色）、AI 审稿（多维评分与问题标注）、定稿（通过后处理步骤自动更新知识库、章节笔记、角色动态状态卡、风格分析），实现"每章必审、每稿必改、定稿即入库"的工业化创作流程。
>
> **角色动态状态面板**：每个角色新增 6 项动态状态字段（当前位置、实力等级、身体状态、心理状态、关键物品、近期事件），定稿时由 LLM 自动解析章节内容并更新角色状态，前台 `Characters.tsx` 集成可折叠状态面板，支持实时查看与手动修正。
>
> **用量统计看板**：全局 LLM 调用统计（总次数/总 token/平均响应时间/成功率）+ 单故事维度统计 + 最近 20 条调用记录明细，帮助作者了解 AI 辅助成本与效率。
>
> **Story System 合同驱动体系**：引入 `MASTER_SETTING` / `CHAPTER` / `REVIEW` 四级合同架构，写前真源（story_contracts）与写后真源（scene_commits）分离，SCENE_COMMIT 提交链驱动 5 个 Projection Writer（State/Index/Summary/Memory/Vector）自动更新 read-model，实现"合同即法律、设定即物理、发明需识别"的防幻觉三定律。
>
> **三层记忆编排器**：Working Memory（最近 5 章 + 活跃角色 + 开放伏笔）+ Episodic Memory（state_changes + relationships）+ Semantic Memory（长期事实，按优先级和源章节窗口过滤），支持按任务类型（write/plan/review）动态分配预算。
>
> **追读力评估系统**：Hook 检测（悬念/冲突/转折）+ Coolpoint 追踪（打脸/收获/揭秘）+ Micropayoff 微兑现 + Debt 债务追踪（含利息与覆盖合同），每章输出 0-100 追读力评分与趋势图。
>
> **43 体裁模板库**：玄幻/仙侠/都市/历史/科幻/悬疑/言情/武侠/无限流/系统流/重生/穿越/凡人流/争霸流/幕后流/签到流/御兽流/诡异流/赛博朋克/克苏鲁/国运流/末日拓荒/灵气复苏/规则怪谈/模拟器流/盗墓流/星际机甲等 43 个内置网文体裁模板，每个含核心基调、节奏策略、反模式清单、参考数据表、典型结构五要素。
>
> **Anti-AI 五维审查**：词汇（cliché 检测 + 重复用词）/ 语法（句式多样性 + 被动语态）/ 叙事（段落均匀度 + 感官密度）/ 情感（标签化检测 + 展示vs告知）/ 对话（说明性对话 + 标签单调），输出综合评分与改进建议。
>
> **v5.6.4 更新**：JSON 解析加固 + 场景生成去重 + 前端自动排版 + CI 三平台修复 — 修复 **4 项生产环境关键问题**。
>
> **JSON 解析全面加固**：`extract_and_sanitize_json` 新增字符串内未转义换行符修复（状态机精确替换）、C 风格注释移除、移除破坏性中文引号替换（原代码破坏 JSON 字符串边界），LLM 返回的"脏 JSON"容错能力大幅提升。
>
> **场景生成去重与幂等性**：`SceneGenerationStep` 新增 `seen_seqs` 去重，跳过 LLM 返回的重复 `sequence_number`；已存在场景直接更新而非重复创建，Bootstrap 重试安全。
>
> **前端自动排版引擎**：新增 `autoFormatText` 智能分段与引号规范化（直引号 →「」『』），集成到所有内容更新路径，LLM 未格式化输出自动转换为标准 HTML 段落；AI 续写新增重复前缀去除保护，避免生成内容与当前文本重复。
>
> **CI 三平台修复**：移除 `.cargo/config.toml` UTF-8 BOM（修复 Windows/Ubuntu 构建失败）；macOS 目标从 `x86_64` 改为 `aarch64-apple-darwin`（适配 GitHub Apple Silicon runner，修复 LanceDB AVX512 链接错误）。
>
> **维度一：幕前幕后自动关联补全（4项）** — `SceneCommitService::auto_commit` 防抖聚合提交（30秒空闲延迟）自动驱动知识图谱同步与向量索引更新，取代旧版 `auto_ingest_chapter` 独立摄取管线，消除重复索引工作；`update_scene` 自动触发 `dataRefresh(knowledgeGraph)`，Ingest 完成后 KG 可视化自动刷新；`commands_v3.rs` KG CRUD 命令统一发射 `dataRefresh(knowledgeGraph)`，消除直接更新路径绕过 StateSync 的问题；前端 `useSyncStore` 补全 `characterRelationshipsUpdated` / `payoffLedgerUpdated` / `ingestionCompleted` case，特定事件独立响应。
>
> **维度二：后台自动化闭环补全（5项）** — Automation Service 集成到全部核心 CRUD 命令（`create_story` 触发 `StoryCreated`、`create_character` 触发 `CharacterCreated`、`update_chapter`/`update_scene` 触发 `ChapterContentUpdated`），后台自动化对所有核心数据变更事件响应；`PlanTemplateLibrary` 从纯内存存储升级为 SQLite 持久化（Migration 46 `plan_templates` 表），重启后学习成果保持，避免重复 LLM 调用；能力进化引擎新增周期触发机制（每记录 5 条执行自动检查阈值并触发进化），不再仅依赖启动时一次性执行；`WorkflowEngine` 恢复实例后自动重新入队 `WorkflowScheduler`，应用重启后中断的工作流实例自动恢复调度；补齐缺失的 `story_metadata` / `scene_characters` / `scene_character_actions` 表定义（Migration 43-45），修复 automation service 运行时表缺失错误。
>
> **维度三：系统整洁度与数据一致性（4项）** — `delete_story` / `delete_character` 加固显式级联清理（事务内清理 `story_metadata`/`foreshadowing_tracker`/`user_preferences`/`ai_operations`/`scene_characters`/`scene_character_actions`/`character_relationships`/`character_states` 等 14+ 关联表），消除外键约束未覆盖的幽灵数据；Settings.tsx 隐藏未实现的"图像生成" Tab，消除死胡同功能；`tauri.conf.json` 窗口配置验证与 `frontstage`/`backstage` label 对齐。
>
> **回归测试**：`cargo check` 零错误，`cargo test` ~225/225 通过，`npm run build` 通过。
>
> **v5.6.3 更新**：IPC 参数一致性全面修复 + Bootstrap 序列化修复 — 幕后界面功能不可用的根本原因修复。**Bootstrap 进度卡死** — LLM 返回 JSON 省略 `age`/`sequence_number` 等字段导致 serde 反序列化失败，Pipeline 中断在前端显示永久 "塑造角色 (3/6)"。修复：给 `CharacterElement`/`SceneElement` 所有可能被 LLM 省略的字段添加 `#[serde(default)]`；`BootstrapProgressEvent` 新增 `status` 字段（`InProgress`/`Completed`/`Failed`），前端失败状态可见。**IPC 参数名全面审计** — 系统审计 `tauri.ts` 全部 40+ 命令与后端签名，修复 7 处 camelCase↔snake_case 不匹配（`smart_execute`/`get_input_hint`/`record_feedback`/`call_mcp_tool`/`check_auto_write_quota`/`check_auto_revise_quota`/`save_settings`）。**后端命令参数补全** — `run_creation_workflow` mode 映射增加 `"human_draft_ai_polish"`，`update_story` 补充 `genre`，`create_character`/`update_character` 补充 `personality`/`goals`/`appearance`/`gender`/`age` 扩展字段。幕后界面全部功能现已恢复正常。
>
> **v5.6.2 更新**：设计-实现对齐全面修复 v5 — 全面检视并修复 5 项设计-实现差距。**前端缓存同步精确化**：`writingStyle` case 同时刷新 `writing_style` 缓存（修复只刷新 `world_building` 的遗漏），`chapterUpdated` 补充 `['chapters', storyId]` 精确刷新当前故事 chapters 列表。**update_scene 向量索引闭环**：Scene 内容更新后同步写入 LanceDB 向量存储，`embed_text_async` → `VectorRecord` → `add_record` 完整链路，语义搜索可检索最新场景内容。**storySelected 关联数据自动刷新**：切换故事时自动刷新 8 项关联数据缓存，消除时序依赖。**dataRefresh 完整覆盖**：补充 `knowledgeGraph`/`characterRelationships` 单独 case。**编译优化**：5 处 dead_code 警告清理，warnings 113→109。
>
> **v5.6.1 更新**：设计-实现对齐全面修复 v4 — 全面检视并修复 8 项设计-实现差距。**幕前幕后自动关联补全**：`sceneCreated`/`sceneDeleted` 同步刷新 `chapters` 缓存，消除场景-章节关联状态滞后。**自适应学习真实反馈**：`record_feedback` 返回真实 `LearningPoint[]`，同步调用 `PreferenceMiner::mine` 获取用户偏好，前端使用返回结果替代硬编码 mock。**前端缓存同步完整覆盖**：`useSyncStore` 新增 `writingStyle`/`storyOutlines`/`foreshadowings` case，所有数据类型修改后前端缓存自动刷新。**后台自动化加固**：Pending vector 从 JSON 文件迁移到 SQLite 持久化（Migration 42），`save_pending_vector_indexes`/`load_pending_vector_indexes` 改为 SQLite 操作，保留 JSON fallback 用于迁移。**Workflow 健壮性**：`schedule_execution` 增加幂等检查（queue + running_instances 去重），防止同一实例重复入队。**文档一致性**：WorkflowScheduler 文档更新为"拓扑有序执行（同层可并行）"。
>
> **v5.6.0 更新**：设计-实现对齐全面修复 v3 — 全面检视并修复 20 项设计-实现差距。数据一致性修复、缓存同步对称性、Workflow 健壮性、前端体验、P2 优化。
>
> **v5.5.1 更新**：设计-实现对齐全面修复 v2 — 全面检视并修复 23 项设计-实现差距。**幕前幕后自动关联补全**：修复 5 处 `unwrap_or_default()` 导致空 story_id 的同步失效问题（update/delete 角色/章节/场景后前端缓存不刷新）、新增 `delete_world_building` 命令、DataRefresh 新增 worldBuilding 分支、scene-chapter 关联缓存正确失效、backstage-shown 事件精准定位故事。**后台自动化闭环**：Bootstrap 后台失败真实上报（`pipeline-complete` 不再硬编码 success）、向量存储启动竞态修复（积压队列自动处理）、Workflow Condition 节点支持变量比较、Workflow 失败自动重试、能力进化路径统一、Task Cron 解析精确化（引入 cron crate）、Genesis Pipeline 运行中取消支持。**技术债务清理**：4 个核心文档版本号同步、5 个过时 v3.x 文档归档、`tauri.ts` 死代码清理、`FrontstageToolbar` 废弃组件移除。
>
> **v5.5.0 更新**：设计-实现对齐全面修复 — 全面检视并修复 10 项设计-实现差距。**幕前幕后自动关联补全**：世界观修改后前端缓存自动刷新（`WorldBuildingUpdated` 事件修复）、删除章节后 Scene 外键正确清理、角色删除精准缓存失效。**后台自动化闭环**：保存章节后自动索引到 LanceDB 向量存储（语义搜索可检索最新写作内容）、Workflow 实例数据库持久化（重启后恢复）、能力进化反馈环闭合（执行记录自动分析并优化能力描述）。**技术债务清理**：移除 54 文件幽灵 crate、同步所有文档版本号。
>
> **v5.4.1 更新**：Bootstrap 编辑器内容丢失修复 — 修复创世流程中"小说已创建但编辑器无文字"的竞态条件问题。`FrontstageEvent::ChapterSwitch` 新增 `content` 字段，后端生成第一章后直接通过事件传递正文内容到前端；前端优先使用事件中的内容，绕过 DB 查询竞态；增加 `final_content` 兜底机制和 chaptersRef 为空时自动重查数据库；`loadStories` 在生成期间禁止自动 `selectStory`，避免拿到空 chapters 导致编辑器被清空。
>
> **v5.4.0 更新**：向量检索语义化 — 从关键词匹配升级到语义理解。新增 `OllamaEmbeddingProvider` 支持 `nomic-embed-text` / `all-minilm` 等真实语义嵌入模型；`QueryPipeline` 四阶段检索扩展为五阶段融合架构（CJK 分词搜索 + 语义向量搜索 + 加权融合 + 知识图谱扩展 + 预算控制）；LanceDB 真实 IVF-PQ 向量索引替代 SQLite 全表扫描，Cosine 距离精准召回。若用户未配置 Ollama/OpenAI embedding，自动 graceful fallback 到 FNV-1a 哈希，零额外配置即可运行。
>
> **v5.3.1 更新**：Bootstrap 体验全面修复 — 修复重复显示小说开头（幽灵文本叠加）、幕后结构要素不显示（queryKey 不匹配）、LLM JSON 解析失败（`missing field id`）、Bootstrap 生成中断（数据库查询容错 + JSON 字段容错）、续写时重复生成开头（预览从尾部截断 6000 字符）。创世引擎现在可以完整生成第一章正文和全部幕后结构要素。
>
> **v5.3.0 更新**：叙事元素模型重构 — 将 Bootstrap（生成小说）和拆书（分析小说）统一为可逆的 NarrativePipeline 架构。正向 GenesisPipeline 与逆向 AnalysisPipeline 操作同一套 `NarrativeElement` 抽象，统一存储层、统一进度系统、统一数据模型。新增 StoryHealthAnalyzer 故事结构健康检查（6维度评分）。
>
> **v5.2.1 更新**：超时修复与白屏修复 — Bootstrap 创建新小说超时延长至 600 秒；进度事件密度增强；LLM 心跳加速至 2 秒；后台窗口白屏修复增强。
>
> **v5.2.0 更新**：设计-实现对齐全面完成 — 通用 Workflow 引擎节点执行器、能力进化反馈环闭合、幕前↔场景双向同步、QueryPipeline 降级感知。
>
> **v5.1.0 更新**：幕前幕后自动关联对齐 — 从"各自为战"到"自动联动"。Chapter↔Scene 双向映射、统一实时状态中心（所有数据修改自动同步）、Bootstrap 完成后幕前自动加载并切换第一章、Ctrl+Shift+B 一键回幕后并定位当前故事、AgentOrchestrator 闭环接入（Writer→Inspector→StyleChecker→Writer 自动质检改写）、自适应学习闭环激活、Zustand↔TanStack Query 状态同步、窗口通信事件标准化。
>
> **v5.0.0 更新**：创世引擎 — 一键创世，万物关联。输入一句话，系统自动在幕后所有对应栏目中创建完整关联卡片。新增故事大纲自动生成、角色完整性格小传入库、角色关系图谱、伏笔自动埋设、知识图谱自动构建、前后台智能联动导航。
>
> **v4.3.0 最新更新**：智能交互创作流程深度优化 — 从"能创作"到"懂创作"。输入"写一稿都市玄幻小说"等创建意图后，**系统直接生成小说正文开头并以 ghost text 展示**，同时在后台自动完成 5 步初始化（故事概念→世界观→角色→场景→第一章），无需等待即可看到正文；PlanGenerator全面模型化，彻底移除关键词匹配，LLM自由理解意图，新增规则确保"写小说"类意图**强制调用 Writer 生成正文**而非返回大纲分析；新增设定修改智能响应（角色/世界观/场景自动更新并标记重写）；CapabilityRegistry扩展MCP工具和查询能力；PlanContext注入世界观、角色、伏笔、风格DNA等完整上下文，续写时真正"记得"故事的一切。
>
> **v4.2.0** (2026-04-23): 智能交互设计重构 V2 — 模型驱动的编排范式。从程序式编排转向模型式编排：LLM自主理解用户意图、生成执行计划、系统只负责执行。移除所有关键词匹配和if/else分支，真正实现"越写越懂"。新增能力自描述系统、计划生成器、提示词进化器、记忆显性化组件。
>
> **v4.1.0** (2026-04-22): 幕前界面深度重构 — 化整为零，萤火随行。从 20+ 可见 UI 元素缩减至 <5 持久元素，AI 功能以萤火暗示形式按需浮现。44px 极简顶栏、底栏删除、内联 `/` 命令菜单、幽灵文本、三态文思模式。

## 🎭 独具特色的双界面设计

StoryForge 独创**"幕前 - 幕后"**双界面架构，让创作与阅读完美融合：

### 🎬 幕前 (Frontstage) - 沉浸式阅读写作

**设计理念**：像阅读一本精美小说一样写作

- **OKLCH 暖色纸张** - 感知均匀的色彩系统，`oklch(96.5% 0.008 95)` 暖色调背景，护眼舒适
- **霞鹜文楷正文字体** - 采用 LXGW WenKai 作为正文字体，中文排版优雅，去除通用字体的"AI 感"
- **大字号阅读体验** - 18px 正文字号，1.8 倍行距，久写不累
- **44px 极简顶栏** - 小说标题（点击进入幕后）、章节信息、字数/总字数/字号、🔥 文思三态切换、禅模式按钮，无汉堡菜单无订阅徽章
- **F1 帮助面板** - 按 `F1` 弹出浮动快捷键指南面板，分类展示写作/模式/操作提示，动画入场，禅模式自动隐藏
- **AI 萤火随行** - `Ctrl+Space` 循环三态文思模式：`off·`（关闭）/ `passive✨`（被动暗示）/ `active🔥`（主动续写）
- **幽灵文本续写** - `Ctrl+Enter` 触发 AI 续写，结果以灰色斜体幽灵文本呈现于编辑器末尾，`Tab` 接受 / `Esc` 拒绝。生成等待期间显示三点起伏动画指示器
- **智能状态调整** - AI 自动判断用户意图：无章节时自动创建第一章、文思模式非 active 时自动切换、无需用户手工配置
- **一键创建小说并直出正文** - 输入"写一稿都市玄幻小说"等意图，系统启动**创世引擎**完整流程：故事概念 → 第一章正文 → 世界观构建 → 故事大纲自动生成 → 角色创建（含完整性格/目标/外貌/年龄/性别） → 场景设计 → 伏笔自动埋设 → 知识图谱自动构建。后台自动在幕后所有对应栏目中创建完整关联卡片，同时直接将生成的第一章正文以 ghost text 展示在编辑器中，Tab 接受即可开始写作
- **内联 `/` 指令输入框** - 编辑器内输入 `/` 触发浮动输入框，可直接输入任意指令（如"续写"、"润色"、"写一篇武侠小说吧"）。回车发送，Esc 取消，再按 `/` 输出字符。用户输入经过**意图引擎**解析，模型自主决定调用续写、润色、技能、MCP 搜索或结构调整。
- **右边缘萤火暗示** - 创作建议从编辑区右边缘淡入（0.8s）→ 停留 → 淡出（1.2s），不打扰写作流
- **空态诗意引导** - 编辑器无内容时居中显示"开始写下第一句话，文思将随你而行"
- **精简侧边栏 Dock** - 3 按钮：修（修订模式）/ 批（生成古典评点）/ 幕（进入幕后）
- **修订模式与变更追踪** - 32px 单行横幅，变更列表可滚动折叠，支持逐条接受/拒绝
- **古典评点生成** - AI 模拟金圣叹风格生成朱红色段落评点（`LXGW WenKai` 字体、左边框、`※` 前缀），内联插入于段落之间
- **禅模式** - `F11` 快捷键进入绝对纯净全屏，隐藏顶栏/侧边栏/所有萤火提示
- **右键上下文菜单** - 修订模式、生成古典评点、全选、复制、剪切、粘贴
- **角色悬浮卡片** - 幕前编辑器中鼠标悬停角色名 600ms，自动显示微型浮卡（状态标签、外貌摘要、上次出场章节），点击直达幕后角色详情
- **Ingest 健康指示器** - 顶栏右侧显示 🧠 图标：最近 Ingest 成功/失败状态一目了然，点击展开最近 3 条 Ingest 记录
- **后台设置同步** - 写作风格、字体设置在后台统一管理

![幕前界面预览](docs/images/frontstage-preview.png)

### 🔧 幕后 (Backstage) - 全能创作工作室

**设计理念**：专业作家的数字工作台

- **故事管理** - 多故事、多场景结构化组织，故事概览面板聚合大纲/角色/场景/伏笔摘要，场景级 Pipeline 进度看板
- **角色管理** - 角色卡片含完整性格/目标/外貌/年龄/性别，关系图谱可视化，性格追踪，动态状态面板（位置/实力/身心状态/物品/事件）
- **场景化叙事** - 以场景为单位的戏剧冲突驱动
- **场景编辑器** - 三标签页设计（基础信息 / 戏剧结构 / 内容编辑）
- **AI 三审 Pipeline** (v0.7.0) - 场景级修稿(Refine)/审稿(Review)/定稿(Finalize)工作流，自动后处理（知识库更新、角色状态同步、风格分析）
- **伏笔看板** - 伏笔全生命周期管理：setup / payoff / abandoned 状态追踪，逾期检测与回收推荐
- **知识图谱可视化** - 基于 ReactFlow 的交互式力导向图谱，支持搜索、筛选、实体编辑，bootstrap 期间自动构建
- **记忆健康与自动归档** - 基于艾宾浩斯遗忘曲线的实体保留分析，一键归档遗忘内容
- **版本控制** - 场景历史自动快照，行级 diff 对比，随时回溯
- **技能系统** - AI 技能插件工坊，支持导入/启用/禁用/执行
- **MCP 外部服务器** - 连接外部 MCP 服务器，扩展工具生态
- **数据导出** - 支持 PDF、EPUB、Markdown 等多种格式，导出前可选 Anti-AI 体检
- **模型映射与路由** - 为不同 Agent 独立配置 LLM 模型
- **意图引擎** - 聊天栏自动解析用户意图并调度对应 Agent 执行
- **创作方法论引擎** - 雪花法 / 场景节拍 / 英雄之旅 / 人物深度 / 高密度世界构建，自动注入创作约束
- **StyleDNA 系统** - 六维定量风格模型，52 种经典作家 DNA，实时风格匹配
- **自适应学习系统** - 记录用户反馈、挖掘偏好、动态调节生成参数
- **创作工作流引擎** - 7 阶段全自动工作流（构思→大纲→场景→写作→审阅→迭代→入库）
- **创世引擎** (v5.0.0) - 输入一句话，自动生成完整小说世界：大纲、角色、场景、伏笔，并在幕后自动创建卡片
- **体裁模板库** (v6.0.0) - 43 内置网文体裁模板，支持自定义编辑与外部化配置
- **数据统计** (v6.0.0) - Settings 页面「数据统计」标签，显示最近 30 天各功能使用次数
- **Projection 健康检查** (v6.0.0) - StorySystem 页面每行提交后查看 5 个 Writer 的投影状态
- **用量统计** (v0.7.0) - 全局与单故事 LLM 调用统计（次数/token/响应时间/成功率），最近 20 条调用明细

![幕后界面预览](docs/images/backstage-preview.png)

### 🔄 双窗口无缝协作

| 功能 | 幕前 | 幕后 |
|------|------|------|
| 阅读写作 | ✅ 沉浸式体验 | - |
| 故事管理 | - | ✅ 完整功能 |
| 场景管理 | ✅ 快速切换 / 版本历史 | ✅ 详细编辑 / 故事线拖拽 |
| AI 续写 | ✅ 流式生成 / 自动续写 | ✅ 参数调节 / 方法论约束 |
| 角色查看 | ✅ 悬浮卡片 | ✅ 完整编辑 / 关系图谱 / StyleDNA |
| 知识图谱 | - | ✅ 可视化 / 编辑 / 归档 |
| 技能执行 | ✅ 快捷执行 | ✅ 技能工坊管理 |
| 文本批注 | - | ✅ 场景级批注 |
| 世界构建 | - | ✅ 概念编辑 / 规则管理 / 历史背景 / 文化体系 |
| 创作方法论 | - | ✅ 雪花法 / 节拍表 / 英雄之旅 / 人物深度 / 高密度世界构建 |
| StyleDNA | - | ✅ 风格选择 / 相似度分析 |
| 工作流引擎 | - | ✅ 一键全自动创作 |
| 自适应学习 | - | ✅ 反馈记录 / 偏好挖掘 |
| Ingest 健康 | ✅ 顶栏状态图标 | ✅ 作业记录 / Projection 健康检查 |
| 伏笔追踪 | - | ✅ 伏笔看板全生命周期管理 |
| 体裁模板 | - | ✅ 43 内置模板 / 自定义编辑 |
| Anti-AI 审查 | - | ✅ 导出前体检 + StorySystem 卡片 |
| Pipeline 审校 | ✅ `/` 指令触发修稿/审稿/定稿 | ✅ 场景级 Actions/Drafts/Reviews 面板 |

**快捷键对照**：
- `Ctrl+Enter` / `Cmd+Enter` - 触发 AI 续写（active 模式下）
- `Ctrl+Space` - 循环文思三态模式（off → passive → active）
- `F11` - 禅模式切换（绝对纯净，隐藏所有 UI）
- `Tab` - 接受 AI 幽灵文本建议
- `Esc` - 拒绝 AI 幽灵文本建议
- `/` - 内联命令菜单（续写/润色/古风/场景/自动续写/审校/评点/排版/修稿/审稿/定稿）

---

## ✨ v4.0.0 核心新特性
- **Canonical State** — 规范状态系统，统一聚合故事/角色/伏笔/知识图谱状态，AI 续写时准确知道"当前处于哪个叙事阶段"
- **Payoff Ledger** — 伏笔账本，时间窗口追踪 + 逾期检测 + 智能回收推荐，防止"挖坑不填"
- **Execution Panel** — 章节执行面板，智能推荐下一步行动（处理伏笔/续写/审校）
- **Audit System** — 五维审计（连续性/人物/风格/节奏/伏笔），light/full 模式 + 智能升降级
- **Creation Wizard** — 5 步小说创建向导（创意→世界观→角色→文风→场景）
- **Structured Outline** — Scene 分 stage 编辑（规划/大纲/起草/审校/定稿）
- **Enhanced Streaming** — Markdown 渲染 + 实时字数 + 打字机效果

### 🔧 v4.0.1 修复与优化
- **代码审计** — 扫描 40+ 模块，修复 17 处 IPC 参数不匹配、9 项空实现
- **SQLite 持久化** — `ChatManager` / `StoryStateManager` / `CollabManager` 数据库表结构就绪（预留接口，未启用）
- **协作编辑基础设施** — WebSocket 服务端框架搭建（Operation/Cursor/Leave/Participants 消息协议定义完成，功能未启用）
- **UI 优化** — 编辑器 padding 调整

### 📖 拆书功能：进度提示增强 + 取消支持

**进度提示内容和频次全面升级**
- 后端 `BookAnalyzer` 5 步 Pipeline 每个子步骤都发送详细进度事件
- 元信息识别：准备样本 → 调用LLM → 识别完成（显示书名/类型）
- 世界观提取：准备样本 → 调用LLM → 整理设定
- 人物拆解：每处理一个文本块都发进度，显示"已识别 N 人"
- 章节概要：每处理一章都发进度，显示"已处理 N 章"
- 故事线生成：调用LLM → 解析结构 → 完成（显示支线/高潮数量）
- 保存结果：保存分析结果 → 保存人物 → 保存场景（93% → 96% → 98% → 100%）
- 前端 `AnalysisProgress` 组件新增 8 步骤指示器、百分比数字、块处理信息

**取消分析功能**
- 后端 `TaskExecutionContext` 新增 `is_cancelled()` 检查机制
- `BookAnalyzer` 在每个耗时循环中定期检查任务是否被取消
- 检测到取消后优雅退出，状态更新为 `Cancelled`
- 新增 IPC 命令 `cancel_book_analysis(book_id)`
- 前端分析界面新增"取消分析"按钮，确认后即时中断
- 已取消状态 UI 展示：步骤指示器显示 `!` 标记，进度条变橙色

### 🧠 智能化创作系统（5 阶段重构）

**Phase 1 - 地基重构：真实上下文**
- `StoryContextBuilder` — 从真实数据库构建丰富的 Agent 上下文（世界观、角色、场景结构）
- `QueryPipeline` — 四阶段知识检索（CJK 分词搜索 → 知识图谱扩展 → 预算控制 → 上下文组装）
- `ContinuityEngine` + `ForeshadowingTracker` — 连续性追踪与伏笔回收系统

**Phase 2 - 方法论注入**
- 创作方法论引擎：`MethodologyEngine` 自动将方法论约束注入 Writer 系统提示词
- 五种经典方法论：雪花法（10 步）· 场景节拍表（6 节拍）· 英雄之旅（12 阶段）· 人物深度模型（6 维度）· 高密度世界构建法（4 阶段）
- `AgentOrchestrator` — Writer→Inspector→Writer 质量反馈循环（可配置阈值与最大循环数）

**Phase 3 - 风格深度化**
- `StyleDNA` 六维定量模型：词汇/句法/修辞/视角/情感/对白
- 52 种内置经典作家 DNA：金庸、张爱玲、海明威、村上春树、莫言、普鲁斯特、马尔克斯、鲁迅、老舍、沈从文、余华、王小波、曹雪芹、蒲松龄、苏轼、阿城、白先勇、钱锺书、郁达夫、川端康成、三岛由纪夫、太宰治、夏目漱石、芥川龙之介、东野圭吾、陀思妥耶夫斯基、托尔斯泰、卡夫卡、福克纳、菲茨杰拉德、博尔赫斯、科塔萨尔、爱伦·坡、洛夫克拉夫特、简·奥斯汀、狄更斯、福楼拜、雨果、纳博科夫，以及赛博朋克、蒸汽朋克、新怪谈、硬科幻、史诗奇幻、黑暗奇幻、仙侠修真、无限流等类型风格，另含古典散文、现代极简、黑色侦探、武侠诗意、浪漫绮丽
- 实时风格相似度计算与提示词注入

**Phase 4 - 自适应学习**
- `FeedbackRecorder` — 记录用户对 AI 生成内容的接受/拒绝/修改行为
- `PreferenceMiner` — 五维度启发式偏好挖掘（主题/风格/节奏/视角/结构）
- `AdaptiveGenerator` — 动态调节温度（temperature）、top-p、提示词权重
- `PromptPersonalizer` — 将用户偏好自动注入系统提示词

**Phase 5 - 工作流闭环**
- `CreationWorkflowEngine` — 7 阶段全自动工作流：构思 → 大纲 → 场景设计 → 写作 → 审阅 → 迭代 → 入库
- 3 种创作模式：一键全自动 / AI 初稿 + 人工精修 / 人工初稿 + AI 润色
- `QualityChecker` — 四维质量评估（结构/人物/风格/情节）

### 🎨 品牌焕新

- 全新 Logo：「草苔」立方体标志 —— 融合自然叶脉纹理的几何立方体造型
- `cargo tauri icon logo.png` 生成全平台图标包（Windows / macOS / iOS / Android）

### 🚀 v4.3.0 智能交互创作流程深度优化

**一键创作体验升级**
- Bootstrap前端进度实时可见 — 构思→世界观→角色→场景→撰写，5步进度在状态栏清晰展示
- 创建完成后自动加载 — 新故事自动切换，第一章内容直接呈现于编辑器
- Chapter/Scene双轨同步 — Bootstrap生成第一章时同时创建Chapter记录，前端零延迟加载

**模型驱动编排全面落地**
- 彻底移除关键词匹配 — `detect_and_route_intent` 已清空，所有用户输入交由PlanGenerator自由理解
- PlanContext上下文增强 — 注入世界观摘要、角色列表、活跃伏笔、风格DNA、MCP可用工具
- PlanGenerator Prompt进化 — 新增技能调用指南、设定修改指南、MCP工具使用指南、伏笔处理指南

**设定修改智能响应**
- `update_character` — LLM解析用户修改意图，自动更新角色属性（姓名/性格/背景/目标）
- `update_world_building` — 智能修改世界观规则、历史背景、力量体系
- `update_scene` — 调整场景结构、戏剧目标、地点时间等属性
- 影响检测 — 设定修改后自动标记受影响场景为 `needs_rewrite`，续写时自动重写

**MCP与技能自动化**
- CapabilityRegistry注册MCP工具 — PlanGenerator知道何时调用外部工具搜索资料
- 内置技能自动调用 — `builtin.style_enhancer` / `character_voice` / `emotion_pacing` 可由模型自主编排

### 🏗️ 架构与质量

- **183 项 Rust 测试全部通过**（新增 15 项 planner 模块测试：bootstrap JSON 提取/概念序列化、executor 参数解析、mod PlanContext/PlanStep）
- `cargo check` 零错误零警告
- `npm run build` 通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 5.0.0

---

## 📊 项目状态概览

**当前版本**: v0.7.4  
**最后更新**: 2026-05-21  
**GitHub**: https://github.com/91zgaoge/StoryForge  
**整体完成度**: 100%

> 🍃 品牌图标：「草苔」立方体标志 —— 融合自然叶脉纹理的几何立方体造型，象征创作的结构化生长与文学的立体纵深

| 模块 | 状态 | 完成度 |
|------|------|--------|
| 核心架构 | ✅ 稳定 | 100% |
| 场景化系统 | ✅ 完成 | 100% |
| 记忆系统 | ✅ 完成 | 100% |
| AI 生成 | ✅ 完成 | 100% |
| 知识图谱可视化 | ✅ 完成 | 100% |
| 工作室配置 | ✅ 完成 | 100% |
| 双界面设计 | ✅ 完成 | 100% |
| LLM 集成 / 流式输出 | ✅ 完成 | 100% |
| 本地模型配置 | ✅ 完成 | 100% |
| Agent 系统 / 意图引擎 | ✅ 完成 | 100% |
| 技能系统 / MCP | ✅ 完成 | 100% |
| 版本控制 / 修订模式 | ✅ 完成 | 100% |
| 场景级批注 | ✅ 完成 | 100% |
| 前端界面 | ✅ 完成 | 100% |
| 桌面构建打包 | ✅ 完成 | 100% |
| 创作方法论引擎 | ✅ 完成 | 100% |
| StyleDNA 系统 | ✅ 完成 | 100% |
| 自适应学习系统 | ✅ 完成 | 100% |
| 创作工作流引擎 | ✅ 完成 | 100% |
| 拆书功能 | ✅ 完成 | 100% |
| 任务系统 | ✅ 完成 | 100% |
| 测试覆盖 | ✅ 完成 | ~225 tests |
| 创世引擎 | ✅ 完成 | 100% |
| Story System 合同驱动 | ✅ 完成 | 100% |
| 三层记忆编排器 | ✅ 完成 | 100% |
| 追读力评估系统 | ✅ 完成 | 100% |
| 43 体裁模板库 | ✅ 完成 | 100% |
| Anti-AI 五维审查 | ✅ 完成 | 100% |
| AI 三审 Pipeline | ✅ 完成 | 100% |
| 角色动态状态 | ✅ 完成 | 100% |
| 用量统计 | ✅ 完成 | 100% |

---

## 🗂️ 项目结构

```
v2-rust/
├── src-frontend/                 # 前端代码 (React + TypeScript)
│   ├── src/
│   │   ├── main.tsx             # 幕后入口
│   │   ├── App.tsx              # 幕后主应用
│   │   ├── frontstage/          # 幕前界面
│   │   │   ├── FrontstageApp.tsx
│   │   │   ├── components/
│   │   │   │   ├── RichTextEditor.tsx    # TipTap 富文本编辑器
│   │   │   │   ├── EditorContextMenu.tsx # 右键上下文菜单
│   │   │   │   ├── AiSuggestionBubble.tsx # AI 氛围提示
│   │   │   │   ├── CharacterCardPopup.tsx # 角色卡片弹窗
│   │   │   │   ├── CharacterPeekCard.tsx    # 角色悬浮卡片 (v6.0.0)
│   │   │   │   ├── IngestHealthIndicator.tsx # Ingest 健康指示器 (v6.0.0)
│   │   │   │   ├── ChapterOutline.tsx
│   │   │   │   └── SceneDividerNode.ts      # 场景分隔节点 (v0.7.2)
│   │   │   ├── extensions/          # TipTap 编辑器扩展
│   │   │   └── styles/frontstage.css
│   │   ├── pages/               # 幕后页面
│   │   │   ├── Dashboard.tsx    # 仪表盘
│   │   │   ├── Stories.tsx      # 故事库
│   │   │   ├── Characters.tsx   # 角色管理
│   │   │   ├── Scenes.tsx       # 场景管理
│   │   │   ├── Chapters.tsx     # 章回管理
│   │   │   ├── KnowledgeGraph.tsx # 知识图谱
│   │   │   ├── Skills.tsx       # 技能工坊
│   │   │   ├── Mcp.tsx          # MCP 服务器配置
│   │   │   └── Settings.tsx     # 设置中心
│   │   ├── components/          # 共享组件
│   │   │   ├── StoryTimeline.tsx    # 故事线视图
│   │   │   ├── SceneEditor.tsx      # 场景编辑器
│   │   │   ├── scene-editor/        # 场景编辑器子组件
│   │   │   │   ├── SceneAuditPanel.tsx    # 场景审校面板
│   │   │   │   └── SceneAnnotationPanel.tsx # 场景批注面板
│   │   │   ├── VersionTimeline.tsx  # 版本历史
│   │   │   ├── DiffViewer.tsx       # 版本对比
│   │   │   ├── VectorSearch.tsx     # 向量搜索
│   │   │   ├── NovelCreationWizard.tsx # 创建向导
│   │   │   ├── ExportDialog.tsx
│   │   │   └── pipeline/            # Pipeline 相关组件
│   │   └── hooks/               # 自定义 Hooks
│   │       ├── useScenes.ts
│   │       ├── useIntent.ts         # 意图解析
│   │       ├── useChangeTracking.ts # 修订追踪
│   │       ├── useSceneAnnotations.ts # 场景批注
│   │       ├── usePipeline.ts        # Pipeline 状态
│   │       ├── useWorkflows.ts       # 工作流管理
│   │       ├── useSceneVersions.ts   # 版本管理
│   │       ├── useMcpTools.ts        # MCP 工具
│   │       ├── useVectorSearch.ts    # 向量搜索
│   │       └── useStudioConfig.ts
│   ├── index.html               # 幕后 HTML
│   ├── frontstage.html          # 幕前 HTML
│   └── package.json
│
├── src-tauri/                   # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs              # 应用入口
│   │   ├── lib.rs               # 库入口
│   │   ├── commands.rs          # Tauri 命令
│   │   ├── commands_v3.rs       # V3 命令集
│   │   ├── intent.rs            # 意图解析引擎
│   │   ├── db/                  # 数据库层
│   │   │   ├── models_v3.rs
│   │   │   └── repositories_v3.rs
│   │   ├── agents/              # Agent 系统
│   │   │   ├── service.rs
 │   │   │   ├── orchestrator.rs  # Writer→Inspector 质量闭环
│   │   │   ├── commentator.rs   # 古典评点家
│   │   │   ├── memory_compressor.rs
│   │   │   └── novel_creation.rs
 │   │   ├── creative_engine/     # 智能化创作引擎 (v3.4.0)
│   │   │   ├── mod.rs
│   │   │   ├── context_builder.rs    # 真实 DB 上下文构建
│   │   │   ├── continuity.rs         # 连续性追踪
│   │   │   ├── foreshadowing.rs      # 伏笔回收系统
│   │   │   ├── methodology/          # 创作方法论引擎
│   │   │   │   ├── mod.rs
│   │   │   │   ├── snowflake.rs
│   │   │   │   ├── scene_structure.rs
│   │   │   │   ├── hero_journey.rs
│   │   │   │   ├── character_depth.rs
│   │   │   │   └── high_density_world_building.rs  # 高密度世界构建法 (v0.7.4)
│   │   │   ├── style/                # StyleDNA 系统
│   │   │   │   ├── mod.rs
│   │   │   │   ├── dna.rs
│   │   │   │   └── classic_styles.rs
│   │   │   ├── adaptive/             # 自适应学习系统
│   │   │   │   ├── mod.rs
│   │   │   │   ├── feedback.rs
│   │   │   │   ├── miner.rs
│   │   │   │   ├── generator.rs
│   │   │   │   └── personalizer.rs
│   │   │   └── workflow/             # 工作流引擎
│   │   │       ├── mod.rs
│   │   │       ├── engine.rs
│   │   │       └── quality.rs
│   │   ├── memory/              # 记忆系统
│   │   │   ├── tokenizer.rs
│   │   │   ├── ingest.rs
│   │   │   ├── query.rs
│   │   │   ├── hybrid_search.rs
│   │   │   ├── multi_agent.rs
│   │   │   └── orchestrator.rs       # 三层记忆编排器 (v6.0.0)
│   │   ├── story_system/        # Story System 合同驱动 (v6.0.0)
│   │   │   ├── mod.rs
│   │   │   └── projection_writers.rs
│   │   ├── reading_power/       # 追读力评估系统 (v6.0.0)
│   │   │   └── mod.rs
│   │   ├── anti_ai/             # Anti-AI 五维审查 (v6.0.0)
│   │   │   └── mod.rs
│   │   ├── telemetry/           # 功能使用度量 (v6.0.0)
│   │   │   └── mod.rs
│   │   ├── llm/                 # LLM 适配器
│   │   │   ├── adapter.rs
│   │   │   ├── openai.rs
│   │   │   ├── anthropic.rs
│   │   │   └── ollama.rs
│   │   ├── pipeline/            # AI 三审 Pipeline (v0.7.0)
│   │   │   ├── mod.rs
│   │   │   ├── refine.rs
│   │   │   ├── review.rs
│   │   │   ├── finalize.rs
│   │   │   └── post_process.rs
│   │   ├── book_deconstruction/ # 拆书系统
│   │   │   ├── mod.rs
│   │   │   ├── analyzer.rs
│   │   │   └── service.rs
│   │   ├── auth/                # OAuth 认证（基础设施预留，未启用）
│   │   │   ├── mod.rs
│   │   │   ├── oauth.rs
│   │   │   └── commands.rs
│   │   ├── planner/             # 计划生成与模板学习
│   │   │   ├── mod.rs
│   │   │   ├── executor.rs
│   │   │   └── template_learning.rs
│   │   ├── capabilities/        # 能力自描述与进化
│   │   │   ├── mod.rs
│   │   │   └── evolution.rs
│   │   ├── narrative/           # 叙事元素模型（创世/拆书）
│   │   │   ├── mod.rs
│   │   │   ├── genesis.rs
│   │   │   └── analysis.rs
│   │   ├── canonical_state/     # 规范状态系统
│   │   │   └── mod.rs
│   │   ├── audit/               # 代码审计模块
│   │   │   └── mod.rs
│   │   ├── task_system/         # 任务调度系统
│   │   │   └── mod.rs
│   │   ├── collab/              # 协作编辑（基础设施预留，未启用）
│   │   │   └── websocket.rs
│   │   └── config/              # 配置管理
│   │       └── studio_manager.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── templates/                   # 模板库
│   ├── genres/                  # 43 个体裁模板源码 (v6.0.0)
│   └── genres.json              # 体裁模板外部化配置 (v6.0.0)
├── scripts/                     # 工具脚本
│   ├── verify-ipc-manifest.py   # IPC 命令注册表一致性检查 (v6.0.0)
│   └── migrate-genres-to-json.py # 体裁模板迁移脚本 (v6.0.0)
│
├── docs/                        # 文档
├── README.md
├── CHANGELOG.md
├── ARCHITECTURE.md
└── AGENTS.md
```

---

## 🎨 前端双界面架构

### 技术栈
- **React 18** - UI 框架
- **Vite 6** - 构建工具，支持多入口
- **TypeScript 5.8** - 类型安全
- **Tailwind CSS 3** - 原子化样式
- **TipTap** - ProseMirror 富文本编辑器
- **Zustand** - 轻量状态管理
- **TanStack Query** - 服务端状态管理
- **Tauri 2.4** - 桌面应用框架
- **@dnd-kit** - 拖拽排序
- **ReactFlow** - 知识图谱可视化

### 核心组件

#### RichTextEditor - 幕前富文本编辑器
集成 TipTap 编辑器，极简沉浸式写作体验：
- **幽灵文本** - AI 续写结果以灰色斜体段落内联呈现，附带萤火操作栏
- **内联 `/` 命令菜单** - 9 命令（续写/润色/古风/场景/自动续写/审校/评点/排版/自由指令），光标处触发
- **修订模式** - trackInsert / trackDelete 可视化标记，32px 紧凑横幅
- **古典评点段落** - 金圣叹式朱批内联插入（红色 `LXGW WenKai` 字体）
- **空态引导** - 无内容时显示诗意提示
- **右键上下文菜单** - 修订模式、生成评点、全选/复制/剪切/粘贴

#### StoryTimeline - 故事线视图
可视化场景序列，支持拖拽重新排序：
- 场景卡片展示戏剧目标、冲突类型
- 拖拽手柄调整场景顺序
- 点击选择场景进行编辑

#### SceneEditor - 场景编辑器
三标签页场景编辑：
- **基础信息** - 标题、场景设置、在场角色、记忆压缩
- **戏剧结构** - 戏剧目标、外部压迫、冲突类型（11 种）
- **内容编辑** - 富文本编辑器、场景批注、版本历史

#### KnowledgeGraphView - 知识图谱可视化
基于 ReactFlow 的交互式力导向图谱：
- 节点按实体类型着色，关系边按强度显示粗细
- 实时搜索与类型筛选，双击节点聚焦居中
- 右侧详情面板支持实体就地编辑
- 记忆健康面板与自动归档建议

#### NovelCreationWizard - 创建向导
引导式小说创建流程：
- 类型输入（灰色提示词）
- 世界观 / 角色谱 / 文风卡片式选择
- 完成自动 Ingest 到知识图谱

---

## ✅ 功能实现详情

### 0. 创世引擎 v5.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 故事大纲自动生成 | ✅ | Bootstrap 流程自动产出结构化故事大纲 |
| 角色完整性格小传 | ✅ | 含 personality/goals/appearance/age/gender 的完整角色卡片 |
| 角色关系图谱 | ✅ | 自动构建角色间关系并可视化展示 |
| 伏笔自动埋设 | ✅ | 智能分析故事结构并自动创建伏笔卡片 |
| 知识图谱自动构建 | ✅ | Bootstrap 完成后自动提取实体关系入库 |
| 前后台智能联动 | ✅ | 幕前创建后幕后自动刷新并展示全部卡片 |
| 空白修复 | ✅ | 修复后台隐藏后重新显示白屏问题 |

### 1. 智能化创作系统 v3.4.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| `StoryContextBuilder` | ✅ | 从真实数据库构建丰富的 Agent 上下文 |
| `QueryPipeline` | ✅ | 四阶段知识检索（CJK→图谱→预算→组装） |
| `ContinuityEngine` | ✅ | 章节连续性追踪与伏笔回收 |
| `ForeshadowingTracker` | ✅ | 伏笔埋设与回收追踪 |
| `MethodologyEngine` | ✅ | 雪花法/节拍表/英雄之旅/人物深度/高密度世界构建 |
| `AgentOrchestrator` | ✅ | Writer→Inspector→Writer 质量闭环 |
| `StyleDNA` | ✅ | 六维定量风格模型，52 种经典作家 DNA |
| `StyleAnalyzer` | ✅ | 从文本提取风格指纹 |
| `StyleChecker` | ✅ | 对比文本与目标 DNA 相似度 |
| `FeedbackRecorder` | ✅ | 记录接受/拒绝/修改行为 |
| `PreferenceMiner` | ✅ | 五维度启发式偏好挖掘 |
| `AdaptiveGenerator` | ✅ | 动态调节 temperature/top-p/权重 |
| `PromptPersonalizer` | ✅ | 将用户偏好注入系统提示词 |
| `CreationWorkflowEngine` | ✅ | 7 阶段全自动工作流 |
| `QualityChecker` | ✅ | 四维质量评估（结构/人物/风格/情节） |

### 2. 场景化叙事系统 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| Scene 模型 | ✅ | 戏剧目标、外部压迫、冲突类型 |
| SceneRepository | ✅ | CRUD + 重新排序 |
| 故事线视图 | ✅ | 拖拽排序、场景卡片 |
| 场景编辑器 | ✅ | 三标签页 + 批注 + 版本历史 |
| 冲突类型枚举 | ✅ | 11 种标准冲突类型 |
| 记忆压缩 | ✅ | MemoryCompressorAgent 集成 |

### 3. 记忆系统 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| CJK 分词器 | ✅ | 二元组分词，中日韩支持 |
| Ingest 管线 | ✅ | 两步思维链：分析→生成 |
| 知识图谱 | ✅ | 实体/关系带强度评分，ReactFlow 可视化 |
| 查询检索 | ✅ | 五阶段融合检索（CJK 分词 + 语义向量 + 加权融合 + 图谱扩展 + 预算控制） |
| 多助手会话 | ✅ | 6 种助手类型独立会话 |
| 混合搜索 | ✅ | BM25 + 向量融合 (RRF) |
| FTS5 全文索引 | ✅ | SQLite 原生全文加速 |
| 场景版本 | ✅ | 版本历史、比较、恢复、版本链 |
| 记忆保留 | ✅ | 遗忘曲线、优先级管理、自动归档 |
| 向量持久化 | ✅ | SQLite + LanceDB 混合存储 |

### 4. AI 智能生成 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| NovelCreationAgent | ✅ | 世界观/角色/文风/首个场景生成 |
| 创建向导 | ✅ | 4 步引导流程，自动 Ingest |
| 卡片式 UI | ✅ | 单击选择，双击编辑 |
| 古典评点家 | ✅ | 金圣叹风格段落点评 |
| 意图引擎 | ✅ | 11 种意图解析 + Agent 调度 |
| 真实 SSE 流式 | ✅ | OpenAI / Anthropic / Ollama 全适配 |
| 风格指纹续写 (v0.7.8) | ✅ | 量化风格约束 + 3 候选选优 + 跨段一致性检测 + 四字格密度补偿 |
| 自动更新系统 (v0.7.8) | ✅ | 后台静默下载 + 增量更新 (delta) + 结构化更新日志 |

### 5. 批注与修订系统 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 修订模式 | ✅ | trackInsert / trackDelete，逐条接受/拒绝 |
| 场景批注 | ✅ | 场景级 note/todo/warning/idea 批注 |
| 古典评点 | ✅ | AI 模拟金圣叹风格生成朱红色段落评点 |
| ~~WebSocket 协作~~ | ⏳ | 协作编辑服务端基础设施预留，功能未启用 |

### 6. 工作室配置与扩展 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| StudioConfig 模型 | ✅ | 每部小说独立配置 |
| ZIP 导出/导入 | ✅ | `.storyforge` 格式，选择性导入 |
| 技能系统 | ✅ | 内置 5+ 技能，支持导入/禁用/执行 |
| MCP 服务器 | ✅ | 外部服务器配置与工具调用 |
| 模型映射 | ✅ | Agent → LLM 独立路由 |
| 默认主题 | ✅ | 幕前暖色 / 幕后暗色 |

### 7. 本地模型与构建

| 模块 | 完成度 | 说明 |
|------|--------|------|
| 本地模型配置 | 100% | Gemma / Qwen / bge-m3 |
| LLM 集成 | 100% | OpenAI/Anthropic/Ollama/本地 API |
| Agent 系统 | 100% | 6 种 Agent + 模型路由 |
| 技能系统 | 100% | 内置技能 + 扩展支持 |
| 向量检索 | 100% | TF-IDF + BM25 + 语义 + 混合 |
| 导出功能 | 100% | PDF/EPUB/Markdown |
| Tauri 打包 | 100% | MSI + NSIS 安装包 |

### 8. Story System 合同驱动体系 v6.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| MASTER_SETTING 合同 | ✅ | 故事级全局设定合同，约束所有子合同 |
| Volume 合同 | ✅ | 卷级设定合同，支持多卷结构 |
| Chapter 合同 | ✅ | 章节级设定与预期合同 |
| Review 合同 | ✅ | 审阅与修订合同，记录修改指令 |
| SCENE_COMMIT 提交链 | ✅ | 写后真源提交，驱动 Projection Writer |
| StateProjectionWriter | ✅ | 解析 state_deltas 写入语义记忆 |
| IndexProjectionWriter | ✅ | 解析 entity_deltas 写入实体记忆 |
| SummaryProjectionWriter | ✅ | 自动生成章节摘要并持久化 |
| MemoryProjectionWriter | ✅ | 解析 accepted_events 写入事件记忆 |
| ~~VectorProjectionWriter~~ | ⏳ | LanceDB 向量索引基础设施预留，功能未启用 |
| ContractTree 查询 | ✅ | 按故事/卷/章节层级查询合同树 |
| RuntimeContract 计算 | ✅ | 动态合并上层合同生成运行时合同 |

### 9. 三层记忆编排器 v6.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| Working Memory | ✅ | 最近 5 章 + 活跃角色 + 开放伏笔 |
| Episodic Memory | ✅ | state_changes + relationships 时间线 |
| Semantic Memory | ✅ | 长期事实，按优先级和源章节窗口过滤 |
| MemoryPack 组装 | ✅ | 按任务类型（write/plan/review）动态分配预算 |
| 冲突检测与警告 | ✅ | 记忆项间矛盾检测，输出 MemoryWarning |
| 优先级排序 | ✅ | Critical > High > Medium > Low > Background |

### 10. 追读力评估系统 v6.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| Hook 检测 | ✅ | 悬念/冲突/转折三类钩子识别 |
| Coolpoint 追踪 | ✅ | 打脸/收获/揭秘爽点计数 |
| Micropayoff 微兑现 | ✅ | 章节内小承诺兑现检测 |
| Debt 债务追踪 | ✅ | 未兑现承诺与伏笔债务，含利息计算 |
| OverrideContract | ✅ | 覆盖合同机制，允许临时跳过债务 |
| 综合评分 | ✅ | 0-100 追读力评分与趋势图 |

### 11. 43 体裁模板库 v6.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 内置模板 | ✅ | 玄幻/仙侠/都市/历史/科幻/悬疑/言情/武侠/无限流/系统流/重生/穿越/凡人流/争霸流/幕后流/签到流/御兽流/诡异流/赛博朋克/克苏鲁/国运流/末日拓荒/灵气复苏/规则怪谈/模拟器流/盗墓流/星际机甲等 43 个 |
| 模板五要素 | ✅ | 核心基调、节奏策略、反模式清单、参考数据表、典型结构 |
| 前端体裁选择 | ✅ | StorySystem 页面支持按体裁过滤和查看 |
| 后端 GenreProfile | ✅ | CRUD + 按 ID 查询接口 |

### 12. Anti-AI 五维审查 v6.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 词汇维度 | ✅ | Cliché 检测 + 重复用词统计 |
| 语法维度 | ✅ | 句式多样性 + 被动语态计数 |
| 叙事维度 | ✅ | 段落均匀度 + 感官密度评估 |
| 情感维度 | ✅ | 标签化检测（"愤怒地"）+ 展示 vs 告知 |
| 对话维度 | ✅ | 说明性对话检测 + 标签单调性 |
| 综合评分 | ✅ | 0-100 分 + 改进建议 + 标记段落 |
| 导出前体检 | ✅ | ExportDialog 新增"出版前体检"可选步骤，审查通过后才导出 |
| StorySystem 卡片 | ✅ | Anti-AI 从 Tab 降级为可展开卡片，降低日常干扰 |

### 13. 可靠性与可观测性 v6.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| Ingest 作业追踪 | ✅ | `ingest_jobs` 表（Migration 55），记录 pending/running/completed/failed 状态 |
| Ingest 健康指示器 | ✅ | 幕前顶栏 🧠 图标，显示最近 Ingest 成功/失败，点击展开最近 3 条记录 |
| Projection 健康检查 | ✅ | `check_projection_health` 解析 `projection_status_json`，显示 5 个 Writer 各自状态 |
| 功能使用度量 | ✅ | `feature_usage_logs` 表（Migration 56），本地 SQLite 记录各功能使用次数 |
| 数据统计面板 | ✅ | Settings 页面「数据统计」标签，30 天柱状图 |
| IPC 一致性检查 | ✅ | `scripts/verify-ipc-manifest.py` 自动检测前端调用与后端注册差异 |

### 14. 类型安全基座 v6.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| ts-rs 类型导出 | ✅ | Rust `SyncEvent` / `FrontstageEvent` / `BackstageEvent` 添加 `#[derive(TS)]` |
| 前端穷尽检查 | ✅ | `useSyncStore.ts` switch-case 使用 `assertUnreachable(type: never)` 穷尽匹配 |
| 字段命名一致 | ✅ | 消除 `camelCase + snake_case fallback` 代码，生成类型保证一致性 |

### 15. UX 微优化 v6.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 角色悬浮卡片 | ✅ | RichTextEditor 中 hover 角色名 600ms 显示微型浮卡（状态/外貌/上次出场） |
| 体裁模板外部化 | ✅ | 43 个模板从 Markdown 迁移到 `templates/genres.json`，支持用户自定义 |
| 导出出版前体检 | ✅ | ExportDialog 4 步流程：格式选择 → 健康检查 → 审查中 → 结果展示 |

### 16. AI 三审 Pipeline 系统 v0.7.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| Pipeline 核心架构 | ✅ | `pipeline/mod.rs` — Refine/Review/Finalize 三阶段 + 后处理(post-process) |
| `run_refine` | ✅ | AI 修稿：对章节草稿进行语言润色、结构调整、错别字修正 |
| `run_review` | ✅ | AI 审稿：多维度评分（overall_score + dimensions JSON）+ 问题列表（issues JSON） |
| `run_finalize` | ✅ | 定稿：执行后处理步骤（kb_import / chapter_notes / character_cards / style_analysis） |
| 后处理步骤追踪 | ✅ | `PostProcessStep` 记录每个步骤状态（Running/Success/Failed），支持关键/非关键分类 |
| `run_character_cards` | ✅ | LLM 驱动角色状态解析：构建角色上下文 + 章节内容 Prompt，解析 JSON 状态更新 |
| 前端 Actions/Drafts/Reviews | ✅ | `Stories.tsx` 场景级 Pipeline 面板：三标签页 + 执行/查看/接受按钮 |
| 前端进度看板 | ✅ | 场景列表显示 execution_stage 彩色徽章 + 多色进度条 |
| 幕前 `/` 指令打通 | ✅ | `AI修稿`/`修稿` → `pipeline_refine`、`AI审稿`/`审稿` → `pipeline_review`、`定稿` → `pipeline_finalize` |

### 17. 角色动态状态 v0.7.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 6 项动态状态字段 | ✅ | `cs_location` / `cs_power_level` / `cs_physical_state` / `cs_mental_state` / `cs_key_items` / `cs_recent_events` |
| LLM 自动解析更新 | ✅ | `finalize.rs` 调用 `run_character_cards`，从章节内容提取角色状态变化 |
| `CharacterStatePanel` | ✅ | 前端可折叠状态面板，展示 6 字段 + `cs_updated_at_chapter` 时间戳 |
| 手动修正 | ✅ | 面板内联编辑，调用 `update_character_state` API 实时更新 |
| 角色卡片集成 | ✅ | `Characters.tsx` 每个角色卡片下方展开状态面板 |

### 18. 用量统计与可观测性 v0.7.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| `UsageStats` 页面 | ✅ | 幕后独立页面「用量统计」（Sidebar 导航 + `BarChart3` 图标） |
| 全局统计 | ✅ | 总调用次数 / 总 token 数 / 平均响应时间 / 成功率 |
| 单故事统计 | ✅ | 按故事维度统计 LLM 调用次数与 token 消耗 |
| 最近调用记录 | ✅ | 最近 20 条调用明细表（模型/功能/token/耗时/状态） |
| IPC 命令 | ✅ | `get_llm_call_stats` / `get_recent_llm_calls` |

### 19. v0.7.2 功能增强 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 拆书存储同构化 | ✅ | `reference_characters`/`reference_scenes` 数据统一汇聚到 `narrative_*` 表，Migration 69 自动迁移历史数据 |
| MCP 工具动态注册 | ✅ | `CapabilityRegistry` 实时同步外部 MCP 服务器工具，前端通过 `mcp.{server_id}.*` 前缀区分内置与外部工具 |
| 1:N 聚合编辑 Schema | ✅ | `scene_commits` 新增 `chapter_id` 外键，支持多场景聚合到单一章节 |
| SceneDividerNode | ✅ | TipTap 原子块节点，可视化区分相邻场景内容 |
| LLM 取消机制 | ✅ | `request_id` 级取消信号，`cancel_generation` 精确中断单条请求 |
| AppError 结构化 IPC | ✅ | 统一 `{ code, message, data }` 错误格式，前端按 code 精准处理 |

### 20. 高密度状态世界构建法 (v0.7.3+)

> **核心理念**：源于 90 年代经典老游戏在极致资源约束下的结构智慧，用极少元素通过状态驱动、桥节点连接、事件回流构建远大于实际篇幅的"活的世界"。

| 功能 | 状态 | 说明 |
|------|------|------|
| 后端方法论引擎 | ✅ | `high_density_world_building.rs`：4 阶段完整实现（Seed/StateExpansion/Convergence/DensityIteration） |
| 阶段 1：最小世界种子 | ✅ | 设计高密度"世界切片"，定义核心状态向量与 3-5 个桥节点 |
| 阶段 2：状态网扩张 | ✅ | 主角群扩展，状态触发表，世界规则显式化，信息不对称矩阵 |
| 阶段 3：多线交织与回流 | ✅ | 桥节点多线映射，回流点规划，事件多功能重用，伏笔与回响网络 |
| 阶段 4：密度迭代与克制 | ✅ | 克制检查清单，未写出的世界，状态一致性审计，涌现性验证，重读价值优化 |
| 前端设置面板 | ✅ | `MethodologySettings.tsx` 新增「高密度世界构建」选项 + 4 阶段选择 UI |
| Agent Prompt 注入 | ✅ | `agents/service.rs`  writer prompt 映射接入，AI 创作时自动引用状态向量与桥节点 |
| 输出 Schema | ✅ | 每阶段提供结构化 JSON Schema，规范 AI 输出格式 |

---

## 📅 更新历史

### v0.7.3 (2026-05-20) - 商业模式重构 + 1:N 架构完成 + SceneDivider 预留接口

> **核心理念**：从"模型配额制"升级为"功能订阅制"，软件不介入模型计费；1:N Chapter↔Scene 架构彻底落地，Scene 成为唯一提交粒度。

**商业模式重构：订阅解锁功能，非模型配额（v0.7.3）**
- **`SubscriptionService` 精简**：移除 `QuotaDetail` / `QuotaCheckResult` / `OFFLINE_GRACE_LIMIT` 及所有配额检查/消费方法（`check_auto_write_quota` / `consume_auto_write_quota` / `check_auto_revise_quota` / `consume_auto_revise_quota` / `check_platform_model_quota` / `consume_platform_model_quota` / `check_ai_quota` / `consume_ai_quota`），订阅层级仅控制功能开关（Free/Pro/Enterprise）
- **`has_feature_access` 细粒度权限**：Free 用户可用基础写作、场景/角色管理、知识图谱查询；Pro 解锁 Bootstrap / Pipeline（Refine/Review/Finalize）/ 拆书 / 自动续写 / 自动修改
- **`AppError::SubscriptionRequired` 取代 `QuotaExceeded`**：错误码从 `QUOTA_EXCEEDED` 改为 `SUBSCRIPTION_REQUIRED`，携带 `feature_id` + `current_tier`，前端按 code 渲染升级引导 UI
- **`log_ai_usage` 纯统计**：`ai_usage_logs` 仅记录调用明细（模型/token/耗时），不参与任何配额控制

**1:N Chapter↔Scene 架构完成（Phase 4）**
- **废弃 `chapters.scene_id`**：Migration 71 从 `chapters` 表移除 `scene_id` 列，旧数据通过 `scenes.chapter_id` 反向查询兼容
- **全链路改为 `scenes.chapter_id`**：`ChapterRepository::create`/`update`/`get_by_story` 不再读写 `chapters.scene_id`；`lib.rs` `create_chapter` / `update_chapter` 通过 `SceneRepository::get_by_chapter` 查询关联场景进行聚合提交
- **`SceneCommitService` 取代 `ChapterCommitService`**：提交粒度从 Chapter 彻底对齐到 Scene；`SceneCommitRepository` 全部 SQL 从 `chapter_commits` 重命名为 `scene_commits`（Migration 70）
- **`SceneDividerNode` 功能预留接口**：`SceneDividerRepository`（`create`/`get_by_chapter`/`set_dividers`/`delete`）+ `scene_divider_nodes` 表（Migration 72）就绪，支持在章节编辑器中标记场景边界

**高密度状态世界构建法（v0.7.3+）**
- **新增第五种创作方法论**：`high_density_world_building.rs` 完整实现 4 阶段世界构建流程
- **阶段 1 - 最小世界种子**：设计高密度"世界切片"，定义核心状态向量与 3-5 个桥节点，每个桥节点至少连接 3 条叙事线
- **阶段 2 - 状态网扩张**：主角群扩展 + 状态触发表 + 世界规则显式化 + 信息不对称矩阵
- **阶段 3 - 多线交织与回流**：桥节点多线映射（正面/侧面/误解视角），每 3-5 章一次回流，事件至少承担 3 种功能
- **阶段 4 - 密度迭代与克制**：克制检查清单 + 未写出的世界 + 状态一致性审计 + 涌现性验证 + 重读价值优化
- **前端集成**：`MethodologySettings.tsx` 新增「高密度世界构建」选项及 4 阶段选择 UI
- **Agent 接入**：`agents/service.rs` writer prompt 映射接入，AI 创作时自动注入状态向量、桥节点、事件回流等核心概念

**编译与测试**
- `cargo check`：零错误
- `cargo test`：~225/225 通过
- `npm run build`：通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 0.7.3

---

### v0.7.2 (2026-05-19) - 存储同构化 + MCP 动态注册 + 聚合编辑 + 场景分隔节点 + LLM 取消

> **核心理念**：夯实数据架构与扩展能力，为后续云端同步与协作编辑奠定 schema 基础。

**拆书分析存储同构化**
- **`reference_characters` → `narrative_characters`**：`BookDeconstructionExecutor` 保存结果时统一写入 `narrative_characters` 表，`source='extracted'` / `status='reference'`，消除 `reference_characters` 与 `narrative_characters` 的数据孤岛
- **`reference_scenes` → `narrative_scenes`**：同上，拆书场景统一汇入 `narrative_scenes`
- **Migration 69**：自动迁移历史 `reference_characters` / `reference_scenes` 数据到 `narrative_*` 表，`INSERT OR IGNORE` 避免重复
- **`delete_book` 级联清理**：删除拆书结果时同步清理 `narrative_characters` / `narrative_scenes` / `narrative_world_buildings`
- **`convert_to_story` 状态切换**：一键转故事时自动将 `narrative_*` 记录 `status` 从 `reference` 更新为 `active`

**MCP 工具动态注册**
- **`CapabilityRegistry` 实时同步**：外部 MCP 服务器连接成功后，自动将其工具列表注册到 `CapabilityRegistry`，`PlanGenerator` 无需重启即可感知新工具
- **前缀命名空间**：外部工具统一使用 `mcp.{server_id}.{tool_name}` 前缀，与内置 `mcp.builtin.*` 工具区分，前端 `list_mcp_tools` 直接展示完整带前缀工具名
- **动态注销**：MCP 服务器断开时从 `CapabilityRegistry` 移除对应工具，避免调用已失效工具

**1:N 聚合编辑数据库 Schema**
- **Migration 68**：`scene_commits` 表新增 `chapter_id` 字段（`Option<String>`），支持多场景（Scene）聚合到单一章节（Chapter）的提交追踪
- **Schema 对齐**：`SceneCommit` 数据结构、`SceneCommitRepository` 查询/插入 SQL、`SceneCommitService` 全部适配新字段

**TipTap SceneDividerNode**
- **原子块节点**：新增 `SceneDividerNode`（`group: 'block'`，`atom: true`），在幕前编辑器中可视化渲染场景边界分隔线
- **不可编辑**：用户无法直接输入或删除分隔节点内容，仅作为结构标记存在
- **事件交互**：支持 `click` / `mouseenter` / `mouseleave` 事件，可扩展悬浮提示场景信息

**LLM 调用取消机制**
- **`request_id` 级取消**：`LlmService` 维护 `cancel_senders: HashMap<String, Sender<()>>`，每次生成分配唯一 `request_id`
- **`cancel_generation(request_id)`**：精确中断单条请求，避免取消影响其他并行 LLM 调用
- **流式适配**：`generate_stream` 在接收每个 chunk 前检查取消信号，响应立即终止

**AppError 结构化 IPC**
- **统一错误格式**：所有 Tauri 命令返回 `Result<T, AppError>`，序列化为 `{ code: string, message: string, data?: unknown }`
- **错误码体系**：`NOT_FOUND` / `VALIDATION_ERROR` / `LLM_ERROR` / `CANCELLED` / `TIMEOUT` 等标准 code
- **前端精准处理**：`loggedInvoke` 解析 `AppError`，按 `code` 做 toast / 重试 / 静默忽略等差异化处理

**编译与测试**
- `cargo check`：零错误
- `cargo test`：~225/225 通过
- `npm run build`：通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 0.7.2

---

### v0.7.0 (2026-05-15) - AI 三审 Pipeline + 角色动态状态 + 用量统计 + 幕前指令升级

> **核心理念**：从"能创作"到"高质量创作"，引入工业化级 AI 审校流水线。每章正文经过修稿→审稿→定稿→后处理四级质量关卡，角色状态随剧情自动演进，用量透明可观测。

**AI 三审 Pipeline 系统**
- **四级创作管线**：`Rewrite` → `Refine`（AI 修稿）→ `Review`（AI 审稿）→ `Finalize`（定稿 + 后处理）
- **Refine 修稿**：对章节草稿进行语言润色、结构调整、错别字修正，输出精炼版本
- **Review 审稿**：多维度评分（overall_score 0-100 + dimensions JSON）+ 问题列表（issues JSON）+ 改进建议
- **Finalize 定稿**：通过后处理步骤自动完成知识入库、章节笔记生成、角色状态更新、风格分析
- **后处理步骤追踪**：`PostProcessStep` 记录 kb_import / chapter_notes / character_cards / style_analysis 执行状态，关键步骤失败阻断、非关键步骤失败记录
- **LLM 驱动角色状态解析**：`run_character_cards` 构建角色上下文 + 章节内容 Prompt，调用 LLM 输出 JSON 状态更新，自动更新 `cs_location`/`cs_power_level`/`cs_physical_state`/`cs_mental_state`/`cs_key_items`/`cs_recent_events`
- **前端 Pipeline 面板**：`Stories.tsx` 场景级 Actions（执行修稿/审稿/定稿）/ Drafts（草稿列表）/ Reviews（审稿结果）三标签页
- **场景进度看板**：场景列表显示 `execution_stage` 彩色徽章（plan/outline/draft/review/final）+ 多色进度条
- **幕前 `/` 指令打通**：`AI修稿`/`修稿` → `pipeline_refine`、`AI审稿`/`审稿` → `pipeline_review`、`定稿` → `pipeline_finalize`，幕前编辑器内直接触发 Pipeline

**角色动态状态面板**
- **6 项动态状态**：`cs_location`（当前位置）、`cs_power_level`（实力等级）、`cs_physical_state`（身体状态）、`cs_mental_state`（心理状态）、`cs_key_items`（关键物品）、`cs_recent_events`（近期事件）
- **自动更新**：定稿时 LLM 自动解析章节内容提取角色变化，写入数据库
- **手动修正**：`CharacterStatePanel` 组件支持内联编辑，实时更新角色状态
- **时间戳追踪**：`cs_updated_at_chapter` 记录最后更新章节号

**用量统计看板**
- **全局统计**：总调用次数 / 总 token 数 / 平均响应时间 / 成功率
- **单故事统计**：按故事维度聚合 LLM 调用与 token 消耗
- **最近调用明细**：最近 20 条调用记录表（模型/功能/token/耗时/状态/时间）
- **独立页面**：幕后 Sidebar「用量统计」入口，`UsageStats.tsx` 完整实现

**架构优化（2026-05-17）**
- **`SceneCommitService::auto_commit` 防抖聚合提交**：取代旧版独立 `auto_ingest_chapter` 摄取管线（5 分钟冷却），改为 30 秒空闲延迟后自动聚合提交，驱动 `MemoryProjectionWriter`，消除重复索引工作
- **导出聚合完整性**：`export_story` 导出前自动检查章节内容，空章节按关联场景的 `sequence_number` 排序聚合填充，确保 Markdown/HTML/PlainText 导出完整无缺
- **大型组件提取重构**：`Settings.tsx` 提取 8 个原子化子组件（`ModelCard`/`ModelList`/`ModelModal`/`StatsSettings`/`MethodologySettings`/`WorkflowSettings`/`GeneralSettings`/`AccountSettings`）；`SceneEditor.tsx` 提取 `SceneAuditPanel` 和 `SceneAnnotationPanel` 到 `scene-editor/` 子目录
- **`StoryTimeline.tsx` 场景进度徽章**：场景卡片新增 `execution_stage` 状态徽标（plan/outline/draft/review/final），叙事阶段（铺垫/上升/高潮/收尾）与执行阶段双轨可视化

**编译与测试**
- `cargo check`：零错误
- `cargo test`：~225/225 通过
- `npm run build`：通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 0.7.0

---

### v6.0.0 (2026-05-15) - Story System 合同驱动 + 三层记忆 + 追读力 + 体裁模板 + Anti-AI 审查

> **核心理念**：全面重构小说创作的"写前-写中-写后"质量管控闭环，从"自由创作"升级为"合同约束下的高质量创作"。

**Story System 合同驱动体系**
- **四级合同架构**：`MASTER_SETTING`（故事级）/ `Volume`（卷级）/ `Chapter`（章节级）/ `Review`（审阅级）
- **SCENE_COMMIT 提交链**：写后真源分离，每次场景保存生成 `SceneCommit`，驱动 5 个 Projection Writer 自动更新 read-model
- **5 个 Projection Writer**：State（状态变化）、Index（实体索引）、Summary（章节摘要）、Memory（事件记忆）、Vector（向量嵌入）
- **ContractTree & RuntimeContract**：按层级查询合同树，动态合并生成运行时约束合同
- **防幻觉三定律**：合同即法律、设定即物理、发明需识别

**三层记忆编排器**
- **Working Memory**：最近 5 章 + 活跃角色 + 开放伏笔，实时上下文窗口
- **Episodic Memory**：state_changes + relationships 时间线，支持叙事连续性
- **Semantic Memory**：长期事实存储，按优先级（Critical > High > Medium > Low）和源章节窗口过滤
- **MemoryPack 动态组装**：按任务类型（write/plan/review）分配不同预算权重

**追读力评估系统**
- **Hook 检测**：悬念/冲突/转折三类钩子识别与计数
- **Coolpoint 追踪**：打脸/收获/揭秘三类爽点追踪
- **Micropayoff 微兑现**：章节内小承诺兑现检测
- **Debt 债务追踪**：未兑现承诺与伏笔债务，含利息计算与逾期检测
- **OverrideContract**：覆盖合同机制，允许作者声明临时跳过债务并记录理由
- **综合评分**：0-100 追读力评分，支持趋势图查询

**43 体裁模板库**
- **内置 43 个网文体裁模板**：玄幻/仙侠/都市/历史/科幻/悬疑/言情/武侠/无限流/系统流/重生/穿越/凡人流/争霸流/幕后流/签到流/御兽流/诡异流/赛博朋克/克苏鲁/国运流/末日拓荒/灵气复苏/规则怪谈/模拟器流/盗墓流/星际机甲等
- **模板五要素**：核心基调、节奏策略、反模式清单、参考数据表、典型结构
- **前端集成**：StorySystem 页面支持按体裁过滤和查看详情

**Anti-AI 五维审查**
- **词汇维度**：Cliché 检测（"浩瀚"、"磅礴"等 AI 高频词）+ 重复用词统计
- **语法维度**：句式多样性（标准差评估）+ 被动语态计数
- **叙事维度**：段落长度均匀度 + 感官密度（视觉/听觉/触觉/嗅觉/味觉关键词密度）
- **情感维度**：标签化检测（"愤怒地"、"悲伤地说"）+ 展示 vs 告知判断
- **对话维度**：说明性对话检测（ exposition_ratio ）+ 标签单调性（"说道"重复）
- **输出**：综合评分 + 各维度评分 + 改进建议 + 标记段落列表
- **导出前体检**：ExportDialog 新增 4 步导出流程，可选运行 Anti-AI 审查后再导出
- **StorySystem 卡片化**：Anti-AI 从顶部 Tab 降级为可展开卡片，降低日常干扰，保留功能入口

**类型安全基座**
- **ts-rs 自动生成 TypeScript 绑定**：Rust `SyncEvent` / `FrontstageEvent` / `BackstageEvent` 添加 `#[derive(TS)]`，编译时自动生成类型
- **前端穷尽匹配**：`useSyncStore.ts` 重构为 typed discriminated union，`default` case 使用 `assertUnreachable(type: never)` 实现穷尽检查
- **IPC 一致性检查**：`scripts/verify-ipc-manifest.py` 自动比对后端 `generate_handler![]` 与前端的 `loggedInvoke` 调用，防止命令注册遗漏

**可靠性与可观测性**
- **Ingest 作业追踪**（Migration 55）：`ingest_jobs` 表记录每次 Ingest 的 pending/running/completed/failed 状态，失败原因持久化
- **Ingest 健康指示器**：幕前顶栏新增 🧠 状态图标，最近 Ingest 成功/失败一目了然，点击浮层展示最近 3 条记录
- **Projection 健康检查**：`check_projection_health` 解析 `scene_commits.projection_status_json`，显示 5 个 Writer 各自成功/失败状态
- **功能使用度量**（Migration 56）：`feature_usage_logs` 表本地记录各功能使用次数，Settings 页面「数据统计」标签展示 30 天柱状图
- **技术债务清理**：删除 `src-tauri/src/tests/bug_condition_v57.rs` 中 11 个已修复的 `#[ignore]` 测试，将核心场景转化为生产代码中的 `debug_assert!` 和 `// EVENT_REQUIRED` 注释标记

**UX 微优化**
- **角色悬浮卡片**：RichTextEditor 中鼠标 hover 角色名 600ms 后显示 140px 微型浮卡，展示状态标签、外貌摘要、上次出场章节
- **体裁模板外部化**：43 个 Markdown 模板迁移到 `templates/genres.json`，启动时优先读取 `{app_data_dir}/templates/genres.json`，支持用户自定义和删除非内置体裁

**编译与测试**
- `cargo check`：零错误（~121 warnings）
- `cargo test`：~225/225 通过（11 项历史 bug condition 测试已删除，0 ignored）
- `npm run build`：通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 6.0.0

### v5.4.1 (2026-05-07) - Bootstrap 编辑器内容丢失修复

**修复问题**：创世流程中"小说已创建但编辑器无文字"

- **`FrontstageEvent::ChapterSwitch` 直接带内容**：后端 `FirstChapterGenerationStep` 生成第一章后，通过 `ChapterSwitch` 事件直接把正文内容传递给前端，不再依赖前端重新查询数据库
- **前端优先用事件内容**：`ChapterSwitch` 事件处理中，如果 `payload.content` 非空，直接 `setContent(payload.content)`，完全绕过 DB 查询竞态
- **chaptersRef 为空时自动重查**：如果本地 chapters 缓存为空（竞态导致），自动重新调用 `get_story_chapters` 获取最新章节
- **`final_content` 兜底机制**：`smartExecute` 返回后，如果数据库查询的 `content` 为空但 `result.final_content` 有内容，直接使用 `final_content` 回写编辑器
- **`loadStories` 竞态保护**：Bootstrap 期间（`isGenerating=true`）禁止自动 `selectStory`，避免在 `FirstChapterGenerationStep` 尚未完成时拿到空 chapters 列表导致编辑器被清空
- **修复 `agents/commands.rs` 编译错误**：补全另一处 `ChapterSwitch` 事件的 `content` 字段

### v5.4.0 (2026-05-04) - 向量检索语义化：从关键词到语义理解
- **OllamaEmbeddingProvider** — `embeddings/provider.rs` 新增 Ollama 语义嵌入后端，支持 `nomic-embed-text` / `all-minilm` / `mxbai-embed-large` 等模型
- **全局语义嵌入路由** — `embed_text_async()` 优先查询全局 `EmbeddingProvider`（Ollama/OpenAI），失败 graceful fallback 到本地 FNV-1a 哈希；`tokio::sync::Mutex` 保证跨 async 边界 `Send` 安全
- **QueryPipeline 五阶段融合** — `memory/query.rs` 从四阶段扩展为五阶段：1a `token_search`（CJK 分词）+ 1b `semantic_search`（embedding 生成 → `search_with_embedding`）+ 1c `fuse_results`（token 权重 0.4 / 语义权重 0.6 加权融合，去重+折半补偿+Top50 截断）+ 2 图谱扩展 + 3 预算控制 + 4 上下文组装
- **LanceDB 真实向量索引** — `vector/lancedb_store.rs` 接入 IVF-PQ + Cosine 距离语义检索，`VectorStore` trait 扩展 `search_with_embedding` 接口；`DbVectorStore` 返回空结果 graceful 降级
- **测试覆盖** — 新增 6 个 `fuse_results` 单元测试，Rust 总测试数 211→217
- **编译验证** — `cargo check` 零错误，`cargo test` 217/217，`npm run build` 通过，`cargo check --release` 通过

### v5.3.1 (2026-05-03) - Bootstrap 体验修复 + 幕后数据刷新
- **Bootstrap 重复显示小说开头** — `handleSmartGeneration` 完成时不再设置 `generatedText` 幽灵文本，避免与 `ChapterSwitch` 加载的 `chapter.content` 叠加
- **幕后结构要素不显示** — `useSyncStore` 中 `invalidateQueries` 的 queryKey 与 hooks 实际使用的 key 不一致（`world-building`≠`world_building`），修复后 TanStack Query 缓存正确过期
- **Bootstrap 解析失败** — 给所有 `NarrativeElement` 结构体字段添加 `#[serde(default)]`，允许 LLM 返回 JSON 省略后端生成字段
- **Bootstrap 生成中断** — `StoryContextBuilder::build` 查询失败时返回默认值；LLM 缺少字段添加 `#[serde(default)]`
- **续写重复生成开头** — `current_content_preview` 从尾部截断 6000 字符保留最新内容
- **后台数据刷新统一通道** — 后台阶段完成后通过 `StateSync::emit_data_refresh()` 发射标准 `sync-event` 事件

### v5.3.0 (2026-05-02) - 叙事元素模型重构：创世-拆书同构架构
- 统一叙事元素模型：`narrative/` 模块 — 正向/逆向共用同一套数据结构
- GenesisPipeline：7步正向流程（概念→世界观→大纲→角色→场景→伏笔→知识图谱）
- AnalysisPipeline：7步逆向流程（元数据→世界观→角色→场景→故事线→伏笔→知识图谱）
- 统一进度系统：`PipelineProgressEvent` + `usePipelineProgress` Hook
- 统一存储层：Migration 38 + `NarrativeRepository`
- StoryHealthAnalyzer：6维度结构健康检查 + `analyze_story_structure` IPC

### v5.2.2 (2026-05-02) - Bootstrap 两阶段架构重构：先出正文，后台完善
- **两阶段执行模型**：`bootstrap.rs` `run_quick_phase()`（同步：概念+正文，2-3分钟）+ `run_background_phase()`（异步：世界观/大纲/角色/场景/伏笔/知识图谱，5-8分钟）
- **用户等待时间**：从 10+ 分钟缩短到 **2-3 分钟**
- **前端体验**：即时完成后"小说已创建！您可以开始写作了"；后台进行中"后台正在完善小说世界..."；完成后 toast "创世完成！"

### v5.2.1 (2026-05-02) - 超时修复与白屏修复
- Bootstrap 超时延长：180 秒→600 秒，匹配本地大模型多步 LLM 调用实际耗时
- 进度密度增强：`bootstrap.rs` 每个 LLM 调用前后增加细粒度进度事件
- LLM 心跳加速：间隔 3 秒→2 秒，上限 40 次→300 次，消息优化为"正在深度思考中..."
- 后台窗口白屏修复 v5.2.1：双重维度尺寸微调 + JS `html+body` 双重重排 + 800ms 延迟 + 前端双重重绘

### v5.2.0 (2026-05-02) - 设计-实现对齐全面完成
- 通用 Workflow 引擎：完整 DAG 节点执行器（WriteChapter/Inspect/Revise/VectorIndex/AnalyzePlot/Condition/Parallel/End）
- 能力进化反馈环：ExecutionRecordStore JSON 持久化 + LLM 分析生成改进建议
- 幕前↔场景双向同步：chapterUpdated 刷新 scenes 缓存 + 编辑器自动刷新（3 秒防循环）
- QueryPipeline 降级感知：`context-degraded` 事件 + 前端 toast 提示
- 废弃组件清理：`FrontstageToolbar` 从索引导出移除
- 版本号统一：v5.2.0

### v5.1.1 (2026-05-01) - 设计-实现对齐全面修复
- **P0 修复**: `update_chapter` / `create_chapter` 保存后自动触发 IngestPipeline，知识图谱实时更新
- **P0 修复**: `state_sync` character/chapter update/delete 事件携带正确 `story_id`
- **P0 修复**: `FrontstageToolbar` 传递 `story_id` 到 `show_backstage`
- **P0 修复**: `WorkflowScheduler::schedule_execution` 实现队列机制
- **P1 补全**: `PromptLibrary` 新增 StyleChecker + Commentator 模板
- **P1 补全**: `prompts/methodologies/` 雪花法/英雄之旅/场景结构模板

### v5.1.0 (2026-05-01) - 幕前幕后自动关联对齐

> **核心理念**：从"各自为战"到"自动联动"。所有数据修改自动同步，前后台零延迟对齐。

**幕前幕后自动关联**
- **Chapter↔Scene 关联** — `scenes.chapter_id` 外键关联，`ChapterRepository` 自动查找/创建关联 Scene（v0.7.3 废弃 `chapters.scene_id`，1:N 架构下单一章节可包含多个场景）
- **统一实时状态中心** — 后端 `state_sync` 模块定义 16 种 `SyncEvent`，所有数据修改命令完成后自动发射同步事件
- **前端 useSyncStore Hook** — 监听 `sync-event` 频道，根据事件类型自动 `invalidateQueries` / `removeQueries`
- **Bootstrap 完成后幕前自动加载** — `smartExecute` 返回后检测 `story_created:` 消息，自动加载新故事并切换到第一章；Bootstrap 完成后双重 `ChapterSwitch` 保险
- **幕前→幕后快速跳转** — `Ctrl+Shift+B` 快捷键，标题栏点击，幕后自动定位当前故事并高亮
- **AgentOrchestrator 闭环接入** — `execute_writer` 集成 `AgentOrchestrator::execute_write_with_inspection`，Writer→Inspector→StyleChecker→Writer 自动质检改写生效
- **自适应学习闭环激活** — `record_feedback` 成功后异步触发 `mine_preferences`，偏好挖掘自动运行
- **Zustand↔TanStack Query 同步** — `App.tsx` 监听 `currentStory` 变化，自动刷新关联数据缓存
- **窗口通信事件标准化** — `DataRefresh` 统一由 `useSyncStore` 处理，消除 `backstage-update` 和 `handleWindowShown` 中的重复刷新

**编译与测试**
- `cargo check`：零错误
- `cargo test`：193/193 全部通过
- `npm run build`：通过

### v5.0.0 (2026-04-30) - 创世引擎：一键创世，万物关联

> **核心理念**：输入一句话，系统自动在幕后所有对应栏目中创建完整关联卡片。从"能创建"到"自动构建完整世界"。

**创世引擎全流程**
- **故事概念生成** — 解析用户意图，生成故事标题、类型、基调、简介
- **第一章直出正文** — 同步生成第一章正文并以 ghost text 展示在幕前编辑器
- **世界观自动构建** — 生成世界观规则、历史背景、力量体系、地理环境
- **故事大纲自动生成** — 产出完整的故事结构、主线/支线、关键转折点
- **角色完整性格小传** — 每个角色含 personality（性格）、goals（目标）、appearance（外貌）、age（年龄）、gender（性别）的完整卡片
- **角色关系图谱** — 自动分析角色间关系并构建可视化图谱
- **场景自动设计** — 按大纲生成场景卡片，含戏剧目标、冲突类型、场景设置
- **伏笔自动埋设** — 智能识别故事中的伏笔点并创建追踪卡片
- **知识图谱自动构建** — 从所有生成内容中提取实体和关系，自动入库并可视化
- **前后台智能联动** — 幕前创建完成后，幕后自动刷新并展示所有生成的卡片，无需手动切换

**关键 Bug 修复**
- **修复后台白屏** — 修复后台窗口隐藏后重新显示时出现空白/白屏的问题
- **修复卡片不显示** — 修复 Bootstrap 完成后幕后不显示生成的角色、场景、伏笔、故事大纲卡片的问题

**编译与测试**
- `cargo check`：零错误零警告
- `cargo test`：193/193 全部通过
- `npm run build`：通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 5.0.0

### v4.2.0 (2026-04-23) - 智能交互设计重构 V2：模型驱动的编排范式

> **核心理念**：从"程序式编排"转向"模型式编排"。人类只定义能力能做什么，模型负责决定什么时候用、怎么用、按什么顺序。真正实现"越写越懂"。

**架构变革**
- **CapabilityRegistry（能力自描述系统）**：Agent 和 Skill 用自然语言描述自己的能力（`description` / `when_to_use` / `input_description` / `output_description`），模型阅读这些描述自主选择需要的能力。人类不再写死 Agent 映射规则。
- **PlanGenerator（模型计划生成器）**：取代旧的 IntentParser + IntentExecutor。LLM 接收系统状态 + 用户输入 + 能力清单，自主输出执行计划（自由文本理解 + 步骤列表 + 参数 + 依赖关系）。没有预设分类标签，没有关键词匹配。
- **PlanExecutor（计划执行引擎）**：Dumb executor，只做一件事——忠实地执行 LLM 生成的计划。按顺序执行步骤、将前一步输出传给后一步、失败时按备选方案处理。所有决策已在计划中。
- **PromptEvolver（提示词进化器）**：每次调用 Agent/Skill 前，LLM 根据当前故事上下文（题材、叙事阶段、用户偏好）自由改写整个 prompt。不是模板变量替换，而是真正的"进化"。
- **AiLearningIndicator（记忆显性化）**：前端组件，每次 AI 交互后展示"系统学到了什么"（如"已记录：你偏好快节奏打斗"）。让"越写越懂"对用户可见。
- **CapabilityEvolutionEngine（能力进化反馈环）**：记录每次能力调用的执行结果，长期优化能力描述的准确性。
- **PlanTemplateLibrary（计划模板学习）**：记录成功的执行计划，类似请求来时直接复用或微调。

**移除的程序式规则**
- 移除 `IntentType` 枚举（11 类预设分类）
- 移除前端正则关键词检测（`/写|创作|生成|续写/`）
- 移除 `IntentExecutor.map_agents` 写死映射
- 移除 `FrontstageApp` 中 `if (!currentStory) { toast.error(...) }` 强制流程
- 移除所有基于关键词的意图分支判断

**前端简化**
- `handleSmartGeneration` / `handleRequestGeneration` 统一走 `smart_execute`
- 用户任何输入都交给模型决定如何处理
- 新增 `smartExecute` IPC 命令

**编译与测试**
- `cargo check`：零错误零警告
- `cargo test`：160/160 全部通过
- `npm run build`：通过

### v4.1.0 (2026-04-22) - 幕前界面深度重构：化整为零，萤火随行

> **设计理念**：从 20+ 可见 UI 元素缩减至 <5 持久元素。AI 功能以萤火暗示（firefly hints）形式按需浮现，用完即隐。

**P0 核心重构**
- 顶栏精简为 44px 细线：小说标题/章节信息/字数/字号/文思三态/禅模式。快捷键提示改为 F1 浮动面板
- 底栏删除：彻底删除底部聊天工具栏。AI 结果以幽灵文本内联呈现，Tab 接受 / Esc 拒绝
- 侧边栏精简：5 按钮 → 3 按钮（修/批/幕）
- 键盘快捷键：`Ctrl+Enter` 续写，`Ctrl+Space` 循环文思，`F11` 禅模式

**P1 萤火系统**
- 幽灵文本：编辑器末尾灰色斜体段落 + 萤火操作栏
- 右边缘萤火：`smartGhostText` 淡入（0.8s）→ 停留 → 淡出（1.2s）
- 空态引导：编辑器无内容时居中诗意提示

**P2 体验优化**
- 内联 `/` 指令输入框：输入 `/` 后弹出浮动输入框，直接输入任意指令
- WenSiPanel 浮动化：从底栏嵌入改为右下角浮动卡片，新增"自由指令"标签页
- 修订横幅精简：多行可展开 → 32px 单行
- 古典评点保留：金圣叹式朱批内联段落，红色 `oklch(55% 0.18 25)`

**🗑️ 移除（设计决策）**
- 显式注释/评论系统：sidebar "注"按钮、注释/评论面板、选中文本弹窗创建按钮、右键菜单项、相关 hooks
- 原因：AI 写作工具不需要创作者标注自己的作品；AI 反馈应以幽灵文本或古典评点形式自然呈现

**📊 统计**
- `cargo check` 零错误零警告，`cargo test` 160/160，`npm run build` 通过
- 修改文件：前端 8 个，删除代码约 800 行

### v4.0.1 (2026-04-22) - 全面代码审计与空实现修复

**Phase A - 代码审计与 P0 修复**
- 综合代码审计：扫描 40+ 模块，识别 5 严重 / 17 参数 / 9 空实现
- IPC 参数统一：17 处 camelCase→snake_case（Tauri v2 反序列化修复）
- 空实现补全：`analytics` 真实统计、`agents/commands` 真实状态、`skills/executor` 真实 MCP 调用、`export/import_from_text` 正则解析章节、`workflow/scheduler` 执行日志、`evolution/updater` manifest CRUD、`mcp/server` 缺失 `.await`
- 前端修复：移除硬编码 API keys、移除 mock 流式、文本分析器增量分析
- UI 优化：编辑器 padding 优化
- 类型统一：移除重复 `McpServerConfig`

**Phase B - 内存模块 SQLite 持久化（基础设施预留）**
- Migration 26/27/28：`chat_sessions` + `chat_messages`、`story_runtime_states`、`collab_sessions` + `collab_participants` 表结构就绪
- `ChatManager` / `StoryStateManager` / `CollabManager` / `WebSocketServer`：数据库表与消息协议定义完成，功能未启用

### v3.4.0 (2026-04-18) - 智能化创作系统（5 阶段重构）

- **Phase 1 地基重构** - `StoryContextBuilder` 真实 DB 上下文, `QueryPipeline` 四阶段检索, `ContinuityEngine`, `ForeshadowingTracker` (27 tests)
- **Phase 2 方法论注入** - 雪花法/场景节拍/英雄之旅/人物深度 + `MethodologyEngine` + `AgentOrchestrator` Writer→Inspector 闭环 (34 tests)
- **Phase 3 风格深度化** - `StyleDNA` 六维模型, 52 经典作家 DNA, `StyleAnalyzer`, `StyleChecker` (45 tests)
- **Phase 4 自适应学习** - `FeedbackRecorder`, `PreferenceMiner`, `AdaptiveGenerator`, `PromptPersonalizer` (54 tests)
- **Phase 5 工作流闭环** - `CreationWorkflowEngine` 7 阶段工作流, `QualityChecker` 四维评估, 3 种创作模式 (63 tests)
- **品牌焕新** - `logo.png` 立方体标志生成全平台图标包
- **版本统一** - Cargo.toml / package.json / tauri.conf.json → 3.4.0

### v3.3.0 (2026-04-15) - 功能断层修复与架构清理

- **幕前右键菜单修复** - Tailwind utilities 补充、事件捕获修复、WebView2 默认菜单禁用、暖色 UI 重构
- **MCP 外部服务器连接** - 配置卡片 + 工具调用
- **技能导入** - 本地文件选择器导入
- **Agent 流式执行与取消** - 实时进度 + 可中断
- **知识图谱实体就地编辑** - 节点属性增删改
- **版本系统增强** - 版本链视图 + diff 元信息
- **LLM 调用路径决策** - 明确 HTTP 直连为官方路径
- **Rust Warnings 降噪** - `cargo check` 0 警告

### v3.2.0 (2026-04-14) - 意图引擎与 Agent 调度 + 知识图谱可视化 + 修订模式

- **知识图谱可视化** - ReactFlow 力导向图谱，搜索/筛选/实体编辑
- **记忆健康与自动归档** - 艾宾浩斯遗忘曲线 + 一键归档
- **意图解析引擎** - 11 种意图类型，聊天栏自动调度 Agent
- **Agent 模型映射** - 按 Agent 类型路由到不同 LLM
- **真实 SSE 流式输出** - OpenAI/Anthropic/Ollama 全适配
- **文本内联批注 + 评论线程** - 选中文本批注与讨论
- **修订模式与变更追踪** - trackInsert/trackDelete + 接受/拒绝
- **古典评点家 Agent** - 金圣叹风格文学点评
- **小说创建向导后端连通** - 4 步引导 + 自动 Ingest
- **记忆压缩师集成** - 场景内容智能压缩
- **SQLite FTS5 全文索引** - BM25 + 向量混合搜索

### v3.1.2 (2026-04-13) - 设置页增强、浏览器开发环境修复与全新应用图标

- **全新羽毛笔品牌图标**
- **模型连接状态指示灯** - 实时检测延迟
- **设置页编辑模型模态框修复** - `custom` 提供商兼容
- **浏览器开发环境兼容** - Vite dev server 模型回退

### v3.1.1 (2026-04-13) - 幕前 Waza 设计重构与 CI 修复

- **幕前界面重构** - OKLCH 颜色系统、LXGW WenKai 字体、精简侧边栏
- **底部 LLM 对话栏** - 悬停显示、流式对话、模型状态灯
- **本地三模型配置** - Gemma-4 / Qwen3.5 / bge-m3
- **Tauri 构建与 CI 修复** - MSI/NSIS 安装包、GitHub Actions Nightly Release

### v3.1.0 (2026-04-13) - 智能记忆与版本管理

- **混合搜索** - BM25 + Vector RRF 融合
- **场景版本历史** - 快照、diff、恢复、统计
- **记忆保留曲线** - 优先级分级、自动归档建议

### v3.0.0 (2026-04-12) - 重大架构调整

- **场景化叙事架构** - Scene 取代 Chapter
- **增强记忆系统** - CJK 分词、Ingest 管线、知识图谱
- **AI 智能生成** - 引导式小说创建
- **工作室配置** - 导入/导出功能

---

## 🚀 快速开始

### 环境要求
- Rust 1.95+
- Node.js 18+ (前端开发)
- SQLite 3

### 开发模式

**快速启动（Windows PowerShell）**:
```powershell
# 一键启动前端和后端
.\start-dev.ps1
```

**手动启动**:
```bash
# 1. 克隆项目
cd v2-rust

# 2. 安装依赖
cd src-frontend && npm install && cd ..

# 3. 终端 1 - 启动前端开发服务器
cd src-frontend && npm run dev

# 4. 终端 2 - 启动 Tauri 应用
cd src-tauri && cargo tauri dev

# 5. 构建发布版本（Windows）
cd src-tauri && cargo tauri build

# 构建产物
# target/release/storyforge.exe          - 独立可执行文件
# target/release/bundle/msi/*.msi        - MSI 安装包
# target/release/bundle/nsis/*-setup.exe - NSIS 安装包
```

**双界面入口**:
- 幕前界面: http://localhost:5173/frontstage.html
- 幕后界面: http://localhost:5173/index.html
- Tauri 应用会自动打开两个窗口，幕前在前，幕后在后

**故障排除**: 参考 [TROUBLESHOOTING.md](TROUBLESHOOTING.md)

### 配置说明

配置文件位置：`~/.config/storyforge/config.json`

```json
{
  "llm": {
    "provider": "openai",
    "api_key": "your-api-key",
    "model": "gpt-4",
    "max_tokens": 4096,
    "temperature": 0.7
  }
}
```

---

## 🛣️ 路线图 (Roadmap)

### 已完成 (v3.4.0) ✅
- [x] **场景化叙事** - 场景取代章节，戏剧冲突驱动
- [x] **记忆系统** - 基于 llm_wiki 的知识图谱
- [x] **AI 智能生成** - 引导式小说创建
- [x] **工作室配置** - 导入/导出功能
- [x] **混合搜索** - BM25 + Vector RRF
- [x] **场景版本历史** - 快照、diff、版本链
- [x] **记忆保留曲线** - 自动归档
- [x] **幕前界面重构** - Waza / OKLCH / LXGW WenKai
- [x] **本地模型配置** - Gemma / Qwen / bge-m3
- [x] **Tauri 构建打包** - MSI / NSIS
- [x] **GitHub Actions CI** - Nightly Release
- [x] **知识图谱可视化** - ReactFlow 交互图谱
- [x] **意图引擎** - Agent 调度
- [x] **修订模式** - 变更追踪
- [x] **场景批注** - 场景级 note/todo/warning/idea
- [x] **MCP 外部服务器** - 工具扩展
- [x] **技能工坊** - 导入/执行/管理
- [x] **创作方法论引擎** - 雪花法/节拍表/英雄之旅/人物深度/高密度世界构建
- [x] **StyleDNA 系统** - 六维风格模型 + 52 经典作家 DNA
- [x] **自适应学习系统** - 反馈→偏好→生成→个性化
- [x] **创作工作流引擎** - 7 阶段全自动闭环
- [x] **拆书功能** - txt/pdf/epub 解析，LLM 分析，一键转故事
- [x] **任务系统** - once/daily/weekly/cron 调度，心跳检测，防重叠执行
- [x] **向量化存储** - 场景/人物 embedding 自动生成并入库

### 已完成 (v5.0.0) ✅
- [x] **创世引擎** - 一键创世：概念→章节→世界观→大纲→角色→场景→伏笔→知识图谱，前后台智能联动

### 已完成 (v3.5.x ~ v4.0.1) ✅
- [x] 前端 UI 接入新方法引擎（方法论选择、StyleDNA 面板、工作流启动）
- [x] 性能优化（大数据量场景）
- [x] 导出模板自定义
- [x] 全面代码审计与空实现修复
- [x] 内存模块 SQLite 持久化

### 已完成 (v6.0.0) ✅
- [x] **Story System 合同驱动** - MASTER_SETTING/Volume/Chapter/Review 四级合同 + SCENE_COMMIT 提交链 + 5 个 Projection Writer
- [x] **三层记忆编排器** - Working/Episodic/Semantic 三层记忆，按任务类型动态分配预算
- [x] **追读力评估系统** - Hook/Coolpoint/Micropayoff/Debt 四维度 + 综合评分与趋势图
- [x] **43 体裁模板库** - 内置 43 个网文体裁模板，含核心基调/节奏策略/反模式/参考数据/典型结构
- [x] **Anti-AI 五维审查** - 词汇/语法/叙事/情感/对话五维度审查，输出综合评分与改进建议
- [x] **类型安全基座** - ts-rs 自动生成 SyncEvent TypeScript 绑定 + 前端穷尽匹配 + IPC 一致性检查脚本
- [x] **Ingest 作业追踪** - `ingest_jobs` 表记录 Ingest 全流程状态，幕前顶栏健康指示器
- [x] **Projection 健康检查** - 解析 `projection_status_json`，逐 Writer 展示投影成功/失败状态
- [x] **功能使用度量** - `feature_usage_logs` 本地记录，Settings 页面 30 天统计柱状图
- [x] **角色悬浮卡片** - 编辑器 hover 角色名显示微型浮卡（状态/外貌/上次出场）
- [x] **体裁模板外部化** - 43 模板从 Markdown 迁移到 JSON，支持用户自定义编辑
- [x] **导出出版前体检** - ExportDialog 4 步流程，可选 Anti-AI 审查后再导出

### 已完成 (v0.7.0) ✅
- [x] **AI 三审 Pipeline** - Refine/Review/Finalize + 后处理（知识库/笔记/角色状态/风格分析）
- [x] **角色动态状态** - 6 字段动态追踪，LLM 自动解析更新，前台可折叠面板
- [x] **用量统计看板** - 全局/单故事 LLM 统计 + 最近 20 条调用明细
- [x] **幕前 Pipeline 指令** - `/` 菜单支持修稿/审稿/定稿直达 Pipeline
- [x] **场景进度看板** - execution_stage 彩色徽章 + 多色进度条

### 短期计划 (v7.1.x)
- [ ] 云端同步
- [ ] 协作写作增强（多人实时编辑 OT 完整实现）
- [ ] 插件市场

### 中期计划 (v7.2.x)
- [ ] WebAssembly 前端
- [ ] 自研小模型
- [ ] 移动端适配

### 长期计划 (v8.0.0)
- [ ] 发布平台集成
- [ ] AI 全自动长篇小说生成

---

## 📚 相关文档

- [架构设计](ARCHITECTURE.md) - 详细架构说明
- [功能清单](docs/FEATURES.md) - 完整功能列表
- [更新日志](CHANGELOG.md) - 版本变更记录
- [项目状态](PROJECT_STATUS.md) - 开发进度
- [Agent 指南](AGENTS.md) - AI 助手开发指南
- [V3 架构计划](docs/plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计

---

## 📄 许可证

MIT License - 详见 [LICENSE](LICENSE)

---

**StoryForge (草苔)** - 让创作更智能 🌿
