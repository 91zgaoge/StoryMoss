# StoryMoss 全面检视报告

> **版本**：v0.58.0　**检视日期**：2026-10-06　**范围**：全仓（Rust 后端 / 前端双窗 / src-server / landing / CI / 文档治理）
> **方法**：第一手基线复测（cargo/vitest/tsc/architecture_guard/线上 release 探针、关键结论逐条回源码核对）+ 五个分域深读（后端架构、前端、数据与可靠性、CI/发布/文档/安全、测试体系）。
> **证据约定**：标「实测」者为本次第一手复测；标「代码定位」者为读过源码但未运行复现；标「未核实」者仅线索级，不作为结论依据。文末附未验证项清单。

---

## 一、摘要（TL;DR）

1. **功能与本地质量基线健康**：`cargo test --lib` **1583 passed / 0 failed / 2 ignored**、`vitest` **607 passed / 3 skipped**、`tsc` 0 错、`architecture_guard` 通过、版本四件套 + landing 兜底版本全部 0.58.0——全部本次实测，与 AGENTS.md 宣称一致。
2. **但质量证据链在 CI 上是断的**：`.github/workflows/build.yml:98` 的 `cargo test --lib` 挂了 `continue-on-error: true`（:96），注释（:95）称「已知 49 个 V092 数据库迁移基线测试失败」——而本次实测 **0 failed**。即：全项目最关键的测试门禁在 CI 中物理上不可能让构建失败，且禁用它所依据的理由已经过时。
3. **产品核心风险仍是续写质量**：用户可见的幕前续写路径**不跑同步质量门**（`run_continue_inner` 走后台 `spawn_editor_qc` fail-open；同步 `handle_gate` 只服务无 UI 入口的批量续写），质量保障实质是「写前约束 + 单次重试 + 后台通报」；v0.41→v0.58 共十余个版本全部在给这条链路打探针补丁，且最新版自己标注「不得宣称续写质量已修复」。
4. **数据层治理有系统性缺口**：迁移表无 checksum 且水位线取 `MAX(version)`（版本号回退的补丁迁移永不执行）；`src-tauri/target/debug/db/migrations/` 仍有陈旧副本（最高 V129，源码 V131）且候选序排在源目录之前；`llm_calls` / `agency_activity_log` / `agency_board_items` / `agency_*` 零剪枝无外键；5 处热查询缺索引。
5. **安全为「条件性 P0 + 一串 P1」**：`src-server` 的 `JWT_SECRET` 有可预测缺省值（`config.rs:39`）、`DEV_UPGRADE_ENABLED` 缺省 true（`config.rs:52-54`）；桌面端 `withGlobalTauri: true` + CSP 含 `unsafe-eval/unsafe-inline` 且 `connect-src *`（`tauri.conf.json:13,15`）；工作室导出 ZIP 默认明文携带 API key；发布走 FTP 明文（`secure: false`）。仓库内 secrets 扫描干净。
6. **发布链路实测断裂**：线上 `StoryMoss_0.58.0_amd64.deb` 返回 **404**（dmg 200），根因是 `upload-releases-ftp.mjs` 的 `RELEASE_FILES` 白名单缺 `.deb`，而 Tauri 的 `latest.json` 仍包含 deb 平台条目——**deb 渠道用户的应用内「检查更新」必然失败**。
7. **工程熵偏高**：`agency/coordinator.rs` 7455 行（生产代码约 6836 行）承担约百个方法；`FrontstageApp.tsx` 5796 行 / 35 个 useState / 50 个 useRef / 28 个 useEffect / 64 个 useCallback；`AGENTS.md` 1208 行且 88% 是逐版本发布流水（已在本会话上下文中被实际截断）；根目录 44 个 md 中 36 个（82%）逾两个月陈旧且已成孤儿文档。
8. **测试是「回归网」而非「质量测量」**：热区（续写/资产/节拍）约 55–75% 的断言是中文子串锁（提示词文案契约）；全部 mock 为理想化脚本队列；唯二的真实模型测试 `#[ignore]` 且绑死内网 IP。因此「续写质量差」这类端到端属性在现有测试体系里**结构性不可见**，只能靠真机血案 → 补一条回归桩的循环演进。

