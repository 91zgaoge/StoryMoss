# StoryForge 后台资产 × 智能创作流程 深度审计报告

> 审计日期：2026-06-20
> 审计范围：`src-tauri/src/` 全量后端资产（技能/能力/方法论/提示词/风格/网文资产/状态/记忆）与智能创作流程（`smart_execute` → `PlanGenerator` → `PlanExecutor` → `AgentOrchestrator`）的关联
> 审计方法：5 路并行代码追踪，覆盖能力层、方法论/提示词/风格层、网文资产层、状态/记忆层、创作流程端到端

---

## 0. 执行摘要（先读这一节）

### 核心结论：存在一个"资产丰盛但默认路径瘦身"的系统性矛盾

StoryForge 经过 v3 → v0.17 的持续建设，积累了**远超普通写作工具的后台资产**：52 种风格 DNA、5 种创作方法论、21 种剧情引擎、13 种高压关系、桥段卡库、体裁画像、知识图谱、规范状态、伏笔账本、记忆管线、自适应学习、技能系统、MCP 外部工具。这套资产的设计初衷是"越写越懂、AI 导演式创作"。

**但是审计发现：默认续写路径（TimeSliced）几乎绕过了全部这套资产。**

v0.14.3 的"场景智能路由"把续写钉死在 TimeSliced 模式（`planner/executor.rs:833-847`），而 TimeSliced（`agents/orchestrator.rs:608-823`）**完全绕过** `StoryContextBuilder::build` 与 `build_writer_prompt`，只使用一个极简的 `WriteTimeBundle`（合同红线 + 角色核心 + 场景大纲 + 体裁反模式）。结果是：

- 默认续写中，**约 90% 的后台资产不进入 LLM prompt**。
- 全套资产只在 Full 模式生效，而 Full 模式仅在"选中文本改写 / 创世首章 / 向导首场景"时触发。
- 用户日常"写第二章""继续写"这类最高频操作，恰恰落在资产最稀薄的路径上。

这是"创作专业性"与"生成速度"之间未解决的权衡——v0.14.3 为根治超时选择了速度，代价是默认路径丢失了几乎全部质量资产。

### 三类资产状态分布

| 状态 | 数量 | 含义 |
|------|------|------|
| ✅ 闭环（生成→消费） | 6 类 | 资产被维护并在创作流程中被读取注入 |
| ⚠️ 半闭环 / 有条件 | 5 类 | 仅在 Full 模式 / Pro 用户 / 特定条件下生效 |
| ❌ 断环 / 死代码 | 9 类 | 已定义但从未进入实时创作路径，或整模块死代码 |

详见第 3 节矩阵与第 4 节逐项分析。

---

## 1. 后台资产清单（按域分组）

### 1.1 能力层（Capability Layer）

| 资产 | 定义位置 | 注册点 | 实时执行点 | 状态 |
|------|----------|--------|------------|------|
| `builtin.style_enhancer` 文风增强 | `skills/builtin.rs:27` | `skills/mod.rs:246` → Capability `capabilities/mod.rs:658` | `agents/orchestrator.rs:1291`（Full/Fast 后处理） | ✅ 实时 |
| `builtin.emotion_pacing` 情感节奏 | `skills/builtin.rs` | 同上 → `capabilities/mod.rs:775` | `agents/orchestrator.rs:1264`（Full/Fast 后处理） | ✅ 实时 |
| `builtin.character_voice` 角色语音 | `skills/builtin.rs` | 同上 → `capabilities/mod.rs:739` | 仅 PlanGenerator 列出 + 手动 IPC | ❌ 创作流不调用 |
| `builtin.plot_twist` 情节反转 | `skills/builtin.rs:58` | 同上 → `capabilities/mod.rs:688` | 仅 PlanGenerator 列出 | ❌ 创作流不调用 |
| `builtin.text_formatter` 文本排版 | `skills/builtin.rs:89` | 同上 → `capabilities/mod.rs:718` | 仅 PlanGenerator 列出 | ❌ 创作流不调用 |
| Agent 能力（writer/inspector/outline_planner/style_mimic/plot_analyzer） | `capabilities/mod.rs:351-514` | `init_registry()` 静态 | `planner/executor.rs:502-509` 派发 | ✅ 实时 |
| 系统命令能力（create_story/chapter/character/update_*/query_kg） | `capabilities/mod.rs:560+` | `init_registry()` 静态 | `planner/executor.rs:502-513` 派发 | ✅ 实时 |
| 内置 MCP 工具（filesystem/text_processing/web_search） | `mcp/server.rs:156-194` | `register_built_in_tools` | **未自动注册进 CapabilityRegistry** | ❌ 半残 |
| 外部 MCP 服务器（用户连接） | `mcp/client.rs` | `commands/mcp.rs:22-33` 自动注册 | `planner/executor.rs:1536` | ✅ 实时 |
| Skill-runtime MCP（技能调用外部工具） | `skills/executor.rs:270-319` | — | 无内置技能使用 | ❌ 死基础设施 |
| Skill hooks 系统 | `skills/registry.rs:8` | — | 无技能声明 hooks，无触发点 | ❌ 死 |
| 能力进化反馈环 | `capabilities/evolution.rs:118` | `planner/executor.rs:373` record | 记录活，LLM 自动改写描述未定时触发 | ⚠️ 半活 |

