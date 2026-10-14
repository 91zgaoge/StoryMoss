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
- `cargo test -p storymoss` ✅ 1816 passed / 5 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划 / 判官一致率 / 手法回灌 / 卷纲 / 稳定性回归门·概念体检）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 625 passed / 3 skipped（+11 幕前切章正文同步 / 幽灵保留 / 懒加载可重开；+2 章纲重规划预览/勾选与忽略；+判官一致率卡；+章法偏好渲染与重置；+2 卷纲页签与合同展示；v0.68.1 分章切换用例 CI 超时加固：`asyncUtilTimeout` 5s + 三处显式等待 10s）
- `npx playwright test` ✅ 39 passed / 5 skipped（门禁仍非阻塞，见未关闭）
- `cargo +nightly fmt` ✅
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.54.0 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 瘦身：根文件只保留最近 5 个版本与关键教训）。

### v0.71.0 - 长篇稳定性回归门 + 创世概念体检（Sherpa 借鉴 P2-6/P2-9）

**改动**：①P2-6 `agency::serial_harness`（确定性 MockLlm、进 CI）：同场景四拍串联验证跨拍
状态约束——写死 → 重演死亡被拦 / 使用不在场持有者物品被拦 / 说出 secrets_unknown 被拦，
违规稿不得落库、合规重试稿落库、死亡状态四拍后仍在；守卫测试
`tests/serial_harness_guard.rs` 防回归门变空门。②P2-9 概念体检：`agency_problem_logline`
加三问（张力/赌注/心理）与「首行 logline + 三行诊断」格式；`split_logline_diagnostics`
切分（advisory 只记日志、不污染 premise、旧格式不误伤）；registry 契约测试防三问被删。

- **验证**：`cargo test --lib` **1816 passed / 5 ignored（+3）** + 守卫 2 项；前端未改动
  （vitest 625 / 3 skipped）；clippy 0 error / fmt / guard / tsc 全绿。
- **契约**：`serial_stability_across_beats`；`logline_split_keeps_single_line_and_extracts_diagnostics`；
  `problem_logline_carries_three_checks`；`serial_harness_keeps_cross_beat_probes_and_violation_markers`。
- **未关闭**：真机未复跑；harness 测跨拍约束回路而非模型文笔；概念体检诊断依赖模型遵循格式
  （不遵循则静默退回旧行为）；诊断只进日志无 UI。

### v0.70.0 - 卷/弧中层计划：VOLUME 合同（Sherpa 借鉴 P1-3）

**为什么**：审计 `docs/audits/2026-10-14-sherpa-benchmark-analysis.md`——Sherpa 的层级规划器
是「季→弧→序列→剧集→场景」；StoryMoss 的三幕书纲与逐章章纲之间没有正向计划层（段摘要每
10 章是回顾不是计划），而 `story_contracts` 的 `VOLUME` 类型早已定义、读路径已实现、全库无
写入方。

**改动**：新模块 `story_system/volume_plan`（卷纲存 VOLUME 合同；同卷号原地更新、version
递增；状态机 no_plan/in_volume/past_end；渲染注入块）；`AgencyCoordinator::generate_volume_plan`
（Producer JSON 调用 + 新资产 `agency_volume_planner_system.md`；作者显式触发）；章纲生成
注入 `volume` 变量（`scene_outline.md` 的 `{{#if volume}}` 条件块——无卷纲时 prompt 逐字不变）；
指南针规模接卷；运行维护「卷纲」页签 + 合同页展示；命令两件。

- **验证**：`cargo test --lib` **1813 passed / 5 ignored（+6）**；vitest **625 passed / 3
  skipped（+3）**；clippy 0 error / fmt / guard / tsc / playwright 39 全绿。
- **契约**：`test_chapter_outline_prompt_carries_volume_goal`；`overview_states_progressively`；
  `save_list_and_current_round_trip`；`compass_scale_links_to_volume_plan` 等（详见 CHANGELOG）。
- **未关闭**：真机未复跑（**不得宣称中观计划已在真机生效**）；卷纲 v1 不可手改；章号顺延
  不重编号卷范围；短剧未接卷纲。

### v0.69.0 - 作者裁决回灌每拍手法池（Sherpa 借鉴 P1-5）

**为什么**：审计 `docs/audits/2026-10-14-sherpa-benchmark-analysis.md`——Sherpa 用行为数据
训练散文引擎；StoryMoss 的对位是把作者裁决变成每拍决策的权重。v0.65.0 起每拍注入一条写作
手法（8 条池按已写篇幅轮换），但作者的 Tab 接受 / Esc 放弃从未影响下一拍用哪条。

