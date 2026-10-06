# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.62.0
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
- `cargo test -p storymoss` ✅ 1671 passed / 3 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 590 passed / 3 skipped（+5 级联中心页面：渲染/去查看/忽略/触发改写/空态）
- `npx playwright test` ✅ 39 passed / 5 skipped（新增幕前续写 spec 3 用例；门禁仍非阻塞，见未关闭）
- `cargo +nightly fmt` ✅
- `cargo clippy --lib` ✅ 本版未重跑
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.54.0 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 瘦身：根文件只保留最近 5 个版本与关键教训）。

### v0.62.0 - 文本质量与成本：prose_lint / 文风逆向学习 / 伏笔增强 / 成本账本

P2 阶段。**P2-A** `story_system::prose_lint` 纯 Rust 两档规则（blocking：注入术语泄漏 + 否定排比 + 章尾预告腔；advisory：破折号密度/重复句/极短收尾/开篇时间跳跃），接入 `auto_commit`（并入 review 记录）与 editor_qc 预注入块。**P2-B** V137 `style_preferences`：`update_scene` 人类编辑 → 防抖 120s + 单故事单处理器 → LLM 提炼可执行文风规则（资产 `style_delta_extraction`，标签「后台风格提炼」）→ 续写注入【作者文风偏好】。**P2-C** V137 给 foreshadowing_tracker 增 evidence/strength/subtlety/related_foreshadow_ids；ingest 自动登记上限 5 条；注入话术按计划回收窗口分档（临近「请勿提前回收」/过期「尽快回收」）。**P2-D** `llm::cost` 按故事聚合 + 阈值提示 + 零增量计费盲区检测（连续 ≥5 次零记账告警），命令 `get_story_cost_summary`。

- **验证**：`cargo test --lib` 1671 passed / 3 ignored（+18）；前端无改动（vitest 590 / 3 skipped）；nightly fmt / guard 全绿。
- **契约**：`flags_pipeline_header_leak_as_blocking`；`clean_literary_text_produces_no_blocking`；`parse_style_delta_validates_and_dedupes`；`style_signal_gate_filters_noise`；`service_hints_annotate_planned_payoff_window`；`zero_token_streak_is_flagged_but_normal_usage_is_not`。
- **未关闭**：真机验证；P2-B/P2-D 无前端 UI（命令已就绪）；**不得宣称续写质量已修复**；网站发布待 Apple 公证解阻。

### v0.61.0 - 分层记忆金字塔：语义摘要 + 段摘要 + 全书纲要 + 自适应窗口

P1 阶段（docs/plans/2026-10-06-p0-p3-roadmap-implementation.md）。**P1-A**：`scene_commits.summary_text` 从「前 1000 字截断」升级为 LLM 语义摘要（100-150 字，资产 `chapter_summary`；`parse_summary_response` 拒绝 JSON/过短/过长，失败回退截断）。**P1-B**：V136 `story_segment_summaries`（segment/book 两级，UNIQUE(story_id,level,segment_index)）——每 10 章由逐章摘要压缩段摘要（数据不足一半跳过），段数 ≥3 生成全书纲要；commit 成功后 `spawn_refresh_after_commit` 后台补齐（后台闸门 + LLM 失败仅告警）。**P1-C**：`adaptive_summary_window`（≤15→10 / 16-50→5 / >50→3）替换写死的「近 3 章」，工作记忆注入最近段摘要 + 全书纲要；续写资产新增【故事纲要】（长篇带段摘要 + 全书纲要，禁止直接复述）。

- **验证**：`cargo test --lib` 1653 passed / 3 ignored（+10）；前端无改动（vitest 590 / 3 skipped）；nightly fmt / architecture_guard 全绿。
- **契约**：`adaptive_summary_window_shrinks_with_book_length`；`segment_math_covers_expected_ranges`；`upsert_segment_summary_is_idempotent_per_index`；`story_so_far_block_includes_segments_only_for_long_books`；`parse_rejects_json_too_short_and_too_long`。
- **未关闭**：真机 10+ 章验证摘要质量；**不得宣称续写质量已修复**；网站发布待 Apple 公证解阻。

### v0.60.0 - 三把尺子：知识边界 / 物品归属 / 改稿级联影响报告

