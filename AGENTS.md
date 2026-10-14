# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.65.4
- **GitHub**: https://github.com/91zgaoge/StoryMoss
- **技术栈**: Tauri 2.4 + Rust 1.95.0 + React 18 + TypeScript 5.8 + Vite 6 + SQLite + LanceDB
- **双界面**: 幕前 `/frontstage.html`（沉浸式写作），幕后 `/index.html`（工作室管理）

## 关键教训（必读）

> 完整档案见 `docs/archive/LESSONS_LEARNED.md`。以下是代价最高的一条，任何涉及启动流程/`State`/窗口创建的改动前必读。

**tauri setup 建窗顺序竞态（v0.33.5 根治，Windows 启动闪退 BEX64/c0000409）**：

- tauri 的 `app::setup()` **先创建 `tauri.conf.json` 配置窗口、后调 `.setup()` 闭包**。Windows 上 WebView2 环境创建会泵消息循环数秒，前端加载完立即发 IPC，若此时 `State` 尚未 `manage()` → `state() called before manage()` panic → 发生在 WebView2 COM 回调（`extern "C"`）内无法解退 → 进程直接 abort，无日志。
- **铁律**：任何 `State` 必须在第一个窗口/WebView 创建前 `manage()`；配置窗口一律 `create: false`，由 setup 末尾在所有状态就绪后用 `WebviewWindowBuilder::from_config` 显式创建（见 `src-tauri/src/app.rs`）。
- `extern "C"` 边界（COM 回调/WNDPROC/WebView IPC）内的代码必须不 panic。
- 诊断 Windows GUI 崩溃无日志时：临时切控制台子系统（去掉 `windows_subsystem = "windows"`）从终端拿 panic 消息；项目已内置启动面包屑（`startup_trace.rs`）与早期 panic hook。

## 编码风格

- **Rust**: `snake_case`，`Result<T, E>`，异步 `async/await`，数据库 `rusqlite` + `r2d2`。
- **TypeScript**: `camelCase`，函数组件 + Hooks，Zustand 状态管理，TanStack Query 调用后端。
- **AI 原生组件**: `src-frontend/src/components/ui/ai/`（P1 生成体验：AiLoading/AiThinking/AiStreamingText/AiPromptBar/AiApprovalCard；P2 代理与任务：AiContextCards/AiToolChips/AiRecommendationCard/AiTaskRows；P3 数据展示：AiSearchList/AiCodeBlock/AiDiffTable/AiFilterTable/AiRecordsTable/AiInsightCards），只引用 `--ai-*` 语义令牌（幕后 tokens.css / 幕前 frontstage.css 各自定义），不写死颜色；tint 缺口用 color-mix 内联零扩令牌，契约现为 17 变量（P4 新增 `--ai-on-accent`；v0.46.0 起跟随当前窗主题 `onAccent`，不再写死 #ffffff）；动画用 tailwind.config.js 注册的 ai keyframes 工具类；受控组件，禁止引入自运行演示逻辑；组件内嵌私有动效/图表（如 AiInsightCards 的 MiniLineChart）不复用为公共 API。v0.49.1 卸掉幕前划词浮条 `AiSelectionActions`。

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
2. **本地构建仅在用户明确要求时执行**：推送后由 GitHub Actions 负责全平台构建（macOS `.dmg` / Windows `.exe`+`.msi` / Linux `.AppImage`+`.deb`）。**除非用户明确要求「构建」/「打包」/「生成本地安装包」，否则不要在本地执行 `cargo tauri build`**——本地 `cargo test --lib` / `cargo check` / `tsc` / `vitest` 等验证命令照常运行，仅省略耗时的打包构建。此规则为用户级永久指令，优先级高于本节其它条目。
3. **版本号统一**：`Git tag`、`Cargo.toml`、`src-tauri/tauri.conf.json`、`src-frontend/package.json` 必须一致。
4. **每次推送必须更新** `README.md` 与以下文档：`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`。
5. **版本标签**：每次推送使用新 tag，禁止 force push 覆盖已有 tag。
   ```bash
   git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z
   ```