**可上线判定**：桌面端主体功能可上线；但（a）deb 分发渠道的更新链必须先修，（b）若 `src-server` 非 compose 方式部署，须先处理 JWT 缺省密钥与 DEV_UPGRADE 开关，（c）不得对外宣称「续写质量已修复」。

---

## 二、第一手基线（全部本次实测）

| 检查 | 结果 | 与文档宣称对照 |
|---|---|---|
| `cargo test --lib` | **1583 passed / 0 failed / 2 ignored**（167.73s） | 与 AGENTS.md 完全一致 ✅；与 CI 注释「49 个失败」矛盾 ⚠️ |
| `npx vitest run` | **607 passed / 3 skipped**（99 文件通过 + 1 文件整体 skip） | 与 AGENTS.md 一致 ✅；3 skipped 全部来自 `LlmProfileForm.bug.spec.tsx` 一个永久 `describe.skip` |
| `npx tsc --noEmit` | 0 错误 | ✅ |
| `python3 scripts/architecture_guard.py` | 通过（移除全局单例 14、已知违规 0） | ✅ |
| `npm run format:check` | 通过 | ✅ |
| 版本四件套 | Cargo.toml / tauri.conf.json / package.json / landing `FALLBACK_VERSION` = 0.58.0 | 规则 3 合规 ✅ |
| 线上 release 探针 | dmg 200 / msi·AppImage（抽查 200）/ **deb 404** | 见 §8.2 |

---

## 三、项目快照

| 区域 | 规模 |
|---|---|
| `src-tauri/src`（Rust 后端） | 524 文件 / **199,653 行** |
| `src-frontend/src`（React 双窗） | 373 文件 / **77,913 行** |
| `src-server/src`（账号/订阅服务） | 11 文件 / 2,375 行 |
| `landing/src`（落地页） | 26 文件 / 1,788 行 |
| 数据库迁移 | 124 个条目（43 个 .sql + 80 个 .rs 迁移 + `mod.rs`；覆盖 V007–V131，缺 V084/V095 两个历史空洞） |
| 提示词资产 | `resources/prompts/` 92 个 .md |
| 测试 | Rust 1583 + vitest 607 + Playwright 34（9 个 spec） |

后端主要模块（约数）：`agency/` 30.4k（主管线）、`db/` 25.3k、`creative_engine/` 22.9k、`agents/` 14.3k（遗留体系）、`memory/` 7.6k、`narrative/` 5.7k、`planner/` 5.4k、`llm/` 5.3k、`model_gateway/` 4.4k，其余约 30 个模块共 ~48k。

---

## 四、架构评估

### 4.1 Agency 主管线（现状）

创世、续写、观察、资产重写四条链路都收敛在 `agency/coordinator.rs`：

- **创世**：概念包 → `producer_depth_assets` → 首章写作 → `assemble_only`（2548）落库 → `spawn_editor_qc`（2634）后台质检，返回 `EditorVerdict::pending`。
- **续写（Append，幕前唯一路径）**：`run_continue`（2915）→ `write_beat_once`（4278，单次 `complete()`、零工具）→ `cleanup_prose_for_persist` 抗重复三件套 → 落库 → 后台 ingest + 检查点 + `spawn_editor_qc`（3236/3267）。**明确跳过同步质量门**（3249-3250 注释自述「批量续写仍走 handle_gate 同步门」——而批量续写无前端入口）。
- **观察**：保存后 30s 空闲窗口的水位判断（`observe.rs:203`），有创作 run 时让路。
- **资产重写**：预览 → 用户确认 → `persist_confirmed_outlines` 落库。

值得肯定的设计（0-LLM 的 Rust 编译层）：节拍卡 `beat_card.rs`、导演锁 `continue_director.rs`、情感张力账本 `emotional_ledger.rs`、冻结 `continue_freeze.rs`、提示词哑拼接器 `prompts/assembly.rs`、原生 function calling（`tool_loop.rs:389`）。这些把「资产准入/约束装配」从模型手里拿回了确定性代码，是本项目最扎实的部分。

### 4.2 生成入口与可达性（含死代码判定）