对照外部五项目对比报告（docs/audits）落地 P0 阶段，把「防吃书」从提示词叮嘱变成可校验机制。**V135** 新增四张表：`story_timeline_events`（世界真相 / 读者认知 / 揭示状态机双栏建模）、`character_knowledge_log`（知情变更审计流水）、`item_holdings`（关键物品持有者账本）、`cascade_impacts`（改稿影响报告）。**知识边界**：ingest 新增 `knowledge_updates`/`timeline_events` 抽取，修掉 secrets 被 COALESCE 永久冻结的断链；续写资产注入【本拍信息差】【未公开真相】禁令（计划内揭示自动豁免）；`detect_knowledge_leaks` 接入续写探针与 editor_qc 疑点清单；Agency 快照不再丢弃 secrets。**物品归属**：`item_holdings` 按 (story,item) upsert，续写注入【在场物品】，`detect_possession_conflicts` 拦「非持有者使用/遗失物再现」（当场转手豁免）。**级联**：场景 re-ingest 后自动跑确定性影响分析（下游章、无处不在实体过滤）＋ LLM 冲突扫描（提示词资产 `cascade_conflict_scan`），发 `SyncEvent::CascadeImpactDetected`，新增 4 命令与幕后「级联中心」页（去查看/重跑分析/触发改写/忽略）——**只报告不改写后文**。

- **验证**：`cargo test --lib` 1643 passed / 3 ignored（+15）；`npx vitest run` 590 passed / 3 skipped（+5）；tsc / nightly fmt / prettier / architecture_guard 全绿。
- **契约**：`test_edit_early_chapter_creates_downstream_impacts_only_for_shared_entities`（帖主测试③）；`test_persist_knowledge_updates_moves_secret_from_unknown_to_known`；`test_knowledge_boundary_detects_unknown_secret_leak` / `..._hidden_truth_reveal`；`test_possession_conflict_flags_absent_holder_but_allows_transfer` / `..._lost_item_reuse`；`test_continuity_gaps_reads_db_and_respects_planned_text`；`test_ubiquitous_entity_is_filtered_out`；`CascadeCenter` 5 用例。
- **未关闭**：真机三把尺子端到端复跑（P3 三测试套件收口）；**不得宣称续写质量已修复**；网站发布仍待 Apple 公证解阻。

### v0.59.4 - 发布纪律门禁与网站链路修复

起因：v0.59.1–v0.59.3 连续三版漏更 `ARCHITECTURE.md`（文档更新脚本无断言、静默失配），且线上 `latest.json` 仍停在 0.58.0。新增 `docs-guard` 作业（tag 推送时机械校验 8 份必需文档都有改动，缺失即 fail）；补齐 ARCHITECTURE.md 的 v0.59.1–v0.59.3 记录；landing 兜底版本回退 0.58.0（0.59.x 线上 404，兜底不得指向不存在版本）。

- **阻塞点（需人工）**：macOS `tauri-build` 失败于 Apple 公证 `403 A required agreement is missing or has expired`；`upload-to-website` 依赖三平台全成功 → 网站未更新（Windows/Linux 构建成功，但未上传）。
- **验证**：`cargo test --lib` 1628 passed / 3 ignored；`npx vitest run` 585 passed / 3 skipped；landing tsc + 24 tests；build.yml YAML 解析通过；本地按 docs-guard 同款命令预演通过。
- **契约**：`docs-guard`（发布必需文档门禁）。
- **未关闭**：签署 Apple 协议后重跑 macOS 构建 → upload-to-website 才会发布 0.59.x（含 0.58.0 缺失的 `.deb`）；真机续写未复跑，**不得宣称续写质量已修复**。

### v0.59.3 - 手写大纲不再被机器改写

V134 给 `story_outlines` 加 `source`（存量 unknown，保持「机器仍可精炼」语义）。作者手写/弹窗确认（`user_created`）时：创世 `materialize` 的 upsert 带 `WHERE source <> 'user_created'` 不覆盖；资产回流 `sync_story_delta` 直接跳过不追加；`StoryOutlineRepository::update` 仅在带内容时打标（只改 structure_json 不改来源）。另删死模块 `memory/hybrid_search.rs`（410 行）与 capability 死权限 `http:default`。

- **验证**：`cargo test --lib` 1628 passed / 3 ignored（+5）；vitest 585 / 3 skipped；tsc / guard / nightly fmt / playwright 全绿。
- **契约**：`test_materialize_does_not_overwrite_user_created_outline`；`test_materialize_still_updates_machine_outline`；`test_sync_story_delta_skips_user_created_outline`；`update_with_content_marks_user_created`；`update_without_content_keeps_source`。
- **未关闭**：真机续写未复跑（**不得宣称续写质量已修复**）；withGlobalTauri + CSP 需真机运行时验证；FTP 明文；Agency↔agents 环依赖 / coordinator 拆分 / llm_calls 保留 / src-server CI。

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
