# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.73.7
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

**发布镜像并发写（v0.73.0 用户更新报 `The signature verification failed`）**：

- **只有稳定 tag 的 `upload-to-website` 可以写官网**。nightly 与 stable 版本号相同、产物同名，2026-10-15 两个上传 job 并发写同一 FTP 路径，把 macOS 更新包写坏，稳定/nightly 签名都验不过。nightly 只留 GitHub Release。
- macOS 更新包在镜像侧必须用版本化文件名（`StoryMoss_<version>_aarch64.app.tar.gz`），任何发布路径都不得让多个 run 共享可变文件名；镜像上传 job 用 `concurrency: storymoss-release-upload` 串行。
- 上传前 `node .github/scripts/upload-releases-ftp.mjs <dir> --check-only`、上传后 `--verify-mirror`（Node 内置 Ed25519+BLAKE2b 做真实 minisign 校验）；只检查 HTTP 200/文件大小无法发现此类损坏。
- 镜像救火用 `Sync Release to Website` workflow（`workflow_dispatch`，拒绝 prerelease 与版本降级）。

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

仓库内置 `.githooks/pre-commit`：提交前自动检查本次 staged 的 Rust（`cargo +nightly-2026-10-05 fmt -- --check`）与前端（`prettier --check`）代码是否已格式化，未格式化则拒绝提交，对齐 CI 的 fmt 检查。

- **首次克隆后启用**：`git config core.hooksPath .githooks`
- **工具链**：`rustup toolchain install nightly-2026-10-05 --profile minimal --component rustfmt`。nightly rustfmt 会随版本漂移改变中文注释换行（2026-10-25 版起与仓库现状不一致，Windows CI 直接红），CI 与钩子固定同一版本，见 `docs/archive/LESSONS_LEARNED.md` 经验 11。
- **行为**：仅检查本次 `git add` 进来的 `.rs` / `.ts` / `.tsx` / `.css` / `.json` 代码文件，纯文档/配置提交不受影响；失败时打印 diff 并给出修复命令。
- **修复**：按提示执行 `(cd src-tauri && cargo +nightly-2026-10-05 fmt)` 或 `(cd src-frontend && npm run format)`，再 `git add -u && git commit`。
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
- `cargo test -p storymoss` ✅ 1833 passed / 10 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划 / 判官一致率 / 手法回灌 / 卷纲 / 稳定性回归门·概念体检 / 语料校准 / 真机功能探针（4 项含换模矩阵）/ 后台链效率（同内容 ingest 复用·摘要+快评合并）/ 创世熔断 salvage·单调用回退 / 超时语义拆分 / 空正文冷却·上下文自动探测 ·共 10 ignored）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 625 passed / 3 skipped（+11 幕前切章正文同步 / 幽灵保留 / 懒加载可重开；+2 章纲重规划预览/勾选与忽略；+判官一致率卡；+章法偏好渲染与重置；+2 卷纲页签与合同展示；v0.68.1 分章切换用例 CI 超时加固：`asyncUtilTimeout` 5s + 三处显式等待 10s）
- `npx playwright test` ✅ 42 passed / 5 skipped（v0.73.5 +1：日志页「复制」按钮不被幕前悬浮启动器遮挡（1680px trial click）；v0.73.1 +2：模型页签「超时」子页签直达超时设置、写作页签在 `list_workflows` 返回 null 时不崩溃；v0.72.3 修掉一处既有 flake：`e2e/mock-tauri.ts` 补 `check_update`/`get_current_version` handler——`useUpdater(true)` 挂载 5s 后自动检查，mock 未实现时慢机/并行负载下页面加载类用例会在「无 console 错误」断言上偶发红；门禁仍非阻塞，见未关闭）
- `cargo +nightly-2026-10-05 fmt` ✅（固定版本，避免 nightly 漂移）
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.54.0 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 瘦身：根文件只保留最近 5 个版本与关键教训）。

### v0.73.7 - 空正文冷却 + 默认模型可删 + 上下文自动探测 + 日志本地时间

**为什么**：真机创世 600s 超时的日志显示 `deepseek-v4` 返回「推理 token 4590 / 正文 0 字符」，
网关换候选后会成功，但用户显式指定的创作模型每轮仍被强制置顶，导致每次调用都先空转一次；
同时用户「早已不用」的 Qwen3.5 仍在 `llm_profiles` 且 `is_default=true`，删除被
`Cannot delete the default profile` 拒绝（日志 `delete_model failed`），`tool_model_id` 继续
指向已下线端点；日志页直接显示 UTC，与本机时间差 8 小时；模型上下文长度前端从未暴露、后端一律 8192。

**改动**：①网关「空正文冷却」——`HealthRegistry` 新增 `empty_content_failures/last_empty_content_at`，
连续 2 次空正文后 10 分钟冷却（探测成功不清零、真实非空生成才清零）；冷却期内
`resolve_role_model` 跳过显式角色置顶、`select_candidates` 额外 -1000 并跳过 +1000 置顶；
②`remove_llm_profile` 允许删除默认模型并提升剩余启用模型为默认，同时清空 active/角色引用；
`AppConfig::load` 自愈失效的角色引用；③新增 `detect_model_context` 命令（OpenAI 兼容 `/models`
的 `max_model_len/context_length` → Ollama `/api/show` → 内置型号目录），模型弹窗新增
「上下文长度 + 自动检测」，留空保存自动探测；旧配置的 8192 已知型号加载时按目录校正；
④日志页与诊断卡片统一转本地时间。