### 1.2 方法论 / 提示词 / 风格层

| 资产 | 定义位置 | 实时注入点 | 状态 |
|------|----------|------------|------|
| `prompts/methodologies/` 雪花法10步/英雄之旅12阶段/场景结构3模板 | `prompts/methodologies/*.rs` | **无任何 use 引用** | ❌ 整目录死代码 |
| `creative_engine/methodology/` 5种方法论（雪花/场景节拍/英雄之旅/人物深度/高密度世界构建） | `creative_engine/methodology/*.rs` | `agents/service.rs:1956-1982`（**Pro only**） | ✅ 实时（Pro） |
| `writer_system` / `writer_continue` / `writer_rewrite` 模板 | `prompts/engine.rs:80-167` | `agents/service.rs:1852,2299,2303` | ✅ 实时 |
| `inspector_system` 模板 | `prompts/engine.rs:170` | `agents/service.rs:2324` | ✅ 实时（AgentService 路径） |
| `outline_planner` 模板 | `prompts/engine.rs:250` | `agents/service.rs:2340` | ✅ 实时（大纲生成） |
| `style_checker_system` 模板 | `prompts/engine.rs:276` | **无调用方**（实时走硬编码 prompt） | ❌ 注册未注入 |
| `commentator_system` 模板 | `prompts/engine.rs:306` | **无调用方**（实时走硬编码 prompt） | ❌ 注册未注入 |
| `PromptManager`（writing_chapter/analyze_plot） | `prompts/mod.rs:26` | 无 `new()` 调用 | ❌ 死代码 |
| `PromptEvolver` | `prompts/evolver.rs:20` | 无调用方 | ❌ 死代码 |
| StyleDNA 六维模型 + **52 种内置风格** | `creative_engine/style/dna.rs:17`；`classic_styles.rs:9`（12核心）+ `classic_styles_extended.rs:3241`（40扩展） | `agents/service.rs:1986-2034`（**Pro only**） | ✅ 实时（Pro） |
| StyleBlend 风格混合 | `creative_engine/style/blend.rs:47` | `context_builder.rs:1127` 预计算 → `service.rs:1987` | ✅ 实时（Pro + 配置） |
| StyleFingerprint 风格指纹 | `creative_engine/style/fingerprint.rs:87` | `agents/service.rs:2121-2163` | ✅ 实时（全版本） |
| Living Author Guard 在世作者保护 | `creative_engine/style/living_author_guard.rs` | `agents/service.rs:2275` | ✅ 实时 |
| WritingStrategy 写作策略（run_mode/conflict/pace/freedom） | `config/settings.rs:211` | `agents/service.rs:1858-1911` | ✅ 实时（Full） |
| StrategySelector LLM 资产选择 | `strategy/selector.rs:22` | **续写路径不调用**（仅测试） | ❌ 未启用 |
| `infer_narrative_quartet` 启发式四元组 | `strategy/quartet_inference.rs:15` | `commands/orchestrator.rs:941` | ✅ 实时 |

### 1.3 网文资产层（v0.17 中文叙事增强）

| 资产 | 数据来源 | DB 表 | 进入创作流程 | 状态 |
|------|----------|-------|--------------|------|
| 体裁画像 GenreProfile | `templates/genres.json` seed + 用户可编辑 | `genre_profiles`（mig 54/87/92） | ✅ 三处注入（genesis/TimeSliced反模式/四元组emotional_payoff） | ✅ 唯一全闭环 |
| reader_promise（体裁画像字段） | seed | `genre_profiles.reader_promise` | ✅ → emotional_payoff | ✅ 闭环 |
| 桥段卡 BeatCard | `builtin_beat_cards()` 硬编码 | `beat_cards`（mig 92）**从未读写** | ⚠️ 仅经四元组（vague/seeded 输入时） | ⚠️ 内存活/DB死 |
| 剧情引擎 StoryEngine（21种） | `builtin_story_engines()` 硬编码 | `story_engines`（mig 92）**从未读写** | ⚠️ 仅经四元组 | ⚠️ 内存活/DB死 |
| 高压关系 PressureRelationship（13种） | `builtin_pressure_relationships()` 硬编码 | `pressure_relationships`（mig 92）**从未读写** | ⚠️ 仅经四元组 | ⚠️ 内存活/DB死 |
| 追读力 ReadingPower | 规则计算 `reading_power/mod.rs:47` | `chapter_reading_power`/`chase_debt` | ❌ 写作 prompt 从不查询 | ❌ 仅审计/分析 |
| Anti-AI 五维审查 | `anti_ai/mod.rs:64` 规则 | 无（内存） | ❌ 写作 prompt 不读；仅经 evolve_style 间接回流 | ❌ 审计专用 |
| AntiAiRewriter 改写闸 | `anti_ai/rewriter.rs:69` | — | ❌ 骨架，返回原文不变 | ❌ 死骨架 |
| model_capability_profile 算力档案 | `model_gateway/executor.rs:256` 基准测试 | `model_capability_profile`（mig 91） | N/A（喂模型路由器，非创作资产） | — 非创作资产 |

### 1.4 状态 / 记忆层