| 入口 / IPC | 引擎 | 前端可达 | 判定 |
|---|---|---|---|
| `smart_execute`（幕前唯一） | 创世→Agency；续写→Append；资产重写→refresh；其余→PlanExecutor | 是 | 主线 |
| `auto_write` / `auto_revise`（文思） | `run_role_task(Writer)` ToolLoop | 是 | 系统提示词退化（见 4.3） |
| `generate_scene_draft`、向导 6 命令、`run_creation_workflow` | legacy agents / novel_creation | 是 | 并行第二体系 |
| `run_ai_generation_task` → 任务系统 → **TimeSliced** | legacy orchestrator | 是（幕后任务页可选） | 与 Agency Append **重复实现** |
| PlanExecutor `execute_writer`（Fast/Full，改写） | legacy orchestrator | 是 | 合理保留（改写场景） |
| `execute_trishot` | legacy TriShot | **否**（`resolve_rewrite_generation_mode` 永不返回 TriShot） | **死代码候选** |
| `agency_start_genesis` / `agency_continue_chapter` / `agency_continue_batch` / `agency_resume_run` | Agency | **无前端调用** | 半死代码；其中 batch 是唯一仍走同步质量门的路径 |

### 4.3 结构性风险

1. **上帝对象**：`coordinator.rs` 混装 LLM 适配、prompt 组装、DB 访问、落库、质检、冻结、预算、后台 spawn、学习埋点（2 个 `impl AgencyCoordinator`、3 个 `impl AgencyLlm`）。
2. **环依赖**：`agency → agents`（`sanitize_novel_output`、`trim_utils`）与 `agents → agency`（`emotional_ledger`）双向引用。
3. **双套续写 writer / 双套质检**：`write_beat_once`（单次）与 `write_chapter`（ToolLoop）；`handle_gate`（同步）与 `spawn_editor_qc`（后台）。UI 只用其一，另一套长期无用户流量却仍需维护。
4. **Writer 角色提示词退化**（代码定位）：`agency_writer_system` / `agency_inspector_system` 等 4 个 prompt_id 无内置资产（`roles/writer.rs:6` 自述占位），落到 `default_role_prompt` 的兜底「你是创作团队的一员。」（`coordinator.rs:6258`），且用户无法在提示词页覆盖——影响 `auto_write` / `auto_revise` 这类仍可达路径。

---

## 五、代码质量与热点

### 5.1 巨石文件

| 文件 | 行数 | 评估 |
|---|---|---|
| `agency/coordinator.rs` | 7455（生产 ~6836） | 见 4.3；建议抽出 continue 装配与 gate_service |
| `agents/orchestrator.rs` | 5441 | legacy 四模式（Fast/TimeSliced/Full/TriShot） |
| `agency/tests.rs` | 4177 | 测试单文件过大，可拆按链路分文件 |
| `frontstage/FrontstageApp.tsx` | 5796 | 35 state / 50 ref / 28 effect / 64 callback；生成生命周期由 9+ 互锁布尔 ref 驱动 |
| `pages/Stories.tsx` 1447、`RichTextEditor.tsx` 1403、`GeneralSettings.tsx` 1116、`WorldBuilding.tsx` 1076 | — | 混合列表+表单+向导+编辑器配置，可抽子组件 |

FrontstageApp 的拆分缝（低耦合、边界明确）：S1 保存链 `1359-1566`、S2 事件注册 `setupEventListeners 2104-2766`、S3 章节导航 `2768-3381`、S4 生成编排 `handleRequestGeneration 3578-4018` / `handleSmartGeneration 4396-5092`、S5 诊断看门狗、S6 渲染层浮层 `5419-5796`。

### 5.2 panic / 错误处理

- 启动路径**干净**：`lib.rs` setup 闭包无 `unwrap/expect/panic`；`CloseRequested` 路径无 panic 原语（符合 AGENTS.md 的「extern "C" 边界不得 panic」铁律）。
- 生产路径残留风险点（代码定位）：`capabilities/mod.rs:343`、`narrative/pipeline.rs:47` 的 poisoned-mutex `expect`（项目已有 `unwrap_or_else(|p| p.into_inner())` 惯例可循）；`llm/service.rs` 十余处 `lock().unwrap()`。
- `panic!`/`todo!`/`unreachable!` 在生产代码 0 处（14 处 `panic!` 全在测试）。

### 5.3 前端重复模式

