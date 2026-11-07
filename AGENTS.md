# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.89.9
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
- `cargo test -p storymoss` ✅ 2033 passed / 15 ignored（v0.89.9：task_system/narrative/style_dna 三族 IPC 参数契约修复（rename_all=snake_case），测试数不变；v0.89.0 +3：sha256 确定性·上传历史读写去重·manifest 序列化；v0.88.0 +5：HttpTransport SSE 解析·is_http·旧配置兼容·V156 建表幂等·V157 建表索引；v0.87.0 +4：is_chat 缺省/别名解析·人设覆盖运行时生效·四角色人设注册；v0.86.0 +8：target_end_chapter 校验·快照完整 ID·换算边界·2 章批量端到端·越界拒绝·短 ID 解析与失败隔离·reqwest 重试分类·peek 一致性；v0.85.1 +3：GitHub 主源顺序、网络/安全重试边界、备用源同版本；迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划 / 判官一致率 / 手法回灌 / 卷纲 / 稳定性回归门·概念体检 / 语料校准 / 真机功能探针（4 项含换模矩阵）/ 后台链效率（同内容 ingest 复用·摘要+快评合并）/ 创世熔断 salvage·单调用回退 / 超时语义拆分 / 空正文冷却·上下文自动探测 / 续写跨档兜底 / 角色身份·改名去重·实体链接 / v0.74 语料仓储·V149 lane/task/lease·导演角色/分区·异步调度·租约·聊天/动作·IPC 契约 / v0.74.1 导演 ToolLoop 拒绝·动作校验·空正文候选剔除·capability 归一化·关系别名 / v0.74.2 大提示词超时豁免·稳定前缀排序·语料分段摘要 / v0.74.3 stable/dynamic 前缀拆分·冷预填充超时校准 / v0.75.0 情感回路·V150·OCC·压力场·读者面板 / v0.76.0 批量续写章节情感弧·gate/revision 同计划·persist_chapter_plan / v0.77.0 多 write 并行·V151 章号序列·pinned 语料·真机导演/语料探针 / v0.77.1 资产引导租约·图片语料元数据/摘要 / v0.78.0 视觉多模态·图片提炼/转资产·真机视觉探针 / v0.79.0 叙事契约 P0·场简报·契约门禁·情感最小链·active story 切换·姓名校验 / v0.80.0 章契约·1–3 场投影·legacy 方法论/叙事策略接线·回滚开关·多故事切换 E2E / v0.80.1 SceneOutcome 落库·姓名账本/称呼块 / v0.80.2 复合姓·候选池·作者称呼规则 / v0.80.3 批量 LLM 章契约·确定性兜底·环境开关 / v0.80.4 多代理熔断 salvage·调度器状态语义·网关 tool_calls·ingest 宽容解析 / v0.80.5 导演 LeadWriter 派发接续写管线·原子章号·GateRunner 装配·导演指令注入 / v0.80.6 导演 target_chapter 修订既有章·原 scene 更新·修订仍过 gate / v0.80.7 章节下拉就地改名·中文数字标题展示 / v0.80.8 幕后窗口关闭白屏修复（prevent_close + from_config 重建） / v0.81.0 幕后 UI 重设计 P0 基建（Modal/ConfirmDialog/Table 等 8 组件·Button 收敛·z 契约） / v0.81.1 导航重组 创作5·资产7·运维5·分析5·系统1·双壳金条指示 / v0.81.2 核心六页模板化·Scenes 800px 溢出修复·视口验收探针 / v0.81.3 数据诊断页模板化·Logs/Tracing 高度魔法数清除·探针 21 页 42 用例 / v0.81.4 confirmDialog 全局入口·20 处 confirm 迁移·UserMenu 暗色化·Settings 文案删除·探针 23 页 46 用例 / v0.81.5 弹窗限高内滚补齐·Card 圆角契约 / v0.81.6 设置控件作用域统一·页面级错误边界 / v0.82.0 伏笔排序/去重/配额8/预算保护/硬指令（+3 测试） / v0.82.1 WriteTimeBundle 限额 3/1→8/2（续写主路径） / v0.82.2 导演 target_scene 指定场景修订·场景 override·gate 只更新目标 scene / v0.82.3 导演快照场景序号·审批卡目标显示 / v0.82.4 伏笔文本计划「收：第N章」进窗口提示 / v0.82.5 中文数字·多条计划优先逾期 / v0.82.6 V154 伏笔计划文本回填结构化窗口 / v0.82.7 补交遗漏新增文件（仓库完整性修复） / v0.82.8 真机探针 导演场景目标·伏笔实际使用（本机实跑） / v0.82.9 Dashboard/Stories 弹窗迁统一 Modal / v0.83.0 体裁·角色·世界观手写弹窗全迁 Modal / v0.83.1 弹窗 800×600 视口回归探针 / v0.83.2 伏笔清单 blob 紧凑计划摘要（埋第X→收第Y） / v0.83.3 升级·角色编辑·模型编辑弹窗壳迁 Modal / v0.83.4 模型编辑 13 控件迁 Input/Select/Textarea（设置逐字段样板） / v0.83.5 AgentConfig·RouteSimulator·PromptsPanel 控件迁共享组件 / v0.83.6 GeneralSettings 9 控件迁 Input/Select/Textarea / v0.83.7 UnifiedModelManager 控件迁移·设置控件组件化完成 / v0.83.8 移除 Google Fonts 阻塞·loading 挂 React 挂载 / v0.83.9 Login 弹窗迁 Modal·暗色化 / v0.84.0 Cinzel 字体自托管·字体零外部依赖 / v0.84.1 StyleDnaModal·ExportDialog 迁共享 Modal·卷号计划·V155 碎片窗口继承 / v0.84.2 V155 计划线程匹配扩展（收后 3 字专名 + 人称过滤）·碎片窗口 9→19 ·共 15 ignored）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 696 passed / 3 skipped（v0.88.0 契约更新：Sidebar IA 23→24（续写训练）；v0.87.0 +10：@提及解析（全角/紧跟文字/叙述不误判/四角色映射）；v0.85.0 +2：创作路径容器布局、Sidebar 窄态分组展开；Panel/Scenes 契约随行为更新；v0.84.1 +5：Modal testid 契约 1、ExportDialog Modal 壳/步骤 4；v0.81.4 +2：confirmDialog 回退/Portal 确认取消；v0.81.1 +1：Sidebar IA 契约按新五组重写；v0.81.0 +19：Modal 4、ConfirmDialog 3、Field/controls 3、Table 4、Skeleton/EmptyState 2、Button 别名与 token 回归 3；v0.80.7 +4：中文数字标题展示 1、下拉就地改名 2、大标题铅笔入口 1；v0.79.0 +3：story-selected seq/origin 契约 2、生成中切换集成 1；v0.75.0 +1：代理工作室情感页签契约/张力/债务/读者面板；v0.74.3 徽章文案改为冷缓存预估（断言更新，数量不变）；v0.74.2 +2：大提示词徽章显示/阈值以下不显示；v0.74.0 +3：语料页列表/详情/AI 提炼与交给导演；AgencyStudio +11：四角色/黑板/时间线/run 选择/错误提示/去重/观察/资产跳转/发送消息/审批确认；v0.73.14 +5：`StyleDnaModal` 单一风格先选后确认 / 关闭不落库 / 弹窗限高 / 混合面板固定按钮与保存 / 混合配置异步回填；v0.73.13 +7：`useExecutionState` 取数与降级 3、`loggedInvoke` 失败日志 4；v0.73.12 +2：文思活跃火焰/被动静态图标契约；+11 幕前切章正文同步 / 幽灵保留 / 懒加载可重开；+2 章纲重规划预览/勾选与忽略；+判官一致率卡；+章法偏好渲染与重置；+2 卷纲页签与合同展示；v0.68.1 分章切换用例 CI 超时加固：`asyncUtilTimeout` 5s + 三处显式等待 10s）
- `npx playwright test` ✅ 127 passed / 5 skipped（v0.85.2 +1：九场景五档几何/滚动/缩放验收；v0.85.0 +29：三档 23 视图扩 23、真实内容布局回归 6；日志遮挡用例改为 Sidebar 普通布局入口；v0.83.1 +3：Modal 800×600 boundingBox+内滚探针；v0.81.4 +14：探针 23 视图 46 用例；v0.81.3 +20：视口探针扩到全导航 21 页；v0.81.2 +12：核心六页 800×600/1400×900 视口验收探针（无横向溢出断言+截图）；v0.80.7 +1：章节下拉就地改名 E2E；v0.80.0 +1：多故事切换 E2E；v0.74.0 +2：语料页加载/选中/AI 提炼；代理工作室剧组/对话/现场与 `agency_chat_send`；v0.73.9 +1：故事卡片点击同步 `notify_frontstage_story_selected`；v0.73.5 +1：日志页「复制」按钮不被幕前悬浮启动器遮挡（1680px trial click）；v0.73.1 +2：模型页签「超时」子页签直达超时设置、写作页签在 `list_workflows` 返回 null 时不崩溃；v0.72.3 修掉一处既有 flake：`e2e/mock-tauri.ts` 补 `check_update`/`get_current_version` handler——`useUpdater(true)` 挂载 5s 后自动检查，mock 未实现时慢机/并行负载下页面加载类用例会在「无 console 错误」断言上偶发红；门禁仍非阻塞，见未关闭）
- `cargo +nightly-2026-10-05 fmt` ✅（固定版本，避免 nightly 漂移）
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.73.13 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 起瘦身，v0.77.0 归档 v0.74.2；根文件只保留最近 5 个版本与关键教训）。