| 资产 | 定义位置 | 进入创作流程 | 状态 |
|------|----------|--------------|------|
| CanonicalStateManager 规范状态 | `canonical_state/manager.rs:20` | ✅ `agents/service.rs:2166-2258`（叙事阶段/冲突/伏笔/角色状态） | ✅ 实时（Full） |
| KnowledgeGraphRepository 知识图谱 | `db/repositories.rs:1730` | ✅ 实体经 `context_builder.rs:1053` 进 scene_structure；关系经 canonical 冲突 | ✅ 实时（Full） |
| ForeshadowingTracker + PayoffLedger 伏笔账本 | `creative_engine/foreshadowing.rs`/`payoff_ledger.rs` | ✅ pending/overdue 经 canonical snapshot 注入 | ✅ 实时（Full） |
| MemoryOrchestrator 记忆包 | `memory/orchestrator.rs:119` | ✅ `context_builder.rs:471` → `service.rs:1800` | ✅ 实时（Full） |
| **QueryPipeline 五阶段管线**（token+语义+融合+图谱扩展+预算） | `memory/query.rs:15` | ❌ **`QueryPipeline::new` 零调用方** | ❌ 整模块死代码 |
| PromptPersonalizer 个性化 | `creative_engine/adaptive/personalizer.rs:24` | ✅ `service.rs:2083-2112`（**Pro only**） | ✅ 实时（Pro） |
| AdaptiveGenerator 自适应参数 | `creative_engine/adaptive/generator.rs:80` | ✅ `service.rs:1133-1191` 调整 temperature/max_tokens | ✅ 实时 |
| FeedbackRecorder/PreferenceMiner 反馈挖掘 | `creative_engine/adaptive/feedback.rs`/`miner.rs` | ✅ `commands/intent.rs:69` 触发 | ✅ 实时 |
| StoryStateManager | `state/manager.rs` | ❌ 文件头标注"RESERVED FOR FUTURE USE" | ❌ 死代码 |
| `evolution/` 模块（EvolutionReviewer/Analyzer/Updater） | `evolution/*.rs` | ❌ 零调用方 | ❌ 死代码 |
| `character_states` 表 | `canonical_state/manager.rs` 读取 | ⚠️ 读取注入但**运行时为空**（无写入方） | ⚠️ 结构活/数据空 |

---

## 2. 智能创作流程的两条路径（理解矩阵的前提）

```
smart_execute (commands/orchestrator.rs:31)
  ├─ 创世分支 → GenesisPipeline（不在本次审计范围）
  └─ 续写/智能创作分支
       → PlanContext 组装（手工拉轻量摘要）
       → PlanExecutor::execute_with_context
            → PlanGenerator（模型驱动，看 CapabilityRegistry 全量）
            → execute_step → execute_writer
                 → 场景路由 (executor.rs:833-847):
                    ├─ selected_text 非空（改写）→ Full 模式
                    └─ 续写/新章首段 → TimeSliced 模式（默认）
                         ├─【Full 路径】StoryContextBuilder::build（全量资产）
                         │   → build_writer_prompt（18 个 section 注入）
                         │   → Writer → Inspector → Rewrite 闭环
                         │   → apply_writing_skills（emotion_pacing + style_enhancer）
                         └─【TimeSliced 路径】WriteTimeBundle（极简资产）
                             → 直接 llm_service.generate（绕过 build_writer_prompt）
                             → 跳过 Inspector/Rewrite/Skills
                             → 后台异步 AuditExecutor（fire-and-forget）
```

**关键事实**：v0.14.3 后，普通续写默认走 TimeSliced；TimeSliced 是"资产瘦身"路径。

---

## 3. 资产可达性矩阵（核心交付物）

图例：✓ = 进入该路径的 LLM prompt；✗ = 不进入；⚠️ = 有条件/部分进入

| 资产类别 | (a) Full 续写 | (b) TimeSliced 续写【默认】 | (c) Plan 生成 | (d) Plan 执行派发 |
|----------|:---:|:---:|:---:|:---:|
| **Skills**（builtin.*） | ✓ emotion_pacing+style_enhancer（Full 后处理） | ✗ 明确跳过 | ⚠️ 仅 capability 列表列出 id | ✓ 经 execute_skill（仅当 plan step 为 builtin.*） |
| **MCP 工具** | ✗ | ✗ | ✓ mcp_tools_available 前5 | ✓ 经 execute_mcp_tool（仅当 plan step 为 mcp.*） |
| **方法论**（5种） | ✓ Pro only | ✗ | ⚠️ 仅 id，标"只读上下文" | ✗ |
| **StyleDNA / 风格混合** | ✓ Pro only | ✗（style_slice override 恒为 None） | ⚠️ 仅 style_dna_info 字符串 | ⚠️ 经 StoryContextBuilder（仅 Full） |
| **写作策略 config** | ✓ | ✗ | ✗ | ✗ |
| **体裁画像 GenreProfile** | ✓ | ⚠️ 仅 anti_patterns | ⚠️ 仅 id | ⚠️ |
| **桥段卡/剧情引擎/高压关系** | ⚠️ 仅经四元组（vague 输入时） | ✗ | ⚠️ 仅 id 不展开 | ⚠️ 序列化注入（仅 Full 读） |
| **知识图谱** | ✓ 实体进 scene_structure | ✗ | ✗ | ⚠️ 经 StoryContextBuilder（仅 Full） |
| **规范状态**（阶段/冲突/角色） | ✓ | ✗ | ⚠️ 仅 story_progress 字符串 | ⚠️ 经 StoryContextBuilder（仅 Full） |
| **伏笔 / 回收账本** | ✓ pending+overdue | ✗ | ⚠️ 仅前5 content | ⚠️ 经 StoryContextBuilder（仅 Full） |
| **记忆包** | ✓ | ✗ | ✗ | ⚠️ 经 StoryContextBuilder（仅 Full） |
| **用户个性化** | ✓ Pro only | ✗ | ✗ | ⚠️ 经 StoryContextBuilder（仅 Full） |
| **风格指纹** | ✓ | ✗ | ✗ | ✗ |
| **叙事结构**（act/position） | ✓ | ✗ | ✗ | ⚠️（仅 Full） |
| **前文摘要** | ✓ | ✗ | ✗ | ⚠️（仅 Full） |