6. **网站 Release 保留策略**：`.github/scripts/upload-releases-ftp.mjs` 每次上传后会自动清理 `/releases` 目录，仅保留最近 5 个版本的安装包（`RELEASE_RETENTION_COUNT=5`，可通过环境变量覆盖），防止服务器空间不足。禁止删除 `latest.json` 与无版本号文件（如 `StoryMoss_aarch64.app.tar.gz`）。v0.30.50 起上传**前**也会先清理（磁盘满 552 自愈——旧版仅上传后清理，磁盘已满时清理永远轮不到）；磁盘满等紧急情况可手动运行 `Cleanup Releases` 工作流（`.github/workflows/cleanup-releases.yml`，workflow_dispatch，可选保留数），它调用脚本的 `--cleanup-only` 模式，不上传只清理。
7. **网站下载页内容及时同步**（用户级永久指令）：落地页（`landing/`）下载区**运行时**从 `https://storymoss.top/releases/latest.json` 拉取版本号并拼出下载链接（`landing/src/hooks/useLatestRelease.ts`），因此每次发版后下载页版本号与链接**自动跟随最新 release，无需重新部署落地页**。注意：发版（tag push）**不**触发 `deploy-landing.yml`（它只在 `landing/**` 变更时构建部署），运行时 fetch 才是保持下载页新鲜的机制。两条强制维护义务：
   - **兜底版本必须随发版 bump**：`useLatestRelease.ts` 的 `FALLBACK_VERSION` 必须与 `Cargo.toml` / `src-frontend/package.json` 的版本号同步更新--这是 fetch 失败（离线/服务器故障）时下载链接仍指向有效版本的最后一道防线，否则兜底链接会指向已被保留策略删除的旧版本而 404。
   - **文件名规律变更时必须校验**：`buildReleaseUrls` 内的 bundle 命名（`StoryMoss_{version}_aarch64.dmg` / `_x64_zh-CN.msi` / `_amd64.AppImage`）在 Tauri bundle 命名或语言包变更时需重新对照线上 `latest.json` 核对。
8. **实现后核验功能和设计，持续迭代直到可上线**（用户级永久指令）：写完代码、勾完计划、提交/发版都不算完成。必须对照设计不变量/契约/验收指标与用户可点路径核验，发现缺口就修再核验，循环直到可上线。未跑通设计验收探针，不得宣称症状已修复。详见 `.cursor/rules/verify-until-shippable.mdc`。

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
- `cargo test -p storymoss` ✅ 1792 passed / 5 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 622 passed / 3 skipped（+11 幕前切章正文同步 / 幽灵保留 / 懒加载可重开；+2 章纲重规划预览/勾选与忽略）
- `npx playwright test` ✅ 39 passed / 5 skipped（门禁仍非阻塞，见未关闭）
- `cargo +nightly fmt` ✅
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.54.0 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 瘦身：根文件只保留最近 5 个版本与关键教训）。

### v0.67.0 - 纲层失效与局部重规划（Sherpa 借鉴 P0-2）

**为什么**：审计 `docs/audits/2026-10-14-sherpa-benchmark-analysis.md` 指出——Sherpa 的层级
规划器「识别变化、追踪下游影响、仅重规划受影响的部分」；StoryMoss 的失效重算（v0.64.11/12）
覆盖物料层（章摘要/分层摘要/快照/关系审计），但**章纲（计划层）不在失效范围内**：改了第 3 章，
第 4 章以后的章纲不受任何影响（改稿后的计划漂移是长篇工作流里的静默缺口）。

**改动**：`recompute` 新增 `KIND_CHAPTER_OUTLINE`（**刻意不入 `ALL_KINDS`**——计划不是物料，
不参与后台自动重算）+ `mark_stale_kind`/`clear_stale_kind` + `should_auto_recompute` 过滤
非物料类（防永不自动清除的计划标记每轮提交空转触发重算）。新模块 `agency::outline_replan`：
`chapter_outline_candidates`（复用 `chapter_plan_from_scene_outline`，手写大纲不误伤）+
`mark_outline_stale_for_edit`（确有下游章纲才标记，from=编辑章+1）+
`preview`（重生成 ≤3 章新纲、**不落库**）+
`apply`（只落库作者勾选条目，`set_chapter_plan_prefix` 保留卡块；确认到第 M 章后标记
清理后重标到 M+1，全部处理完才清除）。`update_scene` 在覆盖已有正文且确有下游章纲时标记。
命令 `preview/apply_chapter_outline_replans` + `dismiss_stale_material`；维护页
「物料重算」新增中文标签、新旧章纲对照对话框（**默认全部不勾选**）与「忽略」，
且「重算」按钮起始章不再被计划层行拉低。

- **验证**：`cargo test --lib` **1792 passed / 5 ignored（+4）**（三态标记 / 自动重算排除 /
  预览-应用往返含标记推进 / 非法条目跳过）；vitest **622 passed / 3 skipped（+2）**（预览
  勾选应用只提交勾选项 + 忽略）；clippy 0 error / fmt / guard / tsc / playwright 39 全绿。
- **契约**：`mark_outline_stale_only_when_downstream_plans_exist`；
  `outline_kind_is_excluded_from_auto_recompute`；`preview_and_apply_round_trip`；
  `apply_skips_mismatched_scene_and_empty_plan`；前端「章纲待重规划：预览后勾选应用」/
  「忽略清掉标记」。