### v0.89.9 - 三族 IPC 参数契约修复（真机日志实证）

**为什么**：真机日志 16:53 连续报 `get_task_logs missing required key taskId`——前端统一 snake_case 传参，后端 task_system 默认 camelCase 绑定。同族 `create_task` 必填 `task_type`/`schedule_type` 在真机同样永远缺失（创建任务必失败），`list_tasks` 状态过滤静默失效，`set_story_style_dna` 与叙事四命令同病；E2E 因 mock 层拦截 invoke 未能暴露。

**改动**：`task_system/commands.rs` 9 个命令、`creation_commands.rs::set_story_style_dna`、`commands/narrative.rs` 4 个命令统一 `#[tauri::command(rename_all = "snake_case")]`；`FALLBACK_VERSION` 补回 v0.87–v0.89 遗漏的同步（0.89.9）。

**验证**：Rust **2033 / 15**；Clippy/check/fmt/guard 全绿；tsc 0；Vitest **696 / 3**；Playwright 基线通过。

**未关闭**：真机复验任务页日志/创建任务/风格 DNA/叙事分析；qwen-1 中转返回 DashScope 原生 400，建议更换端点。

### v0.89.0 - 日志压缩上传官网（显式授权）+ ZCode 每日分析修复

**为什么**：用户要求日志压缩上传网站（显式授权、防重复、服务端只留 3 天），并在 ZCode 设每天定时任务下载日志包分析修复错误、持续优化应用。