**读法**：第 (b) 列几乎全 ✗，印证执行摘要的核心结论——默认续写路径资产稀薄。

---

## 4. 逐域深度发现

### 4.1 能力层：3/5 技能死在创作流，内置 MCP 半残

**发现 4.1.1 — 三个内置技能从未被创作流调用。**
`character_voice`、`plot_twist`、`text_formatter` 已注册进 CapabilityRegistry，PlanGenerator 能列出它们，PlanExecutor 能派发它们（`executor.rs:1021`），但 `AgentOrchestrator::apply_writing_skills`（`orchestrator.rs:1235`）硬编码只调 `emotion_pacing` + `style_enhancer`。这三个技能只有在 LLM 恰好在计划里生成 `builtin.character_voice` 步骤时才会跑——而这不是保证行为，也不在 Writer→Inspector→Rewrite 闭环内。

**发现 4.1.2 — 内置 MCP 工具未自动注册进 CapabilityRegistry。**
`mcp/server.rs:154 register_built_in_tools` 把 filesystem/text_processing/web_search 注册进 `BUILTIN_MCP_SERVER`，但启动时**没有**为它们调用 `Capability::from_mcp_tool`。后果：若 PlanGenerator 输出 `mcp.builtin.web_search` 步骤，`planner/mod.rs:455` 的验证器会**丢弃该步骤**（capability 不存在）。外部 MCP 服务器反而全自动注册（`commands/mcp.rs:22-33`）。内置工具是"半残"——handler 活、发现死。

**发现 4.1.3 — Skill hooks 与 Skill-runtime MCP 是死基础设施。**
`SkillRegistry` 有 `hooks` 字段，5 个内置技能全是 `hooks: vec![]`，创作流从不触发 HookEvent。`SkillExecutor` 有完整 MCP 客户端分支（`executor.rs:270-319`），但无内置技能使用 MCP runtime。两者都是"建好但无人用"。

**发现 4.1.4 — 能力进化环半活。**
`PlanExecutor` 每次 step 后调 `record_execution`（`executor.rs:373`），记录是活的；启动时 `load_evolved_descriptions` 带 `<think>` 清洗（`capabilities/mod.rs:20-67`）也是活的。但 LLM 驱动的描述自动改写（`evolution.rs:267`）未发现定时触发，只在显式 evolution 命令时跑。即"记了但没自动进化"。

### 4.2 方法论 / 提示词层：两套并存，4 个模板被旁路

**发现 4.2.1 — 存在两套互不相通的方法论定义。**
- `prompts/methodologies/`（雪花10步/英雄12阶段/场景3模板 + `Methodology` 枚举）：整目录 `#![allow(dead_code)]`，全仓无 `use` 引用。**死代码。**
- `creative_engine/methodology/`（5 种，带 `Methodology` trait + `MethodologyEngine`）：实时注入 `build_writer_prompt`。

维护混淆风险高——后人可能改错那套。

**发现 4.2.2 — 4 个 PromptLibrary 模板注册但被硬编码 prompt 旁路。**
`style_checker_system`、`commentator_system` 已注册进 PromptRegistry，但实时风格检查走 `StyleChecker::check`（规则）+ `build_llm_check_prompt`（硬编码），评点家走内联硬编码 prompt。后果：前端"提示词覆盖"功能（`save_override`）对这两个 Agent **实际无效**——用户以为能覆盖，实则改的是没人读的模板。

**发现 4.2.3 — 方法论/StyleDNA/个性化是 Pro-only。**
`build_writer_prompt:1954` 的 `if is_pro` 守卫把方法论、StyleDNA、StyleBlend、写作风格设定、作品简介、个性化偏好**六类资产整体挡在 Free 用户门外**。Free 用户续写仅得：策略约束 + 体裁画像 + 风格指纹 + Canonical State + 四元组。这意味着"越写越懂"的个性化核心在 Free 版基本不生效。

**发现 4.2.4 — StrategySelector（LLM 选资产）在续写路径未启用。**
续写只走 `build_selected_strategy`（直接读 story 字段）+ `infer_narrative_quartet`（纯启发式，不调 LLM）。`asset_catalog` 里 52 风格/5 方法论/beat_cards/story_engines/pressure_relationships 作为"可选资产"目前**不会在续写中被 LLM 动态挑选**——只有用户在 story 上显式设定的 `methodology_id`/`style_dna_id` 生效。

