# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.59.2
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
- `cargo test -p storymoss` ✅ 1626 passed / 3 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 585 passed / 3 skipped（删 27 项孤儿 hook 测试；+2 续写质检闭环、+3 空文档判定）
- `npx playwright test` ✅ 39 passed / 5 skipped（新增幕前续写 spec 3 用例；门禁仍非阻塞，见未关闭）
- `cargo +nightly fmt` ✅
- `cargo clippy --lib` ✅ 本版未重跑
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.54.0 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 瘦身：根文件只保留最近 5 个版本与关键教训）。

### v0.59.2 - 修静默清空、清死代码、归档旧文档

载入期空文档保护：ProseMirror 空文档是 `<p></p>`（真值），旧 `if (!content)` 守卫挡不住 → 正文未到时编辑器自带空文档被 2s 防抖保存落库、覆盖整章（e2e 稳定复现）。现 `markSceneContentLoaded` 布防、`isEmptyEditorHtml` 判定、首次非空保存自动解除。另修 JSON 尾随逗号换行形态（模型几乎总把闭合括号另起一行）。E2E 去掉 `continue-on-error` 提升为阻塞门；删 5 个零引用编辑器扩展 + 4 个孤儿 hook（前端测试 −24）；根目录 33 份陈旧 .md 归档到 `docs/archive/root-legacy/`；landing 字体 CDN 上锁 `@3.0.0` + SRI。

- **验证**：`cargo test --lib` 1626 passed / 3 ignored（+2）；`npx vitest run` 585 passed / 3 skipped（净 −24：删 27 孤儿测试 + 新增 3 项空文档判定）；Playwright 39 passed / 5 skipped（连续两轮）；landing 24 passed + build 通过。
- **契约**：`isEmptyEditorHtml` 空文档判定；`test_extract_fenced_json_trailing_comma_newline`；`test_strip_whitespace_trailing_commas_keeps_string_literals`；`frontstage-editing` 自动保存持久化用例（3 轮稳定）。
- **未关闭**：真机续写未复跑（**不得宣称续写质量已修复**）；src-server 无 DB 不可编译、CI 未覆盖；withGlobalTauri + 宽松 CSP、FTP 明文、`story_outlines` 机器覆盖手写大纲、Agency↔agents 环依赖与 coordinator 巨石拆分待办。

### v0.59.1 - 构建修复：对齐新版 nightly rustfmt

v0.59.0 的 CI 卡在「Check Rust formatting」（tauri-build 被跳过，安装包未产出）：浮动 nightly 由 2026-07-17 升到 2026-10-05 后中文注释折行规则变化。已整仓按新规则格式化（106 文件，纯折行无逻辑改动）。

- **验证**：`cargo +nightly fmt -- --check` 0 diff；`cargo test --lib` 1624 passed / 3 ignored；`npx vitest run` 609 passed / 3 skipped（均不变）。
- **复发处置**：CI 若在格式步失败 → `rustup update nightly && (cd src-tauri && cargo +nightly fmt)` 后提交。

### v0.59.0 - 验收证据链、数据层治理与续写质检闭环

对照 `docs/audits/2026-10-06-project-review-v0.58.0.md`。三批实施：CI 恢复阻塞（弃用「49 个 V092 基线失败」过时注释）、迁移记 checksum + 集合水位线（低于水位的补丁迁移可执行）、V133 六索引与删故事级联、后台 ingest/QC 纳入 run 预算与取消传播、续写质检可行动（事件带 mode/chapter + 幕前「按审查意见修订本章」走 `auto_revise(revision_type=editor_qc)`）、补齐 4 个占位 prompt 资产、server JWT/DEV_UPGRADE 缺省加固、导出 ZIP 默认剔除 API key、发布白名单补 .deb、AGENTS.md 1208→190 行、FrontstageApp 抽 `useScenePersistence`。

- **验证**：`cargo test --lib` 1624 passed / 3 ignored（+41）；`npx vitest run` 609 passed / 3 skipped（+2）；`tsc` / `architecture_guard` / `cargo +nightly fmt` / prettier 全绿；Playwright 39 passed / 5 skipped。
- **契约**：`test_apply_pending_backfills_lower_versioned_migration`；`test_record_migration_stores_content_checksum`；`test_pick_migrations_dir_ignores_build_output_copy_even_when_newer`；`export_strips_api_keys_by_default`；`test_prune_activity_log_keeps_recent_rows`；`revise_task_description_injects_editor_qc_issues`；续写质检不合格不提示「重新创世」且出现可修订操作条；`rejects_known_insecure_defaults`（server，独立提取运行）。
- **未关闭**：真机续写仍未复跑，**不得宣称续写质量已修复**；新发现载入期空保存竞态（E2E 门禁暂留非阻塞）；`src-server` 无 DB 环境不可编译、CI 未覆盖；golden harness 需真机基线。

### v0.58.0 - 戏剧工艺 + 短剧格式（AI-drama-pound）

对照 `docs/plans/2026-08-29-drama-craft-fusion-design.md`。工艺来源 [AI-drama-pound](https://github.com/POUND0423/AI-drama-pound)（MIT），不 vendoring 对方 skill，不把主创拉回 ToolLoop。节拍卡增加本拍必须改变项；续写短合同禁止原地踏步；编辑审计可读 impact/fix；探针只在复述近文且未兑现改变项时 gap。V131 `story_format` 默认 novel；显式短剧词才切竖屏剧本组装。幕后新建可选长篇/短剧。幕前仍写 `scenes.content`。

- **验证**：`cargo test --lib` 1583 passed / 2 ignored（+11）；`npx vitest run` 607 passed / 3 skipped（+1）；`tsc` / `architecture_guard.py` 全绿。
- **契约**：`change_delta_from_hostile_cast`；`continue_system_has_stall_example`；`editor_issue_parses_impact_and_fix`；`probe_gaps_when_increment_is_tail_recap`；`probe_does_not_gap_literary_aside_when_not_recap`；`looks_like_short_drama_defaults_novel`；`continue_beat_complete_does_not_require_tools`；制作限制只在短剧显示。
- **未关闭**：真机须再跑创世/续写/短剧；**不得宣称续写质量已修复**。不分镜、不自动删角色脏行。

### v0.56.2 - 下引号不再单独成段；编辑审计顶栏不再报「已完成失败」

幕前对话句号后空行 + 全角缩进会把闭合引号排成带段首缩进的孤段。根因：悬挂合并只认「换行后立刻是引号」，夹着全角空格就漏；空行分段路径还不跑 HTML 孤段合并。现跳过换行与引号之间的空白并丢掉缩进，空行/存量 HTML 都并回上一句。后台编辑审查本是 fail-open（章节已落库），却把 `Err` 标成「后台审查失败」，顶栏拼成「编辑审计已完成后台审查失败」。现 done 固定「后台审查」，失败只走 toast/日志；`friendlyText` 对失败/超时不再加「已完成」。

- **验证**：`cargo test --lib` 1572 passed / 2 ignored（+1）；`npx vitest run` 606 passed / 3 skipped（+4）。
- **契约**：`blank-line path: indented hanging closing quote`；`失败/超时不得拼成「已完成…失败」`；`editor_qc_done_detail_is_fail_open_not_failure`。
- **未关闭**：已落库旧章下次打开会并回孤引号。真机须再续写确认；**不得宣称续写质量已修复**。




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
