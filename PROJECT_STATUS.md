# StoryForge (草苔) v0.23.13 项目完成状态

> 最后更新: 2026-06-22（v0.23.13 强制所有生成路径使用活跃模型）
> GitHub: https://github.com/91zgaoge/StoryForge

---

## ✅ 最近完成功能

### v0.23.13 — 强制所有生成路径使用活跃模型（2026-06-22）

- 🎯 **彻底修复“当前模型是 A，实际调用 B”**：`LlmService::select_profile_for_request`、`GatewayExecutor::select_candidates`、`GatewayExecutor::select_fastest_profile` 全部优先返回/置顶用户当前设置的活跃模型
- 🧭 **Genesis 故事概念、TriShot Call 1、普通路由生成统一走活跃模型**：只要活跃模型健康（Healthy/Degraded），不再被 TTFB 阈值或三维打分绕开
- 🩹 **新增模型即时可用**：`create_model` 完成后立即刷新网关注册表并执行健康探测，探测通过即刻进入可用模型池
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.12 — 彻底修复长超时：活跃模型优先 + 智能创作流程日志（2026-06-22）

- 🎯 **修复长超时根因**：模型网关现在强制把用户当前设置的活跃模型放到候选链首位，避免连接到历史/错误模型导致挂起
- 🧭 **`select_fastest_profile` 活跃模型兜底**：即使活跃模型没有算力档案，也优先使用它
- 📝 **新增 `WorkflowLogger`**：记录 TriShot Call 1/Call 3、LLM 调用起止、模型网关候选链与选择原因、错误等详细步骤
- 📋 **诊断卡片增强**：新增 `工作流日志路径` 与 `智能创作流程最近日志`，可直接查看后端执行轨迹
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.11 — 诊断提示词过滤探测/静默调用（2026-06-22）

- 🛡️ **过滤探测/静默调用**：`LlmService::execute_generation` 只在非静默调用时更新诊断提示词
- 🐛 **修复诊断提示词被 probe 覆盖**：避免 `model_gateway_probe` 的 `Respond with exactly the word OK.` 覆盖用户真正关心的生成提示词
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.10 — 模型网关优先使用当前活跃模型（2026-06-22）

- 🎯 **修复 AI 连到旧模型的问题**：`select_fastest_profile` 现在优先使用当前设置的活跃模型（只要健康且 TTFB 不比最快模型差太多）
- 🔗 **`select_candidates` 兜底活跃模型**：候选链中若不存在活跃模型，自动注入，保证用户设置的模型始终有机会被选中
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.9 — 运行时创作资产能力清单 + TriShot 路由增强（2026-06-22）

- 📚 **运行时创作资产能力清单**：应用启动后自动生成并刷新全部系统资产（methodology、genre_profile、style_dna、skill、beat_card、story_engine、pressure_relationship、workflow 等）的紧凑目录
- 🎯 **TriShot Call 1 可见全局资产**：`PromptSynthesizer` 的 prompt 中新增【系统可用创作资产目录】，让最快模型在选资产时知道可调用的系统级资产
- 🔀 **Call 3 资产透传**：TriShot Call 3 通过 `generate_for_task_with_tags` 把 Call 1 选中的资产 ID/标签透传给 `ModelGateway`
- 🧭 **ModelGateway 识别更多资产标签**：`methodology`、`beat_card`、`story_engine`、`pressure_relationship`、`style_dna`、`skill` 等标签会触发 `HeavyCreation`，优先使用创作能力强的模型
- 🐛 **修复 TriShot request_id 错误**：不再把 `gen_response.model` 当作 `request_id`
- 🛡️ **Call 1 预算守卫**：剩余时间不够完成 Call 1 + Call 3 时直接回退本地 `bundle_prompt`，避免前端长时间无响应
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.8 — AI 进度指示精细化 + 提示词诊断可靠性提升（2026-06-22）