**发现 4.2.5 — 四元组资产内容未完整展开。**
`render_narrative_quartet_section`（`service.rs:2904`）只渲染 `SelectedStrategy` 里的 ID/字段为提示文字，**不**从 `builtin_beat_cards()`/`builtin_story_engines()` 加载完整卡片内容（function/remix_hint/payoff 等）注入。资产目录里卡片的丰富 payload 在续写时未被读取。

### 4.3 网文资产层：3 张 DB 表是死 schema

**发现 4.3.1 — beat_cards/story_engines/pressure_relationships 三张表从未被读写。**
Migration 92（`db/connection.rs:3254-3325`）创建这三张表，注释承诺"所有内置数据由代码侧注入（builtin=1），允许用户在 UI 中追加自定义条目"。**但全仓 grep `INSERT INTO beat_cards|story_engines|pressure_relationships` 和 `SELECT ... FROM` 零命中**：
- 无 Repository（`BeatCardRepository` 等不存在）
- 无 IPC 命令
- 无 seed 逻辑

资产数据只存在于 `creative_engine/{beat_cards,story_engines,pressure_relationships}/` 的硬编码 `builtin_*()` 函数。**用户自定义条目——迁移设计的核心目标——不可能实现。**

**发现 4.3.2 — 追读力与 Anti-AI 是审计专用，不回流创作。**
`reading_power/`（追读力评估 + 追债 + 覆盖契约）和 `anti_ai/`（五维审查）从不被 `build_writer_prompt` 或 `AgentOrchestrator` 查询（零命中）。它们只服务于：自动化后置事件、深度洞察报告、UI 命令。`CONTEXT.md:68` 明确标注追读力"Pending implementation"（`build_review_prompt` 需注入契约）。AntiAiRewriter 是 v0.17.1 骨架，`rewrite()` 返回原文不变。

**发现 4.3.3 — GenreProfile 是唯一全闭环网文资产。**
seed 自 `templates/genres.json`，用户可编辑，三处注入（genesis 策略笔记 / TimeSliced 反模式 / 四元组 emotional_payoff）。这是 v0.17 资产里唯一"生成→存储→可变→消费"全通的。

### 4.4 状态 / 记忆层：QueryPipeline 整模块死，伏笔自动追踪断环

**发现 4.4.1 — QueryPipeline 五阶段管线是整模块死代码。**
`memory/query.rs:15` 的 `QueryPipeline`（token_search + semantic_search + fuse_results + graph_expansion + budget_control + assemble_context）**`new()` 零调用方**。配套的 `KnowledgeGraph` trait、`DbVectorStore` 适配器（`db/repositories.rs:2398`）也只为喂这条死管线而存在。实时记忆走的是更简单的 `MemoryOrchestrator::build_memory_pack`（按大纲关键词过滤 + 20 章窗口），**不做图谱扩展、不做语义向量检索**。v5.4.0 宣称的"语义搜索融合"在运行时不生效。

**发现 4.4.2 — 生成后 ingest 不自动登记伏笔。**
`IngestPipeline::run_ingest` 把伏笔提取进 `IngestAnalysis.foreshadowing`（`memory/ingest.rs:69,464,489`），但**从不调用** `ForeshadowingTracker::add_foreshadowing` 持久化。伏笔只经 GenesisPipeline bootstrap、手动 `create_foreshadowing` 命令、拆书分析进入 `foreshadowing_tracker`。**续写中隐含埋设的伏笔永不被追踪**——回收账本不完整，"逾期伏笔警告"会漏报。

**发现 4.4.3 — character_states 表运行时为空。**
`CanonicalStateManager::fetch_character_states` 读取该表，`build_writer_prompt` 渲染为 `【角色当前状态】`，但创作/ingest 流程**从不写入**该表（写入方 `update_character_state` 在生成后无调用）。该 prompt section 结构上是活的，数据上是空的——LLM 永远看到空的角色状态。

**发现 4.4.4 — StoryStateManager 与 evolution/ 模块死代码。**
`state/manager.rs` 文件头明确"RESERVED FOR FUTURE USE (Phase 4)"，零调用方。`evolution/`（EvolutionReviewer/Analyzer/Updater）零非自身调用方。注意这与活的 `capabilities::evolution`、`creative_engine::style::evolution` 是不同模块。

**发现 4.4.5 — 两条 build_agent_context 路径不一致。**
- Path A（smart_execute）经 `StoryContextBuilder::build`，**不**调 CanonicalStateManager。
- Path B（直接 agent 命令）经 `ContextOptimizer::build_full_context`，**手动注入** canonical snapshot（`agents/commands.rs:1343-1453`）。
- 两条路径最终都汇入 `build_writer_prompt`，后者**独立重新拉取** snapshot（`service.rs:2168`）。

净效果：Path B 算了两次 snapshot，Path A 算一次。架构不一致，有重复计算。

### 4.5 创作流程：TimeSliced 是资产黑洞