**改动**：①`log_report.rs`：近 3 天日志（tracing/工作流/traces≤50/panic/startup）+ manifest 打包（zip Deflated、20MB 预算、SHA-256 命名、本机留 5 个）；IPC `package_log_report/upload_log_report/get_log_upload_history`。②防重复双保险：本机历史判重 + 服务端 sha256 UNIQUE（409 幂等）。③日志页两步授权弹窗（打包不上传 → 展示体积/SHA → 确认才传；无静默上传故不设总开关）。④src-server：`005_log_packages.sql`、`/api/logs/upload`（服务端重算 sha256、21MB 上限）、`/api/logs/packages[+download]`（LOG_OPS_TOKEN）、每小时 retention_sweep（>72h 删文件与行）、docker volume + deploy 注入 secret。⑤ZCode 每天 10:00 自动化：下载→聚类→修复（≤3 个/轮，全门禁）→推送，无问题只写报告。

**验证**：Rust **2033 / 15**（+3）；tsc/Vitest 696/3/Playwright 127/5 全绿；src-server 纯函数单测 + CI 构建。

**未关闭**：deploy-server 部署 + LOG_OPS_TOKEN secret 需用户操作；端到端上传下载清理部署后实测；ZCode 任务首轮保守度观察。

### v0.88.0 - MCP 流式 HTTP 客户端（栗子读书）+ 续写训练定时闭环

