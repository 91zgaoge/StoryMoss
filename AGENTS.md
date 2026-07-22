# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.30.15
- **GitHub**: https://github.com/91zgaoge/StoryMoss
- **技术栈**: Tauri 2.4 + Rust 1.95.0 + React 18 + TypeScript 5.8 + Vite 6 + SQLite + LanceDB
- **双界面**: 幕前 `/frontstage.html`（沉浸式写作），幕后 `/index.html`（工作室管理）

## 编码风格

- **Rust**: `snake_case`，`Result<T, E>`，异步 `async/await`，数据库 `rusqlite` + `r2d2`。
- **TypeScript**: `camelCase`，函数组件 + Hooks，Zustand 状态管理，TanStack Query 调用后端。

## 开发命令

```bash
# 前端开发服务器
cd src-frontend && npm run dev

# 启动 Tauri 桌面应用
cd src-tauri && cargo tauri dev

# 构建生产版本
cd src-tauri && cargo tauri build

# 测试与检查
cd src-tauri && cargo test --lib
cd src-frontend && npx tsc --noEmit
npx vitest run
npm test                              # Playwright E2E
node scripts/cdp-inspect.js           # CDP 截图
```

## Pre-commit 格式守卫

仓库内置 `.githooks/pre-commit`：提交前自动检查本次 staged 的 Rust（`cargo +nightly fmt -- --check`）与前端（`prettier --check`）代码是否已格式化，未格式化则拒绝提交，对齐 CI 的 fmt 检查。

- **首次克隆后启用**：`git config core.hooksPath .githooks`
- **行为**：仅检查本次 `git add` 进来的 `.rs` / `.ts` / `.tsx` / `.css` / `.json` 代码文件，纯文档/配置提交不受影响；失败时打印 diff 并给出修复命令。
- **修复**：按提示执行 `(cd src-tauri && cargo +nightly fmt)` 或 `(cd src-frontend && npm run format)`，再 `git add -u && git commit`。
- **紧急绕过**：`git commit --no-verify`（仅限紧急情况，CI 仍会兜底检查）。

## 强制构建规则（用户级）

1. **每次修改代码后**：先推送到 GitHub，触发 GitHub Actions 全平台构建。
2. **推送后**：在本地执行 `cargo tauri build`，生成本平台安装包（macOS `.dmg` / Windows `.exe`+`.msi` / Linux `.AppImage`+`.deb`）。
3. **版本号统一**：`Git tag`、`Cargo.toml`、`src-tauri/tauri.conf.json`、`src-frontend/package.json` 必须一致。
4. **每次推送必须更新** `README.md` 与以下文档：`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`。
5. **版本标签**：每次推送使用新 tag，禁止 force push 覆盖已有 tag。
   ```bash
   git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z
   ```

## 提交信息格式

```
<type>: <subject>

type:
  feat / fix / docs / style / refactor / test / chore
```

## 重要文档