`extractMessage` 在 18 个文件 60 处；`toast.` 遍布 41 个文件；列表页 `handleDelete×6` / `handleCreate×5`、50 个文件手写 `useState(false)` 管对话框；CRUD hooks（useCharacters/useStories/useWorldBuilding/useForeshadowings）高度同构——缺一层 `useEntityEditPage` / `ConfirmDialog` 脚手架。

### 5.4 死代码候选

- 幕前 4 个无引用的 TipTap 扩展（`TrackChanges`、`CommentAnchor`、`TextAnnotationMark`、`characterName`）。
- 4 个只被自身测试引用的孤儿 hook（`useFrontstageEditor/Generation/Panels/Wensi` 及其 4 个测试文件）。
- `memory/hybrid_search.rs` 整模块 `#![allow(dead_code)]` 且无调用者。
- 后端 `execute_trishot`；4 个无 UI 的 Agency 生成命令（建议补入口或标注 internal）。

---

## 六、数据层与可靠性

### 6.1 迁移治理（P0/P1 混合）

| 问题 | 证据（代码定位） | 后果 |
|---|---|---|
| 无 checksum/完整性校验 | `connection.rs:123-128` 仅 `(version, applied_at)` | 两份不同内容的同版本迁移（如 target 副本被改）永久静默 |
| 水位线 = `MAX(version)` | `connection.rs:62` | **任何低于当前最大值的补丁迁移永不执行**（V084/V095 空洞即历史痕迹） |
| `init_db` 失败不致命 | `lib.rs` setup 仅记日志继续 | 应用以「无数据库」半可用状态启动 |
| target 影子副本仍在 | 实测 `src-tauri/target/debug/db/migrations/` 42 个 .sql、最高 V129；源目录 43 个、最高 V131；候选序 `mod.rs:114` 排在 `:117` 之前 | 本次靠版本号较大侥幸选对源目录；**版本持平时会选陈旧副本** |
| 朴素 `;` 切分 + 静默跳过 already exists | `migrations/mod.rs:319-374` | 未来含触发器/字符串分号的迁移会静默半应用 |

### 6.2 连接 / 持久化 / 并发

生产池：max_size 50、`busy_timeout=5s`、WAL、`synchronous=NORMAL`、`foreign_keys=ON`（`connection.rs:102-118`）。后台 LLM 串行闸门 1 permit（`concurrency.rs:8-9`），Agency 全局 3 permit（`budget.rs:20`），per-story 锁用于资产桥接防并发重复行（`asset_bridge.rs:32-40`）。项目已启用 `unlock_notify`，历史死锁点已在装配路径修正（`coordinator.rs:4954-4959` 先算 scene_update 再开写事务）；同类「跨连接先读后写」模式未做全仓扫描（未核实）。

### 6.3 无界增长表（零剪枝）

| 表 | 增长驱动 | 现有上限 | 风险 |
|---|---|---|---|
| `agency_activity_log`（V129） | 每 run 每事件 1 行 | 仅读取 `LIMIT 200`，**无 DELETE** | P1 |
| `llm_calls`（V062） | **每次 LLM 调用 1 行**（含 prompt_preview 200 字） | 仅删模型时手动清 | P1，usage 聚合随表线性变慢 |
| `agency_board_items`（V107） | 每 run 写草稿（整章正文） | 无 DELETE、无 FK | P1，删故事成孤儿 |
| `agency_runs/messages/checkpoints` | 每 run / 每条黑板消息 | 无剪枝、`story_id` 无外键 | P1/P2 |
| `story_outlines`（V035） | 每故事 1 行 | ingest 侧封顶 4000 字/5 转折点；但 `materialize.rs:352` **整体覆盖** | P2：不膨胀，但累积转折点可能被覆盖丢失 |

### 6.4 索引缺口（均为热查询）

1. `llm_calls(story_id, created_at)`；2. `llm_calls(model_id, created_at)`（健康探测热路径）；3. `agency_board_items(story_id)`（现有索引全以 run_id 打头，跨 run 查询全表扫描）；4. `character_relationships(story_id, source, target)`（每章每关系三方去重查询）；5. `characters(story_id, name)`（每实体一次查找，且无 UNIQUE 约束）。

### 6.5 LanceDB 语义检索

