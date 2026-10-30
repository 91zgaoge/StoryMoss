# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.74.3
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

**大提示词首字节超时与前缀缓存（v0.74.3，Ds10 是算力上限不是死端点）**：

- **稳定文档必须在前，变动内容必须在后，而且要拆到块级**。只把 `bundle` 放到节拍卡之前不够：`condense_story_outline` 曾把每拍的 `【下一节点】` 注入大纲块、`condense_world_setting` 会按当前地点/在场角色重排规则，前缀在开头几百 token 就断了。稳定区固定为「红线 → 故事大纲（不注入 next_node）→ 世界观（原文顺序）→ Logline → 本章大纲」，之后才是下一节点/角色卡/名册/进度/前文/伏笔/检索/节拍卡。
- **Ds10 不是死端点，不要建议删除**。2×4090 跑 27B 激活 MoE 的冷预填充约 2,200–2,900 tok/s：1 万 token ≈4s、10 万 ≈40s、100 万 ≈6–8 分钟；生产前缀复用率 98.7% 时，复用部分接近零成本。
- **超时按冷预填充预算触发，不按总 token**：`tokens / 2,500` 估算；只有预计首字节或总超时会撞线才放大（40 万 → 190s/310s，100 万 → 430s/550s），上限 600s/900s，只放大不缩小。>1 万只做警示；适配器缓存键必须包含首字节 cap。幕前状态栏显示「大提示词 xx.xk」徽章与冷缓存预估。
- **超大文档先分段**：语料提炼 >12k 字符按 12k 分段摘要再汇总（最多 8 段、均匀抽样保留首尾），不要一次把几十万 token 的全新内容塞给模型。

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
- `cargo test -p storymoss` ✅ 1890 passed / 10 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划 / 判官一致率 / 手法回灌 / 卷纲 / 稳定性回归门·概念体检 / 语料校准 / 真机功能探针（4 项含换模矩阵）/ 后台链效率（同内容 ingest 复用·摘要+快评合并）/ 创世熔断 salvage·单调用回退 / 超时语义拆分 / 空正文冷却·上下文自动探测 / 续写跨档兜底 / 角色身份·改名去重·实体链接 / v0.74 语料仓储·V149 lane/task/lease·导演角色/分区·异步调度·租约·聊天/动作·IPC 契约 / v0.74.1 导演 ToolLoop 拒绝·动作校验·空正文候选剔除·capability 归一化·关系别名 / v0.74.2 大提示词超时豁免·稳定前缀排序·语料分段摘要 / v0.74.3 stable/dynamic 前缀拆分·冷预填充超时校准 ·共 10 ignored）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 646 passed / 3 skipped（v0.74.3 徽章文案改为冷缓存预估（断言更新，数量不变）；v0.74.2 +2：大提示词徽章显示/阈值以下不显示；v0.74.0 +3：语料页列表/详情/AI 提炼与交给导演；AgencyStudio +11：四角色/黑板/时间线/run 选择/错误提示/去重/观察/资产跳转/发送消息/审批确认；v0.73.14 +5：`StyleDnaModal` 单一风格先选后确认 / 关闭不落库 / 弹窗限高 / 混合面板固定按钮与保存 / 混合配置异步回填；v0.73.13 +7：`useExecutionState` 取数与降级 3、`loggedInvoke` 失败日志 4；v0.73.12 +2：文思活跃火焰/被动静态图标契约；+11 幕前切章正文同步 / 幽灵保留 / 懒加载可重开；+2 章纲重规划预览/勾选与忽略；+判官一致率卡；+章法偏好渲染与重置；+2 卷纲页签与合同展示；v0.68.1 分章切换用例 CI 超时加固：`asyncUtilTimeout` 5s + 三处显式等待 10s）
- `npx playwright test` ✅ 45 passed / 5 skipped（v0.74.0 +2：语料页加载/选中/AI 提炼；代理工作室剧组/对话/现场与 `agency_chat_send`；v0.73.9 +1：故事卡片点击同步 `notify_frontstage_story_selected`；v0.73.5 +1：日志页「复制」按钮不被幕前悬浮启动器遮挡（1680px trial click）；v0.73.1 +2：模型页签「超时」子页签直达超时设置、写作页签在 `list_workflows` 返回 null 时不崩溃；v0.72.3 修掉一处既有 flake：`e2e/mock-tauri.ts` 补 `check_update`/`get_current_version` handler——`useUpdater(true)` 挂载 5s 后自动检查，mock 未实现时慢机/并行负载下页面加载类用例会在「无 console 错误」断言上偶发红；门禁仍非阻塞，见未关闭）
- `cargo +nightly-2026-10-05 fmt` ✅（固定版本，避免 nightly 漂移）
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.73.13 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 起瘦身，v0.74.3 归档 v0.73.13；根文件只保留最近 5 个版本与关键教训）。