- **未关闭**：真机端到端未复跑（编辑旧章 → 维护页 → 预览/应用 → 标记推进或清除），
  **不得宣称计划漂移已在真机闭环**；重规划手动触发（不自动改写）；每轮上限 3 章。

### v0.66.0 - 续写检索接线（Sherpa 借鉴 P0-1）

**为什么**：审计 `docs/audits/2026-10-14-sherpa-benchmark-analysis.md` 指出——Sherpa 的「叙事
世界模型」是「状态跟踪 + 按需检索」一对，StoryMoss 的状态侧（KG/知识边界/物品账本/分层摘要）
在生产路径，检索侧（LanceDB 向量检索）实现完整却**零生成调用**：`QueryPipeline` 模块级
`#![allow(dead_code)]`（`memory/query.rs:1`）、写时 Bundle 的参考场景退化为关键词重叠 top 3
（`write_time_bundle.rs:213-226`）。本版把「按需取精确上下文」接进续写主路径。

**改动**：新增 `src-tauri/src/agency/retrieval.rs`（纯函数 + fetch）——`build_retrieval_query`
以节拍卡为查询源（在场角色名 ≤4 / 地点 / 冲突双方 ≤2 / 下一节点 / 指令 ≤40 字，总 ≤200 字）；
`render_retrieval_block` 排除当前章及以后、score 降序同章去重、≤4 条 × ≤240 字、块 ≤1200 字、
摘录「【】」降级「「」」防段落标记混淆；`fetch_retrieval_block` 走 `embed_text_async`（全局
provider，缺省哈希回退）+ `ports::VectorStore::search`（`text_search` 是前缀 LIKE，中文多词
召回不了，弃用；RRF 在此场景等价向量路），任何失败静默降级。`AgencyCoordinator` 生产从 Tauri
State `try_state` 取库（拿不到即跳过）、`#[cfg(test)] for_test_with_store` 注入；注入点在
`write_beat_once` 的 `continuity_blocks` 生成后、`render_parts` 前（受保护段，位于 Bundle 区，
prompt 顺序契约不变）；开关 `AppConfig::continue_retrieval_enabled`（默认开，配置文件项）。

- **验证**：`cargo test --lib` **1788 passed / 5 ignored（+8）**：7 项单测（query 组装/截断/去重、
  块渲染排除当前章/上限/同章去重/括号清洗/空结果、章号前缀剥离）+ **召回探针**（FakeVectorStore
  埋「第 2 章 玉佩交给苏会山」→ 写第 40 章召回到）+ coordinator 集成（检索块进 prompt、当前章
  记录排除、无库整块缺席降级）。clippy 0 error / fmt / guard ✅；前端未改动（vitest 620 / 3
  skipped、playwright 39 / 5 skipped 沿用 v0.65.4 基线）。
- **契约**：`retrieval_recall_probe_finds_distant_detail`；
  `test_continue_injects_retrieval_block_and_degrades_without_store`；
  `block_excludes_current_and_future_chapters`；`block_caps_items_chars_and_dedupes_chapters`；
  `block_sanitizes_prompt_markers_and_returns_none_when_empty`；`query_collects_card_slots_and_instruction_with_caps`；
  `query_ignores_empty_slots_and_truncates_long_instruction`；`excerpt_strips_redundant_chapter_prefix`。
- **未关闭**：真机端到端未复跑（**不得宣称检索已改善续写质量**；真机探针=≥10 章的书+远距离
  细节关键词，观察命中与 token/延迟增量）；本地哈希 embedding 召回有限；无相关性阈值过滤
  （噪声靠块头约束 + 1200 字上限控制）。

### v0.65.4 - v0.65.3 的 CI 收尾（用例隔离）

**为什么还要一版**：v0.65.3 的 tag 构建挂在 `frontend-check`——新增用例在本机（macOS）绿、
CI（Linux）红，根因是**用例隔离**而非产品代码：幕前 `useFrontstageStore` 是模块级单例，
同文件上一个用例留下的正文让新用例的 `captured.content` 断言在应用加载完成前就通过
（CI 报 `Unable to find a label with the text of: 展开章节列表`，章下拉尚未渲染）。
按「不覆盖已有 tag」的规则单独发版补齐。

**改动（仅测试）**：`FrontstageApp.latest-chapter.test.tsx` 的 `beforeEach` 增加 store 复位
（`setContent('')` / `setSceneInfo('', '', undefined)` / `setSaveStatus(true, null)`），
每个用例从「无故事、空正文」冷启动；首次交互改用 `findByLabelText('展开章节列表')` 等候选出。

- **验证**：`npx vitest run` 620 passed / 3 skipped；tsc ✅；聚焦重复跑 3 次稳定绿；
  把 pre-fix 的 `FrontstageApp.tsx` 换回去该用例仍为红（`Already attempted lazy-load for chapter`），
  回归语义不变。未改 Rust（沿用 v0.65.2 基线 1780 passed / 5 ignored）。