索引表缺列时 **drop 并重建空表**（`lancedb_store.rs:171-178`），无 reindex/回填命令——向量目录丢失后语义检索永久静默返空；嵌入失败时静默回退 FNV-1a 哈希向量写进索引（`embedding.rs:288-300`）；表维度硬编码 384 而默认嵌入 1024 维（`project_to_dim` 截断，`provider.rs:505-515`）；启动时对积压章逐条补嵌入（`lib.rs:505-540`）构成启动成本。

### 6.6 LLM 韧性与预算/取消（历史问题复核）

- 超时/重试语义清晰：连接超时可重试，`LlmTimeout` 不重试；候选链遇**取消即停**、超窗候选跳过。
- **「取消不传播给已 spawn 的 ingest」确认仍未修**：`spawn_asset_ingest`（`coordinator.rs:2731`）不注册取消，`run_asset_ingest` 自建 300s 自毁 token（`continue_loop.rs:435-440`）；`spawn_editor_qc` 同。且这些后台调用各自 `new` 独立预算（`:2654`），**不计入任何 run 的 token 账**（v0.37.0 backlog 至今成立）。

### 6.7 本地数据与密钥

配置（含各家 API key）序列化为 JSON 存主库 `app_settings`，**无 OS keyring**；**导出泄漏面（P1）**：`export_studio` 默认 `include_llm_config=true` 把含 `api_key` 的 `studio_config.json` 原样打进可分享 ZIP（`studio_manager.rs:127-129,172-175`）。`llm_calls.prompt_preview` 会把正文前 200 字长期留在库内（无剪枝）。

---

## 七、测试体系评估

### 7.1 规模与结构

Rust：1591 个测试属性（242 个 inline `#[cfg(test)]` 块），集成测试目录仅 1 文件 1 个测试；2 个 `#[ignore]` 均为真实模型集成且硬编码内网 IP（`10.62.239.13`，`intention_graph/tests.rs:426,517`）——CI 不可能运行。测试重灾区与生产重灾区**重合**：`agency/tests.rs` 89 个、`orchestrator.rs` 47、`write_time_bundle.rs` 34、`asset_refresh.rs` 34、`continue_assets.rs` 29、`beat_card.rs` 23。

前端：99 个测试文件；大文件覆盖极不均——`FrontstageApp.tsx` 有 16 个测试文件（3870 行）覆盖最好；而 `Stories.tsx`(1447)、`GeneralSettings`(1116)、`WorldBuilding`(1076)、`Foreshadowing`(916)、`WenSiPanel`(780)、`Tasks`、`Skills`、`App.tsx` 等均无相邻测试；47 个 hooks 仅约 7 个有测试。

### 7.2 Mock 保真度与质量判定

主 mock（`agency/tests.rs:30` `MockLlm`、`ports/testing.rs:23`）是**无延迟、无错误形态的脚本队列**——验证的是「给定第 N 条响应，状态机怎么走」，不是「真实网关会怎么坏」。真实故障（reasoning_content 泄漏、空 content、围栏 JSON）只在纯函数层有测试。唯一有时序保真的是 `RoutingMock`（记录每角色 start/end，用于预算断言）。

断言构成：全仓 `contains("…")` 1221 处，其中 1036 处（85%）是中文文案；热区中文子串断言占比 55–75%（`continue_assets.rs` 79/105、`write_time_bundle.rs` 61/89）。**结论：这是「高数量、快、确定性强」的回归网——每个真机血案都被钉成可复现契约（历史回归极少复发），但它不度量质量**：无 golden 章对比、无 provider 故障矩阵，「续写质量差」结构上无法被这套测试发现。

强例：`agency/tests.rs:1073` 真库端到端续写；`asset_bridge.rs:1196` 真并发 TOCTOU 回归；`agency/tests.rs:3841` 质检失败 salvage 边界（姊妹用例断言短稿照丢）。弱例：`test_editor_verdict_pending_defaults`（锁构造函数字段）、字体 CSS 正则锁字节数、`assert!(full.contains("【本拍状态网】"))` 排版锁。

### 7.3 E2E：名存实亡