### v0.74.3 - 稳定/变动前缀拆分 + 冷预填充超时校准

**为什么**：v0.74.2 只把 bundle 放到节拍卡之前，但 `condense_story_outline` 仍把每拍变化的
`【下一节点】` 注入大纲块，`condense_world_setting` 仍按当前地点/在场角色重排规则，
角色卡/名册紧随其后——前缀在开头几百 token 内就断了。用户给出 2×4090 跑 27B MoE 的实测：
冷预填充 2,200–2,900 tok/s，1 万 token ≈ 4s、100 万 ≈ 6–8 分钟，生产前缀复用率 98.7%。
因此「总 token >1 万就放宽到 900s」也是错配：缓存命中时大 prompt 几乎零成本。

**改动**：①`render_continue_assets` 拆 stable pack（红线 → 故事大纲 → 世界观 → Logline →
本章大纲）与 dynamic tail（下一节点、角色卡/名册/关系/冲突/目标、进度、前文、张力、伏笔、
检索块）；新增 `condense_story_outline_stable`（不注入/不按 next_node 筛选）与
`condense_world_setting_stable`（原文顺序，不按本拍重排）；稳定区标记 protected，预算收缩
只切变动区；缓存契约测试断言改变 next_node/地点/在场角色时 `【下一节点】` 之前逐字节相同。
②`large_prompt_limits` 改为按 `tokens / 2,500` 估算冷预填充：只有预计首字节或总超时会撞线
才放大（40 万 → 190s/310s；100 万 → 430s/550s），上限 600s/900s，只放大不缩小；1 万 token
只警示、不放宽。③进度文案与幕前徽章改为「冷缓存预计预填充约 N 秒」，只有真的放宽才说放宽。
④撤回「删除 Ds10」建议：Ds10 是算力上限，不是死端点。

- **验证**：Rust **1890 passed / 10 ignored**（+5）；vitest **646 passed / 3 skipped**；
  playwright 45 / 5；tsc / clippy / fmt / guard 全绿。
- **未关闭**：安装包仍为旧版本；真实 Ds10 首字节与缓存命中率需安装后复测。若 OpenAI 兼容
  响应暴露 `cached_tokens`，下一版用真实命中率替代 2,500 tok/s 估计。

### v0.74.2 - Ds10 首字节超时：稳定前缀 + 大提示词豁免

**为什么**：本机日志里 Ds10 单次调用等 155 秒才返回空正文；同时续写主创提示词把每拍变动的
节拍卡放在最前，模型前缀缓存无法复用（真机曾用 72.5 万 token 的 prompt 复用 72.2 万、仅 6.1 秒）。
超大语料提炼还会一次喂给模型十几万字符的新内容。

**改动**：①`render_writer_user_prompt_with_technique` 改为稳定 bundle 在前、节拍卡/状态/指令/
锚点在后，并加排序契约测试；②`CorpusService::distill` 对 >12k 字符文档按 12k 分段、逐段用
`Summarization` 档提炼要点再汇总，超过 8 段均匀抽样并保留首尾，单段失败回退 2k 字符原文；
③`llm::service` 按 user+system 估算 token，>1 万视为大提示词：生成超时下限 900s、首字节 cap
下限 600s（只放大不缩小），`sent`/心跳文案追加警示，适配器缓存键包含首字节 cap；④幕前底部
状态栏新增「大提示词 12.3k」徽章，前端看门狗检测到后延长至 960s。

- **验证**：Rust **1885 passed / 10 ignored**（+6）；vitest **646 passed / 3 skipped**（+2）；
  playwright 45 / 5；tsc / clippy / fmt / guard 全绿。
- **未关闭**：安装包仍为旧版本；真实 Ds10 首字节需安装后复测。**v0.74.3 更正：Ds10 不是死端点，不应删除**；其慢是 2×4090 冷预填充的算力上限。

### v0.74.1 - 按本机日志修复导演/意图图错误

**为什么**：本机 v0.74.0 运行日志（`com.storymoss.app/logs`）出现三类错误：①导演计划 JSON 被
ToolLoop 当 tool action 解析，连续 8 轮 `missing field type` 熔断；②导演提出 `retry_task` 但
`target_id=null`，审批确认时报 `VALIDATION_FAILED`；③`PlanExecutor` 报
`Unknown capability: agent_writer / agent_inspector`，另有关系 `陈砚 -> 小满` 因只查规范名被跳过。