- **验证**：`cargo test --lib` **1833 passed / 10 ignored**（+6）；clippy 0 error；tsc / vitest 625 /
  playwright 42 passed / 5 skipped；fmt（`nightly-2026-10-05`）/ guard 全绿。
- **未关闭**：真机创世未复跑；内置型号目录为保守参考值，未知型号仍需手动填上下文。

### v0.73.6 - 超时语义拆分：TCP 连接 / 首字节 / 正文生成

**为什么**：用户真机日志显示 deepseek-v4-flash 的非流式调用反复在 **15000ms** 报
`LLM_CONNECTION_TIMEOUT`（重试仍失败，`smart_execute` 255–267s 后整体失败），而设置页写着
连接超时 60s。根因是适配器把「连接已建立后等待响应头」也压在了 `llm_connect_timeout_secs` 上：
推理模型先思考再返回响应头，15s 一到就被判连接失败。

**改动**：①`adapter::send_with_response_timeout`——TCP 建连仍由
`ClientBuilder::connect_timeout` 负责，响应头等待改用「首字节超时」（默认 180s）；
②新增 `AppError::LlmResponseTimeout`（code `LLM_RESPONSE_TIMEOUT`，severity Retry，
仅额外重试 1 次），文案明确指向「首字节超时」设置；③后端默认连接超时 15→60（与前端一致），
`LlmService` 兜底 30/60→60/180；④OpenAI/Anthropic/Ollama 三适配器的非流式与流式请求全部接入；
⑤回归测试 `response_headers_timeout_uses_dedicated_marker`（本地 TCP 只建连不回包）。

- **验证**：`cargo test --lib` **1828 passed / 10 ignored**（+1）；clippy 0 error；fmt / guard 全绿；前端零改动。
- **未关闭**：真机创世未复跑（deepseek-flash 工具档与创作档的下一次实跑仍需观察首字节耗时）。

### v0.73.5 - 幕前启动器移至右下角（不再遮挡页头按钮）

**为什么**：用户截图反馈日志页右上角「复制」按钮被悬浮的「开幕前」启动器盖住、无法点击
（1680px 窗口）。`FrontstageLauncher` 原为 `fixed top-4 right-4 z-50`：既压住各页页头右侧按钮
（日志页「复制」、设置页「导出/导入」），又与右上角 toast 区域重叠。

**改动**：定位改为 `fixed bottom-6 right-6 z-40`；右上让给页头操作与 toast，右下为常规浮动位，
后台页无固定于右下的控件。新增 Playwright 回归：1680px 视口进入日志页，对「复制」执行
`click({ trial: true })`——旧位置下会因悬浮层拦截指针而失败（已实测复现），新位置通过。

- **验证**：playwright **42 passed / 5 skipped**（+1）；tsc / vitest 625 passed 全绿；Rust 零改动。
- **未关闭**：无。

### v0.73.4 - E2E 任务固定 Ubuntu 24.04（CI 环境漂移修复，无应用代码变更）

**为什么**：v0.73.2/v0.73.3 的 `e2e-check` 在未改前端代码的提交上红：
`ERROR: Playwright does not support chromium on ubuntu26.04-x64`。GitHub 当天把 `ubuntu-latest`
指向 Ubuntu 26.04，当前 Playwright 尚不支持该平台；本机与上一轮 24.04 构建全绿。

**改动**：`.github/workflows/build.yml` 的 `e2e-check` 显式 `runs-on: ubuntu-24.04`（其余任务不变）；
经验入档 `docs/archive/LESSONS_LEARNED.md` 经验 12（`*-latest` 镜像也是依赖）。

- **验证**：本地 playwright 41 passed / 5 skipped；Rust 1827 passed / 10 ignored 基线不变。
- **未关闭**：待 Playwright 支持 26.04 后再评估是否回到 `ubuntu-latest`。

### v0.73.3 - 创世管理 Agent 熔断 salvage + 单调用回退

**为什么**：用户真机创世报「创作引擎遇到致命错误：管理 Agent 被熔断（达到最大轮数），资产生产
未完成」。检查发现 legacy 创世对 producer 熔断直接 Err，熔断前写入黑板的资产全部作废；而 producer
要求 5 类资产、tool_loop 仅 12 轮，快速路径回退后更容易耗尽。

**改动**：①`run_genesis_legacy_inner` 熔断后不再直接失败：先落库黑板已有资产、
`emit_progress(assets, degraded)`，继续首章，`spawn_producer_resume` 后台补齐世界/大纲/关系；
②黑板无可用资产时走 `producer_direct_fallback`（单调用直接输出 JSON，复用
`agency_producer_system`，产出世界观/2 角色/关系/大纲/伏笔后写黑板，不新增提示词资产）；
③Producer `max_turns` 12→16，任务提示「写完必需资产立即 final」；④熔断日志记录每轮 action
（tool 名/final/parse_fail），让「详见 run 日志」有据可查。

- **验证**：`cargo test --lib` **1827 passed / 10 ignored**（+2 回归：`test_genesis_salvages_producer_assets_on_max_turns` /
  `test_genesis_recovers_with_producer_direct_fallback`；原 MaxTurns 致命错误断言改为 salvage 成功断言）；
  clippy 0 error；fmt（`nightly-2026-10-05`）/ guard 全绿；前端零改动（仅版本号）。
- **未关闭**：真机创世未复跑（需用 deepseek-flash 创作档 + Qwen3.5 工具档再验一次首章产出）。

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
