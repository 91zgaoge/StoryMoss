# Changelog

All notable changes to StoryMoss (草苔) project will be documented in this file.

## v0.30.7（2026-07-20）

### 修复

- **计划执行失败：Step dependency not found（LLM 在 depends_on 写入上下文名）**：续写小说时弹出 Fatal 诊断卡片（`code: INTERNAL_ERROR`，消息含 `Step step_1 dependency Story Context not found; Step step_2 dependency step_1 not found; ...`），整 plan 链式失败。
  - **根因**：LLM 生成的 ExecutionPlan 在 `depends_on` 中混入了上下文名（如 `"Story Context"`、`"writer"`）而非 plan 内 step_id。`topological_sort`（swarm.rs）已正确跳过非 step_id 依赖，但 `PlanExecutor::execute` 的依赖校验未对齐--遇到非 step_id 依赖直接判 `not found`，导致 step_1 被跳过 -> step_2 依赖 step_1 也 not found -> step_3 链式失败。
  - **Fix（executor.rs）**：依赖校验前收集 `plan_step_ids` 集合，对不在集合中的依赖（非 step_id）跳过校验并 `log::warn`，与 `topological_sort` 行为一致；仅校验真实 step_id 依赖是否已产出。参数引用 `{{step_id}}` 由 `resolve_parameters` 兜底处理缺失键。
  - **Fix（mod.rs）**：Rule 3 强化--明确 `depends_on` MUST ONLY contain step_id values of OTHER steps in this same plan，NEVER put context names / capability names / free text，并举例 `"Story Context"` / `"writer"` 为错误值。
  - 验证：`cargo test --lib` 917 passed（+2：topological_sort 非 step_id 依赖跳过 + 混合依赖排序）；fmt / tsc / architecture_guard 全绿。

## v0.30.6（2026-07-21）

### 修复

- **获取角色失败 dynamic_traits 列 NULL 致 Invalid column type Null**：续写/创世获取角色时弹出 Fatal 诊断卡片（`code: INTERNAL_ERROR`）。
  - **根因**：`characters.dynamic_traits` 列在基础 schema 为 nullable TEXT（无 `NOT NULL`/`DEFAULT`），StoryForge 数据迁移导入的旧角色行该列为 NULL。`get_by_story`/`get_by_id` 用 `row.get::<_, String>(9)` 读非空类型，遇 NULL 即报 `Invalid column type Null at index: 9, name: dynamic_traits`。
  - **修复（双层）**：读取层改读 `Option<String>` 兜底 `"[]"`（NULL 行返回空 `dynamic_traits`）；数据层 V111 迁移回填 `characters.dynamic_traits NULL -> '[]'` 保证一致。
  - 验证：`cargo test --lib` 915 passed（+2：NULL 兜底 + 合法 JSON 解析回归）；fmt/clippy 无新增告警。

## v0.30.5（2026-07-21）

### 修复

- **创世流程严重超时（600s 顶满弹出诊断卡片）**：对照 `creative_workflow.log` 2026-07-20 08:37–08:47 定位 4 个根因并分层修复。
  - **根因**：Agency 创世 5 阶段慢，producer tool_loop 5.5min + writer tool_loop 4.5min（含本地模型连接超时 60s×4 候选=240s）顶满 600s；前端 `Promise.race` 600s 到了先 `llm_cancel_all_generations` 杀掉后端，创世被 CANCELLATION 砍掉无产出 + 僵尸 run 卡死故事续写；writer 在 tool_loop 中盲目 board_read 轮询 7-10 轮，每轮一次 LLM 调用。
  - **Fix 1**：`config/commands.rs` 放开 `smart_execute_total_timeout_secs` / `frontend_timeout_secs` 的 clamp 上限 600->1800（默认仍 600s，本地慢模型可调高）；`GeneralSettings.tsx` 输入框 max 同步到 1800。
  - **Fix 2**：`FrontstageApp.tsx` 创世路径前端超时 = 后端 + 30s 缓冲（主超时 `handleSmartGeneration` + `isGenerating` 看门狗 + 诊断卡片三处统一），保证后端先返回错误并落终态；提取纯函数 `utils/genesisTimeout.ts`（`genesisMainTimeoutSeconds` / `watchdogTimeoutSeconds`）便于单测。
  - **Fix 3（核心）**：`coordinator.rs` 新增 `asset_retrieval_plan`--writer tool_loop 前置单次 LLM 调用，从资产区 catalog 选出本章写作必需的 key（30s 超时包裹，失败兜底全量，`parse_lenient` + `RetrievalPlan` 别名兼容本地模型），消除 writer 多轮 board_read 轮询。
  - **Fix 4**：`coordinator.rs` 新增 `build_writer_assets_context`--检索规划后按 key 过滤资产全文预注入 writer task（8000 字符预算截断），writer 倾向第一轮直接 board_write + final，tool_loop 轮次从 7-10 降到 1-2。
  - **Fix 5**：`tool_loop.rs` 新增 run 级 deadline 感知（`with_deadline` + 每轮检查，剩余 <30s 熔断保产出）；新增 `LoopAbortReason` 枚举（Deadline/ParseFailures/MaxTurns），`circuit_break_reason` 识别 deadline 熔断返回"剩余时间不足"，coordinator writer 路径据此快速失败而非回退 legacy `writer_prose_fallback`（后者单调用也会顶满超时无产出）。
  - **预期效果**：创世总耗时从 600s 顶满降到 ~8-10min 留有余量；超时上限可调；前端不再先杀后端；deadline 保险即使其他修复失效也在硬超时前 30s 熔断保产出。
  - 验证：`cargo test --lib` 913 passed（+14：asset_retrieval_plan 4 + build_writer_assets_context 3 + tool_loop deadline 5 + circuit_break_reason 2）；`npx vitest run` 305 passed（+8：genesisTimeout 纯函数）；tsc / fmt / clippy / format:check / architecture_guard 全绿。

## v0.30.2（2026-07-20）

### 创世稳定性修复：本地模型输出风格兼容
- 修复「管理 Agent 被熔断，资产生产未完成」：本地模型（如 Gemma）输出多动作数组、`type=board_write` 变体、非法 zone（character）导致 ToolLoop 连续解析失败或达轮数上限
- `parse_action` 三层容错：数组解包（单元素解包/多元素取首个并提示）+ name/args 启发式判定
- `board_write` 非法 zone 按 item_type 收编（character/world/outline → asset 并注明）
- 创世三个单调用启用 `response_format: JsonObject`（本地 vLLM/Ollama 均支持），概念/深度资产解析带键名别名
- 熔断错误信息带原因（连续解析失败/达到最大轮数）；producer 任务提示统一格式约束；JSON 调用 token 计量入账

## v0.30.4（2026-07-20）

### 功能

- **幕前输入历史持久化（按故事隔离）**：幕前底部输入框已输入内容现长久保留，关闭窗口/重启后不丢失，与编码工具一致。
  - 每条提交按故事 ID 隔离存入 `localStorage`（`frontstage:inputHistory:<storyId>`），最近 20 条，切换故事自动加载该故事的历史。
  - 保留既有 ghost-hint UX：↑/↓ 切换 LLM 建议 <-> 历史记录，-> 确认填充；持久化对导航交互无侵入。
  - localStorage 不可用（隐私模式/配额超限）时静默降级为内存态，不影响写作。
  - 实现位置：`src-frontend/src/frontstage/FrontstageApp.tsx`（模块级 `loadInputHistory`/`saveInputHistory` + `useEffect` 加载 + `handleInputSubmit` 同步持久化）。
  - 验证：`npx vitest run` 297 passed（+2：持久化写入 + 重载召回）。

## v0.30.3（2026-07-20）

### 功能

- **StoryForge 数据自动迁移**：首次启动 StoryMoss 时，若检测到旧版 StoryForge 数据目录存在，会在数据库初始化之前自动导入全部配置与数据，避免品牌更名造成的数据丢失。
  - 迁移范围：文件树复制、`cinema_ai.db` SQLite 合并（按共有列合并，自动跳过目标库不存在的旧表）、`config.json` 递归对象合并；StoryMoss 已存在的文件、记录与配置键不会被覆盖。
  - 安全策略：迁移前对 StoryMoss 当前数据做复制式备份，旧 StoryForge 目录完整保留，迁移完成后写入 `.storyforge_migrated` 标记避免重复执行；迁移在 `init_db` 之前完成，无需重启即可使用导入数据。
  - 实现位置：后端 `src-tauri/src/migration/storyforge.rs`，由 `src-tauri/src/lib.rs` 的 `setup` 钩子调用。

### 修复

- **创世主创 Agent 熔断（本地模型 JSON 不遵从）**：本地模型（Qwen/Gemma）对 `producer_depth_assets` 的 `complete_json` 返回散文而非 JSON -> 快速路径失败回退 legacy -> legacy writer tool_loop 要求 JSON action 而模型写散文 -> 连续 3 轮解析失败熔断，首章未完成。三级修复：
  - **Fix A（主修复）**：`producer_depth_assets` 在 `parse_lenient` 失败时兜底 salvage 散文（>50 字符）为 world 资产，快速路径继续，避免回退 legacy。
  - **Fix B（可诊断性）**：`tool_loop.rs` 此前零条日志，解析失败的原始响应只存在内存里 run 结束即丢弃，"详见 run 日志"为误导。现每轮解析失败 + 熔断点 + max-turns 均 `log::warn!`（含 role、轮次、截断 raw 500 字）。
  - **Fix C（纵深防御）**：legacy writer "连续解析失败"熔断时回退一次自由体散文单调用（新 `writer_prose_fallback`：读黑板资产 -> `complete()` -> 写 draft），"达到最大轮数"仍直接 Err。
  - 验证：`cargo test --lib` 899 passed（+2 新测试：`test_depth_assets_prose_salvaged` / `test_legacy_writer_prose_fallback`）。

- **StoryForge 迁移健壮性增强**：
  - 迁移失败时写入 `.storyforge_migration_failed` 标记，避免每次启动都重试并堆积备份。
  - 数据库合并期间临时关闭外键检查，避免子表先于父表插入导致失败。
  - 旧数据检测改为只要 `com.storyforge.app` 目录非空即触发迁移，不再硬依赖 `cinema_ai.db` / `config.json`。

- **Windows 启动闪退（BEX64 / 0xc0000409）**：移除 `init_windows` 中 setup 阶段对 `CoreWebView2` 的 unsafe COM 调用，该调用在部分 Windows/WebView2 环境下会在启动时触发缓冲区溢出异常；Windows 右键菜单禁用功能迁移到前端全局 `contextmenu` 事件处理，保留输入框/文本域的原生菜单。
- **CI Rust 格式检查失败（v0.30.2 熔断修复代码未格式化）**：对 `agency/coordinator.rs`（`circuit_break_reason`）、`tool_loop.rs`、`tools.rs` 的熔断修复代码补做 `cargo +nightly fmt`，修复 `rust-check` 三平台 fmt 失败。

### 基础设施

- **自动更新源迁移到官网**：应用内 updater 主端点改为 `https://storymoss.top/releases/latest.json`，GitHub Releases 保留为回退源；CI 构建完成后自动通过 FTP 将签名产物同步到 `storymoss.top/releases/`；落地页下载按钮同步指向官网源。
- **CI FTP 主机解析兼容 URL 格式**：`.github/scripts/upload-releases-ftp.mjs` 现在支持 `FTP_HOST` 为 `ftp://host:port` 形式，优先使用 URL 中的端口，同时保留显式 `FTP_PORT` 的覆盖能力，避免 `ftp://` 前缀被错误地当作主机名解析。
- **官网 latest.json 使用官网下载源**：上传脚本在把 `latest.json` 同步到 `storymoss.top` 前，会将其中的二进制下载 URL 从 GitHub Releases 重写为 `https://storymoss.top/releases/<filename>`；GitHub Releases 上的原始 `latest.json` 仍保留为回退端点。
- **Pre-commit 格式守卫**：仓库内置 `.githooks/pre-commit`，提交前自动检查本次 staged 的 Rust（`cargo +nightly fmt -- --check`）与前端（`prettier --check`）代码是否已格式化，未格式化则拒绝提交，对齐 CI 的 fmt 检查；首次克隆后执行 `git config core.hooksPath .githooks` 启用。

## v0.30.1（2026-07-19）

### 创世提速（Genesis Fastpath）
- 创世从 12-18 次串行 LLM 调用压缩为 4 次（概念包 → 主创首章 ∥ 管理深度资产 → 编辑质量门），典型远程模型首章 ≤3 分钟
- 主创模型优先：多模型时管理/编辑不再与主创同模型（Tool 档排除 active/creative）；单模型时主创先出首章，资产与审查随后
- 单调用解析失败自动回退原串行多轮流程；取消信号不再误入回退路径
- smart_execute 超时回退统一为 600s（原配置加载失败时回退 180s）

## v0.30.0（2026-07-19）

### Agency P5：持续学习 + 代理可视化（框架收官）
- 持续学习双轨：观察层（observations.jsonl，10MB 轮转，防自观察）→ 后台 analyzer（Background 档）→ instinct（trigger/action/confidence 文件层）
- 置信度引擎：按证据初始化 + 采纳 +0.05 / 纠正 −0.1 / 周衰减 −0.02 / prune
- 晋升管线：≥0.8 且跨 story 复现 → 学习中心确认 → 物化为 skill.yaml 技能（重启自动 reload）
- 学习中心页：模式列表 + 置信度 + 晋升提案 + 观察流 + 手动分析
- 代理工作室页：三角色实时状态卡 + 黑板视图（事件驱动刷新）+ 活动时间线
- eval 场景纳入 CI 专用门禁 step；检查点对比 UI；story 级 token 聚合；追读力口径统一

### 功能

- **评估仪表盘 story 级 token 聚合**：`agency_eval_overview` 新增 `story_tokens` 段，每 run 取 `MAX(tokens_used)`（累计快照去重）再跨 run 求和，前端 `AgencyEval` 页同步展示；新增对应单测。
- **rule grader 追读力对齐生产口径**：`reading_power_score_of` 的 coolpoint/micropayoff 归一化由 `min(count,3)/3.0` 改为每命中 +0.1（coolpoint cap 0.8、micropayoff cap 0.4），与 `reading_power/mod.rs` 生产实现一致。

### 修复

- **rusqlite 启用 `unlock_notify` feature**：根治 shared-cache 内存库跨连接 SQLITE_LOCKED（busy_timeout 不兜底）导致的两个测试 flake（test_batch_revision_no_cross_chapter_mixup / test_genesis_cancel_not_overwritten_by_completed）。
- **CI 格式检查失败**：对 P4 resume 与 P5 学习/工作室代码做 `cargo +nightly fmt` / `prettier` 全量格式化，修复 `rust-check` 与 `frontend-check` 的 fmt 失败。

### 基础设施

- **CI 新增 agency eval 场景测试**：`build.yml` 在非 Windows runner 上新增 `cargo test --lib agency::eval_harness` 步骤，固化 pass@k/pass^k 与 baseline 回归门。

## v0.29.0（2026-07-19）

### Agency P4：验证循环
- 四级 grader：code（字数/自重复/合同禁则）→ rule（合同兑现/追读力/规则复检）→ model（rubric 化编辑裁决 1-5 须引证据）→ human（用户修改率后置信号）
- Gate v2 统一加权评分（0.2/0.3/0.5，阈值 0.75）取代二元判定
- V110 检查点：里程碑指标快照 + 现在 vs 当时对比（IPC）
- eval harness：JSON 场景 + pass@k/pass^k + baseline 回归门（随 cargo test 纳入 CI）
- 评估仪表盘前端页（通过率/加权分趋势/判定历史/角色 token 用量）
- migration runner 按最高版本选目（修复陈旧副本遮蔽）；resume 改 spawn 模式

## v0.28.0（2026-07-17）

### Agency P3：代币优化 + 记忆持久性
- 角色×任务模型路由：主创 Creative / 管理 Tool / 编辑 Background（经 ModelRole 体系，用户可按角色指派模型）
- 全局 agency LLM 并发闸门（跨 run 上限 3）+ request_id RAII 注册
- 上下文注入 token 预算（tiktoken 计数截断）+ 黑板三档目录（catalog/summary/full）+ ToolLoop 会话窗口
- agency_sessions 会话快照（机械提取 + Background 档五段摘要双层）
- 跨会话恢复 agency_resume_run（黑板复制 + stale-replay 防护 + .storymoss sessions/ 归档）
- 同 story 并发 run 原子护栏（部分唯一索引）；创作角色落库去重；质量门判定轮次可追溯
- 清理 T8 遗留的创世专属死代码

## v0.27.0（2026-07-17）

### Agency 多代理创作框架（创世 2.0）P1+P2
- 新增 agency 模块：黑板协作 + ReAct 工具循环 + 三角色（主创/管理/编辑审计）
- 质量门：编辑裁决 + 规则复检 + 至多 1 轮修订，未过门不装配
- 并行稳态循环：编辑审 N 与主创写 N+1 并发；按角色并发预算与 run 级 token 预算
- request_id 定点取消（不再全局取消）；续写循环 agency_continue_chapter/batch
- 创作资产自动落库（characters/world_buildings/story_outlines）
- smart_execute 创世路径切换到 agency；旧 GenesisPipeline 移除（TriShot 续写保留）

## [v0.26.59] - StoryForge → StoryMoss 品牌收尾，官网落地页上线（2026-07-11）

### 功能

- **官网落地页上线**：`landing/` 独立站点部署到 `https://ai.91z.net`，面向新用户展示产品核心卖点并引导下载桌面版。技术栈为 React 18 + Vite 6 + Tailwind CSS 3 + Framer Motion，包含 Hero、痛点区、幕后/幕前双空间、Genesis 创世流程、分时介入架构、功能长卷、下载 CTA 与页脚，支持 `prefers-reduced-motion` 与移动端响应式布局。
- **平台感知下载按钮**：落地页根据访问者操作系统自动提供 macOS / Windows / Linux 对应安装包下载链接，点击直接触发下载。

### 重构 / 品牌

- 完成 StoryForge → StoryMoss 全局重命名：所有文档、GitHub 仓库元数据、Release 标题、CI 配置与 Tauri 产物名统一为 StoryMoss。

### 验证

- `npx vitest run`（landing）：19 passed ✅

## [v0.26.58] - 修复 OpenAI/Deepseek 模型因 top_p=0 健康检测失败（2026-07-09）

### 修复

- **根因**：本地配置中 Deepseek 等 OpenAI 兼容模型的 `top_p` 被设为 `0.0`，而 OpenAI API 要求 `top_p ∈ (0, 1.0]`，导致健康探测/生成请求直接返回 `Invalid top_p value`。
- **修复**：在 `OpenAiAdapter` 发送请求前对 `top_p` 做范围过滤，≤0 或 >1 的值不会被序列化，让服务端使用默认值；同步/流式请求均生效。
- **测试**：新增 `llm::openai::tests::sanitize_top_p_keeps_valid_values`，覆盖 `None`、负数、`0`、越界与合法值。

### 验证

- `cargo test --lib`：**770 passed**（+1）
- `cargo +nightly fmt --check` ✅

## [v0.26.57] - 自动划分章节、本地导出保存与提示词目录（2026-07-09）

### 功能

- **自动划分章节**：在「后台设置 → 通用」新增「划分章节方式」，支持 `word_count`（按字数）与 `plot`（按情节）。按字数时可输入单章字数上限，留空/0 则使用默认 3000 字（中文「字」）。场景内容保存空闲约 30 秒后，仅对故事最新一章自动划分，避免中间章节改写时重排后续章号。
  - 后端实现：`src-tauri/src/story_system/chapter_splitter.rs`，含 `ChapterSplitMode`、`find_split_offset`、`plan_split`、`maybe_split_latest_chapter`。
  - 触发点：`SceneService::update_scene` 的 `auto_commit` 防抖窗口内，内容变更后先尝试分章再提交。
- **导出功能可用性加固**：导出结果通过系统原生保存对话框让用户选择本地目录，文本格式（txt/md/html/json）直接写 UTF-8，二进制格式（pdf/epub）从后端临时文件复制字节流；用户取消对话框时不关闭导出弹窗。
  - 前端：`useExport.ts` 新增 `saveExportViaDialog`；`ExportDialog.tsx` 主按钮改为「导出」，可选 Anti-AI 体检下沉为次要入口。
  - 后端：`export_story` 使用 `assemble_export_chapters` 以 `scenes.content` 为真相源聚合章节内容，兼容孤儿场景。
- **提示词本地目录**：「后台 → 提示词注册表」新增「打开目录」按钮，调用 `open_prompts_directory` 用系统文件管理器直接打开 bundled prompts 资源目录；编辑器从 Monaco 改为原生 textarea，避免 CSP 拦截 CDN 导致永久 Loading。

### 修复

- 修复导出章节内容为空时未从关联 scenes 聚合的问题；现在优先使用场景内容，无场景时才回退 `chapters.content`。
- 修复 pdf/epub 二进制导出时被当作文本读取导致前端拿到乱码或失败的问题。

### 验证

- `cargo test --lib`：chapter_splitter 7 passed、export::assemble 8 passed、prompts::registry 15 passed，全量 769 passed。
- `npx vitest run`：`useExport.test.ts` 4 passed、`PromptsPanel.test.tsx` 5 passed，全量 292 passed。
- `npx tsc --noEmit` ✅；`npm run format:check` ✅；`cargo +nightly fmt --check` ✅。

## [v0.26.56] - 网关契约测试串行化（2026-07-09）

### 修复

- **根因**：`tauri::test::mock_app` 共享 `app_data_dir`，并行跑写 config 的 demotion/disable 契约会互相覆盖，偶发 `creative_x_overrides` 读到别的测试的 `keep`。
- **修复**：executor 测试模块加 `mock_app_config_lock`，写 config 的契约串行。

### 验证

- `cargo test --lib -- --test-threads=8`：creative_x / demoted_degraded / sticky_unhealthy / disabled_model 通过

## [v0.26.55] - 幕后模型列表开启/关闭开关（2026-07-09）

### 功能

- **UI**：模型管理列表每张卡片新增「开启/关闭」开关（中文标签），无需进入编辑弹窗。
- **行为**：关闭后走 v0.26.54 已落地的 fail-closed 路径——不调用、不探测、不进网关状态；若关闭当前创作/活跃模型，活跃自动回退到其他已启用模型，角色指针清空。
- **连接轮询**：列表页只轮询 `enabled` 模型；禁用模型不发起连接检测。
- **加固**：`is_promotable_user_model` 要求模型仍在网关注册表（禁用后 refresh 移出），避免残留健康快照被置顶。

### 验证

- `cargo test --lib`：`apply_disable_side_effects` / `disabled_model_excluded` / `test_probe_model_rejects` / `test_disabled_model_not_selected` / `is_promotable` 相关
- `npx vitest run`：`ModelCard.enabled.test.tsx`
- `npx tsc --noEmit` / `architecture_guard` / fmt

## [v0.26.54] - 修复创作模型粘性降级绕过与禁用模型 fail-closed（2026-07-09）

### 修复

- **根因 A（executed）**：用户已将「创作模型」设为 Deepseek，且 `creative_model_id`/`active_llm_profile` 已持久化；但网关在 Deepseek 连续失败后粘性 demotion，`resolve_role_model` 用失败阈值丢弃显式创作角色，`generate()` 再提升仍要求 `is_model_available`（拒绝 Unknown），Call3 候选链首位长期落在本地 MN-Oblivion。
- **修复 A**：显式角色模型不受粘性 demotion 拦截；粘性 Unhealthy 在 resolve 时清一次→Unknown 再探；`set_active_model`/`save_settings` 调用 `clear_model_demotion`；`generate()` 再提升对齐 `is_promotable_user_model`。
- **根因 B**：`update_model` 漏写 `enabled`；禁用后仍探测且活跃/角色指针可留在已禁用模型上。
- **修复 B**：持久化 `enabled`；禁用时 `apply_disable_side_effects`（活跃自动回退、角色清空）+ 跳过探测；keepalive/retry 只保活 enabled 模型。

### 验证

- `cargo test --lib`：`clear_demotion` / `demoted_degraded_creative` / `auto_clears_sticky_unhealthy` / `user_sets_creative_x` / `sync_creative_to_active` / `apply_disable_side_effects` 相关 passed
- `architecture_guard` ✅

## [v0.26.53] - 故事名取消单击回幕后（双击改名可用）（2026-07-09）

### 修复

- **根因**：故事名单击打开幕后，与双击改名冲突，用户无法可靠进入改名。
- **修复**：故事名仅双击改名；回幕后改走右侧设置按钮（禅模式也保留该按钮）。章节标题本就无单击导航。

### 验证

- `FrontstageHeader` / `EditableChapterTitle` vitest：单击故事名不调用 `onOpenBackstage`；设置按钮可回幕后；双击仍进编辑。

## [v0.26.52] - 修复模型新增与默认创作模型即时生效（2026-07-09）

### 修复

- **根因 1（幕前连接状态不刷新）**：底部信号条读 `gateway-status`，但 `model_config`/`app_settings` 刷新只失效 `settings`/`models`；且 `get_gateway_status` 过滤掉 `Unknown`，新模型探测完成前不可见。
- **根因 2（默认创作模型不生效）**：用户指定的创作模型在健康态为 `Unknown`/无快照时被 `is_model_available` 拒绝，网关回退旧 `active_llm_profile`；`set_active_model(role=creative)` 未同步聊天活跃模型。
- **修复**：`useSyncStore` / 幕前 `dataRefresh` 同步失效 `gateway-status`；状态栏展示含 `Unknown`；用户显式角色/活跃模型用 `is_promotable_user_model`（允许 Unknown）；设创作模型时 `sync_creative_to_active_llm`；`delete_model` 补齐 `emit_data_refresh`。

### 验证

- `cargo test --lib`：`include_in_gateway_status` / `test_promotable_*` / `sync_creative_to_active_llm` 4 passed
- `npx vitest run useSyncStore.bug.spec.ts` 5 passed（含 gateway-status 失效）
- `npx tsc --noEmit` ✅；`cargo +nightly fmt` ✅；`architecture_guard` ✅

## [v0.26.51] - 幕前故事名与章节名内联改名（2026-07-09）

### 功能

- **故事名**：空编辑器顶部显示「草苔」；粘贴/输入正文后显示「未命名」并自动创建故事+第一章场景（不走 `selectStory`，避免清空编辑器）；双击内联改名（Enter/blur 保存，Esc/空 blur 取消）。回幕后走设置按钮。
- **章节名**：编辑器上方大标题与顶栏状态章节标签统一双击改名；空标题展示 `第N章`；持久化优先 `update_scene`（title 回写关联 chapter）。

### 验证

- `displayStoryTitle` / `displayChapterTitle` / `FrontstageHeader` / `EditableChapterTitle` vitest 30 passed
- `npx tsc --noEmit` ✅；`npm run format:check` ✅；`architecture_guard` ✅

## [v0.26.50] - 修复打字触发后台运行与深度思考假超时（2026-07-09）

### 修复

- **根因**（用户日志 2026-07-09 07:25–07:42）：幕前打字自动保存立刻 spawn AutoIngest LLM，与续写抢本地模型；`contract-auto-progress` 仍把 `isGenerating` 拉高；后台活动同步会单独启用输入栏「后台运行」；`smart_execute` 挂死后「深度思考」可超 650s 仍不弹诊断。
- **AutoIngest 防抖**：`SceneIngestor::spawn_ingest_debounced`（30s，与 auto_commit 同窗）+ `BACKGROUND_LLM_SEMAPHORE`；`update_scene` 改走防抖路径。
- **合同补齐静默**：`AutoContractBuilder` 不再 emit `contract-auto-progress`；前端 listener 忽略 running 阶段。
- **活动同步**：`backendActivityStore` 活跃不得单独 `setIsGenerating(true)`；用户入口仍显式置位。
- **看门狗**：`isGenerating` 超过 `frontend_timeout_secs` 强制结束并弹出诊断卡片。

### 验证

- `cargo test --lib story_system::scene_service::tests` 6 passed
- `npx vitest run useBackendActivityListener.contract.test.ts` 2 passed
- `cargo check --lib` ✅

## [v0.26.49] - 修复续写与正文脱节（末句硬锚点）（2026-07-09）

### 修复

- **根因**（`creative_workflow.log` 2026-07-09 05:48）：续写 Call3 提示词中 WriteTimeBundle 含「开场建立处境」等开篇指令，前文回顾夹在中段（Lost-in-the-Middle），模型另起「黑暗中荧幕」开篇，与章末「继续前进找线索」完全脱节。
- **修复**：`build_ending_anchor` 提取正文末 2 句，追加到 TriShot Call3 / TimeSliced prompt **最末尾**（输出纪律之后），声明最高优先级并覆盖开场指令；`trishot.call3.ending_anchor` 可观测。

### 验证

- `cargo test --lib agents::orchestrator::tests::test_last_n_sentences` / `test_build_ending_anchor` / `test_ending_anchor` 3 passed
- `cargo +nightly fmt -- --check` ✅

## [v0.26.48] - 修复自动更新：GitHub Releases + latest.json（2026-07-09）

### 修复

- **根因**：`bundle.createUpdaterArtifacts` 未开启，Release 只有 `.dmg/.msi/.deb`，无 `latest.json` 与 `.sig`，应用内「检查更新」拉取 `releases/latest/download/latest.json` 恒 404。
- **配置**：`tauri.conf.json` 开启 `createUpdaterArtifacts: true`；bundle 增加 `appimage`；Windows updater `installMode: passive`。
- **CI**：Linux 构建 `deb,appimage`；macOS 显式 `app,dmg`；上传 `.sig` / `.app.tar.gz` / AppImage；tag 后 `verify-updater-manifest` 门禁确认 `latest.json` 存在。
- **运行时**：下载进度按 chunk 累加；404/清单缺失时给出可操作的 GitHub 错误提示。

### 验证

- `cargo test --lib updater::` 2 passed
- `cargo +nightly fmt -- --check` ✅

## [v0.26.47] - CI 热修复：Rust 格式化（2026-07-09）

### 修复

- v0.26.46 功能代码未跑 `cargo +nightly fmt`，导致 macOS/Ubuntu/Windows `rust-check` 失败。本版仅格式化，无逻辑变更。

## [v0.26.46] - 创世方法论全链路、题材画像 match-or-create 与拆书持久化（2026-07-09）

### 修复

- **Background 方法论静默断链**：v0.26.28 外部化后 5 个 `narrative_*_generate` 模板丢失 `strategy_section` / `quartet_section` 占位符，代码仍传 `strategy_notes` 但运行时未注入。已恢复占位符并新增契约测试。
- **方法论 ID 分裂**：`world_building` / `hdwb` 别名统一为 `high_density_world_building`；Selector / WriteTimeBundle / 设置 UI 对齐。

### 增强

- **Genesis 方法论注入**：`build_strategy_notes` 按 quick/background 步骤注入；ContractSeeding 后推进 `methodology_step`（雪花→4、HDWB→2）；注入规模可观测日志。
- **题材画像 match-or-create**：quick phase 新增 `EnsureGenreProfileStep`——目录匹配则复用，无匹配则 LLM 生成并入库（用户创建 `is_builtin=false`）；概念提示强化题材保真。
- **拆书 Phase A + D0**：StoryArc→大纲、作者字段、伏笔持久化；分块 12h 墙钟上限、短篇禁止单 Full chunk、并发止血；`StoryArcView` 与按 `book_id` 过滤进度。

### 文档

- 方法论/拆书审计与 remediation 计划；`sf-architecture-contract` / `sf-genesis-campaign` 技能更新。

### 验证

- `cargo test --lib` 方法论/Genesis/prompt 契约相关 20+ passed
- `cargo check --lib` / fmt / architecture_guard 通过

## [v0.26.45] - Genesis 人物卡强制落地（姓名 + 欲望/阻力）（2026-07-09）

### 增强

- **开篇人物卡**：合并骨架∪概念为 `ProtagonistCard`，双重注入 `first_scene` Critical 位与 TriShot Call3 尾注（`NOVEL_OUTPUT_DISCIPLINE` 之前）。
- **双目标落地**：人物辨识度（真名，禁「主角」）+ 冲突与目标清晰（本场欲望/阻力纪律）。
- **规则探针**：`name_hit` / `desire_hit` / `obstacle_hit`；与 8% 自重复共享「最多一次额外 Call3」软重试；fail-open。
- **零新增 LLM**：不增加 quick 墙钟预算。

### 验证

- `cargo test --lib narrative::` 61 passed（含 protagonist_card 6）
- `cargo test --lib` 全绿；architecture_guard / vitest / format 通过

## [v0.26.44] - Genesis 首章质量：开篇骨架与提示词加厚（2026-07-09）

### 增强

- **开篇骨架步**：quick_phase 变为「概念 → 策略 → 铺设开篇骨架 → 撰写开篇」四步；骨架硬超时 10s，失败则从加厚概念字段规则映射，永不阻断开篇。
- **概念提示加厚**：`narrative_story_concept_generate` 增加主角/冲突/世界一句话/生存代价/`genre_profile_ids`；概念步 max_tokens 下限 768。
- **策略选择中文化**：`strategy_selector` 改为中文规则，末世类优先 `apocalyptic`。
- **叙事四元组接入创世**：策略选择后调用 `infer_narrative_quartet`（纯启发式，不调 LLM）。
- **占位角色去硬编码**：TriShot preflight 使用骨架/概念主角名与目标，不再写死「在异星末世中生存」。
- **输出纪律单源**：`narrative_first_scene_generate` 去掉与 Call3 `NOVEL_OUTPUT_DISCIPLINE` 重复的纪律段；强调戏剧槽位非空时必须落地。

### 文档

- `USER_GUIDE`：创世延迟改为 30–90s，并说明先铺开篇骨架再写正文。

### 验证

- `cargo test --lib narrative::genesis`：12 passed（+1 骨架解析契约）
- `cargo test --lib extract_story_meta`：2 passed
- 全量验证见发布记录

## [v0.26.43] - 修复底部状态栏 emoji 显示为方框（2026-07-09）

### 修复

- **「准备上下文」前出现 □□**：`getMajorPhase` 把 emoji（📂 等）写入 `generationStatus`，Tauri WebView 缺彩色 emoji 字体时显示为方框；底部栏未接入已有的 `StatusIcon`（Lucide SVG）。
- **状态解析把中文拆碎**：旧正则非贪婪 `.+?` 把「准备上下文」拆成单字，emoji 代理对再拆成残缺字符。
- **修复**：阶段文案改为纯文本；`FrontstageBottomBar` 用 `StatusIcon` 渲染；解析前先剥 emoji，再提取尾部 `(Ns)`。

### 验证

- vitest：266 passed / 3 skipped（StatusIcon + BottomBar +4）
- `npx tsc --noEmit` / `npm run format:check` / `architecture_guard`：✅

## [v0.26.42] - 修复续写 Tab 提示可见但无幽灵文本（2026-07-09）

### 修复

- **续写无幽灵文本 / Tab 确认后无追加**：Tab 接受后 `hideGhostUntil`（30s）与 `postAcceptHideUntilRef` 未在新续写开始时清零，导致 `shouldShowGhostTree` 仍渲染 Tab 提示条，但 `shouldShowGhostParagraph` 被压住——用户只见「按 Tab 接受」、不见续写正文，确认后也无内容可追加。
- **根因**（`creative_workflow.log` 2026-07-09）：`handleSmartGeneration` 只清 `postAcceptLock`，未清 `hideGhostUntil`；RichTextEditor 新 `generatedText` 到达时只移除 `force-hide-ghost` 类，未清本地时间锁。
- **修复**：新续写入口与 `setGeneratedText` 非空写入时清零 `hideGhostUntil`；RichTextEditor 在新幽灵内容到达（非接受中）时清零 `postAcceptHideUntilRef`；父级 `hideGhostUntil===0` 时同步清本地锁。

### 验证

- vitest `RichTextEditor.duplicate.test.tsx`：6 passed（+1 回归：接受后 30s 内新续写须显示幽灵段落）
- `npx vitest run`：262 passed / 3 skipped
- `npx tsc --noEmit` / `npm run format:check` / `architecture_guard`：✅

## [v0.26.41] - 记忆统一读模型与 Finalize scene_id 根治（2026-07-09）

### 修复

- **run_finalize 写错场景**：管线贯穿 `scene_id`（drafts 列 + IPC + SceneEditor）；定稿直写编辑中的场景，不再 `chapter_number → scenes.first()`。旧草稿无 scene_id 时回退旧逻辑。

### 增强

- **记忆统一读模型（非破坏）**：V105 `story_memory_facts` VIEW（kg_entities ∪ memory_items）；V106 `memory_items.kg_entity_id` 链接与按名回填；`MemoryFacade::list_unified_facts`；IPC `get_story_memory_facts`；MemoryTab 显示 KG/记忆徽章。物理表不合并、不 DROP。

### 验证

- `cargo test --lib`：701 passed / 2 ignored
- finalize:: 3；memory::facade 7
- vitest 261；tsc / format / architecture_guard / verify-ipc-manifest：✅

## [v0.26.40] - 幕后资产闭环 P0–P3（2026-07-09）

### 增强

- **P0 认知对齐**：侧栏每项「热/温/冷/配」影响徽章；故事合同 / 知识图谱页注明是否进入默认生成。
- **P1a 场景管线统一**：PipelinePanel 收进 SceneEditor 可折叠「管线」轨；编辑态不再独立第三列。
- **P1b KG→Bundle**：`related_entity_summaries`（top-5）进 WriteTimeBundle / `【相关设定】`；TriShot AssetManifest one-liner。
- **P1c MCP 降级**：侧栏移除「扩展连接」；设置新增「扩展」Tab；`mcp` 视图重定向。
- **P2 MemoryFacade**：KG 摘要与 Full MemoryPack 统一读入口；表不合并；quality_gate 明确永不热路径 LLM。
- **P3 诊断瘦身**：侧栏「诊断」组默认折叠；生成链路展示资产→prompt 覆盖率条。

### 验证

- `cargo test --lib memory::facade`：5 passed；`to_prompt_includes_related`：passed
- 相关 vitest（Sidebar IA / Pipeline / PromptCoverage）：15 passed
- 版本四源：0.26.40

## [v0.26.39] - 幕后信息架构全面重排（2026-07-09）

### 增强

- **侧栏五组分类**：创作 / 故事资产 / 创作工具 / 洞察与运维 / 系统；中文统一命名（故事合同、扩展连接、数据洞察、意图诊断）；CTA「打开幕前写作」。
- **数据洞察合并**：用量 + 写作 + 功能使用三 Tab；`writing-stats` 重定向兼容。
- **设置七 Tab**：模型（管理/路由/健康）| Agent | 写作 | 提示词 | 外观 | 关于 | 账号；拆掉「通用设置」大杂烩。
- **拆书设置就近**：并发配置迁至拆书页折叠区。
- **账号死链修复**：UserMenu「账号设置」跳转设置·账号 Tab。

### 验证

- `npx vitest run`：249 passed / 3 skipped ✅
- `npx tsc --noEmit` / format：✅

## [v0.26.38] - 提示词面板修复与组合智能化（2026-07-09）

### 修复

- **展开提示词永久 Loading**：去掉 `@monaco-editor/react`（CDN 被 CSP `script-src 'self'` 拦截），改用原生 textarea，展开即可编辑。
- **「打开目录」失败**：新增后端 `open_prompts_directory`，用系统 `open`/`explorer`/`xdg-open` 打开资源目录，不再依赖 `shell.open` 本地路径。
- **导出不可用**：改为 dialog `save` + fs `writeFile`；支持「导出已覆盖」与「导出完整包」，并补充说明文案。

### 增强

- **Call 1 框架选择接通**：`render_selected_asset_guidance` 消费 `FrameworkSelections.methodology` 与 `contextual_injectors`（0 额外 LLM），与既有 `prompt_hints`/资产正文一并回灌 Call 3。
- **场景组合预览**：新增 `preview_prompt_composition` IPC + 面板只读分层列表，可跳转到对应提示词。
- **IPC 校验**：`verify-ipc-manifest.py` 跟随 `include!("handlers.rs")`；注册 `write_frontend_log`。

### 验证

- `cargo test --lib`：690 passed / 0 failed / 2 ignored ✅
- `npx vitest run`：244 passed / 3 skipped ✅
- `npx tsc --noEmit` / fmt / format / architecture_guard：✅

## [v0.26.37] - 修复幕前「保存中」常亮与字数不更新（2026-07-09）

### 修复

- **自动保存 IPC 参数错误**：幕前 `update_scene` 误传 `{ id, title, content, word_count }`，后端期望 `{ scene_id, updates }`，导致自动保存静默失败、「保存中...」永不消失。统一为 `buildUpdateSceneIpcArgs`。
- **AI 追加后字数/保存脱节**：`appendAiContent` 只改正文与 store（`isSaved=false`），不更新顶部 `wordCount`、不调度自动保存；现追加后立即刷新字数并 `scheduleAutoSave`。
- **stories API**：`updateScene` 改为嵌套 `updates` 参数，与后端签名对齐。

### 验证

- `npx vitest run`：242 passed / 3 skipped ✅
- `npx tsc --noEmit` / `npm run format:check`：✅

## [v0.26.36] - 后台配置变更即时生效（超时/字体/主题热同步）（2026-07-09）

### 修复

- **超时配置热生效**：`save_settings` 后立即 `LlmService::reload_config()` + `GatewayExecutor::refresh_registry()`，并广播 `app_settings`；幕前/幕后 TanStack Query 立刻失效旧超时。
- **首字节超时接线**：`llm_first_chunk_timeout_secs` 传入 OpenAI/Ollama/Anthropic 适配器，不再硬编码 60s。
- **TriShot 预算读真实配置**：`AppConfig::load` 替代无效的 `try_state`，尊重用户 `smart_execute_total_timeout_secs`。
- **Writer 系统提示覆盖**：从 `LlmService` 内存配置读取 `writer_system_prompt_override`。
- **字体跨窗口同步**：`editor-config-changed` Tauri 事件 + `storage` 双通道，幕前字号/字体即时更新。
- **色调主题跨窗口同步**：`color-theme-changed` 事件；幕后 GeneralSettings ↔ 幕前 ColorThemeDot 双向即时生效。

### 验证

- `cargo test --lib`：685 passed / 0 failed / 2 ignored ✅
- `npx vitest run`：240 passed / 3 skipped ✅
- `npx tsc --noEmit` / fmt / format / architecture_guard：✅

## [v0.26.35] - 全面落地幕后工作室审计残留 R1–R11（2026-07-09）

### 修复

- **R1 仪表盘「场景」口径**：`list_stories` 返回 `StoryListItem`（含真实 `scene_count`）；Dashboard 聚合改用 `scene_count`，不再误用 `chapter_count`。
- **R2 快速创作路径**：`CreationPathGuide.onQuick` 绑定 `runCreationWorkflow`（Stories 直接触发；Dashboard 经 `pendingQuickCreate` 跳转 Stories）；`App.tsx` 导航统一走 `appStore.currentView`，消除壳层 local state 与页面 store 失步。
- **R3 Wizard 半闭环**：新增后端 `apply_wizard_to_story`（角色按名去重、首场景更新/创建、KG 摄取）；前端 `applyWizardToStory` 改为单 IPC。
- **R4 幕后 genesis-warnings**：`App.tsx` 监听并 toast；`GenesisPanel` 收到事件后刷新 runs。
- **R5/R6 场景序号语义**：PipelinePanel / SceneEditor 标注为「场景 #N（管线序号）」，不再写成「第 N 章」。

### 新增

- **R7 世界构建文风 Tab**：世界观 / 文风双 Tab，接入 `useWritingStyle` 编辑与初始化。
- **R8 用量统计启发式加强**：扩展 bootstrap / smart_execute 关键词，并解析 `metadata` JSON 字段。
- **R9 伏笔三列 Kanban**：未回收 / 已回收 / 已放弃列视图；展开行可编辑 Ledger 目标窗口。
- **R10 角色→场景跳转**：角色卡展示关联场景；经 `pendingSceneId` 跳转 Scenes 并选中。
- **R11 拆书转故事导航**：转换成功后 invalidate stories、设当前故事并进入场景页。

### 验证

- `cargo test --lib`：685 passed / 0 failed / 2 ignored ✅
- `cargo +nightly fmt -- --check`：✅
- `cargo check`：✅（仅既有 warning）
- `npx vitest run`：237 passed / 3 skipped ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅
- `npm run format:check`：✅

## [v0.26.34] - 修复提示词导入参数并新增「打开本地目录」功能（2026-07-09）

### 修复

- **后台提示词页面批量导入失效**：`PromptsPanel.handleImportAll` 调用 `save_prompt_override` 时使用了 `promptId`（camelCase），与后端 `rename_all = "snake_case"` 期望的 `prompt_id` 不匹配，导致导入全部静默失败。已修正为 `prompt_id`。
- **加载失败无明确提示**：提示词列表加载失败时页面仅 toast 报错，未展示具体错误。新增 `loadError` 状态并在页面上方显示错误详情。

### 新增

- **后台提示词页面「打开目录」按钮**：新增 `get_prompts_directory` 后端命令，暴露当前生效的 prompts 资源目录路径；前端标题栏新增「打开目录」按钮，点击后使用系统文件管理器打开该目录（开发环境下为项目 `resources/prompts`，生产环境下为应用包内资源目录）。
- **后台提示词页面「刷新」按钮**：支持重新加载提示词列表与目录路径。
- **导出/导入按钮移出重置确认弹窗**：将「导出」「导入」操作放到页面标题栏，避免与「全部重置」混淆。

### 验证

- `cargo test --lib`：685 passed / 0 failed / 2 ignored ✅
- `cargo +nightly fmt -- --check`：✅
- `cargo check`：✅（仅既有 warning）
- `npx vitest run`：237 passed / 3 skipped ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅
- `npm run format:check`：✅
- `npm run build`：✅

## [v0.26.33] - 补齐阶段 2/3/4 具体缺口：KG/角色关系删除、前端解耦（2026-07-08）

### 新增

- **知识图谱实体归档与关系删除 UI**（Stage 4 缺口）：
  - 后端新增 `delete_relation` 仓库方法及 `archive_entity` / `delete_relation` Tauri 命令，成功后发射 `knowledgeGraph` 同步事件。
  - 前端 `KnowledgeGraphView` 实体详情面板新增归档按钮，关系列表新增删除按钮，均带 `confirm()` 确认。
  - `KnowledgeGraph.tsx` 接入删除回调并刷新图谱数据。
  - 新增 `KnowledgeGraphView` 删除相关单元测试。

- **角色关系删除 UI**（Stage 2 缺口）：
  - `useCharacterRelationships.ts` 新增 `useDeleteCharacterRelationship` hook。
  - `Characters.tsx` 关系卡片新增删除按钮（`Trash2`），确认后调用删除 mutation 并刷新列表。
  - 新增 `Characters.test.tsx` 覆盖删除按钮与取消行为。

- **前端 `frontstage ↔ components` 解耦**（Stage 3 缺口）：
  - 新增 `src-frontend/src/hooks/contracts/useEditorConfig.ts`，将 `loadEditorConfig` / `saveEditorConfig` 及 `useEditorConfig` hook 从 `EditorSettings.tsx` 提取到合约层。
  - `FrontstageApp.tsx`、`RichTextEditor.tsx` 改为从 `hooks/contracts/useEditorConfig` 引入，不再依赖 `components/EditorSettings.tsx`。
  - `EditorSettings.tsx` 本身也复用合约层的 load/save 函数。
  - `madge --circular` 验证无循环依赖。
  - 新增 `useEditorConfig.test.tsx`（7 条测试）。

### 验证

- `cargo test --lib`：684 passed ✅
- `cargo +nightly fmt -- --check`：✅
- `cargo clippy --lib`：✅（仅既有 warning）
- `npx vitest run`：234 passed / 3 skipped ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅
- `npm run format:check`：✅

## [v0.26.32] - 完成阶段一剩余项：L1 创作入口、仪表盘统计卡、memory/ingest 测试（2026-07-08）

### 新增

- **L1 创作入口 UX 统一**：Dashboard / Stories 的 `CreationPathGuide` 由纯展示变为可点击，三张卡片分别进入幕前 Genesis、AI 向导、快速创作流程。
  - `CreationPathGuide` 新增 `onFrontstage` / `onWizard` / `onQuick` 回调，卡片可点击并带 hover 反馈。
  - Dashboard 首页主按钮“AI 创建故事”改为进入推荐的幕前 Genesis 流程（`show_frontstage`），与“推荐”标签一致。
  - Stories 页面向导卡片支持创建新故事（放宽 `isWizardOpen && wizardStory` 渲染条件）。
  - 新增 `CreationPathGuide.test.tsx`（5 条测试）与 `Dashboard.test.tsx` 相关用例。

- **仪表盘统计卡修正**：
  - 将“章节”改为“场景”，与跳转目标 `scenes` 视图一致。
  - 新增“字数”统计卡，显示所有故事 `word_count` 总和，点击跳转到场景视图。
  - 统计值优先使用 `useStories` 查询结果，避免 Zustand store 滞后。
  - `Story` 类型新增可选 `word_count` 字段。

- **`memory/ingest` 首批特征测试**：
  - 新增 5 条单元测试覆盖 `extract_json`（markdown 代码块解析、无法解析输入）、`build_event_chain`（事件排序与因果链）、`get_recent_jobs`（DESC 排序与空结果）。
  - 测试使用内存 SQLite pool，不依赖外部 LLM 调用。

### 验证

- `cargo test --lib`：682 passed ✅
- `cargo +nightly fmt -- --check`：✅
- `cargo clippy --lib`：✅（仅既有 warning）
- `npx vitest run`：222 passed / 3 skipped ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅

## [v0.26.31] - 修复幕前状态栏体验、策略解析鲁棒性与新数据库 schema（2026-07-08）

### 修复

- **幕前顶部状态栏字数统计滞后**：章节加载后 `wordCount` 始终为 0，直到首次自动保存成功才更新；切章时 `currentChapterPrevWordCountRef` 也未重置，导致全文字数 diff 基准错误。
  - `selectChapter` 加载正文后即时计算并设置当前章节字数。
  - `handleContentChange` 中字数变化时同步更新 `wordCount`，与 `totalWordCount` 保持一致。
  - 新增回归测试：章节加载后立即显示非零字数。

- **顶部状态栏字体大小不可点击**：点击 `12px` 等字号显示无响应。
  - `FrontstageHeader` 新增 `onOpenFontSettings` 回调，字号显示改为可点击。
  - 扩展 Tauri `show_backstage` 命令支持 `view` / `panel` 参数，点击后打开幕后「通用设置」并自动滚动到「编辑器设置」卡片。

- **底部状态栏后台任务图标显示为缺字符号（tofu）**：`FrontstageBottomBar` 的 `categoryIcons` 直接使用 emoji，在部分系统字体下无法渲染。
  - 将 8 个活动类别的 emoji 图标替换为 `lucide-react` SVG 图标，统一视觉风格。
  - 新增回归测试验证图标渲染为 SVG。

- **策略选择 JSON 解析失败**：LLM 输出仍可能使用旧字段名 `reasoning` 或缺失 `rationale`，在部分边界情况下解析失败。
  - `SelectedStrategy.rationale` 增加 `#[serde(default, alias = "reasoning")]`，缺失时默认空字符串，识别 `reasoning` 别名。
  - 新增回归测试覆盖 `reasoning` 别名与缺失 `rationale` 的默认行为。

- **新数据库仍可能缺失 `source` / `is_auto_generated` 列**：v0.26.30 的兜底修复只覆盖已存在表，新库初始建表语句未包含这两列。
  - `create_tables` 中 `characters` / `scenes` / `world_buildings` / `kg_entities` 四表新增 `source` 与 `is_auto_generated` 列定义，确保新库自创建起即完整。

### 验证

- `cargo test --lib`：677 passed ✅
- `cargo +nightly fmt -- --check`：✅
- `cargo clippy --lib`：✅（仅既有 warning）
- `npx vitest run`：213 passed ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅

## [v0.26.30] - 热修复旧数据库缺失 source/is_auto_generated 列（2026-07-08）

### 修复

- **旧数据库缺失 `source` / `is_auto_generated` 列**：部分用户在 v0.26.28 迁移框架由 inline `run_migrations` 切换为编号 Rust migrations 后，数据库 `schema_migrations` 已到 V102，但 `characters` / `scenes` / `world_buildings` / `kg_entities` 表未成功写入 `source` / `is_auto_generated` 列，导致 Genesis 与资产查询报 `no such column: source`。
  - 新增 Rust migration `V103__ensure_source_columns`，幂等地为上述四表补回缺失列。
  - 在 `init_db` 中新增 `ensure_source_columns` 启动兜底修复，即使迁移记录已处于高版本也能自动补齐列。
  - 新增回归测试 `test_v103_repairs_missing_source_columns`，模拟 `schema_migrations=102` 但列缺失的场景。

### 验证

- `cargo test --lib`：674 passed ✅
- `cargo +nightly fmt -- --check`：✅
- `npx vitest run`：210 passed ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅

## [v0.26.29] - 热修复 prompts 外部化后策略选择 JSON schema 不匹配（2026-07-08）

### 修复

- **策略选择 JSON schema 不匹配**：v0.26.28 将 prompts 外部化后，`resources/prompts/strategy/strategy_selector.md` 仍要求旧版字段（`selected_strategy`/`reasoning`/`asset_combination`），而 `selector.rs` 已改用新版 `SelectedStrategy` schema（`rationale`/`genre_profile_id`/`methodology_id`/…），导致 Genesis「选择创作策略」步骤报 `VALIDATION_FAILED: missing field rationale`。
  - 重写 `strategy_selector.md` 模板，要求 `rationale`、`genre_profile_id`、`methodology_id`、`style_dna_ids`、`skill_ids`、`workflow_id`、`parameters` 字段。
  - 在 `selector.rs` 的 `parse_strategy_response` 中增加 `LegacyStrategyResponse` 兜底解析，兼容旧格式输出，将 `selected_strategy`/`reasoning`/`asset_combination` 映射为新 schema。
  - 新增 `test_parse_strategy_response_legacy_schema` 单元测试覆盖旧格式解析路径。

### 验证

- `cargo test --lib`：673 passed ✅
- `cargo +nightly fmt -- --check`：✅
- `npx vitest run`：210 passed ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅

## [v0.26.28] - Phase 4 架构债务与工程体验（2026-07-07）

### 新增

- **知识图谱手动 CRUD UI**：Graph 页图例面板新增「新建实体」按钮；实体详情面板新增「添加关系」按钮，支持从当前故事已有实体中按名称搜索并建立关系。
- **世界构建 AI 生成**：`WorldBuilding` 页新增「AI 生成」按钮与 `AiWorldBuildingModal`，基于当前故事调用 `generateWorldBuildingOptions` 一键生成世界观并回写。
- **角色 AI 扩展**：`Characters` 页新增「AI 扩展」按钮与 modal，基于当前世界观调用 `generateCharacterProfiles`，选择角色组后批量 `createCharacter`。
- **叙事分析图表**：`NarrativeAnalysis` 页新增 SVG `ReadingPowerChart` 折线/面积图，替代原有条形图展示追读力趋势。

### 重构

- **策略选择移入 Quick Phase**：`genesis.rs` 中 `StrategySelectionStep` 从 `background_steps()` 前移至 `quick_phase_steps()`，位于 `ConceptGenerationStep` 之后、`FirstChapterGenerationStep` 之前；同步更新所有步骤的 `step_number`/`total_steps`/`progress_percent` 与前后端测试契约。
- **外部化 prompts**：`prompts/registry.rs` 中 95 个内置提示词迁移至 `resources/prompts/{category}/{id}.md`（YAML frontmatter + Markdown body）；运行时通过 Tauri 资源目录加载，测试环境回退到 `CARGO_MANIFEST_DIR/../resources/prompts`；`load_overrides` 等用户覆盖逻辑保持不变。
- **迁移脚本拆分**：`db/connection.rs` 中 2,650 行 inline `run_migrations` 拆分为 `src/db/migrations/V028__*.rs` … `V099__*.rs` 共 70 个编号 Rust 迁移文件；`MigrationRunner` 新增 `RustMigration` trait 与 `with_rust_migrations()`，统一排序、过滤、执行 SQL 与 Rust 迁移；原有 `schema_migrations` 版本语义保持不变。

### 验证

- `cargo test --lib`：672 passed ✅
- `cargo +nightly fmt -- --check`：✅
- `npx vitest run`：210 passed ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅

## [v0.26.27] - L4 诊断互链、文档与依赖解耦（2026-07-07）

### 新增

- **GenesisPanel ↔ TracingPanel 互链**：Genesis 运行记录增加「查看生成链路」；链路详情增加「对应 Genesis 运行」，跳转后自动选中对应 session。
- **GenesisPanel → Logs 深链**：失败运行增加「查看日志」，跳转日志页并预填 `session_id`。
- **用量统计按 operation 分组**：新增 全部 / bootstrap / smart_execute / 其他 标签，按 `purpose` / `task_type` 关键词启发式分组（待后端 `operation` 字段补齐后切换为精确分组）。
- **伏笔看板 UX 改进**：`setup_scene_id` 改为场景下拉选择；展开高级区可编辑 `target_start_scene` / `target_end_scene`。

### 重构

- **前端循环依赖解耦**：`stores/*` 不再依赖 `components/*` / `hooks/*`；通过 `types/editor.ts`、`stores/contracts/*` 提取共享类型，`madge` 报告无循环依赖。
- **Tauri 循环依赖解耦**：`creative_engine ↔ llm`、`model_gateway ↔ router` 的共享概念提取到 `ports/` / `domain/` trait，两对模块不再直接互相 import。

### 文档

- 更新 `docs/USER_GUIDE.md`：补全 3.17 生成链路 / 3.18 意图图诊断 / 3.19 日志查看；修正 3.11 伏笔看板、3.12 叙事分析、3.14 用量统计的过度承诺。
- 同步 `AGENTS.md`、`ROADMAP.md`、`TESTING.md`、`ARCHITECTURE.md`、`README.md` 版本与完成状态。

### 验证

- `cargo test --lib`：672 passed ✅
- `cargo +nightly fmt -- --check`：✅
- `npx vitest run`：210 passed ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅

## [v0.26.26] - L2 资产补齐与领域层止血（2026-07-07）

### 新增

- **角色页编辑 + 关系 CRUD**：角色资料卡 hover 显示「编辑」按钮；关系 Tab 与角色卡头部支持「添加关系」；使用现有 `update_character` / `createCharacterRelationship` API。
- **L2 创世溯源徽章**：世界观、角色、场景、知识图谱实体均显示「创世」徽章（与 Foreshadowing 一致）；后端在 Genesis/Wizard 写入时标记 `source="genesis"`、`is_auto_generated=true`。
- **Story System 合同播种状态卡**：Contracts Tab 显示 `MASTER_SETTING` 与 `CHAPTER_1` 合同是否存在；缺失且存在失败 Genesis run 时展示错误摘要与跳转。
- **Scenes 续写跳转幕前**：`ExecutionPanel` 的「继续写作」主行动调用 `show_frontstage` 并 toast 提示。

### 重构

- **拆分 `StorySystem.tsx`**：拆为 8 个独立标签组件，`StorySystem.tsx` 仅保留 125 行 tab 路由。
- **Repository 层 trait 化与拆分**：`db/repositories.rs`（6,566 行）拆分为 `db/repositories/*.rs`；`creative_engine/context_builder.rs` 改为依赖 `db/traits.rs` 中的 trait 而非具体仓库。

### 修复

- 修复测试内存库迁移顺序冲突：将 `V099__intention_graph_sing_data.sql` 重命名为 `V102__...`，避免与 `MAX_INLINE_MIGRATION_VERSION=99` 冲突导致 inline migrations 28–98 被跳过。

### 验证

- `cargo test --lib`：672 passed ✅
- `npx vitest run`：210 passed ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅

## [v0.26.25] - Backstage Genesis 可观测性与测试基线（2026-07-07）

### 新增

- **GenesisPanel 动态步骤模型**：`GenesisPanel` 现在从后端 `steps_json` 解析 Quick（2 步）+ Background（6 步），而非硬编码 8 步；显示非致命 `errors[]` 并支持展开详情。
- **Genesis run 跳转**：运行记录含 `story_id` 时，面板提供「打开故事」和「开幕前」按钮。
- **L1 创作路径引导**：Dashboard 与 Stories 页新增 `CreationPathGuide`，明确区分「幕前 Genesis / 幕后 Wizard / 快速创作」三条路径。
- **Stories Wizard 重复建故事修复**：对已有故事调用 Wizard 时，改为更新现有故事资产，不再重复创建。
- **仪表盘统计卡可点击**：三张统计卡分别跳转故事库、角色页、场景页。

### 测试

- `genesisSteps.ts` 新增 18 个单元测试，覆盖步骤解析、error 计数、进度合并、异常 JSON fallback。
- 为高变更后端模块补充首批特征测试：`model_gateway/executor.rs`、`db/repositories.rs`、`memory/ingest.rs` 各至少 1 条 happy path + 1 条错误路径。

### 验证

- `cargo test --lib`：677 passed ✅
- `npx vitest run`：210 passed ✅
- `npx tsc --noEmit`：✅
- `python3 scripts/architecture_guard.py`：PASSED ✅

## [v0.26.24] - 修复续写重复、截断与跨内容复述（2026-07-07）

### 修复

对照 `creative_workflow.log` 2026-07-07 08:44–09:05 续写会话（新写 → 多次续写），定位 5 项根因并结构性修复：

- **散布式句子块重复**：续写时模型陷入意象循环（冥界/牢笼/苦楚块在单次生成内重复 2–3 次），段落级与 KMP border 检测均抓不到。新增 `trimInterspersedRepeatedBlocks`（Rust + TS 对齐，跨层 golden 双跑）。
- **跨内容重叠复述**：Writer 把 `build_continuation_context` 注入的尾部预览段落重新输出（如「恶魔的嘴唇…」），`startsWith` / `isTextDuplicate` 无法拦截。新增 `stripExistingOverlap`（比对已有正文尾部 3000 字，剥离 ≥25 归一化字重叠前缀），后端 TriShot 与前端 `FrontstageApp` 全路径接入。
- **截断末句污染**：60s 超时硬截断留下极短半句（如「冥界的阴霾更。」）污染后续上下文。新增 `trimDanglingTail`（末句归一化 < 12 字且全文 ≥ 2 句时裁掉）。
- **续写无生成侧重试闸门**：Genesis 有 8% 自重复重试，TriShot 续写只有事后 sanitize。`orchestrator.rs` 补齐 anti-repeat 重试，取更干净版本。
- **前端后处理管线统一**：`sanitizeContinuationOutput` = trimSelfRepetition + stripExistingOverlap + trimDanglingTail，覆盖 `smart_execute`、`appendAiContent`、`handleRequestGeneration`。

### 验证

- `cargo test --lib`：666 passed ✅
- `npx vitest run`（textCleanup + golden）：192 passed ✅
- 跨层 `tests/fixtures/trim_golden.json` 双跑 ✅

## [v0.26.23] - 修复 v0.26.22 CI prettier 格式检查失败（2026-07-07）

### 修复

- **修复 v0.26.22 CI `frontend-check` 失败**：v0.26.22 的 Bug D 重入守卫编辑后未跑 `npm run format:check`，`FrontstageApp.tsx` 一处 prettier 格式不符 CI 检查。运行 `npm run format` 修正。
- 同步落实新规则「推送后必须及时检查 GitHub Actions 构建错误」——本次因及时检查才发现 v0.26.22 CI 失败并立即修复。

### 验证

- `npm run format:check`：✅
- `cargo test --lib` 655 / `npx vitest run` 183（v0.26.22 已验证，本次仅格式变更）

## [v0.26.22] - 修复续写卡死与幽灵文本混乱（4 项根因）（2026-07-07）

### 修复

对照 `creative_workflow.log` 2026-07-07 续写会话时间线，定位并修复 4 个根因：

- **Bug B（卡死主因）— auto_contract 静默化**：续写完成/Tab 接受后触发的 `auto_contract` 管线（master_setting 49s + chapter 184s + scene_outline 151s ≈ 6 分钟）非静默发射进度事件，使 `isAnyBackendActive:true` 长达 6 分钟，阻塞用户发起新续写。将 4 个 `auto_contract_*` context label 加入 `is_silent_background` 列表，后台补齐合同不再阻塞前端活动状态。
- **Bug D（混乱主因）— 续写重入守卫**：第 2 次续写结果生成后幽灵已设，用户 7 秒后发起第 3 次续写，旧幽灵未丢弃，`current_content_len` 未并入，导致两份续写结果竞争。`handleSmartGeneration` 入口加守卫：存在未接受幽灵时先丢弃并提示，再开新生成。
- **Bug A — 幽灵文本 10s 渲染延迟**：`bodyHidingGhost` 在 render 中直接读 DOM (`document.body.classList.contains`)，非响应式；useEffect 移除 `force-hide-ghost` 类后不触发重渲染，`bodyHidingGhost` 保持 stale true。新增 `bodyForceHideGhost` state 镜像类存在性，移除/添加时同步翻转触发重渲染，`shouldShowGhostTree` 立即生效。
- **Bug C — 续写慢模型 fail-fast**：续写 `trishot-writer` 路由到 HeavyCreation（质量权重 0.8），优先选用户慢模型 MN-Oblivion（198s），而快模型 Gemma4 仅 10s。续写（非创世首章）call3 超时上限从 120s 降至 60s，慢模型 fail-fast 回退到快模型。

### 验证

- `cargo test --lib`：655 passed ✅
- `npx vitest run`：183 passed ✅
- `cargo +nightly fmt -- --check` / `npx tsc --noEmit`：✅

## [v0.26.21] - 修复 Windows MSI 构建（迁移文件名重命名）（2026-07-07）

### 修复

- **修复 v0.26.17 起 Windows MSI 构建持续失败（WiX `light.exe` 退出）**：v0.26.17 为 Issue #4 将 `src/db/migrations/` 打包为 Tauri resource，但 24 个迁移文件名含中文/全角逗号 `，`/破折号 `—` 且最长 102 字符。WiX `light.exe` 从文件名生成 `File/@Id` 标识符（限 72 字符、仅 `[A-Za-z0-9_]`），长中文文件名生成超长/非法标识符导致 `light.exe` 静默失败（stderr 被 tauri-action 吞掉）。
- **根因确认**：v0.26.14/v0.26.16（resources 引入前）的 Windows MSI 曾成功发布；v0.26.17 引入 `resources` 打包中文迁移文件后持续失败。v0.26.20 尝试的 `wix.language: zh-CN`（代码页修复）无效——问题在标识符生成而非代码页编码。
- **修复方式**：将 24 个迁移文件重命名为 ASCII 短名（保留 `V###` 前缀与排序，如 `V022__reference_books_add_task_id.sql`）。`schema_migrations` 按 version 跟踪（非文件名），已应用迁移不受影响；`parse_filename` 仅解析 `V###` 前缀，description 仅用于日志，无逻辑变更。
- **macOS 公证**：v0.26.20 起随 Apple Developer 协议续签已恢复成功。

### 验证

- `cargo test --lib migrations`：8 passed ✅（含中文文件名解析测试，解析器仍兼容中文）
- 本地 `cargo tauri build`（macOS）：✅
- CI Windows MSI 构建待验证（v0.26.21 tag）

## [v0.26.20] - 修复 v0.26.19 CI 格式检查失败与 Windows 打包（2026-07-06）

### 修复

- **修复 v0.26.19 CI `cargo +nightly fmt -- --check` 失败**：v0.26.19 中 `ParallelWorldOutlineCharacterStep` 的 doc 注释行超过 `max_width=100`（`wrap_comments=true`），CI 端 rustfmt nightly 与本地 nightly 换行行为差异导致 CI 报 diff。运行 `cargo +nightly fmt` 自动换行该注释，本地 `--check` 与 CI 对齐。
- **修复 Windows MSI 打包失败**：v0.26.18 起在 `tauri.conf.json` 中通过 `bundle.resources` 打包 `src/db/migrations/` 下的 SQL 迁移文件，其中文件名包含中文字符。WiX 默认 `en-US` 语言/代码页无法处理这些文件名，导致 `light.exe` 在 `Running light` 阶段无详细错误地退出。在 `bundle.windows.wix.language` 显式设置为 `zh-CN`，使 MSI 使用支持中文的代码页。
- 仅配置变更，无逻辑改动。

### 已知/待处理

- **macOS 公证失败（外部阻塞）**：`tauri-build (macos-latest)` 在代码签名成功后，`notarytool` 返回 `HTTP 403: A required agreement is missing or has expired`。这是 Apple Developer 团队的法律协议未签署或已过期，需要在 [Apple Developer](https://developer.apple.com/account) 中接受最新协议后方可恢复。在协议签署前，CI 的 macOS 构建无法完成公证。

### 验证

- `cargo +nightly fmt -- --check`：✅
- `cargo test --lib narrative::genesis::`：11 passed ✅
- `npm run format:check`：✅
- `cargo check`：✅（104 warnings，零错误）
- `npx tsc --noEmit`：✅
- Windows MSI 打包：待 GitHub Actions 验证

## [v0.26.19] - Genesis 创世流程全面审计与测试加固（2026-07-06）

### 审计与优化

对照项目文档（架构、逻辑、设计）对「智能创作流程-创世」进行全面审计，分 Phase 1–4 执行修复、加固与测试补齐。

#### Phase 1 — P0 竞态与契约修复

- **Gap B（空 finalContent 不锁 delivered）**：`isFirstChapterReady` 路径在 `finalContent` 经 trim 后为空时不锁定 `genesisDeliveryRef='delivered'`，否则后续 ChapterSwitch 携正文时被 delivered 闸门阻塞，编辑器永久空白。
- **角色生成世界观上下文（P0-2）**：`ParallelWorldOutlineCharacterStep` 中 `character` 提示词读取 `bundle.world_building` 恒为空字符串（闭包捕获竞态），导致角色与世界观脱钩。改为先 await `world` 拿到真实 `world_concept` 再构造 `character` 块；提取 `world_concept_for_character_prompt` 纯函数 + 单测。
- **ChapterSwitch delivered 时序（P0-3）**：`selectChapter` 懒加载 `get_chapter` 失败时不应标记 `delivered`（原实现调用前就标记，失败则编辑器空白且 delivered 已锁）。改为 `markDeliveredOnLoad` 仅在 `setContent` 真正成功后标记。

#### Phase 2 — P1 架构对齐

- **后台错误可观测性**：`GenesisContext` 新增 `errors: Arc<Mutex<Vec<GenesisStepError>>>` 共享错误集合，各步骤非致命错误（存储失败、规则更新失败等）累计其中；后台 phase 完成后序列化到 `genesis_runs.steps_json` 并 emit `genesis-warnings` 事件，前端 toast 区分 warning/error。
- **mutex 中毒锁加固**：`pipeline.rs` 的 `PIPELINE_CANCEL_FLAGS` 与 `model_gateway/executor.rs` 的 registry 锁改用 `unwrap_or_else(|e| e.into_inner())` 恢复中毒锁，避免线程 panic 后全局不可用。新增中毒恢复单测。
- **策略移入 quick phase**：经评估**暂缓**（仅在 `genesis_runs`/`ROADMAP` 记录为债务），优先保障文档载明的延迟契约与当前稳定性。
- **文档/类型对齐 auto-accept 真实路径**：`window/mod.rs` 与生成的 `FrontstageEvent.ts` 注释重写，明确创世第一章 `ChapterSwitch` 不携正文（`content: None`, `auto_accept: false`），正文唯一写者是 `smart_execute.final_content`。

#### Phase 3 — 测试加固

- **Rust Genesis 测试补齐**：将 8% 自重复重试闸门与 ChapterSwitch payload 的内联决策提取为纯函数（`compute_trim_ratio`、`should_retry_self_repetition`、`select_first_chapter_content`、`build_first_chapter_chapter_switch`）并测试边界/阈值/payload 契约；新增 `background_steps` 6 步固定顺序契约测试。
- **前端 Gap B/C + 状态机断言**：新增 Gap C 专用测试（delivered + 编辑器有内容 + 入站非重复 → 跳过 setContent）与状态机端点契约测试（idle → delivered 可观测效果）。
- **跨层共享 trim golden fixture**：新增 `tests/fixtures/trim_golden.json`（7 条用例），Rust `trim_self_repetition` 与 TS `trimSelfRepetition` 双跑同一 fixture，锁定跨层一致性契约（v0.26.16「Rust 对齐前端 KMP」的回归守卫）。
- **降低测试 brittleness**：新 Gap C 测试采用 `waitFor` 轮询替代固定 `setTimeout`。

#### Phase 4 — 代码整洁

- **重命名 `*_future`**：`ParallelWorldOutlineCharacterStep` 的 `world_future`/`outline_future`/`character_future` 实为顺序 await 的 `BoxFuture`，重命名为 `*_gen` 并更新注释澄清非并行 + 标注 world/outline 可并行化延迟债务。
- **去重 `AppConfig::load`**：`FirstChapterGenerationStep::execute` 内连续两次 `AppConfig::load`（第一次结果未被使用）合并为单次。
- **`appendAiContent` skip 路径不 `markAccepted`**：`markAccepted` 移入实际追加成功的 `else` 分支，避免空文本/近期已追加的 skip 路径污染 `useRecentAcceptGuard`。
- **`selectChapter` Gap C 重复入站也跳过 setContent**：移除 `!isTextAlreadyInEditor` 条件，delivered + 编辑器有内容时一律 skip（dup 与 non-dup 都 skip），消除重复入站的冗余重写。
- **评估合并 `isGenesisSettingUpRef` → `genesisDeliveryRef`**：经评估**不合并**——两者覆盖窗口不同（前者覆盖续写路径 story_created bootstrap，后者仅创世 generating 态），合并需扩展状态机语义，按 R4 暂缓。

### 验证

- `cargo test --lib`：**655 passed / 0 failed / 2 ignored**（+10 新增 Rust 测试）
- `npx vitest run`：**183 passed / 3 skipped**（+17 新增前端测试）
- `npx tsc --noEmit`：✅ 零错误
- `cargo +nightly fmt -- --check`：✅
- `npm run format:check`：✅

## [v0.26.18] - Genesis 第一章重复：竞态路径加固（2026-07-06）

### 修复

- **修复 v0.26.16 后仍偶发的 Genesis 第一章内容重复**：用户报告 v0.26.16 后新写小说第一章仍有重复。代码审查发现三个残留竞态缺口：
  - **Gap A（ChapterSwitch 空内容竞态）**：`ChapterSwitch` 事件 `auto_accept=true` 但 `payload.content` 为空时，原逻辑不标记 `delivered` 且 `skipContent=false`，导致 `selectChapter` 从 DB 加载正文，随后 `smart_execute` 返回又 `appendAiContent` 叠加 → 重复。现改为 `skipContent=true`（不从 DB 加载），且不标记 `delivered`（让 `smart_execute` 仍能投递 `final_content`）。
  - **Gap B（delivered 误锁）**：`isFirstChapterReady` 路径在 `if` 块末尾无条件 `genesisDeliveryRef.current = 'delivered'`，即使 `finalContent` 经 trim 后为空或 `isAlreadyPresent` 但编辑器实际为空时也会锁死状态机 → 编辑器空白。现仅在「已 append」或「编辑器已有内容」时标记 `delivered`。
  - **Gap C（selectChapter 咽喉点缺守卫）**：`selectChapter` 注释声称由 `onChapterUpdated` 等通道承担 `delivered` 防护，但 `selectChapter` 是直接调用路径，不受 `onChapterUpdated` 守卫保护。当 `delivered` 已设置且编辑器已有内容时，`skipContent=false` 的 `setContent` 会覆盖或叠加。新增咽喉点守卫：`delivered` 且编辑器已有非空内容且 incoming 不在编辑器中时跳过 `setContent`。
- **回归测试**：新增 `ChapterSwitch auto_accept=true 但 content 为空时 smart_execute 仍能投递 final_content`，覆盖 Gap A 竞态路径。

### 验证

- `npx tsc --noEmit`：✅ 零错误
- `npx vitest run`：**167 passed / 3 skipped**（含 9 个 genesis-duplicate 测试）
- `cargo test --lib`：639 passed（未改动 Rust）

## [v0.26.17] - Issue #4 启动加固：打包 SQL 迁移与 init_db 诊断增强（2026-07-06）

### 修复

- **Issue #4 一级根因加固（init_db 失败路径）**：
  - 生产安装包打包 `src/db/migrations/` 到 `$RESOURCE/db/migrations/`，避免 Release 仅依赖 inline 迁移导致 schema 不完整。
  - `setup` 从 Tauri Resource 目录解析 bundled migrations 并传入 `init_db`。
  - `init_db` 启动前确保 app data 目录存在；失败日志包含完整 DB 路径与 migrations 目录。
  - `create_dir_all` 失败不再静默忽略；降级模式日志明确提示检查 `init_db` 错误。
- **回归测试**：新增 `init_db_succeeds_on_fresh_directory`，覆盖全新目录初始化成功路径。

### 验证

- `cargo test --lib init_db`：**2 passed**（含 Issue #4 不可写目录 + fresh init）
- `cargo check`：✅ 通过

## [v0.26.16] - 根治 Genesis 第一章重复、Issue #4 启动稳定性与代码格式修复（2026-07-06）

### 修复

- **根治 Genesis 第一章内容重复**：替代 v0.26.7–v0.26.14 的散布布尔守卫补丁模式，从两个独立根因进行结构性修复。
  - **R2 生成侧验证闸门（`src-tauri/src/narrative/genesis.rs`）**：检测 LLM 输出自重复比例，≥8% 时用更强 anti-repeat 指令重试一次；重试更干净则采用，否则保留首次清理结果；prompt 模板新增「结构纪律」段，明确禁止首尾回环与整章重复。
  - **R1 前端单写者状态机（`src-frontend/src/frontstage/FrontstageApp.tsx`）**：将 `genesisAutoAcceptedRef` 布尔（10+ 处赋值）替换为 `idle → generating → delivered` 三态状态机；`generating` 态阻塞 `onChapterUpdated` 与 `loadStories` 自动选择；`delivered` 态阻塞 `setGeneratedText` 幽灵文本恢复；`selectChapter` 的 `skipContent` 由调用方按场景传入，不设硬闸门。
  - `textCleanup` 提升到 `src-frontend/src/utils` 供渲染层与排版共享；Rust `trim_self_repetition` 对齐前端 KMP 最长 border 检测；全路径（`selectChapter` / `onChapterUpdated` / `ContentUpdate` / `AppendContent` / `autoFormatText`）统一调用 `trimSelfRepetition`。
- **修复 Issue #4：init_db 失败时应用启动 panic/Windows 闪退**：当应用数据目录不可写时，`init_db` 返回错误但 `setup` 仍构造 `GatewayExecutor`，其内部 `state::<DbPool>()` 因 pool 未 manage 而在启动时 panic。现改为显式将 `pool` 传入 `GatewayExecutor::new`，并在 `setup` 中仅当 `pool` 存在时才初始化管理网关执行器与健康探测调度器；新增回归测试覆盖不可写目录场景。
- **修复 CI 格式检查失败**：`src-tauri/src/utils/text.rs`、`src-frontend/src/frontstage/FrontstageApp.tsx`、`src-frontend/src/utils/__tests__/textCleanup.test.ts` 等文件未通过 `cargo +nightly fmt -- --check` 与 `npm run format:check`；已全局格式化并重新通过检查。

### 验证

- `cargo test --lib`：**637 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt -- --check`：✅ 通过
- `npm run format:check`：✅ 零差异
- `npx tsc --noEmit`：✅ 零错误
- `cargo check`：✅ 通过
- `python3 scripts/architecture_guard.py`：✅ 通过

## [v0.26.14] - 修复 Genesis 第一章模型输出自重复与降低幕前诊断日志压力（2026-07-05）

### 修复

- **修复 v0.26.13 用户仍报告的「新写小说第一章内容重复」**：分析 `creative_workflow.log` 中 13:43 的完整链路（故事《终极遗骸者：诞生者》）后发现：
  - 后端 `genesis.first_chapter.generated` 与 `append_text_check` 均只生成/写入一次正文；
  - 前端 `append_text_check.occurrences=1`、`hasDuplicate=false`；
  - **重复不是前端追加两次造成的**，而是 LLM 输出的 613 字正文自身存在「首段与末段相同」的模型级循环/自重复。
- 新增 `trimSelfRepetition` 工具（`src-frontend/src/frontstage/utils/trimSelfRepetition.ts`）：
  - **段落级**：检测「后半段整体重复前半段」或「末段重复首段」，直接裁剪后半段/末段。
  - **字符级 border**：对归一化文本使用 KMP 最长 border，当尾部与开头重复长度 ≥30 字符且 ≥ 全文 8% 时，裁掉尾部重复。
- 在 `FrontstageApp.appendAiContent` 入口以及 `smart_execute` 返回的 `finalContent` 写入编辑器/幽灵文本前统一调用自重复清理，覆盖 Genesis 自动接受、Tab 接受、`ContentUpdate`/`AppendContent` 等所有路径。

### 性能 / 稳定性

- **缓解「写完后过会儿页面崩溃」**：`RichTextEditor` 的 `frontstage:rich_editor_diag` 渲染诊断日志此前每帧都通过 IPC 写入后端，长时间写作或文思活跃模式下日志量与 IPC 调用激增，可能成为页面卡顿/崩溃诱因。
  - 普通渲染日志从最多 100 次缩减到 **前 20 次**。
  - 仅当存在 `generatedText`、正在隐藏幽灵、或 body 级 `force-hide-ghost` 锁激活时才继续记录渲染日志。
  - IPC 日志节流从 50ms 收紧到 **200ms**。

### 测试

- 新增 `src-frontend/src/frontstage/utils/__tests__/trimSelfRepetition.test.ts`，覆盖：
  - 无重复文本保持不变；
  - 首段与末段重复的段落级裁剪；
  - 整章前后两半完全重复的裁剪；
  - 单段内长 suffix 重复前缀的裁剪；
  - 短文本/短 border 不触发裁剪。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**151 passed / 3 skipped**
- `npx playwright test`：**36 passed / 5 skipped**
- `python3 scripts/architecture_guard.py`：通过

## [v0.26.13] - 修复 Genesis 第一章渲染层视觉重复（幽灵容器残留）（2026-07-05）

### 修复

- **修复 v0.26.12 仍偶发的「新写小说第一章内容重复」视觉问题**：日志显示 `append_ai_done` 只触发一次、`append_text_check` 的 `occurrences: 1`、`hasDuplicate: false`，说明**数据层只写了一次**；重复来自渲染层幽灵文本/空幽灵容器与正文同框。
  - `RichTextEditor` 的 `shouldShowGhostTree` 从 `!!(generatedText || isGenerating)` 改为 `!!generatedText`，避免 `generatedText` 为空但 `isGenerating=true` 时渲染空幽灵容器；该容器若残留旧内容或 React 复用 DOM 节点异常，就会造成「正文 + 幽灵文本」同框。
  - `FrontstageApp` Genesis 自动接受路径里，在 `setGeneratedText('')` / `appendAiContent` 之前先调用 `setIsGenerating(false)`，确保 RichTextEditor 的幽灵树条件立即失效。
  - 渲染诊断日志增加 `isGenerating`、`isHidingGhost`、`bodyHidingGhost`、`generatedTextLen`，便于后续定位。

### 测试

- 增强 Playwright E2E 回归测试 `e2e/genesis-duplicate.spec.ts`：在原有「第一章正文只出现一次」断言基础上，新增 `[data-testid="ghost-paragraph"]` 必须隐藏的断言，从渲染层防止视觉重复。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**148 passed / 3 skipped**
- `npx playwright test --project=chromium`：**35 passed / 5 skipped**
- `python3 scripts/architecture_guard.py`：通过

## [v0.26.12] - 修复角色列表为空/未加载时的幕前崩溃与订阅状态空值（2026-07-05）

### 修复

- **修复打开已有故事或新写小说后页面白屏崩溃**：当 `useCharacters` 尚未返回角色数组（或返回 `null`）时，`RichTextEditor` 的「角色名点击」effect 在初始化阶段直接访问 `characters.length`，触发 `Cannot read properties of null (reading 'length')` 并被 ErrorBoundary 捕获为白屏。
  - `RichTextEditor` 角色点击 effect 增加 `characters` 空值守卫：`!characters || characters.length === 0` 时直接跳过事件注册。
  - `FrontstageApp` 中 `characters` 解构默认值仅对 `undefined` 生效；本次在消费层加防御，避免任何上游返回 `null` 时导致崩溃。

- **修复订阅状态接口返回 null 时的错误日志**：`useSubscription` 在 `getSubscriptionStatus()` 返回 `null` 时访问 `status.tier` 会抛出 TypeError（虽被 catch，但会污染日志）。改为使用 optional chaining 并回退到默认值。

- **增强全局崩溃诊断**：`frontstage/main.tsx` 全局错误处理器现在会输出 `__lastRichEditorEffect__`（如设置），帮助快速定位 RichTextEditor 内部哪个 effect 触发崩溃；`ErrorBoundary` 在捕获错误时也会向浏览器控制台输出完整堆栈。

### 测试

- 新增 Playwright E2E 回归测试 `e2e/genesis-duplicate.spec.ts`：模拟「故事列表已存在一部末世小说，用户输入『新写一部末世小说』走 smart_execute 自动接受第一章」的完整流程，验证编辑器中第一章正文只出现一次且页面不崩溃。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**148 passed / 3 skipped**
- `npx playwright test --project=chromium`：**35 passed / 5 skipped**
- `python3 scripts/architecture_guard.py`：通过

## [v0.26.11] - 修复 Genesis 第一章 store-editor 失步与崩溃隐患（2026-07-05）

### 修复

- **修复 Genesis 第一章内容可能因 store-editor 失步而重复或丢失**：v0.26.10 已确保数据层面只追加一次，但追加后前端 store 仍依赖 200ms onChange debounce 回写。若 `latestContentRef` 与编辑器实际 HTML 恰好指纹相同，`handleContentChange` 会提前返回，导致 store 长期为空；后续章节切换、自动保存或外部同步可能引发内容重复、白屏或崩溃。
  - `FrontstageApp.appendAiContent` 追加完成后立即读取 `editorRef.getHTML()`，并同步到 `useFrontstageStore.setContent` 与 `latestContentRef.current`，不再依赖 debounce 回写。
  - `RichTextEditor.appendText` 空文档分支使用 `setContent` 时标记 `isExternalSyncRef`，并更新 `lastExternalContentRef` 为编辑器实际 HTML，避免父组件随后传入的 `content` prop 被外部同步 effect 再次 `setContent`，从而消除「正文 + 重设正文」的视觉抖动/重复。
  - `RichTextEditorRef` 新增 `getHTML()` 方法，为上层同步提供可靠的编辑器 HTML 快照。

- **修复开发模式下可能加载陈旧 dist 导致崩溃**：`src-tauri/tauri.conf.json` 已明确 `devUrl: http://localhost:5173`，确保 `cargo tauri dev` 始终加载最新 dev server 代码，避免旧 `dist` 中的 bug 代码在开发/调试时引发崩溃。

### 测试

- 新增并强化 `FrontstageApp.genesis-duplicate.test.tsx` 相关用例，验证 Genesis 自动接受后 store content 与编辑器内容一致。
- `RichTextEditor.duplicate.test.tsx` 覆盖空文档追加后外部同步 effect 不再重复 setContent 的场景。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**147 passed / 3 skipped**
- `python3 scripts/architecture_guard.py`：通过

## [v0.26.10] - 强化 Genesis 第一章重复防护（双重基准与追加最终防线）（2026-07-04）

### 修复

- **强化 Genesis 第一章内容重复防护**：v0.26.9 改用 `latestContentRef.current` 作为内容基准后，用户反馈在特定竞态下仍有重复。根因是单一基准可能在 ref 与编辑器 DOM 之间短暂失步，且上游返回的 `final_content` 可能仍包含当前正文前缀。
  - `isTextAlreadyInEditor` 改为双重检测：同时比对 `latestContentRef.current`（React state 同步快照）与 `editorRef.current?.getText()`（TipTap 实时 DOM），任一方显示内容已存在则跳过。
  - `appendAiContent` 增加安全网：若 `rawText` 仍以当前正文开头（上游 prefix 去重因 ref 滞后失效），先剥离该前缀再追加；追加后再用 `editorRef.current?.getText()` 回写校准 `latestContentRef`。
  - `RichTextEditor.appendText` 在插入前增加最终防线：若编辑器尾部已包含要追加的文本，直接跳过并记录 `frontstage:append_text_skip`。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**146 passed / 3 skipped**
- `python3 scripts/architecture_guard.py`：通过

## [v0.26.9] - 根治 Genesis 第一章重复（DOM 竞态与追加去重）（2026-07-04）

### 修复

- **彻底修复 Genesis 第一章内容重复（DOM 竞态）**：v0.26.8 已覆盖 pipeline-complete / ChapterSwitch 多数竞态，但重复检测仍依赖 `editorRef.current.getText()`。TipTap 编辑器的 DOM/Text 状态可能滞后于 React `content` prop，在 ChapterSwitch 或 pipeline-complete 刚加载正文后、编辑器尚未重渲染时，`getText()` 返回空/旧文本，导致去重失效并把已有正文恢复为幽灵文本或再次追加。
  - `isTextAlreadyInEditor`、`handleRequestGeneration`、`handleSmartGeneration` 的前缀去重统一改用 `latestContentRef.current`（React state 的同步快照）作为内容基准。
  - `appendAiContent` 的去重检测与追加后同步均改用 `latestContentRef.current`，避免 200ms onChange debounce 期间收到另一个 append 事件导致同一段内容被追加两次。
  - `RichTextEditor` 中 `editorContainsGeneratedText` 对 `generatedText` 剥离 HTML 标签后再做直接包含检测，避免 ContentUpdate/AppendContent 路径传入 HTML 时幽灵文本永远显示。

### 测试

- 新增 `FrontstageApp.genesis-duplicate.test.tsx` 用例「编辑器 DOM 滞后时仍不应恢复 generatedText」：用 deferred promise 让 ChapterSwitch 先于 smart_execute 返回，并模拟 `getText()` 滞后，验证 `latestContentRef` 路径下不会叠加幽灵文本。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**146 passed / 3 skipped**

## [v0.26.8] - 彻底修复 Genesis 第一章重复（竞态路径覆盖）（2026-07-04）

### 修复

- **彻底修复 Genesis 第一章内容重复**：v0.26.7 已修复 ChapterSwitch 自动加载正文后的重复，但用户反馈在 pipeline-complete 先加载 DB 正文、smart_execute 后返回 final_content 的竞态下，仍会生成幽灵文本并与编辑器正文叠加。根因是 `genesisAutoAcceptedRef` 仅在 ChapterSwitch 路径设置，无法覆盖 pipeline-complete 先完成的情况。
  - 新增 `isTextDuplicate` / `normalizeForDuplicateCheck` 纯文本去重工具（`src-frontend/src/frontstage/utils/isTextDuplicate.ts`），基于归一化后的最长指纹做双向包含检测。
  - 在 `FrontstageApp` 中提取 `isTextAlreadyInEditor` helper，统一检测编辑器当前内容是否已包含生成文本。
  - `pipeline-complete` effect 在加载 DB 正文后设置 `genesisAutoAcceptedRef.current = true`。
  - `handleRequestGeneration` 与 `handleSmartGeneration` 在设置 `generatedText` 前，先调用 `isTextAlreadyInEditor`；若已包含则跳过幽灵文本并标记 Genesis 已自动接受。
  - `appendAiContent` 改用共享的 `isTextDuplicate`，与设置幽灵文本的去重逻辑保持一致。

### 测试

- 新增 `src-frontend/src/frontstage/utils/__tests__/isTextDuplicate.test.ts`：覆盖完全相同、前缀包含、部分重复、标点差异、HTML 标签等场景。
- 更新 `FrontstageApp.pipeline-loop.test.tsx` 的 `MockRichTextEditor` 为 `forwardRef`，提供 `getText`/`appendText`/`setContent` 以支持去重检测测试。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**138 passed / 3 skipped**

## [v0.26.7] - 修复 React #185 无限循环与 Genesis 第一章重复（2026-07-04）

### 修复

- **修复 React #185 页面崩溃（根因）**：`FrontstageApp` 中监听 `pipeline-complete` 的 `useEffect` 依赖了未 memo 的 `selectChapter`，导致每次渲染都重新执行 effect → 无限 `setState` → React 报 `Maximum update depth exceeded`。已将该 effect 改为通过 ref 读取最新状态，并增加 `lastPipelineCompleteRef` 单次处理守卫，彻底切断循环。
- **修复 Genesis 新小说第一章重复**：`loadStories` 在 Genesis 新故事异步装配期间可能自动选择第一个 story 并加载 DB 正文，而此时 `generatedText` 仍持有同样的第一章文本，造成「DB 正文 + 幽灵文本」叠加。新增 `isGenesisSettingUpRef` 守卫，装配期间禁止自动选择 story。
- **稳定化关键回调引用**：`setGeneratedText`、`loadStoryScenes`、`loadStoryChapters`、`selectChapter`、`selectStory`、`loadStories` 全部用 `useCallback` 或 ref 稳定化，避免下游 hook/effect 因依赖不稳定函数而反复触发。
- **修复 mount effect 重复订阅**：`loadStories` / `setupEventListeners` 的初始化 effect 改为只在 mount 执行一次，并显式禁用 `react-hooks/exhaustive-deps` 警告，避免每次依赖变化都重新订阅事件。

### 测试

- 新增 `FrontstageApp.pipeline-loop.test.tsx`：
  - 验证 genesis pipeline complete 不会导致无限渲染循环。
  - 验证 Genesis 装配期间触发 `dataRefresh` 不会自动选择 story 加载正文。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**138 passed / 3 skipped**

## [v0.26.6] - 彻底修复第一章重复与页面崩溃（2026-07-04）

### 修复

- **修复 frontstage-update 事件类型匹配错误（根因）**：`FrontstageApp` 监听 `frontstage-update` 事件时，所有 `switch case` 使用了 PascalCase（`'ChapterSwitch'`、`'ContentUpdate'`、`'AppendContent'`、`'DataRefresh'`、`'SaveStatus'`），但后端通过 `#[serde(tag = "type", content = "payload", rename_all = "camelCase")]` 序列化后实际类型为 `chapterSwitch`、`contentUpdate`、`appendContent`、`dataRefresh`、`saveStatus`。这导致所有 frontstage-update 事件被静默忽略，是「第一章内容未正确加载/重复」与「写完后状态异常崩溃」的深层根因。已统一修正为 camelCase。
- **修复 Genesis 第一章内容重复**：当 `ChapterSwitch` 以 `auto_accept=true` 自动加载正文到编辑器后，新增 `genesisAutoAcceptedRef` 标记，禁止后续 `smart_execute` 结果再把 `final_content` 恢复为幽灵文本，避免「编辑器一份排版正文 + 幽灵文本一份纯文本」的重复渲染。
- **修复 `handleRequestGeneration` 的 Genesis 路径**：此前该路径在 `isFirstChapterReady` 时把 `generatedText` 清空后直接返回，若 `ChapterSwitch` 未加载内容则用户看不到正文。现在会正确将 `final_content` 设为幽灵文本供 Tab 接受。
- **修复 `loadStoryWordCount` 崩溃**：后端返回 `undefined` 或异常时，访问 `result.total_chars` 会抛出未捕获 `TypeError`。已改为 `result?.total_chars ?? 0`。
- **修复 `onChapterUpdated` 不必要的状态更新**：`setContent` 回调在 `prev === formatted` 时仍返回 `formatted`，导致无意义重渲染。现在相同内容时返回 `prev`。
- **修复 `selectChapter` 懒加载无限递归**：对缺少 `content` 的章节按需调用 `get_chapter` 后递归调用 `selectChapter`；若数据异常返回的章节仍无 content，会无限循环。已增加 `lazyLoadingChapterIdsRef` 集合，同一章节只尝试懒加载一次。

### 测试

- 修正 `FrontstageApp.genesis-duplicate.test.tsx`：
  - 事件名称从错误的 `frontstage-event` 改为真实的 `frontstage-update`。
  - 事件结构改为后端真实序列化结构 `{ type: 'chapterSwitch', payload: { ... } }`。
  - 新增「ChapterSwitch 自动接受正文后不再恢复 generatedText」测试。
  - 新增「`get_story_word_count` 返回 undefined 时不抛未捕获异常」测试。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**136 passed / 3 skipped**

## [v0.26.5] - 彻底切断 React #185 同步循环 + 正文完整性修复（2026-07-04）

### 修复

- **React #185 最终防御**：v0.26.3 的 `useBackendActivityStore(selector) + setTimeout(...,0)` 在部分高频进度事件场景下仍被触发。v0.26.5 改为 Zustand 原生 `subscribe`（回调在 React 渲染周期外执行），并加入 100ms 防抖 + `startTransition`，确保 `backendActivityStore → isGenerating` 的同步永不进入 React 同步 setState 链。
- **幽灵文本重复检测增强**：v0.26.4 的指纹 LCS 检测仍受限于只比较前 500 字符。v0.26.5 增加完整文本直接包含检测（归一化空白后），覆盖"编辑器含用户提示词 + 完整正文 / 幽灵文本只有正文片段"的场景，进一步避免重复显示。
- **`sanitize_novel_output` 尾部截断只作用于末尾 20%**：此前元评论标记（如 `（第一幕结束`）出现在正文任意位置都会被截断，导致正文结尾被误伤。v0.26.5 将截断范围限制在文本末尾 20%（ clamp 100–1500 字符），保留正文完整性。

### 验证

- `cargo test --lib`：**632 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check`：通过
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**134 passed / 3 skipped**

## [v0.26.4] - 幽灵文本重复检测支持前缀/片段匹配（2026-07-04）

### 修复

- **幽灵文本重复检测改用最长公共子串（LCS）**：用户反馈幽灵文本往往是正文内容的前缀（结尾少一部分），原来的 `includes` 全匹配在这种情况下会失败，导致幽灵文本继续显示。v0.26.4 新增 `longestCommonSubstringLength`，当编辑器内容与生成内容指纹的重叠度 ≥80% 时就隐藏幽灵文本，覆盖前缀/片段场景。
- 保留 v0.26.3 的 React #185 修复和密集诊断日志。

### 测试

- 新增 `RichTextEditor.duplicate.test.tsx`：
  - 空编辑器显示幽灵文本
  - Tab 接受后幽灵文本从 DOM 移除
  - 相同 generatedText 已存在时不显示幽灵文本
  - 不同 generatedText 仍显示幽灵文本
  - 幽灵文本是正文前缀/片段时也不显示幽灵文本

### 验证

- `cargo test --lib`：**631 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**134 passed / 3 skipped**

## [v0.26.3] - React #185 最终修复：异步化 isGenerating 同步 + 幽灵文本重复兜底（2026-07-04）

### 修复

- **彻底切断 React #185 同步链**：v0.26.2 改用 selector 后仍 crash，因为 `backendActivityStore` 高频更新时，Zustand selector 触发的重绘与 `setIsGenerating` 仍可能落在 React 同一次批处理链中。v0.26.3 将 `setIsGenerating` 包装在 `setTimeout(..., 0)` 中异步执行，强制跳出 React 批处理，从机制上消除同步循环。
- **幽灵文本重复兜底**：`RichTextEditor` 渲染时计算 `editorContainsGeneratedText`；如果编辑器当前内容已包含 `generatedText` 的文本指纹，则强制不渲染幽灵段落，避免用户看到两份相同内容。
- **textFingerprint 提取为模块级函数**：供 setContent 去抖和幽灵文本重复检测共用。

### 验证

- `cargo test --lib`：**631 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**129 passed / 3 skipped**

## [v0.26.2] - 彻底修复 React #185：改用 Zustand selector 同步 isGenerating（2026-07-04）

### 修复

- **backendActivityStore 订阅改为 selector 派生**：`FrontstageApp` 不再使用 `useBackendActivityStore.subscribe` 回调，改为 `useBackendActivityStore(state => state.getIsAnyActive())` 派生布尔值，并在 `useEffect` 中同步到本地 `isGenerating`。这彻底消除了订阅回调在 React render 阶段触发 `setState` 的可能性，从架构上避免 React #185。
- 保留 v0.26.1 全部密集诊断日志，用于同时定位内容重复根因。

### 验证

- `cargo test --lib`：**631 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**129 passed / 3 skipped**

## [v0.26.1] - 内容重复问题诊断日志增强（2026-07-04）

### 诊断

- **RichTextEditor 密集诊断日志**：
  - 每次渲染记录 `generatedText` 长度、`isHidingGhost`、`isGenerating`、`hideGhostUntil` 剩余时间、编辑器 HTML/纯文本长度等关键状态。
  - 幽灵文本渲染条件（外层 `shouldShowGhostTree` / 内层 `shouldShowGhostParagraph`）求值结果写入日志。
  - `setContent` effect 记录入口、被 `hideGhostUntil` 拦截、跳过（same ref / fingerprint match）、执行等分支。
  - `onUpdate` / `onChange` debounce 触发记录。
  - `appendText` 追加前后增加内容预览和指纹信息。
- **FrontstageApp 诊断日志增强**：
  - `setGeneratedText` 每次调用记录文本长度、调用栈、post-accept lock 状态。
  - `handleAcceptGeneration` 记录编辑器接受前的文本长度和指纹。
  - `appendAiContent` 记录重复检测详情（现有内容预览、生成内容预览、是否已存在）。
  - `ChapterSwitch` 事件记录接收时的 `generatedTextRef` 长度和锁状态。
  - `backendActivityStore` 订阅记录 `isAnyActive` 变化，验证 v0.25.1 去抖效果。
- **UX 加固**：幽灵文本段落增加 `user-select: none`，即使异常渲染也不会被复制进剪贴板。

### 验证

- `cargo test --lib`：**631 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**129 passed / 3 skipped**

## [v0.26.0] - 数据飞轮 + Harness 可观测性 + 子代理协作（2026-07-04）

### 数据飞轮与共同进化

- `RecordFeedbackRequest` 与 `FeedbackEvent` 扩展 `original_prompt` / `generated_content` / `subsequent_edit_diff` 字段，让每一次接受/拒绝都携带完整上下文。
- 新建 `creative_engine/adaptive/preference_pair_exporter.rs`，将近期反馈导出为 `.storymoss/feedback/preference_pairs.jsonl`，格式兼容 RLHF / DPO 训练。
- `commands/intent::record_feedback` 直接构建 `FeedbackEvent` 并调用 `PreferencePairExporter::export`，写入工作空间反馈目录。
- 前端 `handleAcceptGeneration` / `handleRejectGeneration` 在 `FrontstageApp.tsx` 中自动收集原始提示词与生成内容，并透传 `subsequent_edit_diff`（接受后的编辑差异）。

### Harness 可观测性

- 新建 `tracing.rs`：定义 `TraceStep` / `GenerationTrace` / `TraceStore`，内存保留最近 200 条 trace，并持久化到 `app_data_dir/logs/traces/{trace_id}.json`。
- `TraceStore` 提供 `start_trace` / `add_step` / `finish_last_step` / `record_step_detail` / `associate_request_id` / `trace_id_for_request` 等 API。
- `GatewayRequest` / `GenerateRequest` / `LlmGeneratingProgress` / `ErrorResponse` 新增 `trace_id` 字段，实现从 orchestrator 到模型网关、LLM 适配器、前端进度事件的全链路透传。
- `AgentOrchestrator::generate` 启动 trace，并在 `execute_time_sliced` / `execute_trishot` 返回后将 LLM `request_id` 与 `trace_id` 关联。
- `GatewayExecutor::generate` 在 trace 存在时记录 `gateway.generate` 步骤，包括候选链、探测结果、候选失败与最终成功信息。
- `LlmService::emit_llm_progress` 通过 `request_id` 反查 `trace_id`，并将 `trace_id` 注入进度事件，前端可按 trace 聚合。
- 新增 IPC 命令 `get_generation_trace` / `list_recent_generation_traces`。
- 前端新增「生成链路」页面 `TracingPanel`，位于侧边栏「生成链路」，支持查看最近 30 条 trace、步骤树、耗时、模型、token 与错误详情。

### 子代理协作模型

- 新增 `Subagent` trait 与 `ReviewNotes` 抽象，统一子代理审查接口。
- 实现 `ContinuityAgent`（连续性检查：伏笔回收、角色一致性、时间线断裂）、`StyleAgent`（文风漂移、在世作者保护、Anti-AI 陈词）、`WorldAgent`（世界观规则、地理/组织一致性）。
- 新增 `PreGenerationGate` / `InGenerationChecker` / `MiniRewrite` 三个阶段检查，在生成前/中/后注入质量门。
- 子代理通过 `ReviewNotes` 汇总问题与建议，供后续 `AuditExecutor` 或 `MiniRewrite` 消费。

### 文件系统工作空间

- 新增 `workspace/` 模块与 `WorkspaceService`，启动时自动在应用数据目录下创建 `.storymoss/` 工作空间。
- 自动生成 `AGENTS.md` / `MEMORY.md` / `LOOPS.md` / `PROGRESS.md` 等上下文文件，为 AI 协作与长期记忆提供锚点。
- 集成 `git2` 进行自动 `git init`、初始提交与后续变更自动提交，使创作过程可追踪、可回滚。
- 新增 `commands/workspace.rs` 提供工作空间初始化与状态查询命令。
- `AssetCapabilityManifest` 支持按 `AssetTaskType` 懒加载，减少启动时加载大量资产的能力描述。

### 验证

- `cargo test --lib`：**631 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npx vitest run`：**129 passed / 3 skipped**
- `npm run format:check`：零差异
- `cargo +nightly fmt -- --check`：通过
- 版本号统一至 `v0.26.0`（`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json`）
- `README.md` / `ARCHITECTURE.md` / `AGENTS.md` 已同步 v0.26.0 内容

## [v0.25.1] - 修复 React #185 无限渲染 + 统一版本号（2026-07-03）

### 修复

- **backendActivityStore 订阅去抖**：`FrontstageApp` 中对 `useBackendActivityStore` 的订阅原本监听整个 store 状态，每次后台进度事件（`pipeline-progress` / `generation-status` 等）都会触发 `setIsGenerating`。当后台任务密集时，连续状态更新可能触发 React #185（Maximum update depth exceeded）。v0.25.1 改为在回调内缓存 `getIsAnyActive()` 的上一次结果，仅当布尔值真正变化时才调用 `setIsGenerating`，显著降低重绘频率。
- **版本号统一**：修正 `src-tauri/tauri.conf.json` 仍停留在 `0.24.9` 的问题，与 `package.json`、`Cargo.toml` 统一为 `0.25.1`。

### 验证

- `cargo test --lib`：**611 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**129 passed / 3 skipped**

## [v0.25.0] - Context Rot 显式防御 + 四级错误分类恢复（2026-07-03）

### Context Rot 显式防御

- 新增 `creative_engine/context_prioritizer.rs`：把系统提示词拆分为 `ContextChunk`，按 `Critical / High / Normal / Background` 四级优先级排序，并把 Critical 约束同时前置和后置（轻量摘要），减少长系统提示词中的 "Lost in the Middle"。
- `build_writer_prompt` 全面改用 `ContextChunk` 收集各系统提示词段落，最后调用 `prioritize_system_prompt` 生成最终系统提示词。
- `ContextHealthMetrics` 记录 `critical/high/normal/background` token 数、总 token 数、预算使用率、Critical 信息丢弃数量。
- `DiagnosticStore` 扩展 `context_health` 字段，新增 `get_context_health` / `set_context_health`。
- 新增 `llm::commands::get_context_health` 命令并注册到 `handlers.rs`。
- 前端诊断卡片在 `captureDiagnosticInfo` 中调用 `get_context_health`，显示上下文健康指标。

### 四级错误分类与恢复

- `AppError` 新增 `ErrorSeverity`（Fatal / Retry / Degraded / UserAction），并为每种错误变体提供 `severity()` 分类。
- `ErrorResponse` 新增 `severity` 字段，前端 `parseStructuredError` 可读取该字段。
- 新建 `error_recovery.rs`：提供 `retry_with_backoff` 与 `with_degraded_fallback`，`RecoveryOutcome` 区分首次成功 / 重试成功 / 降级成功 / 失败。
- `commands/orchestrator.rs` 的 `smart_execute` 初始上下文加载（stories + chapters）外层包裹 `retry_with_backoff`，容忍偶发 DB 锁定。
- `model_gateway/executor.rs` 的 `GatewayExecutor::generate` 对每个候选模型调用外层包裹 `retry_with_backoff`（默认 1 次重试、200–2000 ms 退避）。
- `llm/service.rs` 的连接阶段超时映射为 `AppError::LlmConnectionTimeout`（severity=Retry），让网关重试真正生效。
- 前端 `utils/errorHandler.ts` 的 `resolveUserFacingMessage` 按 severity 兜底，返回 `retry / upgrade / degraded / none` 动作。
- 新建 `AgentInterruptionModal` 组件：Fatal 错误显示“创作引擎遇到致命错误”，UserAction 错误显示“需要您先处理”，并支持一键打开幕后设置。
- `FrontstageApp` 的 `handleRequestGeneration` 与 `handleSmartGeneration` 错误捕获中，对 `severity === Fatal || UserAction` 直接打开 `AgentInterruptionModal`，不再只显示通用诊断卡片。

### 验证

- `cargo check --lib`：通过
- `cargo test --lib`：**611 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `cargo +nightly fmt -- --check`：通过

## [v0.24.9] - TipTap 渲染错误边界 + 接受后 30s 禁止外部 setContent + 重复根因诊断（2026-07-03）

### 修复

- **EditorContent 局部错误边界**：在 `RichTextEditor` 内为 `EditorContent` 增加 `EditorContentBoundary`，TipTap 渲染异常时仅编辑器区域降级为“重试”按钮，避免整个 Frontstage 被顶层 ErrorBoundary 捕获白屏；错误详情写入 `frontstage:crash:tiptap_render`。
- **接受后 30s 禁止外部 setContent**：`RichTextEditor` 的外部同步 effect 在 `hideGhostUntil` 期间直接返回，避免后台 `ChapterCommitted` / `DataRefresh` / `onChapterUpdated` 等同步事件在 Tab 接受后立即重写编辑器内容，减少内容重复和渲染异常风险。
- **ErrorBoundary 日志增强**：记录 `name`、`message`、`stack`、`componentStack` 四个字段，便于 source-map 缺失时也能判断错误类型。
- **追加后重复检测日志**：`appendText` 追加成功后通过 `queueMicrotask` 检查编辑器纯文本中是否出现追加内容两次以上，写入 `frontstage:append_text_check`，帮助确认“内容重复”发生在追加阶段还是渲染阶段。
- **setContent/loadAggregatedScenes 异常兜底**：所有直接调用 `editor.commands.setContent` 的路径补 try-catch，防止单次命令异常扩散为整页白屏。

### 验证

- `cargo test --lib`：**599 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**129 passed / 3 skipped**

## [v0.24.5] - 幽灵文本从 React 树中彻底移除，根治 Tab 接受后内容重复（2026-07-03）

### 修复

- **幽灵文本整棵移除**：之前外层 `editor-ghost-continuation` 仍依赖 `generatedText || isGenerating`，Tab 接受后只要 `isGenerating` 仍为 true，容器就会继续占位；内部段落虽被 CSS 隐藏，但容器内若残留任何渲染物都会造成“内容重复”。v0.24.5 改为外层条件也判断 `!isHidingGhost`，Tab 接受后整棵幽灵树从 React 树中移除，不再依赖 CSS 兜底。
- **保留永久隐藏兜底**：`body.force-hide-ghost` 仍保留，作为双保险。

### 验证

- `npx tsc --noEmit`：零错误
- `npx vitest run`：**129 passed / 3 skipped**
- `npm run format:check`：零差异

## [v0.24.4] - 后台 mini_review 静默化 + 幽灵文本永久隐藏 + 崩溃诊断增强（2026-07-03）

### 修复

- **后台 `mini_review` 静默化**：`SceneCommitService::auto_commit` 在正文返回后调用的 `mini_review` 此前未进入静默列表，每 10s 向前端发射心跳，导致后台任务期间前端持续重绘。现已加入 `is_silent_background`，不再向前端发送进度/心跳事件。
- **幽灵文本永久隐藏**：Tab 接受后原本 30s 会移除 `body.force-hide-ghost` 类，若 DOM 移除竞态则幽灵文本会重新露出（v0.24.3 已移除调试标记，视觉上与正文完全一致，造成“重复”错觉）。现改为永久隐藏，直到新一次生成开始才自动解除。
- **ErrorBoundary 后端日志**：React 错误边界捕获的异常现在通过 `log_frontend_event` 写入 `creative_workflow.log`（`frontstage:crash:error_boundary`），便于无 devtools 时定位渲染崩溃。
- **前端心跳与内存快照**：`frontstage/main.tsx` 每 30s 记录一次 `frontstage:heartbeat`，并附带 `performance.memory` 的 JS 堆内存用量；页面 unload 前记录 `frontstage:crash:beforeunload`，帮助判断 WebKit 进程是否重启。

### 验证

- `cargo check`：零错误
- `cargo test --lib`：**599 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit`：零错误
- `npm run format:check`：零差异
- `npx vitest run`：**129 passed / 3 skipped**
- `npm test`：**34 passed / 5 skipped**
- `python3 scripts/architecture_guard.py`：通过

## [v0.23.74] - 场景优先架构迁移——Scene 成为唯一叙事真相源（2026-06-28）

### 架构变更

四阶段完整迁移，对齐 CONTEXT.md 设计意图，结束 chapter-centric 到 scene-centric 的过渡期：

**Phase 1 — 消灭内容双写**：`scenes.content` 成为唯一叙事真相源。`ChapterRepository::update()` 移除 content 参数、移除 cascade 到 scenes 的同步写入。`SceneRepository::update_in_tx()` 移除 reverse-cascade 到 chapters。`ChapterRepository::create()` 不再向 chapters 表写入 content。新增 `get_content(chapter_id)` 从 scenes 聚合。`get_by_id/get_by_story` 自动 fallback 到 scenes。`total_content_length_by_story()` 改为查询 scenes 表。`ChapterRepo` trait 签名更新。

**Phase 2 — 前端编辑器切到 Scene**：`frontstageStore` 主键从 `chapterId` 改为 `sceneId`，`setChapterInfo` → `setSceneInfo`。自动保存从 `update_chapter` 迁移到 `update_scene`。`ChapterSwitch` 事件新增 `scene_id` 字段。`FrontstageEvent` 类型同步更新。

**Phase 3 — Commit 触发点迁移**：`SceneService::on_scene_updated()` 新增 `SceneCommitDebouncer`（30s 空闲防抖），接管 auto_commit。`ChapterService::on_chapter_updated()` 移除 `ChapterCommitDebouncer`。Vector `record_type` 从 `"chapter"` 改为 `"scene"`。

**Phase 4 — 创世场景化**：

- 新增 `narrative_first_scene_generate` 提示词模板（14 个场景级变量：dramatic_goal、conflict_type、characters_present、setting_location/time/atmosphere、scene_outline 等）
- `first_scene_prompt()` 函数注入完整场景戏剧结构
- Genesis `FirstChapterGenerationStep` 调用场景级模板
- `SceneOutline` 扩展：新增 `characters_present`、`setting_time`、`setting_atmosphere`、`outline_content`
- `WriteTimeBundle::to_prompt()` 渲染新增场景字段

**幕前纯正文**：移除 TipTap `SceneDividerNode` 扩展，幕前编辑器仅显示无缝拼接的纯正文（如成品书页）。新增 `get_chapter_aggregated_content` 命令。

**前后台同步增强**：`SceneUpdated` 事件新增 `content_changed: bool` 字段，区分内容变更与元数据变更。

### 验证

- `cargo check`：零错误
- `cargo test --lib`：**592 passed / 0 failed / 2 ignored**（基线 582 + 新增 10）
- `npx tsc --noEmit`：零错误
- 29 files changed, 580 insertions(+), 126 deletions(-)

## [v0.23.66] - 模型角色分配 × 后台并发根治（2026-06-28）

### 模型角色分配：三层默认模型

- **背景**：此前只有一个 `active_llm_profile`，所有任务（正文生成/路由分析/后台审计）全部分配给同一模型——资源分配不均，创作模型被轻量探测占满
- **新增**：三种模型角色——🎨 **创作模型**（正文生成、Writer、改写）、🔧 **工具模型**（Call 1 路由、探测、JSON 提取）、⚙️ **后台任务模型**（BGP 审计/入库/洞察、Genesis 后台流水线）
- **网关调度**：`select_candidates` 按请求角色偏好选择对应模型并强制置顶；未设置时自动分配（快模型→工具，闲置→后台，创作回退 active）
- **前端 UI**：`UnifiedModelManager` 顶部新增「模型角色分配」卡片，三个下拉框可选所有聊天模型或"自动分配"；`ModelCard` 显示角色徽章（琥珀=创作/蓝=工具/紫=后台）
- **修改**：`config/settings.rs` ModelRole 枚举 + AppConfig 三字段；`model_gateway/executor.rs` `resolve_role_model` + 自动分配逻辑；`config/commands.rs` `set_active_model` 支持 role 参数；前端 5 文件（types/settings/context/components）

### 后台并发过载根治：Genesis 后台流水线串行化

- **根因**：v0.23.64 代码中 `ParallelWorldOutlineCharacterStep` 使用 `tokio::join!` 3 路并发（世界观+大纲+角色），加上 BGP-4 同时发射，共 4 个 LLM 调用打向同一本地 26B 模型 → 模型过载 → `INTERNAL_ERROR` 洪流 → 前端页面崩溃/空白
- **修复**：`tokio::join!` 改为串行 `.await` + 外层 `BACKGROUND_LLM_SEMAPHORE` 保护；BGP-4 `run_insight` 前加信号量；Genesis 后台 spawn 入口加信号量。任何时刻最多 1 个后台 LLM 调用
- **修改**：`narrative/genesis.rs`（3 路并发→串行）、`agents/orchestrator.rs`（BGP-4 信号量）、`commands/orchestrator.rs`（后台 spawn 信号量）

### 历史修复（本迭代）

- **v0.23.64-65 的并行修复**：正文重复渲染去重、`sanitize_novel_output` 思考链/规划块剥离、UTF-8 字节边界 panic 修复、探测日志静默化

- 验证：`cargo test --lib` **582 passed / 0 failed / 2 ignored**；`cargo check` ✓；`npx tsc --noEmit` ✓；`cargo +nightly fmt -- --check` ✓

## [v0.23.65] - 提示词工程全链路修复（2026-06-27）

### P0-1/P0-2：`writer_system` 全链透传（最高优先级）

- **根因**：`writer_system`（7 条写作准则）此前仅在 Full 路径（`build_writer_prompt`）生效，TriShot Call 3 和 TimeSliced（默认续写，占 90%+ 流量）完全旁路——80+ 高质量提示词在默认路径被系统性绕过
- **修复**：打通 system_prompt 全链路——orchestrator → `generate_for_task*` → `GatewayRequest` → `execute_generation` → `GenerateRequest` → 适配器。新增三级优先级：每模型 `system_prompt_override` > adapter 默认 > PromptRegistry `writer_system` 默认
- **修改**：`GatewayRequest` +`system_prompt` 字段；`LlmService` 全族函数增 `system_prompt: Option<String>` 参数；Ollama 适配器 `system_prompt` 前置拼接；`pipeline/refine.rs`/`review.rs`/`model_gateway/dispatcher.rs` 参数对不齐修复

### P0-3：选中资产正文回灌

- **根因**：Call 1 返回 `selected_asset_ids`（如 `beat_card.*`、`story_engine.*`、`pressure_relationship.*`），此前只被转为路由标签用于模型网关调度，资产内容（`function`/`when_to_use`/`remix_hint`/`avoid`/`core_payoff`）从未到达 Writer
- **修复**：新增 `render_selected_asset_guidance`——从 `AssetCapabilityManifest` 回查资产完整内容，格式化为紧凑创作指导文本，注入 TriShot Call 3 的 system_prompt（限 5 条，控制 token 预算）。同时消费 `FrameworkSelections.prompt_hints`
- **修改**：`agents/service.rs` 新增 4 个辅助函数（`render_selected_asset_guidance` / `format_single_asset_guidance` / `truncate_str` / `resolve_prompt_for_hint`）；`orchestrator.rs` TriShot 路径 system_prompt 三级合并

### P1-2：LivingAuthorGuard 注入所有 Writer 路径

- **修复**：`render_writer_system_from_bundle` 内置在世作者名清除（41 位作者黑名单，替换为"具备相同手工艺特征的写作风格"）+ 手工艺滑块（5 维 × 3 档：句长偏好 / 对话比例 / 比喻密度 / 内心独白比例 / 视角粘度），此前只在 Full 路径 `build_writer_prompt` 生效
- **修改**：`agents/service.rs` `render_writer_system_from_bundle` 增强

### P1-3：Anti-AI cliché 避免指令注入

- **修复**：27 个 AI 高频陈词滥调（不言而喻、显而易见、总的来说、嘴角微微上扬…）作为"反 AI 味写作指令"注入所有 Writer 路径的 system_prompt
- **修改**：`agents/service.rs` `render_writer_system_from_bundle` 追加反 cliché 段

### P2：模板注册表修复

- **P2-5**：`orchestrator_timesliced_writer` 注册表补全 `continuation` 变量声明（此前代码注入了但注册表未声明，前端编辑器不可见）
- **P2-1**：模板默认内容新增 `{{continuation}}` 变量使用（此前只在硬编码 fallback 路径使用）

- 验证：`cargo test --lib` **582 passed / 0 failed / 2 ignored**；`cargo check` ✓；`npx tsc --noEmit` ✓；`cargo +nightly fmt -- --check` ✓

## [v0.23.64] - 续写内容质量根因修复（2026-06-27）

### P0：Writer LLM 看不到前文正文 → 每次续写生成全新故事

- **根因**：TimeSliced（默认续写）路径 `execute_time_sliced` 完全不读取 `current_content`，Writer 只收到 `WriteTimeBundle` 结构化约束；TriShot 路径 `current_content` 截断 6000→600 字只给 Call 1 做意图检测，Call 3 Writer 看不到任何原始正文
- **修复**：新增 `build_continuation_context` 函数，智能构建续写上下文（近章摘要 3 章各 300 字 + 当前章尾部预览 2000 字），注入 TimeSliced 和 TriShot 路径。升级 `MemoryWriter::extract_summary` 从 200 字前缀改为 300 字尾部（续写需要最近发生的事而非开头）

### P1：sanitize_novel_output 清洗不足 → 规划 markdown 泄漏到正文

- **根因**：`sanitize_novel_output` 不剥离推理模型思考链（`<thinking>`）、`+++++` 文件分隔符、编号规划块（`1. 世界观设定`）、markdown 代码块
- **修复**：在现有 4 步清洗前增加第 0 步（4 个子步骤）：剥离思考链（复用 `strip_reasoning_blocks`）、`+++++` 分隔符截断、编号规划块检测（连续 3+ 行含规划关键词）、markdown 代码块删除。新增 4 个单元测试

### P2：前端幽灵文本与编辑器内容同时渲染 → 内容重复两份

- **根因**：`AppendContent` 事件处理 `setContent(prev => prev + formatted)` 无去重守卫；bootstrap 回退路径 `setContent(autoFormatText(result.final_content))` 不清空 `generatedText`
- **修复**：`AppendContent` 追加前检查尾部 200 字是否已存在；bootstrap 回退路径前清空 `generatedText`

- 验证：`cargo test --lib` **582 passed / 0 failed / 2 ignored**；`cargo check` ✓；`npx tsc --noEmit` ✓

## [v0.23.63] - 创世静默挂死根治 + 探测日志洪流根治（2026-06-27）

### P0：byte-slice UTF-8 边界 panic 根治

- **根因（日志确认）**：v0.23.62 创世流程中，`trishot.call3.done`（作家模型生成完成）正常发射后，第二条 `trishot.call3.done`（正文已生成）从未出现。两者之间的 `&content[..content.len().min(120)]` 在多字节 UTF-8 边界（中文 3 字节/字符）**panic**——Rust 字符串字节索引必须落在 char 边界。`tokio::time::timeout` 无法捕获 panic → `execute_trishot` tokio task 静默崩溃 → `smart_execute` 600s 超时 → 前端报"三击生成完成"但 `currentStory` 为 null
- **修复**：全局 9 处 `&str[..str.len().min(N)]` 字节切片改为 `str.chars().take(N).collect::<String>()`，字符边界安全。覆盖：
  - `agents/orchestrator.rs`（trishot.call3.done 日志）
  - `narrative/genesis.rs`×2（first_chapter.generated + chapter_switch.sent 日志）
  - `llm/service.rs`（stream prompt debug 日志）
  - `book_deconstruction/analyzer.rs`（JSON 解析错误信息）
  - `commands/orchestrator.rs`（final_content_preview 日志）
  - `task_system/audit_executor.rs`×2（Inspector 响应无 JSON + JSON 解析失败日志）
  - `db/migrations.rs`（幂等跳过日志）

### P1：探测调用日志静默化

- **根因（日志确认）**：v0.23.60 后台 keepalive 每 10s 探测 3 个模型，走完整 LLM 生成路径。虽然 `is_silent_background=true` 跳过了 `emit_llm_progress` 和心跳，但 `workflow_log`（`llm.generate.start`/`llm.heartbeat.*`/`llm.record_call.*`/`llm.emit_completed.*`/`llm.generate.completed`/`llm.generate.return_ok`）和 `record_llm_call`（DB INSERT）**未被跳过** → 每 10s 产生 ~24 行噪声日志 + 3 条 DB 记录，27361 行日志中 17832 行是探测噪声
- **修复**：`execute_generation` 中所有 `workflow_log` 调用和 `record_llm_call` 调用全部加 `if !is_silent_background` 守卫，探测调用零日志、零 DB 写入

- 验证：`cargo test --lib` **578 passed / 0 failed / 2 ignored**；`cargo check` ✓；`npx tsc --noEmit` ✓

## [v0.23.61] - 系统提示词可配置 + 第一章注册表化 + 框架级智能路由（2026-06-27）

### Gap 1：第一章正文指令注册表化

- **根因**：`genesis.rs:634-654` 一个大段 `format!()` 硬编码，不经过 PromptRegistry，用户和后台都无法编辑
- **修复**：新增 `narrative_first_chapter_generate` 到 PromptRegistry（Creation 分类，15 个模板变量）；新增 `first_chapter_prompt()`（`narrative/prompts.rs`）走 `resolve_and_render` 模式；`genesis.rs` 改为调用 `first_chapter_prompt()`
- 后台「提示词注册表」页面可编辑覆盖

### Gap 2：系统提示词覆盖真正生效

- **根因**：`writer_system_prompt_override` 字段定义了、存了但从不被消费。OpenAI/Anthropic adapter 硬编码 `"You are a professional creative writing assistant."` 覆盖掉 PromptRegistry 的 `writer_system` 中文指令。MN-Oblivion 的模型卡指令无法激活
- **修复**：`LlmProfile.system_prompt_override` → `GenerateRequest.system_prompt` → adapter 改用 `req.system_prompt` 替代硬编码。优先级：每模型 > adapter 默认。前端 ModelModal 新增「系统提示词覆盖」多行文本框

### Gap 3：框架级智能提示词路由

- **根因**：84 个提示词无路由逻辑，每个步骤用写死的 prompt_id。没有任何代码根据题材/意图/叙事阶段选择提示词
- **修复**：新增 `FrameworkSelections` 结构体（methodology/quality_gate/contextual_injectors/prompt_hints）。`SynthesisResult` +`framework_selections` 字段。Call 1 最快模型收到 `build_prompt_framework_catalog()`（5 种方法论/3 种质量门/3 种注入器），输出 `framework_selections`。`trishot_synthesizer` prompt 扩展输出格式

- 验证：`cargo test --lib` **578 passed / 0 failed / 2 ignored**；fmt ✓；tsc ✓

## [v0.23.60] - 网关探测异步化 + 调度退避 + 并发限流 + 卡死诊断（2026-06-27）

### 网关探测异步化

- **根因**：每条 gateway 调用嵌入 5s 内联预探测，运行时每调用 +5s 延迟
- **修复**：后台 keepalive 每 10s 刷新健康/降级模型（5s 超时）；`GatewayExecutor::is_health_fresh()` 检查 `last_checked_at` <15s → 跳过内联探测；`generate_with_fastest` 同样适用。正常运行时 0ms 附加开销

### 死模型退避

- **根因**：MN-Oblivion 等死模型持续失败 4h+，每 60s 浪费 2 个模型 × 1s 探测
- **修复**：重写 `scheduler.rs` 三轮架构：启动全量 + keepalive 10s + retry backoff。连续失败 ≥3 → 指数退避 30→60→120→…→3600s

### 后台 LLM 并发限流

- **根因**：Call 3 后 BGP-1+BGP-3+ingest 3 个 LLM 调用同时发射，竞争 1 个健康模型
- **修复**：新增 `BACKGROUND_LLM_SEMAPHORE(1)`，BGP-1/BGP-3 受信号量限流串行化

### post-call3 卡死诊断

- **根因**：最新创世流程 `trishot.call3.done` 后 56s 零 genesis 事件
- **修复**：`execute_trishot` 返回前、`orchestrator.generate` 返回前、DB 保存前后添加 `log::warn!` 诊断点

- 验证：`cargo test --lib` **578 passed / 0 failed / 2 ignored**；fmt ✓；tsc ✓

## [v0.23.59] - 全面修复并强化模型网关调度（2026-06-27）

### 修复：创世流程 5 个 LLM 调用中 4 个绕过网关，死模型挂起 300s 无候选切换

- **根因（审计确认）**：创世流程 5 个 LLM 调用中，只有 TriShot Call 3 经过网关（带 5s 预探测 + 候选 fallback）。故事概念生成、Call 1 路由合成、5 个后台 pipeline 步骤（世界观/大纲/角色/场景/伏笔）全部绕过网关——直接走 `select_profile_for_request` + 单适配器，死模型挂起 300s 直到 LLM 超时，无候选切换。

### 修复：真实调用失败对调度器完全不可见

- **根因（审计确认）**：候选循环中，真实调用失败只 `continue` 到下一候选，不更新健康注册表。只有预探测失败才标记 Unhealthy。而预探测成功≠真实调用成功——模型能说 "OK" 但无法生成正文时，下次 `generate()` 又强制置顶该模型（`+1000` 分）。`Degraded` 状态从未被任何路径写入，`-20` 惩罚形同虚设。

### 变更

#### Fix 1：所有创世 LLM 调用路由到网关

- `generate_for_request_with_context_and_pipeline`（service.rs）改为委托 `generate_for_request_with_request_id`（网关路径，自带 5s 探测 + 候选 fallback + 内部直接适配器兜底）。单点修改覆盖概念生成 + 5 个后台 pipeline 调用点。
- 从 `context_label` 派生 `intent_verb`/`intent_object`（新增 `derive_intent_from_label` 助手），激活网关意图感知分类（`classify_by_intention`）：生成概念→HeavyCreation，提取/分析→LightTool，让模型路由更精准。
- `pipeline_ctx`（心跳步骤前缀）不再透传——后台步骤均在 `is_silent_background` 中不发射心跳，概念步骤的 `context_label` 已足够标识当前阶段。

#### Fix 2：`generate_with_fastest` 增加 5s 探测 + 候选 fallback

- 此前直接调适配器，死模型挂起 300s 无候选切换。现在探测通过才直接调用（保留最快模型速度优势），探测失败则标记 Unhealthy + 递增连续失败计数，回退到网关候选链（自带探测 + fallback）。
- 新增 `probe_profile_quick` 私有助手（与网关候选循环内探测逻辑一致）。
- 新增 `GatewayExecutor::mark_unhealthy` / `record_success_public` 公开接口供 LlmService 调用。

#### Fix 3：活跃模型连续失败降级

- **健康注册表增加连续失败计数**（health.rs）：`HealthRecord` +`consecutive_failures` 字段；新增 `record_failure`/`record_success`/`consecutive_failures` 方法；`apply_probe_result` 同步维护计数。7 个新单元测试覆盖。
- **网关候选循环跟踪真实调用成败**（executor.rs）：预探测成功→`record_success` 重置计数；预探测失败/超时→`record_failure(Unhealthy)` 替换直接 `guard.update()`；**真实调用失败→`record_failure(Degraded)`**（关键变更，让真实调用失败对调度器可见，Degraded 保留在候选池受 -20 惩罚但不再被强制置顶）；真实调用成功→`record_success` 重置计数。
- **强制置顶增加降级门控**（executor.rs）：新增 `active_model_demoted()` 方法 + `ACTIVE_MODEL_DEMOTION_THRESHOLD = 2` 常量。3 个强制置顶点（`select_candidates` force-promote、`generate()` re-promote、`select_fastest_profile` short-circuit）在连续失败≥2 时跳过强制置顶，让其他健康候选接管。降级时记录 workflow_log。成功 1 次即清零恢复。

#### Fix 4: TimeSliced 写作策略硬编码替换为用户真实配置

- **根因**：诊断日志显示 `运行模式：标准\n冲突强度：0.5\n叙事节奏：正常\nAI 自由度：0.5` 是 `WriteTimeBundle::load_sync()` 的硬编码字符串，`execute_time_sliced()` 从未从 `AppConfig` 读取用户的真实 `WritingStrategy` 设置。
- **修复**（orchestrator.rs）：`execute_time_sliced()` 在 bundle 加载后从 `AppConfig::load()` 读取 `writing_strategy`，用 `format_writing_strategy_constraints()`（新增于 `write_time_bundle.rs`）生成约束文本，覆盖 `bundle.writing_strategy_constraints` 的硬编码默认值。加载失败时保留默认值（优雅降级）。
- 覆盖 Full 模式和 Genesis 路径——它们通过 `build_writer_prompt` 早已正确读取 `writer_app_config()`。

- 验证：`cargo check` 零错误；`cargo test --lib` **578 passed / 0 failed / 2 ignored**（571 基线 + 7 新增）；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误

## [v0.23.54] - 创世正文重复 + 页面崩溃根治（ErrorBoundary + autoFormatText + generatedText 安全网）（2026-06-27）

### 修复：创世第一章正文重复显示（排版版 + 纯文本版）

- **根因**：`onChapterUpdated` 后台同步用 `updated.content`（原始纯文本）覆盖编辑器，而 ChapterSwitch 用 `autoFormatText(content)`（HTML 排版版）。两者交替覆盖导致排版丢失。同时 `generatedText` 幽灵段落若残留正文，会与编辑器正文并存 → 两份重复。
- **修复**：
  - `onChapterUpdated` 改用 `autoFormatText(updated.content)` 格式化，与 ChapterSwitch 保持一致
  - 新增 `isGenerating` true→false 转换时清空 `generatedText` 的安全网 useEffect，确保任何路径遗漏都不残留幽灵文本

### 修复：创世后前端页面崩溃空白（需右键 reload 恢复）

- **根因**：渲染崩溃无 ErrorBoundary 捕获 → 白屏。
- **修复**：幕前 `main.tsx` 用 `ErrorBoundary` 包裹 `FrontstageApp`，崩溃时显示恢复 UI（错误信息 + 刷新按钮）而非空白页面

### 修复：CI fmt 差异

- `narrative/mod.rs` 注释换行 + 测试字符串换行对齐 nightly rustfmt

## [v0.23.52] - 创世后页面崩溃根治：消除重复 Completed 事件 + 生成中跳过 chapterUpdated（2026-06-26）

### 修复：创世正文输出后前端页面崩溃空白需 reload 恢复

- **症状**：创世（TriShot）成功返回"三击生成完成"后，前端页面崩溃空白，需右键 reload 恢复。诊断显示"距最后一次进度事件 458 秒"，作品和第一章实际已创建成功（编辑器 2011 字），但 `smart_execute` invoke 未 resolve，前端等 600s 超时。
- **根因**：`generate()` 在 `execute_trishot` 返回后重复发射状态事件——`execute_trishot` 已在 Call 3 完成时 emit `GenerationPhase::Completed`（"三击生成完成"），但 `generate()` 又 emit 了 `SavingMemory`（"保存记忆..."）和第二个 `Completed`（"创作完成"）。前端 `mainGenerationCompletedRef` 在第一个 Completed 时置 true，但随后的 SavingMemory 覆盖了"已完成"状态，导致状态不一致。同时后台 `auto_commit` 的 `chapterUpdated` 事件在生成期间到达，与 ChapterSwitch 加载正文竞争，`onChapterUpdated` 用原始内容（未 autoFormatText）覆盖编辑器刚加载的 HTML 内容，触发渲染竞态。
- **修复**：
  - **后端**：`generate()` 在 TriShot 模式下跳过重复的 `SavingMemory` 和 `Completed` emit（`execute_trishot` 已自行 emit）。其他模式不受影响。内存写入/钩子/审计等后台 spawn 仍正常执行。
  - **前端**：`onChapterUpdated` 在 `isGenerating` 期间跳过（`useGenerationStore.getState().isGenerating`），避免后台 `auto_commit` 的 chapterUpdated 在创世期间覆盖编辑器正文。
- 验证：`cargo check` 零错误；`cargo +nightly fmt --check` 通过；`cargo test --lib` **571 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误

## [v0.23.51] - 状态提示统一使用模型名称而非模型 ID（2026-06-26）

### 修复：进度状态文案中显示模型 ID 而非模型名称

- **症状**：顶部流程进度和底部 AI 进程状态提示中，部分文案显示模型 ID（如 `model-458b5096`、`mn-oblivion-26b-hitop-q6-k`）而非人类可读的模型名称（如 `gemma4-e2b`、`MN-Oblivion-26B-UNCENSORED-HITOP-Q6_K.gguf`）。
- **根因**：`llm/service.rs` 中面向用户的进度消息文案（连接模型 / 已连接 / 回应完成 / 等待回应）全部用 `model_id`（profile.id，UUID 或 slug）格式化，而 `model_name`（profile.model，人类可读名称）只放在结构化 `model` 字段里。前端虽有 `appendModelName` 追加名称，但消息文本本身已嵌入 ID，追加后变成"模型 model-458b5096 回应完成... · gemma4-e2b (OpenAI)"，ID 仍然可见。
- **修复**：将 `execute_generation` 中所有 8 处面向用户的进度消息文案（`connecting_msg` / `sent_msg` / `completed_msg` / 心跳 `message`）从 `model_id` 改为 `model_name`。结构化 `model_id` 字段保留不变（前端用它做查找/匹配）。
- 验证：`cargo check` 零错误；`cargo +nightly fmt --check` 通过；`cargo test --lib` **571 passed / 0 failed / 2 ignored**

## [v0.23.50] - 续写卡死"最终输出" + 创世正文重复 + 页面崩溃根治（2026-06-26）

### 修复：续写/创世卡在"最终输出"长时间超时无诊断提示

- **症状**：续写或创世时，进度卡在"最终输出"环节长达 600s 直到前端超时，且无诊断卡片弹出。诊断显示"距最后一次进度事件 588s"但其实后端每 ~60s 都在发事件——只是事件被 probe 的进度文案覆盖。
- **根因（日志确认）**：v0.23.47 引入的 pre-call 实时探测用 `context_label = "pre-call-probe"`，但 `is_silent_background` 静默列表只匹配 `"model_gateway_probe"`，两者不匹配。探测的进度事件"模型 X 回应完成 [pre-call-probe]，正在解析结果..."未静默，直接显示到前端 UI，覆盖了主流程的"最终输出/已完成"状态，造成"卡住"假象。
- **修复**：将 `"pre-call-probe"` 加入 `is_silent_background` 静默列表（`llm/service.rs`）。探测是轻量连接验证，其进度不应显示给用户。

### 修复：死模型反复探测导致持续轮询

- **根因**：`generate()` 在 `select_candidates` 过滤掉 Unhealthy 模型后，会把用户设置的活跃模型（即使是死模型）**无条件重新插入候选链首位**（`executor.rs:568` 的 `else if` 分支不做健康检查）。每次 `generate()` 调用都先 5s 探测这个死模型 → 失败 → continue → 下次调用又探测，形成"死模型永远排在第一位被反复探测"的无效循环。
- **修复**：活跃模型重插入前加 `is_model_available` 健康检查。若活跃模型已被判为 Unhealthy，不再强行插回首位，交给候选链中的其他健康模型。

### 修复：创世第一章正文重复显示（排版版 + 纯文本版两份）

- **症状**：生成新故事第一章后，编辑器出现两份相同文字的正文，一份有 HTML 排版（`<p>` 标签），一份是纯文本。
- **根因**：v0.23.37 回滚了创世成功后清空 `generatedText` 的逻辑（`FrontstageApp.tsx:2392`），改为"用诊断日志定位根因"。若上一次续写/生成在 `generatedText`（纯文本幽灵段落）留下了正文，创世成功后 ChapterSwitch 把正文加载进编辑器（HTML 排版版），stale `generatedText` 仍显示为幽灵段落 → 两份重复。日志确认根因正是 stale 文本。
- **修复**：恢复 `isFirstChapterReady` 时 `setGeneratedText('')` 清空。正文已通过 ChapterSwitch 事件加载到编辑器，幽灵文本必须清空，否则两份并存。

### 修复：输出正文后前端页面崩溃空白

- **根因**：与 Bug 1 同源。非静默的 pre-call-probe 错误事件（`INTERNAL_ERROR`）高频涌入前端，叠加本地模型无法处理并发请求返回错误，大量错误事件导致前端渲染崩溃（与 v0.16.2/v0.23.45 崩溃机制同源）。静默化 probe 后，错误事件不再涌入前端。
- 验证：`cargo check` 零错误；`cargo +nightly fmt --check` 通过；`cargo test --lib` **571 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npx vitest run` **127 passed / 3 skipped**（含创世正文重复回归测试通过）

## [v0.23.49] - 推理模型思考链导致 JSON 提取出空对象修复（2026-06-26）

### 修复：创世「解析故事概念失败: missing field `title`」

- **症状**：用推理模型（如 MN-Oblivion-26B-UNCENSORED）创世时，`ConceptGenerationStep` 报 `missing field 'title' at line 1 column 2`。LLM 实际成功返回 5191 字符、2018 tokens（日志 `llm.generate.return_ok`），失败发生在 JSON 提取阶段。
- **根因**：推理模型在正文前输出 `önh...` / `<thinking>...</thinking>` 思考链。思考链里常出现花括号（如 "用 {} 格式表示"、"return {}"）。v0.23.48 的 `extract_first_json_object` 用 `content.find('{')` 找第一个 `{` —— 它落在思考链里那个 `{}` 上，括号匹配返回空对象 `{}`，serde 反序列化 `StoryMetaElement` 时找不到必填的 `title`，报错位置 `line 1 column 2` 正是 `{}` 的 `}`。
- **修复**：
  - 新增 `strip_reasoning_blocks`：在 `extract_and_sanitize_json` 第一步剥离配对的 `önh...` / `<thinking>...</thinking>` 块（标签以字节数组构造，避免源码中出现完整标签字面量）。未闭合标签保持原样，交给后续括号匹配发现 JSON。
  - `extract_first_json_object` 增强：跳过空对象 `{}` / `{ }`（候选去掉首尾花括号后不含 `:` 即视为空），继续向后扫描找真实 JSON。对任何前导杂散花括号的纵深防御。
- **影响**：`extract_and_sanitize_json` 是 genesis/ingest/analysis/auto_contract 共用的 JSON 提取咽喉点，所有结构化 LLM 响应解析一次性受益。
- 新增 3 个回归测试：思考链前缀+花括号、`<thinking>` 标签、前导空对象跳过。
- 验证：`cargo check` 零错误；`cargo test --lib` **571 passed / 0 failed / 2 ignored**（+3 新测试，零回归）

## [v0.23.47] - 调用模型前实时连接探测 + JSON 尾部多余文本容错（2026-06-25）

### 新增：模型调用前 5 秒实时连接探测

- **根因**：模型列表里可能存在已失效但健康状态仍为 Healthy 的死模型（本地 llama.cpp/MLX 服务已停止但缓存未更新），直接调用浪费 30-300s 直到 LLM 超时。
- **修复**：`GatewayExecutor::generate` 候选循环中，每个候选模型在实际 LLM 调用前先执行 5s 超时实时探测（prompt: `Respond with exactly the word OK.`，max_tokens=4，temperature=0.0）。探测通过则继续实际调用；探测失败/超时则标记 `HealthStatus::Unhealthy`，跳到下一候选。
- 三处 WorkflowLogger 日志点：`pre_call_probe.ok` / `pre_call_probe.fail` / `pre_call_probe.timeout`。

### 修复：JSON 解析 `trailing characters` 错误

- **根因**：`extract_and_sanitize_json` 用 `content.rfind('}')` 找 JSON 结尾，但 LLM 在 JSON 后输出包含 `}` 的额外文本时，`rfind` 误提取过多内容，`serde_json` 报 `trailing characters at line N column M`。
- **修复**：新增 `extract_first_json_object` 辅助函数，用括号匹配（跟踪 `{`/`}` 深度，跳过字符串字面量内的括号）精确提取第一个完整 JSON 对象边界。`memory/ingest.rs` 的 `extract_json` 同步复用。
- 新增 5 个单元测试覆盖：尾部多余文本含 `}`、markdown 围栏+尾部文本、字符串值含 `}`、嵌套对象、无 JSON 对象。
- 验证：`cargo check` 零错误；`cargo +nightly fmt --check` 通过；`cargo test --lib` **568 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；prettier 通过

## [v0.23.46] - AI 状态提示使用模型名称（2026-06-25）

### 修复：IngestPipeline LLM 调用未静默导致前端崩溃（Bug B 残留根因）

- **根因（日志确认）**：创世正文返回后，IngestPipeline 并发发起多个"记忆-内容分析"LLM 调用（提取实体/关系/事件），`context_label` 未匹配 `is_silent_background` 静默列表，`is_silent_background=false` 导致进度事件覆盖前端主活动状态（"准备上下文"卡住）。同时本地模型无法处理并发请求返回 `INTERNAL_ERROR`，大量错误事件涌入导致前端页面崩溃空白。
- **修复**：将 IngestPipeline 的三个 `context_label`（`"记忆-内容分析"`、`"记忆-生成知识"`、`"记忆-叙事事件提取"`）加入 `is_silent_background` 静默列表。
- 验证：`cargo check` 零错误

## [v0.23.44] - AI 状态提示使用模型名称（2026-06-25）

### 优化：AI 进程状态提示使用模型名称而非 ID

- `generation-status` 和 `llm-generating-progress` 心跳事件的状态文案追加模型名称（格式：`准备上下文... · gemma4-e2b (OpenAI) (15s)`），提高识别度。

## [v0.23.43] - 前端诊断日志 + log_frontend_event 命令（2026-06-25）

### 新增：前端写入 WorkflowLogger 的 Tauri 命令

- 新增 `log_frontend_event` Tauri 命令，前端关键路径（`setContent`/`selectChapter`/`ChapterSwitch`）可写入 `creative_workflow.log`，诊断卡片自动收集。
- 前端 `[DEBUG-dup]` / `[DEBUG-act]` 诊断日志接入 WorkflowLogger。

## [v0.23.42] - 根治创世卡在"最终输出"：BGP-4 自死锁修复（2026-06-25）

### 修复：TriShot BGP-4 spawn_blocking 自死锁（Bug B 主根因）

- **根因（日志确认）**：`execute_trishot` 在 Call 3 LLM 成功返回正文后，用 `spawn_blocking().await` **同步等待** BGP-4 深度洞察的 `should_trigger` DB 查询。该查询与 BGP-1（审计）/BGP-3（入库）的后台任务竞争 `std::sync::Mutex`，导致**自死锁**——`execute_trichot` 永不返回，`FirstChapterGenerationStep` 中的 DB 保存和 `ChapterSwitch` 事件发送（都在 `generate().await` 之后）**从未执行**，`smart_execute` 超时 600s。
- **修复**：BGP-4 从 `spawn_blocking().await`（同步等待）改为 `tokio::spawn`（fire-and-forget），`execute_trichot` 在 Call 3 完成后**立即返回**。
- **诊断日志确认**：`trishot.call3.done` → `trishot.bgp4.spawn` → `trishot.bgp4.done` 全部在 1ms 内完成，不再卡死。
- 附带修复 `rustls v0.23.41` checksum（crates.io 重新发布导致 CI 失败）。
- 验证：`cargo test --lib` **563 passed / 0 failed / 2 ignored**；`cargo +nightly fmt -- --check` 通过；`npx tsc --noEmit` 零错误

## [v0.23.41] - BGP-4 fire-and-forget 修复（2026-06-25）

- 与 v0.23.42 相同修复的初次提交（v0.23.42 包含 rustls checksum 修复）。

## [v0.23.40] - 参照现有诊断机制添加 WorkflowLogger 日志点（2026-06-25）

### 新增：关键路径 WorkflowLogger 日志点

- 参照应用已有的 `WorkflowLogger` 诊断机制（写 `logs/creative_workflow.log`，诊断卡片自动收集最近 30 行），在以下关键缺失点补充了日志：
  - **Bug A（正文重复）**：`genesis.first_chapter.generated`、`genesis.chapter_switch.sent`、`genesis.final_content`（记录是摘要还是完整正文）
  - **Bug B（超时/卡死）**：`smart_execute.start`、`trishot.call3.done`、`trishot.bgp4.start`/`bgp4.done`
- 前端 `[DEBUG-dup]` / `[DEBUG-act]` console.warn 诊断日志。

## [v0.23.39] - 回滚激进前端修改恢复可用性（2026-06-25）

### 修复：回滚 v0.23.37/38 中导致页面空白和超时的前端修改

- 回滚 `selectChapter` 同章同内容跳过 guard
- 回滚 `story_created` 块跳过 `selectChapter`
- 回滚 `isFirstChapterReady` 时 `setGeneratedText('')`
- 保留：后端 Bug B 修复（Genesis 补发 completed/error + timeout/error 映射为 failed）、诊断日志

## [v0.23.38] - 诊断日志版本（2026-06-25）

- 添加 `[DEBUG-dup]` / `[DEBUG-act]` 诊断日志到前端关键路径。

## [v0.23.37] - Genesis 活动清理 + 前端正文重复修复尝试（2026-06-25）

### 修复

- **Bug B-1**：Genesis 成功路径补发 `smart-execute-progress` 的 `completed` 事件，错误路径补发 `error` 事件。
- **Bug B-2**：`smart-execute-progress` 处理器把 `timeout`/`error` 也映射为 `failed`（此前只认 `completed`）。
- **Bug A 尝试**：`selectChapter` 同章同内容跳过 + `story_created` 块跳过 + `isFirstChapterReady` 时清空 `generatedText`（后在 v0.23.39 回滚）。

## [v0.23.36] - 创世正文质量优化 + 后台作业不再禁用输入框（2026-06-24）

### 优化：创世正文质量（问题1）

- **根因**：TriShot Call 3 的 `final_prompt` 来自合成器（Call 1 LLM 生成的 `synthesized_prompt`），只含创作约束（题材/角色/风格），**完全没有输出纪律**。`writer_system` 模板虽有纪律约束，但 Call 3 不使用它——合成器提示词直接当 user prompt 发送。模型无格式约束 → 自由发挥输出"好的，作为...""【创作分析】"、`#` 标题、`***` 分隔符等元评论与 markdown 格式。
- **修复A（prompt 约束）**：Call 3 调用前给 `final_prompt` 追加 `NOVEL_OUTPUT_DISCIPLINE` 输出纪律段——禁止元评论、markdown 格式、`【】`小节标题、`（幕结束）`批注。
- **修复B（后处理兜底）**：新增 `sanitize_novel_output` 函数做最后防线，四步清洗：
  1. 逐行去除 markdown 符号（`**`加粗、`*`斜体、`#`标题前缀）
  2. 去除尾部元评论（"【创作分析】"、"（第一幕结束）"等及之后内容）
  3. 去除前导元评论（"好的，作为..."等过渡语）
  4. 去除整行`【】`小节标题与`（）`幕结束批注行、纯分隔符行
- **设计原则**：宁可保留可疑行也不误删正文。尾部截断用的标记是元评论强信号（正文不会整句出现"创作分析与策略说明"），不限制位置。
- **测试**：新增 7 个单元测试覆盖前导过渡语/尾部创作分析/幕结束批注/小节标题/markdown格式/纯净正文不误伤/空输入

### 优化：后台作业不再禁用输入框（问题2）

- **根因**：Genesis 后台阶段（世界观/角色/场景/伏笔）的 `pipeline-progress` 事件被 `useBackendActivityListener` 注册为 running activity → `getIsAnyActive()=true` → `setIsGenerating(true)` → 输入框 `disabled={isGenerating}`。快速阶段已返回、用户本可写作，但后台事件把 `isGenerating` 重新拉高。
- **修复**：后台阶段 `pipeline-progress` 事件打 `metadata: {background: true}` 标记。前端 `useBackendActivityListener` 检测到 `background` 标记后跳过注册 running activity。状态文案仍由已有的 `novel-bootstrap-progress` 监听器更新（"后台正在完善小说世界..."），不影响提示。
- **效果**：创世快速阶段完成后，用户可立即开始写作，后台完善世界观/角色/场景时输入框保持可用。

### 验证

- `cargo test --lib` ✅ **563 passed / 0 failed / 2 ignored**（新增 7 个 sanitize 测试）
- `npx tsc --noEmit` ✅ 零错误

## [v0.23.34] - 修复 select_candidates 中 std::sync::Mutex 自死锁（2026-06-23）

### 修复（根因）

- **`select_candidates` `health_registry` Mutex 自死锁**：第125行 `let health = health_registry.lock().ok()` 获取 MutexGuard，但变量存活到函数末尾才释放。第188行 `is_model_available` 再次调用 `health_registry.lock()` → `std::sync::Mutex` 不可重入 → 同一线程等待自己释放 → 600s 超时。
- Call 1 走 `select_fastest_profile`（不调 `select_candidates`），因此不受影响。Call 3 走 `gateway.generate` → `select_candidates` → 必死锁。
- **修复**：将 `health` 锁移入嵌套块作用域 `{ let health = ...; }`，块结束时 MutexGuard 自动释放。后续 `is_model_available` 可安全重新锁定。

### 诊断

- v0.23.31-33 全链路 15 个诊断标记精确定位死锁发生在 `select_candidates_start` 和 `select_candidates_done` 之间

### 验证

- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check` ✅

## [v0.23.33] - 全链路 15 个精确定位诊断标记（2026-06-23）

### 诊断

- 从 `trishot.call3.start` 到 `llm.generate.start` 共 15 个 workflow_log 标记，覆盖所有可能的 Mutex 阻塞点

## [v0.23.32] - select_candidates 内部精确定位诊断标记（2026-06-23）

### 诊断

- `select_candidates` 内部新增 `cap_done`、`active_ok`、`model_available`、`registry_ok` 标记

## [v0.23.31] - gateway.generate 入口诊断标记（2026-06-23）

### 诊断

- `gateway.generate` 入口新增 `gateway.generate.enter`、`cap_profiles_loaded`、`select_candidates_start/done` 标记

### 架构改进

- `GenerationMode::genesis_default()` 显式化 Genesis 模式选择，替代写死的 `GenerationMode::TriShot`
- 前端 Genesis 期间底部栏显示"[创世]"而非"[三击]"等技术术语

### 验证

- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit` ✅ 零错误
- `npx vitest run` ✅ 126 passed / 3 skipped

## [v0.23.29] - Chapter 保存 spawn_blocking（2026-06-23）

### 修复

- `FirstChapterGenerationStep` 中 ChapterRepository 的 `get_by_story`/`update`/`create` 在 async 块中同步执行，连接池满时阻塞 tokio worker。整个保存逻辑（检查已有 chapter + update/create）放入 `spawn_blocking`。

### 验证

- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**

## [v0.23.28] - select_candidates spawn_blocking 预加载能力档案（2026-06-23）

### 修复

- `GatewayExecutor::generate` 中 `select_candidates` 同步执行 `CapabilityStore::load_all()` DB 查询，连接池满时 `pool.get()` 阻塞 tokio worker 线程，导致 Call 3 的 `llm.generate.start` 永不出现。修复：在异步 `generate` 中用 `spawn_blocking` 预加载能力档案，`select_candidates` 接收 Optional 参数跳过 DB 查询。

### 验证

- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**

## [v0.23.27] - Genesis 跳过 Call 2 精修器（2026-06-23）

### 修复

- v0.23.25 日志显示 Call 1 成功(16s)后卡在"精修提示词..."600s。Call 2 精修器使用 `generate_for_task`（无超时保护），模型挂起时卡死。修复：Genesis 第 1 章 + 无已有内容时跳过 Call 2。
- 信号竖条分差优化：无 TTFB/TPS 数据时用模型名 hash 生成稳定分差(0.2-0.8)，确保竖条有高低区分。

### 验证

- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit` ✅ / `npx vitest run` ✅ 126 passed

## [v0.23.25] - 模型状态指示器改为信号竖条（2026-06-23）

### 增强

- 底部栏模型连接状态从 8px 圆点改为无线信号风格竖条组：每个模型一个竖条(3px宽)，按得分从低到高排列形成上升阶梯，高度 4-16px，最多 8 条。
- 得分 = TTFB 得分(0-0.5) + TPS 得分(0-0.3) + 健康加分(0.2)，无数据用 stable hash 分差。

### 验证

- `npx tsc --noEmit` ✅ / `npx vitest run` ✅ 126 passed

## [v0.23.24] - setContent 内容未变化时不标记未保存（2026-06-23）

### 修复

- `frontstageStore.setContent` 无条件设 `isSaved: false`，内容未变化时不更新。`handleContentChange` 加内容比较。`selectChapter` 同步 `latestContentRef`。从根源杜绝伪"保存中"。

### 验证

- `npx tsc --noEmit` ✅ / `npx vitest run` ✅ 126 passed

## [v0.23.23] - RichTextEditor 外部内容同步跳过 onChange（2026-06-23）

### 修复

- RichTextEditor 外部 `setContent` 触发 TipTap `onUpdate` → `handleContentChange` → 伪"保存中"→ 自动保存 IPC 调用 → 连接池满时阻塞 Genesis。用 `isExternalSyncRef` 标记外部同步，覆盖三处入口（useEffect / `setContent` / `loadAggregatedScenes`）。

### 验证

- `npx tsc --noEmit` ✅ / `npm run format:check` ✅

## [v0.23.22] - 诊断增强 + select_candidates 慢查询标记（2026-06-23）

### 增强

- `select_candidates` 中 CapabilityStore::load_all > 100ms 时输出 `gateway.select_candidates.cap_store_slow` 工作流日志
- TriShot Call 1 添加 `trishot.call1.start` / `trishot.call1.done` 工作流日志标记
- `trishot.bundle.start` 日志标记 bundle 加载阶段

### 验证

- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**

### 修复

- **TriShot auto_fill 做 5 次 LLM 调用导致 600s 超时**：v0.23.20 日志显示概念生成 1.1s 完成、故事已创建、TriShot 启动，但 `trishot.start` 后卡住。根因：新故事无角色 → `QuickPreflightChecker::check` 失败 → 触发 `AutoContractBuilder::auto_fill` 做 **5 次 LLM 调用**（角色 + 场景 + MASTER_SETTING + CHAPTER 合同 + 大纲），每次 30-90s，总计 150-450s，超出 TriShot "最多 3 次 LLM" 设计目标。修复：Genesis TriShot 路径预检失败时，只创建一个最小占位角色（`spawn_blocking` + 纯 DB INSERT，不调 LLM），然后直接进入 Call 1。
- 新增 `trishot.bundle.start` 工作流日志标记，便于诊断 bundle 加载阶段卡点。

### 验证

- `cargo check` ✅
- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check` ✅

## [v0.23.20] - DB 连接池状态可视化 + record_llm_call 整体 spawn_blocking + update_chapter async 化（2026-06-22）

### 修复

- **record_llm_call 仍卡 4 分钟**：v0.23.19 只把 DB 写入移入 spawn_blocking，但 `try_state` + `count_tokens` + `get_active_profile` + 数据收集仍在 tokio worker 线程同步执行。日志显示 `try_state` 到 `spawn` 卡 4 分 16 秒。修复：整个 `record_llm_call` 函数体移入 `spawn_blocking`，async 线程只 clone owned 数据，tokio worker 线程零阻塞。
- **update_chapter "保存中"卡死**：`update_chapter` 是同步 Tauri command（`pub fn`），连接池满时 `pool.get()` 阻塞 Tauri 主线程，前端"保存中..."永不消失。修复：改为 `pub async fn`，DB 操作用 `spawn_blocking` 包裹。

### 增强

- **连接池扩容 20 → 50**：缓冲 auto_commit/ingest/projection writers 并发占用。SQLite WAL 模式支持并发读 + 单写，50 连接对本地 SQLite 无压力。
- **新增 `get_db_pool_status` Tauri 命令**：返回 `{max_size, connections, idle, in_use, connection_timeout_secs}`，利用 r2d2 `Pool::state()` API。
- **前端 DB 连接池状态可视化**：
  - `useDbPoolStatus` Hook：5s 轮询 `get_db_pool_status`
  - FrontstageHeader 状态栏：使用率 <80% 不显示 / ≥80% 黄色 `DB 12/50` / ≥95% 红色 `DB 50/50 ⚠`
  - 诊断卡片新增 `DB连接池` 字段：`48/50（空闲2，超时5s）`

### 验证

- `cargo check` ✅
- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check` ✅
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异
- `npx vitest run` ✅ 126 passed / 3 skipped

## [v0.23.19] - 根治 600s 超时：record_llm_call DB 写入不再阻塞 tokio worker（2026-06-22）

### 修复

- **record_llm_call 同步 DB INSERT 阻塞 tokio worker 线程**：v0.23.18 行级工作流日志精准定位卡点——概念生成 LLM 调用 1.1s 完成，但随后的 `record_llm_call` 同步 DB INSERT 卡住 600s 永不返回。根因是 `record_llm_call` 在 async 上下文中直接执行同步 `pool.get()` + `conn.execute()`，而生产连接池未配置 `connection_timeout`，连接池满时 `pool.get()` 无限阻塞 tokio worker 线程，`tokio::time::timeout` 无法 poll。
  - **Fix 1 生产连接池加 `connection_timeout(5s)`**：`init_db` 的 `Pool::builder()` 补 `.connection_timeout(Duration::from_secs(5))`，与测试池一致，防止 `pool.get()` 无限阻塞
  - **Fix 2 `record_llm_call` 改为 fire-and-forget `spawn_blocking`**：收集 owned 数据后提交 DB 写入到阻塞线程池，立即返回不等待结果。指标记录是审计用途，失败不影响生成结果，永不阻塞主流程
  - 工作流日志新增 `llm.record_call.spawn` phase 标记 fire-and-forget 提交点
- **影响**：此前从 v0.23.14 起反复出现的"概念 LLM 秒回但 pipeline 阻塞 600s"彻底根治。Genesis 快速阶段应在 30-60s 返回第一章正文。

### 验证

- `cargo check` ✅
- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check` ✅
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.18] - 行级诊断：execute_generation Ok 分支 12+ 标记（2026-06-22）

### 诊断

- 为精确定位 v0.23.16/17 中概念 LLM 完成后的卡点，在 `execute_generation` 的 Ok 分支每个步骤前后插入工作流日志标记：`record_call.start` → `try_state` → `db_write` → `db_done` → `record_call.done` → `emit_completed.start` → `emit_completed.done` → `generate.return_ok`
- `record_llm_call` 内部添加 `try_state` / `db_write` / `db_done` / `no_pool` 诊断标记

### 测试

- 新增 5 个独立模块测试（不在 Python E2E 测试覆盖范围内）：
  - `test_heartbeat_abort_does_not_block_indefinitely`：心跳 abort 后 JoinHandle 立即 resolve
  - `test_heartbeat_with_blocking_emit_handled_by_timeout`：阻塞 emit 由 5s 超时保护
  - `test_task_start_times_mutex_no_deadlock`：TASK_START_TIMES Mutex 并发无死锁
  - `test_record_llm_call_pool_timeout`：pool.get() 超时在 5s 内返回
  - `test_record_llm_call_non_blocking`：record_llm_call 并发写入 15s 内完成

### 验证

- `cargo test --lib` ✅ **556 passed / 0 failed / 2 ignored**

## [v0.23.17] - 心跳阻塞 + 连接池超时双保险（2026-06-23）

### 修复

- **心跳 await 无限阻塞**：`heartbeat_handle.await` 等待 tokio task 终止。若心跳在 `emit()` 等同步操作中卡住，`abort()` 无法立即生效，导致主流程永久阻塞。修复：用 `tokio::time::timeout(5s)` 包裹，5 秒后强制继续。
- **r2d2 连接池无限等待**：`pool.get()` 默认无超时。连接池耗尽时阻塞 tokio worker 线程，导致 `tokio::time::timeout` 无法触发。修复：`Pool::builder().connection_timeout(10s)`。
- **LLM 返回路径诊断**：`generate_with_profile_context_and_pipeline` 添加 workflow log 追踪（`record_call.start/done`、`heartbeat.abort/aborted`、`return_ok`），写入 `creative_workflow.log`。

## [v0.23.16] - Genesis 快速阶段卡死修复 + E2E 集成测试（2026-06-22）

### 修复

- **Genesis 快速阶段 600s 卡死**：v0.23.15 的概念 LLM 在 6.7s 完成后 Story 从未创建，pipeline 阻塞 600s 直到前端超时。根因是 `StoryRepository::create()` 为同步 r2d2 调用，在 async 上下文中直接执行。若 DB 连接池满或有写锁冲突，阻塞 tokio worker 线程导致外层 `tokio::time::timeout(600s)` 无法 poll future，超时永不触发。
  - 修复：`story_repo.create()` 改用 `tokio::task::spawn_blocking` 异步化
  - `ConceptGenerationStep` LLM 调用后每个子步骤添加 `log::warn!` 诊断日志（LLM 返回 → JSON 解析 → Story 创建 → ctx 写入）
  - `FirstChapterGenerationStep` 入口添加诊断日志
  - `smart_execute_inner` Genesis 路径添加 pipeline 启动/完成诊断日志

### 测试基础设施

- 新增 `scripts/test_trishot_e2e.py` 端到端集成测试脚本
  - 从应用 `app_settings` 表读取真实模型配置
  - 模拟「写一部异星末世生存的小说」走完完整 Call 1-3 流程
  - 验证结果：**73.2s 完成，2270 字符，1852 中文字，全部检查通过**

### 验证

- `cargo check` ✅
- `cargo test --lib` ✅ **551 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt --check` ✅
- E2E 集成测试 ✅ Gemma4-e2b 真实 LLM，73.2s 生成 1852 中文字

## [v0.23.15] - TriShot 管线 4 处缺陷修复（2026-06-22）

### 修复

- **P0 Genesis 新故事必然失败**：`execute_trishot` 预检用 `QuickPreflightChecker`（仅检查角色非空），不触发 auto-fill。新故事角色表为空 → 预检失败 → 整个 Genesis 报错。修复：预检失败时调 `AutoContractBuilder::auto_fill` 补齐角色后重试预检，与 `prepare_writer_context` 路径行为一致。
- **P1 前端消息标记错误**：v0.23.14 后端返回 `novel_bootstrap_background_started` 但 `final_content` 实际有第一章正文，前端将其当"后台生成中"处理，正文被设为幽灵文本。修复：改名为 `novel_bootstrap_first_chapter_ready`，前端区分"正文已就绪"与"后台生成中"两条路径。
- **P2 Call 1 预算守卫失效**：`t_synth` 在计算前刚创建，`elapsed≈0`，`remaining_budget` 永远等于 `total_budget`，跳过条件永远 false。修复：用函数入口 `total_start` 计算已耗时间（含预检/auto-fill/bundle 加载）。
- **P2 Call 2 预算硬编码 + Call 3 无超时覆盖**：Call 2 硬编码 `total_budget=180` 不读配置；Call 3 走 `generate_for_task_with_tags` 无 `timeout_seconds_override`，可跑满 profile 300s。修复：Call 2 读配置 `total_budget`；新增 `generate_for_task_with_tags_and_timeout`，Call 3 按剩余预算计算超时（30-120s）+ 空内容检查（空字符串直接报错不静默传递）。
- **死代码清理**：移除 `GenesisPipeline::strategy_selection_step()`（无调用者，`StrategySelectionStep` 已包含在 `background_steps()` 中）。

### 验证

- `cargo check` 零错误（92 warnings，减少 1 个死代码警告）
- `cargo test --lib` **551 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit` 零错误
- `cargo +nightly fmt --check` 通过
- `npm run format:check` 通过

## [v0.23.14] - 干净健康的模型池 + 统一身份 + 实时健康报告（2026-06-22）

### 核心目标

建设一个**干净的、健康的模型池**：启动归零 → 实时探测生成 → 候选池只含存活模型 → 删除即彻底清除 → 显示统一为 canonical name。彻底消灭死模型/硬编码模型混入候选链导致 600s 超时的根因。

### 修复

- **L1 净化候选池 + 启动归零**
  - 启动时自动清空 `llm_calls` 历史表（归零）+ 过滤 `HealthRegistry` 中不在当前 config 的残留条目
  - `delete_model` 级联清理：刷新网关注册表 + 清除健康快照 + 删除历史调用记录（此前三项都不做，死模型残留）
  - `update_model` 刷新网关注册表 + 清除旧健康快照 + 用新 endpoint 重新探测（此前修改 endpoint/api_key 后网关仍用旧连接）
- **L2 统一身份 + 强引用校验**
  - `set_active_llm_profile` 拒绝将 disabled 模型设为活跃（此前指针指向死模型后静默跳过）
  - `default()` 不再默认指向 disabled 占位 profile `Qwen3.5-...`
  - `delete_model` active 重置增加 `enabled` 过滤
  - `set_active_model` 新增 `refresh_registry`，与 `create_model` 保持对称
- **L3 清理硬编码死模型**
  - 删除 `config/models.ts`（含写死局域网 IP `10.62.239.13` 的死代码）
  - 删除 `hooks/useModel.ts`（含 `id: modelId, name: modelId` 混淆 bug）
  - 净化 `BROWSER_FALLBACK_MODELS` 为空列表，不再注入假模型
- **L4 显示统一 + 健康报告数据源切换**
  - `get_model_health_reports` 从 `llm_calls` 历史表切换为 `HealthRegistry` 实时探测快照
  - `ModelHealthReport` 新增 `tps`/`capability_score`/`speed_score`/`quality_score` 字段
  - 面板副标题从裸 `model_id` 改为 `provider · model`
  - 删除前端 dead code `primaryModel`
- **运行时健康状态自动恢复**
  - `run_retry_probe` 覆盖 `Unknown` 模型（此前 Unknown 无人探测，永久卡死）
  - 面板"重新检测全部"按钮触发真实后端探测（此前只读缓存）
  - 每个模型卡片新增独立"重新检测"按钮，词元限制恢复后即时回池
- **模型能力分数合成**
  - `CapabilityProfile::compute_scores()` 从实测 TTFB/TPS/成功率合成 `speed_score`/`quality_score`/`capability_score`（0-100）
  - `run_initial_benchmark` 调用 `compute_scores` 持久化
  - `select_candidates` 第 209 行消费 `capability_score`（之前永远是 None → 0，质量维度缺失）
  - `select_fastest_profile` 排序从纯 TTFB 改为"分桶 + 能力分决胜"：同 200ms TTFB 桶内按 `capability_score` 降序

### 新增测试（11 个）

- `health.rs`: 7 个（purge/retain/probe_success_rate/probe_count）
- `settings_tests.rs`: 1 个（reject disabled）+ 3 个已有适配
- `types.rs`: 3 个（compute_scores 快/慢/无数据）

### 验证

- `cargo test --lib` ✅ **551 passed / 0 failed / 2 ignored**
- `cargo +nightly fmt -- --check` ✅ 通过
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.13] - 强制所有生成路径使用活跃模型（2026-06-22）

### 修复

- **Genesis / TriShot / 路由生成统一优先使用用户设置的活跃模型**
  - `LlmService::select_profile_for_request` 现在优先返回 `active_llm_profile`，不再让路由器把调用发到用户未预期的“最快”模型
  - `GatewayExecutor::select_candidates` 现在把健康活跃模型强制置顶为 primary，避免被三维打分或算力档案绕开
  - `GatewayExecutor::select_fastest_profile` 无条件优先使用健康注册表中的活跃模型（Healthy/Degraded），仅在活跃模型不可用时才回退到最快模型
  - 避免再次出现“当前模型是 Gemma4-e2b，实际调用 RavenX/Qwen3.5 导致 600 秒超时”的情况
- **新增模型即时进入可用池**
  - `create_model` 保存后立刻刷新网关注册表并执行一次健康探测
  - 探测通过的模型立即参与后续调度，避免新模型被遗漏

### 验证

- `cargo test --lib` ✅ 540 passed / 0 failed / 2 ignored
- `cargo +nightly fmt -- --check` ✅ 通过
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.12] - 彻底修复长超时：活跃模型优先 + 智能创作流程日志（2026-06-22）

### 修复

- **修复长超时根因：连接了错误的模型**
  - `GatewayExecutor::generate` 现在会把用户当前设置的活跃模型强制提升到候选链首位
  - 若候选链里没有活跃模型，自动插入到首位
  - `select_fastest_profile` 在活跃模型没有算力档案时，也优先使用活跃模型，而不是选一个历史“最快”模型
  - 仅当活跃模型不健康或明显慢（TTFB > 最快模型 3 倍且 > 3000ms）时才回退
- **探测/静默调用不再污染诊断提示词**（延续 v0.23.11）

### 新增

- 新增 `WorkflowLogger` 智能创作流程详细日志
  - 日志文件：`logs/creative_workflow.log`（JSON Lines，10MB 滚动截断）
  - 记录 TriShot 每个阶段：启动、bundle 加载、Call 1 合成结果、Call 3 路由与 asset_tags、LLM 调用起止与耗时、模型网关候选链、错误
- 新增命令 `get_workflow_logs` / `get_workflow_log_path`
- 诊断卡片新增字段：**工作流日志路径**、**智能创作流程最近日志**

### 验证

- `cargo test --lib` ✅ 540 passed / 0 failed / 2 ignored
- `cargo +nightly fmt -- --check` ✅ 通过
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.11] - 诊断提示词过滤探测/静默调用（2026-06-22）

### 修复

- 修复诊断卡片中“最后发给模型的提示词”显示为探测 prompt `Respond with exactly the word OK.` 的问题
  - `LlmService::execute_generation` 现在只在**非静默/非探测**调用时更新 `DiagnosticStore` 和 `llm-prompt-sent` 事件
  - 被过滤的调用包括：`model_gateway_probe`、`input_hint`、`intent_detection`、各类 `async-*` 后台审计/洞察、`tri-shot-router/refiner`、`bg-auto-rewriter`、`bg-ingest`

### 验证

- `cargo test --lib` ✅ 540 passed / 0 failed / 2 ignored
- `cargo +nightly fmt -- --check` ✅ 通过
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.10] - 模型网关优先使用当前活跃模型（2026-06-22）

### 修复

- 修复 `select_fastest_profile` 可能选中用户已不用的旧模型的问题
  - 现在先读取当前 `active profile`
  - 若活跃模型健康且 `short_ttfb_ms_p50` 不超过最快模型 3 倍（至少 3000ms），优先使用活跃模型
  - 仅当活跃模型明显慢或不可用时才回退到全局最快模型
- 修复 `select_candidates` 候选链可能不包含当前活跃模型的问题
  - 若活跃模型不在候选链中且健康，将其以当前 top 分数注入候选链
  - 保证用户设置的模型始终有机会被选中

### 验证

- `cargo test --lib` ✅ 540 passed / 0 failed / 2 ignored
- `cargo +nightly fmt -- --check` ✅ 通过
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.9] - 运行时创作资产能力清单 + TriShot 路由增强（2026-06-22）

### 新增

- 新增 `creative_engine::asset_capability_manifest::AssetCapabilityManifest`
  - 应用启动时基于 `strategy::load_all_assets()` 自动生成并刷新
  - 包含全部系统创作资产：methodology、genre_profile、style_dna、skill、beat_card、story_engine、pressure_relationship、workflow 等
  - 生成按 kind 分组的紧凑文本摘要（6000 字符预算 + 截断），供 LLM 阅读
- `AssetCapabilityManifest` 作为 Tauri State 注入，供后端各模块读取
- `PromptSynthesizer`（TriShot Call 1）现在会收到 `【系统可用创作资产目录】`
  - Call 1 除了当前故事的 `WriteTimeBundle` 约束外，还能看到全局可选资产
  - 提示 LLM 从目录中选择相关资产并把 asset id 放入 `selected_asset_ids`
- 新增 `LlmService::generate_for_task_with_tags(...)`
  - 支持携带 `asset_tags` 和 `discovered_asset_ids` 到模型网关
- TriShot Call 3 现在会把 Call 1 选中的资产 ID 透传给模型网关
- `model_gateway::dispatcher::TaskClassifier::adjust_by_asset_tags` 识别更多创作资产标签
  - `methodology`、`beat_card`、`story_engine`、`pressure_relationship`、`style_dna`、`skill` 等标签会触发 `HeavyCreation`，优先使用创作能力强的模型

### 修复

- 修复 TriShot 把 `gen_response.model` 错误地当作 `request_id` 返回的问题
- 修复 TriShot Call 1 没有预算守卫的问题：当剩余时间不够完成 Call 1 + Call 3 时，直接回退到本地 `bundle_prompt`，避免前端长时间无响应

### 验证

- `cargo test --lib` ✅ 540 passed / 0 failed / 2 ignored
- `cargo +nightly fmt -- --check` ✅ 通过
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.8] - AI 进度指示精细化 + 提示词诊断可靠性提升（2026-06-22）

### 新增

- LLM 生成进度事件（`llm-generating-progress`）新增字段：`model_id`、`provider`、`prompt_chars`、`prompt_tokens`、`response_tokens`
- 进度文案具体化，不再只显示“构思故事”四个字：
  - `connecting`：连接模型 `model_id`（`provider`）
  - `sent`：已连接模型，组合提示词约 X 字符（估算 Y tokens），正在发送请求
  - `generating`（心跳）：等待模型 `model_id`（`provider`）回应中，提示词约 X 字符，已等待 Z 秒
  - `completed`：模型回应完成，共 X tokens，正在解析结果
- 新增 `diagnostics::DiagnosticStore` Tauri State，用于可靠保存“最后发给 LLM 的提示词全文”
- 新增命令 `get_last_llm_prompt`，前端诊断卡片在事件未送达时可通过命令兜底读取

### 修复

- 修复 `llm-prompt-sent` 事件可能因提示词过大而丢失或无法送达前端的问题
- 修复诊断卡片在 LLM 实际已调用但提示词仍显示“未捕获”的问题

### 验证

- `cargo test --lib` ✅ 538 passed / 0 failed / 2 ignored
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.7] - 诊断信息增强 + 超时文案去硬编码（2026-06-22）

### 修复

- 修复诊断卡片 `应用版本` 显示固定 `0.16.0` 的问题
  - `src-frontend/src/main.tsx` 与 `src/frontstage/main.tsx` 改为从 `package.json` 动态注入 `__STORYFORGE_VERSION__`
- 修复前端超时/后端超时提示文案硬编码 `200s` / `180s` 的问题
  - `FrontstageApp.tsx` 的 `handleRequestGeneration` / `handleSmartGeneration` 现在从 `settings.frontend_timeout_secs` / `settings.smart_execute_total_timeout_secs` 读取实际值
  - 错误提示与诊断卡片中的超时说明全部改为实际配置值

### 新增

- 诊断卡片新增 **AI 生成模式**（`settings.generation_mode`）
- 诊断卡片新增 **当前连接模型 ID / 名称 / 提供商 / 端点**
- 诊断卡片新增 **最后调用模型** 与 **最后发给模型的提示词全文**（后端通过 `llm-prompt-sent` 事件广播，前端实时捕获）
- 后端 `LlmService::generate_for_request_with_request_id` 在调用模型网关前发射 `llm-prompt-sent` 事件，携带 `request_id`、`context_label`、`model_id`、`model_name`、`provider`、`prompt`

### 验证

- `cargo check` ✅ 零错误（82 warnings 均为既有）
- `npx tsc --noEmit` ✅ 零错误
- `npm run format:check` ✅ 零差异

## [v0.23.6] - 修复 macOS 启动崩溃（2026-06-22）

### 修复

- 修复启动时 `state() called before manage() for Arc<dyn VectorStore>` panic 导致的 macOS 崩溃
  - 根因：`init_task_system_and_automation` 在 `app.manage(vector_store)` 之前通过 `app_handle.state()` 获取向量存储
  - 方案：将 `LanceVectorStore` 的创建与 `app.manage` 提前到所有依赖组件之前，仅保留异步 `init()` 在原地

### 验证

- 本地 `cargo run` 启动成功，未再出现 APPLICATION PANIC
- `cargo test --lib` 538 passed
- `npm run format:check` / `npm run type-check` 通过
- `python3 scripts/architecture_guard.py` 通过

## [v0.23.5] - CI 格式化修复（2026-06-21）

### 修复

- 修复 Rust nightly `cargo fmt` 格式化差异（import 顺序、函数参数折行、单行化）
- 修复前端 Prettier 格式化差异（`GeneralSettings.tsx` 类型断言单行化）

### 备注

- 无业务逻辑变更；仅代码风格修复，使 `rust-check` / `frontend-check` 通过 GitHub Actions 门控

## [v0.23.4] - 智能层闭环落地（2026-06-21）

### 背景

TriShot 管线与架构债务清偿完成后，智能创作层仍需补齐最后一环：LLM 结构化输出原生支持、MemoryPack 预算语义化、拆书数据统一汇入 narrative 资产表。本版本完成 Phase 6，不改动业务行为，只提升输出确定性与数据一致性。

### 新增

- `llm::adapter::ResponseFormat` 枚举，当前支持 `JsonObject`
- OpenAI 适配器请求体附加 `response_format: { "type": "json_object" }`
- Ollama 适配器请求体附加 `format: "json"`
- `LlmService::generate_for_task_with_format` / `generate_with_profile_and_request_id_with_format` 等结构化输出入口
- `GatewayRequest` 新增 `response_format` 字段，模型网关可透传 JSON mode
- `RefineChangeNote` 类型与 `RefineResult.refinement_notes`，修稿结果携带分类变更说明
- 拆书场景统一字段：`SceneElement.key_events` / `emotional_tone`
- 数据库迁移 `V100__拆书存储统一_删除_reference_表.sql`：补齐 `narrative_scenes` 字段并删除 `reference_characters` / `reference_scenes`

### 改造

- `pipeline/review.rs` 调用 `generate_for_task_with_format(..., JsonObject)`，保留 JSON 解析容错
- `pipeline/refine.rs` 调用 JSON mode 并解析 `{ refined_content, change_summary, refinement_notes }`
- `MemoryBudget::for_task_type` 改为接收 `MemoryTaskType { Write, Plan, Review }`，IPC/调用点同步更新
- `BookDeconstructionService` 删除 `ReferenceCharacterRepository` / `ReferenceSceneRepository` 双写，统一写入 `NarrativeCharacterRepository` / `NarrativeSceneRepository`
- `BookDeconstructionRepository::get_book_analysis_summary` 改为查询 `narrative_scenes`
- `WriteTimeBundle` 参考场景 few-shots 改用 `NarrativeSceneRepository` + `SceneElement`
- Prompt 缓存键隔离 JSON mode 与普通文本输出
- `CONTEXT.md` 拆书存储章节更新为已解决

### Schema

- `narrative_scenes` 新增列：`key_events`、`emotional_tone`、`narrative_intensity`、`narrative_sentiment`、`narrative_event_types`、`act_number`、`position_in_act`
- 新数据库的 `narrative_scenes` 建表语句已包含上述列

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **538 passed / 0 failed / 2 ignored**
- `npx tsc --noEmit` ✅ 零错误
- `python3 scripts/architecture_guard.py` ✅ 通过

## [v0.23.1] - 架构债务清偿：全局单例治理与模块依赖解耦（2026-06-21）

### 背景

TriShot 管线落地后，后端仍存在 v0.22.x 之前遗留的全局 `static` 单例与模块间循环导入，阻碍并行初始化、单测 mock 与长期演进。本版本彻底清理这些架构债务，不改动任何业务行为。

### 治理

- 移除 14 个已禁止的全局单例/缓存：`VECTOR_STORE`、`DB_POOL`、`LLM_SERVICE`、`APP_CONFIG`、`SKILL_MANAGER`、`CHAPTER_COMMIT_DEBOUNCE`、`PENDING_VECTOR_INDEXES`、`WRITER_APP_DIR`、`WRITER_APP_CONFIG`、`WRITER_GENRE_PROFILES`、`WRITER_STYLE_DNAS`、`APP_CONFIG_CACHE` 等
- 全局依赖全部替换为 Tauri `state` 注入或函数级显式参数传递

### 架构

- 扩展 `domain` 领域层，统一放置跨模块共享类型：
  - `agent_context`、`agent_types`、`foreshadowing`、`search`、`write_time_bundle`
  - `asset_snapshot`、`continuity`、`adaptive`、`prompt_synthesis`
  - `agent_service`（`AgentServicePort` 行为端口）、`creative_engine`（`CreativeEnginePort` 行为端口）
- 解决历史模块依赖方向：
  - `memory → agents`、`narrative → memory`、`narrative → creative_engine`：数据类型下沉到 `domain`
  - `agents ↔ creative_engine`：通过 `CreativeEnginePort` / `AgentServicePort` 双向依赖反转，消除循环导入
- `creative_engine` 提供 `CreativeEngineAdapter` 实现 `CreativeEnginePort`；`agents/service.rs` 实现 `AgentServicePort`
- `scripts/architecture_guard.py` 清空 `KNOWN_VIOLATIONS`，当前报告：
  - Enforced global singletons removed: **14**
  - Known violations tracked: **0**

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 486 passed / 48 failed（48 failed 为已知 V092 基线问题，零新回归）
- `npx tsc --noEmit` ✅ 零错误（本版本未改动前端源码）

## [v0.23.2] - 事件总线与状态同步治理（2026-06-21）

### 背景

v0.23.1 清除了全局单例与模块循环依赖后，后端仍缺少一次完整 `CHAPTER_COMMIT` 的显式事件，前端 `FrontstageApp` 仍在本地 `useState` 维护编辑器内容，未兑现 "single source of truth"。本版本补齐事件流并收敛状态源。

### 新增

- 后端 `SyncEvent::ChapterCommitted` 变体（`state_sync/events.rs`），携带 `story_id`、`chapter_id`、`chapter_number`、`projection_status`（`HashMap<String, String>`）
- `StateSync::emit_chapter_committed` 发射助手（`state_sync/service.rs`）
- `SceneCommitService::apply_commit` 在同步 + 异步 projections 全部完成后发射 `ChapterCommitted`
- `useSyncStore` 中处理 `chapterCommitted`：失效 `chapters`、`chapterDetail`、`knowledgeGraph`、`foreshadowings`、`payoffLedger` 缓存

### 改造

- `FrontstageApp.tsx` 将 `content` / `isSaved` 从本地 `useState` 迁移到 `useFrontstageStore`
  - 保留 `isSaved` 与编辑器焦点双重保护，后台同步事件不会覆盖未保存内容
  - 自动保存成功后通过 `setSaveStatus` 回写保存状态与时间戳
  - 章节切换时同步写入 `frontstageStore.setChapterInfo`
- 删除所有 `backstage-data-refreshed` 废弃注释（`App.tsx`、`Scenes.tsx`、`Characters.tsx`、`Foreshadowing.tsx`）
- `src-frontend/src/CONTEXT.md` 更新 Data Refresh 定义为仅 `sync-event`
- `useWebViewRedrawFix` 注释由 `TODO` 改为 `FIXME`，明确仍是临时 hack

### 测试

- 新增 `state_sync::events::trishot_event_tests::test_chapter_committed_serialization` 序列化测试
- 前端 vitest 基线不变：126 passed / 3 skipped

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 487 passed / 48 failed（48 failed 为已知 V092 基线问题，零新回归；新增 1 测试通过）
- `npx tsc --noEmit` ✅ 零错误
- `npx vitest run` ✅ 126 passed / 3 skipped
- `python3 scripts/architecture_guard.py` ✅ 通过

## [v0.23.3] - 测试基线修复 + 工程化（2026-06-21）

### 背景

自 v0.17.0 起 `cargo test --lib` 一直有 48 个失败，被当作“V092 基线问题”。根因是 `V095__意图图_SING_数据层.sql` 在 inline Rust migrations 之前运行，导致 28–94 号 inline migrations 被跳过，大量表/列缺失。本版本彻底修复。

### 修复

- `MigrationRunner::run_with_legacy` 改为按版本交错执行：SQL 文件版本 ≤ `MAX_INLINE_MIGRATION_VERSION` 时先运行，然后运行 inline Rust migrations，最后运行更高版本的 SQL 文件
- `V095__意图图_SING_数据层.sql` 重命名为 `V099__意图图_SING_数据层.sql`，确保其永远在所有 inline migrations 之后应用
- `db/connection.rs` 新增 `MAX_INLINE_MIGRATION_VERSION = 98` 常量与防护注释

### Schema

- `narrative_characters` / `narrative_scenes` / `narrative_world_buildings` 建表语句加入 `status TEXT NOT NULL DEFAULT 'active'`
- 新增 inline Migration 98：为已存在的三张 narrative 表 `ALTER TABLE ADD COLUMN status`

### Repository

- `domain/narrative_elements.rs` 为 `ElementSource` / `ElementStatus` 新增 `as_str()` / `from_str()`（snake_case 英文）
- `db/repositories_narrative.rs` 存储与解析统一使用英文键，修复中文 `Display` 与英文解析不匹配导致的 round-trip 失败

### 测试

- 新增 `db::repositories_narrative::tests` 3 例：source/status round-trip、repository 读写 round-trip

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **538 passed / 0 failed / 2 ignored**（首次全绿）
- `npx tsc --noEmit` ✅ 零错误
- `npx vitest run` ✅ 126 passed / 3 skipped
- `python3 scripts/architecture_guard.py` ✅ 通过

## [v0.23.0] - TriShot 三击生成管线（2026-06-21）

### 背景

智能创作主流程存在两个核心痛点：(1) 资产注入是「笨拼接」——`WriteTimeBundle::to_prompt()` 把 ~17 个资产段落无差别堆砌，不区分相关性，不解决段落间冲突；(2) Full 模式（改写/质检）最多 5 次串行 LLM，本地模型累计 250-335s 必然超时。用户提出「最多 3 次连接」的思路，本版本全面实施。

### 新增

- 新增 `GenerationMode::TriShot` 三击模式（`agents/orchestrator.rs`），与 Fast/TimeSliced/Full 并存，`AppConfig.generation_mode = "tri_shot"` 配置切换
- 新增 `creative_engine/prompt_synthesis/` 模块（`manifest.rs` / `synthesizer.rs` / `refiner.rs`）：Call 1 用最快模型选资产+合成提示词；Call 2(可选) 精修提示词；Call 3 Writer 生成
- 新增 `GatewayExecutor::select_fastest_profile()` + `LlmService::generate_with_fastest()`：按 CapabilityProfile TTFB 选最快可用模型
- 新增 `GatewayRequest::for_fast_routing()` 构造函数
- 新增 BGP-2 后台自动改写器 `task_system/auto_rewrite_executor.rs`：HIGH 严重度自动改写+可撤销，LOW 仅建议
- 新增 `PlanStep::long_running` 标志，TriShot 步骤跳过 90s 步超时（受 180s 伞保护）
- 新增 `SyncEvent::ContentAutoRevised` / `RevisionSuggested` 变体 + 前端路由
- 新增 `silent_background` 白名单标签：`tri-shot-router` / `tri-shot-refiner` / `bg-auto-rewriter` / `bg-ingest`
- 注册表新增 `trishot_synthesizer` / `trishot_refiner` prompt 模板
- 新增 `auto_rewrite_severity_threshold` 配置字段（默认 "high"）

### 改造

- `PlanExecutor::execute_with_context` 新增 TriShot 快速路径：跳过 SING/PlanGenerator，直接单步 writer step
- `execute_writer` mode 路由新增 `"tri_shot"` 分支
- `AuditExecutor::run_audit_inner` 链式 spawn AutoRewriteExecutor（BGP-2）
- `execute_trishot` 后台 spawn Audit（BGP-1）+ IngestPipeline（BGP-3）+ Insight（BGP-4）
- 默认续写路径从 2 次 LLM（计划+Writer）优化为 2~3 次 LLM（合成+精修+Writer），资产选择由 LLM 智能替代本地拼接
- `extract_redline_text` 提升为 `pub(crate)` 供 manifest 复用

### 测试

- 新增 `GenerationMode` 枚举测试 1 例
- 新增 `prompt_synthesis::manifest` 测试 5 例（空清单/红线+角色/截断/元数据/综合）
- 新增 `prompt_synthesis::synthesizer` 测试 10 例（code fence 剥离/JSON 解析容错/字段缺失/低置信度/回退）
- 新增 `GatewayRequest::for_fast_routing` 测试 1 例
- 新增 `SyncEvent::ContentAutoRevised` / `RevisionSuggested` 序列化测试 2 例
- 新增 `prompt_synthesis` 集成测试 1 例（清单+回退流程）

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 486 passed / 48 failed（48 failed 为已知 V092 基线，零新回归）；新增 TriShot 相关测试全部通过
- `npx tsc --noEmit` ✅ 零错误

### 设计文档

- `docs/plans/2026-06-21-trishot-pipeline-design.md`

## [v0.22.4] - 「异星球末世生存」智能创作流程优化（2026-06-21）

### 背景

用户输入「写一部异星球末世生存题材的小说」时，后台资产链路存在断链：自然语言题材词无法解析为多个 `GenreProfile`、意图图发现无法感知复合题材、模型网关调度未利用资产标签、TimeSliced 默认续写 prompt 只注入主题材画像。本版本系统性修复这些问题。

### 新增

- 新增 `GenreResolver` 题材解析服务（`src-tauri/src/strategy/genre_resolver.rs`），支持精确匹配、别名匹配、子串匹配、同义词扩展、复合题材拆分
- `GenreProfile` 别名表扩展：新增「末世」「末日」「废土」「生存」「异星球」「异星」「星际」「外星」「机甲」「科幻」「未来」「太空」「拓荒」「荒野求生」等中文别名
- `AssetNode` 支持 `tags` 字段，资产同步时自动注入 `genre_profile` / `mcp_tool` / `system_command` / `creative_writing` 等标签
- `GatewayRequest` 新增 `asset_tags` 和 `discovered_asset_ids` 字段，打通意图图→AgentService→模型网关的资产上下文
- `WriteTimeBundle` 新增 `secondary_genre_profile_strategy` 字段与 `load_for_continue_writing` 参数，复合题材时自动加载次要题材画像策略

### 改造

- `StrategySelector::exact_genre_match` 接入 `GenreResolver`，优先按解析出的 `genre_profile_ids` 匹配推荐资产
- `build_selected_strategy` 将 LLM 输出的自由格式题材词经 GenreResolver 归一化为标准 `genre_profile_ids`
- `story_concept_prompt` 要求 LLM 输出标准 `genre_profile_ids` 字段
- `IntentionGraphPlanner::discover_assets` 增加 `GenreResolver` 复合题材解析分支，为未通过 PPR 发现的题材画像资产补充分数
- `TaskClassifier::classify_task` 新增 `adjust_by_asset_tags` 校准：外部工具/系统命令标签→LightTool，genre_profile/creative_writing 标签→HeavyCreation
- `GatewayExecutor` 按 `asset_tags` 重叠加分，提升携带创作资产标签的候选模型排序
- `AgentService::generate_for_request_with_request_id` 透传 `asset_tags`/`discovered_asset_ids` 到 `GatewayRequest`

### 测试

- 新增 `strategy::genre_resolver` 单元测试 5 例
- 新增 `strategy::selector` 复合题材/别名匹配测试 6 例
- 新增 `write_time_bundle` 次要题材渲染测试 1 例（累计 13 例）
- 新增 `model_gateway::dispatcher` 资产标签校准测试 2 例
- 新增 `intention_graph` 标签与发现相关测试 2 例

### 验证

- `cargo check` 零错误
- `cargo test --lib` 新增 targeted tests 全部通过（39 passed；49 failed 为已知 V092 基线问题，非本次引入）
- `npx tsc --noEmit` 零错误

## [v0.22.3] - 钥匙串彻底移除 + 模型健康报告自动刷新 + 配置加载优化（2026-06-21）

### 🔥 核心变更

#### 1. 彻底移除 macOS 钥匙串依赖

- **移除 `keyring` crate**（Linux/Windows/macOS native 全平台依赖已清除），编译产物不再链接 Security.framework
- **移除 `secure_storage` 模块**（~75 行），API Key 改为直接存入 SQLite
- **移除 `store_api_keys_securely` 配置项**，不再需要钥匙串开关
- **移除 `load()` 中的钥匙串迁移 + 恢复逻辑**（~130 行），启动时不再创建 `keyring::Entry` 实例
- **移除 `save()` 中的钥匙串持久化逻辑**（~55 行），保存时不再写钥匙串
- **移除 `save_to_db()` 中的 API Key 剥离逻辑**，完整配置（含 Key）直接写入 SQLite

**影响**：本地模型用户（API Key 为空）永远不会触发钥匙串；云 API 用户在设置页面输入密钥后直接存数据库，不再弹出系统级密码提示。

#### 2. 模型健康报告自动刷新

- 前端 `useModelHealthReports` 添加 `refetchInterval: 30_000`（每 30 秒自动刷新）
- 后端 `get_model_health_reports` 改为 `async` 命令，不再阻塞 IPC 线程
- 配合钥匙串移除，查询毫秒级返回，用户不再需要手动点击「刷新」

#### 3. AppConfig.load() 冗余调用消除

- `planner/executor.rs`: `execute_writer` 中 2 次 `AppConfig::load()` → 1 次
- `narrative/genesis.rs`: `FirstChapterGenerationStep` 中 3 次 `AppConfig::load()` → 1 次
- `book_deconstruction/executor.rs`: 移除死代码（`_concurrency` 结果未使用）
- 缓存 TTL 保持 30 秒，绝大多数命令走内存缓存

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 425 passed（49 failed = 已知 V092 基线，零回归）
- `npx tsc --noEmit` ✅ 零错误

## [v0.22.2] - 题材画像推荐资产种子 + 策略选择硬约束 + CI 修复（2026-06-21）

### 建设性意见实施

- **GenreProfile 推荐资产字段种子**：`seed_genre_recommendations()` 为 7 个常用题材写入推荐风格/方法论/技能映射（末世→余华/英雄之旅 等）
- **策略选择器硬约束**：`build_selected_strategy` 中体裁画像有推荐时跳过 LLM 直接使用推荐资产
- **算力档案默认值修正**：`capability_score` 未测试时默认值从 0.5 改为 0.0，避免虚假质量分基准
- **CI 修复**：`cargo +nightly fmt` + `prettier` 格式化全部通过

## [v0.22.1] - 提示词与后台资产深度结合：5 条建设性意见全部实施（2026-06-21）

### 5 条意见实施

- **意见 1：StrategySelector 题材推荐映射**：`get_genre_recommendations()` 覆盖末世/玄幻/都市/科幻/悬疑/古言/校园 7 种题材→风格推荐
- **意见 2：StyleDNA 句长偏差检测**：`execute_time_sliced` 生成后计算实际句长 vs 目标句长，偏差>30% 记录建议
- **意见 3：Inspector 方法论动态 prompt ID**：按 `methodology_id` 动态选择 prompt（雪花法/英雄之旅/场景结构/人物深度/高密度世界构建）
- **意见 4 (Phase F)：GenreProfile 推荐资产字段**：4 新列 + Migration 96 + Repository SQL 更新
- **意见 5：算力档案质量分权重**：HeavyCreation→quality80%，LightTool→speed60%，含 TPS/success_rate/capability_score

## [v0.22.0] - 提示词与后台资产完整结合：5 个系统性缺口全部修复（2026-06-21）

### Phase A: TimeSliced 路径全资产注入

- **WriteTimeBundle** 新增 4 字段：`style_dna_extension`（完整六维）、`methodology_extension`（方法论规则）、`genre_profile_strategy`（题材画像全字段）、`writing_strategy_constraints`
- `load_sync` 加载 StyleDNA → `to_prompt_extension()`、方法论 → `build_prompt_extension()`、GenreProfile 全字段、写作策略
- `to_prompt()` 追加 4 个新 section（⑪-⑭）

### Phase B: Inspector 全资产注入

- `build_inspector_prompt` 追加体裁画像策略、方法论约束、角色当前状态、活跃冲突、叙事四元组 5 个 section

### Phase C: 意图感知调度接线

- `LlmService.generate_for_request_with_request_id` 新增 `intent_verb`/`intent_object` 参数
- `generate_for_agent_with_options` 按 agent_type 自动推导意图（Writer→generate/prose 等）

### Phase D: 算力档案消费闭环

- `select_candidates` 加载 `CapabilityProfile`，TTFB<2s 模型获速度加分

### Phase E: 资产→生成参数规则映射

- 新增 `creative_engine/adaptive/asset_params.rs` —— StyleDNA→temperature, methodology→max_tokens, genre→max_tokens 规则映射表

### 修复前后对比

| 指标                       | 修复前             | 修复后                             |
| -------------------------- | ------------------ | ---------------------------------- |
| TimeSliced Writer 注入资产 | 仅一句话摘要       | StyleDNA六维+方法论+题材画像+策略  |
| Inspector 检查维度         | 4 维度             | 5+ 维度（含题材/角色/冲突/方法论） |
| 意图感知调度               | intent 恒 None     | Writer→(generate,prose)            |
| 末世→风格推荐              | 莫言（魔幻现实）❌ | 余华（冷酷白描）✅                 |

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib intention_graph` ✅ 18/18 通过
- `cargo test --lib adaptive::asset_params` ✅ 3/3 通过
- 真实模型测试（Gemma4-e2b, 6 场景）✅ 6/6 通过

## [v0.21.0] - 提示词全量可配置化：所有硬编码提示词提取到前端可编辑（2026-06-21）

### 核心升级

- **所有 LLM 提示词均可在前端后台设置页面中显式查看、修改和保存**，用户修改后由应用调用。此前仅 14 个提示词真正可覆盖，其余 40+ 个硬编码提示词完全旁路 registry，用户无法修改。

### 关键变更

- **Phase 1: registry 扩展**：新增 6 个 PromptCategory（Pipeline/Audit/Intent/Deconstruction/Creation/Strategy），注册 ~50 个新提示词条目覆盖所有旁路硬编码，新增 `resolve_prompt_with_vars` 便捷函数
- **Phase 2: 假接入修复**：雪花法 10 个 + Agent 助手 5 个 key 从 `resolve_prompt_default`（旁路 DB 覆盖）改为 `resolve_prompt`（含 DB 覆盖），用户覆盖真正生效
- **Phase 3: 旁路提示词接线**：narrative/prompts.rs(14) + pipeline/_(4) + intention_graph(1) + planner/_(4) + agents/*(6) + memory(1) + audit(1) + strategy(1) + deconstruction(4) + methodology(3) = 39 个提示词接入 registry
- **Phase 5: 前端 PromptsPanel 升级**：textarea → Monaco 编辑器（语法高亮/行号/Ctrl+S），新增 6 个分类标签，新增批量导入/导出功能（JSON 格式）

### 提示词覆盖统计

| 类别                                | 修复前 | 修复后 |
| ----------------------------------- | ------ | ------ |
| 真正可覆盖（走 resolve_prompt）     | 14     | 53+    |
| 假接入（走 resolve_prompt_default） | 15     | 0      |
| 完全旁路（硬编码）                  | 40+    | 0      |

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib intention_graph` ✅ 18/18 通过
- `npx tsc --noEmit` ✅ 零错误
- 真实模型测试 ✅ Gemma4-e2b 全流程通过

## [v0.20.1] - SING 意图图断环修复：让集成真正生效（2026-06-21）

### 审计背景

对 v0.20.0 SING 意图图集成进行审计后发现 5 处致命断环（P0）和 4 处理论对齐偏差（P1），导致整个意图图路径在运行时**从未生效**，所有请求静默回退到原有 PlanGenerator。本版本系统性修复全部问题。

### P0 断环修复（5 项）

- **P0-1 意图图数据填充**：`lib.rs` setup 阶段新增 `AssetSyncEngine::full_initialize` 调用 + `warm_up_cache`，将 CapabilityRegistry / SelectableAsset / 内置 Agent / 系统命令同步到意图图数据库。修复后 6 张意图图表不再为空，`discover_server_level` 能返回真实资产。共享预热缓存的 `IntentionGraphRepository` 注册为 Tauri state，`from_app_handle` 和 IPC 命令复用同一缓存实例。
- **P0-2 模型网关意图感知**：`GatewayRequest` 新增 `intent_verb` / `intent_object` 可选字段；`TaskClassifier::classify_task` 优先使用 `classify_by_intention` 进行意图感知分类，None 时回退到 TaskType + agent_id。修复后模型路由能基于 SING 意图动词-宾语精确映射复杂度。
- **P0-3 执行图持久化**：`execute_with_context` 意图图路径成功后调用 `record_execution_graph` 持久化执行图到数据库。修复后前端诊断面板"最近执行"不再为空。
- **P0-4 ReAct 真实执行**：`execute_with_react` 的 `Invoke` 分支改为通过 `invoke_fn` 回调真正执行步骤（替代硬编码 `{"status": "executed"}`），记录执行时间和输出，失败时标记 `Failed` 状态。执行图循环结束后持久化 graph + 所有 execution nodes。
- **P0-5 LLM 合成三阶段**：`IntentSynthesisPipeline::synthesize_query` 新增 LLM 增强版（`synthesize_query_with_llm`），用 LLM 理解自然语言意图并提取动词-宾语 JSON，失败时回退到规则匹配。`synthesize_full` 改为 async。

### P1 理论对齐修复（4 项）

- **P1-1 评分权重对齐论文**：`discover_tool_level` 评分公式从 `0.3·desc + 0.4·intent + 0.2·ppr + 0.1·collab` 改为论文等权 `desc + intent + ppr`（collab 作为 0.2 权重辅助信号）。
- **P1-2 PPR 真实传播**：`discover_server_level` 重写——构建异构图邻接表（intention→asset 双向边 + asset→asset tool_next/tool_cooccur 边），真正调用 `GraphScorer::ppr_propagate` 从根意图种子传播（此前是一跳邻域冒充 PPR）。`discover_tool_level` 从 server-level 结果接收真实 PPR 分数（替代 `0.5` 占位）。
- **P1-4 语义嵌入生成**：`AssetSyncEngine` 所有资产/意图节点创建时调用 `generate_embedding` 生成语义嵌入（Ollama/OpenAI provider 优先，失败 graceful fallback）。修复后 `discover_tool_level` 描述匹配走余弦相似度而非 Jaccard 词重叠。
- **P1-3 Server 节点**：StoryMoss 语境下 MCP server 未建模为独立图节点（无 `AssetType::Server` 变体），当前以 MCP tool 资产直接参与图传播。此为有意的简化，非缺陷。

### 验证

- `cargo check` ✅ 零错误（意图图相关警告已清理）
- `cargo test --lib intention_graph` ✅ 16/16 通过
- `npx tsc --noEmit` ✅ 零错误

## [v0.20.0] - SING 意图图集成：动态 ReAct + 分层发现（2026-06-21）

### 核心功能

- **SING 意图图架构**：全面集成 arXiv:2606.16591v2 论文的意图-工具异构图理论，实现从"关键词匹配"到"意图驱动"的智能创作调度范式升级。
- **意图合成流水线**：三阶段合成（Query Synthesis → Chain Expansion → Atomic Intention Extraction），将用户自然语言输入转化为原子化动词-宾语意图节点。
- **分层发现机制**：Server-level PPR 图传播 + Tool-level 描述/意图/图信号融合评分，动态发现最匹配的 Agent/Skill/MCP 工具。
- **动态 ReAct 执行**：Actions ∈ {Discover, Invoke, Respond}，工具集在运行时动态累积，支持意图漂移自适应。
- **模型网关意图感知**：`classify_by_intention()` 将 SING 意图动词映射到 TaskClass（LightTool/BalancedWork/HeavyCreation），实现意图感知的模型路由。

### 后端架构

- **新模块 `intention_graph/`**（11 个文件）：
  - `models.rs`：意图节点/资产节点/边类型/执行图等核心数据结构
  - `graph.rs`：SQLite + 内存缓存混合存储，`IntentionGraphRepository` 提供完整 CRUD
  - `builder.rs`：`IntentSynthesisPipeline` 离线意图合成
  - `discovery.rs`：`LayeredDiscovery` 分层发现 + `GraphScorer` PPR 评分
  - `reactor.rs`：`DynamicReactor` 动态 ReAct 执行循环
  - `planner.rs`：`IntentionGraphPlanner` wrapping PlanGenerator，提供 `generate_plan()` / `execute_with_react()`
  - `commands.rs`：2 个 IPC 诊断命令（`get_intention_graph_diagnostics` / `get_execution_graph_detail`）
  - `mod.rs` / `tests.rs`：模块组织 + 16 个单元测试
- **Migration 95**：6 张新表（`intention_nodes` / `asset_nodes` / `intention_asset_edges` / `asset_asset_edges` / `execution_graphs` / `execution_nodes`）
- **PlanExecutor 集成**：`execute_with_context()` 新增 IntentionGraphPlanner 路径，模板匹配 → 意图图 → PlanGenerator → 直接 Writer 四级回退
- **模型网关集成**：`model_gateway/dispatcher.rs` 新增 `classify_by_intention()` + `classify_by_object()`，3 个单元测试

### 前端诊断面板

- **意图图诊断页面**：`IntentionGraphDiagnostics.tsx` —— 统计卡片（意图/资产/边数量）、最近执行记录列表、执行图详情钻取（JSON 计划/结果可视化）
- **侧边栏入口**：新增「意图图」导航项（BrainCircuit 图标），`ViewType` 扩展 `'intention-graph'`

### 设计决策

- **零回归风险**：IntentionGraphPlanner 作为 PlanGenerator 的增强包装器，所有现有路径保持不变，仅在高置信度时启用意图图路径
- **混合存储**：热数据走内存缓存，冷数据持久化到 SQLite，启动时 `warm_up_cache()` 自动加载
- **离线合成**：意图合成不依赖 LLM 实时调用，通过规则 + 模板在本地完成

### 编译状态

- `cargo check` ✅ 零错误
- `cargo test --lib intention_graph` ✅ 16/16 通过
- `npx tsc --noEmit` ✅ 零错误

## [v0.19.0] - 提示词全面可配置化（2026-06-18）

### 核心功能

- **提示词注册表**：所有 35+ 内置 LLM 提示词集中注册到 `prompts/registry.rs`，支持 15 个分类（写作核心/质检/评点/规划/分析/世界观/角色/叙事/方法论/技能/记忆/知识/探测/系统/其他）。
- **前端提示词管理面板**：`PromptsPanel` 全面重构——支持分类筛选、关键词搜索、批量重置、内置默认值预览、模板变量高亮显示。
- **运行时覆盖生效**：所有 LLM 调用统一经 `resolve_prompt()` 读取，优先使用用户自定义覆盖，无缝回退到内置默认。

### 后端迁移

- **Agent 服务**：`build_writer_prompt` / `execute_commentator` / `execute_style_mimic` / `execute_plot_analyzer` / `execute_memory_compressor` / `execute_knowledge_distiller` 全部改用 `resolve_prompt()`。
- **技能系统**：`SkillExecutor` 新增 `with_db_pool()` 支持，执行时根据 `skill_id` 映射到 `prompt_id`，从 PromptRegistry 读取覆盖的 system_prompt。
- **记忆系统**：`IngestPipeline::extract_narrative_events` 改用 `resolve_prompt("narrative_event_extraction")`，带 fallback 提示词。
- **方法论**：`SnowflakeStep::prompt_instruction()` 优先从 PromptRegistry 读取 10 步雪花法提示词，保留硬编码 fallback。
- **多助手系统**：`memory/multi_agent.rs` 5 个 Agent 系统提示词改用 `resolve_prompt_default()`。

### 前端改进

- **GeneralSettings 提示词入口**：替换旧版 2 个 textarea 覆盖为「提示词注册表」卡片，一键跳转到完整管理面板。
- **PromptsPanel 搜索与筛选**：实时搜索（ID/名称/描述/内容）、15 分类下拉筛选、搜索结果计数。
- **批量重置**：支持一键重置所有自定义覆盖，带确认对话框。
- **默认值预览**：已覆盖的提示词展开时显示内置默认值（只读），方便对比修改。

### API 新增

- `reset_all_prompt_overrides` IPC 命令：批量删除所有 `prompt_overrides` 表记录。

## [v0.18.1] - 设置超时修复：数字输入体验 + 配置读取路径（2026-06-20）

### 修复

- **前端数字输入过快保存**：后台设置「超时设置」数字输入框从 `onChange` + 300ms 防抖改为 **本地 state + `onBlur` 保存**。用户输入多位数字（如 300→600）时，不再在输入中途弹出「设置已保存」。
- **后端超时配置不生效**：修复 3 处 `AppConfig::load` 错误使用 `std::env::current_dir()` 而非 `app_handle.path().app_data_dir()` 的问题：
  - `commands/orchestrator.rs` `smart_execute` 总超时读取
  - `planner/executor.rs` 单步超时读取
  - `model_gateway/executor.rs` 探测提示词覆盖读取
  - **根因**：配置存储在应用数据目录（`~/Library/Application Support/com.storymoss.app/`），但代码从当前工作目录读取，导致总是读到默认值（180 秒），用户在前端设置的 600 秒总超时永不生效。

## [v0.18.0] - 后台资产深度审计 × 智能创作流程全面优化（2026-06-20）

### 审计背景

对后台资产（技能/能力/方法论/提示词/风格/网文资产/状态/记忆）与智能创作流程的关联进行了全面深度审计，发现核心矛盾：**默认续写路径（TimeSliced）绕过约 90% 后台资产**，用户最高频操作落在资产最稀薄的路径上。此外存在多处"建好但未接通"的断环和死代码。本版本系统性修复全部问题（P0-P3 共 14 项）。

### P0 断环修复（4 项）

- **P0-1 ingest→伏笔自动追踪闭环**：`IngestPipeline::run_ingest` 新增 `persist_foreshadowings`，将分析阶段提取的 setup 型伏笔按 (story_id, content) 去重后写入 `foreshadowing_tracker`。此前续写中隐含埋设的伏笔永不被追踪，回收账本不完整。
- **P0-2 character_states 写入闭环**：`IngestPipeline` 新增 `persist_character_states`，从 Character 实体 attributes 提取 location/mood/goal，按名称匹配 characters 表后 INSERT OR REPLACE 到 `character_states`（保留既有 secrets/arc_progress）。此前 `【角色当前状态】` prompt 段永远为空。
- **P0-3 内置 MCP 工具自动注册进 CapabilityRegistry**：`lib.rs` setup 阶段用 `try_lock` 读取 `BUILTIN_MCP_SERVER` 工具，对每个调 `Capability::from_mcp_tool("builtin", tool)` + `registry.register`。此前 PlanGenerator 输出的 `mcp.builtin.*` 步骤被验证器丢弃。
- **P0-4 四元组资产内容完整展开**：`serialize_quartet_for_prompt` 增加 category/when_to_use（beat_cards）、pairs_well_with（engines）、works_with（pressure_relationships）；`render_narrative_quartet_section` 同步渲染新字段。此前只渲染 ID/名称。

### P1 核心优化（4 项）

- **P1-1 TimeSliced 接入精选资产子集（核心）**：`WriteTimeBundle` 扩展 5 个新字段（叙事阶段、待回收伏笔 top3、逾期伏笔 top1、主导风格摘要、叙事四元组）；`load_sync` 经 `CreativeAssetSnapshot` 加载规范状态快照 + StyleDNA 摘要；`to_prompt` 渲染 5 个新 section；`execute_time_sliced` 从 task.parameters 提取四元组注入 bundle。**解决"资产黑洞"——默认续写路径现在能感知伏笔状态、叙事阶段、风格约束和四元组。**
- **P1-2 接通 3 个休眠技能**：`apply_writing_skills` 新增场景智能激活——`character_voice`（引号密度≥3 对触发）、`plot_twist`（叙事阶段为转折/高潮时触发）、`text_formatter`（连续空行或超长行触发）。此前这三个技能已注册但创作流从不调用。
- **P1-3 Full 模式 Inspector 接入全量上下文**：`build_inspector_prompt` 注入世界观规则、待回收/逾期伏笔（经 CreativeAssetSnapshot）、叙事阶段、风格 DNA 约束。此前 Inspector 是"半盲"的，只看 title/genre/characters/content，无法对照设定检查。
- **P1-4 审计触发自动 Rewrite 建议**：新增 `SyncEvent::AuditRewriteSuggested` 事件变体；`AuditExecutor` 发现 high 严重性问题时发射事件，前端可提示用户是否修订（保持用户控制权，不静默改文）。

### P2 死代码清理（3 项）

- **P2-1 删除死代码**：删除 `prompts/methodologies/`（雪花/英雄/场景旧实现，与 `creative_engine/methodology/` 重复）、`prompts/evolver.rs`（PromptEvolver，零调用方）、`state/`（StoryStateManager，标注 RESERVED 已久）、`evolution/`（EvolutionReviewer/Analyzer/Updater，零调用方）；`prompts/mod.rs` 移除 PromptManager + PromptTemplateDef。共删除 5 个目录/文件 + 4 个 mod 声明。
- **P2-2 消除 prompt 旁路**：`execute_commentator` 改用 `resolve_prompt("commentator_system")` 模板（支持前端"提示词覆盖"功能生效）。此前评点家用内联硬编码 prompt，前端覆盖无效。
- **P2-3 删除死表**：Migration 94 `DROP TABLE IF EXISTS beat_cards/story_engines/pressure_relationships`（Migration 92 创建但从未读写，资产仅存内存 `builtin_*()`）。

### P3 架构性优化（3 项）

- **P3-1 Pro/Free 资产边界精细化分层**：`build_writer_prompt` 重构——单一 StyleDNA 移出 `is_pro`（Free 可用）、写作风格设定 + 作品简介移出 `is_pro`（所有用户可用）、StyleBlend + 方法论 + 个性化保持 Pro-only。此前 `if is_pro` 一刀切挡住六类资产。
- **P3-2 启用资产自动匹配**：`build_selected_strategy` 当 story 未显式设定资产时，按 `story.genre` 自动匹配 GenreProfile（纯 DB 查询，无 LLM 延迟），让四元组推断对未配置资产的用户也能生效。
- **P3-3 统一资产注入网关**：新建 `creative_engine/asset_snapshot.rs`（`CreativeAssetSnapshot` 统一加载器），`WriteTimeBundle::load_sync` 和 `build_inspector_prompt` 均改用该网关，消除重复的规范状态/风格 DNA 加载逻辑。

### CI 修复

- **修复 48 个测试失败**：删除 `migrations/` 目录中重复的 V092/V093 SQL 文件——它们与 `run_migrations` 函数中的内联 Migration 92/93 重复，导致 `MigrationRunner::run()` 先运行 SQL 文件将 `current_version` 提升到 93，使 `run_migrations` 中的 Migration 28-91 全部跳过（`genre_profile_id` 列、`character_relationships` 表等不创建）。

### 文档

- 新增 `docs/AUDIT_后台资产与智能创作流程.md`——完整审计报告（资产清单、可达性矩阵、逐域发现、根因分析、优化方案）
- 新增 `docs/CREATION_FLOW_AND_ASSETS_REFERENCE.md`——智能创作流程 × 后台资产权威参考文档

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **444/444** 通过（零回归）
- `npx tsc --noEmit` ✅ 零错误
- `cargo +nightly fmt -- --check` ✅ 通过

## [v0.17.1] - 智能后台预访谈 + Anti-AI 改写闸 + 在世作者保护 + 提示词注册表 + 关键 Bug 修复（2026-06-19）

### 关键 Bug 修复

- **修复超时设置保存失败 undefined**：`AppSettingsData` 长期缺失 v0.16.0 引入的 timeout / advanced override 字段（`frontend_timeout_secs` / `executor_step_timeout_secs` / `smart_execute_total_timeout_secs` / `llm_connect_timeout_secs` / `llm_first_chunk_timeout_secs` / `style_weight` / `narrative_weight` / `skip_rewrite_threshold` / `keep_revision_history` / `context_budget_ratio` / `generation_mode` / `writer_system_prompt_override` / `probe_prompt_override`），任何尝试调整超时数字都触发 IPC 反序列化失败。修复：后端 `AppSettingsData` 全部字段加 `#[serde(default)]` + 13 个新 `Option<T>` 字段；`save_settings` 按 `if let Some()` 应用；`get_settings` 同步暴露；前端 `SettingsContext.updateSettingsMutation` 读取 query 缓存合并 patch 后下发完整对象。
- **模型健康报告增强**：`ModelHealthReport` 新增 `total_calls` / `last_called_at` / `generated_at` 三字段；前端 `useModelHealthReports` 关掉缓存（`staleTime: 0` / `gcTime: 0` / `refetchOnMount: 'always'`）确保每次打开 Settings/模型健康都是最新数据；ModelHealthPanel 头部显示「数据更新于 X」与每个模型的「近期调用次数」「最近一次调用」时间，让用户能直接判断数据新鲜度。

### 提示词注册表（PromptRegistry）

把分散在 `prompts/engine.rs` / `llm/prompt.rs` / `task_system/audit_executor.rs` 的硬编码 prompt 全部抽取到统一注册表：

- **Migration 93** `prompt_overrides` 表 —— `prompt_id` PK + `overridden_content` + `updated_at`。
- **`prompts/registry.rs`**：8 个内置 prompt（writer_system / writer_continue / writer_rewrite / inspector_system / style_checker_system / outline_planner / commentator_system / model_gateway_probe），分 6 类（写作核心 / 审校与质量 / 评点 / 规划 / 分析 / 探测），含名称 / 描述 / 变量列表 / 默认内容元数据；`list_prompts()` / `resolve_prompt()` / `save_override()` / `reset_override()` API。
- **IPC 命令**：`list_prompt_entries` / `save_prompt_override` / `reset_prompt_override` / `resolve_prompt_content`（全部 `rename_all = "snake_case"`）。
- **前端 PromptsPanel**：Settings 新增「提示词」标签页，按分类折叠分组展示，每条 prompt 可展开编辑，显示「已覆盖 / 未保存 / 当前内置默认」状态徽章 + 支持的模板变量列表 + 保存覆盖 / 恢复默认按钮。
- **运行时接入**：`AgentService::resolve_prompt(id)` 优先查 DB override，否则回退默认；Writer / Inspector / OutlinePlanner 全部经 registry 读取；Model Gateway 探测 prompt 同步接入 registry → AppConfig.probe_prompt_override → 内置默认三层降级。
- **5 个单测** + 验证零回归（391 → 396 passing）。

### 智能后台预访谈（v0.17.0 后续）

- **InputClarity 三档判定**：`intent.rs::detect_input_clarity()` —— Vague / WithSeed / WithFullConcept，启发式不调 LLM；6 个单测。
- **NarrativeQuartet 透明推断**：`strategy/quartet_inference.rs` —— 当用户输入处于 Vague/WithSeed 时，后端透明补全 5 元组（emotional_payoff / pressure_relationship / conflict_arena / story_engine / beat_card）；不弹卡片，全部走 GenreProfile.reader_promise + 默认推荐；8 个单测。
- **Writer Prompt 注入**：PlanExecutor::execute_writer 把序列化后的四元组写入 `task.parameters["narrative_quartet"]`；`build_writer_prompt` 末尾追加「叙事四元组」段；3 个单测。

### Anti-AI 改写闸 + 开篇清晰度门（骨架）

- **AI cliché 词表 +7**：关键在于 / 值得注意的是 / 综上所述 / 让我们 / 在某种程度上 / 与此同时 / 这一切的背后。
- **`anti_ai/rewriter.rs`**：`AntiAiRewriter` 骨架 —— `RewriteStrategy`（LocalReplace / ParagraphRewrite / ChapterRewrite）+ `should_trigger`（overall_score < 60 或任一 high severity）+ 异步 `rewrite()` 入口（v0.17.1 直接返回原文，v0.17.2 接 LLM）；4 个单测。
- **`audit/opening_clarity.rs`**：6 要素门（Danger / Humiliation / Loss / Puzzle / PhysicalAnchor / GenreSignal），按前 200 字检查；`signal_for_genre()` 为 5 种主流题材（赘婿 / 修真 / 末世 / 悬疑 / 校园）提供差异化检测词；5 个单测。
- **AuditExecutor 7→11 维**：`task_system/audit_executor.rs` prompt 扩 4 维（desire / payoff / aftertaste / opening_clarity），`dimension_priority` / `dimension_label` 同步更新；2 个新单测。

### 在世作者保护

- **`creative_engine/style/living_author_guard.rs`** —— 在世作者黑名单 41 位（中文 26 + 外文 15），命中即替换为「具备相同手工艺特征的写作风格」+ 自动追加「手工艺滑块」段（5 维 × 3 档：句长偏好 / 对话比例 / 比喻密度 / 内心独白比例 / 视角粘度）。
- **`build_writer_prompt` 接入**：在最终组装后调用 `sanitize_style_brief()` 自动清洗。
- 6 个单测。

### 验证

- `cargo check` 零错误（33 warnings 全为既有）
- `cargo test --lib` 396 passed / 48 failed（48 为 v0.17.0 起 V092 测试 DB 基线，零新回归）
- `npx tsc --noEmit` 零错误
- 新增 33 个单测全部通过（6 + 8 + 3 + 4 + 5 + 2 + 6 + 5 prompt registry）

## [v0.17.0] - 中文叙事增强：桥段卡 / 剧情引擎 / 高压关系 / 读者承诺四件套（2026-06-19）

### 新增功能

v0.17.0 是中文叙事能力的一次系统性升级，引入业界共识级的四类创作资产，与既有的方法论 / 体裁画像 / Style DNA 三轴互补，让模型在创作前能够同时考虑「叙事动力 + 关系张力 + 读者爽点」。

#### 1. 31 张经典桥段卡（Beat Cards）

**文件**：`src-tauri/src/creative_engine/beat_cards/`（mod.rs + registry.rs）

把流传已久的戏剧 / 章回 / 中文网文桥段抽象为「可复用功能 + 重构提示」的 31 张卡，分 7 大类：

| 类别               | 数量 | 代表卡片                                                         |
| ------------------ | ---- | ---------------------------------------------------------------- |
| 跌落与回归         | 4    | 跌落归来式 / 弱者挑战式 / 长线复仇式 / 守住尊严式                |
| 公开证明与打脸     | 4    | 拍卖鉴宝式 / 庭审翻案式 / 规则破解式 / 话语权反转式              |
| 身份与识别         | 5    | 低估识别式 / 身份互换式 / 失忆识身式 / 藏锋入禁式 / 失位继承式   |
| 悬疑与真相重构     | 5    | 多视角重述式 / 前提反转式 / 线索错序式 / 细节翻案式 / 无心证物式 |
| 情感拉扯           | 4    | 误判与重识式 / 冷面伤痕式 / 迟来认知式 / 护卫秘恋式              |
| 制度与规则压力     | 5    | 制度夹缝式 / 团队劫案式 / 荒诞规则式 / 崩盘先知式 / 倒计时生存式 |
| 后台视角与组织讽刺 | 4    | 后台视角式 / 表里错位式 / 继任考验式 / 档案链揭露式              |

每张卡含五要素：可复用功能 / 何时使用 / 重构提示 / 反例 / 标签。所有卡片名称与描述使用通用化中文，不绑定特定作品。

#### 2. 21 种剧情引擎（Story Engines）

**文件**：`src-tauri/src/creative_engine/story_engines/mod.rs`

正交叙事动力库，可组合 2-4 个使用：隐藏身份 / 重生回溯 / 契约绑定 / 错认身份 / 双强博弈 / 成长阶梯 / 公开舞台 / 竞价鉴价 / 试炼评测 / 阴谋线索链 / 封印记忆 / 禁忌交易 / 敌人庇护 / 阶层错位 / 规则漏洞 / 外行仪式障 / 物证关键 / 强制低谷 / 后台任务视角 / 利益相关碰撞 / 程序倒计时。

#### 3. 13 种高压关系（Pressure Relationships）

**文件**：`src-tauri/src/creative_engine/pressure_relationships/mod.rs`

冲突放大器：真假继承人 / 前夫前妻 / 替身白月光 / 赘婿与岳家 / 师徒宗门 / 上位者与外来者 / 后台执行者与台前英雄 / 继任者与守门人 / 救命恩人与错认者 / 仇人合作者 / 债主与欠债人 / 亲人继亲 / 护卫与被护者。

#### 4. 体裁读者承诺（Reader Promise）

**文件**：`src-tauri/src/creative_engine/reader_promise.rs` + Migration 92

`genre_profiles.reader_promise` 字段：用 9 种基础情绪（爽 / 甜 / 虐 / 恨 / 惊 / 燃 / 怕 / 痛 / 治愈）+ 衍生爽点描述每个体裁的核心读者期待。43 个内置体裁全部映射 reader_promise，启动期回填，已设置值不会被覆盖。

### 资产架构升级

- **AssetKind 扩展（+3 变体）**：BeatCard / StoryEngine / PressureRelationship
- **SelectedStrategy 扩展（+5 字段）**：emotional_payoff / pressure_relationship_id / conflict_arena / story_engine_ids / beat_card_ids
- **StrategyOverrides 同步扩展**：用户可在 UI 中锁定四元组取值
- **strategy/asset_catalog.rs**：新增 3 个工厂函数 + 自动并入 load_assets_with_genre_profiles —— 既有 StrategySelector 的 LLM 路由立即对新资产生效

### 数据库迁移

Migration 92（V092__中文叙事增强）：

- `genre_profiles.reader_promise` 字段（带列存在性检查，幂等）
- 三张新表：`beat_cards` / `story_engines` / `pressure_relationships`
- 索引：`category` / `is_builtin`

### 下一步迭代（v0.17.1+）

- **v0.17.1**：智能后台预访谈 —— 输入清晰度检测 + LLM 自动选四元组并注入 Writer prompt
- **v0.17.2**：反 AI 味自动改写闸 + 开篇清晰度门 + 11 维质量门
- **v0.17.3**：在世作家风格信号翻译（living-author guard）

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **357 passed**（v0.16.2 基线 344 + 新增 13）零回归
- `npx tsc --noEmit` ✅ 零错误

## [v0.16.2] - 修复后台审计（AuditExecutor）LLM 调用误导前端假超时（2026-06-18）

### 问题

用户输入"写第二章"后诊断卡片弹出：`最后事件类型: 最终输出 / async-audit-inspector 完成`。智能创作实际已进入 TimeSliced 分时模式，正文生成返回后前端收到 "已完成" 事件，但紧接着后台 `AuditExecutor`（时间线 2）被 spawn 执行 Inspector LLM 调用，其 `emit_llm_progress` 同步发射 `generation-status` 事件，覆盖了主流程的 "已完成"，让前端误以为主流程仍在运行，最终 200s 超时弹出诊断卡。

### 根因

TimeSliced 模式在正文生成后 `tokio::spawn` 启动异步审计，审计中的 `generate_for_task` 调用了 `emit_llm_progress`，其中 `request_id` 非空时同步发出 `generation-status` 事件（phase=`FinalOutput`/ 最终输出）。该 label `"async-audit-inspector"` 未被纳入 `is_silent_background` 白名单，导致前端收到错误的状态事件。

### 修复

#### 后端：扩展 silent_background 白名单

**文件**：`src-tauri/src/llm/service.rs:835-846`

将时间线 2/3 的后台任务 label 纳入静默白名单：

- `async-audit-inspector`（异步 Inspector 审计）
- `async-insight`、`async-deep-insight`（深度洞察）
- `background-summary`（背景摘要）

命中后跳过所有 `emit_llm_progress` 与心跳发射。

#### 前端：主流程完成标记

**文件**：`src-frontend/src/frontstage/FrontstageApp.tsx`

新增 `mainGenerationCompletedRef`：

- `handleGenerationStatus` 在 phase="已完成"/"出错"/"已取消" 时置位
- `updateLastEventTime` 跳过已完成后的更新时间
- `scheduleFallbackPrompt` tick 跳过已完成后的状态更新
- 新一轮 `startElapsedTimer` 重置标记

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **392/392** 通过
- `npx tsc --noEmit` ✅ 零错误

## [v0.16.1] - 修复"距上次响应 80006 秒"计数 Bug（2026-06-18）

### 问题

诊断卡偶尔显示"距上次响应 80006 秒"的荒谬数值。根因：`lastEventTimeRef.current` 初始化为 `Date.now()`，但若首次事件延时较长，其与 `Date.now()` 的时间差被解释为"距上次响应"，导致天文数字。

### 修复

**文件**：`src-frontend/src/frontstage/FrontstageApp.tsx:645-660`

- `lastEventTimeRef` 初始值从 `Date.now()` 改为 `null`
- tick 函数添加 `null` 守卫，首次 tick 不产生 `sinceLastEvent`
- `sinceLastEvent` 变量声明修复避免 TS2304

### 验证

- `cargo check` ✅ 零错误
- `npx tsc --noEmit` ✅ 零错误

## [v0.16.0] - 智能创作参数全面可配置（2026-06-18）

### 新增功能

所有超时参数与创作参数可从前端设置界面配置，无需修改代码。

#### 前端设置（GeneralSettings 新增 3 张卡片）

**文件**：`src-frontend/src/pages/settings/GeneralSettings.tsx`

| 卡片       | 字段                                                                                                                                                                            | 默认值       |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| 创作参数   | `skip_rewrite_threshold` / `style_weight` / `narrative_weight` / `context_budget_ratio` / `writer_local_concurrency` / `keep_revision_history`                                  | 各参数默认值 |
| 超时设置   | `llm_connect_timeout_secs`(30) / `smart_execute_total_timeout_secs`(180) / `executor_step_timeout_secs`(90) / `frontend_timeout_secs`(200) / `llm_first_chunk_timeout_secs`(60) | 中文描述     |
| 提示词覆盖 | `writer_system_prompt_override` / `probe_prompt_override`                                                                                                                       | 可选自定义   |

#### 后端配置扩展

**文件**：`src-tauri/src/config/settings.rs`

AppConfig 新增字段组，持久化到 `app_config.json`：

- 超时组：`llm_connect_timeout_secs` / `smart_execute_total_timeout_secs` / `executor_step_timeout_secs` / `frontend_timeout_secs` / `llm_first_chunk_timeout_secs`
- 创作参数组：`writer_local_concurrency` / `context_budget_ratio` / `skip_rewrite_threshold`
- 提示词覆盖组：`writer_system_prompt_override` / `probe_prompt_override`

#### 前段类型扩展

**文件**：`src-frontend/src/types/llm.ts`

`AppSettings` 接口新增所有配置字段。

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **392/392** 通过
- `npx tsc --noEmit` ✅ 零错误

## [v0.15.2] - 修复「已完成」事件在错误检测前发射（2026-06-18）

### 问题

`smart_execute` 路径中 `emit_progress("completed", ...)` 在成功判定之前执行，导致即使后端返回空内容或错误，前端仍收到"已完成"事件。诊断卡显示已完成但正文为空。

### 修复

**文件**：`src-tauri/src/commands/orchestrator.rs:711-755`

移动 `emit_progress("completed")` 至成功路径末尾——仅在实际内容非空且 `result.success == true` 时才发射。失败路径改发 "error" 事件。

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **392/392** 通过

## [v0.15.1] - 生成阶段提示汉字化（2026-06-18）

### 变更

**文件**：`src-tauri/src/events.rs:17-36`

`GenerationPhase::as_str()` 返回值从英文改为中文：

- `PreparingContext` → "准备上下文"
- `GeneratingCandidates` → "候选生成"
- `Inspecting` → "内容审校"
- `Rewriting` → "润色改写"
- `FinalOutput` → "最终输出"
- `Completed` → "已完成"

所有阶段事件在底部状态栏与诊断卡片中均以中文显示，提升中文用户的使用体验。

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **392/392** 通过

## [v0.15.0] - 模型网关智能调度器（ModelGateway Smart Scheduler）（2026-06-17）

### 新增功能

模型网关（ModelGateway）新增智能调度层，自动对创作任务分类并选择最优模型。

#### Capability Profile 系统

- **新表**：V091 迁移 `model_capability_profile` 记录每个模型在各维度（写作/分析/创意/速度/性价比）的评分
- **新模块**：`src-tauri/src/model_gateway/capability_store.rs` 提供 CRUD 操作
- **TaskClassifier**：按 Agent 类型 + prompt 长度将任务分为 `LightTool` / `BalancedWork` / `HeavyCreation` 三类

#### Streaming TTFB 真实基准测试

**文件**：`src-tauri/src/model_gateway/benchmark.rs`

- 长测（固定 prompt + 512 max_tokens）和短测（固定 prompt + 64 max_tokens）
- 使用真实首个数据块时间戳（`Duration::elapsed`），替代旧版魔术 `duration-10` 估算
- 结果写入 `model_gateway_benchmarks` 表

#### 3D 智能评分路由

**文件**：`src-tauri/src/model_gateway/executor.rs`

`select_candidates` 使用三维加权评分选择最优模型：

- **能力匹配（capability）** 50%
- **用户偏好（preference）** 30%
- **任务拟合度（fit）** 20%

#### 网关路由全覆盖

**文件**：`src-tauri/src/agents/service.rs:723`

关键修复：所有 LLM 调用路径均经过网关。此前 `execute_agent` 通过 `AgentMapping` 直接读取 profile 绕过调度器，导致智能调度失效。现统一从网关 `resolve_task` 获取模型 ID，智能调度对所有创作任务生效。

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **392/392** 通过
- `npx tsc --noEmit` ✅ 零错误

## [v0.14.4] - 修复"应用启动后自动进入生成进程"假象（2026-06-18）

### 问题

用户反馈：v0.14.3 安装版应用一启动就自动进入"最终输出"或类似生成进程的状态栏显示，实际上用户并未发起任何生成请求。

### 根因

应用启动时 `GatewayScheduler::start_periodic_probe`（src-tauri/src/model_gateway/scheduler.rs:47）会自动对所有已配置模型发起健康探测：

```
[GatewayScheduler] 启动时全量探测 2 个模型
[LLM] Starting sync generation with profile=Qwen3.5-27B-Uncensored-Q4_K_M prompt_len=33
```

`probe_model` 通过 `LlmService::generate_with_profile_and_request_id` 调用，该路径会：

1. 启动 10 秒间隔的心跳任务，发射 `llm-generating-progress` 事件
2. 发射 `connecting` / `sent` / `completed` / `error` 阶段事件

前端 `FrontstageApp.tsx` 的 `llm-generating-progress` 监听器**无条件**调用 `setGenerationStatus(...)`，把 "AI 正在深度思考中..." 文案显示在状态栏，让用户误以为创作任务正在运行。

类似问题：`get_input_hint`（输入框提示）、`detect_intent`（意图识别）等轻量后台 LLM 调用也会触发同样假象。

### 修复

#### 后端：`is_silent_background` 标识 + 全路径跳过

**文件**：`src-tauri/src/llm/service.rs:826-832`

在 `execute_generation` 中识别静默后台调用：

```rust
let is_silent_background = matches!(
    label,
    "model_gateway_probe" | "input_hint" | "intent_detection"
);
```

对静默调用：

- **跳过心跳任务**：`heartbeat_handle` 改为空闭包（保留 abort handle 接口）
- **跳过 emit_llm_progress**：connecting / sent / completed / error 全部跳过
- 行为透明，仅日志记录，前端零打扰

#### 前端：兜底防护

**文件**：`src-frontend/src/frontstage/FrontstageApp.tsx`

`llm-generating-progress` 监听器加入早返回：

```typescript
if (!isGenerating && !smartExecuteInFlightRef.current) {
  return; // 用户未发起生成，忽略心跳事件
}
```

防止未来其他后台调用通道再次触发同样假象。

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **392/392** 通过
- `npx tsc --noEmit` ✅ 零错误
- `NODE_ENV=test npx vitest run` ✅ 126/126 通过
- `cargo +nightly fmt` ✅ 通过

### 预期效果

| 场景         | 修复前                               | 修复后               |
| ------------ | ------------------------------------ | -------------------- |
| 应用启动     | 状态栏显示"AI 正在深度思考中..."假象 | 状态栏静默 ✅        |
| 启动健康探测 | 触发心跳事件被前端误显示             | 后台静默完成 ✅      |
| 用户主动续写 | 正常显示生成进度                     | 正常显示（无回归）✅ |
| 用户主动重写 | 正常显示生成进度                     | 正常显示（无回归）✅ |

## [v0.14.3] - 智能创作生成内容根因修复——场景智能路由（2026-06-17）

### 真正的根本症结（v0.14.2 超时防线之上）

v0.14.2 建立了五层超时防线，但**仅能避免无限等待，不能让生成真正完成**。继续深入查找发现：

**`smart_execute` 路径写死 `GenerationMode::Full`**，导致每次智能创作触发：

- 1 次 Writer（含 prepare_writer_context + 候选生成）
- 最多 2 次 Inspector（每次 50s）
- 最多 2 次 Rewrite（每次 67s）

累计最多 **5 次同步 LLM 调用 × 50-67s = 250-335 秒**，远超 v0.14.2 的 200s 前端超时和 180s 后端超时，**必然超时退出，无法生成内容**。

**设计文档实施遗漏**：`docs/plans/2026-06-14-time-sliced-intervention-design.md:456` 明确指定 `smart_execute` 默认 `TimeSliced`，但实施时只改了 `agents/executor.rs:79`（任务系统入口），漏改了 `smart_execute` 实际走的 `PlanExecutor::execute_writer` 路径。

### 修复内容（场景智能路由）

#### 第一层：PlanExecutor::execute_writer 场景智能路由（P0 - 核心）

**文件：`src-tauri/src/planner/executor.rs:803-833`**

添加场景智能路由逻辑，优先级：plan 参数 > AppConfig.generation_mode > 场景智能默认：

- `selected_text` 非空（重写选中文本）→ `GenerationMode::Full`（含质检改写）
- `selected_text` 为空（续写或新章节首段）→ `GenerationMode::TimeSliced`（单次 LLM，30-60s）
- 用户可在设置或 plan 参数中显式覆盖（`auto`/`time_sliced`/`fast`/`full`）

#### 第二层：AppConfig 新增 generation_mode 配置（P1）

**文件：`src-tauri/src/config/settings.rs`**

- 新增 `AppConfig.generation_mode` 字段，默认 `"auto"`（场景智能路由）
- 可选值：`auto`、`time_sliced`、`fast`、`full`

#### 第三层：前端 GeneralSettings 暴露生成模式下拉（P1）

**文件：`src-frontend/src/pages/settings/GeneralSettings.tsx`**

在"写作策略"区域顶部新增"AI 生成模式"下拉，4 个选项：

- **智能路由（推荐）**：续写快速、重写精修
- **分时模式**：最快（30-60秒）
- **快速模式**：单次 + 风格技能（约 60秒）
- **精修模式**：含质检改写（2-5 分钟）

#### 第四层：超时与 max_tokens 默认值优化（P2）

**文件：`src-tauri/src/config/settings.rs`**

- `DEFAULT_LLM_TIMEOUT_SECONDS`: 240 → **120**（生成 1500 tokens × 30 tokens/s ≈ 50s，120s 留 2.4× 余量）
- LlmProfile 默认 `max_tokens`: 8192 → **2500**（续写 800-1500 字足够，避免 LLM 生成过长内容拖慢响应）

### 路径一致性验证

| 路径                                          | 修复前        | 修复后                 |
| --------------------------------------------- | ------------- | ---------------------- |
| AiGenerationExecutor（任务系统）              | TimeSliced ✅ | TimeSliced             |
| smart_execute → PlanExecutor → execute_writer | **Full ❌**   | **场景智能路由**       |
| Genesis（新建小说）                           | Full          | Full（保持不变）       |
| auto_write / auto_revise                      | TimeSliced ✅ | TimeSliced             |
| Workflow Scheduler                            | Full          | Full（自动化保持不变） |

### 预期效果

| 场景             | v0.14.2 之前    | v0.14.3                    |
| ---------------- | --------------- | -------------------------- |
| 续写（最常见）   | 200-300s 必超时 | **30-60s 稳定生成**        |
| 重写选中文本     | 200-300s 必超时 | 60-180s（Full 受预算约束） |
| 新章首段         | 200-300s 必超时 | **30-60s 稳定生成**        |
| 新建小说 Genesis | 不变（双阶段）  | 不变                       |

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ **392/392** 通过
- `npx tsc --noEmit` ✅ 零错误
- `NODE_ENV=test npx vitest run` ✅ 126/126 通过
- `cargo +nightly fmt` ✅ 通过

### 核心价值

**用户最常用的"续写"场景从"必然超时无法生成"变为"30-60 秒稳定生成"**，智能创作功能从不可用变为可用。质量改善由后台审计（time-sliced 第二阶段）异步保证，不阻塞用户写作流。

## [v0.14.2] - 智能创作超时退出根因修复：多层超时防线（2026-06-17）

### 核心问题

智能创作（`smart_execute`）在"准备上下文"阶段长时间延时后退出且不弹诊断卡片，导致用户无法判断失败原因。经全面检视，根因是**多层超时缺失的系统性问题**：

- 后端 `smart_execute` 无整体超时，可能无限等待
- LLM 生成超时按 chunk 刷新，vllm "连接成功但首字节迟迟不来"时可挂满 240s 甚至更久
- Full 模式 270s 预算是声明但从未调用的**死代码**
- 准备上下文阶段的同步 DB 调用阻塞 tokio worker，可能饿死心跳
- 前端 330s 超时远长于后端实际需要的处理时间，用户体验差

### 修复内容（快速失败优先策略）

#### 第一层：后端 smart_execute 整体超时（P0）

- `smart_execute` 函数体提取为 `smart_execute_inner`，外层包裹 **180 秒** `tokio::time::timeout`
- 超时时主动调用 `LlmService::cancel_all_generations()` 取消所有进行中 LLM 生成，避免孤儿任务
- 超时时发射 `smart-execute-progress` timeout 事件通知前端

#### 第二层：每步骤 + 每阶段超时（P0）

- `PlanExecutor::execute_step` 单步超时 **90 秒**，超时记为 step failed 但不中断后续批次（保持容错语义）
- 激活 Full 模式预算死代码：Inspector/Rewrite 调用受 `remaining_budget_secs()` 约束
- Inspector 循环开头增加预算检查：剩余时间 <30s 时跳过质检，直接返回 Writer 结果
- Inspector/Rewrite 各自用 `tokio::time::timeout` 包裹，超时 break 保留当前内容

#### 第三层：修复 LLM 生成超时（P0 - 防止 vllm 半挂）

- `read_body_with_generation_timeout` 增加**首字节超时**：首个 chunk 使用 `min(generation_timeout, 60s)`
- 增加**绝对超时**：从开始读取到结束不超过 `generation_timeout * 1.5`
- per-chunk 超时保持不变，支持本地模型慢速但持续输出

#### 第四层：准备上下文阶段 spawn_blocking（P1）

- `build_agent_context` 中 `CanonicalStateManager::get_snapshot_sync`（多表聚合）用 `spawn_blocking` 包裹
- `ForeshadowingTracker::get_writing_hints` 和 `StoryRepository::get_by_id` 同样包裹
- `smart_execute` Step 4 风格 DNA 查询和 `build_selected_strategy` 用 `spawn_blocking` 包裹
- 消除同步 DB 阻塞 tokio worker 导致心跳饿死的风险

#### 第五层：前端超时与诊断卡片完善（P1）

- 前端超时从 330s 降至 **200s**（后端 180s + 20s 余量），确保前端总在后端之后超时
- 前端超时触发时调用 `llm_cancel_all_generations` 通知后端取消（best-effort）
- 新增 `llm_cancel_all_generations` Tauri 命令并注册到 invoke handler
- `useBackendActivityListener`：`plan-executor-step` 收到 `failed` 状态时保持 activity 为 `running`，避免在 invoke reject 到达前清空 isGenerating 导致安全网误判

### 超时值汇总

| 环节                | 修复前                          | 修复后                          |
| ------------------- | ------------------------------- | ------------------------------- |
| smart_execute 整体  | 无限                            | 180s                            |
| PlanExecutor 单步   | 无限                            | 90s                             |
| Full 模式 Inspector | 240s×2重试（死代码预算未生效）  | remaining_budget_secs()         |
| Full 模式 Rewrite   | 240s×2重试                      | remaining_budget_secs()         |
| LLM 首字节          | 240s（等满 generation_timeout） | min(240s, 60s) = 60s            |
| LLM 绝对上限        | 无（chunk 刷新可无限）          | generation_timeout × 1.5 = 360s |
| 前端超时            | 330s                            | 200s                            |

### 预期效果

- **最坏情况**：从"无限等待"变为"最多 200s 后快速失败并弹诊断卡片"
- **vllm 半挂**：从"等满 240s 才超时"变为"首字节 60s 超时"
- **准备上下文卡住**：从"无进度无超时"变为"180s 整体兜底"
- **正常使用**：不受影响（正常 vllm 响应 3-10s，远低于超时阈值）

### 验证

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 392/392 通过
- `npx tsc --noEmit` ✅ 零错误
- `NODE_ENV=test npx vitest run` ✅ 126 passed / 3 skipped
- `cargo +nightly fmt` ✅ 通过

## [v0.14.1] - 后台设置即时更新重构：统一状态层与乐观更新（2026-06-17）

### 统一后台设置状态层

- 新增 `SettingsProvider`（`src/contexts/SettingsContext.tsx` + `src/contexts/settingsContextBase.ts`），在 `main.tsx` 全局挂载
- 所有后台设置数据（`settings`、`models`、`agentMappings`）通过 `useSettingsContext()` 统一读取
- 所有写操作集中管理：
  - `updateSettings`：保存通用设置
  - `createModel` / `updateModel` / `deleteModel`：模型配置 CRUD
  - `setActiveModel`：切换当前活跃模型
  - `updateAgentMapping`：更新 Agent 模型映射

### 乐观更新与失败回滚

- 每个 mutation 均实现 `onMutate` 乐观更新、`onError` 自动回滚、`onSettled` 统一缓存失效
- 失败时 UI 自动恢复到修改前状态，并弹出明确 toast 错误提示
- 统一失效范围覆盖 `settings` / `models` / `agent-mappings` / `model-health-reports`，确保跨标签页/跨组件同步

### 组件状态漂移治理

- `GeneralSettings.tsx`：移除本地 `useState/useEffect`，所有输入直接绑定 `settings`；debounce 只包裹提交调用
- `MethodologySettings.tsx`：移除本地 state，选择方法论/步骤即时保存并回显
- `AgentConfig.tsx`：接入 Context 的 `updateAgentMapping`，下拉变更即时生效
- `UnifiedModelManager.tsx` / `ModelModal.tsx`：接入 Context 进行模型增删改查与活跃模型切换
- `ModelList.tsx` / `ModelCard.tsx`：增加删除中状态，防止重复点击

### 底层 hooks 同步增强

- `useSettings.ts` 各 mutation 补齐乐观更新与统一失效，保留给非设置页面/遗留代码使用
- `useStories.ts` 的 `useUpdateStory` 增加乐观更新，并同步刷新 Zustand 中的 `currentStory`
- 新增通用防抖 hook：`src/hooks/useDebounceCallback.ts`

### 代码格式修复

- 运行 `cargo +nightly fmt` 修复 v0.14.0 模型网关相关 Rust 文件的 nightly rustfmt 格式问题，使 CI 的 `Check Rust formatting` 步骤通过

### 验证

- `npx tsc --noEmit` 零错误
- 本次修改文件 `eslint --max-warnings 0` 通过
- `cargo +nightly fmt -- --check` 通过
- `cargo check` 零错误

## [v0.14.0] - 模型网关（Model Gateway）：自动路由、健康探测与多模型状态栏（2026-06-17）

### 新增：模型网关架构

- 新增 `model_gateway` 后端模块，包含：
  - `health`: 模型健康探测与注册表，维护每个启用模型的 TTFB、TPS、成功率
  - `registry`: 网关视角的模型注册表
  - `dispatcher`: 任务分类与动态复杂度评估
  - `executor`: 候选链执行与自动 fallback
  - `scheduler`: 启动时全量探测 + 定时 healthy ping / degraded 重试
  - `commands`: 向前端暴露 `get_gateway_status`、`refresh_model_health` 等命令
- 扩展 `ModelCapability`：新增 `reasoning`、`tool_use`、`structured_output`、`streaming`、`fast`、`image_generation`
- 扩展 `LlmProfile`：新增 `supports_system_prompt`、`supports_streaming`、`knowledge_cutoff`、`reasoning_effort`
- 启动时自动根据 provider/model 名称推断并补齐能力标签

### 智能路由与降级

- `LlmService::generate_for_request_with_request_id` 优先走 `ModelGateway`，主模型失败时自动 fallback 到次优模型
- `AgentService` 无固定映射路径统一走网关，支持 timeout/max_retries 覆盖透传
- `execute_time_sliced` 等直接调用 `generate_for_task` 的路径也已通过网关执行
- 路由评分融合实时健康数据：unhealthy 模型大幅降权，healthy 模型按 TPS/TTFB 微调

### 前端多模型状态栏

- 幕前底部输入栏从「单模型状态」升级为「所有可连接模型状态」
- 通过 `get_gateway_status` 每 30 秒拉取所有 enabled 模型的健康快照
- 状态点颜色区分 healthy / degraded / unhealthy / unknown
- Tooltip 展示每个模型的提供商、TTFB、TPS、主模型/fallback 标记
- 提供刷新按钮可手动重新探测

### 验证

- `cargo check` 零错误
- `cargo test --lib` 392/392 通过
- `npx tsc --noEmit` 零错误

## [v0.13.3] - 诊断卡片安全网：修复「准备上下文」退出未弹诊断卡片（2026-06-17）

### 根因

v0.13.2 已修复多数诊断卡片问题，但用户反馈「输入续写指令后，智能创作在『准备上下文』阶段长时间延时后退出，仍然没有弹出诊断卡片」。

诊断卡片仅在 `catch` 块中触发，而实际运行中存在多条**静默退出路径**：

1. `handleRequestGeneration` / `handleSmartGeneration` 成功返回后，若 `final_content` 为空或 `result.success === false`，仅弹出 toast 就提前 `return`，不会调用 `captureDiagnosticInfo`。
2. 某些状态流转异常（如 `useBackendActivityStore` 订阅回调、`isGenerating` 被异常清空）也可能绕过 `catch` 块。
3. `startElapsedTimer` 在启动时就把 `backendEverRespondedRef` 设为 `true`，导致诊断信息里「是否收到过后端响应」始终显示「是」，干扰判断。

### 修复

#### 1. 全局诊断卡片安全网

- 新增 `smartExecuteNeedDiagnosticRef`：标记本次 `smart_execute` 是否仍需要诊断卡片兜底。
- 新增 `lastGenerationCancelledRef`：区分用户主动取消与异常结束，避免误弹。
- 新增 `useEffect` 监听 `isGenerating` 从 `true` → `false`：只要生成曾启动且非用户取消，就兜底调用 `captureDiagnosticInfo('生成过程异常结束，未收到有效内容')` 弹出诊断卡片。

#### 2. 堵上成功路径中的静默退出

- `handleRequestGeneration`：当后端返回 `final_content` 为空时，立即调用 `captureDiagnosticInfo('AI 返回了空内容（final_content 为空）')`。
- `handleSmartGeneration`：当 `result.success === false` 或 `final_content` 为空时，同样立即调用 `captureDiagnosticInfo`。
- 正常完成有内容时，清除 `smartExecuteNeedDiagnosticRef`，安全网不再兜底。
- 用户主动取消时，设置 `lastGenerationCancelledRef` 并清除诊断需求，避免弹窗。

#### 3. 修正后端响应判定

- `startElapsedTimer` 不再在启动时设置 `backendEverRespondedRef = true`。
- 只有真正收到 `llm-generating-progress` 等后端事件并通过 `updateLastEventTime` 时，才标记为已响应，使诊断信息更准确。

### 验证

- `cargo check` 零错误（仅既有 warning）
- `cargo test --lib` 392/392 通过
- `npx tsc --noEmit` 零错误
- `NODE_ENV=test npx vitest run` 126 passed, 3 skipped（零回归）

## [v0.13.2] - 诊断卡片增强 + 前端自救计时器 + 心跳日志（2026-06-17）

### 诊断卡片改进（根据首次用户反馈）

首次诊断报告揭示了多个问题，本次逐一修复：

- **版本号显示**：注入 `__STORYFORGE_VERSION__` 到窗口对象，诊断卡片不再显示 "unknown"
- **已用时修复**：新增独立 `diagnosticStartTimeRef`，不会被 `stopElapsedTimer` 意外清空
- **后端响应判定修复**：改用 `backendEverRespondedRef` 精确追踪是否收到过后端事件
- **新诊断字段**：增加「作品存在」（显示 currentStory 是否 null）、「后端超时配置」、「模型建议」
- **心跳接入诊断**：`llm-generating-progress` 事件也记录到 `lastProgressEventRef`

### 后端心跳可观测性

- 心跳 `app_handle.emit` 结果改为 `log::warn!` 级别记录（无论日志过滤设置如何都能输出）
- 心跳日志包含已用时秒数和消息内容，帮助确认心跳是否真正在运行

### 前端自救计时器

- `scheduleFallbackPrompt` 从单次 `setTimeout` 改为自我循环的递归调度
- 即使后端心跳完全不送达，前端每 10 秒自动更新状态栏显示 "AI 正在深度思考中...（已用时 XX 秒，距上次响应 YY 秒）"
- 用户不会再看到 269 秒零反馈

### 不再错误假设 Ollama

- 将诊断提示中的 "请检查 Ollama / API 服务状态" 改为通用 "请检查模型服务（vllm/Ollama/OpenAI）"
- 适配使用 vllm + Qwen 等非 Ollama 模型的用户

### 防止 activityStore 提前清空

- 新增 `smartExecuteInFlightRef`，防止 `useBackendActivityStore` 效果在 smart_execute 仍在飞行中时清空 `isGenerating` 和计时器
- 仅在 `smart_execute` 返回后才允许 activityStore 清空状态

### 验证

- `cargo check` 零错误
- `cargo +nightly fmt -- --check` 通过
- `npx tsc --noEmit` 零错误
- `npx prettier --check` 全部通过
- `npm run build` 通过
- `NODE_ENV=test npx vitest run` 126 passed, 3 skipped（零回归）

## [v0.13.1] - 修复智能创作卡死在「准备上下文」阶段（2026-06-15）

### 根因

智能创作（幕前续写 / 智能输入栏）会卡死在「准备上下文」阶段，最终前端 300 秒超时退出。

**完整因果链：**

1. 能力进化反馈环 `CapabilityEvolution::evolve_capability_descriptions`（`capabilities/evolution.rs`）将 LLM 返回内容直接 `trim()` 后保存为能力的 `when_to_use` 描述，**零清洗**。
2. 部分推理模型（qwen / deepseek 等）在正文前输出 `<think>...</think>` 思考链，且常常未闭合即被截断。这段 982 字符的中英混杂思考链被当作 `writer` 的 `when_to_use` 持久化到 `evolved_descriptions.json`。
3. 应用启动时 `load_evolved_descriptions` → `update_when_to_use` **无校验**地赋值，污染进入 `CapabilityRegistry`。
4. 每次智能创作，`to_llm_context()` 把这段污染文本注入 `PlanGenerator` 的 prompt。
5. 计划生成 LLM 被混乱输入干扰，无法输出有效 JSON 计划，卡在「准备上下文 / 生成计划」阶段，前端 300s 超时退出。

**证据：** 应用日志显示启动正常但用户操作期间无任何运行日志；用户机器 `evolved_descriptions.json` 内容确认为未闭合的 `<think>` 思考链。

### 修复（三层防御）

- **写入清洗**（`src-tauri/src/capabilities/evolution.rs`）：新增 `sanitize_evolved_description()`，剥离 `<think>...</think>` 标签（含未闭合情况）、去除 markdown 代码块、300 字符上限、<20 字符拒绝；新增 5 个单元测试覆盖各类污染输入。
- **加载防御**（`src-tauri/src/capabilities/mod.rs`）：`load_evolved_descriptions()` 过滤含 `<think>` 标签或超 300 字符的条目，丢弃并 `warn` 告警，防止历史污染数据再次注入。
- **数据清理**：用户机器 `evolved_descriptions.json` 已重置为 `{}`，立即恢复。

### 验证

- `cargo check` 零错误
- `cargo test --lib` **392/392 通过**（原 387 + 新增 5），零回归

## [v0.13.0] - 分时介入架构：解开「质量与速度不可兼得」的根本矛盾（2026-06-14）

### 核心变更：分时介入架构（三条时间线）

解决 AI 长篇小说创作的根本矛盾——强化专业资产介入导致生成过慢，放松则质量低劣。根因诊断：资产被错误地同步化（B）、写与审被错误耦合（E）。第一性原理：**把大灾难变成即时可见的小债务**。

设计文档：[`docs/plans/2026-06-14-time-sliced-intervention-design.md`](docs/plans/2026-06-14-time-sliced-intervention-design.md)

- **时间线 1（写作时刻，< 15s）**：`GenerationMode::TimeSliced` + `QuickPreflightChecker` + `WriteTimeBundle`（红线突出注入 + 题材自适应）+ 直连 `generate_for_task`，绕过 `execute_writer_raw` 的 Full Preflight 与 auto_contract
  - `src-tauri/src/agents/orchestrator.rs`：新增 `TimeSliced` 模式与 `execute_time_sliced`
  - `src-tauri/src/creative_engine/write_time_bundle.rs`：**新增**最小约束包（合同红线/角色核心/场景大纲/GenreProfile 反模式/可选风格片段）
  - `src-tauri/src/story_system/preflight.rs`：新增 `QuickPreflightChecker`（仅角色非空，不触发 auto_contract）
  - `src-tauri/src/agents/commands.rs` / `executor.rs`：`auto_write` / `auto_revise` / 普通生成迁移到 `TimeSliced`，默认值改为 `TimeSliced`
- **时间线 2（审计时刻，后台 30-90s）**：正文返回后后台 spawn `AuditExecutor` 跑 Inspector 7 维审计，问题以 inline annotation 回流
  - `src-tauri/src/task_system/audit_executor.rs`：**新增**异步审计执行器（Inspector JSON 解析 + memory 优先排序 + annotation 创建 + sync-event 发射）
  - `src-tauri/src/task_system/models.rs`：新增 `TaskType::AsyncAudit`
  - `src-tauri/src/db/models.rs`：`AnnotationType::AiAudit` + `metadata` / `severity` 字段
  - `src-tauri/src/db/repositories.rs`：`create_annotation_with_meta` 方法 + 查询更新
  - `src-tauri/src/db/connection.rs`：legacy migration 90（text_annotations 加列）
  - 前端：`TextAnnotationMark.ts` 支持 `ai_audit` severity 动态着色；`types/v3.ts` / `useSceneAnnotations.ts` / `useTextAnnotations.ts` 类型补全
- **时间线 3（洞察时刻，每 5 段条件触发）**：`InsightExecutor` 汇总追读力趋势 + 追读债务 + annotation 盘点，产出整体健康度报告
  - `src-tauri/src/task_system/insight_executor.rs`：**新增**深度洞察执行器（`should_trigger` 条件判断 + `build_report` 数据汇总 + `run_insight` 直调）
  - `src-tauri/src/agents/orchestrator.rs`：`execute_time_sliced` 末尾条件 spawn InsightExecutor
  - 前端：`pages/NarrativeAnalysis.tsx` 新增「深度洞察」section（健康度仪表盘 + 追读力趋势柱状图 + 债务/标注汇总卡片）

### Phase 0 实测验证（qwen3.6-35b，3 场景 A/B 盲测）

- 最小约束 vs 全量资产平均质量差距 **7.9%**（< 30% 阈值），架构成立
- prompt 长 160% 仅耗时多 7%，证实「慢在同步链路而非 Writer 本身」
- 三条实证改进写入 WriteTimeBundle：红线突出注入、题材自适应、memory 维度优先审计

### 前端体验

- **DebtIndicator（债务指示器）**：幕前顶栏显示未处理 annotation 计数，超阈值（>10 high 或 >30 总计）红色警告，点击跳转幕后
- **首次引导 toast**：首次出现 ai_audit annotation 时提示用户（localStorage 标记，只出现一次）
- `src-frontend/src/frontstage/components/DebtIndicator.tsx`：**新增**
- `src-frontend/src/frontstage/FrontstageApp.tsx`：`onAnnotationCreated` 回调
- `src-frontend/src/frontstage/components/FrontstageHeader.tsx`：嵌入 DebtIndicator

### 测试与文档

- **387 Rust 单元测试全通过**（含 23 个新增测试：WriteTimeBundle 12 + QuickPreflightChecker 3 + AuditExecutor 9 + InsightExecutor 2 + 端到端集成 4 之前的 383→实际加 e2e 后更多）
- TypeScript 类型检查零错误
- `docs/plans/2026-06-14-time-sliced-intervention-design.md`：正解设计文档（Phase 0 已验证）
- `docs/plans/2026-06-14-time-sliced-intervention-implementation.md`：实施计划
- `docs/plans/2026-06-14-asset-tier-creation-design.md` / `-v2.md`：标注已废弃
- `docs/time-sliced-architecture-qa-checklist.md`：**新增** QA 验收清单（7 类 35 项）
- `ARCHITECTURE.md`：新增「⏱️ 分时介入架构 (v0.13.0)」章节

### 已知限制

- annotation 为段落级定位（非字符级精确高亮）——LLM 给字符偏移不可靠
- InsightExecutor 当前不含 KG/向量检索，聚焦追读力+债务+annotation 汇总
- WriteTimeBundle 的 style_slice 暂未接入 StyleDna（`load_sync` 的 `style_slice_override` 传 None）
- 审计的 scene_id 当前传 None（annotation 挂到 story 级），后续接入精确 scene

---

## [v0.12.0] - 智能创作性能全面重构（2026-06-14）

### 修复：点击「写一部小说/续写」后长期在个别进程上无响应、最后无输出

- **本地/局域网模型默认单候选 + 全局并发限流**：`src-tauri/src/agents/orchestrator.rs` / `src-tauri/src/llm/service.rs`
  - 本地/局域网模型固定 1 候选；远程模型默认可配置 1–2 候选
  - 增加 `tokio::sync::Semaphore` 限制全局并发 Writer 数量：本地 1、远程 2
  - 候选总超时硬上限从 270s 收紧到 90s
- **LLM 调用层超时与取消加固**：`src-tauri/src/llm/service.rs` / `src-tauri/src/llm/adapter.rs` / `src-tauri/src/error.rs`
  - 拆分连接超时与生成超时；连接超时可重试 1 次，生成超时不再重试
  - 增加 `connect_timeout` 并覆盖响应体读取阶段
  - 心跳任务 `AbortOnDrop` 保险，确保任何错误路径都中止
- **写作上下文准备 spawn_blocking 化**：`src-tauri/src/agents/service.rs` / `src-tauri/src/creative_engine/context_builder.rs`
  - 将同步 DB 查询块整体包裹 `tokio::task::spawn_blocking`，避免阻塞 tokio worker
- **提示词构建减少重复 IO**：`src-tauri/src/agents/service.rs`
  - `AppConfig` / `GenreProfile` / `StyleDNA` 进程级只读缓存，避免每次从磁盘加载
- **SQLite 高频路径 spawn_blocking 化**：`src-tauri/src/scene_commands.rs` / `src-tauri/src/creation_commands.rs`
- **全局 Mutex 替换**：`src-tauri/src/lib.rs` / `src-tauri/src/llm/service.rs` / `src-tauri/src/creative_engine/context_builder.rs`
  - `DB_POOL` / `LLM_SERVICE` 改用 `OnceLock`/`OnceCell<Arc>`；`ContextCache` 改用 `tokio::sync::RwLock`

### 优化：前端响应与大数据量场景

- **前端生成状态可取消与反馈**：`src-frontend/src/frontstage/FrontstageApp.tsx` / `src-frontend/src/hooks/useBackendActivityListener.ts`
  - 显示精确阶段（准备上下文 / 候选生成 / Inspector / 改写 / 最终输出 / 保存记忆）与已用时间
  - 取消按钮可靠调用 `agent_cancel_all_tasks` 并清理状态
- **前端输入路径减负**：`src-frontend/src/frontstage/FrontstageApp.tsx` / `src-frontend/src/frontstage/autoSave.ts`
  - 字数统计移入 `requestIdleCallback`；IPC 节流 350ms；autoSave payload getter 化
- **关闭/替换高频心跳**：`src-frontend/src/frontstage/FrontstageApp.tsx` / `src-frontend/src/frontstage/components/FrontstageBottomBar.tsx`
  - 移除 1s setInterval 心跳，改用 CSS 动画；16ms 打字机改为 `requestAnimationFrame`
- **字数统计增量化**：`src-frontend/src/frontstage/FrontstageApp.tsx` / `src-frontend/src/hooks/useExecutionState.ts`
  - 当前章节字数增量 diff 更新 + 后端 SQL 聚合
- **场景/章节数据分页与延迟加载**：`src-tauri/src/db/repositories.rs` / `src-frontend/src/hooks/useScenes.ts` / `useChapters.ts`
  - 新增 `get_by_story_paged`，列表不返回 `content` 大字段
- **合并 sync-event 失效**：`src-frontend/src/hooks/useSyncStore.ts`
  - 9 次独立 `invalidateQueries` 合并为一次 predicate 批量刷新
- **LanceDB 查询优化**：`src-tauri/src/vector/lancedb_store.rs`
  - 参数化 filter；前缀 LIKE；`hybrid_search` RRF 移入 `spawn_blocking`
- **Embedding 批处理**：`src-tauri/src/embeddings/provider.rs`
  - Ollama/OpenAI 按 batch（32/100）请求
- **前端状态拆分**：`src-frontend/src/stores/generationStore.ts` / `src-frontend/src/stores/bootstrapStore.ts`
- **文思分析异步化**：`src-frontend/src/frontstage/ai-perception/textAnalyzer.worker.ts` / `asyncTextAnalyzer.ts` / `SmartHintSystem.tsx`
  - 移入 Web Worker，支持 AbortSignal 取消
- **RichTextEditor HTML 序列化节流**：`src-frontend/src/frontstage/components/RichTextEditor.tsx`
  - `getText()` 轻量更新 + 200ms 防抖 `getHTML()`

### 架构级优化

- **统一生成状态事件**：`src-tauri/src/events.rs`
  - 新增 `generation-status` 事件，前端统一消费，减少重复状态更新
- **知识图谱虚拟化/LOD**：`src-frontend/src/components/KnowledgeGraph/KnowledgeGraphView.tsx`
  - `onlyRenderVisibleElements`、viewport 裁剪、节点 >200 时 LOD 分层
- **Agent 编排可观测性**：`src-tauri/src/agents/orchestrator.rs` / `src-tauri/src/agents/service.rs` / `src-tauri/src/llm/service.rs`
  - 结构化 trace：上下文准备、候选、Inspector、改写、LLM TTFT、DB 耗时
- **后台任务队列与背压**：`src-tauri/src/memory/writer.rs` / `src-tauri/src/memory/ingest.rs` / `src-tauri/src/agents/orchestrator.rs`
  - `MemoryWriter + IngestPipeline` 全局 Semaphore（默认 2）、取消令牌注册表、取消传播
- **真实 tokenizer 与上下文预算**：`src-tauri/src/memory/tokenizer.rs` / `src-tauri/src/memory/query.rs` / `src-tauri/src/creative_engine/context_builder.rs`
  - 引入 `tiktoken-rs`；`record_llm_call` 与 `budget_control` 改用真实 token 计数
  - `ContextBudget` 按系统提示 / 故事设定 / 场景 / 用户输入分配预算，优先截断较早 scene

### 质量门禁

- `cargo check --all-targets` ✅
- `cargo test --all-targets` ✅ 357 passed
- `npx tsc --noEmit` ✅
- `npm run test:run` ✅ 126 passed, 3 skipped
- `npx playwright test e2e/performance/stage3-performance.spec.ts` ✅ 3 passed

---

## [v0.11.7] - 紧急修复：候选仍串行 + 输入框未打字即自动进入运行进程（2026-06-13）

### 修复：候选生成阶段仍显示“生成候选 1 / 2（本地模型串行）”并挂起 500s

- **强制候选阶段并行，彻底忽略旧配置的串行标志**：`src-tauri/src/agents/orchestrator.rs`
  - 旧配置持久化可能保存了 `candidate_local_sequential=true`，导致 v0.11.5/v0.11.6 的“默认并行”被覆盖
  - v0.11.7 直接移除串行分支，代码层强制并行；无论配置如何，候选 1 都不会阻塞候选 2
- **候选单个超时增加硬上限**：`src-tauri/src/agents/orchestrator.rs`
  - 本地候选：最多 60s；远程候选：最多 120s
  - 若用户旧配置保存了 600s 等超大值，直接按硬上限截断，避免失败候选挂死 500s+
- **候选阶段仍不重试、失败即跳过**：与 v0.11.5 一致，单个候选超时/失败不影响其他候选

### 修复：输入框还没打字就自动进入运行进程 / 输入框被禁用

- **去掉 `get_input_hint` 的 LLM 调用**：`src-tauri/src/commands/orchestrator.rs`
  - 输入框聚焦、上下箭头切换 ghost hint 时会自动调用 `get_input_hint`
  - 原实现会调用 LLM 生成建议，产生 `agent-stage-update` 事件并被聚合为后台主活动，导致输入框被禁用
  - v0.11.7 仅返回规则驱动的候选建议（零后端 LLM 调用），既保留 ghost hint 又不再锁死输入框
- **保留 v0.11.6 的启动修复**：`spawn_background_tasks` 空实现、`init_workflow_engine` 不再自动恢复并执行上次未完成的 workflow 实例

## [v0.11.6] - 紧急修复：启动时 capability_evolution 后台挂起 + 版本号遗漏（2026-06-13）

### 修复：应用启动后未输入任何指令就进入后台进程并卡顿 500s

- **禁用启动时自动能力进化**：`src-tauri/src/lib.rs`
  - `spawn_background_tasks` 原会在启动 30s 后无条件调用 LLM 分析所有能力执行记录，若模型未启用或响应慢，会导致应用在用户未输入任何指令时卡住 500s 以上
  - v0.11.6 默认禁用该自动任务，改为空实现并记录日志
- **禁用计划执行后自动能力进化**：`src-tauri/src/planner/executor.rs`
  - 每次 `smart_execute` / 计划执行完成后也会异步触发能力进化，v0.11.6 移除该自动触发
- **禁用每 5 条记录自动触发能力进化**：`src-tauri/src/capabilities/evolution.rs`
  - `record_execution` 不再每累计 5 条记录就发起后台 LLM 调用
- **能力进化手动触发增加 60s 超时保护**：`src-tauri/src/capabilities/evolution.rs`
  - 保留 `evolve_capabilities` 手动命令，但单次 LLM 分析最多等待 60s，超时则跳过该能力

### 修复：构建产物版本号仍显示 0.11.3

- **统一版本号为 0.11.6**：
  - `src-tauri/Cargo.toml`
  - `src-tauri/tauri.conf.json`（Tauri 构建产物版本来源）
  - `src-frontend/package.json`
  - `README.md` 版本徽章与最新动态

## [v0.11.5] - 智能创作候选阶段卡顿与进度显示修复（2026-06-12）

### 修复：候选生成阶段长时间卡顿 / 500s 无进展

- **本地候选默认并行、不再串行阻塞**：`src-tauri/src/agents/orchestrator.rs`
  - 移除本地模型候选串行策略默认 `true`，改为默认并行
  - 避免候选 1 挂起时阻塞候选 2，导致用户看到“生成候选 1/2”长时间不动
- **候选阶段单次失败不再重试**：`src-tauri/src/agents/orchestrator.rs`
  - 候选阶段单个候选超时/失败立即跳过，进入下一个候选或降级单轮生成
  - 消除“120s 超时 × 2 次 × 多个候选”叠加到 500s 的问题
- **区分本地/远程候选超时**：`src-tauri/src/agents/orchestrator.rs` / `src-tauri/src/config/settings.rs`
  - 本地模型：60s/候选；远程模型：120s/候选
  - 总超时 = 单个超时 × 候选数 + 30s，最低 180s，异常时更快失败
- **reqwest 客户端超时与 profile 配置一致**：`src-tauri/src/llm/service.rs` / `src-tauri/src/llm/{ollama,openai,anthropic}.rs`
  - 移除硬编码 600s，改为使用 `LlmProfile.timeout_seconds`
  - 缓存 key 增加 timeout，避免配置变更后复用旧 adapter
- **JSON 反序列化隔离到 blocking 线程池**：`src-tauri/src/llm/{ollama,openai,anthropic}.rs`
  - `response.json()` 改为先 `response.bytes().await` 再 `spawn_blocking` 反序列化
  - 避免大响应同步解析阻塞 tokio worker

### 修复：前端进度显示 / 取消体验

- **进度条改为不确定动画**：`src-frontend/src/hooks/useBackendActivityListener.ts` / `src-frontend/src/frontstage/components/FrontstageBottomBar.tsx`
  - Orchestrator / LLM 心跳等无具体百分比阶段不再硬编码 0.3，显示滚动 indeterminate 动画
- **心跳持续发送不再在 600s 停止**：`src-tauri/src/llm/service.rs`
  - 移除 `tick_count >= 60` 退出，生成多久就心跳多久
- **取消按钮真正通知后端**：`src-frontend/src/frontstage/FrontstageApp.tsx` / `src-tauri/src/agents/commands.rs`
  - 新增 `agent_cancel_all_tasks` 命令并注册到 handlers
  - 前端取消时调用后端取消所有 Agent 任务，而不只是清理本地状态
- **超时/取消后清理 backendActivityStore**：`src-frontend/src/frontstage/FrontstageApp.tsx`
  - 前端 300s 超时、用户取消、生成失败均调用 `failAllRunning`
  - 避免状态栏在任务结束后仍显示“系统正在处理中”
- **前端超时从 600s 缩短到 300s**：`src-frontend/src/frontstage/FrontstageApp.tsx`
  - 与后端总超时（本地约 150s / 远程约 270s）保持合理余量

## [v0.11.4] - 智能创作超时根因根治（2026-06-12）

### 修复：任何指令都陷入"系统正在处理中..."超时

- **活跃模型只返回 enabled 模型**：`src-tauri/src/config/settings.rs`
  - `get_active_llm_profile` 过滤 `enabled=false` 的占位模型
  - 默认 disabled 占位模型不再被自动选中，避免向 `localhost:11434` 空等 300s
- **生成入口强制校验 enabled**：`src-tauri/src/llm/service.rs`
  - `execute_generation` / `generate_stream` / `generate_with_request_id` / `get_profile_by_id` 统一拒绝 disabled 模型
  - 立即返回 `VALIDATION_FAILED` 并提示用户启用或切换模型
- **配置变更后自动刷新 LLM 服务**：`src-tauri/src/config/commands.rs`
  - `create_model` / `update_model` / `delete_model` 保存后调用 `LlmService::reload_config()`
  - 删除/禁用模型后立即生效，不再依赖重启

### 修复：后端准备阶段阻塞 tokio worker

- **全局缓存 `AppConfig`**：`src-tauri/src/config/settings.rs`
  - 避免每次命令都新建 `max_size=1` 的 SQLite pool
  - 5 秒 TTL，save() 主动刷新，显著降低 SQLite 锁竞争
- **同步 DB 操作隔离到 `spawn_blocking`**：
  - `src-tauri/src/agents/service.rs::build_writer_prompt`
  - `src-tauri/src/agents/context_optimizer.rs`（L0/L1/L2/完整上下文）
  - `src-tauri/src/creative_engine/context_builder.rs`
  - `src-tauri/src/creative_engine/adaptive/generator.rs`
  - `src-tauri/src/memory/orchestrator.rs`
- **关键准备阶段加整体 60s 超时**：
  - `src-tauri/src/agents/service.rs::prepare_writer_context`
  - `src-tauri/src/planner/mod.rs::generate_plan`
  - 卡住时快速失败，不再让前端显示"系统正在处理中..."

### 修复：超时错误被包装成普通内部错误

- **透传 `LlmTimeout`**：`src-tauri/src/planner/executor.rs`、`src-tauri/src/commands/orchestrator.rs`
  - `PlanExecutionResult` 新增 `error: Option<AppError>` 字段
  - 底层 `LLM_TIMEOUT` 直接返回前端，可触发"检查模型"恢复动作
- **GenesisPipeline 错误正确转换**：`src-tauri/src/commands/orchestrator.rs`
  - `PipelineError::LlmError(timeout)` 映射为 `AppError::LlmTimeout`
  - `PipelineError::Cancelled` 映射为 `AppError::Cancellation`

### 修复：取消后状态栏仍显示"系统正在处理中"

- **orchestrator 活动生命周期**：`src-tauri/src/agents/orchestrator.rs`
  - 编排结束/失败时 emit `orchestrator-step` with `status: completed/failed`
- **前端正确结束活动**：`src-frontend/src/hooks/useBackendActivityListener.ts`
  - 收到 completed/failed 时结束 orchestrator 后台活动
- **取消按钮清理残留活动**：`src-frontend/src/frontstage/FrontstageApp.tsx`
  - `handleCancelGeneration` 调用 `backendActivityStore.failAllRunning('用户已取消')`
  - 补齐 `handleRequestGeneration` 的 `cancelGenerationRef` 设置
- **新增 `failAllRunning` API**：`src-frontend/src/stores/backendActivityStore.ts`

### 体验优化

- **流式首 chunk 超时动态化**：`src-tauri/src/llm/service.rs::generate_stream`
  - 按 `profile.timeout_seconds` 计算，范围 30–120s
  - 本地模型冷启动不再被硬编码 30s 误杀
- **关键路径结构化日志**：`src-tauri/src/llm/service.rs`、`src-tauri/src/agents/service.rs`
  - `execute_generation` 入口记录 model/provider/timeout/max_retries
  - `generate_for_agent_with_options` 记录 agent/story_id/prompt_len/max_tokens
  - `prepare_writer_context` 记录入口/出口耗时

### 验证

- `cargo check --lib` ✅
- `cargo check --tests` ✅
- `cargo test --lib` ✅ 333 passed, 0 failed
- `npm run type-check` ✅

---

## [v0.11.3] - 模型状态光晕与设为当前模型同步（2026-06-13）

### 新增：底部模型状态绿点心跳光晕

- **CSS 心跳动画**：`src-frontend/src/frontstage/styles/frontstage.css`
  - 为 `.model-status-dot.status-connected` 新增 `heartbeat-glow` 关键帧动画
  - 绿色圆点在正常连接时会有两次轻闪 + 光晕扩散的呼吸效果

### 修复：模型列表「设为当前」不立即生效

- **前端扩大失效范围**：`src-frontend/src/hooks/useSettings.ts`
  - `useSetActiveModel` 成功时同时失效 `['settings']` 和 `['models']`
- **前端跨窗口同步**：`src-frontend/src/hooks/useSyncStore.ts`
  - `dataRefresh` 分支新增 `model_config`，刷新 settings/models
- **幕前处理 DataRefresh**：`src-frontend/src/frontstage/FrontstageApp.tsx`
  - 收到 `DataRefresh { entity: "model_config" }` 时立即失效 settings/models query
- **后端热重载 LLM 配置**：`src-tauri/src/config/commands.rs`
  - `set_active_model` 保存后调用 `LlmService::reload_config()`，清除适配器缓存
- **后端广播模型变更**：`src-tauri/src/config/commands.rs`
  - `set_active_model` 同时发送 `FrontstageEvent::DataRefresh` 和 `SyncEvent::DataRefresh`
- **修复 multimodal 活跃模型返回**：`src-tauri/src/config/commands.rs`
  - `get_settings` 中 `multimodal` 从 `active_llm_profile` 读取，不再硬编码为空

### 验证

- `cargo check --all-features` ✅
- `npm run type-check` ✅
- `npm run test:run` ✅ 116 passed / 3 skipped

---

## [v0.11.2] - 修复 AI 续写超时无反馈与模型删除不生效（2026-06-13）

### 修复：AI 续写长时间无反馈 / 500s+ 无输出

- **后端候选阶段增加整体超时**：`src-tauri/src/agents/orchestrator.rs`
  - `generate_candidates` 外部包装 `tokio::time::timeout`
  - 总超时 = `candidate_timeout_seconds × 候选数 + 60s`，不低于 240s
  - 超过总超时立即返回 `LLM_TIMEOUT`，不再进入 fallback 单轮 300s 链路
- **超时错误不再重试**：`src-tauri/src/llm/service.rs`
  - `is_retriable_error` 中 `AppError::LlmTimeout { .. } => false`
  - 避免「120s 单次超时 × 1 次重试 × 2 候选 × fallback」叠加到 500s 以上
- **更细粒度的后台进度事件**：`src-tauri/src/agents/orchestrator.rs`
  - 候选上下文准备完成
  - 每个候选开始 / 完成 / 失败
  - 候选质量评估阶段
  - 失败降级阶段

### 修复：前端状态提示重复与进度不透明

- **修复「系统仍在处理中...」重复追加**：`src-frontend/src/frontstage/FrontstageApp.tsx`
  - 新增 `cleanStatusBase` 统一清理时间后缀与兜底提示
  - `elapsedTimerRef` / `fallbackTimerRef` 都基于清理后的 base 重建文案
- **保留具体进度，避免 LLM 心跳覆盖**：`src-frontend/src/frontstage/FrontstageApp.tsx`
  - `updateGenerationPhase`、`llm-generating-progress`、`agent-stage-update` 处理中，
    若当前状态包含「候选 / 第 N 轮 / 评分 / 匹配度 / 降级 / 失败 / 准备中」等具体信息，
    不再被通用大阶段文案覆盖

### 修复：模型删除不生效

- **后端无条件持久化并广播刷新**：`src-tauri/src/config/commands.rs`
  - `delete_model` 删除成功后无条件调用 `config.save`
  - 删除后向 frontstage 发送 `DataRefresh { entity: "model_config" }`
- **前端同时失效相关 queryKey**：`src-frontend/src/hooks/useSettings.ts`
  - `useDeleteModel` 成功时同时失效 `models`、`settings`、`agent-mappings`

### 验证

- `cargo check --all-features` ✅
- `cargo test --all-features` ✅ 332 passed
- `npm run type-check` ✅
- `npm run test:run` ✅ 116 passed / 3 skipped

---

## [v0.11.1] - 候选生成性能优化与 UI 提示统一（2026-06-13）

### 性能优化

- **候选生成链路重构**：`src-tauri/src/agents/orchestrator.rs` / `service.rs`
  - 新增 `WriterPreparedContext`、`prepare_writer_context` 与 `execute_writer_prepared`
  - 将候选阶段从「每个候选走完整 Writer 链路」改为「一次预准备 + 多次轻量采样」
  - 预检、自动补齐、prompt 构建、生成策略计算只执行一次，两个候选共享结果
  - 候选阶段使用专用短超时（默认 120s）与少重试（默认 1 次）
  - 本地模型自动串行执行，避免在服务端排队导致双超时
  - 候选全部失败时自动降级为单轮完整生成，不中断用户体验
- **LLM 调用链支持超时/重试覆盖**：`src-tauri/src/llm/service.rs`
  - `execute_generation` 新增 `timeout_seconds_override` / `max_retries_override`
  - 所有同步生成入口保留向后兼容
- **候选参数配置化**：`src-tauri/src/config/settings.rs` / `agents/orchestrator.rs`
  - `AppConfig` / `WorkflowConfig` 新增 `candidate_timeout_seconds`、`candidate_max_retries`、`candidate_local_sequential`

### UI 统一

- **彻底移除幕前黑色 toast 提示**：`src-frontend/src/frontstage/`
  - 移除 `frontstage/main.tsx` 中的 `react-hot-toast` `Toaster`
  - `FrontstageApp.tsx` 内建统一 `toast` 桥接，将原有 `toast()` / `toast.success()` / `toast.error()` / `toast.loading()` 全部转发到顶部状态栏
  - `UpgradePanel.tsx` / `RichTextEditor.tsx` / `WenSiPanel.tsx` 中的 toast 全部替换为统一状态提示或面板内状态
  - 后台 AI 流程通知继续显示在底部 AI 编排器状态栏
  - 创作进程/结果提示显示在顶部字数/字号后的编排器状态区

### 验证

- `cargo check --all-features` ✅
- `cargo test --all-features` ✅ 332 passed
- `npm run type-check` ✅
- `npm run test:run` ✅ 116 passed / 3 skipped

---

## [v0.10.2] - 修复模型连接硬编码、增加删除模型与 Agent 映射对齐（2026-06-13）

### 修复

- **修复状态栏显示已删除模型ID**：`agents/service.rs` 中「使用指定模型 … 生成…」现在显示模型**名称**而非原始ID；若Agent映射指向已删除模型，自动fallback到默认模型，不再显示不存在的硬编码ID
- **删除模型时自动清理Agent映射**：后端 `delete_model` 删除LLM/Embedding配置后，会清除所有Agent映射（writer/inspector/outline_planner/style_mimic/plot_analyzer）中对该模型的引用，并重新设置活跃模型为剩余配置中的第一个
- **Agent模型选择增加有效性校验**：`get_agent_model_id` 校验映射的模型ID是否仍存在于 `llm_profiles` / `embedding_profiles`，引用已删除模型时返回None并fallback到默认模型

### 新增

- **模型管理支持删除模型**：在「设置 → 模型管理」每个模型卡片新增删除按钮，可删除错误/不再使用的模型配置；删除前弹窗确认，并提示会自动清理Agent引用

### 涉及文件

- 后端：`src-tauri/src/config/commands.rs`、`src-tauri/src/agents/service.rs`
- 前端：`src/frontstage/components/FrontstageBottomBar.tsx`（无直接改动，但受益于后端名称显示）、`src/pages/settings/ModelCard.tsx`、`src/pages/settings/ModelList.tsx`、`src/pages/settings/UnifiedModelManager.tsx`、`src/hooks/useSettings.ts`

### 验证

- `cargo test --lib` ✅ 332 passed
- `cargo check --lib` ✅ 无警告
- `npm run type-check` ✅
- `npm run test:run` ✅ 116 passed / 3 skipped
- `cargo +nightly fmt` ✅ / `prettier --write` ✅

---

## [v0.10.1] - 生成状态提示统一到底部 AI 编排器状态栏（2026-06-13）

### 改进

- **取消顶部黑色 loading toast**：幕前创作页的「正在生成续写内容…」等阶段性提示不再以悬浮黑色 toast 形式出现，避免与应用整体 parchment/暖沙风格冲突
- **统一状态展示位置**：所有生成阶段文案统一合并到底部 AI 编排器状态栏（`FrontstageBottomBar`）中，与输入区、模型状态、后台任务保持同一视觉层级
- **美化状态栏显示**：
  - 状态文本拆分为基础文案、后缀说明、运行时长三段，避免时间截断主文案
  - 本地生成时显示不确定进度动画；后台活动有具体进度时显示真实进度条
  - 后台任务显示类别图标与标签（编排 / 续写 / Agent / 流水线等）
  - 类别标签改为 pill 形状，与整体圆角、边框风格一致
- **修复 elapsed timer 文案截断 bug**：`prev.split(' (')[0]` 替代原来错误的 `' ('.split('')[0]`，保证运行时长只追加在时间部分

### 前端变更

- `src/frontstage/FrontstageApp.tsx`
  - `updateToastPhase` 重命名为 `updateGenerationPhase`，不再调用 `toast.loading`，改为更新 `generationStatus`
  - 创建小说 / 续写入口不再创建顶部 loading toast
  - 修复 `startElapsedTimer` 中状态前缀分割逻辑
- `src/frontstage/components/FrontstageBottomBar.tsx`
  - 新增状态文本解析（基础 / 时长 / 后缀）
  - 本地生成无后台活动时显示不确定进度条
  - 后台活动显示类别图标与 pill 标签
- `src/frontstage/styles/frontstage.css`
  - 新增 `.generation-status-base` / `.generation-status-suffix` / `.generation-status-elapsed` / `.generation-status-category-icon`
  - 新增 indeterminate 进度动画
  - 优化类别标签为圆角 pill 样式

### 验证

- `npm run type-check` ✅
- `npm run test:run` ✅ 116 passed / 3 skipped
- `cargo check --lib` ✅
- `cargo +nightly fmt` ✅
- `prettier --write` ✅

---

## [v0.10.0] - 能力发现与自动编排（2026-06-12）

### 摘要

- 实现「给模型完整的技能表，供其自动选取」：技能、方法论、43 个网文体裁画像、Style DNA 统一注册为可发现资产
- 模型在小说世界生成、章节规划、正文写作各阶段自动选择并应用创作策略
- GenesisPipeline 概念生成后自动选择体裁画像、方法论、风格 DNA 并持久化到故事
- Planner 与 Writer prompt 自动注入已选策略上下文，43 个网文模板真正参与智能创作
- 前端创建向导新增 AI 推荐策略确认步骤，故事卡片与编辑表单支持查看和人工覆盖策略

### 后端改进

- **统一资产目录与能力注册表**：`src-tauri/src/strategy/`
  - 新增 `SelectableAsset` / `AssetKind` / `SelectedStrategy` / `SelectionContext` 统一抽象
  - 技能、方法论、体裁画像、Style DNA、Workflow 全部转换为 `SelectableAsset`
  - 应用启动时通过 `CapabilityRegistry::register_selectable_assets` 完成全局注册
- **策略选择器**：`src-tauri/src/strategy/selector.rs` / `src-tauri/src/commands/strategy.rs`
  - `StrategySelector` 支持精确匹配 + LLM 选择双模式
  - 暴露 `select_creation_strategy` Tauri 命令，前端可预览模型推荐策略
- **GenesisPipeline 接入策略选择**：`src-tauri/src/narrative/genesis.rs`
  - 概念生成后增加 `StrategySelectionStep`，自动调用 `StrategySelector`
  - 选择结果写入 `story.genre_profile_id` / `story.methodology_id` / `story.style_dna_id`
  - 第一章 prompt 注入体裁画像完整内容（含 typical_structure）与方法论说明
- **Planner / Writer 注入策略上下文**：`src-tauri/src/planner/mod.rs` / `src-tauri/src/agents/mod.rs` / `src-tauri/src/commands/orchestrator.rs`
  - `PlanContext` 新增 `selected_strategy`
  - `StoryContext` 新增 `genre_profile_id`
  - `build_writer_prompt` 注入体裁画像策略与方法论参数
- **数据层补齐**：`src-tauri/src/db/connection.rs` / `src-tauri/src/db/models.rs` / `src-tauri/src/db/dto.rs` / `src-tauri/src/db/repositories.rs`
  - `genre_profiles` 表新增 `typical_structure_json`
  - `stories` 表新增 `genre_profile_id`
  - `CreateStoryRequest` / `UpdateStoryRequest` / `Story` 模型扩展对应字段
- **旧向导兼容**：`src-tauri/src/creation_commands.rs` / `src-tauri/src/commands/story.rs`
  - `create_story_with_wizard` 与 `create_story` 均接受 `style_dna_id` / `genre_profile_id` / `methodology_id`

### 前端改进

- **策略选择 API**：`src-frontend/src/services/api/genesis.ts` / `src-frontend/src/types/index.ts`
  - 新增 `selectCreationStrategy` 与 `SelectedStrategy` / `StrategySelectionRequest` 类型
- **创建向导策略确认**：`src-frontend/src/components/NovelCreationWizard.tsx`
  - 用户输入后新增「AI 推荐创作策略」步骤，展示推荐理由、体裁画像、方法论、风格 DNA、推荐技能
  - 确认后进入世界观生成，选择结果随 `create_story_with_wizard` 保存
- **故事策略展示与人工覆盖**：`src-frontend/src/pages/Stories.tsx` / `src-frontend/src/types/index.ts`
  - `Story` / `CreateStoryRequest` 类型扩展策略字段
  - 故事卡片显示体裁/方法/风格标签
  - 编辑表单增加体裁画像 ID、风格 DNA ID 输入框，支持人工修改
- **体裁表单修复**：`src-frontend/src/pages/StorySystem.tsx` / `src-frontend/src/services/api/quality.ts` / `src-frontend/src/types/api.ts`
  - 统一 `GenreProfile` 类型定义，修复 `typical_structure_json` 字段缺失导致的类型错误

### 测试

- `cargo test --lib` ✅ 332/332 通过
- `cargo check --lib` ✅ 无警告
- `npm run type-check`（src-frontend）✅
- `npm run test:run` ✅ 116 passed / 3 skipped

---

## [v0.9.7] - 技能与设置参数对智能创作真正生效（2026-06-13）

### 摘要

- 针对用户反馈的「项目丰富的技能与后台参数设定没有真正影响小说内容生成」问题进行全面修复
- 将前端设置、后端配置、创作流水线（Writer / Inspector / Planner / Genesis）重新连接，确保每项参数都能调节模型创作行为
- 新增 Agent 模型映射 UI，可为每个 Agent 单独指定聊天/嵌入/多模态模型
- 技能（Skills）参数默认值、config、温度/token 现在真正生效
- Genesis 向导与创作工作流读取用户写作策略、模型配置、质检阈值
- 模型高级参数（top_p / frequency_penalty / presence_penalty）可持久化并传入 LLM 适配器
- 通用设置（主题/语言/自动保存/字号/行高）与隐私设置真正持久化
- 写作风格详细字段、作品简介、场景氛围等上下文注入 Writer 提示词

### 后端改进

- **统一 WorkflowConfig 从 AppConfig 读取**：`src-tauri/src/agents/orchestrator.rs` / `src-tauri/src/config/settings.rs`
  - `WorkflowConfig` 新增 `from_app_config`，读取 `rewrite_threshold` / `max_feedback_loops` / `style_weight` / `narrative_weight` / `skip_rewrite_threshold` / `keep_revision_history`
  - 所有创作路径（`writer_agent_execute`、`auto_write`、`auto_revise`、任务系统执行器、Planner、Genesis、工作流调度器、场景草稿生成）统一使用用户配置，不再写死
- **Agent 模型映射真正可用**：`src-tauri/src/agents/service.rs`
  - 新增 `get_agent_llm_params`：按 Agent 类型读取映射的 chat 模型 profile 的 `temperature` / `max_tokens`
  - Writer / Inspector / OutlinePlanner / StyleMimic / PlotAnalyzer / Commentator / MemoryCompressor / KnowledgeDistiller 均优先使用用户指定模型参数
- **技能系统完整生效**：`src-tauri/src/skills/executor.rs` / `src-tauri/src/skills/builtin.rs` / `src-tauri/src/commands/skill.rs`
  - `SkillParameter.default` 缺失时自动合并到调用参数
  - `SkillManifest.config` 作为低优先级参数合并，支持 `temperature` / `max_tokens`
  - 内置技能补充默认 config
  - fallback LLM 调用也使用 skill 返回的参数
- **Genesis / 创作向导读取配置**：`src-tauri/src/narrative/genesis.rs` / `src-tauri/src/creative_engine/workflow/engine.rs`
  - 概念生成读取 active LLM profile 的 `temperature` / `max_tokens`
  - 第一章注入用户 `writing_strategy` 与可配置目标字数（`genesis_first_chapter_word_count_target`，默认 2000）
  - 创作工作流 `review_threshold` / `max_iterations` 从 `AppConfig` 读取
- **模型高级参数持久化与传递**：`src-tauri/src/config/settings.rs` / `src-tauri/src/config/commands.rs` / `src-tauri/src/llm/adapter.rs` / `src-tauri/src/llm/service.rs` / `src-tauri/src/llm/openai.rs` / `src-tauri/src/llm/anthropic.rs` / `src-tauri/src/llm/ollama.rs`
  - `LlmProfile` 新增 `top_p` / `frequency_penalty` / `presence_penalty`
  - `ModelConfigInput` 接收并保存上述字段
  - `GenerateRequest` 传递这些字段到 OpenAI / Anthropic / Ollama 适配器
- **通用/隐私设置持久化**：`src-tauri/src/config/settings.rs` / `src-tauri/src/config/commands.rs`
  - `AppConfig` 新增 `theme` / `language` / `auto_save` / `auto_save_interval` / `font_size` / `line_height` / `share_usage_data` / `store_api_keys_securely`
  - `get_settings` / `save_settings` 真正读写这些字段，不再硬编码
- **风格与场景参数补全注入**：`src-tauri/src/agents/mod.rs` / `src-tauri/src/creative_engine/context_builder.rs` / `src-tauri/src/agents/service.rs`
  - `StoryContext` 新增 `description`，`StyleContext` 新增写作风格详细字段
  - `build_writer_prompt` 注入【作品简介】与【写作风格约束】
  - `format_scene_structure` 显式渲染 `setting_atmosphere`

### 前端改进

- **Agent 模型映射 UI**：`src-frontend/src/pages/settings/AgentConfig.tsx` / `src-frontend/src/pages/Settings.tsx`
  - 替换原占位组件，支持为 8 个 Agent 配置 chat / embedding / multimodal 模型
  - 实时保存，调用已有 `update_agent_mapping` API
- **模型高级参数输入**：`src-frontend/src/pages/settings/ModelModal.tsx`
  - 新增 Top P、Frequency Penalty、Presence Penalty 输入框

### 测试

- `cargo test --lib` ✅ 323/323 通过
- `npm run type-check`（src-frontend）✅
- `npm run test:run` ✅ 116 passed / 3 skipped

---

## [v0.9.6] - 智能创作性能与质量双重优化（2026-06-12）

### 摘要

- 针对用户反馈的「智能创作流程速度慢、等待时间长、甚至超出模型连接限时」问题进行全面性能优化
- 针对「人物单一、场景简单、情节单调、命名雷同（林/陈单字名）」等质量问题进行系统性创作质量提升
- 在不牺牲创作质量理解准确性的前提下，从统一超时、服务缓存、上下文缓存、异步化、后台化、协作取消六个方向改善响应能力
- GenesisPipeline 故事创建后第一章改为后台生成，用户可立即进入工作台
- 内置增强技能（情感节奏、文风润色）现在会在 Writer 输出后自动调用
- 已撰写完整性能优化报告：`PERFORMANCE_OPTIMIZATION_REPORT.md`

### 后端改进

- **统一并延长 LLM 超时**：`src-tauri/src/llm/service.rs`
  - 默认超时从 120s 提升到 300s，减少大模型长输出超时失败
  - 所有调用路径共享 `effective_timeout_seconds`，超时逻辑一致
- **`LlmService` 全局单例**：`src-tauri/src/llm/service.rs` / `src-tauri/src/lib.rs`
  - 通过 `init_llm_service` 将服务作为 Tauri State 注册，避免每次请求重建服务
  - 减少 Provider 初始化、配置加载等重复开销
- **Prompt/Response 缓存**：`src-tauri/src/llm/service.rs`
  - 对 `test_connection` 等确定性调用进行缓存，5min TTL / 100 条目
  - 缓存命中时直接返回，不再调用模型
- **LLM 调用可观测性**：`src-tauri/src/llm/service.rs`
  - 每次 LLM 调用写入 `llm_calls` 表
  - 输出结构化 `llm_metrics` 日志，便于监控超时率与调用次数
- **流式输出聚合**：`src-tauri/src/llm/service.rs` + `src-frontend/src/hooks/useLlmStream.ts`
  - 后端按 80ms / 40 字符聚合 chunk，减少 IPC 频率
  - 前端 `stopStream()` 正确调用 `llm_cancel_generation`
- **`StoryContextBuilder` LRU 缓存**：`src-tauri/src/creative_engine/context_builder.rs`
  - 50 条目 / 5min TTL，按故事/场景/内容哈希键控
  - 同场景连续续写时显著减少重复 DB 查询
- **DB 连接池扩容**：`src-tauri/src/db/connection.rs`
  - 生产环境 `max_size` 10 → 20，测试环境 5 → 10
- **阻塞查询异步化**：`src-tauri/src/commands/orchestrator.rs` / `src-tauri/src/story_system/preflight.rs`
  - `smart_execute` 初始上下文加载、`check_preflight` 改为 `spawn_blocking`
  - 避免同步 SQLite 查询阻塞 tokio worker
- **协作式取消**：`src-tauri/src/agents/service.rs` / `src-tauri/src/agents/orchestrator.rs`
  - `LlmService` 与 `AgentService` 增加 `is_cancelled` 检查
  - `AgentOrchestrator` 在生成循环中持续检查取消标志
- **Embedding Provider 缓存**：`src-tauri/src/embeddings/provider.rs`
  - OpenAI / Ollama provider 对相同 `(model, text)` 结果缓存 1h / 1000 条目
- **GenesisPipeline 后台化**：`src-tauri/src/commands/orchestrator.rs` / `src-tauri/src/narrative/genesis.rs`
  - 故事概念同步阶段完成后立即返回，用户瞬间看到故事创建成功
  - 第一章与背景设定在后台 `tauri::async_runtime::spawn` 中生成
  - 后台阶段通过 `pipeline-progress` / `pipeline-complete` 事件向前端汇报
- **AgentOrchestrator Full 模式按需降级**：`src-tauri/src/agents/orchestrator.rs`
  - `WorkflowConfig` 新增 `skip_rewrite_threshold`（默认 0.90）
  - 初稿综合评分达到阈值时直接返回，跳过 Inspector→Writer 改写循环

### 创作质量提升

- **重写 Writer 系统提示词**：`src-tauri/src/prompts/engine.rs`
  - 明确要求角色立体（内心/动机/成长）、场景生动（多感官描写）、情节张力（目标-阻碍-转折-困境-决定）
  - 强制伏笔回收、悬念推进、避免陈词滥调
  - 新增命名多样性约束：禁止高频单字姓、禁止单字名、主要角色姓氏不得重复
- **增强上下文注入**：`src-tauri/src/creative_engine/context_builder.rs` / `src-tauri/src/agents/mod.rs`
  - `StoryContextBuilder` 现在读取 `story.methodology_id` / `methodology_step` 并传入 `WorldContext`
  - 场景结构注入 `title`、`outline_content`、`draft_content`
  - 新增 `outline_context` 字段，向 Writer 提供当前场景/章节大纲定位
  - 角色信息注入 `appearance` / `gender` / `age`
  - 新增 `fetch_story_outline_summary`，从故事大纲提取当前场景附近摘要
- **修复 ContextOptimizer 角色信息**：`src-tauri/src/agents/context_optimizer.rs`
  - `CharacterCard` 与 `CharacterInfo` 同步包含 `appearance` / `gender` / `age`
- **命名多样性约束**：`src-tauri/src/narrative/prompts.rs` / `src-tauri/src/agents/novel_creation.rs` / `src-tauri/src/story_system/auto_contract.rs`
  - 所有角色生成 prompt 增加命名多样性要求
  - AutoContract 默认主角生成也要求外貌、性别、年龄
- **默认配置调整**：`src-tauri/src/config/settings.rs` / `src-tauri/src/agents/service.rs` / `src-tauri/src/narrative/genesis.rs`
  - 默认写作策略 `run_mode` 从 `fast` 改为 `balanced`，冲突强度默认 60
  - 免费版 max_tokens 限制从 1000 放宽到 2000
  - Genesis 第一章从 `Fast` 模式改为 `Full` 模式，通过 Inspector 闭环保证第一印象质量
- **内置技能自动调用**：`src-tauri/src/agents/orchestrator.rs`
  - 新增 `apply_writing_skills`：Writer 输出后自动调用 `builtin.emotion_pacing` + `builtin.style_enhancer`
  - 情感节奏优化与文风润色不再依赖 Planner 偶然选中
- **升级 Inspector 质检标准**：`src-tauri/src/prompts/engine.rs` / `src-tauri/src/agents/orchestrator.rs`
  - Inspector 系统提示词从 7 维升级为 7 维质量评分（logic/character/writing/scene/plot/pacing/world）
  - 显式检查人物深度、场景丰富度、情节张力、命名一致性、陈词滥调
  - `parse_inspector_style_analysis` 同步解析新的维度分数
  - 风格分 fallback 使用 writing + scene 平均分，确保无参考文本时也能反映质量

### 前端改进

- **Genesis 后台生成适配**：`src-frontend/src/frontstage/FrontstageApp.tsx`
  - 识别 `novel_bootstrap_background_started` 消息，避免误报「AI 返回了空内容」
  - 故事创建后提示「第一章正在后台生成，完成后会自动加载」
  - 通过 `usePipelineComplete` 监听 `pipeline-complete`，后台完成后自动刷新并切换到第一章
- **测试适配**：`src-frontend/src/frontstage/__tests__/FrontstageApp.test.tsx`
  - mock 更新，添加 `usePipelineComplete`

### 已取消的优化

- **Phase 3.3 计划生成缓存与内置模板**：`src-tauri/src/planner/executor.rs` / `src-tauri/src/planner/template_learning.rs`
  - 硬编码模板和基于 user_input 的计划缓存因创作指令高度依赖上下文而被回退
  - 保留 LLM planner 动态理解意图，避免路由误判导致内容生成偏离用户意图
  - 详细分析见 `PERFORMANCE_OPTIMIZATION_REPORT.md`

### 编译与测试状态

- `cargo check --manifest-path src-tauri/Cargo.toml` ✅
- `cargo clippy --lib` ✅（无新增 warning，仅既有历史 warning）
- `cargo test --lib` ✅ 323/323 通过
- `npx tsc --noEmit`（src-frontend） ✅
- `npx vitest run` ✅ 116 passed / 3 skipped

### 版本号

- `src-tauri/Cargo.toml`: `0.9.4` → `0.9.6`
- `src-tauri/tauri.conf.json`: `0.9.4` → `0.9.6`
- `src-frontend/package.json`: `0.9.4` → `0.9.6`

---

## [v0.9.5] - 智能创作补齐采摘闭环（2026-06-12）

### 摘要

- 修复智能创作（`smart_execute` / `AgentOrchestrator::generate`）生成成功后未触发完整采摘（Ingest）的问题
- 生成内容现在会异步进入 `IngestPipeline`，提取实体/关系并更新知识图谱，与 `auto_write` 保持一致

### 后端改进

- **`AgentOrchestrator::generate` 触发完整采摘**：`src-tauri/src/agents/orchestrator.rs`
  - 在 `MemoryWriter::write` 成功后，异步启动 `IngestPipeline::ingest`
  - 将提取到的实体/关系批量保存到知识图谱（`KnowledgeGraphRepository::save_entities_batch` / `save_relations_batch`）
  - source 标记为 `smart_execute:chapter:{chapter_number}`，便于追踪
  - 失败时仅记录 warn 日志，不阻塞创作结果返回

### 编译与测试状态

- `cargo check --manifest-path src-tauri/Cargo.toml` ✅
- `cargo clippy` ✅（301 warnings 均为既有历史 warning）
- `cargo test --lib` ✅ 318/318 通过

---

## [v0.9.4] - 修复智能创作进度提示长时间卡住的问题（2026-06-12）

### 摘要

- 解决用户反馈的「智能创作/续写时提示长时间显示“正在理解您的创作意图”」问题
- 将 `orchestrator-step`（生成/质检/改写）事件监听从局部改为全局，智能输入栏也能实时看到写作进度
- 在 `smart_execute` 上下文加载阶段新增细粒度进度事件，避免初始阶段无反馈
- 优化初始提示文案：续写意图显示“正在续写...”，通用指令显示“正在理解创作意图并执行...”

### 前端改进

- **全局 orchestrator-step 监听**：`frontstage/FrontstageApp.tsx`
  - 新增全局 `orchestrator-step` 监听器，覆盖智能输入栏（`handleSmartGeneration`）和 Ctrl+Enter（`handleRequestGeneration`）两条路径
  - 统一更新 `generationStatus`、`orchestratorStatus` 与顶部 Toast 大阶段
  - 移除 `handleRequestGeneration` 中的局部监听器，避免重复接收事件
- **Toast 大阶段映射扩展**：`frontstage/FrontstageApp.tsx` 的 `getMajorPhase`
  - 新增“加载上下文 / 读取故事 / 读取章节”→“正在加载故事上下文...”
  - 新增“分析故事上下文 / planning / context”→“正在规划创作步骤...”
  - 新增“执行创作计划 / executing”→“正在执行创作计划...”
  - 新增“生成 / 续写 / writing / draft”→“正在生成续写内容...”
  - 新增“完成 / completed”→“创作计划执行完成...”
- **更准确的初始提示**：`frontstage/FrontstageApp.tsx`
  - 新增 `isContinuationIntent` 辅助函数，识别“续写 / 接着写 / 往下写 / 继续 / 后续”等明确续写意图
  - 续写意图：状态栏显示“正在续写...”，Toast 显示“📝 正在续写...”
  - 通用指令：状态栏显示“正在理解创作意图并执行...”，Toast 显示“💭 正在理解创作意图并执行...”
  - 生成结束后统一清空 `orchestratorStatus`，避免状态残留
- **后台活动监听器修复**：`hooks/useBackendActivityListener.ts`
  - `orchestrator-step` 的 `step_type` 映射从英文（Generation/Inspection/Rewrite）修正为中文（生成 / 质检 / 改写），与后端实际发射值一致
  - 支持 `detail` 字段，优先展示更详细的阶段描述
- **删除“我学到这些”卡片式提示**：`frontstage/FrontstageApp.tsx` + `frontstage/components/AiLearningIndicator.tsx` + `frontstage/styles/frontstage.css`
  - 移除右下角的 `AiLearningIndicator` 卡片组件及相关样式
  - 接受/拒绝续写后的学习反馈改为 `toast.success` 进程提示，样式与其他操作反馈统一
  - 接受时提示：“已记录接受偏好，系统将学习此方向”
  - 拒绝时提示：“已记录拒绝偏好，系统将调整生成策略”
- **完全删除左侧边栏**：`frontstage/FrontstageApp.tsx` + `frontstage/components/FrontstageSidebar.tsx` + `frontstage/styles/frontstage.css`
  - 移除修订模式按钮、生成古典评点按钮、打开幕后工作室按钮
  - 删除 `FrontstageSidebar` 组件文件及全部相关样式
  - 同步清理 `RichTextEditor.tsx` 中的修订模式状态、TrackChanges 扩展、变更追踪 hooks、修订模式横幅
  - 同步清理 `EditorContextMenu.tsx` 中的“修订模式”与“生成古典评点”菜单项
- **打开幕后工作室按钮改为设置图标并移到顶部**：`frontstage/components/FrontstageHeader.tsx`
  - 在顶部色调设置（ColorThemeDot）旁边新增设置按钮
  - 使用 `Settings` 图标，tooltip 为“打开设置 / 幕后工作室”
  - 点击后仍打开幕后工作室
- **采摘图标重新设计**：`frontstage/components/IngestHealthIndicator.tsx`
  - 移除 `Brain`（原图标像橡皮）
  - 新增统一 VI 风格的自定义 SVG 图标：漏斗 + 下箭头，表示知识/素材汇入
- **编辑器右键菜单重新设计**：`frontstage/components/EditorContextMenu.tsx` + `frontstage/styles/frontstage.css`
  - 仅保留 4 个功能：剪切、复制、粘贴、全选
  - 改为统一 VI 风格的纵向列表：暖色纸张背景、圆角、柔和阴影、hover 高亮、`active:scale(0.98)`
  - 图标与文字横向排列，剪切/复制在未选中文本时自动禁用

### 后端改进

- **`smart_execute` 上下文加载进度细化**：`src-tauri/src/commands/orchestrator.rs`
  - “正在加载故事上下文...”→“正在读取故事信息...”
  - 新增“正在读取章节与场景结构...”
  - 新增“正在读取世界观、角色与伏笔...”
  - 新增“正在读取风格配置...”
  - 让初始 DB 查询阶段也有可见反馈，避免用户以为进程卡住

### 编译与测试状态

- `cargo check --manifest-path src-tauri/Cargo.toml` ✅
- `cargo +nightly fmt` ✅
- `npx tsc --noEmit`（src-frontend） ✅
- `npx vitest run src/frontstage/components/__tests__` ✅ 30 passed
- `npx vitest run` ✅ 116 passed（全量前端测试）
- 注：`cargo clippy` 当前仓库存在约 300 个历史 warning，均不在本次修改文件内

### 版本号

- `src-tauri/Cargo.toml`: `0.9.3` → `0.9.4`
- `src-tauri/tauri.conf.json`: `0.9.3` → `0.9.4`
- `src-frontend/package.json`: `0.9.3` → `0.9.4`

---

## [v0.9.3] - 续写性能再优化：候选精简、上下文并行、候选共享缓存（2026-06-12）

### 摘要

- 针对用户反馈的「单次续写 5–10 分钟太慢」做第二轮优化
- 在保留生成质量的前提下，减少 LLM 调用次数、并行化上下文查询、让候选间共享预计算缓存
- 修复 AI 续写接受后内容插入光标位置导致段落混乱的问题

### 性能优化

- **Writer 默认候选数 3 → 2**：`agents/orchestrator.rs` 在续写场景下将并行候选从 3 个减为 2 个，temperature 调整为 `[0.82, 1.0]`，保留多样性的同时减少 1 次 LLM 调用
- **StoryContextBuilder 查询并行化**：`creative_engine/context_builder.rs`
  - `build` / `build_quick` / `build_for_scene` 改为 `async`
  - 第一阶段 `tokio::try_join!` 并行获取 story、characters、scenes、world_rules、writing_style、relevant_entities
  - 第二阶段 `tokio::try_join!` 并行构建 MemoryPack、叙事结构、活跃线索
  - 所有调用方已同步改为 `.await`
- **候选间共享 AgentContext 缓存**：`agents/mod.rs` + `creative_engine/context_builder.rs` + `agents/service.rs`
  - `StyleContext` 新增 `style_dna_extension: Option<String>`
  - `StoryContext` 新增 `personalizer_extension: Option<String>`
  - `StoryContextBuilder` 一次性查库并预计算风格 DNA 提示词扩展与个性化偏好扩展
  - `build_writer_prompt` 优先使用预计算缓存，避免每个 Writer 候选重复查库

### 前端体验优化

- **续写过程状态提示细化**：`agents/orchestrator.rs` + `frontstage/FrontstageApp.tsx`
  - `orchestrator-step` 事件新增 `detail` 字段
  - 候选生成阶段提示："生成候选中（共 2 个）"、"候选评估完成，选用最优结果（匹配度 XX%）"
  - 质检阶段提示："正在评估内容与风格一致性..."、"质检中... 评分 XX%"
  - 改写阶段提示："质检未达标（风格 XX%，叙事 XX%），进入第 N 轮改写优化"

### Bug 修复

- **AI 续写接受后始终追加到正文最后**：`frontstage/components/RichTextEditor.tsx` 新增 `appendText` 方法，`FrontstageApp.tsx` 接受续写时改用 `appendText`，避免插入光标处造成段落混乱

### 编译状态

- `cargo check --lib` ✅
- `cargo +nightly fmt -- --check` ✅
- `cargo test --lib` ✅ **318/318** 通过
- `npm run type-check` ✅
- `npm run test:run` ✅ 124 passed
- `npm run format:check` ✅
- `npm run build` ✅

### 版本号

- `src-tauri/Cargo.toml`: `0.9.2` → `0.9.3`
- `src-tauri/tauri.conf.json`: `0.9.2` → `0.9.3`
- `src-frontend/package.json`: `0.9.2` → `0.9.3`

---

## [v0.9.2] - 自动创作性能优化：并行化、缓存与前端收敛（2026-06-11）

### 摘要

- 全面优化自动创作性能，解决“后台任务多”和“创作速度慢”两大痛点
- `cargo test --lib` 318/318 通过，`vitest run` 124 passed
- 前端状态栏同一时刻只显示一个主任务，减少用户感知混乱

### 后端性能优化

- **PlanExecutor 同 batch 步骤并行**：`planner/executor.rs` 中拓扑排序后的无依赖步骤使用 `join_all` 并行执行，多独立 LLM 调用从串行求和变为并行取最大值
- **GenesisPipeline 后台阶段分组并行**：`narrative/genesis.rs` 将世界观/大纲/角色合并为 `ParallelWorldOutlineCharacterStep`，内部使用 `tokio::join!` 并行调用 LLM；场景、伏笔、知识图谱按依赖顺序执行。后台阶段从 6 个串行步骤优化为 4 个步骤
- **上下文共享可写化**：`GenesisContext.bundle` 升级为 `Arc<RwLock<NarrativeBundle>>`，支持多个后台步骤安全并发读写
- **StoryContextBuilder 查询去重**：`creative_engine/context_builder.rs` 同一次构建中只查一次 scenes，消除 `previous_scenes` 与 `current_scene` 的重复 `get_by_story`
- **LLM 调用层优化**：`llm/service.rs`
  - 按 provider+model+api_base+max_tokens+temperature 缓存 Adapter，避免重复创建 `reqwest::Client`
  - 实际读取 `LlmProfile.timeout_seconds` 作为单次调用超时
  - 增加指数退避重试（最多 2 次），自动识别超时/网络/5xx 等可重试错误
- **数据库调优**：`db/connection.rs` 启用 SQLite WAL、busy_timeout=5000、synchronous=NORMAL，连接池从 5 提升到 10

### 前端体验优化

- **单一主活动显示**：`hooks/useBackendActivityListener.ts` 将合同补齐、Orchestrator、Agent 阶段、smart_execute、pipeline、plan_executor 等 6 类事件聚合为一个 `ai-primary-activity`，按优先级切换显示
- **生成状态收敛**：`FrontstageApp.tsx` 中本地 `isGenerating` 与 `backendActivityStore` 对齐，后台无活动时自动关闭生成锁

### 编译状态

- `cargo build --package storymoss` ✅ 成功
- `cargo test --lib` ✅ **318/318** 通过
- `cd src-frontend && npx tsc --noEmit` ✅ 零错误
- `cd src-frontend && vitest run` ✅ 124 passed, 3 skipped, 0 failed
- `cd src-frontend && npm run build` ✅ 成功

### 版本号

- `src-tauri/Cargo.toml`: `0.9.0` → `0.9.2`
- `src-tauri/tauri.conf.json`: `0.9.0` → `0.9.2`
- `src-frontend/package.json`: `0.9.0` → `0.9.2`

---

## [v0.9.1] - 架构拆分与全面测试覆盖（2026-06-10）

### 摘要

- 完成 Phase 3 架构拆分：God File 拆解 + 模型领域拆分 + RESERVED 模块清理
- 完成 Phase 4 测试覆盖：前端 71 新测试 + Rust 21 新测试 + E2E 36 行为驱动测试
- `cargo check` 零警告，`cargo test` 318/318 通过
- 前端 `tsc --noEmit` 零错误，`vitest run` 124 passed
- E2E `npx playwright test` 32 passed, 4 skipped

### Phase 3 架构拆分

- **repositories.rs 拆分**：6198 行 → 183 行。24 个 Repository 提取到独立 `repositories_{domain}.rs` 文件，保留 Trait Implementations 和 `pub use` 重导出
- **models.rs 拆分**：按领域拆分为 8 个子模块（scene/story/world/knowledge/studio/change_track/user/pipeline），`models/mod.rs` 统一重导出
- **FrontstageApp.tsx 拆分**：提取 5 个自定义 hooks + 2 个纯展示子组件
  - Hooks：`useFrontstageData`、`useFrontstageEditor`、`useFrontstageGeneration`、`useFrontstageWensi`、`useFrontstagePanels`
  - 组件：`HelpPanel.tsx`、`ZenModeExit.tsx`
- **RESERVED 模块清理**：移除 3 个幽灵模块（`src-core` crate、StoryStateManager、Chat 模块）

### Phase 4 测试覆盖

- **前端单元测试（71 新测试）**：
  - Hooks：`useFrontstageWensi` 6 例、`useFrontstagePanels` 8 例、`useFrontstageEditor` 7 例、`useFrontstageGeneration` 6 例
  - 组件：`HelpPanel` 3 例、`ZenModeExit` 2 例
  - 工具函数：`format.ts`（countWords/autoFormatText）14 例、`numberFormat.ts`（normalizeFloat/clampNumber）19 例
- **Rust 核心测试（21 新测试）**：
  - `utils/text`：word_count（中/英/混合）、truncate、normalize_whitespace、remove_markdown — 7 例
  - `utils/file`：extension、sanitize_filename、unique_filename — 3 例
  - `pipeline/refine`：calculate_diff_ratio LCS 差异算法 — 3 例
  - `pipeline/review`：parse_review_json JSON 提取与容错 — 3 例
  - `story_system/scene_service`：should_ingest 场景更新过滤 — 5 例
- **E2E 测试重写（36 测试，7 文件）**：
  - 重写 `storymoss.spec.ts`：从截图驱动转为行为驱动，12 个真实断言
  - 新建 `frontstage-editing.spec.ts`：编辑器输入、自动保存、禅/修订模式 — 7 例
  - 新建 `navigation.spec.ts`：URL 路由、前后台导航 — 4 例
  - 新建 `backstage-pages.spec.ts`：仪表盘/故事/角色/场景/设置/世界观/知识图谱页面加载 — 8 例
  - 共享 `mock-tauri.ts`：集中式 Tauri API mock 工具

### 编译状态

- `cargo check` ✅ 零警告
- `cargo test --lib` ✅ **318/318** 通过
- `cd src-frontend && npx tsc --noEmit` ✅ 零错误
- `cd src-frontend && vitest run` ✅ 124 passed, 3 skipped, 0 failed
- `npx playwright test` ✅ 32 passed, 4 skipped, 0 failed

---

## [v0.9.0] - Brooks-Lint 代码质量重构：DTO、服务下沉、前端拆分、迁移框架（2026-06-08）

### 摘要

- 基于 Brooks-Lint v1.0 扫描报告，完成第一轮代码质量重构
- `cargo check` 接近零警告（仅 1 处预留测试辅助函数 dead_code 提示）
- 前端 `tsc --noEmit` 零错误
- Rust 测试覆盖从 264 → **297** passed，全部本地 SQLite + mock 运行

### Phase 1 基础设施与测试根基

- **测试覆盖扩展至 297 例**：新增 `db/repositories_tests.rs`、`db/cascade_tests.rs`、`canonical_state/tests.rs` 等模块，为核心 Repository、Cascade 删除、规范状态构建器铺设回归保护网
- **本地可运行**：所有新增测试基于纯本地 SQLite + mock LLM，无需网络即可通过
- **迁移框架 SQL 化**：在 `src-tauri/src/db/migrations.rs` 实现自定义 `MigrationRunner`，将历史内联迁移提取为 `V007 ~ V027` 共 21 个版本化 `.sql` 文件，按 `schema_migrations` 表版本顺序执行，支持幂等跳过与 legacy inline 迁移兼容

### Phase 2 启动序列与全局状态治理

- **`lib.rs` 初始化热点收敛**：`run()` 中 500+ 行的发散式 setup 逻辑拆分为 `init_task_system_and_automation()`、`seed_builtin_data()`、`graceful_shutdown()` 等独立函数
- **全局静态文档化**：`DB_POOL`、`APP_CONFIG`、`SKILL_MANAGER`、`CHAPTER_COMMIT_DEBOUNCE` 等全局单例添加详细 SAFETY 注释，明确生命周期与迁移路径
- **连接池初始化整合**：`init_db()` 通过 `MigrationRunner::run_with_legacy()` 统一执行 SQL 迁移与遗留 Rust 内联迁移，避免启动闪退

### Phase 3 领域层重构

- **DTO 独立化**：新建 `src-tauri/src/db/dto.rs`，将 `CreateSceneRequest`、`UpdateStoryRequest`、`CreateChapterRequest`、`CreateCharacterRequest`、`CreateAiOperationRequest` 等 18+ 个请求/响应 DTO 从 `models.rs` 迁出，消除贫血模型与 DTO 混杂
- **Story System 服务下沉**：
  - 新建 `story_system/chapter_service.rs`：`ChapterService` / `ChapterCommitDebouncer` / `PayoffDetector` / `AutomationTrigger`，统一处理章节更新后的 debounce commit、伏笔逾期检测、状态同步、Skill Hook
  - 新建 `story_system/scene_service.rs`：`SceneService` / `SceneIngestor` / `SceneAutomationTrigger`，统一处理场景内容变更后的 KG Ingest、向量索引、world_building 刷新
- **命令层薄化**：`commands/chapter.rs` 与 `scene_commands.rs` 只保留参数校验与事件发射，业务编排全部委托领域服务

### Phase 4 前端架构清理

- **`services/tauri.ts` 拆分完成**：原 1,340 行上帝文件拆分为 `services/api/` 下 17 个按域子模块
  - `core.ts`：仅保留 `loggedInvoke<T>`（带参数脱敏与耗时日志）
  - `stories.ts`、`storySystem.ts`、`skills.ts`、`settings.ts`、`intent.ts`、`annotations.ts`、`knowledge.ts`、`memory.ts`、`pipeline.ts`、`quality.ts`、`genesis.ts`、`stream.ts`、`subscription.ts`、`writing.ts`、`wizard.ts`
  - `index.ts`：barrel export，统一汇总
- **兼容保留**：`services/tauri.ts` 现为 3 行 barrel，历史调用方 `import { ... } from '@/services/tauri'` 无需修改
- **状态同步 Hook 完善**：`useSyncStore.ts` 覆盖 Story / Character / Scene / Chapter / WorldBuilding / StyleDna / Task / Annotation / PayoffLedger / DataRefresh 等全量资源，自动调用 `queryClient.invalidateQueries/removeQueries`

### Phase 5 后端代码质量收尾

- **`cargo check` 接近零警告**：消除 Brooks-Lint 报告中的未使用变量、冗余导入、未处理 Result 等批量问题
- **字段与序列化清理**：`settings.rs` temperature 序列化、`numberFormat.ts` 浮点精度等前期修复保持稳定
- **连接状态模块**：`modelConnectionStore.ts` + `ModelCard` 连接测试可视化稳定运行

### 迁移框架技术细节

- **自定义 `MigrationRunner`**：因项目使用 rusqlite 0.39，未启用 refinery 默认特性，而是实现兼容 runner
- **路径探测**：支持 exe 旁、`CARGO_MANIFEST_DIR`、`src-tauri/src/db/migrations` 等多环境路径
- **事务管理**：每个迁移在独立事务中执行，自动忽略 SQL 中的 `BEGIN/COMMIT/ROLLBACK`
- **幂等安全**：对 `duplicate column name` / `already exists` 等错误做日志警告并跳过，兼容已有数据库
- **版本追踪**：沿用既有 `schema_migrations` 表，现有用户数据库可平滑升级

**编译状态**: `cargo check` 1 警告（测试辅助函数未使用），`cargo test` **297/297** 通过，前端 `tsc --noEmit` 零错误。

---

## [v0.8.2] - LitSeg 拆书融合 Phase 1-6 全面完成（2026-06-03）

### 📖 LitSeg 叙事感知分块与模型增强

- **Phase 1: 叙事感知分块** (`book_deconstruction/chunker.rs`)
  - `NarrativeAware` 分块策略：章节边界为首要叙事边界，大章节(>8000字)按场景转换点再分
  - 3 个单元测试全部通过
- **Phase 2: 模型增强** (`narrative/elements.rs`, `intensity_mapper.rs`)
  - `SceneElement` / `ReferenceScene` 新增 narrative 字段：`narrative_intensity`、`sentiment`、`event_types`、`act_number`、`position_in_act`
  - 新建 `intensity_mapper.rs`：冲突类型→强度、情感基调→极性映射
  - Migration 85：`reference_scenes` 表增强 narrative 字段

### 🔧 Pipeline 后处理与向量化

- **Phase 3: Executor 后处理** (`book_deconstruction/executor.rs`)
  - Pipeline 完成后运行 LitSeg 后处理：计算 intensity、推断幕结构、标注 `act_number`
  - Migration 86：`reference_books` 添加 `analyzed_structure_json`
- **Phase 6: 向量化增强** (`vector/lancedb_store.rs`)
  - `VectorRecord` / `SearchResult` 新增 `metadata` 字段
  - LanceDB schema 添加 `metadata` 列，旧表自动重建

### 🔄 转故事与前端升级

- **Phase 4: convert_to_story 迁移** (`book_deconstruction/service.rs`)
  - 拆书转故事时自动创建 `story_outlines`，携带 narrative 结构
- **Phase 5: 前端升级**
  - `StoryArcView.tsx` 新增幕结构图、场景叙事强度时间线、场景情感分布
  - 保留原有 `story_arc` 解析（向后兼容）

**编译状态**: `cargo check` 零错误，`cargo test` 通过。

---

## [v0.8.1] - LitSeg 叙事感知分段深度融合（2026-05-30）

### 📖 LitSeg 叙事感知分段深度融合

- **深度融合而非机械叠加** — 基于论文 "Narrative-Aware Document Segmentation for Literary RAG" 的核心洞察，将 LitSeg 分析能力融入现有架构而非创建平行系统
- **删除 3 张冗余表**：`narrative_events` / `narrative_threads` / `narrative_structure`
- **增强 4 张现有表**：
  - `scenes` 新增 7 个 narrative 字段（intensity/sentiment/event_types/act_number/position_in_act 等）
  - `foreshadowing_tracker` 新增 setup_event_id / payoff_event_id / risk_signals_score
  - `character_states` 新增 state_transitions_json / arc_type
  - `story_outlines` 新增 analyzed_structure_json
- **新建 1 张表**：`conflict_escalations`（从 narrative_threads.conflict_escalation 提取为结构化表）
- **保留 2 张表**：`narrative_structure_positions`（场景级精细定位）、`narrative_chunks`（物化缓存）

### 🧠 AI 叙事结构感知

- **ingest 流程增强** — 保存章节后自动提取叙事事件并更新 scenes 表 narrative 字段
- **叙事分析流水线** — 在 kg ingest 完成后触发，自动推断叙事线索、分析幕结构、生成叙事感知文本块
- **Agent 上下文增强** — Writer Agent 系统提示词自动注入当前叙事位置（如"第3幕75%，接近高潮"）

### 🖥️ 叙事分析页面

- 新增"叙事分析"侧边栏导航项
- **幕级结构可视化** — 起承转合四幕图，显示章节范围
- **事件强度时间线** — 按章节排序的强度条，直观展示故事节奏
- **活跃线索面板** — 未回收伏笔、角色弧光、冲突升级状态

### 🔧 数据库迁移

- **Migration 79-84**：6 个新迁移完成表结构变更
- `cargo check` 零错误，`cargo test` 通过

---

## [v0.8.0] - 模型管理重构 + 浮点数精度修复 + 连接状态增强（2026-05-29）

### 🎯 模型管理统一集中

- **单一模型管理入口** — 将分散的 Chat/Embedding/Multimodal/Image 四个 Tab 合并为统一的"模型管理"页面 (`UnifiedModelManager`)
- **顶部类型筛选器** — 全部 / 聊天 / 嵌入 / 多模态 / 图像五档筛选，实时过滤
- **按类型分组展示** — 同类型模型归为一组，每组带图标和计数
- **新建模型类型选择** — 添加模型时先选择类型（四卡片 UI），再进入配置表单

### 🔢 浮点数精度全面修复

- **后端 temperature 序列化规范化** (`settings.rs`) — 新增 `temperature_serde` 模块，序列化/反序列化时统一截断到 2 位小数，范围 `[0.0, 2.0]`
- **前端数字工具函数** (`numberFormat.ts`) — `normalizeFloat` / `formatDisplayFloat` / `normalizeInt` / `clampNumber` / `formatLatencyWithQuality`
- **GeneralSettings rewriteThreshold** — slider 值和展示值均经过 `normalizeFloat(value, 2)` 处理，彻底消除 `0.8999999` 类显示问题

### 🔌 模型连接状态丰富化

- **连接测试步骤可视化** (`ModelCard`) — 检测中显示当前步骤名称 + 脉冲动画；已连接显示延迟 + 质量评级（优秀/良好/一般）；连接失败显示红色状态 + 重试按钮 + 可展开的步骤详情列表
- **全局连接状态 Store** (`modelConnectionStore.ts`) — Zustand 统一管理，支持自动轮询（30s）、手动重试、批量检测
- **状态变更 Toast 提示** — 连接恢复 / 断开时自动弹出通知

### 🖥️ 幕前底部栏 Tooltip 增强

- **悬停模型状态点** 显示丰富信息：模型提供商、API Base 简写、连接延迟、最后检测时间
- **连接失败时** 显示"前往配置 →"快捷链接，一键跳转到设置页

### 🛠️ 后端命令重构

- **`commands.rs` 辅助函数提取** — `parse_llm_provider` / `parse_capabilities` / `normalize_temperature` / `build_llm_profile`
- **`create_model` 重复逻辑合并** — Chat/Multimodal 共用 `build_llm_profile`
- **`test_model_connection` 重写** — 返回带 `steps` 字段的详细探测结果，每步包含 `name` / `status` / `detail`

**编译状态**: `cargo check` 零错误，`cargo test` 通过，前端 `tsc --noEmit` 通过，单元测试 59 passed。

---

## [v0.7.9] - 六阶段架构深度优化（2026-05-29）

### 🔒 安全与稳定性

- **修复 FrontstageApp 内存泄漏** — 9 个 Tauri 事件监听器保存 unlisten 回调，组件卸载时统一清理
- **删除旧版前端死代码** — 移除 src/main.js（~1000 行）和 src/views.js（~1400 行），构建产物仅使用 src-frontend/
- **清理 lib.rs 空白行** — 删除 200+ 行连续空白行
- **移除 detect_and_route_intent 死代码** — 始终返回 None 的占位函数及其所有调用点

### 🏗️ 后端模块化

- **拆分 story_commands.rs** — 3445 行单体文件拆分为 4 个领域文件：scene_commands.rs（33 命令）、creation_commands.rs（24 命令）、studio_commands.rs（32 命令）、revision_commands.rs（16 命令）
- **统一错误处理** — 14 个命令文件从 `Result<T, String>` 迁移到 `Result<T, AppError>`，消除两套错误模式并存
- **标准化状态注入** — 所有命令文件从全局 `get_pool()` 改为 `State<'_, DbPool>` 参数注入

### 🔄 状态同步

- **32 个 mutation 命令补全状态同步事件** — skill/export/story_system/intent/studio/creation/revision 等领域
- **React Query 缓存优化** — currentStory 切换时先 `cancelQueries()` 取消过时请求，再 `invalidateQueries()`
- **DOM hack 封装** — App.tsx 中的 forceRedraw 提取为 `useWebViewRedrawFix()` hook

### 🔧 构建工具链

- **Rust 格式化配置** — 新增 rustfmt.toml（max_width=100, edition=2021, imports_granularity）
- **Clippy 配置** — 新增 .clippy.toml（自定义 doc-valid-idents）
- **前端格式化** — 新增 .prettierrc + eslint.config.mjs（ESLint v9 flat config）
- **Vite 代码分割** — manualChunks 配置：react-vendor / editor-vendor / ui-vendor / data-vendor
- **CI 质量门禁** — build.yml 新增 cargo fmt --check、cargo clippy -- -D warnings、npm run format:check、npm run lint

### 🗄️ 数据层深化

- **迁移版本控制** — 新增 schema_migrations 表，`get_current_version()` + `record_migration()` 管理 72 个迁移
- **删除 v3 模型文件** — models_v3.rs（~2900 行）和 repositories_v3.rs（~3200 行）已删除
- **合并 create_v3_tables** — 将 v3 表定义内联到 create_tables，消除重复调用
- **向量存储统一** — 删除 FallbackVectorStore，所有向量操作统一走 LanceVectorStore

### 🏛️ 架构改进

- **AgentContext 拆分** — 拆分为 StoryContext / NarrativeContext / StyleContext / WorldContext / AgentMemoryContext 5 个子结构
- **优雅关闭** — `graceful_shutdown()` 执行 SQLite WAL checkpoint → 持久化 pending vector indexes → 停止 automation service → exit(0)
- **修复 LLM 取消竞态** — `cancel_senders` 改为 `HashMap<String, Option<Sender<()>>>`，`cancel_generation()` 使用 `take()` 原子消费 sender，消除 TOCTOU 竞态

**编译状态**: `cargo check` 零错误，`cargo test` 通过，前端 `tsc --noEmit` 通过。

---

## [v0.7.8] - 记忆无处不在 + 续写风格指纹加固 + 自动更新增强（2026-05-26）

### 🧠 记忆无处不在 — 记忆系统与创作流程深度融合

- **章节号正确传递** — `auto_write` / `smart_execute` / `PlanExecutor` 均使用当前场景的 `sequence_number` 作为章节号，修复了记忆上下文永远按第1章构建的问题
- **MemoryContext 基础设施** — `AgentContext` 新增 `memory_context` 字段，支持带相关度评分的结构化记忆注入（`ScoredMemoryEntry`），`format_memory_context` 显示分数和注入理由
- **Inspector 第7维「记忆一致性」** — 质检 prompt 新增角色状态一致性(30分)、伏笔回收状态(25分)、世界观规则遵守(25分)、时间线连续性(20分)四子维度
- **MemoryWriter 自动写入** — 定稿后自动生成内容摘要，更新 `scene_commits.summary_text` 并创建 `memory_items`，`Orchestrator::generate()` 和 `auto_write` 每轮后自动触发
- **MemoryHealthDaemon** — 每小时运行一次，自动归档遗忘实体，发射 `memory-health-report` 事件到前端
- **完整记忆读写闭环** — 读（章节号正确传递 + MemoryContext 注入）→ 校验（Inspector 第7维）→ 写（MemoryWriter 自动压缩）→ 维护（HealthDaemon 定时归档）

### ✍️ 续写功能风格指纹加固

- **风格指纹引擎** — 从任意参考文本提取句长分布、四字格密度、虚词频率、标志性词汇、锚点片段等量化特征
- **Writer prompt 自动注入** — 实时从 `current_content` 提取指纹注入 system prompt，支持外部参考文本 + `style_weight` 调节
- **Inspector 第6维「风格一致性评分」** — 句长(25%) + 词汇(25%) + 虚词(15%) + 四字格(15%) + 语感(20%) 五子维度
- **Orchestrator 双轨平衡** — `style_score` + `narrative_score` 分别评分，低于阈值自动调节权重
- **3 候选并行生成选优** — temperature 0.75/0.90/1.05 产生多样性，指纹打分选最优（句长40% + 四字格35% + 虚词25%）
- **跨段一致性 4 维度漂移检测** — 句长偏离、四字格密度偏离、虚词偏好偏离、标志性词汇偏离
- **后处理替换层** — 虚词对齐（19组映射）+ 四字格密度补偿（25组二字→四字映射，密度低于70%触发）
- **前端 WenSiPanel 增强** — 参考文本输入 + 风格-叙事平衡滑块 + 实时风格分数显示（绿/橙/红三色）

### 🔄 自动更新系统增强

- **检测间隔缩短** — 24h → 4h
- **后台静默下载** — 下载进度可视化
- **结构化更新日志** — 新增/修复/注意分类
- **CI 配置增量更新** — delta patch
- **CI artifact 上传** — PR / nightly / stable 三种构建场景均上传 .dmg/.deb/.msi

**编译状态**: `cargo check` 零错误，`cargo test` 通过，前端 `tsc --noEmit` 通过，E2E 通过。

---

## [v0.7.7] - 后端预检自动补齐 + 统一后台活动提示系统（2026-05-25）

### 🛡️ 后端预检自动补齐

- **`execute_writer_raw` 自我修复** — 当 `PreflightChecker` 发现缺少 `MASTER_SETTING` / `CHAPTER` 合同或场景大纲时，不再直接返回 `PREFLIGHT_FAILED` 错误，而是自动调用 `AutoContractBuilder::auto_fill` 补齐缺失要素
- **补齐后重检** — 自动补齐完成后重新运行 `PreflightChecker::check()`，只有通过后才继续写作流程
- **事件通知** — 补齐过程中发射 `agent-stage-update` 事件，前端可感知"正在自动补齐"状态
- **全覆盖** — 所有后端写作入口（`smart_execute`、`auto_write`、`auto_revise`、`generate_scene_draft`、`workflow WriteChapter/Revise`）均受益

### 💓 统一后台活动提示系统

- **`backendActivityStore` (Zustand)** — 新增统一后台活动状态管理 Store，支持注册/更新/完成/失败/清理活动，按类别优先级自动选择"最重要"的活动作为 `primaryActivity`
- **`useBackendActivityListener` Hook** — 统一监听 6 类后台事件（`contract-auto-progress`、`orchestrator-step`、`agent-stage-update`、`smart-execute-progress`、`pipeline-progress`、`plan-executor-step`），将分散的进度事件聚合为单一活动状态流
- **`FrontstageBottomBar` 增强** — 底部状态栏新增心跳脉冲动画（`Activity` 图标 + `animate-ping` 扩散圈）、进度条、多任务计数（`+N`）、类别标签（补齐/编排/流水线/续写/修改），让用户持续感知后台智能平台状态
- **大阶段 Toast 提示** — `FrontstageApp` 订阅 store，当 `primaryActivity` 发生阶段变化时自动触发 toast（`📋 补齐` / `📦 流水线` / `⚙️ 编排` / `💭 智能执行`）
- **`WenSiPanel` 同步** — 自动续写/修改的进度（`auto-write-progress-*` / `auto-revise-progress-*`）同步到统一 store，主界面也能感知

### 🔧 版本同步

- **`tauri.conf.json`** — 版本号从 `0.7.5` 同步为 `0.7.7`

**编译状态**: `cargo check` 零错误，前端 `tsc --noEmit` 通过。

---

## [v0.7.6] - 系统性差距审计 + 自动补齐修复 + 事件系统强化（2026-05-23）

### 🔧 场景大纲自动补齐修复

- **`autoCreateSceneOutline` fallback** — 当 `currentScene` 为 null 时，自动创建临时 Scene 对象而非 panic，确保大纲生成流程不中断

### 📋 系统性差距审计与修复计划

- **`docs/plans/2026-05-23-systemic-gap-audit.md`** — 全面审计 14 个"有设计未集成"子系统，分类为 5 个健康等级（完全健康 / 轻微差距 / 显著差距 / 严重差距 / 有设计未集成）
- **`docs/plans/2026-05-23-systemic-gap-fix-plan.md`** — 制定可执行的修复路线图，Phase 5.1~5.3 分步落地

### 🧹 代码清理与架构同步

- **删除废弃 `Chapters.tsx` 页面** — 362 行旧章节管理页面彻底移除，功能已迁移至 Scene 核心流程
- **`CLAUDE.md` 升级** — 追加 Zero-Pause 连续执行层规范 + GitNexus 代码智能协议（影响分析、变更检测、符号重命名约束）
- **Rust 后端模块同步**
  - `memory/mod.rs` / `memory/orchestrator.rs` — 记忆编排器事件处理强化
  - `state_sync/events.rs` / `state_sync/service.rs` — 状态同步事件扩展
  - `story_system/mod.rs` — 故事系统模块接口更新
  - `commands_v3.rs` / `lib.rs` — 命令注册与 handler 映射同步
  - `subscription/commands.rs` — 订阅命令更新
  - `updater/mod.rs` — 更新器模块清理冗余代码

**编译状态**: `cargo check` 零错误，`npm run build` 通过。

---

## [v0.7.5] - 事件驱动创作增强中枢：6 个"有设计未集成"子系统全面落地（2026-05-22）

> **核心理念**：将 `AutomationService` 从"Ghost 系统"升级为事件驱动的创作增强中枢，6 个先前仅存在于设计文档中的高级子系统首次真正融入小说创作核心流程。

### 🎯 事件驱动架构升级

#### 自动化事件覆盖全部关键生命周期

- **`TriggerEvent` 扩展** — 新增 `SceneContentUpdated`、`SceneGenerationRequested`、`SceneGenerated`、`ChapterFinalized` 等事件变体
- **`create_scene`** — 创建成功后触发 `TriggerEvent::SceneCreated`
- **`update_scene`** — 更新成功后触发 `TriggerEvent::SceneContentUpdated`
- **`finalize_draft`** — 定稿成功后触发 `TriggerEvent::ChapterFinalized`
- **Handler 注册** — `evaluate_reading_power_on_update` / `evaluate_reading_power_on_finalize` 两个自动化触发器注册到 `AutomationService`

### 📖 追读力自动评估集成

#### 后端自动触发

- **`update_scene()`** — 内容更新后自动调用 `ReadingPowerEvaluator::evaluate_chapter()`
- **`finalize_draft()`** — 定稿完成后自动评估并保存追读力数据
- **`ChapterReadingPowerRepository`** — 评估结果写入 `chapter_reading_power` 表

#### 前端可操作化

- **StorySystem "追读力"标签页** — 新增"重新评估"按钮，支持手动触发评估
- **API 确认** — `evaluateReadingPower` 已注册到 `tauri.ts`

### 🛡️ Writer Agent 预检集成

#### 真实预检逻辑（替代存根）

- **`PreflightChecker::check()`** — 实现 4 项真实检查：
  - `MASTER_SETTING` 合同是否存在
  - `CHAPTER` 合同是否存在（解析 JSON 提取 chapter_number）
  - 角色列表是否非空
  - 当前 scene 是否有 outline
- **返回结构化结果** — `PreflightResult { ready, issues, blocking_issues }`

#### Writer Agent 前拦截

- **`agents/service.rs` `execute_writer_raw()`** — `build_writer_prompt` 之前调用 `PreflightChecker::check()`
- **阻塞时返回** — `AppError::PreflightFailed { message, issues }`，前端可精准展示阻塞原因
- **`AppError` 扩展** — 新增 `PreflightFailed` 变体，错误码 `PREFLIGHT_FAILED`

### 🔍 语义检索自动注入 Writer Agent

#### `kb_search` 集成到上下文构建

- **`agents/commands.rs`** — `build_context` 之后、`build_writer_prompt` 之前，检查 `request.input` 长度 >= 10
- **自动查询** — 调用 `kb_search`（hybrid 模式，top 5），将语义检索结果格式化为 "相关记忆检索" 段落
- **注入位置** — 追加到 `context.scene_structure`，Writer Prompt 自动包含检索到的相关章节摘要

### 📜 合同与提交链前端可操作化

#### StorySystem "合同"标签页

- **空状态时** — 显示"生成世界观合同"按钮，调用 `createMasterSetting`
- **选择章节后** — 显示"生成章节合同"按钮，调用 `createChapterContract`

#### StorySystem "提交链"标签页

- **空状态时** — 显示"初始化提交"按钮，调用 `initChapterCommit`
- **每条 commit 旁** — 添加"应用提交"按钮，调用 `applyChapterCommit`

### 🔬 叙事审计 story-level 命令与前端面板

#### 后端

- **`audit_story` 命令** — 遍历 story 全部 scene/chapter，调用 `StoryStructureAuditor` 5 维度审计方法
- **聚合报告** — 返回 `StoryAnalysisReport`，含各维度评分、发现问题列表、综合建议
- **注册** — `lib.rs` `generate_handler!` 宏注册

#### 前端

- **StorySystem 新增"审计"标签页** — 显示"运行全面审计"按钮
- **评分展示** — 5 维度进度条（伏笔/角色/场景/世界构建/大纲）
- **问题列表** — 按严重程度分级（Error/Warning/Info）
- **API** — `auditStory` 已注册到 `tauri.ts`

### 🧬 风格进化接入审校反馈

#### 后端

- **`evolve_style_from_anti_ai_review` 命令** — 接收 `story_id` + `AntiAiReview`，调用 `StyleEvolutionEngine::evolve_from_reviews()`
- **DNA 更新** — 计算 `StyleDnaDelta` 后写入 `style_dna` 表（`update_dna_json`）
- **注册** — `lib.rs` `generate_handler!` 宏注册

#### 前端

- **Anti-AI 审校结果面板** — 新增"接受审校并进化风格"按钮
- **状态管理** — `isEvolving` 状态 + `handleEvolveStyle` 处理函数
- **API** — `evolveStyleFromAntiAiReview` 已注册到 `tauri.ts`

### 🤖 合同自动补齐（AutoContractBuilder）

#### 自动补齐缺失合同

- **`story_system/auto_contract.rs`** — 新增 `AutoContractBuilder`，当预检发现缺少 `MASTER_SETTING` 或 `CHAPTER` 合同时自动触发
- **世界观合同自动生成** — 读取故事标题/体裁/简介/角色（最多10个）/世界构建（概念/规则/历史）/已有章节摘要（最多5章），构建 Prompt 调用 LLM 生成 `MasterSettingContract`，自动保存到 `story_contracts`
- **章节合同自动生成** — 读取故事信息 + 前一章摘要（500字）/ 当前章内容（1000字）/ 后一章摘要（300字）+ 世界观概要，构建 Prompt 调用 LLM 生成 `ChapterContract`，自动保存到 `story_contracts`
- **进度事件** — `contract-auto-progress`（stage/message/progress），前端实时展示补齐进度

#### 前端预检逻辑重构

- **`FrontstageApp.tsx`** — `handleRequestGeneration` / `handleSmartGeneration` 预检失败时，若检测到缺少合同，自动调用 `autoCreateMissingContracts` 而非仅报错
- **进度监听** — 监听 `contract-auto-progress` 事件，将进度消息实时显示在生成状态栏（"正在自动补齐合同..." → "世界观合同已生成并保存" → "合同补齐完成，继续生成..."）
- **补齐后自动续写** — 合同补齐成功后自动继续 AI 生成流程，无需用户再次点击

**编译状态**: `cargo check` 零错误，`npm run build` 通过。

---

### 🧹 废弃系统清理（Phase 4）

#### WebSocket 协作服务器

- **`lib.rs`** — WebSocket 服务器启动代码已注释掉，减少运行时资源占用
- **`useCollaboration.ts`** — `connect()` 直接 toast 提示"协同编辑功能即将推出，敬请期待"，不再尝试连接
- **模块标记** — `chat` / `collab` / `state` 模块声明旁标注 `RESERVED`

#### StoryStateManager

- **`state/manager.rs`** — 模块顶部添加 `RESERVED FOR FUTURE USE` 说明，与 `CanonicalStateManager` + `StateSync` 功能重叠，暂不维护

#### Chat 模块

- **`chat/mod.rs`** — 模块顶部添加 `RESERVED FOR FUTURE USE` 说明，有 DB 表但无命令暴露

**编译状态**: `cargo check` 零错误，`npm run build` 通过。

---

## [v0.7.4] - 世界构建页面 + 世界-场景自动关联（2026-05-22）

### 🌍 幕后世界构建页面

#### 新增 `WorldBuilding` backstage 页面

- **文件**: `src-frontend/src/pages/WorldBuilding.tsx`
- **功能**: 显式调整和设置小说的世界观
  - 核心概念编辑（textarea + 800ms debounce 自动保存）
  - 世界规则管理（增删改弹窗，8 种类型标签，1-10 重要性星级）
  - 历史背景编辑（textarea + 自动保存）
  - 文化体系管理（增删改弹窗，习俗/价值观标签展示）
- **数据流**: 复用已有 `useWorldBuilding` / `useCreateWorldBuilding` / `useUpdateWorldBuilding` hooks
- **空状态**: 未选择故事提示；无 world_building 数据时显示「初始化世界构建」按钮

#### 路由与导航注册

- `types/index.ts`: `ViewType` 扩展 `'world_building'`
- `Sidebar.tsx`: `navItems` 新增「世界构建」（Globe 图标，位于「角色」与「场景」之间）
- `App.tsx`: `renderView()` switch 注册 `<WorldBuilding />`

### 🔗 世界-场景自动关联（场景增世界增，场景减世界减）

#### `SceneRepository` 注入同步逻辑

- `update()` — 场景 setting 字段变更时自动同步到 `world_building`:
  - `setting_location` → 自动生成 `Physical` 类型 `WorldRule`（如不存在）
  - `setting_atmosphere` → 自动生成 `Cultural` 类型 `WorldRule`（如不存在）
  - `setting_time` → 去重追加到 `world_buildings.history`
- `delete()` — 场景删除后检查其他场景是否仍引用相同 setting，如无引用则删除对应的 auto-generated 规则
- 自动生成规则通过 `description` 中的 `(auto-generated from scene)` 标记，与用户手动规则区分

#### 实时同步事件

- `create_scene` / `update_scene` / `delete_scene` 命令在 setting 字段变更后追加 `emit_world_building_updated`，确保前端世界构建页面实时刷新

**编译状态**: `cargo check` 零错误，`npm run build` 通过。

---

## [v0.7.3+] - 数据库初始化闪退修复（2026-05-22）

### 🐛 修复：应用启动闪退

#### 根因

旧数据库升级路径中存在 4 处迁移冲突，导致 `init_db` 返回错误，`r2d2::Pool` 未被 `app.manage()` 注册，后续 `app.state()` 调用触发 `state() called before manage()` panic。

#### 修复内容

| 修复点                      | 问题                                                                                                               | 解决                                                             |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------- |
| `create_tables` 初始 Schema | `scene_divider_nodes` 表在 `scenes` 表之前创建，FK 引用失败                                                        | 移除 `scene_divider_nodes`（已由 Migration 72 处理）             |
| Migration 71                | `ALTER TABLE chapters DROP COLUMN scene_id` 失败，因为列上有索引                                                   | 先 `DROP INDEX idx_chapters_scene`，再 `DROP COLUMN`             |
| Migration 69                | `INSERT OR IGNORE INTO narrative_*` 从 `reference_*` 迁移数据时，`book_id` 不存在于 `stories` 表，触发 FK 约束失败 | 添加 `EXISTS (SELECT 1 FROM stories WHERE id = rc.book_id)` 过滤 |
| Migration 70                | `ALTER TABLE chapter_commits RENAME TO scene_commits` 失败，因为 Migration 48 已创建空的 `scene_commits`           | 重命名前检测并 `DROP` 空表                                       |

**编译状态**：`cargo check` 零错误，`cargo test` ~225/225 通过，`npm run build` 通过。

---

## [v0.7.3] - 商业模式重构 + 1:N Chapter↔Scene 架构完成 + SceneDivider 预留接口（2026-05-20）

### 💼 商业模式重构：订阅解锁功能，非模型配额

#### SubscriptionService 精简

- **移除配额计量体系**：删除 `QuotaDetail`、`QuotaCheckResult`、`OFFLINE_GRACE_LIMIT` 及全部配额检查/消费方法
- **移除方法**：`get_or_create_quota`、`get_quota_detail`、`check_auto_write_quota`、`consume_auto_write_quota`、`check_auto_revise_quota`、`consume_auto_revise_quota`、`check_platform_model_quota`、`consume_platform_model_quota`、`check_ai_quota`、`consume_ai_quota`
- **保留方法**：`get_or_create_subscription`（仅创建/查询订阅状态）、`has_feature_access`（功能开关检查）、`upgrade_subscription`、`log_ai_usage`（纯统计）

#### `has_feature_access` 细粒度功能权限

- **Free 用户可用**：`writer`、`scene_management`、`character_management`、`knowledge_graph_query`、`outline`
- **Pro/Enterprise 解锁**：`pipeline_refine`、`pipeline_review`、`pipeline_finalize`、`book_deconstruction`、`auto_write`、`auto_revise` 等全部高级功能
- **拆书与 Pipeline 命令接入**：`book_deconstruction/commands.rs` 和 `pipeline/commands.rs` 统一调用 `has_feature_access`，未授权返回 `AppError::subscription_required`

#### `AppError::SubscriptionRequired` 取代 `QuotaExceeded`

- **错误码变更**：`QUOTA_EXCEEDED` → `SUBSCRIPTION_REQUIRED`
- **字段变更**：`{ quota_type, remaining }` → `{ feature_id, current_tier }`
- **构造函数变更**：`quota_exceeded()` → `subscription_required(feature_id, message)`
- **前端兼容**：`loggedInvoke` 按 `SUBSCRIPTION_REQUIRED` code 渲染升级引导 UI

#### `log_ai_usage` 纯统计（不参与配额控制）

- 记录字段：`user_id`、`story_id`、`chapter_id`、`agent_type`、`instruction`、`prompt_tokens`、`completion_tokens`、`model_used`、`cost`、`duration_ms`、`tier_at_time`
- 仅用于 UsageStats 看板展示，不做任何拦截或限制

### 🏗️ 1:N Chapter↔Scene 架构完成（Phase 4）

#### 废弃 `chapters.scene_id`

- **Migration 71**：`chapters` 表移除 `scene_id` 列（旧数据库 `DROP COLUMN IF EXISTS`）
- **`Chapter` 模型**：删除 `pub scene_id: Option<String>` 字段
- **全链路改为 `scenes.chapter_id` 查询**：
  - `ChapterRepository::create` / `update` / `get_by_story`：不再读写 `chapters.scene_id`
  - `SceneRepository::create`：不再 `UPDATE chapters SET scene_id = ?`
  - `SceneRepository::delete`：不再 `UPDATE chapters SET scene_id = NULL`
  - `lib.rs` `create_chapter` / `update_chapter`：改为 `SceneRepository::get_by_chapter(&chapter.id)` 查询关联场景，触发 `SceneCommitService::auto_commit`

#### `SceneCommitService` 取代 `ChapterCommitService`

- **提交粒度对齐 Scene**：`scene_commits` 表新增 `scene_id` 外键（可空），记录 `state_deltas_json` / `entity_deltas_json` / `accepted_events_json`，提交链以 Scene 为单元
- **Migration 70**：`chapter_commits` 表重命名为 `scene_commits`；所有索引同步重建（`idx_scene_commits_story` / `idx_scene_commits_scene` / `idx_scene_commits_number` / `idx_scene_commits_chapter`）
- **Repository 重命名**：`ChapterCommitRepository` → `SceneCommitRepository`，`ChapterCommit` → `SceneCommit`
- **Service 重命名**：`story_system::SceneCommitService::auto_commit(story_id, chapter_id, scene_id)` — 有 `scene_id` 时写入 scene_commits，无则保持基于 chapter 的兼容路径
- **IPC 命令**：`init_chapter_commit` / `apply_chapter_commit` / `get_chapter_commits` 保留原签名，内部操作 `scene_commits` 表

#### `SceneDividerNode` 功能预留接口

- **`SceneDividerNode` 模型**（`db/models_v3.rs`）：`id` / `chapter_id` / `position` / `scene_id` / `label` / `created_at` / `updated_at`
- **`SceneDividerRepository`**（`db/repositories_v3.rs`）：`create`、`get_by_chapter`、`set_dividers`（事务级全量替换）、`delete`、`delete_by_chapter`
- **Migration 72**：`scene_divider_nodes` 表（`TEXT` 主键 + `chapter_id` 外键 + `position` 整数 + `scene_id` / `label`），新建数据库自动创建；`CREATE INDEX idx_scene_divider_chapter ON scene_divider_nodes(chapter_id)`
- **前端保留**：`SceneDividerNode.ts` TipTap 扩展维持原子块节点定义，等待 1:N 编辑器模式激活

### 🔧 其他变更

- **`db/connection.rs` 初始 Schema**：`scenes` 表 CREATE TABLE 添加 `chapter_id TEXT` + `FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL`；`CREATE INDEX idx_scene_chapter ON scenes(chapter_id)`；新增 `scene_divider_nodes` 表定义
- **编译状态**：`cargo check` 零错误，`cargo test` ~225/225 通过，`npm run build` 通过
- **版本号**：Cargo.toml / package.json / tauri.conf.json → 0.7.3

---

## [v0.7.3+] - 高密度状态世界构建法（2026-05-21）

> **核心理念**：源于 90 年代经典老游戏在极致资源约束下的结构智慧。用极少元素，通过状态驱动、桥节点连接、事件回流与多功能重用，构建远大于实际篇幅的"活的世界"。

### 🌍 新增第五种创作方法论

#### 后端实现

- **`high_density_world_building.rs`** — 新建方法论模块，定义 `WorldBuildingPhase` 枚举（Seed/StateExpansion/Convergence/DensityIteration）+ `HighDensityWorldBuildingMethodology` 实现 `Methodology` trait
- **`FromStr` 解析** — 支持 `"seed"` / `"1"` / `"state_expansion"` / `"2"` / `"convergence"` / `"3"` / `"density_iteration"` / `"4"` 等多种阶段标识解析
- **`MethodologyType` 注册** — `mod.rs` 新增 `HighDensityWorldBuilding` 变体，`name()` / `description()` / `build_prompt_extension()` / `list_available()` 完整分支
- **`agents/service.rs` 接入** — writer prompt 映射新增 `"world_building"` → `MethodologyType::HighDensityWorldBuilding`

#### 四阶段世界构建流程

- **阶段 1：最小世界种子** — 设计高密度"世界切片"（锚点场景/地点/事件），定义核心状态向量（身份/资源/关系旗标/历史旗标/心理目标），创建 3-5 个桥节点（每个至少连接 3 条叙事线）
- **阶段 2：状态网扩张** — 主角群扩展（每人独特初始状态但共享桥节点），列出"状态触发表"（资源匮乏+关系敌对→冲突等），世界规则显式化，信息不对称矩阵
- **阶段 3：多线交织与回流** — 桥节点多线映射（正面/侧面/误解视角），每 3-5 章至少一次回流，事件多功能重用（叙事+世界构建+象征/驱动），伏笔与回响网络
- **阶段 4：密度迭代与克制** — 克制检查清单（每引入新元素问能否被现有替代），"未写出的世界"留白审计，状态一致性审计，涌现性验证，重读价值优化

#### 前端集成

- **`MethodologySettings.tsx`** — `methodologies` 数组新增 `{ id: 'world_building', name: '高密度世界构建', description: '...' }` 选项
- **阶段选择 UI** — 新增 `worldBuildingPhases` 数组（4 个阶段名称）+ `methodologyId === 'world_building'` 条件渲染阶段选择器
- **保存逻辑** — `handleSave` 正确写入 `methodology_id` + `methodology_step`

#### 输出 Schema

每阶段提供结构化 JSON Schema，规范 AI 输出格式：

- Seed：`{ seed: { anchor_scene, state_vectors[], bridge_nodes[] } }`
- StateExpansion：`{ protagonists[], trigger_table[], world_rules[], information_asymmetry[] }`
- Convergence：`{ bridge_perspectives[], convergence_points[], event_functions[], foreshadowing[] }`
- DensityIteration：`{ restraint_check, unwritten_world, state_consistency[], emergence_validation, reread_value[] }`

**编译与测试**

- `cargo check`：零错误
- `cargo test`：~225/225 通过
- `npm run build`：通过

---

## [v0.7.0] - AI 三审 Pipeline + 角色动态状态 + 用量统计 + 幕前指令升级（2026-05-15）

### 🏭 AI 三审 Pipeline 系统

#### Pipeline 核心架构

- **`pipeline/mod.rs`** — 四级创作管线：`Rewrite` → `Refine` → `Review` → `Finalize`
- **`pipeline/refine.rs`** — `run_refine(story_id, draft_id, chapter_info, config)`：AI 修稿，对章节草稿进行语言润色、结构调整、错别字修正
- **`pipeline/review.rs`** — `run_review(...)`：AI 审稿，输出 `overall_score`（0-100）+ `dimensions` JSON 数组 + `issues` JSON 数组 + `content` 总结
- **`pipeline/finalize.rs`** — `finalize_draft(...)`：定稿主流程，创建 `PostProcessStep` 记录 → 执行各后处理步骤 → 更新状态为 Success/Failed
- **`pipeline/post_process.rs`** — `run_post_process_step(...)` + `run_character_cards(...)`：执行单个后处理步骤，LLM 驱动角色状态解析

#### 后处理步骤追踪

- **`PostProcessStepDef`** 定义 4 个标准步骤：`kb_import`（知识库更新）、`chapter_notes`（章节笔记）、`character_cards`（角色状态卡）、`style_analysis`（风格分析）
- **`PostProcessStep`** 数据库记录：`id`/`story_id`/`chapter_number`/`step_type`/`status`（Running/Success/Failed）/`is_critical`/`error_message`/`created_at`/`updated_at`
- **关键/非关键分类**：`is_critical=true` 的步骤失败时阻断定稿流程；非关键步骤失败仅记录日志，不影响整体定稿
- **真实执行**：`finalize.rs` 创建 `LlmService` 并调用 `run_post_process_step`，更新每一步的真实状态（替代原占位实现）

#### LLM 驱动角色状态解析

- `run_character_cards` 构建综合 Prompt：角色上下文（姓名/背景/性格/目标/外貌/关系）+ 章节内容
- 调用 LLM 输出 JSON 数组：`[{ "character_id": "...", "cs_location": "...", "cs_power_level": "...", ... }]`
- 解析 JSON 后批量更新 `characters` 表的 6 个动态状态字段 + `cs_updated_at_chapter`

#### 前端 Pipeline 面板

- **`Stories.tsx`** 场景级 Pipeline 进度看板：场景列表显示 `execution_stage` 彩色徽章（plan/outline/draft/review/final）+ 多色进度条
- **Actions/Drafts/Reviews 三标签页**：Actions 执行修稿/审稿/定稿；Drafts 查看草稿列表；Reviews 查看审稿结果与评分
- **`usePipeline.ts`** 新增 `parseReviewResult()` 辅助函数，将后端 `PipelineReview`（JSON 字符串字段）解析为结构化 `ReviewResult`

#### 幕前 `/` 指令打通

- **`RichTextEditor.tsx`** 扩展 slash 命令映射：`AI修稿`/`修稿` → `pipeline_refine`、`AI审稿`/`审稿` → `pipeline_review`、`定稿` → `pipeline_finalize`
- **`FrontstageApp.tsx`** 新增 `handlePipelineRefine` / `handlePipelineReview` / `handlePipelineFinalize` 处理器，调用 `runRefine` / `runReview` / `runFinalize` IPC
- `RichTextEditorRef` 扩展 `setContent(text: string)` 方法，支持 Pipeline 执行后自动回写编辑器内容

### 🧬 角色动态状态系统

#### 6 项动态状态字段

- `cs_location` — 角色当前所在位置
- `cs_power_level` — 实力等级/修为层次
- `cs_physical_state` — 身体状态（健康/负伤/疲惫等）
- `cs_mental_state` — 心理状态（平静/焦虑/愤怒等）
- `cs_key_items` — 随身携带的关键物品
- `cs_recent_events` — 最近经历的重要事件
- `cs_updated_at_chapter` — 状态最后更新的章节号

#### `CharacterStatePanel` 组件

- 可折叠 UI：每个角色卡片下方展开/收起状态面板
- 6 字段只读展示 + `cs_updated_at_chapter` 时间戳
- 内联编辑：点击字段值进入编辑模式，失焦自动保存
- API：`update_character_state` IPC 命令更新单字段

### 📊 用量统计与可观测性

#### `UsageStats` 页面

- 幕后独立页面，Sidebar `BarChart3` 图标导航
- **全局统计卡片**：总调用次数 / 总 token 数 / 平均响应时间 / 成功率
- **单故事统计**：按故事维度聚合（调用次数 / token 消耗）
- **最近调用记录表**：最近 20 条，展示模型名称、功能、token、耗时、状态、时间

#### 后端 API

- `get_llm_call_stats(story_id?: string)` — 返回 `LlmCallStats`（全局或单故事）
- `get_recent_llm_calls(limit: i64)` — 返回最近 N 条 `LlmCallRecord`
- `LlmCallRepository` — `get_stats()` / `get_recent()` 统计查询

### 🖥️ 前端架构升级

- **`Sidebar.tsx`** 新增「用量统计」导航项（`BarChart3` 图标）
- **`App.tsx`** 新增 `UsageStats` 路由 case（`'usage-stats'`）
- **`types/index.ts`** `ViewType` 扩展 `'usage-stats'`

### 🔧 技术细节

- `finalize.rs` 原占位实现重写为真实执行逻辑：创建 `LlmService` → 遍历步骤定义 → 执行 `run_post_process_step` → 更新数据库状态
- `post_process.rs` 修复 `resp.text` → `resp.content`（`GenerateResponse` 字段名修正）
- `usePipeline.ts` 类型对齐：`refreshReviews` 返回 `ReviewResult | null`（通过 `parseReviewResult` 转换）

### 编译状态

- `cargo check` ✅ 零错误
- `cargo test` ✅ ~225/225 全部通过
- `npm run build` ✅ 通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 0.7.0

---

## [v0.7.2+1] - 网文体裁模板扩充至 43 个 + typical_structure 全面补全（2026-05-20）

### 📝 网文体裁模板扩充与优化

#### 新增 5 个 2026 年热门/经典缺失体裁模板

- **灵气复苏** (`spiritual-recovery`) — 现代都市中灵气突然复苏，强调"日常与超凡的撕裂感"与旧秩序崩溃后的新博弈
- **规则怪谈** (`rules-horror`) — 以"规则文本"为核心恐怖机制的独立流派，逻辑推理与在规则夹缝中求生存
- **模拟器流** (`simulator`) — 系统流独立分支，"人生模拟器"推演不同选择，用无数次虚拟死亡换取现实中一次正确选择
- **盗墓流** (`tomb-raiding`) — 经典探险流派，古墓探险、机关解谜、风水秘术，揭开历史谜团
- **星际机甲** (`mecha-stellar`) — 科幻核心分支，机甲战斗、星际战争、宇宙探索，钢铁浪漫与史诗感

#### 现有 38 个模板全面优化

- **typical_structure 补全** — 全部 38 个现有模板从空数组 `[]` 补充为完整的 `{title, description}` 典型结构节点（平均 5-6 个阶段），为 AI 生成提供更清晰的叙事结构指引
- **凡人流反模式修复** — 原 `anti_patterns` 为空的凡人流模板补充 5 条反模式（资质逆转、准备无敌、越阶无代价、人缘逆天、长生无感）

#### 数据更新

- `templates/genres.json`：`count` 38 → 43，全部 43 个 profile 已补充 `typical_structure`
- 新增 5 个 Markdown 模板文件：`mecha-stellar.md` / `spiritual-recovery.md` / `rules-horror.md` / `simulator.md` / `tomb-raiding.md`

**编译与测试**

- `cargo check`：零错误
- `cargo test`：~225/225 通过
- `npm run build`：通过
- 版本号维持：Cargo.toml / package.json / tauri.conf.json → 0.7.2

---

## [v0.7.2] - 存储同构化 + MCP 动态注册 + 聚合编辑 + 场景分隔节点 + LLM 取消（2026-05-19）

### 🗄️ 拆书分析存储同构化

#### 统一存储层：reference*\* → narrative*\*

- **`BookDeconstructionExecutor`** — 分析结果保存时统一写入 `narrative_characters` / `narrative_scenes` / `narrative_world_buildings`，`source='extracted'` / `status='reference'`
- **`BookDeconstructionService::run_analysis`** — 保存到 `reference_*` 后，同步转换为 `CharacterElement` / `SceneElement` 并写入 `narrative_*`，保持过渡期双向兼容
- **`BookDeconstructionService::get_analysis`** — 从 `narrative_*` 读取并转换回 `BookAnalysisResult`，API 接口零变动
- **`BookDeconstructionService::delete_book`** — 删除时同步清理 `narrative_characters` / `narrative_scenes` / `narrative_world_buildings`
- **`BookDeconstructionService::convert_to_story`** — 一键转故事时自动 `UPDATE narrative_* SET status = 'active'`，角色/场景从参考态切换为生产态

#### Migration 69：历史数据自动迁移

- `reference_characters` → `narrative_characters`：`INSERT OR IGNORE` + `LEFT JOIN` 去重，缺失字段（`background`/`goals`/`gender`/`age`）置空或默认值
- `reference_scenes` → `narrative_scenes`：同上，字段映射对齐 `SceneElement` 结构

### 🔌 MCP 工具动态注册

- **`CapabilityRegistry` 实时同步** — 外部 MCP 服务器连接成功后，将其工具列表注入 `CapabilityRegistry`，`PlanGenerator` 即时感知新工具，无需重启应用
- **前缀命名空间** — 内置工具 `mcp.builtin.*`，外部工具 `mcp.{server_id}.*`，前端 `list_mcp_tools` 返回完整带前缀列表，零配置区分来源
- **动态注销** — MCP 服务器断开时从 `CapabilityRegistry` 移除对应工具，防止调用已失效外部工具
- **`execute_mcp_tool` 注册修复** — 移除对不存在的 `GenericToolHandler` 的引用，动态注册仅更新 `CapabilityRegistry`，工具调用走现有 `call_mcp_tool` 路径

### 📝 1:N 聚合编辑数据库 Schema

- **Migration 68**：`chapter_commits` 表新增 `chapter_id` 字段（`TEXT` / `Option<String>`），支持多场景聚合到单一章节的提交追踪
- **全链路对齐** — `ChapterCommit` 结构体、`ChapterCommitRepository`（create/get/update SQL）、`story_system/mod.rs` 全部适配新字段
- **`ChapterCommitService::auto_commit`** — 有 `chapter_id` 时写入外键，无则保持 `NULL`，兼容现有单场景提交路径

### 🖊️ TipTap SceneDividerNode

- **原子块节点** — `src-frontend/src/frontstage/extensions/SceneDividerNode.ts`，`group: 'block'` / `atom: true` / `selectable: false`
- **可视化渲染** — 幕前编辑器中相邻场景间显示水平分隔线 + 场景标题标签，结构边界一目了然
- **不可编辑** — 用户无法直接输入或删除分隔节点，仅作为结构标记，防止误操作破坏场景边界
- **事件扩展** — 支持 `click` / `mouseenter` / `mouseleave`，可扩展悬浮面板展示场景元信息

### ⏹️ LLM 调用取消机制

- **`request_id` 级取消** — `LlmService` 维护 `cancel_senders: HashMap<String, Sender<()>>`，每次 `generate` / `generate_stream` 分配唯一 `request_id`
- **`cancel_generation(request_id)`** — 精确向指定 sender 发送 `()` 信号，中断对应请求，不影响其他并行 LLM 调用
- **流式适配** — `generate_stream` 每接收一个 chunk 前 `try_recv` 检查取消信号，收到后立即终止流并返回已生成内容
- **`AgentOrchestrator`** — `WorkflowResult` 新增 `request_id` 字段，上层可通过同一 ID 取消正在执行的生成任务

### ⚠️ AppError 结构化 IPC

- **统一错误格式** — 所有 Tauri 命令返回 `Result<T, AppError>`，JSON 序列化为 `{ code: string, message: string, data?: unknown }`
- **错误码体系** — `NOT_FOUND` / `VALIDATION_ERROR` / `LLM_ERROR` / `CANCELLED` / `TIMEOUT` / `INTERNAL_ERROR` / `UNAUTHORIZED`
- **前端精准处理** — `loggedInvoke` 捕获 `AppError` 后按 `code` 分支：toast 提示 / 静默忽略 / 自动重试 / 引导用户操作
- **向后兼容** — 未显式返回 `AppError` 的命令仍走原有字符串错误路径，逐步迁移

### 🔧 其他修复

- **`CharacterElement::fears` 字段补全** — `service.rs` 构造 `CharacterElement` 时新增 `fears: String::new()`，修复 `cargo check` 编译错误
- **`AgentResult::request_id` 测试补全** — `orchestrator.rs` 测试初始化补充 `request_id: None`，修复 `cargo test` 编译错误

### 编译状态

- `cargo check` ✅ 零错误
- `cargo test` ✅ ~225/225 全部通过
- `npm run build` ✅ 通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 0.7.2

---

## [v0.7.1] - 架构优化：聚合提交 + 导出完整性 + 组件提取（2026-05-17）

### 🏗️ 后端架构优化

#### ChapterCommitService 防抖聚合提交

- **`lib.rs`** — 移除独立的 `auto_ingest_chapter` 函数、`INGEST_COOLDOWN`、`hash_content`，消除与 Projection Writer 的重复索引工作
- **`CHAPTER_COMMIT_DEBOUNCE`** — 新增全局防抖状态，`CHAPTER_COMMIT_DEBOUNCE_SECONDS = 30`
- **`ChapterCommitService::auto_commit()`** — 取代 `auto_ingest_chapter`，30 秒空闲延迟后自动聚合提交，驱动 `VectorProjectionWriter` / `MemoryProjectionWriter`
- `update_chapter` / `create_chapter` 命令统一调用 `auto_commit` 而非独立摄取

#### 导出聚合完整性

- **`export/mod.rs`** — `export_to_file` 新增 `scenes` 参数
- **`export_story`（`lib.rs`）** — 导出前自动检查章节内容，空章节按关联场景的 `sequence_number` 排序聚合填充，确保 Markdown/HTML/PlainText 导出完整无缺
- `generate_json` 导出 schema 扩展为包含 `scenes` 数组，支持全数据便携导出

### 🖥️ 前端架构优化

#### 大型组件提取重构

- **`Settings.tsx`** — 提取 8 个原子化子组件到 `src/pages/settings/`：`ModelCard`、`ModelList`、`ModelModal`、`StatsSettings`、`MethodologySettings`、`WorkflowSettings`、`GeneralSettings`、`AccountSettings`
- **`SceneEditor.tsx`** — 提取 `SceneAuditPanel` 和 `SceneAnnotationPanel` 到 `src/components/scene-editor/` 子目录，消除重复渲染与关注点混杂
- 清理未使用导入：`Image`、`createLogger`、`Clock`、`Eye`、`FileText`、`CharacterConflict`、`AuditReport`
- 移除 `SceneEditor.tsx` 中未使用的 `handleStageChange` 函数

#### StoryTimeline 场景进度可视化

- **`StoryTimeline.tsx`** — 场景卡片新增 `execution_stage` 彩色徽章（plan/outline/draft/review/final），与叙事阶段（铺垫/上升/高潮/收尾）双轨可视化
- 新增辅助函数 `getExecutionStageLabel()` / `getExecutionStageColor()`

### 编译状态

- `cargo check` ✅ 零错误
- `cargo test` ✅ ~225/225 全部通过
- `npm run build` ✅ 通过

---

## [v6.0.0] - Story System 合同驱动 + 三层记忆编排 + 追读力评估 + 37 体裁模板 + Anti-AI 审查（2026-05-15）

### 🏗️ 架构级新体系：Story System 合同驱动

#### 四级合同架构

- **`story_contracts` 表**（Migration 47）— 四级合同存储：`MASTER_SETTING`（故事级全局设定）/ `Volume`（卷级设定）/ `Chapter`（章节级设定与预期）/ `Review`（审阅与修订合同）
- **`chapter_commits` 表**（Migration 48）— CHAPTER_COMMIT 写后真源，记录 `state_deltas_json`、`entity_deltas_json`、`accepted_events_json`，形成提交链
- **8 个新 Repository** — `StoryContractRepository`、`ChapterCommitRepository`、`MemoryItemRepository`、`ChapterReadingPowerRepository`、`ChaseDebtRepository`、`OverrideContractRepository`、`ReviewIssueRepository`、`GenreProfileRepository`

#### CHAPTER_COMMIT 提交链与 Projection Writer

- `ChapterCommitService::init_commit()` — 创建初始 commit 记录
- `ChapterCommitService::apply_commit()` — 异步应用 commit，驱动 5 个 Projection Writer
  - `StateProjectionWriter` — 解析 `state_deltas_json`，写入 `memory_items`（category="state"）
  - `IndexProjectionWriter` — 解析 `entity_deltas_json`，写入 `memory_items`（category="entity"）
  - `SummaryProjectionWriter` — 生成章节摘要，写入 `story_summaries`
  - `MemoryProjectionWriter` — 解析 `accepted_events_json`，写入 `memory_items`（category="event"）
  - `VectorProjectionWriter` — 生成摘要 embedding，写入 LanceDB `VectorRecord`
- `ContractTree` / `RuntimeContract` — 按故事/卷/章节层级查询合同树，动态合并上层合同生成运行时约束合同
- **防幻觉三定律**：合同即法律、设定即物理、发明需识别

### 🧠 三层记忆编排器

#### MemoryOrchestrator

- `build_memory_pack()` — 按任务类型（write/plan/review）动态组装 MemoryPack
- **三层记忆模型**：
  - **Working Memory**：最近 5 章 + 活跃角色（出场 > 3 次）+ 开放伏笔（未回收）
  - **Episodic Memory**：state_changes + relationships 时间线，最近 10 条
  - **Semantic Memory**：长期事实，按优先级排序（Critical > High > Medium > Low > Background），支持源章节窗口过滤（仅保留最近 30 章内的事实）
- **MemoryBudget**：write 任务分配 Working 50% / Episodic 30% / Semantic 20%；plan 任务 Semantic 优先
- **冲突检测**：比较 Working 与 Semantic 记忆，检测矛盾并输出 `MemoryWarning`
- `MemoryPack` / `MemoryEntry` / `MemoryItemDto` / `MemoryStats` 完整数据结构

### 📈 追读力评估系统

#### ReadingPowerEvaluator

- `evaluate()` — 单章追读力五维评估：
  - **Hook 检测**：悬念/冲突/转折三类钩子识别与计数
  - **Coolpoint 追踪**：打脸/收获/揭秘三类爽点追踪
  - **Micropayoff 微兑现**：章节内小承诺兑现检测
  - **综合评分**：0-100 分，加权计算
- `get_trend()` — 返回最近 N 章评分趋势数组

#### DebtManager

- `create_debt()` — 创建未兑现承诺/伏笔债务
- `accrue_interest()` — 债务逾期自动计算利息（每日 5%）
- `check_overdue_debts()` — 扫描逾期债务并返回告警
- `create_override_contract()` — 作者声明临时跳过债务并记录理由
- `fulfill_contract()` — 债务兑现后更新状态
- `OverrideContract` / `ChaseDebt` 完整数据结构

### 📚 37 体裁模板库

#### GenreProfile

- **`templates/genres/`** 目录新增 37 个 Markdown 体裁模板文件
- **内置模板覆盖**：玄幻/仙侠/都市/历史/科幻/悬疑/言情/武侠/游戏/修真/无限流/系统流/重生/穿越/快穿/凡人流/争霸流/幕后流/签到流/御兽流/驭鬼流/诡异流/赛博朋克/蒸汽朋克/克苏鲁/国运流/种田/末世/轻小说/体育/军事/西幻/灵异/现实/洪荒/武侠仙侠/诸天万界
- **模板五要素**：核心基调、节奏策略、反模式清单、参考数据表、典型结构
- `GenreProfileRepository` — SQLite 持久化，支持 `get_all()` / `get_by_id()`
- 前端 `StorySystem.tsx` 集成体裁查看面板

### 🔍 Anti-AI 五维审查

#### AntiAiReviewer

- `review(text, genre)` — 对输入文本进行五维度 AI 痕迹审查：
  - **词汇维度**：Cliché 检测（"浩瀚"、"磅礴"、"无尽"、"宛如"等 AI 高频词列表）+ 重复用词统计
  - **语法维度**：句式多样性（句长标准差评估，< 5 为单调）+ 被动语态计数
  - **叙事维度**：段落长度均匀度（变异系数 < 0.3 为可疑）+ 感官密度（五感关键词密度，< 3% 为贫乏）
  - **情感维度**：标签化检测（"愤怒地"、"悲伤地说"等副词+说组合）+ 展示 vs 告知判断（直接情感词密度 > 5% 为过度告知）
  - **对话维度**：说明性对话检测（对话中包含设定/背景信息占比 > 30% 为过度说明）+ 标签单调性（连续 3 句使用相同对话标签如"说道"）
- **输出结构**：`overall_score`（0-100）+ `dimensions[]`（各维度 0-100 分）+ `issues[]`（问题列表）+ `suggestions[]`（改进建议）+ `flagged_passages[]`（标记段落）

### 🖥️ 前端集成

#### StorySystem.tsx

- 新增幕后页面「故事系统」，5 个标签页：
  - **Contracts** — 合同树浏览、运行时合同查看
  - **Commits** — CHAPTER_COMMIT 提交历史、Projection 状态追踪
  - **Reading** — 追读力评分与趋势图、Debt 债务看板
  - **Memory** — 记忆包组装结果、三层记忆浏览
  - **Anti-AI** — 五维审查结果、评分与建议

#### Sidebar.tsx

- 新增「故事系统」导航入口（ShieldCheck 图标）

#### App.tsx

- 导入 `StorySystem` 组件，添加 `story-system` 路由 case

#### tauri.ts

- 新增 17 个 v6.0.0 IPC 命令的 TypeScript 接口与 API 函数：
  - `create_master_setting` / `create_chapter_contract` / `get_contract_tree` / `get_runtime_contract`
  - `init_chapter_commit` / `apply_chapter_commit` / `get_chapter_commits`
  - `build_memory_pack` / `get_memory_items` / `create_memory_item`
  - `evaluate_reading_power` / `get_reading_power_trend` / `get_chase_debts` / `create_override_contract`
  - `get_genre_profiles` / `get_genre_profile`
  - `anti_ai_review`

### 🗄️ 数据库迁移

- **Migration 47**：`story_contracts` 表（四级合同）
- **Migration 48**：`chapter_commits` 表（提交链）
- **Migration 49**：`memory_items` 表（记忆项）
- **Migration 50**：`story_summaries` 表（章节摘要）
- **Migration 51**：`chapter_reading_powers` 表（追读力评分）
- **Migration 52**：`chase_debts` 表（债务追踪）
- **Migration 53**：`override_contracts` 表（覆盖合同）
- **Migration 54**：`review_issues` 表（审查问题）
- **Migration 55**：`genre_profiles` 表（体裁模板）
- **scenes 表扩展**：新增 `writing_phase` 字段（v6.0.0 叙事阶段标记）

### 🔧 技术细节

- `IpcResponse` trait 要求 `Serialize` derive — `ContractTree` / `RuntimeContract` / `MemoryPack` / `ReadingPowerEvaluation` 等新增结构体均已实现
- `apply_chapter_commit` 为异步 Tauri 命令，通过 `VECTOR_STORE.get()` 获取 LanceDB 实例进行向量投影
- `rusqlite 0.39` 兼容：`row.get::<_, i32>()` 显式类型注解
- Anti-AI 审查中中文引号使用 Unicode 转义（`\u{201C}` / `\u{201D}` / `\u{2018}` / `\u{2019}`）避免空字符字面量编译错误

**类型安全基座**

- ts-rs 集成：`SyncEvent` / `FrontstageEvent` / `BackstageEvent` 添加 `#[derive(TS)]`
- 前端穷尽匹配：`useSyncStore.ts` 重构为 typed discriminated union，`assertUnreachable(type: never)`
- IPC 一致性检查：`scripts/verify-ipc-manifest.py` 自动比对前后端命令注册

**可靠性与可观测性**

- Ingest 作业追踪（Migration 55）：`ingest_jobs` 表记录 pending/running/completed/failed
- 采摘健康指示器：幕前顶栏 🧠 图标，点击展示最近 3 条记录
- Projection 健康检查：`check_projection_health` 解析 `projection_status_json`
- 功能使用度量（Migration 56）：`feature_usage_logs` 表 + Settings「数据统计」标签
- 技术债务清理：删除 `bug_condition_v57.rs` 中 11 个已修复的 `#[ignore]` 测试

**UX 微优化**

- 角色悬浮卡片：RichTextEditor hover 角色名 600ms 显示微型浮卡
- 体裁模板外部化：`templates/genres.json` 支持用户自定义
- 导出出版前体检：ExportDialog 4 步流程，可选 Anti-AI 审查

### 编译状态

- `cargo check` ✅ 零错误（~121 warnings）
- `cargo test` ✅ ~225/225 全部通过（0 ignored，历史 bug condition 测试已删除）
- `npm run build` ✅ 通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 6.0.0

---

## [v5.6.4] - v5.6.4 补丁：JSON 修复、场景去重、自动排版、CI 修复（2026-05-15）

### 🔴 P0 JSON 解析与生成稳定性

#### `extract_and_sanitize_json` 全面增强

- **字符串内未转义换行符修复** — LLM 经常在 JSON 字符串值中直接换行，导致 `serde_json::from_str` 解析失败。新增状态机：仅在字符串内部将实际换行符替换为 `\n`，避免破坏 JSON 结构
- **C 风格注释移除** — LLM 有时在 JSON 中插入 `//` 或 `/* */` 注释，导致解析失败。新增注释跳过逻辑（保留换行以维持行号）
- **移除破坏性中文引号替换** — 原代码将中文引号「」『』强制替换为 ASCII 引号 `"`，这会破坏 JSON 字符串边界（如键名或值中包含中文引号时）。修复：不再替换中文引号，JSON 格式错误由 LLM 自行修正

#### 场景生成去重与幂等性

- **跳过重复 `sequence_number`** — LLM 返回的场景列表中可能包含重复的 `sequence_number`（重试或格式错误）。`SceneGenerationStep` 新增 `seen_seqs` HashSet，遇到重复序号时跳过并记录警告日志
- **更新已存在场景而非重复创建** — Bootstrap 重试或重新执行时，`sequence_number` 已存在的场景不再新建，而是获取现有记录并更新标题/内容，避免数据库中出现重复场景

#### 数据库类型安全加固

- **`scene_id`/`chapter_id` 显式类型注解** — `repositories.rs` 和 `repositories_v3.rs` 中 `row.get(9)?` 等调用添加显式 `Option<String>` 类型注解，消除编译器推断歧义，防止空值场景下的运行时 panic
- **`pending_vector_indexes` 查询容错** — `lib.rs` 中 `chapter_id` 查询结果从 `String` 改为 `Option<String>`，数据库中可能存在的 NULL 值不再导致 `rusqlite::Error`

### 🟡 P1 前端排版与体验

#### `autoFormatText` 自动排版引擎

- **新增 `src-frontend/src/utils/format.ts`** — 智能中文段落分段与引号规范化
  - 直引号 `"..."` / `'...'` 自动转换为中文弯引号「...」/『...』
  - 按句子长度（2~4 句/段）、对话检测（以引号开头优先独立成段）智能分段
  - 已有 `<p>` 标签的 HTML 输入保留结构，仅规范化引号
  - 输出标准 HTML `<p>` 包裹
- **集成到 `FrontstageApp`** — 所有内容更新路径（`ContentUpdate`/`AppendContent`/`ChapterSwitch`/`SmartGeneration`）统一经过 `autoFormatText`，LLM 返回的未格式化纯文本自动转换为标准 HTML 段落

#### AI 续写去重保护

- **去除重复前缀** — LLM 有时返回包含当前编辑器完整内容的续写结果，导致用户看到重复文本。`requestGeneration` 中新增前缀检测：若生成内容以当前编辑器文本开头，自动截去重复前缀，仅保留新增部分
- **空内容保护** — 去重后若内容为空，直接提示"AI 续写内容与当前文本相同，无需添加"，不插入空幽灵文本

#### 排版与样式优化

- **`frontstage.css` 借鉴 heti 排版理念** — 添加 `overflow-wrap: break-word`、`hyphens: auto`、`text-spacing-trim: space-all`、`text-autospace: ideograph-alpha`，改善中西文混排效果
- **段落间距公式化** — `.ProseMirror p` 的 `margin-block-start/end` 改为基于行高的动态计算，更符合中文排版网格
- **`AiSuggestionNode` 接受逻辑修复** — 接受 AI 建议时先删除原文段落再插入新内容，避免旧内容残留导致重复

### 🟢 P2 基础设施

#### GitHub Actions CI 修复

- **移除 `.cargo/config.toml` UTF-8 BOM** — 字符 65279（`\u{feff}`）导致 `tauri-action` 发布步骤在 Windows 和 Ubuntu 上解析失败。移除 BOM 后三平台构建通过
- **macOS 构建目标修正** — `macos-latest` runner 已升级为 Apple Silicon（arm64/M1/M2），原 `x86_64-apple-darwin` 目标在交叉编译时触发 LanceDB AVX512 链接错误（`_sum_4bit_dist_table_32bytes_batch_avx512` symbol not found）。修复：目标改为 `aarch64-apple-darwin`
- **同步本地缺失的 workflow 文件** — `.github/workflows/build.yml` 之前仅存在于 GitHub 远程，未纳入本地版本控制。现已同步到仓库

### 编译状态

- `cargo check` ✅ 零错误（121 warnings）
- `cargo test` ✅ ~225/225 全部通过
- `npm run build` ✅ 通过
- GitHub Actions ✅ rust-check / frontend-check / e2e-check 全部通过；tauri-build 三平台修复验证中

---

## [v5.6.4] - Tauri v2 IPC `rename_all = "snake_case"` 根本修复（2026-05-08）

### 🔴 P0 核心断裂修复

#### Tauri v2 自动 camelCase 转换导致 IPC 参数静默丢弃

- **根因**：Tauri v2 默认行为 — `#[tauri::command]` 自动将 Rust `snake_case` 参数名转换为 `camelCase` 传给 JS 前端。v5.6.3 修复将前端参数从 camelCase 改为 snake_case，但未同步修改后端命令宏，导致 Tauri 仍期望 camelCase 而前端传 snake_case，参数全部静默丢弃
- **影响范围**：`smart_execute`（`user_input`/`current_content` 被丢弃 → AI 续写不可用）、`get_input_hint`、`record_feedback`、`call_mcp_tool`、`check_auto_write_quota`/`check_auto_revise_quota` 等全部命令
- **修复**：157 个后端 `#[tauri::command]` 全部添加 `rename_all = "snake_case"`
  - `src-tauri/src/lib.rs`：63 个命令
  - `src-tauri/src/commands_v3.rs`：92 个命令
  - `src-tauri/src/subscription/commands.rs`：2 个命令
- **机制**：`rename_all = "snake_case"` 禁用 Tauri 自动转换，前端传 `user_input` → 后端接收 `user_input`，零映射歧义

### v5.6.4 设计-实现对齐全面修复 v6（2026-05-13）

#### 🔴 P0 数据层根因修复 — 补齐缺失表定义与级联删除

- **`story_metadata` 表定义补齐**（Migration 43）— `automation/service.rs` 大量操作 `story_metadata` 表，但 `connection.rs` schema 中缺失 CREATE TABLE。修复：新增 `story_metadata` 表（`story_id`/`key`/`value`/`updated_at`）+ 复合索引 + `REFERENCES stories(id) ON DELETE CASCADE` 外键约束
- **`scene_characters` 表定义补齐**（Migration 44）— `SceneCharacterRepository` 操作 `scene_characters` 表，但 schema 缺失。修复：新增表（`id`/`scene_id`/`character_id`/`created_at`）+ 双外键级联 + 双索引
- **`scene_character_actions` 表定义补齐**（Migration 45）— 同上，新增表（`id`/`scene_id`/`character_id`/`action_type`/`content`/`created_at`）+ 双外键级联
- **`delete_story` 显式级联清理加固** — 原仅依赖外键约束，但 `story_metadata`/`foreshadowing_tracker`/`user_preferences`/`ai_operations` 等表无外键或外键未覆盖。修复：事务内显式 DELETE 14+ 关联表数据（`story_metadata`/`story_outlines`/`foreshadowing_tracker`/`user_preferences`/`world_buildings`/`character_relationships`/`character_states`/`scenes`/`chapters`/`scene_characters`/`scene_character_actions`/`ai_operations`/`narrative_characters`/`narrative_scenes`），防御性编程确保零幽灵数据
- **`delete_character` 显式级联清理加固** — 事务内显式清理 `scene_characters`/`scene_character_actions`/`character_relationships`/`character_states`，消除外键未覆盖的残留

#### 🔴 P0 同步事件补全 — 幕前幕后自动关联

- **`auto_ingest_chapter` 发射同步事件** — Ingest 成功保存实体/关系后，追加 `emit_ingestion_completed` + `emit_data_refresh(_, _, "knowledgeGraph")`，幕后 KG 可视化自动刷新新抽取的实体
- **`update_scene` auto ingest 发射同步事件** — 异步 ingest 块完成保存后，通过 `AppHandle` 发射 `ingestionCompleted` + `dataRefresh(knowledgeGraph)`，消除场景内容更新后 KG 不刷新的问题
- **KG CRUD 命令统一 StateSync** — `create_entity`/`update_entity`/`delete_entity`/`create_relation`/`delete_relation` 命令末尾全部追加 `emit_data_refresh(_, _, "knowledgeGraph")`，所有 KG 更新统一经过 StateSync，前端实时感知变更
- **前端 `useSyncStore` 补全特定事件 case** — 新增 `case 'characterRelationshipsUpdated'`（刷新 `characterRelationships` 缓存）、`case 'payoffLedgerUpdated'`（刷新 `payoffLedger` 缓存）、`case 'ingestionCompleted'`（刷新 `knowledgeGraph` 缓存），后端直接发射特定事件时前端正确响应

#### 🟡 P1 Automation Service 全面集成

- **`create_story` 触发 `StoryCreated`** — 故事创建完成后推入 Automation Service 事件队列，激活 `init_story_structure` 等触发器
- **`create_character` 触发 `CharacterCreated`** — 角色创建完成后触发自动化事件，激活角色分析/关系推断等触发器
- **`update_chapter` 触发 `ChapterContentUpdated`** — 章节保存后触发内容更新事件（带字数），激活章节审校/向量索引等触发器
- **`update_scene` 触发内容更新事件** — 场景内容更新后推入 Automation Service，扩展自动化覆盖范围
- **`automation/service.rs` Tauri v2 API 修复** — `emit_all` → `emit`（Tauri v2 `Emitter` trait），`word_count` 类型 `usize` → `i32` 转换修复

#### 🟡 P1 后台自动化闭环

- **`PlanTemplateLibrary` SQLite 持久化**（Migration 46）— 原纯 `Vec<PlanTemplate>` 内存存储，重启后学习成果丢失。修复：新建 `plan_templates` 表（`id`/`trigger_patterns`/`plan_json`/`success_count`/`failure_count`/`created_at`）；`new()` 时从数据库加载；`record_success()` 时保存到 SQLite；`find_match()` 从内存+数据库查询。避免重复 LLM 调用，实现"越写越懂"效果持续累积
- **能力进化周期触发机制** — 原仅启动时延迟 30 秒执行一次 `evolve_capability_descriptions()`，长时间运行后不进化。修复：`ExecutionRecordStore::append()` 每次追加记录后检查总记录数是否达到阈值（默认 5 的倍数），达到则 `tokio::spawn` 异步触发进化。保留启动时兜底进化
- **`WorkflowEngine` 恢复实例自动入队** — `with_pool()` 从数据库加载 Pending/Running/Paused 实例到内存 HashMap，但恢复的实例未加入 Scheduler 队列，重启后中断工作流永不执行。修复：`with_pool()` 返回待恢复实例 ID 列表；`lib.rs` setup 中遍历列表调用 `scheduler.schedule_execution(instance_id).await`，确保应用重启后工作流自动恢复调度

#### 🟢 P2 系统整洁度优化

- **Settings.tsx 隐藏图像生成 Tab** — 后端无图像生成 IPC 命令或 Agent，用户配置后无法使用。修复：隐藏"图像生成" Tab 并标注"暂未实现"，消除死胡同功能
- **StateSync 注释加固** — 在 KG 更新命令中添加注释，说明所有 KG 更新必须经过 StateSync，防止未来开发绕过同步路径

### 编译状态

- `cargo check` ✅ 零错误（109 warnings）
- `cargo test` ✅ ~225/225 全部通过（0 ignored，历史 bug condition 测试已删除）
- `npm run build` ✅ 通过

---

## [v5.6.3] - IPC 参数一致性全面修复 + Bootstrap 序列化修复（2026-05-08）

### 🔴 P0 核心断裂修复

#### Bootstrap 进度卡死修复

- **角色字段缺失导致 serde 反序列化失败** — LLM 返回的 JSON 可能省略 `CharacterElement::age` 和 `SceneElement` 的 8 个核心字段（`sequence_number`/`title`/`summary`/`dramatic_goal`/`external_pressure`/`conflict_type`/`setting_location`/`setting_time`）。缺失字段导致 `serde_json::from_str` 失败 → `PipelineError::ParseError` → 后续步骤永不执行 → 前端永久显示 "塑造角色 (3/6)"。修复：给所有可能被 LLM 省略的字段添加 `#[serde(default)]`
- **Bootstrap 事件缺少状态传递** — `BootstrapProgressEvent` 没有 `status` 字段，前端无法区分进行中和失败。修复：新增 `BootstrapStatus` 枚举（`InProgress`/`Completed`/`Failed`），事件包含 `status`，前端根据状态显示 ❌ 失败标记

#### IPC 参数名全面审计与修复

- **smart_execute camelCase 传参** — 前端传 `userInput`/`currentContent`，后端期望 `user_input`/`current_content`。Tauri v2 反序列化不匹配导致参数静默丢弃为 `None`，AI 续写/润色完全不可用。修复：前端改为 snake_case 传参
- **get_input_hint camelCase 传参** — 前端传 `currentContent`，后端期望 `current_content`。修复：改为 snake_case
- **record_feedback 参数结构错误** — 前端将请求对象展开为平铺字段传递，后端期望 `{ request: RecordFeedbackRequest }` 包裹对象。修复：前端改为 `{ request: req }`
- **call_mcp_tool camelCase 传参** — 前端传 `toolName`，后端期望 `tool_name`。修复：改为 snake_case
- **check_auto_write_quota / check_auto_revise_quota camelCase 传参** — 前端传 `requestedChars`，后端期望 `requested_chars`。修复：改为 snake_case
- **updateConfig 裸 invoke** — `save_settings` 使用裸 `invoke` 绕过日志追踪和错误脱敏。修复：统一使用 `loggedInvoke`

#### 后端命令参数补全

- **run_creation_workflow mode 映射错误** — 前端传 `"human_draft_ai_polish"`，后端只识别 `"human_first"`，导致 "我初稿 + AI 润色" 模式被错误映射为 "AI 初稿 + 我精修"。修复： `"human_draft_ai_polish"` 映射到 `CreationMode::HumanDraftAiPolish`
- **update_story 缺少 genre 参数** — 后端 `update_story` 命令签名缺少 `genre`，前端 Stories.tsx 编辑表单修改类型被静默忽略。修复：后端添加 `genre: Option<String>`，更新 `UpdateStoryRequest` / `StoryRepository::update` SQL
- **create_character 扩展字段被忽略** — 后端只接受 `story_id`/`name`/`background`，前端传的 `personality`/`goals`/`appearance`/`gender`/`age` 被硬编码为 `None`。修复：后端扩展参数列表
- **update_character 扩展字段被忽略** — 后端只接受 `name`/`background`/`personality`/`goals`，缺少 `appearance`/`gender`/`age`。修复：后端扩展参数并传给 Repository

### 编译状态

- `cargo check` ✅ 零错误（109 warnings）
- `npm run build` ✅ 通过
- `cargo tauri build` ✅ Windows 安装包生成

---

## [v5.6.2] - 设计-实现对齐全面修复 v5（2026-05-08）

### 🔴 P0 核心断裂修复

#### 前端缓存同步精确化

- **writingStyle 缓存刷新错误** — `useSyncStore.ts` 中 `case 'writingStyle'` 刷新的是 `['world_building', storyId]`，但 `useWritingStyle` hook 使用的 queryKey 是 `['writing_style', storyId]`。写作风格更新后前端写作风格缓存不会自动刷新。修复：同时刷新 `['writing_style', storyId]` 缓存
- **chapterUpdated 缓存刷新不精确** — `case 'chapterUpdated'` 仅调用 `invalidateQueries(['chapters'])`（全局），未刷新 `['chapters', storyId]`（当前故事）。幕前保存章节后，幕后 chapters 列表可能不立即刷新。修复：补充 `invalidateQueries(['chapters', storyId])`

#### 后台自动化闭环补全

- **update_scene 未触发向量索引** — `update_chapter`/`create_chapter` 保存后触发 `auto_ingest_chapter`（含 KG 分析 + 向量索引），但 `update_scene` 仅内联了 KG 分析，未写入 LanceDB 向量存储。修复：将 `VECTOR_STORE`/`embeddings` 可见性提升为 `pub(crate)`；`update_scene` 的 Ingest 逻辑补充 `embed_text_async` → `VectorRecord` → `add_record` 向量索引闭环；通过独立作用域隔离 `Box<dyn Error>` 避免 `Send` 编译错误

### 🟡 P1 功能补全

#### 前端缓存同步增强

- **storySelected 未刷新关联数据** — `case 'storySelected'` 仅触发回调，未调用 `invalidateQueries`。幕后切换故事时关联数据刷新依赖 `App.tsx` 中的 `useEffect` 时序。修复：补充 characters/scenes/chapters/worldBuilding/foreshadowings/storyOutlines/knowledgeGraph/characterRelationships 缓存刷新
- **dataRefresh 缺少 knowledgeGraph/characterRelationships 单独 case** — 后端可能发射单独资源类型事件，但前端 switch 未处理。修复：补充 `case 'knowledgeGraph'` 和 `case 'characterRelationships'`

### 🟢 P2 优化

- ** cargo warnings 清理** — `resource_type()`/`emit_story_selected`/`emit_scene_selected`/`validate_token`/`redirect_port` 等 5 处 dead_code 警告。修复：添加 `#[allow(dead_code)]` 标记保留 API（未来可能使用），warnings 从 113 降至 109

### 编译状态

- `cargo check` ✅ 零错误
- `npm run build` ✅ 通过
- `cargo test` 待验证

---

## [v5.6.1] - 设计-实现对齐全面修复 v4（2026-05-08）

### 🔴 P0 核心断裂修复

#### 幕前幕后自动关联补全

- **sceneCreated/sceneDeleted 缓存不对称** — `useSyncStore.ts` 中 `sceneCreated`/`sceneDeleted` 只刷新 `scenes` 缓存，不刷新 `chapters` 缓存。后端 `SceneRepository::create`/`delete` 会修改 `chapters.scene_id` / `chapters.chapter_id`，但前端 chapters 列表中的 scene 关联状态滞后。修复：两个 case 中追加 `invalidateQueries(['chapters', storyId])`

#### 自适应学习真实反馈

- **FrontstageApp learnings 伪实现** — v5.6.0 注释声称"非硬编码 mock"，但 `setLearnings()` 仍是固定字符串。修复：后端 `record_feedback` 返回 `Vec<LearningPoint>`，同步调用 `PreferenceMiner::mine` 获取真实偏好；前端 `handleAcceptGeneration`/`handleRejectGeneration` 使用返回结果设置 learnings，无结果时 graceful fallback

### 🟡 P1 功能补全

#### 前端缓存同步完整覆盖

- **WritingStyle 更新缓存不刷新** — `update_writing_style` 发射 `data-refresh("writingStyle")`，但 `useSyncStore.ts` 无对应 case。修复：新增 `case 'writingStyle'` 刷新 `worldBuilding` 缓存
- **Outline/Foreshadowing 更新缓存不刷新** — 后端发射 `storyOutlines`/`foreshadowings`，前端 `useSyncStore.ts` 缺少对应 case。修复：新增 `case 'storyOutlines'` 和 `case 'foreshadowings'` 分别刷新对应缓存

#### 后台自动化加固

- **Pending vector SQLite 持久化** — v5.5.0 使用 `pending_vector_indexes.json` 文件持久化，与文档声明的"SQLite 持久化"不符。修复：Migration 42 创建 `pending_vector_indexes` 表；`save_pending_vector_indexes`/`load_pending_vector_indexes` 改为 SQLite 操作，保留 JSON fallback 用于迁移

### 🟢 P2 优化

- **WorkflowScheduler 文档一致性** — 代码使用 `join_all` 并行执行同层节点，但文档描述为"串行拓扑执行"。修复：AGENTS.md 更新为"拓扑有序执行（同层可并行）"
- **Workflow 幂等性** — `schedule_execution` 无幂等检查，同一 instance_id 可被重复入队。修复：入队前检查 queue 和 running_instances，已存在则跳过

### 编译状态

- `cargo check` ✅ 零错误
- `cargo test` ✅ 217/217 通过
- `npm run build` ✅ 通过
- `cargo tauri build` ✅ Windows 安装包生成

---

## [v5.6.0] - 设计-实现对齐全面修复 v3（2026-05-08）

### 🔴 P0 致命差距修复

#### 数据一致性

- **Scene 删除外键悬空** — `SceneRepository::delete` 删除前未清理 `chapters.scene_id` 外键，导致 chapter 指向已删除 scene。修复：删除 scene 前 `UPDATE chapters SET scene_id = NULL WHERE scene_id = ?`
- **Wizard 创建后前端不刷新** — `create_story_with_wizard` 完成所有步骤后未发射同步事件，前端 Stories 列表不显示新故事。修复：流程结束时发射 `story_created` + `data_refresh("all")` 双重事件
- **CharacterElement relationships 硬编码空数组** — `NarrativeCharacterRepository::get_by_story` 返回 `relationships: Vec::new()`，角色关系卡片始终为空。修复：二次查询 `character_relationships` 表，按 `source_character_id` JOIN `characters` 获取 `target_name` 填充 `CharacterRelationship`
- **Collab 文档同步返回空** — `CollabSession::get_current_document` 直接返回 `(String::new(), 0)`，协同编辑文档内容丢失。修复：遍历 `self.operations` 使用 OT 变换逐条 apply 重建文档内容
- **Workflow EdgeCondition 永不匹配** — `get_next_nodes` 原代码 `all(|edge| completed.contains(&edge.from_node))` 忽略了 `edge.condition` 字段，条件边永远被视为满足。修复：`EdgeCondition::evaluate()` 实现完整条件表达式求值（Eq/Neq/Gt/Gte/Lt/Lte/Contains/NotContains），根据 `instance.context.variables` 判断

#### 任务系统可靠性

- **Task 心跳超时无重试** — HeartbeatMonitor 检测到任务超时后仅标记 `Failed`，不触发重试。修复：超时后若 `retry_count < max_retries`，计算指数退避 `30*2^retry` 秒更新 `next_run_at`，状态回退为 `Pending`，发射 `task-retried` 事件

### 🟡 P1 重要差距修复

#### 缓存同步对称性

- **Outline/Foreshadowing 修改无同步** — `update_story_outline`、`create_foreshadowing`、`update_foreshadowing_status`、`update_payoff_ledger_fields` 修改数据后未发射同步事件。修复：所有方法完成后调用 `StateSync::emit_data_refresh`
- **Cache 失效不对称** — `sceneUpdated` 只失效 scenes 缓存但 chapter 数据中的 scene 引用已变；`chapterDeleted` 不清理关联 scenes 缓存。修复：`sceneUpdated` 追加 `invalidateQueries(['chapters', storyId])`；`chapterDeleted` 追加 `invalidateQueries(['scenes', storyId])`

#### Workflow 健壮性

- **Workflow 节点无限阻塞** — `execute_node` 中 LLM 调用可能永久阻塞（本地模型无响应），无超时机制。修复：每个节点执行包裹 `tokio::time::timeout`，默认 300s，超时标记 `Failed` 并触发重试
- **INGEST_COOLDOWN 内存泄漏** — `HashMap<String, (u64, Instant)>` 只增不减，长期运行内存膨胀。修复：`cleanup_expired_entries()` 在每次插入时清理 24h 前条目

#### 前端体验

- **FrontstageApp mock learnings** — `setLearnings()` 硬编码 3 条假数据。修复：接入 `recordFeedback()` API，根据用户实际反馈动态生成学习提示

### 🟢 P2 优化差距修复

- **WritingStyle 更新无同步** — `update_writing_style` 修改后前端写作风格面板不刷新。修复：更新后发射 `data_refresh(story_id, "writingStyle")`
- **Remove notifyFrontstageDataRefresh** — `useStories.ts`、`useChapters.ts`、`useCharacters.ts`、`services/tauri.ts` 中废弃的 `notifyFrontstageDataRefresh` 辅助函数已移除，避免与 `useSyncStore` 重复刷新
- **Workflow 并发重复执行** — `run_instance` 无运行状态检查，同一实例可能被多个线程同时执行。修复：入口检查 `instance.status == Running` 则直接返回错误；`start_workflow_instance` 命令不再直接调用 `execute_next`，由队列自动消费
- **Retry 非幂等** — 失败节点重试时不检查是否已在 `completed_nodes` 中，可能重复执行副作用操作。修复：重试前检查 `completed_nodes.contains(&node_id)`，已完成的跳过
- **Pending vector 内存队列丢失** — `PENDING_VECTOR_INDEXES` 是进程内存 HashSet，应用重启后丢失。修复：新增 `pending_vector_indexes` SQLite 表，持久化待索引 chapter_id；vector store init 时从 SQLite 加载并批量处理
- **Task 执行无限阻塞** — `run_task_internal` 中 `executor.execute()` 可能永久阻塞。修复：包裹 `tokio::time::timeout(300s)`，超时标记失败

### 🧪 质量保障

- `cargo check` 零错误（114 dead_code warnings 来自 `#![warn(dead_code)]` 激活）
- `cargo test` 217/217 全部通过
- `npm run build` 通过

## [v5.5.1] - 设计-实现对齐全面修复 v2（2026-05-08）

### 🔴 P0 致命差距修复

#### 幕前幕后自动关联

- **state_sync 空 story_id 修复** — `update_character`/`delete_character`/`update_chapter`/`delete_chapter`/`update_scene` 共 5 处使用 `unwrap_or_default()` 获取 story_id，数据库查询失败时发射 `story_id=""` 的同步事件，前端 `if (storyId)` 判断为 falsy 导致缓存永不刷新。修复为 `if let Some(story_id)` 条件发射，确保 update/delete 后前端对应故事的数据列表自动刷新（而非全局刷新所有故事的缓存）。
- **`delete_world_building` 命令补全** — 后端新增 IPC 命令 + `WorldBuildingRepository::delete()` + 前端 `useDeleteWorldBuilding` Hook。此前只有 create/get/update，幕后无法删除世界观设定。
- **`useSyncStore` DataRefresh 缺 worldBuilding** — `dataRefresh` case 中新增 `worldBuilding` 分支，后端批量刷新世界观信号不再被前端忽略。
- **`create_scene` 额外字段更新后缺 `scene_updated`** — 创建场景时若同时提供 `dramatic_goal`/`content` 等额外字段，先 `repo.create()` 再 `repo.update()`，原代码只发射 `scene_created` 事件，前端可能读到未更新额外字段的旧数据。修复为 `has_extra` 分支后追加 `emit_scene_updated`。
- **`useSceneWithChapter` 缓存失效** — `sceneUpdated`/`sceneDeleted` handler 中追加 `['scenes', 'chapter', sceneId]` 的 invalidate/remove，确保场景-章节关联数据不 stale。
- **`App.tsx` `backstage-shown` 未用 story_id** — 监听事件时读取 payload 中的 `story_id` 并调用 `setCurrentStory`，幕后窗口重新 show 时自动定位到当前故事。

#### 后台自动化

- **Bootstrap 后台失败不可见** — `pipeline-complete` 事件原硬编码 `success: true`、`elements_created: default()`、`error_message: None`。修复为根据 `bg_executor.execute()` 实际结果设置 success/error，并从 `GenesisContext.bundle` 统计实际生成的元素数量（world_rules/characters/scenes/foreshadowings/plot_points）。前端可区分成功与失败。
- **向量存储初始化竞态** — `VECTOR_STORE` 是 `OnceCell`，应用启动后立即保存章节时若 LanceDB 尚未 init 则跳过索引，该章节永不被向量检索。修复：新增全局 `PENDING_VECTOR_INDEXES` 队列，未初始化时将 chapter_id 入队；LanceDB init 成功后自动批量处理积压队列，查询数据库→生成 embedding→写入 LanceDB。

### 🟡 P1 重要差距修复

- **Workflow Condition 节点空壳** — 原仅支持字符串 `"true"`/`"1"` 判断。修复：实现轻量级条件表达式求值，支持 `{{score}} > 0.7`、`{{status}} == "approved"` 等上下文变量比较，回退到硬编码 truthy 判断。
- **Workflow 失败实例不重试** — `run_instance` 返回 `Err` 时仅记录日志，实例永久丢失。修复：节点失败时若 `retry_count < 3`，更新状态为 `Pending` 并重新入队，发射 `workflow-instance-retried` 事件；超次后标记 `Failed`。
- **能力进化路径不一致** — `evolution.rs` 和 `mod.rs` 各有一个 `load_evolved_descriptions()`，前者从 `storage_path.parent()` 计算路径，后者从 `EVOLVED_DESCRIPTIONS_PATH` 全局路径读取，路径不一致。修复：`evolution.rs` 统一使用全局 `EVOLVED_DESCRIPTIONS_PATH`。
- **Task Cron 解析过于简化** — 原仅支持 `*/N` 和 `0 H * * *`，其他表达式静默降级为 24 小时间隔。修复：引入 `cron` crate，新增 `spawn_cron` 方法精确计算下次执行时间（`schedule.upcoming(chrono::Utc)`），替代固定间隔 ticker。
- **`cancel_genesis_pipeline` 无法中断运行中 LLM** — 取消标志只在步骤边界检查，LLM 调用期间（30-120秒）无法中断。修复：`tokio::select!` 同时运行 `step.execute()` 和取消监听循环（每 500ms 检查标志），用户点击取消后立即返回 `Cancelled` 错误。

### 🟢 P2 优化差距修复

- **文档版本号同步** — `ARCHITECTURE.md` / `AGENTS.md` / `ROADMAP.md` / `docs/FEATURES.md` 版本号更新至 `v5.5.1`
- **过时文档归档** — `docs/UPDATE_SUMMARY.md`(v3.0.0)、`docs/FIXES_2025_04_11.md`(v2.0)、`docs/NOVEL_CREATION_WORKFLOW.md`(v3.1.2)、`docs/plans/PROGRESS.md`(v3.0)、`docs/plans/ARCHITECTURE_V3_PLAN.md`(v3.0) 移至 `docs/archive/`
- **`tauri.ts` 死代码清理** — 移除 5 个无引用的 `@deprecated` 导出：`getDashboardState`、`getSkillsByCategory`、`embedChapter`、`createEntity`、`createRelation`
- **`FrontstageToolbar` 废弃组件清理** — 删除 `FrontstageToolbar.tsx` 文件及 `index.ts` 中的注释引用

### 🧪 质量保障

- `cargo check` 零错误零警告
- `cargo test` 217/217 全部通过
- `npm run build` 通过

## [v5.5.0] - 设计-实现对齐全面修复（2026-05-07）

### 🔧 架构对齐

#### 幕前幕后自动关联补全

- `create_world_building` / `update_world_building` 正确发射 `WorldBuildingUpdated` 同步事件（原错误发射 `StoryUpdated`）
- `ChapterRepository::delete` 添加事务清理 `scenes.chapter_id` 外键，消除悬空引用
- `characterDeleted` 按 `storyId` 精准失效缓存（原全局失效所有 characters）

#### 后台自动化闭环

- `auto_ingest_chapter` 成功后写入 LanceDB 向量存储：`embed_text_async` 生成 embedding → 创建 `VectorRecord` → `store.add_record()`，语义搜索可检索最新写作内容
- WorkflowEngine 支持数据库持久化：Migration 41 创建 `workflow_instances` 表，`with_pool()` 初始化时自动加载，`update_instance()` 自动保存
- 能力进化反馈环闭合：`evolve_capability_descriptions` 自动保存进化描述到 JSON；`build_default_registry()` 加载并应用已进化描述；PlanExecutor 每次执行完成后后台触发进化分析

#### 技术债务清理

- 移除 `src-core` 幽灵 crate（54 文件、15 模块，名义依赖但零引用）
- 同步 `FEATURES.md` / `ROADMAP.md` / `ARCHITECTURE.md` 版本号至 v5.4.1

### 🧪 质量保障

- `cargo check` 零错误零警告
- `cargo test` 217/217 全部通过
- `npm run build` 通过
- `cargo tauri build` Windows 安装包生成

## [v5.4.1] - Bootstrap 编辑器内容丢失修复（2026-05-07）

### 🐛 Bug修复

#### 创世流程编辑器内容丢失

- **根因**：`ConceptGenerationStep` 创建 Story 后发射 `storyCreated` 事件 → `useSyncStore` 调用 `loadStories()` → `selectStory()` → `get_story_chapters` 返回空列表（此时 `FirstChapterGenerationStep` 尚未执行）→ `setContent('')` 清空编辑器。随后 `ChapterSwitch` 事件到达时，`currentStory` 已设置走 `else` 分支，但 `chaptersRef` 为空数组找不到 chapter，不调用 `selectChapter`
- **修复1**：`FrontstageEvent::ChapterSwitch` 新增 `content` 字段，`FirstChapterGenerationStep` 直接通过事件传递生成内容到前端
- **修复2**：前端 `ChapterSwitch` 事件处理优先使用 `payload.content`，绕过 DB 查询竞态
- **修复3**：`chaptersRef` 为空时自动重新查询数据库获取最新章节
- **修复4**：`smartExecute` 返回后增加 `final_content` 兜底机制
- **修复5**：`loadStories` 在 `isGenerating=true` 时禁止自动 `selectStory`
- **文件**：`src-tauri/src/window/mod.rs`, `src-tauri/src/narrative/genesis.rs`, `src-frontend/src/frontstage/FrontstageApp.tsx`, `src-tauri/src/agents/commands.rs`

## [v5.3.1] - Bootstrap体验修复 + 幕后数据刷新（2026-05-03）

### 🐛 Bug修复

#### Bootstrap重复显示小说开头

- **根因**：`handleSmartGeneration` 在 Bootstrap 完成时设置 `generatedText`（幽灵文本），同时 `ChapterSwitch` 事件加载 `chapter.content`（正文），编辑器同时显示两份内容
- **修复**：Bootstrap 完成时不再设置 `generatedText`，内容已通过数据库保存并由 `ChapterSwitch` 事件加载到编辑器
- **文件**：`src-frontend/src/frontstage/FrontstageApp.tsx`

#### 幕后结构要素不显示

- **根因**：`useSyncStore` 中 `invalidateQueries` 的 queryKey 与 hooks 实际使用的 key 不一致：
  - `['world-building', storyId]` ≠ `['world_building', storyId]`
  - `['story-outlines', storyId]` ≠ `['story-outline', storyId]`
- **后果**：后台阶段生成数据保存到数据库并发射 `sync-event` 刷新事件，但 TanStack Query 缓存永不过期，幕后永远显示空数据
- **修复**：统一 `useSyncStore.ts` 中的 KEYS 为 hooks 实际使用的 queryKey
- **文件**：`src-frontend/src/hooks/useSyncStore.ts`

#### Bootstrap解析失败：missing field `id`

- **根因**：`ConceptGenerationStep` 中 LLM 返回的 JSON 缺少 `id`/`story_id`/`source` 等后端生成字段，`serde_json::from_str::<StoryMetaElement>()` 反序列化失败
- **修复**：给所有 `NarrativeElement` 结构体的 `id`/`story_id`/`source`/`source_ref_id`/`status` 字段添加 `#[serde(default)]`，允许 LLM 返回的 JSON 省略这些字段
- **文件**：`src-tauri/src/narrative/elements.rs`

#### Bootstrap生成中断：幕前无正文 + 幕后无结构要素

- **根因1**：`StoryContextBuilder::build` 中 `fetch_characters`/`fetch_previous_scenes`/`fetch_writing_style` 在 Bootstrap 时数据库为空返回 `Err`，导致 `FirstChapterGenerationStep` 失败，第一章无法生成
- **修复1**：`build` 方法中这些查询失败时返回默认值（`vec![]`/`None`）而非传播错误
- **根因2**：LLM 返回的角色/场景/世界观/大纲 JSON 可能缺少 `relationships`/`rules`/`key_locations`/`power_system`/`total_scenes_estimate`/`key_plot_points`/`estimated_scenes` 等字段，后台阶段反序列化失败中断
- **修复2**：给所有可能缺失的字段添加 `#[serde(default)]`
- **文件**：`src-tauri/src/creative_engine/context_builder.rs`、`src-tauri/src/narrative/elements.rs`

#### 续写时重复生成小说开头

- **根因**：`current_content_preview` 从**头部截断 2000 字符**，第一次续写后总字数超过 2000，LLM 只能看到第一章内容，看不到续写内容，于是重新生成开头
- **修复**：改为从**尾部截断 6000 字符**（保留最新内容），并标注省略字数，LLM 能看到最近的续写内容并在此基础上继续
- **文件**：`src-tauri/src/lib.rs`

#### 其他

- 移除 `state_sync/mod.rs` 未使用的 `SyncEvent` 导入
- `lib.rs`：后台阶段完成后通过 `StateSync::emit_data_refresh()` 发射标准 `sync-event` 事件

### 编译与测试

- `cargo check`：零错误
- `cargo test`：193/193 全部通过
- `npm run build`：通过
- `cargo tauri build`：Windows `.exe` / `.msi` / `-setup.exe` 生成成功

---

## [v5.3.0] - 叙事元素模型重构：创世-拆书同构架构（2026-05-02）

### 🏗️ 架构级重构：统一叙事元素模型

核心理念：无论正向生成（Bootstrap/创世）还是逆向分析（拆书），操作的叙事元素是同一套抽象。

#### Phase 1: 统一数据模型

- **新建 `src-tauri/src/narrative/` 模块**（8个文件）：
  - `elements.rs` — `CharacterElement/SceneElement/WorldBuildingElement/OutlineElement/ForeshadowingElement/StoryMetaElement` + `ElementSource` 枚举
  - `pipeline.rs` — `NarrativePipelineExecutor` + `PipelineStep` trait
  - `progress.rs` — 统一 `PipelineProgressEvent` 替代两套进度系统
  - `prompts.rs` — 共享 Prompt 模板（Generate/Extract 双模式）
  - `genesis.rs` — **GenesisPipeline** 7步正向流程
  - `analysis.rs` — **AnalysisPipeline** 7步逆向流程（含新增伏笔提取、知识图谱构建）
- **Migration 38**: `narrative_characters/scenes/world_buildings/outlines/foreshadowings/character_relationships` 统一表

#### Phase 2: Pipeline 框架切换

- `smart_execute` 已切换到 `GenesisPipeline`
- 拆书 `executor.rs` 已切换到 `AnalysisPipeline`
- 向后兼容：同时发射 `pipeline-progress`（新）和旧事件

#### Phase 3: 统一进度系统

- 前端新建 `usePipelineProgress.ts` Hook
- `AnalysisProgress.tsx` 和 `FrontstageApp.tsx` 已接入统一进度

#### Phase 4: 统一存储层

- `repositories_narrative.rs` — `NarrativeCharacterRepository`, `NarrativeSceneRepository`, `NarrativeWorldBuildingRepository`
- 生产表和参考表数据最终都汇聚到统一表中

#### Phase 5: 故事→分析功能

- **`StoryHealthAnalyzer`** — 6 维度结构健康检查：
  - 伏笔回收率、角色弧光完整度、冲突类型多样性
  - 大纲覆盖率、世界观完整度、角色关系网络密度
- **`analyze_story_structure`** IPC 命令 — 前端可调用分析已有故事
- `HealthReport` / `HealthCheck` / `HealthStatus` — 完整报告结构

#### 附带修复

- `audit.rs` `ForeshadowingTracker` 导入路径修复（`get_by_story` → `get_all`）
- `audit.rs` `ForeshadowingRecord` 字段访问修复（`is_paid_off` → `matches!(status, Payoff)`）

### 编译与测试

- `cargo check`：零错误（1 个已有警告 `unused import: events::SyncEvent`）
- `cargo test`：193/193 全部通过
- `npm run build`：通过
- `cargo tauri build`：Windows `.exe` / `.msi` / `-setup.exe` 生成成功

---

## [v5.2.0] - 设计-实现对齐全面完成（2026-05-02）

### 🎯 P0 核心差距修复

#### 通用 Workflow 引擎节点执行器实现

- **`WorkflowScheduler::run_instance` 从空实现到完整 DAG 执行**：支持 Start → WriteChapter → Inspect → Revise → VectorIndex → AnalyzePlot → End 全节点类型
- **节点执行映射**：WriteChapter/Revise → Writer Agent、Inspect → Inspector Agent、AnalyzePlot → PlotAnalyzer、VectorIndex → IngestPipeline
- **串行拓扑执行**：按 DAG 依赖关系遍历，状态管理（Pending → Running → Completed/Failed），上下文变量传递
- **进度事件**：`workflow-started` / `workflow-node-started` / `workflow-node-completed` / `workflow-node-failed` / `workflow-completed`
- **IPC 命令**：`register_workflow` / `create_workflow_instance` / `start_workflow_instance` / `get_workflow_instance_status`
- **注册标准模板**：`standard_writing_workflow` (Write → Inspect → Index) 在 setup 时自动注册

#### 能力进化反馈环闭合

- **`ExecutionRecordStore` JSON 持久化**：`app_data_dir/capability_execution_records.json`，自动保留最近 500 条记录
- **`record_execution` 真正持久化**：`PlanExecutor::execute_step` 每次能力执行后自动记录（capability_id / success / duration）
- **`evolve_capability_descriptions` LLM 分析**：查询执行历史 → 计算成功率 → LLM 生成改进后的 `when_to_use` 描述
- **统计查询**：`get_statistics()` 按能力汇总成功/失败次数

#### 幕前↔场景内容双向同步

- **useSyncStore chapterUpdated → scenes 刷新**：`chapterUpdated` 事件处理中新增 `invalidateQueries(['scenes', storyId])`，因为 chapter 更新会同步到 scene
- **FrontstageApp 监听 chapter-updated**：当当前编辑的 chapter 被幕后更新时，自动刷新编辑器内容（3 秒防循环保护）
- **数据库双向同步已验证**：`ChapterRepository::update` 同步到 scene，`SceneRepository::update` 同步到 chapter

### 🎯 P1 差距修复

#### 废弃组件清理

- **`FrontstageToolbar` 从索引移除**：`frontstage/components/index.ts` 中不再导出，组件文件保留供参考

#### QueryPipeline 降级感知

- **后端 `context-degraded` 事件**：`build_agent_context` 中 `StoryContextBuilder` 降级到 `minimal` 时发射事件
- **前端 toast 提示**：`FrontstageApp` 监听 `context-degraded`，显示 "正在使用简化上下文生成内容..."

### 编译与测试

- `cargo check`：零错误（1 个已有警告 `unused import: events::SyncEvent`）
- `cargo test`：193/193 全部通过
- `npm run build`：通过

---

## [v5.2.2] - Bootstrap两阶段架构重构：先出正文，后台完善（2026-05-02）

### 🏗️ 架构级重构

#### Bootstrap 两阶段执行模型（核心体验优化）

- **即时阶段**（同步，2-3分钟）：生成故事概念 + 第一章正文 → 立即返回给前端，用户可以开始写作
- **后台阶段**（异步，`tokio::spawn`，5-8分钟）：世界观 → 大纲 → 角色 → 场景 → 伏笔 → 知识图谱
- **用户等待时间**：从 10+ 分钟缩短到 **2-3 分钟**
- **实现**：`bootstrap.rs` `run()` 拆分为 `run_quick_phase()` + `run_background_phase()`；`lib.rs` 调用 `run()` 后，后台任务在 spawn 中继续执行

#### 前端体验优化

- Bootstrap 即时完成后显示："小说已创建！第一章已生成，您可以开始写作了"
- 后台阶段进行中状态栏显示："后台正在完善小说世界..."
- 后台全部完成后 toast："创世完成！世界观、角色、场景、伏笔已全部生成"
- `novel-bootstrap-progress` 事件处理区分"即时完成"和"后台完成"

### 编译与测试

- `cargo check`：零错误（1 个已有警告 `unused import: events::SyncEvent`）
- `cargo test`：193/193 全部通过
- `npm run build`：通过

---

## [v5.2.1] - 超时修复与白屏修复（2026-05-02）

### 🐛 Bug 修复

#### 小说创建超时修复

- **Bootstrap 超时延长**：前端 `handleSmartGeneration` 中创建新小说超时从 180 秒延长至 **600 秒**（10 分钟），匹配本地大模型多步 LLM 调用实际耗时
- **超时提示优化**：超时错误信息区分 Bootstrap 与普通操作，引导用户检查模型服务
- **进度事件密度增强**：`bootstrap.rs` 在 `generate_first_chapter`、`generate_world_building`、`generate_story_outline`、`generate_characters`、`generate_scene_outline` 等每个 LLM 调用前后增加进度事件，用户可实时看到"正在调用AI..."→"已生成，正在解析..."的细粒度状态
- **LLM 心跳频率加快**：`llm/service.rs` 心跳间隔从 3 秒缩短至 **2 秒**，心跳上限从 40 次扩展到 300 次（匹配 600 秒超时），消息优化为"正在深度思考中..."
- **Bootstrap 进度提示细化**：各步骤提示增加预计耗时说明，如"（1500-2500字，可能需要1-3分钟）"、"（8-12个核心场景）"

#### 后台窗口白屏修复（v5.2.0 增强版）

- **双重维度尺寸微调**：`show_backstage` 中不仅微调 width，还微调 height（width+1/height+1 → 恢复），更全面地触发 WebView2 重绘
- **JS 重排增强**：`document.documentElement` 和 `document.body` 双重强制重排，额外触发 scroll 事件和自定义 `backstage-window-restored` 事件
- **延迟时间延长**：`backstage-shown` 事件发射延迟从 300ms 延长至 **800ms**，给 WebView2 充足时间从休眠恢复；延迟期间再次执行尺寸微调
- **前端刷新增强**：`App.tsx` `handleWindowShown` 后调用 `forceRedraw()`：立即 + 300ms 延迟两次触发 `setRenderKey`，确保 React 重新挂载
- **前端监听恢复事件**：新增 `backstage-window-restored` DOM 事件监听，双重保险触发重绘

### 编译与测试

- `cargo check`：零错误（1 个已有警告 `unused import: events::SyncEvent`）
- `cargo test`：193/193 全部通过
- `npm run build`：通过

---

## [v5.1.1] - 设计-实现对齐全面修复（2026-05-01）

### 🎯 P0 核心断裂修复

- **`update_chapter` 保存后自动触发 IngestPipeline**：`lib.rs` 中 `update_chapter` 命令成功后 `tokio::spawn` 异步调用 `auto_ingest_chapter()`，知识图谱实时更新
- **`create_chapter` Ingest 固化触发**：在 `AfterChapterSave` skill hook 之外**硬编码**触发 Ingest，确保无论 skills 配置如何，知识图谱必定更新
- **`state_sync` 空 story_id 修复**：`update_character` / `delete_character` / `update_chapter` / `delete_chapter` 在发射同步事件前先查询对应的 `story_id`，`useSyncStore` 可精准刷新缓存
- **`FrontstageToolbar` story_id 传递**：废弃组件 `FrontstageToolbar.tsx` 新增 `storyId` prop，`show_backstage` 调用正确传递 `story_id`

### 🎯 后台自动化修复

- **`WorkflowScheduler::schedule_execution` 队列机制**：从空实现（仅 log）改为真正的内存队列（`VecDeque`），`execute_next()` 支持串行执行工作流实例

### 🎯 代码审查修复

- **LLM 5 分钟冷却期 + 内容哈希去重**：`auto_ingest_chapter` 内置 `INGEST_COOLDOWN` 全局状态，相同内容或 5 分钟内重复保存跳过 Ingest，防止 API 成本失控
- **未使用导入清理**：`FrontstageToolbar.tsx` 删除 `Sparkles`、`Settings`；`workflow/scheduler.rs` 删除 `Workflow`、`NodeType`
- **`WorkflowScheduler::run_instance` 明确错误**：返回 `Err("Workflow node execution is not yet implemented")` 而非空 `Ok(())`

### 📦 基础设施

- **`PromptLibrary` 扩展**：新增 `style_checker_system_template()` + `commentator_system_template()`
- **`prompts/methodologies/` 方法论模板库**：雪花法 10 步 (`snowflake.rs`) + 英雄之旅 12 阶段 (`hero_journey.rs`) + 场景结构 3 变体 (`scene_structure.rs`)

### 编译与测试

- `cargo check`：零错误
- `cargo test`：193/193 全部通过
- `npm run build`：通过

---

## [v5.1.0] - 幕前幕后自动关联对齐（2026-05-01）

### 🎯 幕前幕后自动关联

- **Chapter↔Scene 双向映射**：Migration 37 新增 `chapters.scene_id` + `scenes.chapter_id` 外键关联，`ChapterRepository::create` 事务内自动查找/创建关联 Scene
- **统一实时状态中心**：后端 `state_sync` 模块（`events.rs` + `service.rs` + `mod.rs`），定义 16 种 `SyncEvent`，所有数据修改命令完成后自动发射同步事件到 `sync-event` 频道
- **前端 useSyncStore Hook**：监听 `sync-event`，根据事件类型自动 `invalidateQueries` / `removeQueries`，实现前后台数据零延迟对齐
- **Bootstrap 完成后幕前自动加载**：`smartExecute` 返回后检测 `story_created:` 消息自动加载新故事并切换第一章；Bootstrap 完成后双重 `ChapterSwitch` 保险
- **幕前→幕后快速跳转**：`Ctrl+Shift+B` 快捷键，标题栏点击，`show_backstage` 接收 `story_id` 参数，幕后自动定位当前故事

### 🎯 后台自动化对齐

- **AgentOrchestrator 闭环接入**：`execute_writer` 集成 `AgentOrchestrator::execute_write_with_inspection`，Writer→Inspector→StyleChecker→Writer 自动质检改写生效；修复递归 async fn 调用（`Box::pin`）
- **自适应学习闭环激活**：`AdaptiveLearningEngine::record_feedback` 成功后 `std::thread::spawn` 异步触发 `mine_preferences`，偏好挖掘自动运行

### 🎯 状态管理与数据流优化

- **Zustand↔TanStack Query 同步**：`App.tsx` 使用 `useAppStore` 订阅 `currentStory`，`useEffect` 监听变化自动刷新关联数据缓存
- **窗口通信事件标准化**：`DataRefresh` 统一由 `useSyncStore` 处理，移除 `backstage-update` 和 `handleWindowShown` 中的重复 `invalidateQueries`

### 编译与测试

- `cargo check`：零错误
- `cargo test`：193/193 全部通过
- `npm run build`：通过

## [v5.0.0] - 创世引擎：一键创世，万物关联（2026-04-30）

### 🎯 创世引擎 (Genesis Engine)

- **一键生成完整小说世界**：输入"写一部都市玄幻小说"，系统自动生成故事概念、第一章正文、完整大纲、主要角色及性格小传、场景规划、伏笔埋设
- **7步创世工作流**：构思故事 → 撰写开篇 → 构建世界 → 生成大纲 → 塑造角色 → 铺设场景 → 埋设伏笔 → 编织关联
- **自动幕后卡片创建**：所有生成内容自动在幕后对应栏目创建卡片，无需手动操作

### 🎯 故事大纲系统

- **新增 `story_outlines` 表**：存储完整故事大纲（Markdown + 结构化 JSON）
- **3幕结构自动生成**：每幕含标题、摘要、关键情节点、预估场景数
- **前端故事概览面板**：Stories 页面新增"概览"视图，展示大纲、角色、场景、伏笔总览

### 🎯 角色系统增强

- **完整性格小传入库**：`characters` 表新增 `appearance`/`gender`/`age` 字段
- **角色关系图谱**：新增 `character_relationships` 表，记录角色间关系（朋友/敌人/恋人/师徒等）
- **前端关系视图**：Characters 页面新增"关系"标签页，展示角色关联网络

### 🎯 伏笔自动生成

- **Bootstrap 自动埋设伏笔**：基于故事大纲识别 3-5 个核心伏笔
- **伏笔与场景自动关联**：第一个伏笔自动关联到第一章场景
- **创世标记**：自动生成的伏笔显示"创世"金色徽章

### 🎯 知识图谱自动构建

- **创世时自动创建 KG 实体**：角色 → Character、场景 → Event、伏笔 → PlotDevice
- **自动关系连接**：角色参与场景、伏笔设置于场景

### 🎯 前后台智能联动

- **Bootstrap 完成后自动导航**：幕后界面自动切换到 Stories 并高亮新故事
- **故事概览自动展开**：新故事"概览"面板自动打开
- **实时卡片创建事件**：新增 `novel-bootstrap-card-created` 事件，前端实时显示卡片创建进度

### 🐛 Bug 修复（v5.0.0 热修复 v3）

- **后台窗口白屏修复**：修复后台窗口隐藏后重新显示时出现空白/白屏的问题
  - **根因 v3**：WebView2 窗口 `hide()` 后重新 `show()` 时渲染表面丢失；JS 强制重排不够可靠
  - **修复 v3**：`show_backstage` 命令**微调窗口大小再恢复**（`width+1` → `width`），强制 WebView2 重新创建渲染表面；配合 JS 强制重排；延迟 300ms 发射 `backstage-shown` 事件确保前端监听器就绪
- **后台卡片显示修复**：修复 Bootstrap 小说创建后，后台不显示生成的卡片（故事大纲、完整角色传记、场景、伏笔）的问题
  - **根因 v3**：（1）Bootstrap 完成时后台窗口被隐藏，事件丢失；（2）`DataLoader` 与 `App.tsx` 同时加载 stories 造成**竞态条件**；（3）Bootstrap LLM 调用失败时错误被 `log::warn` 吞掉，前端完全不可见
  - **修复 v3**：（1）`DataLoader` **移除 stories 查询**，完全由 `App.tsx` 控制数据加载，消除竞态；（2）`App.tsx` 引入 `useQueryClient`，`handleWindowShown` 中主动 `invalidateQueries` 强制刷新角色/场景/伏笔/大纲等所有页面数据；（3）`bootstrap.rs` LLM 调用失败时发射 `novel-bootstrap-error` 事件到前端，让错误可见

### 🎯 数据库迁移

- **Migration 34**: `story_outlines` 表
- **Migration 35**: `characters` 增强 + `character_relationships` 表
- **Migration 36**: `scenes.foreshadowing_ids` 字段

### 📊 统计

- Rust 测试：193/193 全部通过
- 前端构建：npm run build 通过
- 新增后端模块：StoryOutlineRepository、CharacterRelationshipRepository
- 新增前端组件：StoryOverview、CharacterGrid、SceneTimelineMini、ForeshadowingListMini

---

## [v4.5.0] - 多账号认证与云端主站（2026-04-28）

### 🎯 多账号 OAuth 登录系统

- **桌面端 OAuth2 登录**：支持 Google / GitHub OAuth2 登录，PKCE + Authorization Code 流程
- **可选登录、本地优先**：不登录可正常使用所有功能，登录后解锁未来云同步能力
- **微信/QQ 预留框架**：OAuth URL 和类型已定义，二期补充具体实现
- **数据层**：`users` / `oauth_accounts` / `sessions` 表 + UserRepository 持久化
- **JWT Session 管理**：`jsonwebtoken` 签发/验证，7 天有效期

### 🎯 云端主站（Linux 服务端）

- **Actix-web 后端**：RESTful API，PostgreSQL 持久化，JWT 中间件认证
- **Web 前端**：Vite + React + Tailwind CSS，落地页 / 登录页 / 用户后台
- **Docker 部署**：`docker-compose.yml` + `.env.example` + `deploy.sh`，一键部署
- **数据库迁移**：`src-server/migrations/` 完整表结构（users / oauth_accounts / sessions / stories）

### 🎯 Bug 修复

- **API KEY 保存**：重写 `update_model` 为直接字段修改（取代 delete+create 模式），避免密钥在多次读写配置时丢失
- **前端密钥逻辑**：编辑模型时，用户输入非空值才更新 API Key，未输入则保留旧值
- **LLM 流式生成超时**：`generate_stream` 添加 30 秒启动超时 + 15 秒 chunk 超时，防止服务器挂起导致无响应
- **LLM 同步生成超时**：`generate` 添加 60 秒整体超时

### 🎯 构建与部署

- **Rust 升级**：1.85.0 → 1.95.0（MSVC toolchain）
- **oauth2 v5.0 兼容**：修复 Breaking API 变化（类型状态模式 builder）
- **GitHub Actions**：全平台构建触发
- **本地构建**：Windows `.exe` + `.msi` + `-setup.exe` 已生成

## [v4.4.0] - 3风格三角框架：通用风格混合系统（2026-04-28）

### 🎯 通用风格混合系统（StyleBlend）

- **新增 `StyleBlendConfig` + `BlendComponent`**：支持任意 2-5 个 StyleDNA 按权重组合，不绑定固定三角
- **主导/辅助角色自动分配**：权重 >= 50% → Dominant，>= 20% → Secondary，其余 Tertiary
- **权重归一化**：拖动滑块自动调整，总和始终为 100%
- **验证机制**：主导风格必须存在，最多 5 个风格，权重总和必须为 1.0

### 🎯 3风格三角创作框架

- **新增内置风格 DNA**：普鲁斯特（意识流/长句/内心独白 70%）+ 马尔克斯（魔幻现实/全知视角/循环时间）
- **海明威风格已存在**：极简/短句/对话驱动，avg_sentence_length=15
- **三角示例**：普鲁斯特 65% + 海明威 20% + 马尔克斯 15% = 心理深度 + 节奏对话 + 氛围哲理的有机融合

### 🎯 混合风格 Prompt 注入

- **主导风格完整注入**：Writer prompt 中注入完整 StyleDNA.to_prompt_extension()
- **辅助风格差异注入**：仅注入与主导风格的关键差异维度（句长/对话比/比喻密度/内心独白/情感外露）
- **融合规则**：主导定基调，辅助在特定场景渗透；冲突时以主导为准，辅助渗透"精神"而非"形式"
- **PlanGenerator Rule 20**：模型必须遵循混合权重，主动判断当前场景适合哪种风格元素主导

### 🎯 防漂移自检清单（5项检查）

- **新增 `StyleDriftChecker`**：每章生成后自动运行风格匹配度检查
- 1. 句长检查：加权平均 ± 30% 容差
- 2. 对话比例检查：加权平均 ± 15% 容差
- 3. 比喻密度检查：加权平均 ± 50% 相对容差
- 4. 内心独白比例检查：加权平均 ± 20% 容差
- 5. 情感外露检查：加权平均情感词密度 ± 30% 容差
- **评分机制**：每项 0.0-1.0，总体 >= 0.7 且单项全部通过才算合格

### 🎯 数据层扩展

- **Migration 30**：`story_style_configs` 表（story_id + blend_json + is_active）
- **Migration 31**：`scenes` 表新增 `style_blend_override` 字段，支持章节级风格覆盖
- **新增 `StoryStyleConfigRepository`**：CRUD + set_active 激活配置

### 🎯 前端 UI 升级

- **Stories.tsx 风格配置面板**："单一风格" / "风格混合" 双标签页
- **`StyleBlendPanel` 组件**：添加/移除风格、权重滑块、实时归一化、验证提示
- **新增 IPC 命令**：`get_story_style_blend` / `set_story_style_blend` / `update_scene_style_blend` / `check_style_drift`
- **向后兼容**：保留 `style_dna_id` 单一风格选择，混合配置优先于单一风格

### 测试

- Rust 测试：193/193 全部通过（新增 blend 4 项 + drift_checker 3 项 + classic_styles 2 项）
- 前端构建：npm run build 通过

## [v4.0.0] - 借鉴 AI-Novel-Writing-Assistant 全面优化（2026-04-22）

### 🎯 Canonical State 规范状态系统

- 新增 `canonical_state/` 后端模块，`CanonicalStateManager` 实时聚合 stories/scenes/characters/KG/foreshadowing 分散状态
- 定义 `CanonicalStateSnapshot`：story_context（当前场景/开放冲突/待兑现伏笔/逾期伏笔）、character_states、world_facts、timeline、narrative_phase
- `build_agent_context` 优先使用 Canonical State 构建上下文，AI 续写时准确知道"当前处于故事哪个阶段"
- 新增 `get_canonical_state` IPC 命令，8 个单元测试

### 🎯 Payoff Ledger 伏笔账本

- Migration 24 扩展 `foreshadowing_tracker` 表：target_start_scene / target_end_scene / risk_signals / scope_type / ledger_key
- 新增 `PayoffLedger` 后端模块：逾期检测（基于重要性动态阈值）、回收时机智能推荐（高潮阶段自动提升 urgency）
- 前端 `Foreshadowing.tsx` 升级为 Ledger 视图：生命周期时间轴、逾期告警横幅、回收推荐卡片
- 新增 4 个 IPC 命令 + 3 个前端 Hook

### 🎯 Execution Panel 章节执行面板

- 新增 `ExecutionPanel.tsx` 前端组件，智能推荐下一步行动（处理逾期伏笔 / 续写 / 运行审校）
- 集成到 `Scenes.tsx` 右侧栏（三栏布局）和 `FrontstageApp` 标题栏（「下一步」快捷按钮）
- 根据叙事阶段、逾期伏笔、场景置信度动态调整推荐

### 🎯 Narrative Phase Detection 叙事阶段检测

- 增强 `calculate_narrative_phase`：逾期伏笔→ConflictActive、最近3场景高置信长内容→Climax、主要伏笔回收+场景数≥50→Resolution
- 各阶段返回 `writer_guidance()` 指导语，注入 Writer Agent prompt
- 前端 `StoryTimeline.tsx` 场景节点旁标注阶段标签（蓝/琥珀/红/绿）

### 🎯 Structured Outline 结构化大纲

- Migration 25 扩展 `scenes` 表：execution_stage / outline_content / draft_content
- `SceneEditor` 重写为 6 标签页：规划 / 大纲 / 起草 / 审校 / 定稿 / 批注
- 阶段间流转按钮：生成大纲 → 根据大纲起草 → 提升为定稿
- 新增 `generate_scene_outline` / `generate_scene_draft` IPC 命令

### 🎯 Audit System 审计系统

- 新增 `audit/` 后端模块，整合 ContinuityEngine / StyleChecker / QualityChecker / PayoffLedger
- 五维评分：continuity / character / style / pacing / payoff，0-1 分制
- 支持 light（规则快速检查）和 full（+ LLM 深度评估）两种审计模式
- 智能升降级：字数 < 200 或 > 5000 自动触发完整审计
- 前端 SceneEditor「审校」Tab 展示五维进度条 + issue 列表 + 修复建议

### 🎯 Novel Creation Wizard 小说创建向导

- 新增 `CreationWizard.tsx` 页面，5 步向导：创意输入 → 世界观选择 → 角色谱选择 → 文风选择 → 首个场景生成
- 每步调用已有 IPC（generate_world_building_options / generate_character_profiles 等）
- 右侧汇总栏显示所有选择，可点击跳转修改
- Stories.tsx「AI 一键创作」按钮改为二级菜单：快速创作 / 向导创作

### 🎯 Enhanced Streaming 增强流式输出

- 新增 `StreamOutput.tsx` 组件：Markdown 渲染、实时字数统计、停止生成按钮、打字机效果、复制/全屏
- 支持 simulated 模式（前端打字机）和 real 模式（后端真实流式）
- 接入 FrontstageApp AI 续写面板、WenSiPanel 自动修改结果、CreationWizard 场景生成

### 🎯 Strategy Configuration 写作策略配置

- Settings.tsx 新增「写作策略」卡片：运行模式（快速/精修）、冲突强度（0-100）、叙事节奏（慢/均衡/快）、AI 自由度（低/中/高）
- `AppConfig` 扩展 `WritingStrategy`，`build_writer_prompt` 根据策略动态注入 prompt 约束
- 冲突强度≥80 → "每 500 字至少一次冲突"；pace=fast → "减少环境描写，增加动作"

### 📊 统计

- Rust 测试：160/160 全部通过
- 新增 Migration：24 / 25
- 新增后端模块：canonical_state / audit / payoff_ledger
- 新增前端页面：CreationWizard.tsx / ExecutionPanel.tsx / StreamOutput.tsx

## [v4.1.0] - 幕前界面深度重构：化整为零，萤火随行（2026-04-22）

> **设计理念**：从 20+ 可见 UI 元素缩减至 <5 持久元素。AI 功能以萤火暗示（firefly hints）形式按需浮现，用完即隐。

### P0 核心重构

- **顶栏精简**：44px 细线设计。小说标题（点击进入幕后）、章节信息、字数/总字数/字号、🔥 文思三态切换（`off·` / `passive✨` / `active🔥`）、禅模式按钮。移除：汉堡菜单、订阅徽章、"开启文思"按钮、"AI 续写"按钮、主行动按钮。
- **底栏删除**：彻底删除底部聊天工具栏（chat input、模型状态点、WenSiPanel 嵌入、Slash textarea 菜单）。AI 生成结果以幽灵文本内联呈现，Tab 接受 / Esc 拒绝。
- **侧边栏精简**：5 按钮 → 3 按钮（修/批/幕）。"修"=修订模式切换，"批"=生成古典评点，"幕"=进入幕后。
- **键盘快捷键**：`Ctrl+Enter` / `Cmd+Enter` 全局触发续写，`Ctrl+Space` 循环文思模式，`F11` 禅模式。

### P1 萤火系统

- **幽灵文本**：编辑器末尾灰色斜体段落（`opacity: 0.35`），附带萤火操作栏（Tab 接受 / Esc 拒绝）。
- **右边缘萤火**：`smartGhostText` 从编辑区右边缘淡入（0.8s）→ 停留 → 淡出（1.2s），不打扰写作流。
- **空态引导**：编辑器无内容时居中显示诗意提示"开始写下第一句话，文思将随你而行 / 按 / 查看可用命令"。

### P2 体验优化

- **内联 `/` 命令菜单**：光标处触发，8 命令——续写/润色/古风/场景/自动续写/审校/评点/排版。方向键导航，回车执行，Esc 关闭，自动删除 `/` 字符。
- **WenSiPanel 浮动化**：从底栏嵌入改为 FrontstageApp 右下角浮动卡片，通过 `/` 菜单高级命令（auto_write/auto_revise）触发。
- **修订横幅精简**：从多行可展开缩减为 32px 单行，变更列表可滚动，默认折叠。
- **古典评点保留**：AI 生成的段落评点（金圣叹式朱批）保留为内联段落，朱红色 `oklch(55% 0.18 25)`，`LXGW WenKai` 字体，左边框红色，`※` 前缀，缩进 3em。通过 `/` 菜单、sidebar "批"按钮或右键菜单触发。

### 🗑️ 移除（设计决策）

- **显式注释/评论系统**：sidebar "注"按钮、注释/评论面板、选中文本弹窗创建按钮、右键菜单注释项、所有相关 hooks（`useTextAnnotations`、`useCommentThreads`）。
- **原因**：AI 写作工具不需要创作者标注自己的作品；AI 反馈应以幽灵文本或古典评点形式自然呈现。

### 📊 统计

- Rust 测试：160/160 全部通过
- 前端构建：通过
- 修改文件：Rust 0 个 + 前端 8 个（FrontstageApp / RichTextEditor / EditorContextMenu / frontstage.css / useTextAnnotations / useCommentThreads / hooks/index.ts 导出清理）
- 删除代码：约 800 行（底栏、注释系统、评论面板）
- 设计原则："化整为零，萤火随行" — 从显性 UI 到隐性 AI

## [v4.0.1] - 全面代码审计与空实现修复（2026-04-22）

### Phase A: 代码审计与 P0 修复

- **综合代码审计**: 扫描 40+ 模块，识别 5 项严重问题、17 项参数不匹配、9 项空实现，输出 `CODE_AUDIT_REPORT_V4.md`
- **IPC 参数统一**: 修复 17 处 camelCase→snake_case 参数名（`services/tauri.ts` 7 处、`settings.ts` 2 处、`useBookDeconstruction.ts` 6 处、`FrontstageApp.tsx` 4 处），消除 Tauri v2 反序列化静默失败
- **空实现补全**:
  - `analytics/mod.rs`: 真实写作统计（streak/longest/productivity/avg words 从 chapter 日期计算）
  - `agents/commands.rs`: `agent_get_status` 查询 `TASK_HANDLES` 返回真实状态
  - `skills/executor.rs`: `execute_mcp` 异步连接真实 `McpClient` 并调用工具
  - `export/mod.rs`: `import_from_text` 正则解析章节（"第X章"/"Chapter X"）
  - `workflow/scheduler.rs`: 添加执行日志记录
  - `evolution/updater.rs`: `apply_update` 实现 manifest 字段 CRUD
  - `mcp/server.rs`: 修复 `execute_tool` 缺失 `.await`
- **前端修复**:
  - `services/settings.ts`: 移除硬编码浏览器 fallback API keys/内部 IPs
  - `hooks/useCollaboration.ts`: WebSocket 实例保存到 ref，实现 `sendOperation`/`sendCursorPosition`
  - `hooks/useStreamingGeneration.ts`: 生产环境移除 `mockStreamGeneration`
  - `frontstage/ai-perception/textAnalyzer.ts`: 实现 `analyzeRecent` 增量分析逻辑
- **UI 调整**: 底部聊天工具栏从 `absolute bottom-0` 改为正常 flex 流，`ProseMirror` padding-bottom 从 `10rem` 降至 `3rem`
- **类型统一**: `skills/mod.rs` 移除重复 `McpServerConfig`，复用 `crate::mcp::types::McpServerConfig`

### Phase B: 内存模块 SQLite 持久化

- **Migration 26**: `chat_sessions` + `chat_messages` 表，支持聊天记录持久化
- **Migration 27**: `story_runtime_states` 表，支持故事运行状态持久化
- **Migration 28**: `collab_sessions` + `collab_participants` 表，支持协作会话持久化
- `chat/mod.rs`: `ChatManager` 从内存 `HashMap` 改为 `DbPool` 持久化
- `state/manager.rs`: `StoryStateManager` 从内存 `HashMap` 改为 `DbPool` 持久化
- `collab/mod.rs`: `CollabManager` 从内存 `HashMap` 改为 `DbPool` 持久化
- `collab/websocket.rs`: 完整实现 Operation/Cursor/Leave/Participants 消息处理，修复 user_id 硬编码，WebSocketServer 支持 `with_pool`

### 📊 统计

- Rust 测试：160/160 全部通过
- 前端构建：通过
- 新增 Migration：26 / 27 / 28
- 修复文件：Rust 12 个 + 前端 10 个

## [v3.7.1] - 智能化创作系统 5 阶段重构深度修复（2026-04-22）

### Phase A: P0 核心断裂修复（5 项）

- QueryPipeline `graph_expansion` 内容分词后逐 token 匹配实体，修复图谱扩展永不命中
- QueryPipeline `budget_control` 修复内层 break 只跳出内层循环的预算泄漏
- ContinuityEngine `check_world_rules` 修复检查方向（提取禁止条款后检测）
- ContinuityEngine `get_character_states` 效率优化 O(N×M)→O(N+M)
- PreferenceMiner `record_feedback` 成功后异步触发 `mine_preferences`，自适应学习闭环激活
- StyleChecker 接入 `AgentOrchestrator` 闭环，Writer→Inspector→StyleChecker→Writer
- Ingestion 实现真正的内容保存 + 简化知识图谱实体提取

### Phase B: P1 功能补全（6 项）

- 方法论：Migration 22 添加 methodology_id/methodology_step，Settings 新增创作方法论配置
- 创作模式：`CreationWorkflowEngine` 按 CreationMode 分支（AI全自动/AI初稿+精修/人工初稿+润色）
- 进度反馈：`useWorkflowProgress` Hook + Stories.tsx 进度弹窗
- Orchestrator 事件：前端监听 `orchestrator-step` 实时状态，Settings 暴露阈值/循环数配置
- AdaptiveGenerator `calculate_temperature` 累加而非覆盖
- 反馈记录：AiSuggestionNode + WenSiPanel 接入 `record_feedback`

### Phase C: P2 优化（4 项）

- StyleAnalyzer 新增 `analyze_with_llm` + `analyze_style_sample` IPC
- QualityChecker 新增 `check_with_llm`，Review 阶段优先 LLM 评估
- PhaseWorkflow 硬编码阶段逻辑迁移到配置驱动
- 增量 Context：每阶段完成后关键产出回注 `AgentContext`

## [v3.6.1] - 全面功能审计与深度修复（2026-04-22）

### P0 紧急修复（10 项）

- DB: Migration 21 补全 scenes/kg_relations `confidence_score` 缺失列
- IPC: 统一 25 处 camelCase→snake_case 参数名
- 场景: `create_scene` 后端扩展参数
- Orchestrator: 修复 Rewrite 事件错误携带初稿分数
- 技能: `execute_skill` 注入真实 StoryContext，SkillExecutor 实现真正 LLM 调用
- 自适应学习: FrontstageApp accept/reject 接入 `record_feedback`
- 审计: `LlmService::generate` 完成后调用 `log_ai_usage`
- 配额: auto_write/auto_revise 错误处理识别配额关键字

### P1 功能补全（8 项）

- ContinuityEngine 补全 timeline + character_emotion + relationship 检查
- 一键创作 `CreationWorkflowEngine` 每阶段发射 `workflow-progress` 事件
- SceneRepository 新增 5 个单元测试（139→144→145）
- hooks/index.ts 补全 useCommentThreads 等 6 个 Hook
- 类型: ChangeTrack.scene_id 改为 `string | undefined`
- 评论: RichTextEditor 已解决评论支持「重新打开」
- 变更追踪: 修订模式增加单条 change 独立接受/拒绝按钮
- 清理: 移除弃用 `check_ai_quota` IPC 注册

### P2 优化（6 项）

- Sidebar `chapter_count` 显示从"场景"改为"章"
- SceneEditor 置信度滑块 step 从 0.05 改为 0.1
- 拆书转故事字段映射优化
- 幕后新增 Foreshadowing 页面
- 6 个关键业务点激活技能 Hook 调用
- 孤儿表评估保留兼容

## [v3.5.2] - 全功能落地：剩余 7 项修复完成（2026-04-22）

### 🎯 修复项 #17 - auto_revise 取消/进度事件

- `auto_revise` 从同步阻塞调用改造为后台任务模式（同 `auto_write`）
- 新增 4 阶段进度事件：`preparing` → `revising` → `saving` → `completed`
- 新增 `auto_revise_cancel` IPC 命令，支持用户随时取消
- 前端 `WenSiPanel` 新增进度条（百分比 + 阶段信息）和"停止修改"按钮

### 🎯 修复项 #20 - confidence_score 类型补全

- 前端 `Scene` interface 补全缺失的 `confidence_score?: number` 字段
- `SceneEditor` 戏剧结构 Tab 新增 AI 生成置信度滑块（0-100%）
- 保存时置信度值随场景数据一并持久化到数据库

### 🎯 修复项 #16 - MCP 持久连接

- 新增全局 `MCP_CONNECTIONS` 连接池（`tokio::sync::Mutex<HashMap<String, McpClient>>`）
- `connect_mcp_server` 连接后持久保存到池中，`call_mcp_tool` 复用已有连接
- 新增 `disconnect_mcp_server` 和 `get_mcp_connections` 命令
- 前端 `useMcpTools` 适配新 API，断开连接时真正释放后端资源
- `WebSearchTool` 改为真实 DuckDuckGo 搜索（HTML 解析），失败时回退模拟数据

### 🎯 修复项 #19 - 一键创作按钮

- `Stories` 页面每个故事卡片新增"一键创作"按钮（Sparkles 图标）
- 调用 `run_creation_workflow` 命令，`ai_only` 模式基于故事描述自动生成
- 加载状态防重复点击，结果显示 toast 通知

### 🎯 修复项 #18 - StyleDNA 前端选择 UI

- `stories` 表新增 `style_dna_id` 字段（Migration 20 自动迁移）
- 后端新增 `list_style_dnas` 和 `set_story_style_dna` IPC 命令
- `build_agent_context` 自动读取 story 的 `style_dna_id` 并注入 `AgentContext`
- 前端 `Stories` 页面每个故事卡片新增"风格"按钮
- 弹出 StyleDNA 选择模态框，展示所有内置/自定义风格，一键切换
- `StoryRepository` / `Story` 模型全链路支持 `style_dna_id` 读写

### 🎯 修复项 #15 - 技能系统补全 LLM 调用 + 缺失技能

- `execute_skill` 命令从同步改为异步，内部自动调用 `LlmService::generate`
- 所有 PromptRuntime 技能（style_enhancer / plot_twist / text_formatter 等）现在真正调用 LLM
- `format_text` 简化为复用 `execute_skill`，移除重复的低级 HTTP 调用代码
- 新增内置技能 `character_voice`（角色声音一致性检查与增强）
- 新增内置技能 `emotion_pacing`（情感曲线分析与节奏优化）
- 内置技能总数从 3 个补全至 5 个

### 🎯 修复项 #14 - 意图引擎接入聊天栏

- `RichTextEditor` 聊天栏接入 `useIntent` hook
- 用户发送消息后先调用 `parseIntent` 解析意图类型
- `text_generate` / `text_rewrite` / `unknown` → 走现有 `writerAgentExecute` 路径
- `plot_suggest` / `character_check` / `world_consistency` / `style_shift` / `outline_expand` → 走 `executeIntent` 路径
- 解析失败时自动回退到 WriterAgent，保证用户体验不中断

### 📊 质量验证

- **139 项 Rust 后端测试全部通过**
- **前端构建通过**
- `cargo check` 零警告
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 3.5.2

---

## [v3.5.1] - 全面功能审计与修复（2026-04-22）

### 🔧 关键缺陷修复（13 项）

**自动修改 (auto_revise)**

- 修复修改结果永不应用到编辑器的致命 bug
- 后端自动保存修改后的内容到 scenes 表
- 前端 `WenSiPanel` 新增 `onReviseResult` 回调，`RichTextEditor` 接收后更新内容

**拆书功能 (book_deconstruction)**

- 修复提取的书名/作者永不写入数据库的 bug
- 修复 `convert_to_story` 返回错误 story_id 导致角色/场景关联失效的 bug
- 修复任务执行器未调用 `store_embeddings` 导致向量存储缺失的 bug
- 修复任务完成后数据库进度停在 95% 的问题（改为 100%）
- 修复心跳事件 progress=0 造成 UI 进度条闪烁的问题
- 前端 `BookListGrid` 新增 `cancelled: '已取消'` 状态标签
- 前端 `useBookDeconstruction` 过滤非当前 task_id 的事件，避免多任务进度乱跳

**场景模型与版本控制**

- 生产环境 `create_v3_tables` 中新增完整 `scene_versions` 表定义
- Migration 19 为已有数据库补建 `scene_versions` 表
- 修复 `conflict_type` 从错误列索引（5 而非 6）读取的 bug
- `Scenes.tsx` 版本快照检测扩展至全部字段（戏剧目标、外部压迫、冲突类型、场景设置等）
- `create_scene` 命令新增 `dramatic_goal`/`external_pressure`/`conflict_type` 参数

**AI 生成核心**

- `AgentOrchestrator` 集成到 `writer_agent_execute`，实现 Writer→Inspector→Writer 闭环优化
- `AgentOrchestrator` 每步完成后发射 `orchestrator-step-{task_id}` 事件到前端
- `ContinuityEngine` 集成到 `execute_writer` Reviewing 阶段，自动检测一致性 issues
- `ForeshadowingTracker` 集成到 `build_agent_context`，将未解决伏笔注入 Writer prompt
- `AdaptiveGenerator` 动态参数实际应用到 LLM 调用（temperature/max_tokens 替代硬编码）
- `auto_write` 循环结束后保存到数据库并后台触发 `IngestPipeline` 知识图谱更新
- Inspector prompt 改为要求 JSON 结构化输出，`parse_inspection_result` 增强三层解析（JSON→正则→关键词）

**基础设施**

- LLM 取消机制：`LlmService` 新增 `cancel_senders`，`cancel_generation()` 发送取消信号
- `llm_cancel_generation` 命令从 TODO stub 改为实际实现
- 前端 `useLlmStream` hook 封装真实 SSE 流式生成，替换 mock 数据
- `FrontstageApp` 集成 `useLlmStream`，`handleRequestGeneration` 调用真实流式接口
- StyleDNA 内置风格自动种子化：App 启动时检测空表则插入 10 种经典作家 DNA
- `CreationWorkflowEngine` 暴露 `run_creation_workflow` Tauri 命令，支持 3 种创作模式

### 📊 质量验证

- **139 项 Rust 后端测试全部通过**
- **前端构建通过**
- `cargo check` 零警告
- 已推送至 GitHub

---

## [v3.5.0] - 拆书体验升级（2026-04-21）

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

**数据库**

- `reference_books` 表新增 `task_id` 字段，关联拆书任务
- Migration 18 自动迁移

### 🏗️ 架构与质量

- **139 项 Rust 后端测试全部通过**
- **前端构建通过**
- `cargo check` 零警告
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 3.5.0

## [v3.4.0] - 智能化创作系统（2026-04-18）

### 🧠 智能化创作系统（5 阶段重构）

**Phase 1 - 地基重构：真实上下文**

- `StoryContextBuilder` — 从真实数据库构建丰富的 Agent 上下文（世界观、角色、场景结构）
- `QueryPipeline` — 四阶段知识检索（CJK 分词搜索 → 知识图谱扩展 → 预算控制 → 上下文组装）
- `ContinuityEngine` + `ForeshadowingTracker` — 连续性追踪与伏笔回收系统
- `IngestPipeline` 自动触发 — 场景保存后自动摄取知识图谱

**Phase 2 - 方法论注入**

- 创作方法论引擎：`MethodologyEngine` 自动将方法论约束注入 Writer 系统提示词
- 四种经典方法论：
  - **雪花法**（10 步渐进细化）
  - **场景节拍表**（6 节拍：开场→冲突→行动→转折→高潮→结局）
  - **英雄之旅**（12 阶段：平凡世界→冒险召唤→拒绝→导师→跨越→考验→深渊→蜕变→奖赏→归途→复活→携宝归乡）
  - **人物深度模型**（6 维度：性格/动机/关系/成长/语言/秘密）
- `AgentOrchestrator` — Writer→Inspector→Writer 质量反馈循环
  - 可配置质量阈值（默认 0.75）和最大循环数（默认 2）
  - Inspector 评分未达标时自动生成重写反馈

**Phase 3 - 风格深度化**

- `StyleDNA` 六维定量模型：词汇/句法/修辞/视角/情感/对白
- 10 种内置经典作家 DNA：金庸、张爱玲、海明威、村上春树、莫言、古典散文、现代极简、黑色侦探、武侠诗意、浪漫主义
- `StyleAnalyzer` — 从文本提取 StyleDNA 指纹
- `StyleChecker` — 对比文本与目标 DNA 的相似度
- 实时风格相似度计算与提示词注入

**Phase 4 - 自适应学习**

- `FeedbackRecorder` — 记录用户对 AI 生成内容的接受/拒绝/修改行为
- `PreferenceMiner` — 五维度启发式偏好挖掘（主题/风格/节奏/视角/结构）
- `AdaptiveGenerator` — 动态调节温度（temperature）、top-p、提示词权重
- `PromptPersonalizer` — 将用户偏好自动注入系统提示词
- `AdaptiveLearningEngine` — 统一入口，整合反馈→挖掘→生成→个性化全流程

**Phase 5 - 工作流闭环**

- `CreationWorkflowEngine` — 7 阶段全自动工作流
  - Conception（构思）→ Outlining（大纲）→ SceneDesign（场景设计）→ Writing（写作）→ Review（审阅）→ Iteration（迭代）→ Ingestion（入库）
- 3 种创作模式：
  - `OneClick` — 一键全自动
  - `AiDraftHumanEdit` — AI 初稿 + 人工精修
  - `HumanDraftAiPolish` — 人工初稿 + AI 润色
- `QualityChecker` — 四维质量评估（结构/人物/风格/情节）

### 📖 拆书功能 + 任务系统（2026-04-19）

**拆书功能**

- **文件解析**: 支持 txt/pdf/epub 三种格式，txt 自动检测 UTF-8/GBK 编码
- **智能分块**: 短篇全文分析 / 中篇按章节 / 长篇固定大小(~5000字)全量覆盖，不采样跳过
- **LLM 分析 Pipeline**: 5 步深度分析 — 元信息识别 → 世界观提取 → 人物拆解 → 章节概要 → 故事线生成
- **分析结果**: 小说类型、基本信息(标题/作者)、世界观设定、人物角色与性格、章节大纲、故事线(主线/支线/高潮/转折)
- **参考素材库**: 独立 `reference_books`/`reference_characters`/`reference_scenes` 表存储，支持 file_hash 去重
- **一键转故事**: 拆书结果可一键转化为 StoryMoss 故事项目
- **前端界面**: 幕后界面新增「拆书」页面，支持上传/列表/搜索/详情查看（概览/人物/章节/故事线标签页）

**任务系统（参考 memoh-X 设计）**

- **任务调度器**: 基于 tokio::time 的共享调度器，支持 once/daily/weekly/cron 四种调度类型
- **心跳检测**: 任务执行中每步更新心跳，检测器每60秒扫描，超时5分钟自动标记失败并重试
- **防重叠执行**: 每个任务独立互斥锁，避免同一任务并发执行
- **拆书改为任务**: 每次拆书自动创建为 `book_deconstruction` 类型任务，由任务系统调度执行
- **前端任务页面**: 幕后界面新增「任务」页面，状态分组、心跳指示器、进度条、执行日志
- **IPC 命令**: 8个 Tauri 命令 — create/update/delete/list/get/trigger/cancel_task + get_task_logs

**向量化存储**

- **拆书结果入库**: 分析完成后自动为场景(summary)和人物(personality)生成 embedding
- **接入 LanceVectorStore**: 使用现有 `embeddings::embed_text` + `LanceVectorStore::upsert`
- **进度实时推送**: Tauri 事件 `book-analysis-progress` 实时推送分析进度到前端

### 🔧 Bug 修复与测试建设（2026-04-19）

**关键架构修复：TaskService 全局共享**

- **Bug**: 每个 `#[command]` 独立 `TaskService::new()` 创建实例，`BookDeconstructionExecutor` 注册在局部变量 → 前端创建的任务找不到执行器 → 拆书功能不可用
- **修复**: `TaskService` 改为泛型 `<R: Runtime>` + 手动实现 `Clone`（不依赖 `R: Clone`，确保 `Arc<Mutex<ExecutorRegistry>>` 共享）
- **修复**: `commands.rs` 所有 command 改为 `tauri::State<'_, TaskService>` 获取，不再新建实例
- **修复**: `lib.rs` `app.manage(task_service)` 全局注册，setup 阶段注册 executor 后所有 command 共享

**缓存失效修复**

- `useSetActiveModel` mutation `onSuccess` 中 `invalidateQueries({ queryKey: ['settings'] })`，解决"设为当前"后列表状态不同步问题

**测试基础设施**

- `vitest.config.ts` + `jsdom` + `@testing-library/react` 前端测试环境
- Rust `tempfile` dev-dep + `test_utils.rs` 临时目录辅助工具

**单元测试（新增 71 个）**

- `config/settings_tests.rs` — 16 tests (profile CRUD, active model, default conflict)
- `task_system/tests.rs` — 13 tests (status machine, repository CRUD, heartbeat timeout)
- `db/repositories_tests.rs` — 14 tests (Story/Character/Chapter CRUD)
- `utils/validation_tests.rs` — 20 tests (email, url, json, uuid, password, html sanitize)
- 前端 `services/__tests__/settings.test.ts` — 10 tests
- 前端 `hooks/__tests__/useSettings.test.tsx` — 4 tests
- 前端 `utils/__tests__/cn.test.ts` — 5 tests

**集成测试（新增 5 个）**

- `task_system/integration_tests.rs` — 5 tests (executor registry shared via Arc, task full lifecycle, scheduler register/unregister, no-executor failure, book deconstruction duplicate detection)
- 集成测试验证端到端流程：创建任务 → 调度 → 执行 → 状态更新，能发现单元测试发现不了的架构级 bug

**数据库修复**

- `create_test_pool()` 补充 `scene_versions` 表创建（被 `change_tracks`/`comment_threads` 外键引用）

### 🎨 品牌焕新

- 全新 Logo：「草苔」立方体标志 —— 融合自然叶脉纹理的几何立方体造型
- `cargo tauri icon logo.png` 生成全平台图标包（Windows / macOS / iOS / Android）
- 清理旧图标：`LOGO.jpg`、`icon.jpg`、`logo-source.png`

### 💎 Freemium 付费系统（2026-04-18）

**Phase 1 — 后端基础设施**

- 数据库迁移：`subscriptions`、`ai_usage_quota`、`ai_usage_logs` 表
- `SubscriptionService`：订阅状态管理、配额检查与消费、调用日志记录
- Tauri 命令：`get_subscription_status`、`check_ai_quota`、`record_ai_usage`、`dev_upgrade_subscription`

**Phase 2 — 前端付费开关**

- `useSubscription` Hook：全局订阅状态 + `canUseFeature` + `hasQuota`
- `SubscriptionStatus` 组件：Header 订阅状态指示器（免费版显示剩余配额，专业版显示"文思泉涌中"）
- 后端配额中间件：`check_ai_quota_sync` + `consume_ai_quota_sync` 统一拦截

**Phase 3 — 转化漏斗 UI**

- `SmartHintSystem` tier 感知：免费用户只显示分析提示（不生成内联修改）
- `free-hint-toast`：免费用户看到"句式单调"等提示，点击"查看 AI 改写"打开付费引导
- `UpgradePanel`：功能对比 + ¥19/月定价 + 立即升级按钮（开发测试模式）
- `quota-exhausted-toast`：配额用尽时引导升级

**Phase 4 — Agent 质量分层**

- 免费版：`max_tokens` 强制上限 1000，跳过创作方法论/风格 DNA/个性化偏好注入
- 专业版：完整 `max_tokens` + 全部高级提示词扩展

**9 项优化修复**

1. `get_user_tier` 缓存：通过 `AgentTask.tier` 避免每次调用重复查库
2. 配额先扣后执行 → 成功后扣费：避免用户为失败请求买单
3. 内联回调防抖修复：`useCallback` 包裹 `onFreeHint`，稳定引用避免定时器重置
4. `consume_ai_quota` 原子化：事务内查询+扣减，消除竞态窗口
5. 免费提示 session 冷却：`MIN_HINT_INTERVAL_MS = 30s` + `dismissedHintIdsRef` 去重
6. auto-save 定时器清理：`autoSaveTimerRef` 避免保存到错误章节
7. UpgradePanel 替换原生 `alert`：`react-hot-toast` + 加载状态
8. 配额检查失败策略：乐观策略 `allowed: true`，后端做最终校验
9. 离线 Pro 降级修复：`localStorage` 缓存订阅状态

### 🏗️ 架构与质量

- **139 项 Rust 后端测试全部通过**（63 原有 + 71 单元测试新增 + 5 集成测试新增）
- **21 项前端测试全部通过**
- `cargo check` 零警告
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 3.4.0
- `Box<dyn std::error::Error + Send + Sync>` 全链路修复 — Tauri 异步命令 Send 要求

## [Unreleased] - v3.3.0 功能断层修复与架构清理

### 🍃 品牌 Logo 全面应用（2026-04-15）

- **应用全新品牌标志**
  - 将项目根目录 `logo.png`（草苔立方体标志）生成全平台图标包
  - `cargo tauri icon` 重新生成 Windows / macOS / iOS / Android 全尺寸图标
  - 前端 `index.html` / `frontstage.html` favicon 从 `feather.svg` 替换为 `favicon.ico`
  - 生成 `apple-touch-icon.png`、`icon-192.png`、`icon-512.png` 供多设备使用
  - `docs/images/logo.png` 作为 README 及文档展示用图
  - 更新 `README.md`、`CHANGELOG.md`、`PROJECT_STATUS.md` 中的品牌图标描述

### 🖱️ 幕前右键菜单修复与样式重构（2026-04-15）

- **修复右键菜单不出现的问题**
  - `frontstage.css` 补充 `@tailwind utilities;`，解决 Tailwind utility 类（`fixed`、`z-[9999]` 等）在幕前入口不生效的问题
  - `RichTextEditor.tsx` 将事件监听改为捕获阶段，兼容 Tauri WebView 中 `contenteditable` 的原生事件拦截
  - Rust 后端通过 `webview2-com` 调用 WebView2 API 禁用 Windows 默认系统右键菜单

- **右键菜单 UI 暖色重构**
  - `EditorContextMenu.tsx` 整体色调从深色突兀风格切换为幕前暖色纸张规范
  - 背景：`bg-[var(--ivory)]`，边框：`border-[var(--warm-sand)]`
  - 主文字：`text-[var(--charcoal)]`，图标：`text-[var(--stone-gray)]`
  - Hover：`hover:bg-[var(--warm-sand)]`，禁用态：`text-[var(--stone-gray)]/60`
  - 分隔线改为 `bg-[var(--charcoal)]/10`，与暖色背景协调

### 🔧 API 一致性审计修复（2026-04-14）

- **MCP 外部服务器连接**
  - `Mcp.tsx` 新增外部服务器配置卡片，支持配置名称、启动命令、参数和环境变量
  - `useMcpTools` 新增 `connectServer` / `callExternalTool` / `disconnectServer`
  - 外部工具与内置工具统一展示，执行时自动区分调用路径

- **技能工坊 — 技能导入**
  - `Skills.tsx` 新增"导入技能"按钮
  - 集成 `@tauri-apps/plugin-dialog` 文件选择器，调用 `import_skill`

- **Agent 执行 — 取消任务 + 流式执行**
  - 后端 `agents/commands.rs` 引入全局 `TASK_HANDLES`，`agent_cancel_task` 实现真正的 `AbortHandle.abort()`
  - `agent_execute_stream` 保存任务句柄并在完成后自动清理
  - 前端 `SkillExecutionPanel` 迁移到 `agent_execute_stream`，支持实时进度事件监听
  - 添加"取消"按钮，执行中的长任务可被中断

- **知识图谱 — 实体就地编辑**
  - `KnowledgeGraphView` 实体详情面板新增编辑模式
  - 支持修改实体名称、动态增删改属性、调用 `update_entity` 保存
  - 保存后自动刷新图谱数据

- **版本系统增强**
  - `VersionTimeline` 新增"版本链"视图切换，调用 `useVersionChain` 展示分支/深度关系
  - `DiffViewer` 接入 `useVersionDiff`，在版本对比时展示元信息（标题/场景/角色/戏剧目标变更、字数/置信度变化）

- **代码对齐与清理**
  - `useVectorSearch.ts` 统一复用 `services/tauri.ts` 中的 `searchSimilar` / `textSearchVectors` / `hybridSearchVectors`
  - `services/tauri.ts` 中对暂未使用的导出添加 `@deprecated` JSDoc 标记

### 🏗️ 架构决策

- **LLM 调用路径决策**
  - 新增 `docs/LLM_CALL_PATH_DECISION.md`
  - 明确保留 HTTP 直连 (`modelService.ts`) 为前端唯一官方 LLM 调用路径
  - Tauri 侧 `llm_generate` 等命令降级为内部/备用用途

### 📝 幕前排版与 AI 续写优化（2026-04-17）

- **段落间距优化**
  - `frontstage.css` 将 `.ProseMirror p` 的 `margin-bottom` 从 `1.5em` 统一降至 `0`
  - 为所有 `.ProseMirror p` 增加 `text-indent: 2em`，符合中文小说首行缩进排版
  - 同步调整 classical / modernCN / minimal / romantic 四种写作风格的段落间距

- **底部栏遮挡修复**
  - `.rich-text-editor .ProseMirror` 的 `padding-bottom` 从 `3rem` 增至 `10rem`
  - 长文本滚动到底部时，最后一段文字不再被底部 chat toolbar 遮挡

- **自动续写**
  - `RichTextEditor.tsx` 提取 `executeWriterAgent(instruction)` 通用函数
  - 新增 `handleAcceptAndContinue`：用户按 `Tab` 或点击「接受」后，若 `aiEnabled` 开启且不在 Zen 模式，自动延迟 300ms 调用 `executeWriterAgent('续写')` 发起下一轮生成

- **Zen 模式 AI 纯净**
  - 禅模式下完全隐藏 `AiSuggestionBubble`、`generatedText` 预览、`isAiThinking` 指示器
  - 禁用 `Tab`/`Esc` 接受/拒绝快捷键，确保 F11 禅模式仅保留文字与空白

### 🔇 质量提升

- **Rust Warnings 降噪**
  - 在 50+ 个文件中批量添加 `#![allow(dead_code)]` / `#[allow(unused_imports)]` / `_` 前缀
  - `cargo check` warnings 从 **163 降至 0**
  - 未删除任何代码，仅做标记和压制

---

## [v3.2.0] - 意图引擎与 Agent 调度 + 知识图谱可视化 + 自动归档 + 场景批注 + LLM 流式升级 + 修订模式

### 🕸️ 知识图谱可视化

- **后端图数据 API**
  - `get_relations_by_story`：按故事 ID 批量查询关系
  - `get_story_graph`：一次性返回完整知识图谱（实体 + 关系）

- **交互式图谱视图** (`src-frontend/src/components/KnowledgeGraph/`)
  - 基于 **ReactFlow** 实现可缩放、可拖拽的力导向图谱
  - 节点按实体类型着色（角色/地点/物品/组织/概念/事件）
  - 关系边按强度显示不同粗细和透明度，高强度边带动画效果
  - 左上角图例面板显示统计信息
  - 点击节点展开右侧详情面板，展示属性和关联关系

- **页面集成**
  - 新增 backstage 「知识图谱」页面和 Sidebar 导航入口
  - 自动绑定当前选中的故事，空状态引导用户先选择故事

### 🧠 记忆健康与自动归档系统

- **后端保留报告 API**
  - `get_retention_report`：基于 Ebbinghaus 遗忘曲线计算实体保留状态
  - 复用已有的 `RetentionManager`，按实体类型应用不同衰减配置

- **自动归档工作流**
  - `kg_entities` 表新增 `is_archived` 和 `archived_at` 字段
  - `archive_forgotten_entities`：一键归档所有遗忘状态实体
  - `restore_archived_entity`：从归档状态恢复指定实体
  - `get_archived_entities`：查询故事的已归档实体列表
  - 数据库迁移脚本自动补全旧表缺失的保留/归档字段

- **记忆健康面板**（集成在知识图谱页面）
  - 汇总卡片：总实体数、平均优先级、系统健康状态
  - 自动归档建议：根据遗忘比例生成动态推荐文案，支持一键执行
  - 优先级分布可视化：关键/高/中/低/已遗忘五级进度条
  - 关键实体列表和待归档实体列表

- **已归档页签**
  - 知识图谱页面新增「已归档」标签页
  - 展示所有已归档实体，支持逐条恢复

### 🤖 Agent 模型映射与路由

- **后端配置持久化**
  - `AppConfig` 新增 `agent_mappings` 字段，支持 JSON 持久化
  - 默认映射：writer/inspector/outline_planner/style_mimic/plot_analyzer → Qwen 3.5
  - `get_settings` / `save_settings` 完整读写 agent_mappings
  - `get_agent_mappings` / `update_agent_mapping` 从硬编码改为读取/写入真实配置

- **模型路由逻辑**
  - `LlmService` 新增 `generate_with_profile`，支持按模型 ID 调用指定配置
  - `AgentService` 新增 `generate_for_agent`，自动根据 Agent 类型查找映射模型
  - 5 种 Agent（写作/质检/大纲/文风/情节）均已接入模型路由
  - 未配置映射时自动回退到活跃 LLM Profile

### 🧠 意图解析引擎 (Intent Engine)

- **后端意图解析器** (`src-tauri/src/intent.rs`)
  - 基于 LLM 的 JSON 意图提取，支持 11 种意图类型
  - 包含 `IntentParser`（解析）和 `IntentExecutor`（执行）两个核心组件
  - 新增 `parse_intent` 和 `execute_intent` Tauri 命令

- **Agent 调度执行**
  - 将意图的 `required_agents` 映射到现有的 `AgentService`
  - 支持串行 (`serial`) 和并行 (`parallel`) 两种执行模式
  - 执行结果包含每个 Agent 的步骤输出、评分和建议

- **前端意图感知对话**
  - `useIntent` Hook 新增 `executeIntent` 方法
  - `RichTextEditor` 聊天栏根据意图类型自动选择执行路径
  - `text_generate` / `text_rewrite` 继续走流式输出路径
  - `plot_suggest` / `character_check` / `world_consistency` 等走 Agent 调度路径
  - 聊天消息显示意图标签（如 "情节建议 · 建议卡片"）

### 📝 场景批注系统

- **数据库与后端 API**
  - 新增 `scene_annotations` 表，支持场景级批注/笔记/待办
  - 7 个 Tauri 命令：`create_scene_annotation`、`get_scene_annotations`、`get_story_unresolved_annotations`、`update_scene_annotation`、`resolve_scene_annotation`、`unresolve_scene_annotation`、`delete_scene_annotation`
  - 批注类型：`note` / `todo` / `warning` / `idea`
  - 支持标记「已解决」与恢复未解决状态

- **前端集成**
  - `SceneEditor` 新增「批注」标签页
  - 支持新建批注（带类型选择）、编辑、解决/恢复、删除
  - 已解决批注显示划线与降透明度
  - React Query Hook：`useSceneAnnotations`、`useStoryUnresolvedAnnotations`

### 🧠 实体嵌入持久化修复

- `kg_entities.embedding` BLOB 读写修复
  - `create_entity` 现在接受并持久化 `Option<Vec<f32>>` 嵌入向量
  - 所有查询方法（`get_entities_by_story`、`get_archived_entities`、`get_entity_by_id`）正确反序列化 BLOB 为 `Vec<f32>`
  - 小说创建向导的自动 Ingest 结果中的实体嵌入现已正确保存到数据库

### 🌊 LLM 真实 SSE 流式输出

- **适配器架构升级**
  - `LlmAdapter` trait 新增 `generate_stream` 方法，统一流式接口
  - `OpenAiAdapter` 实现真实 SSE 流式调用（`stream=true`）
  - 新增 `AnthropicAdapter`：支持同步与 SSE 流式生成
  - 新增 `OllamaAdapter`：支持同步与 NDJSON 流式生成

- **服务层接入**
  - `LlmService::generate_stream` 从模拟文本改为调用真实适配器流式 API
  - 通过 `tokio::sync::mpsc` channel 消费 chunk，实时推送 `llm-stream-chunk-{request_id}` 事件到前端
  - 前端事件格式保持不变，无需修改即可接入真实流式生成

### 🕸️ 知识图谱交互增强

- `KnowledgeGraphView` 新增搜索与筛选面板
  - 实时按名称搜索节点
  - 按实体类型（6 种）快速过滤，支持全选/清空
  - 双击节点聚焦并平滑动画居中
  - 图例面板同步显示可见/隐藏节点统计

### 💾 SQLite 向量存储持久化

- **替换 JSON 内存 fallback**
  - `LanceVectorStore` 内部实现从 `HashMap + records.json` 改为 `SQLite + vector_store.db`
  - 保留完全相同的公共 API：`upsert`、`search`、`delete`、`count`
  - 所有现有调用方（`search_similar`、`embed_chapter`、`HybridSearch`）无需修改

- **数据表结构**
  - `vector_records` 表存储 `id`、`story_id`、`chapter_id`、`text`、`record_type`、`embedding`（JSON）
  - 创建 `story_id` 和 `chapter_id` 索引优化查询

- **持久化验证**
  - 单元测试验证：跨实例重启后记录不丢失
  - `upsert` 使用 `ON CONFLICT(id) DO UPDATE` 实现幂等写入

### 🛠️ 技能工坊 (Skills) 后端连通

- **前端类型对齐**
  - `Skill` 接口扩展为完整 `SkillInfo` 结构，包含 `parameters`、`hooks`、`runtime_type` 等字段

- **真实数据接入**
  - `Skills.tsx` 从 mock 数据改为调用 `getSkills()` 拉取后端技能列表
  - 支持按分类筛选（全部 / 写作 / 分析 / 角色 / 情节 / 风格等）

- **技能操作**
  - 启用/禁用开关调用 `enable_skill` / `disable_skill`
  - 执行按钮支持 Prompt 技能运行，自动弹出必填参数输入框
  - 非内置技能显示卸载按钮，调用 `uninstall_skill`

### 🪄 小说创建向导 (NovelCreationWizard) 后端连通与自动 Ingest

- **后端 Agent 命令**
  - `generate_world_building_options`：基于用户输入生成世界观选项
  - `generate_character_profiles`：基于世界观生成角色谱选项
  - `generate_writing_styles`：生成文字风格选项
  - `generate_first_scene`：生成首个场景
  - `create_story_with_wizard`：一键保存故事、世界观、角色、文风、首个场景，并自动触发 Ingest

- **Dashboard 集成**
  - 主按钮从「新建故事」改为「AI 创建故事」，打开 NovelCreationWizard
  - 保留「手动创建」入口作为备用
  - 空状态时同时显示 AI 创建和手动创建按钮

- **前端向导重构**
  - `NovelCreationWizard` 从 mock 数据改为真实调用后端 Agent 命令
  - 每一步显示加载状态，失败时自动回退并提示重试
  - 完成页展示世界观、角色、文风、场景四项准备状态

- **自动 Ingest**
  - 向导完成后自动将世界观、角色设定、首个场景内容送入 `IngestPipeline`
  - 提取实体和关系并保存到知识图谱
  - 创建成功 toast 显示摄取的实体数和关系数

### ✏️ 文本内联批注系统

- **数据库与后端 API**
  - 新增 `text_annotations` 表，支持文本级别的内联批注
  - 8 个 Tauri 命令：`create_text_annotation`、`get_text_annotations_by_chapter`、`get_text_annotations_by_scene`、`update_text_annotation`、`resolve_text_annotation`、`unresolve_text_annotation`、`delete_text_annotation`
  - 支持按 `chapter_id` 或 `scene_id` 查询，带 `from_pos` / `to_pos` 文本坐标

- **前端集成**
  - 新增 `useTextAnnotations` 系列 React Query Hook
  - 完整支持新建、编辑、解决/恢复、删除批注

### 🔄 修订模式与变更追踪 (P3)

#### Phase 1 — 变更追踪核心

- **数据库与后端 API**
  - 新增 `change_tracks` 表，记录单条编辑操作的类型、位置、内容、作者和状态
  - `ChangeTrackRepository` 支持创建、查询、状态更新、批量接受/拒绝
  - 6 个 Tauri 命令：`track_change`、`accept_change`、`reject_change`、`get_pending_changes`、`accept_all_changes`、`reject_all_changes`

- **TipTap 编辑器扩展**
  - `TrackInsert` Mark：蓝色下划线 + 淡蓝背景，带 `changeId` / `authorId` 属性
  - `TrackDelete` Mark：红色删除线 + 淡红背景
  - `RichTextEditor` 集成修订模式开关、待审变更数横幅、全部接受/拒绝/退出按钮

- **前端状态管理**
  - `useChangeTracking` 系列 Hook：待审变更查询、单条追踪、接受/拒绝、批量操作
  - 实时 diff 检测：`onUpdate` 中对比文本变化，自动调用 `track_change`

#### Phase 2 — 评论线程系统

- **数据库与后端 API**
  - 新增 `comment_threads` 和 `comment_messages` 表，支持多回复线程
  - `CommentThreadRepository` 支持创建线程、添加消息、查询、解决/重开/删除
  - 8 个 Tauri 命令：`create_comment_thread`、`add_comment_message`、`get_comment_threads`、`resolve_comment_thread`、`reopen_comment_thread`、`delete_comment_thread`

- **TipTap 编辑器扩展**
  - `CommentAnchor` Mark：黄色高亮 + 虚线下划线，锚定 `threadId`

- **前端集成**
  - `useCommentThreads` 系列 Hook：线程查询、创建、回复、解决、重开、删除
  - `RichTextEditor` 右侧评论面板：选中文本创建线程、浏览消息、状态切换

#### Phase 3 — 版本集成

- **自动 diff 生成 ChangeTrack**
  - `create_scene_version` 在创建版本时自动与上一版本内容做字符级 diff
  - 将差异转换为 `ChangeTrack`（Insert / Delete）并绑定到该 `version_id`

- **版本历史集成**
  - 新增 `get_version_change_tracks` 命令和 `ChangeTrackRepository::get_by_version`
  - `VersionTimeline` 选中版本时展示该版本的所有变更追踪详情
  - `Scenes.tsx` 预览面板新增「版本历史」标签页，集成 `VersionTimeline` 和 `DiffViewer`
  - 保存场景时自动创建版本快照（内容或元数据变更触发）

### 🎭 古典评点家 Agent (CommentatorAgent)

- **后端 Agent 实现**
  - 新增 `CommentatorAgent` (`agents/commentator.rs`)，模拟金圣叹风格对小说段落进行实时文学点评
  - 支持 `ParagraphCommentary` 结构，返回段落索引、点评内容和语气类型
  - `AgentType` 新增 `Commentator` 变体，集成到 `AgentService` 模型路由
  - 新增 Tauri 命令 `generate_paragraph_commentaries`

- **前端集成**
  - `RichTextEditor` 聊天栏新增「生成古典评点」按钮
  - 调用后端逐段生成评点后，以 `commentary-paragraph` 样式插入编辑器
  - 古典批注样式：小字号（0.8em）、赤陶色（terracotta）、斜体、左侧缩进，还原传统小说批注效果

### ⚡ 性能与缓存优化

- **实体向量自动更新**
  - `update_entity` 命令支持 `regenerate_embedding` 参数
  - 当实体名称或属性变更时，可选自动重新生成并保存嵌入向量

- **向量搜索缓存**
  - `LanceVectorStore` 新增 `HashMap` 结果缓存，最大容量 100 条
  - 简单 LRU 淘汰策略（溢出时移除最旧的 20%），写操作时自动失效缓存

- **并行 Ingest 处理**
  - `IngestBatch::process` 改为使用 `futures::future::join_all` 并发执行内容摄取
  - 显著提升批量内容的处理吞吐量

### 🧠 Agent 上下文增强

- **`build_agent_context` 真实数据库接入**
  - 修复 `agents/commands.rs` 中长期存在的 TODO
  - 现在所有 Agent 执行任务时，上下文会自动从数据库拉取：
    - 作品标题、题材、文风、节奏（从 `stories` + `writing_styles` 表）
    - 角色信息（从 `characters` 表，包含姓名、性格、角色定位）
    - 前场景摘要（从 `scenes` 表，按 sequence_number 过滤并生成摘要）
  - 写作助手、质检员、评点家、记忆压缩师等 Agent 均获得更精准的上下文

### 🗜️ 记忆压缩师集成 (MemoryCompressorAgent)

- **后端命令**
  - 新增 `compress_content`：对任意内容进行记忆压缩
  - 新增 `compress_scene`：自动读取场景内容并调用压缩 Agent
  - 支持 `target_ratio` 参数控制压缩比例（默认 25%）

- **前端集成**
  - `SceneEditor` 内容标签页新增「记忆压缩」按钮
  - 压缩结果以下方面板展示，支持「应用」到场景内容或「关闭」
  - 新增 `useCompressScene` / `useCompressContent` React Query Hooks

### ⚔️ 冲突类型扩展

- `ConflictType` 新增 4 种戏剧冲突：
  - `ManVsTime` — 人与时间
  - `ManVsMorality` — 人与道德
  - `ManVsIdentity` — 人与身份
  - `FactionVsFaction` — 群体冲突
- `SceneEditor` 冲突选择网格从 2 列调整为 3 列，容纳 11 种冲突类型

### 🔍 SQLite FTS5 语义搜索优化

- **FTS5 全文索引**
  - `vector_records` 表新增 FTS5 虚拟表 `vector_records_fts`
  - 自动触发器同步 INSERT/UPDATE/DELETE，无需应用层手动维护

- **新搜索 API**
  - `text_search_vectors`：基于 BM25 的全文关键词搜索
  - `hybrid_search_vectors`：向量相似度 + FTS5 全文搜索的 RRF 融合
  - 前端新增 `useTextSearchVectors` / `useHybridSearchVectors` Hooks

- **性能收益**
  - 文本搜索从纯向量扫描升级为 FTS5 索引加速
  - 混合搜索通过 RRF（Reciprocal Rank Fusion）融合两路结果，召回率和相关性显著提升

## [3.1.2] - 2026-04-13 - 设置页增强、浏览器开发环境修复与全新应用图标

### 🎨 全新应用图标

- 从 [iconbuddy.com](https://iconbuddy.com) 引入 **Lucide `feather`** 作为 StoryMoss 品牌图标
- 设计理念：羽毛笔象征创作与文学，金色羽毛配合深色背景，优雅且富有辨识度
- 使用 `cargo tauri icon` 重新生成全平台图标包（Windows .ico / macOS .icns / iOS / Android / UWP）
- 前端 favicon 同步替换为 `feather.svg`

### 🔧 幕后设置页修复

- **编辑模型模态框修复**
  - 修复 `custom` 提供商在编辑时缺少 API Key 输入框的问题
  - 现在 `custom` 类型模型始终显示 API Key 字段，兼容本地无密钥与有密钥模型

- **模型连接状态指示灯**
  - 模型卡片右上角新增实时连接状态检测
  - **检测中**：灰色加载动画
  - **已连接 (xxms)**：绿色圆点 + 延迟显示
  - **连接失败**：红色圆点（hover 查看错误详情）
  - 浏览器开发环境下通过 `fetch` 探测 `api_base` 可用性（5 秒超时）

### 🌐 浏览器开发环境兼容

- **Vite dev server 模型回退**
  - `getModels()` / `getSettings()` / `testModelConnection()` 在浏览器环境下自动回退到本地硬编码模型
  - backstage 设置页在 `npm run dev` 浏览器模式下不再显示「暂无模型配置」
  - 同步更新 `docs/images/backstage-preview.png`

---

## [3.1.1] - 2026-04-13 - 幕前界面重构、Waza 设计与 CI 修复

### 🎭 幕前界面重构（Waza 设计原则落地）

- **精简侧边栏**
  - 侧边栏宽度缩减至 120px，仅保留"幕后"切换按钮
  - 去除冗余图标和文字，追求极简禅意
  - 修复按钮溢出侧边栏宽度的布局问题

- **颜色系统重构（OKLCH）**
  - 所有 Hex/HSL 颜色替换为 OKLCH，建立感知均匀的 60-30-10 视觉权重
  - 主背景：`oklch(96.5% 0.008 95)`（暖纸张色）
  - 强调色：`oklch(58% 0.13 45)`（赤陶色）
  - 去除装饰性纸张噪点纹理，背景更纯净

- **字体系统升级**
  - 移除 Waza 反感的 Crimson Pro / Cormorant Garamond / Inter
  - 正文字体统一为「霞鹜文楷 (LXGW WenKai) + 思源宋体」
  - 无衬线回退：`SF Pro Display / Segoe UI / PingFang SC`

- **微交互与排版**
  - 所有按钮增加 `active:scale-95` 触感反馈
  - 全面清除 `transition: all` 反模式，改为精确属性过渡
  - Blockquote 从左边框模板改为「背景色块 + 大引号装饰」

- **顶部动态状态栏**
  - 字数统计、字体大小、快捷键提示、保存状态集中展示
  - 去除底部固定的 AI 续写按钮，界面更加纯净

- **底部 LLM 对话栏**
  - 默认隐藏，鼠标悬停底部区域时优雅浮现
  - 集成模型状态指示灯（绿/黄/红三色 + 呼吸动画）
  - 去除对话/多模态模式切换图标，保持输入框极简
  - 占位文案："在此驾驭智能文思"
  - Enter 发送，Shift+Enter 换行，支持流式对话输出

### 🤖 本地三模型配置

- **Gemma-4-31B-it-Q6_K** (`http://10.62.239.13:17099/v1`)
  - 用途：多模态对话
  - 状态：已配置，无 API Key

- **Qwen3.5-27B-Uncensored-Q4_K_M** (`http://10.62.239.13:17098/v1`)
  - 用途：语言模型对话（默认"文思助手"）
  - 状态：已配置，无 API Key

- **bge-m3** (`http://10.62.239.13:8089`)
  - 用途：Embedding 向量嵌入
  - 状态：已配置，带 API Key

### 🖥️ Tauri 本地构建与 CI 修复

- 修复 `tauri.conf.json` 中 `beforeBuildCommand` 在 Windows 下的路径兼容性问题
- 成功构建 Release 版本并打包 Windows 安装程序
- 生成 MSI (12.3 MB) 和 NSIS (8.1 MB) 两种安装包
- 修复 GitHub Actions 跨平台构建缺少 `icons/icon.icns` 的问题
- `rust-check` 三平台（Ubuntu / Windows / macOS）全部通过
- **自动发布 Nightly Release**：每次推送到 master 自动构建并发布三平台安装包到 GitHub Releases

---

## [3.1.0] - 2025-04-13 - 智能记忆与版本管理

### 🔍 Hybrid Search (混合搜索)

**Phase 1.3 Implementation**

- **BM25 Search** (`memory/hybrid_search.rs`)
  - CJK Bigram tokenizer for Chinese text
  - Inverted index with TF-IDF scoring
  - Configurable k1 and b parameters

- **Hybrid Search Engine**
  - BM25 + Vector similarity fusion
  - RRF (Reciprocal Rank Fusion) ranking
  - Configurable weights (default: BM25 40%, Vector 60%)

- **Entity Hybrid Search**
  - Name matching + vector similarity
  - Cosine similarity calculation
  - Priority scoring for entity retrieval

### 📜 Scene Version Management (场景版本管理)

**Phase 3.x Implementation**

- **SceneVersionRepository** (`db/repositories_v3.rs`)
  - `create_version()` - Snapshot current scene state
  - `get_versions()` - List version history
  - `get_version()` - Get specific version
  - `delete_version()` - Remove version

- **SceneVersionService** (`versions/service.rs`)
  - `compare_versions()` - Line-level diff with word count delta
  - `restore_version()` - Restore to any historical version
  - `get_version_chain()` - Version chain with branch structure
  - `get_version_stats()` - Edit distribution, avg confidence

- **Frontend Components**
  - `VersionTimeline.tsx` - Vertical timeline with selection
  - `ConfidenceIndicator.tsx` - Circular/bar progress indicator
  - `DiffViewer.tsx` - Side-by-side diff view
  - `useSceneVersions.ts` - React Query hooks

### 🧠 Memory Retention Management (记忆保留管理)

**Phase 1.4 Implementation**

- **RetentionManager** (`memory/retention.rs`)
  - Ebbinghaus forgetting curve: R(t) = R₀ × e^(-λt)
  - 5 priority levels: Critical/High/Medium/Low/Forgotten
  - Retention report generation
  - Context window optimization

---

## [3.0.0] - 2025-04-12 - 重大架构调整

### 🎪 场景化叙事架构

- Scene 取代 Chapter，戏剧冲突驱动
- 戏剧目标、外部压迫、冲突类型、角色冲突
- StoryTimeline 拖拽排序、SceneEditor 三标签页

### 🧠 增强记忆系统

- CJK Bigram Tokenizer
- 两步 Ingest Pipeline
- 带权知识图谱
- 四阶段 Query Pipeline
- 多助手独立会话

### 🤖 AI 智能生成

- NovelCreationAgent
- 4 步引导式创建向导
- 卡片式 UI

### 📦 工作室配置

- 每部小说独立配置
- ZIP 导入/导出

---

## [2.0.0] - 2025-04-12

- 幕前-幕后双界面架构
- 双窗口通信

## [1.5.0] - 2025-04-08

- Agent 系统
- 工作流引擎
- 向量存储

## [1.0.0] - 2025-04-01

- 基础架构
- LLM 集成
- 数据库设计