**为什么**：用户需要通过 MCP 调取本机栗子读书（legado `http://127.0.0.1:1122/mcp`，Bearer 头）的小说任意章节或本地 epub 全本，并开启定时任务：随机取若干章 → 续写 → 与真实后续章对比 → 不断修正续写能力。MCP 模块此前只有 stdio、连接不持久化、定时任务无法在后端直连。

**改动**：①`McpServerConfig` 增 `url`/`headers`；新增 `HttpTransport`（Streamable HTTP：JSON/SSE 双格式、Mcp-Session-Id 会话、通知 202、DELETE 会话），`McpClient` 按 url 分流并补发 `notifications/initialized`；`tools/list` 按规范解 `{tools:[...]}` 包裹；V156 `mcp_server_configs` 持久化 + 启动自动连接 HTTP 型；MCP 页 HTTP/stdio 模式与已保存列表。②`TaskType::ContinuationTraining` + 执行器：mcp（显式工具名+`$book`/`$index` 模板）/file（复用 parse_book）取章 → 独立沙盒故事（V157 报告表）→ 逐章灌 scene → 真实章节风格 DNA → 上轮 improvement 注入指令 → `run_continue_batch_with_cancel` 续写 → 真实后续章作基准五维对比 → 报告落库 → 同书保留 3 个沙盒级联清理；任务取消 5s 传播。③幕后「分析」新增「续写训练」页：配置/cron/立即跑/报告趋势与详情。

**验证**：Rust **2030 / 15**（+5）；Clippy/check/fmt/guard 全绿；tsc 0；Vitest **696 / 3**（Sidebar IA 23→24）；Playwright **127 / 5**；未本地打包。

**未关闭**：栗子读书在线时真机接入实测；五维分数判官一致性多轮观察；stdio 型启动自动拉起未做。

### v0.87.0 - 统一幕后/幕前对话（@agent 智能路由）+ 工作室 agent.md 人设编辑

**为什么**：幕前没有真正的聊天（底部输入只走 `smart_execute` 生成管线），幕后 AgencyStudio 独享剧组对话；四个 agent 的人设提示词只能进设置页按扁平 prompt id 编辑，工作室没有入口，用户要求「幕前默认 @导演」的统一对话与显式编辑个性化 agent.md。

**改动**：①`utils/agentMention.ts` 统一解析 `@导演/@管理/@编审/@主创`（含全角 ＠、紧跟文字），幕后 `handleSend` 与幕前入口共用；②聊天区抽取共享 `ChatPanel`（消息/角色胶囊/会话选择/审批卡/输入栏），幕后/幕前同组件同缓存，会话天然同步；③`WritingIntentClassification.is_chat`（serde default，提示词补对话示例），幕前 `handleSmartGeneration` 分类后拦截 `is_chat && confidence≥0.6` 转入 `FrontstageChatOverlay`（默认 @导演，预填不自动发送——双保险）；`@提及` 快路直接发送；无故事回落生成管线；④工作室角色卡新增「人设」入口 → `PersonaEditModal`（四角色 agent.md 编辑/已定制徽章/保存覆盖/恢复默认），覆盖经 `prompt_overrides` 即时生效（resolve_role_prompt_with_pool + resolve_director_prompt 单点注入）；⑤三份人设专业化精修升 0.87.0（主创+场景引擎/修订模式/交付自查；管理+交付标准/边界与升级；编审+复核模式/裁决纪律），operational 内容逐字保留。