**发现 4.5.1（最关键）— TimeSliced 完全绕过资产装配中心。**
`execute_time_sliced`（`orchestrator.rs:608-823`）不调 `StoryContextBuilder::build`，不调 `build_writer_prompt`，只用 `WriteTimeBundle`（`write_time_bundle.rs:139`）。`WriteTimeBundle.to_prompt()` 包含的资产仅：
1. 合同红线（`StoryContractRepository` MASTER_SETTING，截断 800 字）
2. 角色核心（姓名 + background + physical/mental_state + location + personality）
3. 场景大纲（dramatic_goal/conflict_type/external_pressure/setting_location）
4. GenreProfile 反模式（anti_patterns_json）

**完全不接触**：Skills、MCP、方法论、StyleDNA/blend、写作策略 config、四元组（`narrative_quartet` 参数被忽略，因 TimeSliced 绕过 `build_writer_prompt`）、知识图谱、规范状态、伏笔、记忆包、个性化、风格指纹、叙事结构、前文摘要、写作风格详细设定。

**发现 4.5.2 — WriteTimeBundle 的 style_slice 是死代码。**
`execute_time_sliced` 调 `load_sync(..., None)`（`orchestrator.rs:666` 注释"任务 1.6 暂不接入 StyleDna，留空"）。即使题材命中 RealismEmotional/Mystery，`style_slice_override` 恒为 None。所以 TimeSliced 实际连题材自适应的风格片段都没有。

**发现 4.5.3 — 审计是 fire-and-forget。**
TimeSliced 在正文返回后 spawn `AuditExecutor::run_audit`（`orchestrator.rs:753-769`）。审计跑 11 维检查，结果写 `TextAnnotationRepository` + emit annotation 事件，**不触发自动 Rewrite**。`run_audit` 注释明确"审计失败（静默，不影响用户）"。质量问题依赖用户手动处理 annotation。只有 Full 模式同步 Inspector 才驱动改写闭环。

**发现 4.5.4 — Full 模式 Inspector 是"半盲"的。**
`build_inspector_prompt`（`service.rs:2312`）只注入 story_title/genre/characters/content，**不注入**世界观/伏笔/记忆/风格细节。Inspector 对设定的检查只能凭 LLM 自身从 content 推断，而非对照原始设定。真正对照设定检查的是异步 AuditExecutor，但那条路径不驱动改写。`execute_inspector`（`executor.rs:870`）调 `build_agent_context` 建了全量上下文，但 Inspector 的 prompt 模板不用它——**上下文建了却没注入到 prompt**，是潜在不一致。

**发现 4.5.5 — PlanContext 喂给 PlanGenerator 的是手工摘要，非全量。**
`smart_execute_inner` 自己拉"轻量摘要"（stories/chapters/scenes/world/chars/foreshadowing/style_blend），不含 Skills、KG 实体、Memory pack、Canonical snapshot、叙事结构、方法论详细约束、beat_cards 完整内容。所以 plan 生成阶段看到的资产远少于 Full 写作阶段。PlanGenerator 唯一拿到全量的是 CapabilityRegistry（`to_llm_context()`），但若 >4000 字会降级为硬编码简化清单（`planner/mod.rs:286-310`）。

---

## 5. 根因分析

### 5.1 速度-质量权衡未被架构性解决

v0.14.3 为根治本地大模型超时，把续写默认设为 TimeSliced（单次 LLM，30-60s）。这是正确的紧急止血。但代价是 TimeSliced 成为一条**独立的精简代码路径**，与 Full 模式的资产装配中心（`StoryContextBuilder` + `build_writer_prompt`）完全分叉。这不是"Full 的子集"，而是"另一套 prompt"。

后果：所有后续资产建设（v0.17 四元组、风格指纹、个性化、记忆包……）默认都不进入用户最高频的操作。资产越建越多，默认路径却越来越瘦——形成"资产丰盛但用户无感"的悖论。

### 5.2 资产消费点分散，缺少统一注入网关

资产注入散落在 4 个地方：`smart_execute`（PlanContext 摘要）、`StoryContextBuilder::build`（AgentContext）、`build_writer_prompt`（system prompt 18 段）、`WriteTimeBundle::to_prompt`（TimeSliced 4 段）。没有单一"资产注入网关"，导致：
- 新增资产时容易漏接某条路径
- TimeSliced 路径无人维护资产接入（发现 4.5.2 的 style_slice 死代码就是典型）
- 同一资产在 Full 路径有、TimeSliced 路径无，用户体感不一致

### 5.3 "建好即用"假设失效，多处闭环断裂

多个资产遵循"定义→注册→以为自动生效"的模式，但实际消费点未接通：
- QueryPipeline 五阶段：建了整模块，零调用方
- beat_cards 三张表：建了 schema，零读写
- 3 个内置技能：注册了，orchestrator 不调
- 内置 MCP：注册了 handler，不注册 capability
- character_states：读了表，无人写
- ingest 伏笔：提取了，不持久化
- 4 个 prompt 模板：注册了，走硬编码

这是"定义≠生效"的系统性问题——缺少"消费点验证"。

### 5.4 Pro 守卫一刀切，削弱 Free 版专业性

`build_writer_prompt:1954` 的 `if is_pro` 把方法论、StyleDNA、blend、写作风格、作品简介、个性化**六类**整体挡在 Free 门外。这意味着 Free 用户的"创作专业性"资产仅剩策略约束 + 体裁画像 + 风格指纹 + Canonical State + 四元组。考虑到 AGENTS.md 强调的"越写越懂"核心理念，Free 版的个性化体验与设计愿景差距较大。