- 🎯 **LLM 进度阶段具体化**：每个 LLM 调用都会显示连接模型 ID/提供商、组合提示词规模、等待模型回应、模型回应 token 数、解析结果，不再只显示“构思故事”
- 📊 **`LlmGeneratingProgress` 字段扩展**：新增 `model_id`、`provider`、`prompt_chars`、`prompt_tokens`、`response_tokens`
- 🛡️ **提示词诊断兜底机制**：新增 `diagnostics::DiagnosticStore` Tauri State 与 `get_last_llm_prompt` 命令，解决大提示词事件可能丢失的问题
- 🩹 **修复诊断卡片“未捕获提示词”**：即使 `llm-prompt-sent` 事件未送达，诊断时也会主动通过命令读取完整提示词
- ✅ **验证**：`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.7 — 诊断信息增强 + 超时文案去硬编码（2026-06-22）

- 🩹 **修复诊断版本号硬编码**：`__STORYFORGE_VERSION__` 改为从 `package.json` 动态读取，不再显示 `0.16.0`
- 🩹 **修复超时文案硬编码**：`handleRequestGeneration` / `handleSmartGeneration` 现在从 `settings` 读取 `frontend_timeout_secs` / `smart_execute_total_timeout_secs`，错误提示与诊断卡片均显示实际配置值
- 📋 **诊断卡片新增 AI 生成模式**：显示 `settings.generation_mode`（`auto` / `time_sliced` / `fast` / `full` / `tri_shot`）
- 🤖 **诊断卡片新增当前模型信息**：模型 ID / 名称 / 提供商 / 端点
- 📝 **诊断卡片新增最后发给模型的提示词全文**：后端通过 `llm-prompt-sent` 事件广播，前端实时捕获并展示（上限 12000 字符）
- ✅ **验证**：`cargo check` 零错误；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.6 — 修复 macOS 启动崩溃：VectorStore State 初始化顺序（2026-06-22）

- 🐛 **修复启动 panic**：解决 `state() called before manage() for Arc<dyn VectorStore>` 导致的 macOS 启动崩溃
- 🔧 **根因**：`init_task_system_and_automation` 在 `app.manage(vector_store)` 之前通过 `app_handle.state()` 获取向量存储
- 🔧 **方案**：将 `LanceVectorStore` 创建与 `app.manage(vector_store)` 提前到依赖组件之前，异步 `init()` 保留原地
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 通过；`python3 scripts/architecture_guard.py` 通过

### v0.23.5 — CI 格式化修复（2026-06-21）

- 🎨 **Rust nightly fmt**：修复 import 顺序、函数参数折行、单行化等格式化差异
- 🎨 **前端 Prettier**：修复 `GeneralSettings.tsx` 类型断言单行化差异
- 📋 **无业务逻辑变更**：仅代码风格修复，使 GitHub Actions `rust-check` / `frontend-check` 通过

### v0.23.4 — 智能层闭环落地（2026-06-21）

- 🧠 **LLM JSON mode 原生支持**：新增 `llm::adapter::ResponseFormat::JsonObject`，OpenAI/Ollama 适配器分别附加 `response_format` / `format`，模型网关可透传
- ✍️ **Review/Refine Pipeline 结构化输出**：调用 JSON mode 并解析 `{ refined_content, change_summary, refinement_notes }`
- 💰 **MemoryPack 预算语义强类型化**：`MemoryBudget::for_task_type` 接收 `MemoryTaskType { Write, Plan, Review }`
- 📚 **拆书存储统一**：删除 `reference_characters` / `reference_scenes`，人物/场景数据全部汇入 `narrative_*` 表；迁移 `V100__拆书存储统一_删除_reference_表.sql`
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.3 — 测试基线修复 + 工程化（2026-06-21）

- 🐛 **MigrationRunner 交错执行**：`run_with_legacy` 按版本将 SQL 文件 migration 与 inline Rust migration 交错执行，避免高版本 SQL 文件跳过低版本 inline migrations
- 🗂️ **SING migration 版本上调**：`V095__意图图_SING_数据层.sql` → `V099__...`，确保其跑在所有 inline migrations 之后
- 🗂️ **`narrative_*` 表补 status 列**：`narrative_characters` / `narrative_scenes` / `narrative_world_buildings` 加入 `status TEXT NOT NULL DEFAULT 'active'`，新增 inline Migration 98 为已存在表补列
- 🔄 **ElementSource/ElementStatus round-trip 修复**：`domain/narrative_elements.rs` 新增 `as_str()` / `from_str()`（snake_case 英文）；`db/repositories_narrative.rs` 存储与解析统一使用英文键
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**（新增 3 个测试，零回归）；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.2 — 事件总线与状态同步治理（2026-06-21）

- 📡 **后端 `SyncEvent::ChapterCommitted`**：携带 `projection_status`，`SceneCommitService::apply_commit` 在 projections 完成后统一发射
- 🖥️ **前端 `content/isSaved` 迁移到 `frontstageStore`**：移除本地 `useState`，保留 `isSaved` + editor focus 双重保护
- 🧹 **清理遗留事件/hack**：删除所有 `backstage-data-refreshed` 废弃注释；`useWebViewRedrawFix` 改为 `FIXME` 标记
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 487 passed / 48 failed（零新回归）；`npx tsc --noEmit` 零错误；`npx vitest run` 126 passed / 3 skipped

### v0.23.1 — 架构债务清偿：全局单例治理 + 模块依赖解耦（2026-06-21）

- 🗑️ **全局单例清零**：彻底移除 14 个全局 `static`/缓存，全部改为 Tauri State 注入或每次调用重新加载
- 🏗️ **domain 领域层扩展**：新增 `agent_context` / `agent_types` / `foreshadowing` / `search` / `write_time_bundle` / `asset_snapshot` / `continuity` / `adaptive` / `prompt_synthesis` / `agent_service` / `creative_engine` 等共享类型与端口
- 🔗 **模块循环依赖斩断**：`memory → agents`、`narrative → memory`、`narrative → creative_engine` 数据类型下沉到 `domain`；`agents ↔ creative_engine` 行为依赖通过 `CreativeEnginePort` / `AgentServicePort` 双向反转
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed / 48 failed（零新回归）；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.0 — TriShot 三击生成管线：关键路径压缩至最多 3 次 LLM（2026-06-21）

- 🎯 **TriShot 三击管线**：新增 `GenerationMode::TriShot`（三击），Call 1 最快模型选资产+合成提示词 → Call 2(可选) 精修 → Call 3 Writer 生成。质检/改写/入库/洞察全部下沉后台静默执行
- ⚡ **快速模型选取**：`GatewayExecutor::select_fastest_profile()` 按算力档案 TTFB 升序选最快可用模型，`LlmService::generate_with_fastest()` 捷径
- 🧩 **prompt_synthesis 模块**：`AssetManifest` 把 ~17 段资产打包为紧凑清单（4000 字符预算）+ `PromptSynthesizer` JSON 结构输出 + `PromptRefiner` 可选精修（预算守卫跳过）
- 🏎️ **PlanExecutor 快速路径**：TriShot 跳过 SING/PlanGenerator，`PlanStep::long_running` 跳过 90s 步超时
- 🤖 **BGP-2 智能改写**：`auto_rewrite_executor.rs` 按严重度分流——HIGH 自动改写+可撤销，LOW 仅建议
- 📡 **SyncEvent 扩展**：`ContentAutoRevised`（toast 通知）+ `RevisionSuggested`（审阅面板）
- 🖥️ **前端配置**：设置页面新增「三击模式」下拉选项
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed（新增 TriShot 19 测试全部通过，零回归）；`npx tsc --noEmit` 零错误

### v0.22.4 — 「异星球末世生存」智能创作流程优化 + 后台资产审计（2026-06-21）

- 🧩 **GenreResolver 题材解析**：精确/别名/子串/同义词/复合题材解析，解决自然语言题材词断链
- 🗺️ **意图图资产发现增强**：`AssetNode` tags + `discover_assets` 复合题材补充发现
- 🌉 **模型网关资产感知调度**：`asset_tags`/`discovered_asset_ids` 全链路透传，任务类别按标签校准
- ✍️ **TimeSliced 复合题材补强**：`secondary_genre_profile_strategy` 注入次要题材画像
- 📋 **后台资产全面审计**：新增 `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md`，梳理全部 22 类创作资产、智能创作流程注入点、12 项断链/断环问题与 10 条修复建议
- 🗺️ **项目流程图技术文档**：新增 `docs/PROJECT_PROCESS_FLOWCHARTS_v0.22.4.md`，覆盖创世、拆书、智能创作主路径、79+ 提示词、43 个网文题材模板、40+ 创意资产、Story System 全子系统流程图
- 🏗️ **架构审计报告**：新增 `docs/BROOKS_LINT_ARCHITECTURE_AUDIT_v0.22.4.md`，模块依赖图 + 6 大 decay risks 诊断，Health Score 18/100
- ✅ **验证**：新增 targeted tests 39 passed；`cargo check` 零错误；`npx tsc` 零错误

### v0.22.3 — 钥匙串彻底移除 + 模型健康报告自动刷新（2026-06-21）

- 🔐 **钥匙串彻底移除**：删除 keyring crate、secure_storage 模块、store_api_keys_securely 配置项
- 🧹 **移除 ~260 行钥匙串读写逻辑**：load/save 中全部钥匙串访问已清除
- 📊 **模型健康报告自动刷新**：前端每 30 秒自动刷新，后端改为 async
- ⚡ **冗余 load 消除**：execute_writer 2→1 次、FirstChapterGenerationStep 3→1 次
- ✅ **零回归**：cargo check 零错误，425 passed，tsc 零错误

- GenreProfile 推荐资产种子：7 个题材写入推荐风格/方法论/技能
- 策略选择硬约束：体裁画像有推荐时跳过 LLM 直接使用
- 算力档案默认值修正：capability_score 未测试时默认 0.0

### v0.22.1 — 5 条建设性意见（2026-06-21）

- StrategySelector 题材推荐映射：7 种题材→风格推荐
- StyleDNA 句长偏差检测：>30% 偏差记录建议
- Inspector 方法论动态 prompt：5 种方法论全覆盖
- GenreProfile 推荐字段：4 新列 + Migration 96
- 算力档案质量分权重：HeavyCreation→quality80%

### v0.22.0 — 提示词与后台资产深度结合（2026-06-21）

- Phase A：TimeSliced 路径全资产注入（StyleDNA六维+方法论+题材画像+策略）
- Phase B：Inspector 全资产注入（体裁画像+角色状态+冲突+四元组）
- Phase C：意图感知调度接线（agent_type→intent 自动推导）
- Phase D：算力档案消费闭环（TTFB/TPS 参与候选排序）
- Phase E：资产→生成参数规则映射（asset_params.rs）

### v0.21.0 — 提示词全量可配置化（2026-06-21）

- 79 个提示词全部前端可编辑（21 个分类）
- 假接入修复：15 个 key 改为 resolve_prompt（含 DB 覆盖）
- 旁路接线：40+ 个硬编码提示词全部接入 registry
- 前端 Monaco 编辑器 + 批量导入导出

---

## 🔧 编译状态

| 检查项 | 状态 |
|--------|------|
| `cargo check` | ✅ 零错误 |
| `cargo test --lib` | ✅ 538 passed / 0 failed / 2 ignored |
| `cargo test --lib intention_graph` | ✅ 21/21 |
| `cargo test --lib adaptive::asset_params` | ✅ 3/3 |
| `cargo test --lib genre_resolver` | ✅ 5/5 |
| `cargo test --lib selector` | ✅ 6/6 |
| `cargo test --lib write_time_bundle` | ✅ 13/13 |
| `cargo test --lib dispatcher` | ✅ 5/5 |
| 真实模型测试（Gemma4-e2b） | ✅ 6/6 |
| `npx tsc --noEmit` | ✅ 零错误 |
| `npx vitest run` | ✅ 126 passed / 3 skipped |
| `cargo +nightly fmt -- --check` | ✅ 零差异 |
| `npm run format:check` | ✅ 零差异 |
| `python3 scripts/architecture_guard.py` | ✅ 通过 |
| 后台资产审计 | ✅ 完成，见 `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md` |
| 已知测试失败 | ✅ 无（V092 基线问题已在 v0.23.3 清零） |

---

## 📊 提示词覆盖统计

| 类别 | 数量 | 状态 |
|------|------|------|
| Writer/Inspector/Commentator | 5 | ✅ 全部可覆盖 |
| Planner/Analyzer | 4 | ✅ 全部可覆盖 |
| Pipeline（审稿/修稿/后处理） | 4 | ✅ v0.22.0 新增 |
| Audit（质量审计） | 1 | ✅ v0.22.0 新增 |
| Intent（意图解析） | 1 | ✅ v0.22.0 新增 |
| Deconstruction（拆书） | 5 | ✅ v0.22.0 新增 |
| Creation（创世流程） | 14 | ✅ v0.22.0 新增 |
| Strategy（策略选择） | 1 | ✅ v0.22.0 新增 |
| Methodology（方法论） | 19 | ✅ 全部可覆盖 |
| Skill（技能） | 5 | ✅ 全部可覆盖 |
| Memory/Knowledge/Probe | 7 | ✅ 全部可覆盖 |
| Narrative（叙事） | 2 | ✅ 全部可覆盖 |
| World/Character（世界/角色） | 6 | ✅ 全部可覆盖 |
| System/Other | 5 | ✅ 全部可覆盖 |
| **总计** | **79** | ✅ |
