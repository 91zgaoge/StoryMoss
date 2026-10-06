# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.64.1
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
- `cargo test -p storymoss` ✅ 1691 passed / 3 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 594 passed / 3 skipped（+5 级联中心页；+4 运行维护页）
- `npx playwright test` ✅ 39 passed / 5 skipped（新增幕前续写 spec 3 用例；门禁仍非阻塞，见未关闭）
- `cargo +nightly fmt` ✅
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.54.0 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 瘦身：根文件只保留最近 5 个版本与关键教训）。

### v0.64.1 - 发布链路恢复：0.64.0 上线 + 兜底版本 + 用例加固

**发布确认**：Apple 协议签署生效 → v0.64.0 完整发布（latest.json=0.64.0；dmg/msi/AppImage/**deb** 全 200）。**landing** `FALLBACK_VERSION` 0.58.0→0.64.0（最近一次确认在线版本；保留策略 5 版内有效）。**加固**：`FrontstageApp.split-auto-switch` 分章切换用例三处等待显式 5s（默认 1000ms 在 CI 高负载下超时，曾致 v0.64.0 首轮构建失败；纯测试时序）。**流程修正**：v0.60.0–v0.63.0 此前只推到 Cursor 代理，GitHub master 停在 v0.59.4、CI 未运行——已补推，后续每版双 remote 推送。

- **验证**：Rust 1691 passed / 3 ignored（无改动）；vitest 594 / 3 skipped；landing 24；clippy 0 error；fmt/prettier/guard 全绿。
- **未关闭**：真机端到端未复跑（**不得宣称续写质量已修复**）。

### v0.64.0 - 运行维护页 + 声明式投影路由表（P3-F 落地与 CI 修复）

**P3-F 声明式投影路由表**（`story_system::projection_writers`）：`CommitArtifact` × `ProjectionWriterKind` 纯数据表 `PROJECTION_ROUTES`；`get_projection_writers` 与 `projection_status_keys()` 均从表派生（漏接线告警、新增 writer 自动进状态键），每次 commit 记录路由摘要便于审计；契约测试 6 项（产物全覆盖/顺序与 name 一致/无孤儿/状态键含异步/空值判定/摘要 on-off）。**幕后「运行维护」页**：质量债、待确认、文风偏好、成本四 Tab（此前四处的后端命令均无界面）；新增 `list_style_preferences` / `set_style_preference_status` 命令与 4 项 vitest。**修复**：`llm::cost` 的 `clippy::redundant_comparisons`（deny 级）阻塞了 v0.63.0 的 CI 发布链路——修复并把 `cargo clippy` 纳入每版验证。

- **验证**：`cargo test --lib` 1691 passed / 3 ignored（+6）；`npx vitest run` 594 passed / 3 skipped（+4）；`cargo clippy` 0 error；tsc / nightly fmt / prettier / guard 全绿。
- **契约**：`routing_table_covers_every_artifact`；`sync_kinds_are_constructible_and_match_writer_names`；`status_keys_include_deferred_writers`；`artifact_has_content_treats_empty_payload_as_absent`；`route_summary_reports_on_off_per_artifact`；`Maintenance` 4 用例。
- **未关闭**：真机端到端未复跑（**不得宣称续写质量已修复**）；v0.63.0 tag 保留（CI 已失败，不回改）；网站发布待本版 tag 跑通。

### v0.63.0 - 工程纪律：质量债 / 时间旅行 / 终局指南针 / 待确认队列（P0–P3 收官）

P3 阶段，也是 P0–P3 四阶段路线图的收官版本。**V138** 三张表：`quality_debts`（质检降级不再静默：RevisionRequired / salvage / 异常降级入账，含建议回收窗口，幂等 upsert）、`story_checkpoints`（每 10 章随段摘要写连续性快照：角色状态/物品归属/未回收伏笔/段摘要/全书纲要）、`pending_reviews`（规则类新增物待确认队列）。**时间旅行**：`story_system::checkpoint::query_as_of` 基于 append-only 知情流水与 `reveal_chapter` 给出「截至第 N 章」的角色已知与真相揭示状态（物品账本非追加式，结果标注 current_only）。**终局指南针**：`story_system::compass` 确定性派生（核心冲突行 / 活跃伏笔 / 进度）并注入续写，零额外 LLM。**待确认**：ingest 中 importance ≥7 的世界规则进队列。**三把尺子组合契约**：`tests::three_rulers_contract_test` 在同一故事内组合验证玉佩/知识边界/级联三条主线。5 个新命令：`list_quality_debts` / `resolve_quality_debt` / `query_story_as_of` / `list_pending_reviews` / `resolve_pending_review`。

- **验证**：`cargo test --lib` 1685 passed / 3 ignored（+14）；前端无改动（vitest 590 / 3 skipped）；nightly fmt / guard 全绿。
- **契约**：`record_is_idempotent_and_lists_open_debts`；`as_of_filters_knowledge_and_reveals_by_chapter`；`checkpoint_upsert_and_lookup_by_chapter`；`compass_derives_direction_threads_and_scale`；`note_is_idempotent_and_resolve_removes_from_pending`；`three_rulers_hold_on_one_story`。
- **未关闭**：P2-B/P2-D/P3-A/P3-B 的后端与命令已就绪但**无 UI**；P3-F 声明式投影路由表未实施；真机三把尺子端到端复跑未做（**不得宣称续写质量已修复**）；网站发布待 Apple 公证解阻。

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
