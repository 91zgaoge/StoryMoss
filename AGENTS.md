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
- `cargo test -p storymoss` ✅ 1816 passed / 9 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划 / 判官一致率 / 手法回灌 / 卷纲 / 稳定性回归门·概念体检 / 语料校准 / 真机功能探针·共 9 ignored）
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

### v0.72.2 - 真机功能探针（v0.66.0–v0.72.0 首次真机验收）+ 官网换实拍

**新增（测试基建，全 `#[ignore]`）**：`agency::real_machine_probes`——真机库/向量库副本 + 真机
AppConfig 构造真实适配器（写作档 `creative_model_id`、编辑档 `background_model_id`，按 `TaskType`
路由），三项探针：①续写（检索块注入 + 手法落库 + 同步质量门 → 判官裁决落库）；②卷纲生成 +
章纲 prompt 注入契约；③重规划预览与「预览不落库」。`life_status` 真机探针加数据形态守卫。

**真机结论**：✅ v0.66 检索（第 1/2 章原文按细节召回，增量 1510 字/100s）；✅ v0.69 手法落库；
✅ v0.70 卷纲 + 章纲注入契约；✅ v0.67 预览不落库；✅ v0.72 语料校准对真机 3 章 0 误报；
⚠️ v0.68 链路可达但编辑档模型不产出裁决。**发现**：①deepseek-flash 是推理模型（长输出 content
空 → 章纲类任务不可用）；②真机质量门编辑档达最大轮数熔断（审查打折，建议换遵循 JSON 的模型）；
③仪表盘「创作路径」面板 ≤1115pt 文字竖排；④知识图谱画布真机不渲染（待复现）。
**官网**：三张截图换真机实拍（旧图实为连接错误态截图）。

- **验证**：`cargo test --lib` 1816 passed / 9 ignored（+3 ignored）；clippy/fmt/guard ✅；
  官网 25 passed / tsc / prettier / build ✅。
- **未关闭**：发现 a/b 需换模型后复跑；c/d 未修；后台质检链路真机手动续写观察。

### v0.72.1 - 文档与官网同步（无代码变更）

**为什么**：Sherpa 借鉴清单八项在 v0.66.0–v0.72.0 全部落地后，专题文档与官网内容落后于实现
（专题文档停在 v0.17–v0.23 口径、审计无落地状态、官网仍写「五维审查/模式卡/四级评分者」）。

**改动**：①专题文档——`docs/FEATURES.md` 新功能补充、`docs/CREATION_FLOW_AND_ASSETS_REFERENCE.md`
「增补与更正」（创世/续写主线、新资产/新表/VOLUME 语义/注入块）、`docs/MODULE_CHARTER.md` agency
行 + 关键子模块登记 + prompts 禁依赖规则、`docs/TROUBLESHOOTING.md` 4 条新排查 + 版本/死链修复、
`KIMI.md` 版本、`docs/USER_GUIDE.md` 头部；②审计落地状态——Sherpa 审计加「落地状态」表（八项→
版本与契约，P2-8 未落地）、ai-novel 审计加「八、落地状态补记」、project-review 加治理注记、
laya 审计修正引用；③16 份历史快照加横幅；④`ARCHITECTURE.md` 顶部清单补 v0.66–v0.72.1 八条；
⑤官网新增「长篇」区块（按需检索/卷纲/改稿不漂/跨拍守卫）+ 修正三处过时文案 + CTA/meta/测试。
代码零改动。

- **验证**：Rust 基线不变（1816 passed / 6 ignored）；官网 vitest **25 passed**（+1）、tsc / build /
  prettier 全绿；推送触发 deploy-landing。
- **未关闭**：官网截图仍为 2026-07 旧版（需运行应用重截）；`public/screenshots/` 5 张未引用图保留。

### v0.72.0 - 人类语料校准（Sherpa 借鉴 P2-7）

**改动**：引入 `resources/voice_corpus/`（6 部 PG 公版中文小说、各取正文前 2 万字符，附来源/许可）
+ `human_voice::corpus_calibration` 探针（`#[ignore]`，打印逐篇规则命中与 SD/CV 分位，可落 JSON，
语料缺失跳过不红）。**据证据调整**：`FLAT_RUN_LEN` 3 → 4（人类语料 5/6 篇命中「恰好 3 句」、
机器腔样本该规则零命中——3 是噪声门；提到 4 后人类只剩 1/6、机器腔命中数不变）；
**解耦隐患修复**：句长离散度指标不再与发现门共用常量（新增 `SD_MIN_PARAGRAPH_SENTENCES=3`，
否则改门会让指标从 12–14 漂到 6–9）。报告
`docs/audits/2026-10-14-human-voice-corpus-calibration.md`（含未动规则理由：语气词规则在明清
文言上 1/6 误报但放宽会削弱现代语域判别力；段落 CV 单独无判别力、「且无短段」合取承重）。

- **验证**：`cargo test --lib` **1816 passed / 6 ignored（+1 ignored）**；机器腔探针复跑命中不降
  （5 条）、人类对照样本 0 误报；clippy 0 error / fmt / guard / tsc / vitest 625 全绿。
- **契约**：`flat_rhythm_flags_uniform_sentence_run_but_not_varied_prose`（更新）；
  `voice_corpus_calibration_probe`（ignore）。
- **未关闭**：明清白话 ≠ 现代网文语域（下限参照）；未断言人类语料命中率上限；探针不进 CI。

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