**改动**：手法池加稳定 id（`Technique{id,label,text}`）；**V147 `beat_technique_log`** 每拍
落一行；新模块 `agency::technique_feedback` 按**时间窗**归因反馈（幕前反馈不带 scene_id →
取「反馈时刻之前最近一拍」且 ≤1 小时、`agent_type` 为 writer），产出统计并可重置；
`human_voice::choose_technique_index`（纯函数）：样本 <5 走基准轮换、有「质量 ≥0.5 且近期
未用」的手法则优选、基准位质量 ≤0 则前移避开；`write_beat_once` 选招传入 prompt
（`render_writer_user_prompt_with_technique`，旧函数保留为 `None` 包装，顺序契约测试不动）、
Append/NextChapter 两条落库路径写日志、学习中心新增「章法偏好」区块 + 重置命令。纪律：
只影响一条写作手段，不参与门限。

- **验证**：`cargo test --lib` **1807 passed / 5 ignored（+10）**；vitest **623 passed / 3
  skipped（+1）**；clippy 0 error / fmt / guard / tsc / playwright 39 全绿。
- **契约**：`choose_technique_prefers_proven_good_and_avoids_proven_bad`；
  `stats_attribute_feedback_by_time_window`；`test_continue_logs_beat_technique_and_replays_author_verdict`；
  `v147_creates_beat_technique_log_idempotently` 等 11 项（详见 CHANGELOG）。
- **未关闭**：真机未复跑（**不得宣称作者偏好已真正改善续写**）；归因是时间窗近似；
  「modify」类反馈前端尚未产生。

### v0.68.1 - v0.68.0 的 CI 收尾（分章切换用例超时加固，仅测试）

**为什么**：v0.68.0 的 tag 构建挂在 `frontend-check`——`FrontstageApp.split-auto-switch.test.tsx`
在 Linux CI 上有两条用例先后因 `waitFor` 超时失败（默认 1s 上界下 `chapterListReloadCalls()
.length` 0>0；另一条 5s 上界在 5015ms 超时）。同内容在 **v0.67.0 的 tag 构建全绿、本机重复跑
全绿、v0.66.0（未改前端）同样红**——纯时序抖动，非产品缺陷。按「不覆盖已有 tag」规则单独发版。

**改动（仅测试）**：`configure({ asyncUtilTimeout: 5000 })` 统一该文件等待上界；三处曾超时的
显式等待 5s → 10s。产品代码零改动。

- **验证**：vitest 622 passed / 3 skipped；该文件连续重复跑 3 次全绿；tsc / prettier ✅；
  未改 Rust（沿用 v0.68.0 基线 1797 passed / 5 ignored）。
- **未关闭**：v0.66.0 / v0.68.0 的 tag 构建因超时未产出安装包（其 `tauri-build`/上传步骤随之
  跳过，不污染 `latest.json`）；本版构建通过后以本版为发布版本。

### v0.68.0 - 判官一致率（Sherpa 借鉴 P1-4）

**为什么**：审计 `docs/audits/2026-10-14-sherpa-benchmark-analysis.md`——Sherpa 用「评估器与
人类审阅者 80–90% 匹配率」证明判官可用；StoryMoss 的 editor 判 blocking 时必须引用草稿原文
（`blocking_issues[].evidence`），但这条材料此前被压平成字符串、从未落库，没有指标回答
「编辑判的问题，作者后来改了吗」。

**改动**：**V146 `editor_gate_records`**（每次 gate 判定落一行；findings_json 原样保留含
evidence 的结构；写入点在 `evaluate_gate_impl` 三条调用路径共用，best-effort）。新模块
`agency::editor_agreement`：**引用幸存判定**（确定性）——引用的原文在作者改后成稿里近逐字
留存（char-bigram 包含率 ≥0.8；引用 ≥6 字才可判）= 未采纳，被删/被改写 = 采纳；每章取最新
gate 记录，配 `human_signals`（修改率）对照。`agency_eval_overview` 新增 `editor_agreement`
段；AgencyEval 页新增「判官一致率」卡（总采纳率 / 阻断命中 / 命中密度 + 每章
阻断·可判·采纳率·修改率对照表）。**只报不改**：不参与门限、不自动调权重；评审模型档位
（editor 默认落 Background，`llm/service.rs:267-280`）仅在文档说明。

- **验证**：`cargo test --lib` **1797 passed / 5 ignored（+5）**（引用三态 / 归一 / 记录取舍与
  计分 / 短引用与无成稿不可判 / V146 幂等）；vitest 622（AgencyEval 卡渲染断言）；
  clippy 0 error / fmt / guard / tsc / playwright 39 全绿。
- **契约**：`quote_survival_three_states`；`compute_agreement_joins_records_and_final_text`；
  `short_evidence_and_missing_final_are_not_judgeable`；`v146_creates_editor_gate_records_idempotently`。
- **未关闭**：真机未复跑（需 ≥3 章后人工核对 1 个样本），**不得宣称判官质量已可量化到可信**；
  未覆盖「误报」维度（需作者显式忽略/回退信号）；一致率只报不改。

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