- [README.md](./README.md)
- [docs/USER_GUIDE.md](./docs/USER_GUIDE.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [TESTING.md](./TESTING.md)
- [CHANGELOG.md](./CHANGELOG.md)
- [ROADMAP.md](./ROADMAP.md)
- [docs/archive/AGENTS_HISTORY.md](./docs/archive/AGENTS_HISTORY.md) — 完整历史版本记录
- [docs/archive/LESSONS_LEARNED.md](./docs/archive/LESSONS_LEARNED.md) — 项目修复过程中积累的经验教训与反模式

## 当前编译状态

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 964 passed
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 305 passed / 3 skipped
- `npx playwright test` ✅ 本版未重跑 E2E
- `cargo +nightly fmt` ✅
- `cargo clippy --lib` ✅ 无新增告警
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅

## 最近完成的功能

### v0.30.15 - 场景围绕故事大纲生成（创作原则加固）

- **创作原则**：有故事大纲时，场景必须围绕故事大纲展开。用户报告续写内容与故事大纲"两张皮"（场景大纲写"金敏秀"，续写跑偏到核电站，与故事大纲"韩雪/李明在首尔"脱节）。
- **根因 A（场景大纲生成用错提示词）**：`generate_scene_outline` 复用故事级 `outline_planner.md`（要求三幕式/章节划分/角色弧线），`task.input` 几乎为空且**不注入 story_outlines.content** -> 模型幻觉新角色"金敏秀"（不在角色卡），场景大纲与故事大纲冲突。
- **根因 B（writer 看不到故事大纲）**：续写走 TimeSliced/TriShot，prompt 只用 `WriteTimeBundle.to_prompt()`；故事大纲只在 Full/Fast 路径计算，**从未到达 writer** -> 内容偏离大纲。
- **Fix A（场景大纲生成锚定故事大纲，`creation_commands.rs` + `agents/service.rs` + 新提示词）**：新增场景级提示词 `resources/prompts/planner/scene_outline.md`（强制复用已登场角色、禁止发明新角色、围绕故事大纲对应节点展开）；`generate_scene_outline` 加载 `story_outlines.content` + 场景序号注入 `task.parameters`；`build_outline_prompt` 分流（场景模式用 `scene_outline`，workflow 故事级仍用 `outline_planner`）。
- **Fix B（writer 锚定故事大纲，`domain/write_time_bundle.rs` + `creative_engine/write_time_bundle.rs`）**：WriteTimeBundle 新增 `story_outline` 字段，`load_sync` 加载 `story_outlines.content`，`to_prompt()` 在世界观红线**之后**插入权威段【故事大纲（本场景必须围绕此大纲展开，禁止偏离）】（保持红线第一不变量）；冲突时以故事大纲为准并使用已登场角色。一处覆盖 TimeSliced+TriShot。
- **验证**：`cargo test --lib` 964 passed（+4）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。
- **注意**：现有"金敏秀"场景大纲需用户重新点"生成大纲"覆盖（Fix A 修生成器）；Fix B 让 writer 即使面对旧毒大纲也锚定故事大纲。

### v0.30.14 - 续写返回风格增强模板修复（多步 plan 尾部非 writer 覆盖正文）

- **根因（结构性）**：`execute_plan`（`planner/executor.rs:685-687`）用**最后产出 `content` 的步骤**作为 `final_content` 返回用户。force-correction（防线 2）只修正**首步**，无法拦截多步 plan **尾部**的 `style_enhancer`/`inspector`--尾部非 writer 的模板/报告会覆盖 writer 已产出的正文。用户报告"增强第二章"得到 `[inspector, style_enhancer]` 多步 plan，style_enhancer 收到 inspector 报告后抱怨"这是一份质量检查报告而非章节原文"。这是该误路由 bug **第 5 次复发**（v0.30.10/11/12/13 各堵一条路径：模板重放/朴素子串/inspector 漏拦/SING 绕过，但多步尾部漏网）。
- **Fix（防线 3，`planner/mod.rs` + `planner/executor.rs`）**：新增 `PlanGenerator::sanitize_plan_for_prose_request`，在 plan 执行咽喉点 `execute_with_context`（force-correction 之后）对所有 `is_prose_request` plan 统一净化：①移除 `builtin.style_enhancer`/`text_formatter`/`character_voice`/`emotion_pacing` 等绝不产出正文的技能步骤；②续写（`is_continuation`）塌缩为单 writer 步；③其余 prose 请求弹出尾部非 writer 步骤，**保证末步为 writer**（`final_content` = 正文），保留 `[inspector, writer]` 等 Rule 9 合法流；④净化后空则补 writer 步。非 prose 请求（显式审查 `Audit`/`is_prose_request=false`）不净化。
- **验证**：`cargo test --lib` 960 passed（+12 sanitize 回归）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。

### v0.30.13 - 续写返回风格增强模板修复（SING 路径绕过 force-correction）

- **根因**：planner force-correction（防线 2）只在 `PlanGenerator::generate_plan` 内施加，而 `PlanExecutor::execute_with_context` 的 SING（IntentionGraphPlanner）路径直接返回 plan（`planner/executor.rs:148-178`）、**完全绕过** `generate_plan`。当 SING 把续写路由到 `builtin.style_enhancer`（Skill 资产）作为首步时，force-correction 从不执行，style_enhancer 收到空 content 返回"请提供需要增强的原始文本"模板。v0.30.11 禁用模板重放消除了模板路径，但 SING 路径的绕过漏洞仍在。
- **Fix（结构修复，`planner/mod.rs` + `planner/executor.rs`）**：提取 `PlanGenerator::force_correct_first_step_to_writer` 为 `pub(crate)` 方法（封装 swap + understanding/purpose 标注），在 `generate_plan` 与 **plan 执行咽喉点** `execute_with_context`（所有 plan 来源 SING/PlanGenerator/fallback 必经，`execute_plan` 之前）**统一施加**。SING 路径产生的 `builtin.style_enhancer`/`inspector`/`outline_planner` 等首步经咽喉点修正为 `writer`。幂等：已为 writer 的首步不受影响，两处重复调用安全。
- **验证**：`cargo test --lib` 948 passed（+4 咽喉点回归）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550 -> 549）。

### v0.30.12 - 续写返回审查报告修复（force-correction 漏拦 inspector）

- **根因**：planner force-correction（`planner/mod.rs` 防线 2）的"强制改 writer"capability 列表含 `outline_planner`/`style_mimic`/`plot_analyzer`/`builtin.*`，**漏掉 `inspector`**；提示词 Rule 9 允许"有内容时用 inspector 先审后写"、Rule 21 never-use 列表也漏 inspector。本地模型（Gemma-4-31B）把"继续写当前这部小说"误判为"审查/改进已有文本"路由到 `inspector` -> force-correction 不拦 -> inspector 运行 `inspector_system` 提示词 -> 产出"总体评分 0.85 / 具体问题清单"审查报告作为生成结果。
- **Fix A（force-correction 主修复）**：提取纯函数 `PlanGenerator::should_force_correct_to_writer`（可单测），将 `inspector` 纳入 swap-to-writer 列表，按 LLM 分类分流：续写（`is_continuation`）/ 创世 / 无分类 / 审查且 `is_prose_request=true`（分类矛盾兜底）强制 `writer`；仅纯审查（`Audit` 且非 prose）与改写润色（`Rewrite`，Rule 9 流，最终输出仍是 writer 正文）保留 `inspector`。
- **Fix B（提示词）**：Rule 9 澄清"继续写/续写/往下写"是续写而非 refine，必须直接 `writer`、绝不用 `inspector`；Rule 21 将 `inspector` 加入 prose 请求 never-use 列表。
- **验证**：`cargo test --lib` 944 passed（+8 force-correction 回归）；`npx vitest run` 305 passed；tsc / fmt / clippy / format:check 全绿。

### v0.30.11 - 全面整改：用 LLM 解析器替换朴素子串意图匹配

- **背景**：审计全项目发现 ~30 处 `.contains()`/`.includes()` 朴素子串匹配，其中 6 处高危直接在用户自然语言输入上做意图路由（`find_match`、`is_novel_creation_intent` 前后端、`from_instruction_and_context`、force-correction、`synthesize_query_rule_based`），是 v0.30.10 `PlanTemplateLibrary` bug 的同类。用户指示用 LLM 解析器替代。
- **核心：`IntentParser::classify_writing_intent`（intent.rs）**：一次 LLM 调用产出 `WritingIntentClassification`（is_new_novel / is_continuation / task_type / is_prose_request / input_clarity / detected_genre / confidence）。最快模型 + 8s 超时 + 保守兜底（is_new_novel=false=续写）+ 会话 LRU 缓存。误判代价不对称：误判续写为创世会启动 Agency 全流程覆盖工作（灾难），故默认偏向续写。
- **Site 4**：`smart_execute` 用 `classification.is_new_novel` 替代 `is_novel_creation_intent`；前端 `classify_intent` IPC 先行，payload 透传分类，后端信任不重复调用。
- **Site 1**：`find_template` 禁用（恒返回 None，patterns 来自 LLM understanding 切词噪声）；`find_match` 标 `#[allow(dead_code)]`。
- **Site 4b/5**：TriShot 守卫 + 续写绕过 + force-correction 读 `PlanContext.intent_classification`（无新 LLM 调用）。
- **Site 3**：`from_instruction_and_context` 修运算符优先级 bug + 移除单字 pattern + `hint` 参数经 `task.parameters["task_type_hint"]` 透传。
- **Site 8**：`build_writer_prompt` 题材优先 LLM `detected_genre` > `extract_genre`（加否定窗口 + 长度降序）> 故事 genre。
- **Site 7**：`detect_input_clarity` 移除单字信号；调用方读 `classification.input_clarity`。
- **Site 2**：`intention_graph::builder` LLM 主路径硬化（JSON 子串截取 + raw_input 推断）+ 规则兜底默认 `generate prose`。
- **前端**：新增 `classifyIntent` API；`handleSmartGeneration` 入口调分类（缓存 + 兜底）；删除 `isNovelCreationIntent`/`isContinuationIntent`。
- **字段名 bug**：prompt 指示返回 `"is_prose"` 但 struct 字段 `is_prose_request` 无 alias 致恒 false；加 `#[serde(alias = "is_prose")]` 修复（单测捕获）。
- **不适用 LLM（诚实标注）**：Site 9 `derive_model_role_from_label`（内部 label）、Site 10 `discover_from_outputs`（LLM 输出，需结构化 findings 改造）保留为后续。
- **验证**：`cargo test --lib` 936 passed（+1）；`npx vitest run` 305 passed；tsc / fmt / format:check / clippy / architecture_guard 全绿。

### v0.30.10 - 续写返回风格增强模板修复（模板匹配误路由 + content 空兜底）

- **根因**：`PlanTemplateLibrary::find_match` 用朴素 substring 匹配（`user_input.contains(pattern)`），之前记录的 style_enhancer 计划的触发词（如"这部小说"）会匹配"继续写当前这部小说"，导致续写请求**跳过 planner LLM 和所有安全规则**，直接重放 style_enhancer 计划。style_enhancer 收到空 content 后返回"在您提供文本后，我将从以下几个方面进行增强"模板而非续写正文。
- **Fix A（executor.rs 主修复）**：`execute_with_context` 在 `find_template` 前检测续写意图词（继续/续写/接着写/往下写/接下来/后续/接着），命中则跳过模板匹配，强制走 planner LLM 路径，确保续写请求由 Rules 8/19/21 正确路由到 writer。
- **Fix B（mod.rs 防线 2 扩展）**：force-correction 从仅捕获 `outline_planner` 扩展到 `style_mimic` / `plot_analyzer` / `builtin.style_enhancer` / `builtin.text_formatter` / `builtin.character_voice` / `builtin.emotion_pacing`，当首步为这些 capability 且输入含写作/续写关键词时强制改为 `writer`。
- **Fix C（executor.rs content 兜底）**：新增 `inject_content_fallback` 静态方法，为 `style_mimic` / `plot_analyzer` / `builtin.*` 技能在 content 为空时按 depends_on -> step_outputs -> plan_context.current_content_preview 顺序注入文本，与 v0.30.9 inspector draft 兜底同理。
- **Fix D（mod.rs Rule 21 强化）**：Rule 21 新增"继续"/"续写"关键词和"这部"/"当前"故事相关主语，并明确禁止 `style_mimic` / `plot_analyzer` / `builtin.style_enhancer` 用于 prose 请求。
- **验证**：`cargo test --lib` 929 passed（+5：content 兜底注入 5 场景）；fmt / clippy 无新增告警。

### v0.30.9 - 续写返回 Inspector 审查模板修复（draft 空内容兜底注入）

- **根因**：legacy planner 的 LLM 生成的 ExecutionPlan 中 inspector 步骤常遗漏 `"draft": "{{step_N}}"` 参数。`execute_inspector` 仅从 `params["draft"]` 读取待检查正文，缺失时 `task.input` 为空串，`build_inspector_prompt` 渲染出"【待检查内容】部分为空"的模板文本，Inspector 直接将该模板作为"审查结果"返回，用户看到审查模板而非续写正文。
- **Fix A（主修复·executor.rs）**：`resolved_params` 块新增 inspector draft 兜底注入--当 `capability_id == "inspector"` 且 `draft` 为空时，按 `depends_on` 顺序查找 writer 步骤的 `step_outputs["content"]`，找不到则扫描全部 `step_outputs`，自动注入非空 content 作为 `draft`。提取为可测静态方法 `inject_inspector_draft_fallback`。
- **Fix B（提示词·mod.rs）**：planner 提示词 Rule 9 强化--明确要求 inspector 必须使用 `"draft": "{{step_id}}"` 传参，否则 inspector 收到空内容只返回请求输入的模板；JSON 示例增加 inspector 步骤示范 `"draft": "{{step_1}}"` + `depends_on: ["step_1"]`。
- **验证**：`cargo test --lib` 924 passed（+5：inspector draft 兜底注入 5 场景）；fmt / clippy 无新增告警。

### v0.30.8 - 全面修复 nullable 列读取（Invalid column type Null 系列）

- **根因**：`world_buildings.cultures`（index 5）和 `rules`（index 3）在基础 schema 为 nullable TEXT，旧数据该列为 NULL，repository 用 `row.get(N)?` 读非空 `String` 即报 `Invalid column type Null`。与 v0.30.6 `dynamic_traits` NULL 同类。
- **全面排查**：系统性审查全部 27 个 repository 文件，发现并修复所有 nullable 列被当作非空 `String` 读取的问题（共 8 个文件、31 处）：`world_building_repository`（cultures/rules）、`scene_repository`（characters_present/character_conflicts × 4 方法）、`scene_version_repository`（同上 × 2 方法）、`studio_config_repository`（llm_config/ui_config/agent_bots）、`writing_style_repository`（custom_rules）、`knowledge_graph_repository`（attributes × 4 / evidence × 2）、`user_preference_repository`（6 列 × 2 方法）。全部改为 `Option<String>` + `unwrap_or_default`/`unwrap_or_else` 兜底。
- **迁移**：V112 回填 `world_buildings.cultures/rules`；V113 全面回填 scenes/scene_versions/studio_configs/writing_styles/kg_entities/kg_relations/user_preferences 的所有 nullable JSON/TEXT 列。
- **验证**：`cargo test --lib` 919 passed（+2：world_buildings NULL 兜底 + 合法 JSON 解析）；fmt / architecture_guard 全绿。

### v0.30.7 - 计划执行失败修复（LLM 在 depends_on 写入上下文名）

- **根因**：LLM 生成的 ExecutionPlan 在 `depends_on` 中混入上下文名（如 `"Story Context"`、`"writer"`）而非 plan 内 step_id。`topological_sort`（swarm.rs）已正确跳过非 step_id 依赖，但 `PlanExecutor::execute` 的依赖校验未对齐--遇到非 step_id 依赖直接判 `not found`，导致 step_1 被跳过 -> step_2 依赖 step_1 也 not found -> step_3 链式失败，整 plan 崩溃。
- **Fix（executor.rs）**：依赖校验前收集 `plan_step_ids` 集合，对不在集合中的依赖（非 step_id）跳过校验并 `log::warn`，与 `topological_sort` 行为一致；仅校验真实 step_id 依赖是否已产出。参数引用 `{{step_id}}` 由 `resolve_parameters` 兜底处理缺失键。
- **Fix（mod.rs）**：Rule 3 强化--明确 `depends_on` MUST ONLY contain step_id values of OTHER steps in this same plan，NEVER put context names / capability names / free text，并举例 `"Story Context"` / `"writer"` 为错误值。
- **验证**：`cargo test --lib` 917 passed（+2：topological_sort 非 step_id 依赖跳过 + 混合依赖排序）；fmt / tsc / architecture_guard 全绿。

### v0.30.6 - 获取角色失败修复（dynamic_traits 列 NULL）

- **根因**：`characters.dynamic_traits` 列在基础 schema 为 nullable TEXT（无 `NOT NULL`/`DEFAULT`），StoryForge 数据迁移导入的旧角色行该列为 NULL。`get_by_story`/`get_by_id` 用 `row.get::<_, String>(9)` 读非空类型，遇 NULL 即报 `Invalid column type Null at index: 9, name: dynamic_traits`，续写/创世获取角色失败弹 Fatal 诊断卡片。
- **修复（双层）**：读取层 `get_by_story`/`get_by_id` 改读 `Option<String>` 兜底 `"[]"`（NULL 行返回空 `dynamic_traits`）；数据层 V111 迁移回填 `characters.dynamic_traits NULL -> '[]'` 保证一致。
- **验证**：`cargo test --lib` 915 passed（+2：NULL 兜底 + 合法 JSON 解析回归）；fmt/clippy 无新增告警。

### v0.30.4 - 幕前输入历史持久化（按故事隔离）

- **功能**：幕前底部输入框已输入内容现长久保留，关闭窗口/重启后不丢失，与编码工具一致。每条提交按故事 ID 隔离存入 `localStorage`（`frontstage:inputHistory:<storyId>`，最近 20 条），切换故事自动加载该故事的历史。
- **UX**：保留既有 ghost-hint 交互（↑/↓ 切换 LLM 建议 <-> 历史记录，-> 确认填充），持久化对导航无侵入。localStorage 不可用时静默降级为内存态。
- **实现**：`src-frontend/src/frontstage/FrontstageApp.tsx`（模块级 `loadInputHistory`/`saveInputHistory` + `useEffect` 加载 + `handleInputSubmit` 同步持久化）。
- **验证**：`npx vitest run` 297 passed（+2：持久化写入 + 重载召回）；tsc / prettier 通过。纯前端，无 Rust 变更。

### v0.30.5 - 创世流程严重超时修复（600s 顶满 + 前端先杀后端）

- **根因（对照 `creative_workflow.log` 2026-07-20 08:37–08:47）**：Agency 创世 5 阶段慢，producer tool_loop 5.5min + writer tool_loop 4.5min（含本地模型连接超时 60s×4 候选=240s）顶满 600s；前端 `Promise.race` 600s 到了先 `llm_cancel_all_generations` 杀掉后端，创世被 CANCELLATION 砍掉无产出 + 僵尸 run 卡死故事续写；writer 在 tool_loop 中盲目 board_read 轮询 7-10 轮。
- **Fix 1**：`config/commands.rs` 放开 `smart_execute_total_timeout_secs` / `frontend_timeout_secs` clamp 上限 600->1800（默认仍 600s）；`GeneralSettings.tsx` 输入框 max 同步到 1800。
- **Fix 2**：`FrontstageApp.tsx` 创世路径前端超时 = 后端 + 30s 缓冲（主超时 + 看门狗 + 诊断卡片三处统一）；提取纯函数 `utils/genesisTimeout.ts`。
- **Fix 3（核心）**：`coordinator.rs` 新增 `asset_retrieval_plan`--writer 前置单次 LLM 调用从资产 catalog 选出必需 key（30s 超时 + 失败兜底全量 + `RetrievalPlan` 别名兼容），消除 writer 多轮 board_read 轮询。
- **Fix 4**：`coordinator.rs` 新增 `build_writer_assets_context`--检索规划后按 key 过滤资产全文预注入 writer task（8000 字符预算截断），tool_loop 轮次从 7-10 降到 1-2。
- **Fix 5**：`tool_loop.rs` 新增 run 级 deadline 感知（`with_deadline` + 每轮检查，剩余 <30s 熔断保产出）；新增 `LoopAbortReason` 枚举，`circuit_break_reason` 识别 deadline 熔断返回"剩余时间不足"，coordinator writer 路径据此快速失败而非回退 legacy（避免 legacy 又跑一遍超时）。
- **验证**：`cargo test --lib` 913 passed（+14）；`npx vitest run` 305 passed（+8：genesisTimeout 纯函数）；tsc / fmt / clippy / format:check / architecture_guard 全绿。

### v0.30.3 - 创世主创 Agent 熔断修复（本地模型 JSON 不遵从）

- **根因**：本地模型（Qwen/Gemma）对 `producer_depth_assets` 的 `complete_json` 返回散文而非 JSON -> 快速路径失败回退 legacy -> legacy writer tool_loop 要求 JSON action 而模型写散文 -> 连续 3 轮解析失败熔断，首章未完成。
- **Fix A（主修复）**：`producer_depth_assets` 在 `parse_lenient` 失败时兜底 salvage 散文为 world 资产，快速路径继续，避免回退 legacy。
- **Fix B（可诊断性）**：`tool_loop.rs` 此前零条日志，解析失败 raw 响应只存在内存 run 结束即丢弃。现每轮解析失败 + 熔断点 + max-turns 均 `log::warn!`（含 role、轮次、截断 raw 500 字）。
- **Fix C（纵深防御）**：legacy writer "连续解析失败"熔断时回退自由体散文单调用（新 `writer_prose_fallback`），"达到最大轮数"仍直接 Err。
- **验证**：`cargo test --lib` 899 passed（+2 新测试）；fmt/clippy 通过。

### Agency 多代理创作框架 P1 — 创世 2.0 骨架（串行端到端）

- **新模块**：`src-tauri/src/agency/`（board 黑板 / tool_loop ReAct 工具循环 / roles 三角色 / coordinator 协调器（P2 起含并行稳态循环 gate(n-1)∥writer(n)）/ repository+models 持久化 / bus 消息总线（P2 已接线：修订提案 proposal）/ budget 角色预算 / commands IPC）。
- **IPC**：`agency_start_genesis` / `agency_get_run` / `agency_list_board` / `agency_cancel_run` / `agency_continue_chapter` / `agency_continue_batch`。
- **提示词目录**：`resources/prompts/agency/`。
- **依赖边界**：agency 允许依赖 db/llm/router/prompts，不允许被反向依赖。
- **教训**：迁移文件必须与引用它的代码同一 commit 提交（P3 T5 教训：V109 与测试被并行 CI 提交拆散导致断档）。
- 设计：`docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（P1-P3 已完成，除真机验收外）。

### v0.26.59 — StoryForge → StoryMoss 品牌收尾，官网落地页上线

- **品牌重命名**：完成仓库文档、CI、Tauri 配置与 GitHub Release 的 StoryForge → StoryMoss 全局替换。
- **官网落地页**：`landing/` 站点部署至 `https://ai.91z.net`，重写产品介绍并加入 Logo；下载按钮按平台自动匹配安装包。
- **验证**：landing 19 tests passed。

### v0.26.58 — 修复 OpenAI/Deepseek 模型因 top_p=0 健康检测失败

- **根因**：OpenAI 兼容 API（含 Deepseek）不接受 `top_p = 0.0`，会返回 `Invalid top_p value`。
- **修复**：`OpenAiAdapter` 在序列化前过滤 `top_p`，仅保留 `(0, 1.0]` 的合法值；非法值自动省略，让服务端使用默认值。
- **验证**：新增 `llm::openai` 单元测试；`cargo test --lib` 770 passed。

### v0.26.57 — 自动划分章节、本地导出保存与提示词目录

- **自动划分章节**：后台设置新增「按字数 / 按情节」分章策略；字数上限留空默认 3000 字；场景保存空闲 30s 后仅对最新章自动切分。
- **本地导出保存**：导出结果通过系统保存对话框落盘；文本写 UTF-8，二进制（pdf/epub）复制后端临时文件；取消时不关闭弹窗。
- **提示词目录**：提示词注册表新增「打开目录」按钮，直接用系统文件管理器打开 bundled prompts 目录；编辑器改用原生 textarea 避免 CSP 导致 Loading。
- **验证**：`cargo test --lib` 769 passed；`npx vitest run` 292 passed；tsc / fmt / format:check 全绿。

### v0.26.56 — 网关契约测试串行化（mock app_data_dir）

- **修复**：写 AppConfig 的 executor 契约测试加进程锁，避免并行污染导致 `creative_x_overrides` 偶发失败。
- **验证**：`creative_x_overrides` / `demoted_degraded` / `sticky_unhealthy` / `disabled_model` 并行 `--test-threads=8` 通过。

### v0.26.55 — 幕后模型列表开启/关闭开关

- **UI**：模型卡片「开启/关闭」；仅轮询已启用模型。
- **运行时**：复用 v0.26.54 fail-closed；`is_promotable` 要求仍在注册表。
- **验证**：ModelCard vitest + disable/probe Rust 契约。

### v0.26.54 — 修复创作模型被粘性降级绕过

- **根因**：Deepseek 已是创作/活跃，但连续失败 demotion 让 `resolve_role_model` 丢弃显式角色，Call3 长期用 MN-Oblivion。
- **修复**：显式角色不受粘性 demotion；Unhealthy 在 resolve 清一次再探；`set_active_model`/`save_settings` 清 demotion；`generate()` 用 `is_promotable`；禁用模型 fail-closed（持久化 enabled、不探测、活跃自动回退）。
- **验证**：gateway/health/commands 契约通过；architecture_guard。

### v0.26.53 — 故事名取消单击回幕后（双击改名可用）

- **修复**：故事名不再单击打开幕后（与双击改名冲突）；回幕后走设置按钮（禅模式也保留）。
- **验证**：Header 单击不调 `onOpenBackstage`；设置按钮可回幕后；双击仍进编辑。

### v0.26.52 — 修复模型新增与默认创作模型即时生效

- **幕前连接状态**：`model_config`/`app_settings` 刷新同步失效 `gateway-status`；状态栏含 `Unknown`。
- **创作模型**：用户显式角色允许 Unknown 置顶；`set_active_model(creative)` / `save_settings` 同步 `active_llm_profile`。
- **验证**：Rust 4 + vitest 5；tsc/fmt/architecture_guard。

### v0.26.51 — 幕前故事名与章节名内联改名

- **故事名**：草苔/未命名展示；有正文自动建「未命名」故事；双击改名。
- **章节名**：编辑器上方 + 顶栏状态统一双击改名；空标题 `第N章`；`update_scene` 持久化。
- **验证**：displayStoryTitle/ChapterTitle + Header/EditableChapterTitle 相关测试；tsc/format/architecture_guard。

### v0.26.50 — 修复打字触发后台运行与深度思考假超时

- **AutoIngest 防抖**：打字自动保存不再立刻抢本地模型（30s + BACKGROUND_LLM_SEMAPHORE）。
- **合同补齐静默**：不再用 `contract-auto-progress` 拉高 `isGenerating`。
- **活动同步**：后台活动不得单独禁用输入栏；`isGenerating` 超时看门狗强制弹诊断。
- **验证**：scene_service 6；contract gate 2。

### v0.26.49 — 修复续写与正文脱节（末句硬锚点）

- Call3/TimeSliced 在 prompt 最末尾注入末 2 句硬锚点，覆盖「开场」类大纲指令；抗 Lost-in-the-Middle。
- **验证**：ending_anchor 相关 3 passed。

### v0.26.48 — 修复自动更新（GitHub Releases + latest.json）

- 开启 `createUpdaterArtifacts`；CI 产出签名更新包与 `latest.json`；Linux AppImage；下载进度累加与 404 提示。
- **验证**：`cargo test --lib updater::` 2 passed。

### v0.26.47 — CI 热修复（Rust fmt）

- `cargo +nightly fmt` 修复 v0.26.46 rust-check 失败；无逻辑变更。

### v0.26.46 — 创世方法论全链路、题材 match-or-create 与拆书持久化

- **方法论**：5 个 background 模板恢复 `strategy_section`；Genesis 分步注入 + `methodology_step` 推进；ID 归一化；Selector 预填 `recommended_methodology_id`。
- **题材**：`EnsureGenreProfileStep` match-or-create；概念保真硬化。
- **拆书**：StoryArc/作者/伏笔落库；分块超时与并发止血；前端按书过滤进度。
- **验证**：genesis/methodology/prompt 契约 20+ passed。

### v0.26.45 — Genesis 人物卡强制落地（姓名 + 欲望/阻力）

- **人物卡**：merge/render/probe 纯函数；first_scene + Call3 双重注入；真名≥80%、欲/阻信号探针；软重试 fail-open。
- **验证**：narrative 61；protagonist_card 6。

### v0.26.44 — Genesis 首章质量：开篇骨架与提示词加厚

- **开篇骨架**：quick_phase 四步（概念→策略→骨架→开篇）；10s 超时 fail-open；概念字段规则映射降级。
- **提示词**：概念加厚（主角/冲突/世界锚点）；strategy_selector 中文化；first_scene 纪律单源化。
- **四元组 + 占位角色**：Genesis 接入 `infer_narrative_quartet`；TriShot 占位用骨架主角，去掉「异星末世」硬编码。
- **验证**：`narrative::genesis` 12 passed；骨架解析契约 +1。

### v0.26.43 — 修复底部状态栏 emoji 显示为方框

- **根因**：阶段文案嵌入 emoji + 解析正则拆碎中文/代理对；WebView 缺字显示 □□。
- **修复**：纯文案 + `StatusIcon`（Lucide）；解析前剥 emoji。
- **验证**：StatusIcon / FrontstageBottomBar 相关 18 passed。

### v0.26.42 — 修复续写 Tab 提示可见但无幽灵文本

- **根因**：Tab 接受后 30s `hideGhostUntil` / `postAcceptHideUntilRef` 未在新续写时清零；幽灵树仍渲染 Tab 条，幽灵段落被压住。
- **修复**：续写入口与 `setGeneratedText` 清零父级锁；RichTextEditor 新幽灵到达时清零本地锁（接受中不解除）。
- **验证**：`RichTextEditor.duplicate.test.tsx` 6 passed（+1）。

### v0.26.41 — 记忆统一读模型与 Finalize scene_id 根治

- **Finalize**：`scene_id` 贯穿 drafts/IPC/UI；直写编辑场景。
- **记忆**：`story_memory_facts` VIEW + `kg_entity_id` 链接；`list_unified_facts`；表不 DROP。
- **验证**：cargo 701；facade 7；finalize 3；vitest 261。

### v0.26.40 — 幕后资产闭环 P0–P3

- **P0**：侧栏热/温/冷/配徽章；合同/KG 生成影响说明；诊断组默认折叠。
- **P1**：SceneEditor 管线轨；KG 摘要进 WriteTimeBundle；MCP→设置扩展；Wizard 幂等+KG（既有）。
- **P2**：MemoryFacade；quality_gate 永不热路径 LLM。
- **P3**：TracingPanel 资产→prompt 覆盖率。

### v0.26.39 — 幕后信息架构全面重排

- **侧栏五组**：创作 / 故事资产 / 创作工具 / 洞察与运维 / 系统；中文重命名。
- **数据洞察**：合并用量/写作/功能使用；设置七 Tab 重组；拆书设置就近；账号死链修复。
- **验证**：`npx vitest run` 249 passed / 3 skipped；tsc/format 通过。

### v0.26.38 — 提示词面板修复与组合智能化

- **修复 Loading / 打开目录 / 导出**：Monaco CDN → textarea；`open_prompts_directory` 原生打开；dialog+fs 导出覆盖/完整包。
- **接通 FrameworkSelections**：methodology + contextual_injectors 确定性回灌 Call 3（0 额外 LLM）。
- **场景组合预览**：`preview_prompt_composition` + 面板分层跳转。
- **验证**：`cargo test --lib` 690 passed；`npx vitest run` 244 passed / 3 skipped；fmt、format、architecture_guard 均通过。

### v0.26.37 — 修复幕前「保存中」常亮与字数不更新

- **根因**：`update_scene` IPC 参数形状错误 + `appendAiContent` 不刷新字数/不调度保存。
- **修复**：`buildUpdateSceneIpcArgs`；追加后同步 `wordCount` + `scheduleAutoSave`。
- **验证**：vitest 242 passed / 3 skipped。

### v0.26.36 — 后台配置变更即时生效（超时/字体/主题）

- **超时热重载**：`save_settings` → `reload_config` + `app_settings` sync；幕前立刻用新超时。
- **首字节超时**：`llm_first_chunk_timeout_secs` 接入三适配器。
- **字体/主题跨窗口**：Tauri 事件 `editor-config-changed` / `color-theme-changed`。
- **验证**：cargo test 685；vitest 240 passed / 3 skipped。

### v0.26.35 — 全面落地幕后工作室审计残留 R1–R11

- **R1**：`list_stories` 返回真实 `scene_count`；Dashboard「场景」统计对齐。
- **R2**：CreationPathGuide 快速创作绑定 `runCreationWorkflow`；`App` 导航统一 `appStore.currentView`。
- **R3**：后端 `apply_wizard_to_story`（角色去重、首场景 upsert、KG 摄取）；前端单 IPC。
- **R4**：幕后 `App`/`GenesisPanel` 监听 `genesis-warnings`。
- **R5/R6**：PipelinePanel / SceneEditor 标注场景序号语义。
- **R7–R11**：世界构建文风 Tab、UsageStats 启发式、伏笔 Kanban、角色→场景跳转、拆书转故事导航。
- **验证**：`cargo test --lib` 685 passed；`npx vitest run` 237 passed / 3 skipped；fmt、format、architecture_guard、tsc 均通过。

### v0.26.34 — 修复提示词导入参数并新增「打开本地目录」功能

- **修复批量导入静默失败**：`PromptsPanel.handleImportAll` 调用 `save_prompt_override` 时参数键由错误的 `promptId` 修正为 `prompt_id`，与后端 `rename_all = "snake_case"` 对齐。
- **新增「打开目录」功能**：后端新增 `get_prompts_directory` 命令暴露当前 prompts 资源目录；前端标题栏新增「打开目录」按钮，使用系统文件管理器打开目录。
- **新增「刷新」功能**：支持重新加载提示词列表与目录路径。
- **改善错误展示**：加载失败时在页面上方显示具体错误信息。
- **导出/导入按钮归位**：将「导出」「导入」按钮从「全部重置」确认弹窗移至页面标题栏。
- **验证**：`cargo test --lib` 685 passed；`npx vitest run` 237 passed / 3 skipped；fmt、format、architecture_guard 均通过。

### v0.26.24 — 修复续写重复、截断与跨内容复述（5 项根因）

对照 `creative_workflow.log` 2026-07-07 08:44–09:05 续写会话（新写 → 多次续写）：

- **散布式句子块重复**：新增 `trimInterspersedRepeatedBlocks`（Rust + TS 对齐，golden 双跑），处理单次生成内意象循环重复。
- **跨内容重叠复述**：新增 `stripExistingOverlap`，剥离 Writer 复述已有正文段落（`startsWith` / `isTextDuplicate` 无法拦截的路径）。
- **截断末句污染**：新增 `trimDanglingTail`，裁掉 60s 超时硬截断留下的极短半句。
- **续写 8% 重试闸门**：TriShot 续写路径补齐 anti-repeat 重试（对齐 Genesis）。
- **前端管线统一**：`sanitizeContinuationOutput` 覆盖 smart_execute / appendAiContent / handleRequestGeneration。

### v0.26.23 — 修复续写卡死与幽灵文本混乱（4 项根因）

对照 `creative_workflow.log` 2026-07-07 续写会话时间线定位 4 个根因：

- **Bug B（卡死主因）**：`auto_contract` 4 个 LLM label（master_setting/chapter/scene_outline/default_character）加入 `is_silent_background`，后台补齐合同不再阻塞 `isAnyBackendActive`（原 6 分钟阻塞续写）。
- **Bug D（混乱主因）**：`handleSmartGeneration` 入口加重入守卫——存在未接受幽灵时先丢弃并提示，避免新旧续写结果竞争。
- **Bug A**：`RichTextEditor` 新增 `bodyForceHideGhost` state 镜像 `force-hide-ghost` 类，移除类时触发重渲染，消除幽灵 10s 渲染延迟。
- **Bug C**：续写（非创世首章）call3 超时上限 120s→60s，慢模型 fail-fast 回退到快模型（Gemma4 10s vs MN-Oblivion 198s）。

### v0.26.21 — 修复 Windows MSI 构建（迁移文件名重命名）

- v0.26.17 起打包 `src/db/migrations/` 为 Tauri resource，但 24 个迁移文件名含中文/全角逗号/破折号且最长 102 字符，导致 WiX `light.exe` 标识符生成失败（v0.26.14/v0.26.16 resources 引入前 Windows MSI 曾成功）。
- 重命名 24 个迁移文件为 ASCII 短名（保留 `V###` 前缀与排序）。`schema_migrations` 按 version 跟踪，已应用迁移不受影响；`parse_filename` 仅解析 `V###` 前缀，无逻辑变更。
- v0.26.20 尝试的 `wix.language: zh-CN` 无效（问题在标识符生成而非代码页）。

### v0.26.20 — 修复 v0.26.19 CI 格式检查失败

- `ParallelWorldOutlineCharacterStep` doc 注释超 `max_width=100`，`cargo +nightly fmt` 自动换行。仅注释格式变更。
- macOS 公证随 Apple Developer 协议续签恢复成功。

### v0.26.19 — Genesis 创世流程全面审计与测试加固

对照项目文档对「智能创作流程-创世」全面审计，分 Phase 1–4 执行：

- **Phase 1（P0 竞态与契约）**：
  - **Gap B**：`isFirstChapterReady` 路径在 `finalContent` 为空时不锁 `delivered`，避免编辑器永久空白。
  - **P0-2 角色世界观上下文**：`ParallelWorldOutlineCharacterStep` 中 character 提示词读取 `bundle.world_building` 恒为空（闭包捕获竞态），改为先 await world 拿真实 `world_concept` 再构造 character；提取 `world_concept_for_character_prompt` 纯函数 + 单测。
  - **P0-3 ChapterSwitch delivered 时序**：`selectChapter` 懒加载失败时不标记 `delivered`（`markDeliveredOnLoad` 仅在 `setContent` 成功后标记）。
- **Phase 2（P1 架构对齐）**：后台错误可观测性（`GenesisContext.errors` → `genesis_runs.steps_json` + `genesis-warnings` 事件 → 前端 toast）；mutex 中毒锁加固；策略移入 quick phase 经评估暂缓（记录为债务）；`window/mod.rs` 与 `FrontstageEvent.ts` 注释对齐 auto-accept 真实路径。
- **Phase 3（测试加固）**：8% 重试闸门 + ChapterSwitch payload 提取纯函数 + 契约测试；前端 Gap C + 状态机端点测试；**跨层共享 trim golden fixture**（`tests/fixtures/trim_golden.json`，Rust + TS 双跑锁定 `trim_self_repetition` 跨层一致性）。
- **Phase 4（代码整洁）**：`*_future` → `*_gen` 重命名；`AppConfig::load` 去重；`appendAiContent` skip 路径不 `markAccepted`；Gap C 重复入站也跳过 setContent；`isGenesisSettingUpRef` 合并评估——不合并（覆盖窗口不同）。
- **验证**：`cargo test --lib` 655 passed（+10）；`npx vitest run` 183 passed（+17）；`npx tsc --noEmit` 零错误；fmt 通过。

### v0.26.18 — Genesis 第一章重复：竞态路径加固

- **Gap A**：ChapterSwitch `auto_accept=true` 但 content 为空时 `skipContent=true` 且不标记 `delivered`，让 smart_execute 投递。
- **Gap B**：`isFirstChapterReady` 路径仅在已 append 或编辑器已有内容时标记 `delivered`。
- **Gap C**：`selectChapter` 咽喉点新增 `delivered` + 编辑器已有内容守卫，跳过 `setContent`。
- **回归测试**：新增 Gap A 竞态路径单测，vitest 167 passed。

### v0.26.17 — Issue #4 启动加固：打包 SQL 迁移与 init_db 诊断增强

- **打包 SQL 迁移**：Release 安装包包含 `$RESOURCE/db/migrations/`。
- **init_db 加固**：启动前确保 app data 目录；失败日志含 DB 路径；新增 fresh init 回归测试。

### v0.26.16 — 根治 Genesis 第一章重复、Issue #4 启动稳定性与代码格式修复

- **根治 Genesis 第一章内容重复**：替代 v0.26.7–v0.26.14 的散布布尔守卫补丁模式，从两个独立根因进行结构性修复。
  - **R2 生成侧验证闸门（`src-tauri/src/narrative/genesis.rs`）**：检测 LLM 输出自重复比例，≥8% 时用更强 anti-repeat 指令重试一次；prompt 模板新增「结构纪律」段，明确禁止首尾回环与整章重复。
  - **R1 前端单写者状态机（`src-frontend/src/frontstage/FrontstageApp.tsx`）**：将 `genesisAutoAcceptedRef` 布尔替换为 `idle → generating → delivered` 三态状态机；`generating` 态阻塞 `onChapterUpdated` 与 `loadStories` 自动选择；`delivered` 态阻塞 `setGeneratedText` 幽灵文本恢复。
  - `textCleanup` 提升到 `src-frontend/src/utils` 共享；Rust `trim_self_repetition` 对齐前端 KMP 最长 border 检测；全路径统一调用 `trimSelfRepetition`。
- **修复 Issue #4：init_db 失败时启动 panic/Windows 闪退**：`GatewayExecutor::new` 改为显式接收 `pool`，`setup` 仅在 pool 可用时初始化网关执行器，避免 `state::<DbPool>()` 在启动时 panic；新增不可写目录回归测试。
- **修复 CI 格式检查失败**：`cargo +nightly fmt -- --check` 与 `npm run format:check` 现已通过。

### v0.26.14 — 修复 Genesis 第一章模型输出自重复与幕前诊断日志过载

- **修复 v0.26.13 仍被用户感知的「新写小说第一章内容重复」**：通过分析 `creative_workflow.log` 中 13:43 的完整链路，确认前端 `append_ai_done` 只触发一次、`append_text_check.occurrences=1`，**不是前端把内容追加了两次**；重复来自 LLM 生成的 613 字正文自身首尾段落重复。
- 新增 `trimSelfRepetition` 工具（`src/frontstage/utils/trimSelfRepetition.ts`）：
  - 段落级：检测「后半段 == 前半段」或「末段 == 首段」并裁剪。
  - 字符级：对归一化文本做 KMP 最长 border 检测，保守阈值（重复长度 ≥30 字符且 ≥ 全文 8%）下裁掉尾部重复前缀。
- 在 `FrontstageApp` 的 `appendAiContent` 以及 `smart_execute` 返回的 `finalContent` 进入编辑器/幽灵文本前调用自重复清理，覆盖 Genesis 自动接受、Tab 接受、ContentUpdate/AppendContent 等全部路径。
- **缓解「写完后过会儿页面崩溃」**：`RichTextEditor` 的 `frontstage:rich_editor_diag` 渲染诊断日志从每帧都记改为仅前 20 次渲染 + 幽灵文本/隐藏锁状态变化时记录，并将 IPC 日志节流从 50ms 收紧到 200ms，降低长时间写作或文思活跃模式下的 IPC 与日志压力。
- 新增 `trimSelfRepetition` 单元测试，覆盖首尾段落重复、整章重复、单段内 suffix 重复、短文本不裁剪等场景。

### v0.26.13 — 修复 Genesis 第一章渲染层视觉重复（幽灵容器残留）

- 修复 v0.26.12 仍偶发的「新写小说第一章内容重复」视觉问题：数据层只写一次，重复来自渲染层幽灵文本/空幽灵容器与正文同框。
- `RichTextEditor` 的 `shouldShowGhostTree` 改为仅在 `generatedText` 非空时渲染，避免生成中状态的空幽灵容器残留或复用旧内容。
- `FrontstageApp` Genesis 自动接受路径先 `setIsGenerating(false)`，确保幽灵树条件立即失效。
- 增强 `frontstage:rich_editor_diag` 诊断字段：`isGenerating`、`isHidingGhost`、`bodyHidingGhost`、`generatedTextLen`。
- 增强 Playwright E2E 回归测试，新增自动接受后 `ghost-paragraph` 必须隐藏的断言。

### v0.26.12 — 修复角色列表为空/未加载时的幕前崩溃与订阅状态空值

- 修复 `useCharacters` 返回 `null` 或未加载完成时，`RichTextEditor`「角色名点击」effect 访问 `characters.length` 导致白屏崩溃的问题。
- `useSubscription` 增加空值防护，避免 `getSubscriptionStatus()` 返回 `null` 时产生错误日志。
- 新增 Playwright E2E 回归测试 `e2e/genesis-duplicate.spec.ts`，覆盖「已有故事 + 新写末世小说」完整流程。
- `frontstage/main.tsx` 与 `ErrorBoundary` 增强崩溃诊断输出。

### v0.26.11 — 修复 Genesis 第一章 store-editor 失步与崩溃隐患

- 修复 Genesis 自动接受第一章后，store 依赖 200ms onChange debounce 回写导致的 store-editor 失步。
- `appendAiContent` 追加后立即用 `editorRef.getHTML()` 同步 store 与 `latestContentRef`。
- `RichTextEditor.appendText` 空文档分支标记外部同步并更新 `lastExternalContentRef`，防止 content prop 被再次 setContent。
- `RichTextEditorRef` 新增 `getHTML()` 方法。
- 确认 `tauri.conf.json` `devUrl` 指向 dev server，避免开发时加载陈旧 dist 崩溃。

### v0.26.10 — 强化 Genesis 第一章重复防护（双重基准与追加最终防线）

- 修复 v0.26.9 单一 `latestContentRef` 基准与编辑器 DOM 短暂失步时，重复检测仍可能失效的问题。
- `isTextAlreadyInEditor`、`appendAiContent` 采用 `latestContentRef` + `editorRef.getText()` 双重基准。
- `appendAiContent` 增加正文前缀剥离安全网，并在追加后用 DOM 文本校准 `latestContentRef`。
- `RichTextEditor.appendText` 增加最终防线：编辑器尾部已包含待追加内容则直接跳过。

### v0.26.9 — 根治 Genesis 第一章重复（DOM 竞态与追加去重）

- 修复 TipTap DOM 状态滞后于 React `content` prop 时，重复检测依赖 `editorRef.getText()` 导致失效的问题。
- `isTextAlreadyInEditor`、`handleRequestGeneration`、`handleSmartGeneration`、`appendAiContent` 统一改用 `latestContentRef` 作为内容基准。
- `appendAiContent` 追加后立即同步 `latestContentRef`，避免 onChange debounce 窗口期内重复追加。
- `RichTextEditor` 幽灵文本直接包含检测剥离 HTML 标签，覆盖 ContentUpdate/AppendContent 路径。
- 新增 DOM 滞后竞态单元测试。

### v0.26.8 — 彻底修复 Genesis 第一章重复（竞态路径覆盖）

- 修复 `genesisAutoAcceptedRef` 无法覆盖 pipeline-complete 先加载 DB 正文竞态的问题。
- 新增 `isTextDuplicate` 归一化去重工具与 `isTextAlreadyInEditor` helper。
- `handleRequestGeneration` / `handleSmartGeneration` 设置幽灵文本前检测编辑器是否已包含生成内容。
- `pipeline-complete` 加载正文后标记 Genesis 已自动接受。

---

_最后更新: 2026-07-22 - v0.30.15_

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **StoryMoss** (19433 symbols, 39706 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "master"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({search_query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/StoryMoss/context` | Codebase overview, check index freshness |
| `gitnexus://repo/StoryMoss/clusters` | All functional areas |
| `gitnexus://repo/StoryMoss/processes` | All execution flows |
| `gitnexus://repo/StoryMoss/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