- **未关闭**：同 v0.65.3（真机端到端未复跑，**不得宣称症状已在真机消失**）。

### v0.65.3 - 幕前「续写后正文消失 / 章节点不开」真机缺陷修复

**三条症状一条链路**：真机《帝国的烟火》第 2 章续写 → 自动分章出第 3 章 → 幕前自动切换，
用户看到 ①续文突然消失 ②重启后进第 3 章续文又在 ③点第 2 章再点第 3 章打不开。
本机日志（`storymoss.2026-10-08` + `creative_workflow.log`）留痕三处独立缺陷，逐条修：

- **懒加载守卫永久封锁**（症状③，日志两条 `Already attempted lazy-load for chapter`）：
  `lazyLoadingChapterIdsRef` 只 add 不 delete——懒加载过一次的章在本次会话内再也点不开。
  改为**在途去重 + 可重试**（结束即释放，重取而非缓存），递归重入仍无正文时用
  `opts.lazyLoadRetried` 中止，防死循环契约不变。
- **切章正文被焦点守卫吞掉**（症状②）：`RichTextEditor` 的「有焦点不强制 setContent（W2-F1）」
  与「Tab 接受后 30s 禁外部 setContent（v0.24.9）」在**无交互的分章自动切换**下把新章正文挡掉，
  屏幕停在旧章、章标题已切走。改成**以章 id 为文档身份**：章 id 变化优先于两道守卫（也不再
  受 1s/3 次熔断计数影响）；同章后台同步照旧让位于守卫。
- **切章丢弃未确认幽灵续文**（症状①）：`selectChapter` 无条件 `setGeneratedText('')` 吞掉用户
  刚看到、未 Tab 确认的续文。新增纯函数 `isGhostDeliveredInContent`：新章正文已含幽灵（整体/
  前缀或幽灵结尾——「重演旧文尾部 + 新续写」的分章形态）则照旧清空，否则保留并顶栏提示
  「未确认的续写内容保留在文末（Tab 接受 / Esc 放弃）」。

- **验证**：`npx vitest run` **620 passed / 3 skipped**（+11，含 3 章切换正文同步 / 2 分章幽灵保留 /
  1 懒加载可重开 / 5 纯函数）；`npx tsc --noEmit` ✅；`npx playwright test` 39 passed / 5 skipped ✅；
  prettier ✅。未改 Rust（沿用 v0.65.2 基线 `cargo test --lib` 1780 passed / 5 ignored）。
  **回归探针修复前逐条复跑为红**。
- **契约**：`RichTextEditor 切章正文同步`（3 用例）；「懒加载过的章节再次点击应能重新打开」；
  「分章切换：幽灵已随新章正文呈现时清空」/「幽灵有新章正文没有的文字时必须保留」；
  `isGhostDeliveredInContent`（5 用例，含真机分章形态文本样本）。
- **未关闭**：真机端到端未复跑（**不得宣称症状已在真机消失**）；自动分章仍可能在续写生成中触发
  （后端判据是场景落库后 30s 空闲，不知道幕前在途生成）——本版只保证切换后正文/幽灵不丢、
  该章可再次打开。

### v0.65.2 - 主续写路径 system 提示词补文笔准则

**为什么还要一版**：v0.65.0 注入的是**用户**提示词（每拍组装），核对链路时发现主续写路径的 **system** 提示词其实是 `CONTINUE_BEAT_SYSTEM`（`prompts/assembly.rs` 内联常量），**不是** `agency_lead_writer_system`——它是用户无法覆盖的常量，是准则送达最可靠的一条渠道，此前没有文笔条款。**改动**：`CONTINUE_BEAT_SYSTEM` 新增第 7 条文笔（人类文笔基线）+ 一组对照示例（Wrong：她心里一紧，背脊发凉，喉咙发紧／Right：她害怕。他把灯芯剪短了一截，没说话）；`DRAMA_BEAT_SYSTEM` 按**剧本语域**补第 6 条（情绪用可见动作与台词演，不用内心身体描写代替可拍内容；台词允许语气词与重复「说」）——sepia「按体裁校准，别把不同体裁改成一个声音」的落地。

- **验证**：`cargo test --lib` 1780 passed / 5 ignored（+2）；clippy 0 error；fmt / guard / tsc / vitest 全绿。
- **契约**：`continue_beat_system_carries_human_voice_doctrine`；`drama_beat_system_bans_unfilmable_interiority`；`continue_beat_operational_contract_has_three_examples`（行数上界 19→24，仍设界防膨胀）。
- **未关闭**：同 v0.65.1（叙事架构层无确定性检查；真机端到端未复跑，**不得宣称续写质量已修复**）。

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