---

## 6. 完善与优化方案

### 方案分级

| 级别 | 含义 | 建议优先级 |
|------|------|------------|
| P0 | 修复断环/死代码，让已建资产真正生效 | 立即 |
| P1 | 解决 TimeSliced 资产黑洞，统一注入网关 | 本里程碑 |
| P2 | 清理死代码，消除维护混淆 | 本里程碑 |
| P3 | 增强 Pro/Free 分层与资产丰富度 | 下里程碑 |

### P0-1：接通 ingest→伏笔自动追踪闭环

**问题**：发现 4.4.2，续写中隐含埋设的伏笔不被追踪，回收账本不完整。

**方案**：在 `IngestPipeline::run_ingest`（`memory/ingest.rs:262`）成功提取 `IngestAnalysis.foreshadowing` 后，调用 `ForeshadowingTracker::add_foreshadowing` 持久化。需去重（按 content 哈希）防止重复登记。

**验证**：续写后查 `foreshadowing_tracker` 表有新增；后续续写的"待回收伏笔"提示包含新埋伏笔。

### P0-2：character_states 写入闭环

**问题**：发现 4.4.3，`【角色当前状态】` prompt 段永远为空。

**方案**：在 `IngestPipeline` 或 `MemoryWriter::write` 后，用 `update_character_state`（已存在）写入角色当前 physical/mental_state/location。状态来源：可由 ingest 的 LLM 分析提取，或由 AuditExecutor 的 character 维度结果回写。

**验证**：续写后 `character_states` 表有更新；`【角色当前状态】`段非空。

### P0-3：内置 MCP 工具自动注册进 CapabilityRegistry

**问题**：发现 4.1.2，PlanGenerator 输出的 `mcp.builtin.*` 步骤被验证器丢弃。

**方案**：在 `lib.rs` setup 阶段（`register_built_in_tools` 之后），遍历 `BUILTIN_MCP_SERVER` 的工具，对每个调 `Capability::from_mcp_tool` + `registry.register`。

**验证**：PlanGenerator 生成的 `mcp.builtin.web_search` 步骤能通过验证并执行。

### P0-4：四元组资产内容完整展开

**问题**：发现 4.2.5，beat_cards/story_engines/pressure_relationships 只渲染 ID，不展开 payload。

**方案**：`render_narrative_quartet_section`（`service.rs:2904`）改为从 `builtin_beat_cards()`/`builtin_story_engines()`/`builtin_pressure_relationships()` 按 ID 查找完整卡片，注入 function/remix_hint/payoff/avoid 等字段。

**验证**：续写 prompt 含完整卡片内容（非仅 ID）。

### P1-1（核心）：TimeSliced 接入关键资产子集

**问题**：发现 4.5.1，默认续写路径资产黑洞。

**方案**：扩展 `WriteTimeBundle`，按"速度优先但保专业性"原则接入**精选资产子集**（控制总 prompt 在 ~2K token 增量内，不影响速度）：
1. **伏笔状态**（pending top3 + overdue top1，每条一行）——保证"逾期伏笔警告"在默认续写生效
2. **规范状态叙事阶段**（一行）——让续写知道当前是 ConflictActive/Climax/Resolution
3. **StyleDNA 主导风格一句话摘要**（非六维全文）——保底风格一致性
4. **四元组**（已序列化的 `narrative_quartet`，TimeSliced 当前忽略）——v0.17 核心资产

每项压缩为 1-3 行，总增量可控。`WriteTimeBundle::load_sync` 增加这些字段的 DB 查询（均轻量）。

**验证**：TimeSliced 续写 prompt 含上述 4 项；本地 Qwen 续写时长仍在 30-60s 区间（用 TTFB 基准验证）。

### P1-2：接通 3 个休眠技能

**问题**：发现 4.1.1，character_voice/plot_twist/text_formatter 从未被创作流调用。

**方案**：在 `apply_writing_skills`（`orchestrator.rs:1235`）增加**场景智能选择**：
- 检测到对话段密集 → 调 `character_voice`
- 检测到情节转折点（Canonical State 阶段跃迁）→ 调 `plot_twist`
- 检测到段落排版混乱 → 调 `text_formatter`

或更简单：让 `strategy.skill_ids`（已存在但闲置，发现 4.1.4 末）真正驱动 `apply_writing_skills` 的技能选择，而非硬编码。

**验证**：相应场景下对应技能被调用（日志可见）。

### P1-3：Full 模式 Inspector 接入全量上下文

**问题**：发现 4.5.4，Inspector 半盲。

**方案**：`build_inspector_prompt`（`service.rs:2312`）增加注入：active foreshadowings、world rules、style DNA 摘要。`execute_inspector` 已建全量 AgentContext，只需把关键字段传入 prompt 模板。

**验证**：Inspector 能报告"伏笔 X 在本章未推进"等对照设定的具体问题。

### P1-4：审计触发自动 Rewrite（可选）

**问题**：发现 4.5.3，审计 fire-and-forget。