**验证**：Rust **2025 / 15**（+4：is_chat 解析、人设覆盖运行时生效/注册）；Clippy/check/fmt/guard 全绿；tsc 0；Vitest **696 / 3**（+10）；Playwright **127 / 5** 基线不变；未本地打包。

**未关闭**：幕前智能路由误判率需真机观察；幕前浮层无黑板/时间线深浏览页签。

### v0.86.0 - 导演主工作流三项修复（完整 ID / 网络重试 / 续写到目标章）

**为什么**：真机（超梦神探 48 章）导演无法执行「续写，在 60 章结束第一阶段」：①巡检快照把 task/run ID 截成前 8 位（`render_director_user_prompt` 的 `take(8)`），系统提示词又要求 target_id 来自快照，导演照抄 8 位 ID 去 `retry_task` 必然「任务不存在: 8c0629c1」炸掉整回合；②deepseek 瞬时网络故障（reqwest 「error sending request」）不在 `is_retriable_error` 的子串分类里，0 重试直接判死；③导演派发硬编码 `count=1`，一次只写 1 章，「写到 60 章」没有系统机制。

**改动**：①快照输出完整 UUID；执行器 `resolve_task_target`/`resolve_run_target` 按「全等→唯一前缀」解析短 ID，歧义/未命中给可操作错误；`cancel_run` 未命中不再静默。②execute 每个 arm 包 async 块收集失败，单动作失败只落 `director_actions` failed、追加系统消息，全部失败才判回合死。③`is_retriable_error` 认可「error sending request」，走既有退避重试。④新增 `DirectorActionSpec.target_end_chapter`：`peek_next_chapter`（与原子分配同口径）换算批量章数，走既有批量管线逐章 gate+装配，到章自动停；单次上限 5 章，更长区间导演用 depends_on_action 串行（5+5+2）；越界/超限任务失败并提示，不消耗 LLM。⑤提示词升 0.86.0，审批卡显示「续写至第N章」。

**验证**：Rust **2021 / 15**（+8）；Clippy/check/fmt/guard 全绿；tsc 0 错误；Vitest **686 / 3**；Playwright **127 / 5** 基线不变；未本地打包。

**未关闭**：真机升级后复测「60 章结束第一阶段」全流程；导演串行分批 depends_on 等待需真机观察；第 48 章 1067 字异常短章建议 `target_chapter=48` 修订。

### v0.85.2 - 场景故事线与章节执行面板错位修复

**为什么**：中屏 Grid 未定义行高，九场景实测故事线/正文只剩约 55px 标题条，正文被裁切；旧探针只测执行区可达，没测面板高度和返回顶部。

**改动**：中屏 `lg:grid-rows-[100%_auto]`，第一行完整承载故事线 + 正文，执行区第二行跨两列且 `h-auto`；宽屏 `xl:grid-rows-[minmax(0,1fr)]`，三栏 `h-full/min-h-0`；小屏自然单列。不加 absolute/z-index/强制滚动复位。

**验证**：新增 1000/1024/1279/1280/1400 五档几何探针（九场景长正文、面板矩形、正文归属、滚动返回、缩放）；Playwright **127 / 5**（+1）；Vitest **686 / 3**；tsc/prettier/guard 全绿；Rust 业务未改。

**未关闭**：使用 CI 安装包复测原生窗口缩放。

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