根目录 `e2e/` 9 个 spec / 34 个 test，最后更新 2026-08-12（部分 7 月），落后 9 个版本；CI 中 `e2e-check` 挂 `continue-on-error: true`（`build.yml:165`）且依赖 260 行假 IPC（`e2e/mock-tauri.ts`），对新增 IPC 天然失明；16 处 `waitForTimeout` 固定等待。**v0.41+ 的 Agency 续写主路径没有任何 E2E 覆盖。**

---

## 八、核心痛点专题：续写质量防线

**当前五层防线**（幕前 Append 实际路径）：

1. 写前：节拍卡 + 导演锁 + 冻结 pin（Rust 0-LLM 编译，可选 Producer enrich）；
2. 写：11 行合同 + 6 组 Wrong/Right 范例，单次 `complete()` 零工具；
3. 后处理：CoT 剥离 → 自重复 ≥8% 重试一次 → <200 字短稿重试一次 → 探针缺口重试一次（取更优）；
4. 落库：抗重复三件套 cleanup；
5. 后台：`spawn_editor_qc` fail-open，审查意见回灌下一拍节拍卡（≤2 条）。

**判断**：这是**分层补丁而非统一设计**。症状驱动逐条加正则（v0.56.1 抱衣角、v0.56.2 下引号、v0.53.6 死人重演……），探针只重试一次、失败即 salvage 落库；最关键的是**用户可见路径没有同步质量门与修订循环**——质量门的 revise 能力只存在于无 UI 入口的批量路径。因此当前的质量保障实质是「事前约束强、事后校验弱」。

**建议**（按收益排序）：

1. 给 Append 落库后补一次**异步可修订**轮（复用现成 `evaluate_gate_impl`），或在幕前暴露「质检不达标 → 重写本拍」按钮，把后台 QC 从通报升级为闭环；
2. 建立**端到端质量度量**（固定开头 + 固定模型 + golden 章对比 + 人工评分基线），否则任何质量声称都不可验证；
3. 把真机血案的修复模式从「加探针」升级为「补 provider 故障注入测试 + 探针」双轨，避免探针列表无限膨胀。

---

## 九、治理：CI / 发布 / 文档

### 9.1 CI 门禁缺口（本次最重要治理发现）

| 门禁 | 现状 | 后果 |
|---|---|---|
| `cargo test --lib` | **continue-on-error: true**（`build.yml:96`），注释称 49 个 V092 基线失败（:95）——实测 0 failed | 1583 个测试在 CI 不设防，注释依据已过时 |
| `cargo clippy` | 无 `-D warnings`（:86） | 只保证能编译 |
| Playwright E2E | job 级 continue-on-error（:165） | 咨询信号 |
| `src-server` / `src-server-web` | 完全不在 CI（仅 deploy 工作流引用） | 账号服务的测试无执行 |
| landing vitest（24 test） | 不在 CI | ✅ 但无人执行 |
| 正面 | `fmt --check` 阻塞、`agency::eval_harness` 阻塞、`architecture_guard` 独立阻塞、pre-commit 钩子覆盖 src-tauri + src-frontend | — |

### 9.2 发布链路（实测断裂）

- **deb 404**（实测）：`RELEASE_FILES` 白名单（`upload-releases-ftp.mjs:30-39`）含 msi/dmg/app.tar.gz/AppImage 与 latest.json，**缺 `.deb`**；而 Tauri 生成的 `latest.json` 含 `linux-x86_64-deb` 条目 → deb 用户检查更新 404。
- 命名一致性：macOS dmg / Windows msi（zh-CN）/ Linux AppImage 三个渠道线上实测 200，与 `useLatestRelease.ts` 的 `buildReleaseUrls` 一致 ✅；`bundle.targets` 声明的 nsis/rpm 实际从不构建（配置噪音）。
- 保留策略（前清理 + 保留 5 版 + latest.json 最后上传）设计合理 ✅；两处 FTP 均 `secure: false`（明文凭据与产物）⚠️。

### 9.3 版本/文档规则合规率

- **规则 3（版本四件套 + landing 兜底）= 100%**（实测）。
- **规则 4（每次推送更新 8 份文档）**：按发版提交口径抽查 5 个区间 **5/5 = 100% 合规**；按「每次推送」字面口径，最近 40 个非 merge 提交中 30 个（75%）合规（缺口集中在开发中间提交）。规则文本比实际执行更严，建议把规则改为「发版必更 + 中间提交至少更新 CHANGELOG/AGENTS」。