**方案**：`AuditExecutor::run_audit` 若发现 critical 级问题（如逻辑硬伤、角色矛盾），emit 一个 `AutoRewriteRequested` 事件，前端确认后触发一次 Full 模式 Rewrite。保持用户控制权，不静默改文。

**验证**：审计发现 critical 问题后，前端弹出"检测到问题，是否自动修订"。

### P2-1：删除死代码

| 死代码 | 位置 | 处理 |
|--------|------|------|
| `prompts/methodologies/` 整目录 | `prompts/methodologies/*.rs` | 删除（实时用 `creative_engine/methodology/`） |
| `PromptManager` | `prompts/mod.rs:26` | 删除 |
| `PromptEvolver` | `prompts/evolver.rs` | 删除 |
| `QueryPipeline` 五阶段 | `memory/query.rs` | 删除或标记 `#[deprecated]`（实时用 `MemoryOrchestrator`） |
| `StoryStateManager` + schema | `state/` | 删除（标注 RESERVED 已久） |
| `evolution/` 模块 | `evolution/` | 删除（与 capabilities::evolution 混淆） |
| AntiAiRewriter 骨架 | `anti_ai/rewriter.rs` | 删除或实现 |

### P2-2：消除两套方法论/两套 prompt 旁路

**问题**：发现 4.2.1、4.2.2。

**方案**：
- 删除 `prompts/methodologies/`（P2-1 已含）
- `StyleChecker` 与 `CommentatorAgent` 改为读 PromptRegistry 模板（`style_checker_system`/`commentator_system`），使前端"提示词覆盖"生效

### P2-3：beat_cards/story_engines/pressure_relationships 表要么实现要么删除

**问题**：发现 4.3.1，三张表死 schema。

**方案**（二选一）：
- **实现**：建 Repository + seed builtin 数据 + IPC 命令，兑现"用户自定义"设计
- **删除**：若短期不计划支持自定义，删除三张表（migration 92 的表创建部分），资产仅留内存 `builtin_*()`

建议先删除，避免维护空表；待真要做自定义时再建。

### P3-1：重新划分 Pro/Free 资产边界

**问题**：发现 4.2.3，`if is_pro` 一刀切六类资产。

**方案**：精细化分层——
- Free 可用：方法论（基础 1-2 种）、StyleDNA（用户可选 3-5 种基础）、风格指纹、个性化（基础偏好）
- Pro 增强：全部 52 风格、StyleBlend 混合、5 种方法论、高级个性化、作品简介注入

### P3-2：启用 StrategySelector 在续写路径

**问题**：发现 4.2.4，LLM 资产选择闲置。

**方案**：在 `build_selected_strategy` 中，当 story 未显式设定 methodology/style_dna 时，调 `StrategySelector::select_strategy`（LLM 从资产目录选），而非只走启发式。让资产能被"按需智能匹配"。

### P3-3：统一资产注入网关

**问题**：根因 5.2，注入点分散。

**方案**：抽象 `AssetInjector` trait，Full 与 TimeSliced 各有实现，但共享资产加载逻辑。新增资产只需在一个地方注册，自动在两条路径生效（按各路径预算裁剪）。

---

## 7. 优先级路线图

```
本里程碑（v0.17.x）
├─ P0-1 ingest→伏笔闭环          [1-2h]  ← 数据完整性
├─ P0-2 character_states 写入     [2-3h]  ← 数据完整性
├─ P0-3 内置 MCP 自动注册         [0.5h]  ← 功能可用
├─ P0-4 四元组内容完整展开        [1h]    ← 资产生效
├─ P1-1 TimeSliced 接入精选资产   [3-4h]  ← 核心：解决资产黑洞
├─ P1-2 接通 3 个休眠技能         [2h]    ← 资产生效
├─ P1-3 Full Inspector 全量上下文 [1-2h]  ← 质检质量
└─ P2-1/2/3 死代码清理            [2-3h]  ← 维护性

下里程碑（v0.18）
├─ P1-4 审计触发自动 Rewrite      [3h]
├─ P3-1 Pro/Free 精细化分层       [4h]
├─ P3-2 启用 StrategySelector     [3h]
└─ P3-3 统一资产注入网关          [6-8h]  ← 架构性
```

---

## 8. 审计结论

StoryForge 的后台资产**设计完备、积累深厚**，在同类 AI 写作工具中属上乘。但审计揭示三个系统性问题：

1. **默认路径资产黑洞**（P1-1）——v0.14.3 的速度优先决策导致 TimeSliced 绕过 90% 资产，用户最高频操作享受不到资产红利。这是"专业性"落地的最大障碍。

2. **多处闭环断裂**（P0 系列）——伏笔自动追踪、角色状态、四元组展开、内置 MCP 等"建好但未接通"的断环，让已投入的资产价值流失。

3. **死代码与双系统并存**（P2 系列）——两套方法论、4 个旁路模板、QueryPipeline 整模块、3 张死表，增加维护成本与混淆风险。

**建议**：本里程碑优先 P0（断环修复，低成本高回报）+ P1-1（TimeSliced 资产接入，核心痛点），即可让"越写越懂"的核心理念在默认路径真正生效。P2 死代码清理同步进行，P3 架构性优化放下里程碑。

---

*本报告基于 2026-06-20 代码库快照，5 路并行 Agent 全量追踪生成。所有 file:line 引用均经交叉验证。*