**改动**：①`TaskScheduler::dispatch_sub_agent`、`run_role_task(_reported)`、`DirectorExecutor`
拒绝 `AgentRole::Director`——导演只走 DirectorPlanner，不再进 ToolLoop；②导演计划新增
`validate_action`：dispatch 校验 role/instruction、retry/cancel 校验 task_id、cancel_run/
resume/materialize/recheck 校验 run_id，不合格动作丢弃并在 reply 说明；提示词同步要求
target_id 来自巡检快照 tasks/runs；③网关执行候选链前剔除「连续空正文、10 分钟冷却」模型，
全部冷却时快速返回可操作错误，避免 Ds10/死端点空转 100s+；④`canonical_capability_id` 归一化
意图图旧 id（agent_writer/agent_rewriter→writer、agent_inspector→inspector、skill_*→builtin.*）；
⑤`asset_bridge`/`materialize` 角色名解析增加 `character_aliases` 回退，简称也能落关系。

- **验证**：Rust **1879 passed / 10 ignored**（+5）；vitest 644 / 3；playwright 45 / 5；
  tsc / clippy / fmt / guard / IPC manifest 全绿。
- **未关闭**：安装包仍为 v0.74.0，运行中的二进制不会热更新；v0.74.1 安装后再复测日志。
  **v0.74.3 更正**：Ds10（Qwen3.5）不是死端点，不要删除；首字慢是 2×4090 冷预填充算力上限。

### v0.74.0 - 语料资产 + 导演 Agent + 幕后剧组对话 + lane/task/lease

**为什么**：作者需要把新闻/故事/概念/灵感/图片带进创作流程；幕后代理工作室只有只读观测，
没有用户 ↔ 管理/编审的对话入口；三个 agent 之上缺少一个统筹、巡检、修复的主 agent。
同时 V109「每故事一个活跃 run」把写入事务锁做得过粗：模型层已多模型并发，真正的串行点
应在故事写入提交，而不是把所有进程拉成一条线。

**改动**：①新增故事级 `corpus_items/corpus_links`、语料页、文本/链接/文件/图片导入、
AI 提炼与转为角色/世界观/伏笔/场景草稿、删除故事清物理文件；②新增 `AgentRole::Director`、
`agency_director_system` 提示词与「导演区」黑板，导演 inspect→plan→异步 dispatch→follow-up，
覆盖手写资产/取消/续写需确认；③代理工作室改三栏（花名册/对话/现场），支持 @管理/@编审/
@主创、审批卡、任务/告警/租约/黑板/时间线，会话与消息落库；④V149 把 V109 索引升级为
`(story_id, lane)`，新增 `agency_agent_tasks`、`story_resource_leases`、`TaskScheduler`、
`TaskReporter`、`LeaseManager`；对话/巡检不建 write run；`AGENCY_GLOBAL_LLM_SEM` 3→4、
新增 `DIRECTOR_LLM_SEM`；⑤所有 agency 角色获得只读 `corpus_search`/`corpus_read`。

- **验证**：Rust **1874 passed / 10 ignored**（+12）；vitest **644 passed / 3 skipped**（+14）；
  playwright **45 passed / 5 skipped**（+2）；tsc / clippy / fmt / guard / IPC manifest 全绿。
- **未关闭**：真实模型端到端（导演规划、语料提炼/转资产）尚未跑真机探针；多 write 并行
  （资源租约 + 原子章号）与 pinned 语料进续写上下文属 P1。

### v0.73.14 - 角色改名不再生成重复角色 + 风格弹窗可确认

**为什么**：真机反馈两件事：①编辑角色（尤其改名）后列表多出同名角色卡，用户担心续写人物混乱；
②混合风格找不到确认/关闭按钮，单一风格看不出是否选中。根因：编辑后的 ingest 丢掉已链接的实体
ID、逐条 `create_entity` 重新生成 UUID；自动识别角色在 `characters` 与 `kg_entities` 中 id 不同，
`repo.update` 只按 id 更新、`get_by_story` 只按 id 判重，旧名 legacy 行因此再次出现。

**改动**：①`update_character` 的 ingest 改 `save_entities_batch`（按链接 id upsert）；
②`CharacterRepository::update` id 未命中时按旧名回退更新 legacy 行；③`get_by_story` legacy 兜底
按名字排除已有 kg 实体；④改名登记旧名别称、同步场景出场/冲突 token 与物品持有名；
⑤`link_entities` 支持别称解析，旧称呼回收原实体；⑥V148 迁移合并存量同名重复角色；⑦实体属性
合并时空值不覆盖作者已填内容；⑧风格弹窗抽成 `StyleDnaModal`：限高 + 内容区滚动 + 底部固定操作区，
单一风格先选中再点确定，新增右上角关闭 / Esc / 保存失败 toast，混合面板按钮 sticky。

- **验证**：Rust **1844 passed / 10 ignored**（+6）；vitest **639 passed / 3 skipped**（+5）；
  tsc / clippy / fmt / guard / IPC manifest 全绿。
- **未关闭**：真机双窗口未复测；关系表对 ingest 建角色的 id 映射（legacy vs kg）是既有问题，本版未动。

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