### 9.4 文档堆积与 AGENTS.md 膨胀

- 根目录 44 个 md 中 **36 个（82%）逾两个月陈旧**（剔除批量改名提交后真实陈旧度 3–6 个月），且抽查几乎全是孤儿（仅被 CHANGELOG/AGENTS_HISTORY 引用）；`docs/plans/` 90 份、`docs/` 根 19 份、`docs/archive/` 7 份持续堆积。
- **`AGENTS.md` 1208 行 / 165KB，88% 是「最近完成的功能」版本流水**——它已经在系统上下文里被截断，即「自己装不下自己」。建议：根文件只留「关键教训 + 强制规则 + 最近 3–5 版」，其余移入 `docs/archive/AGENTS_HISTORY.md`，目标 ≤300 行。
- 收敛方案（建议）：CODE_AUDIT_REPORT×2、PERFORMANCE_*×2、QA-Stage*×3、ARCHITECTURE_REVIEW/UPDATE、各类 v5.x/v0.9/v0.22 审计 → 合并归档到 `docs/archive/`（每类一份带日期说明）；`*_INSTALLED.md` 类安装快照 → 删除。

### 9.5 安全检视

| 项 | 判定 | 证据 |
|---|---|---|
| 仓库 secrets | **干净** ✅ | 模式扫描（sk-/ghp_/AKIA/PRIVATE KEY/赋值式）零命中；`.gitignore` 覆盖 .env/*.db/config.json/证书；仅 `.env.example` 被跟踪（历史 git 未扫描，未核实） |
| 桌面端 WebView | **风险（P1）** | `withGlobalTauri: true`（tauri.conf.json:13）+ CSP `unsafe-eval/unsafe-inline`、`connect-src *`（:15）→ XSS 可直达 42 个已授权 IPC；`http:default` 为死权限（前端零 import）；无 fs scope 限定 |
| src-server 认证 | 设计正确 ✅ | JWT 验签 + sessions 表吊销 + disabled 检查（有测试）；Admin 查库校验 role；SQL 全参数化 |
| src-server 配置 | **条件性 P0** | `JWT_SECRET` 缺省 `storymoss-default-secret-change-me`（`config.rs:39`）/compose 缺省 `change-me-in-production`；`DEV_UPGRADE_ENABLED` 缺省 true（`config.rs:52-54`）→ 非 compose 部署即自助升级后门；**线上实际取值无法从仓库验证** |
| src-server 其他 | P1/P2 | 零限流；session token 明文入库；web 端 JWT 存 localStorage（键名仍 `sf_token`） |
| landing | P2 | jsdelivr 字体 CSS 无 SRI；桌面端启动请求 Google Fonts（幕前字体已本地化） |
| 传输 | P1 | FTP 明文（两处 `secure: false`）；安装包完整性由 minisign 兜底 |

---

## 十、风险总表

### P0（建议立即处理）

| # | 风险 | 证据 | 建议 |
|---|---|---|---|
| 1 | CI 的 `cargo test --lib` 永不阻塞，禁用依据（49 个基线失败）已被实测推翻 | `build.yml:95-98` + 本次 0 failed | 去掉 `continue-on-error`（或先只在 master 阻塞），同步改注释 |
| 2 | `src-server` JWT 缺省密钥 + DEV_UPGRADE 缺省 true | `config.rs:39,52-54`；compose 覆盖了但裸跑无 | 去除可预测缺省（缺失即启动失败）；DEV_UPGRADE 缺省 false；核对线上 env |
| 3 | 幕前续写无同步质量门，质量闭环缺失 | `coordinator.rs:3249-3250`、质检仅后台 fail-open | 见 §8 建议 1：后台 QC 升级为可修订闭环 |
| 4 | 迁移无 checksum + `MAX(version)` 水位线 | `connection.rs:62,123-128` | 记录内容 hash；水位线改「已应用集合」；低版本补丁走显式修复通道 |

### P1

5. deb 更新链 404（`RELEASE_FILES` 补 `.deb`/`.deb.sig`，一行修复）；6. target 影子迁移副本仍在候选序前（删目录或调整候选序）；7. 后台 spawn（ingest/QC）不在预算与取消内；8. `llm_calls`/`agency_*` 零剪枝无外键；9. 导出 ZIP 明文携带 API key（默认改为不含 key）；10. LanceDB 无回填 + FNV-1a 静默降级 + 384 维截断；11. `agency_writer_system` 等占位 prompt_id 导致 Writer 提示词退化且不可覆盖；12. 5 处索引缺口；13. `withGlobalTauri` + 宽松 CSP + 死权限 `http:default`；14. AGENTS.md 膨胀至 1208 行（自我截断）。

### P2

15. 巨石文件拆分（coordinator / FrontstageApp / Stories 等）；16. 死代码清理（4 TipTap 扩展 + 4 孤儿 hooks + `hybrid_search.rs` + `execute_trishot` + 4 个无 UI Agency 命令）；17. `story_outlines` 覆盖语义冲突；18. 迁移 `;` 切分与静默跳过；19. 前端重复模式脚手架（extractMessage/toast/CRUD）；20. E2E 重建（Agency Append 主路径）+ 提升出 `continue-on-error`；21. landing SRI、web localStorage token、FTP 换 SFTP/FTPS；22. 根文档堆积收敛。

### 建议路线

- **第一批（1–2 天，低风险高收益）**：P0-1（CI 门禁）、P0-2（server 缺省值）、P1-5（deb）、P1-6（删影子副本）、P1-9（导出不含 key）、P1-14（AGENTS.md 瘦身）。
- **第二批（1–2 周）**：P0-3（质检闭环）、P0-4（迁移治理）、P1-7/8/12（后台预算与剪枝、索引）、P1-11（补提示词资产）。
- **第三批（中期）**：端到端质量度量（golden 章 + provider 故障注入）、E2E 重建、巨石拆分。

---

## 十一、结论

StoryMoss 在**功能交付与本地工程质量**上是健康的：1583+607 测试全绿、启动路径无 panic 隐患、版本与发版流程高度自律（发版文档合规 100%）、0-LLM 的 Rust 约束编译层（节拍卡/导演锁/情感账本）是超出同类项目的扎实设计。它的问题不在「会不会崩」，而在**证据链与质量闭环**：CI 不强制测试、E2E 退化、测试只能锁住已知症状而非度量质量、用户可见的续写路径没有修订回路——这正是「续写质量」在十几个版本里反复出现、且团队自己始终标注「不得宣称已修复」的结构性原因。安全与发布链路上则存在一批可快速止血的确定性缺口（server 缺省密钥、deb 404、导出带 key）。

按本报告 §10 的三批路线推进，第一、二批合计约两周即可把「证据链」补齐；第三批决定这个项目能否把「小说质量」从玄学变成可度量的工程指标。

---

## 附录 A：未验证项（不作为结论依据）

- 线上 `JWT_SECRET` / `DEV_UPGRADE_ENABLED` / `FRONTEND_URL` 的实际取值；nginx 层限流；GitHub 分支保护是否把 `architecture-guard` / `build.yml` 列为必需检查。
- git 历史中是否曾提交过真实 secret（本次只扫了工作树）。
- `unlock_notify` 死锁点是否还有其它「跨连接先读后写」残留（仅定点核查装配路径）。
- LanceDB 目录损坏的具体错误路径；删故事是否清理向量行。
- 陈旧根文档的内容与当前模块的逐条矛盾（按版本号与时序推断，未逐句核对）。
- `AiInsightCards` 的 `tone` 调用点收敛情况。
- 候选链超时/退避在真实网关下的端到端行为（需真机）。

## 附录 B：本次实测命令与结果

```bash
cd src-tauri && cargo test --lib          # 1583 passed; 0 failed; 2 ignored; 167.73s
cd src-frontend && npx vitest run         # 607 passed | 3 skipped (610); 99 files + 1 skipped
cd src-frontend && npx tsc --noEmit       # 0 errors
python3 scripts/architecture_guard.py     # pass（removed singletons 14 / tracked violations 0）
cd src-frontend && npm run format:check   # pass
curl -I https://storymoss.top/releases/StoryMoss_0.58.0_amd64.deb      # 404
curl -I https://storymoss.top/releases/StoryMoss_0.58.0_aarch64.dmg    # 200
```
