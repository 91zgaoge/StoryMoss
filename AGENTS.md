# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.95.0
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
- `cargo test -p storymoss` ✅ 2130 passed / 17 ignored（v0.95.0 +23：lifecycle 转移/取消 RAII 5、MCP Unknown 权限与重试边界、style-learning/recompute RAII、updater 多源、log_report 重复包、订阅过期门控、PostProcess 解析失败、拆书终态重试、projection 前缀；v0.94.0 +6：训练资产重建落库全链路、资产 prompt 契约、键值散文 salvage、未接地人名过滤、逐章梗概映射、判官承接基准；v0.93.16 +5：基准章不泄漏×2（style/承接锚点）/空稿与拒绝话术校验/空稿覆盖守卫；v0.93.15 +10：MCP 只读重试×4（分类/权限/重置重试/写不重发）；判官评分×6（五维齐全/拒绝 null/拒绝缺维/字符串与 7/10 宽容/overall 均值与 0-10 归一/章号标签）；v0.93.14 +1：进度/日志/心跳同源时间；v0.93.13 数量不变（CI 预热）；v0.93.12 +2：StyleDNA 小数/数字字符串平均句长宽容解析；v0.93.11 数量不变（CI 重试）；v0.93.10 数量不变（CI 发布门禁）；v0.93.9 数量不变（纯前端侧栏 IA）；v0.93.7 +2：StyleDNA 缺字段/缺块回退默认；v0.93.6 测试数不变（服务端非宏 SQL）；v0.93.5 +3：风格锚点取尾/短章边界/硬承接常量措辞；v0.93.4 +4：LLM_REQUEST_FAILED 重试分类、错误行计数、自动上传条件判定×2；v0.93.3 +16：导出规范化 content 12（tiptap 分段/实体解码/围栏清理/边界回退）+ pdf 4（CJK 宽度折行/内嵌字体端到端）；v0.93.2 +3：取消监听器置位·共享执行锁互斥语义·UpdateTaskRequest payload 更新；v0.93.0 +3：自动连接状态/训练自动选工具与参数、MCP utils；v0.92.0 +6：instinct 块 promoted 过滤/预算、skill_execute 白名单、写作策略覆盖、叙事节奏块、追读债空返回、导演洞察 prompt；v0.91.1 +12：MCP 映射契约/字段 alias/permission/loopback raw HTTP 测试；v0.91.0 +1：导演 prompt 质量债区块；v0.90.0 +1：run trace 注册表 roundtrip/guard drop；v0.89.9：task_system/narrative/style_dna 三族 IPC 参数契约修复（rename_all=snake_case），测试数不变；v0.89.0 +3：sha256 确定性·上传历史读写去重·manifest 序列化；v0.88.0 +5：HttpTransport SSE 解析·is_http·旧配置兼容·V156 建表幂等·V157 建表索引；v0.87.0 +4：is_chat 缺省/别名解析·人设覆盖运行时生效·四角色人设注册；v0.86.0 +8：target_end_chapter 校验·快照完整 ID·换算边界·2 章批量端到端·越界拒绝·短 ID 解析与失败隔离·reqwest 重试分类·peek 一致性；v0.85.1 +3：GitHub 主源顺序、网络/安全重试边界、备用源同版本；迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划 / 判官一致率 / 手法回灌 / 卷纲 / 稳定性回归门·概念体检 / 语料校准 / 真机功能探针（4 项含换模矩阵）/ 后台链效率（同内容 ingest 复用·摘要+快评合并）/ 创世熔断 salvage·单调用回退 / 超时语义拆分 / 空正文冷却·上下文自动探测 / 续写跨档兜底 / 角色身份·改名去重·实体链接 / v0.74 语料仓储·V149 lane/task/lease·导演角色/分区·异步调度·租约·聊天/动作·IPC 契约 / v0.74.1 导演 ToolLoop 拒绝·动作校验·空正文候选剔除·capability 归一化·关系别名 / v0.74.2 大提示词超时豁免·稳定前缀排序·语料分段摘要 / v0.74.3 stable/dynamic 前缀拆分·冷预填充超时校准 / v0.75.0 情感回路·V150·OCC·压力场·读者面板 / v0.76.0 批量续写章节情感弧·gate/revision 同计划·persist_chapter_plan / v0.77.0 多 write 并行·V151 章号序列·pinned 语料·真机导演/语料探针 / v0.77.1 资产引导租约·图片语料元数据/摘要 / v0.78.0 视觉多模态·图片提炼/转资产·真机视觉探针 / v0.79.0 叙事契约 P0·场简报·契约门禁·情感最小链·active story 切换·姓名校验 / v0.80.0 章契约·1–3 场投影·legacy 方法论/叙事策略接线·回滚开关·多故事切换 E2E / v0.80.1 SceneOutcome 落库·姓名账本/称呼块 / v0.80.2 复合姓·候选池·作者称呼规则 / v0.80.3 批量 LLM 章契约·确定性兜底·环境开关 / v0.80.4 多代理熔断 salvage·调度器状态语义·网关 tool_calls·ingest 宽容解析 / v0.80.5 导演 LeadWriter 派发接续写管线·原子章号·GateRunner 装配·导演指令注入 / v0.80.6 导演 target_chapter 修订既有章·原 scene 更新·修订仍过 gate / v0.80.7 章节下拉就地改名·中文数字标题展示 / v0.80.8 幕后窗口关闭白屏修复（prevent_close + from_config 重建） / v0.81.0 幕后 UI 重设计 P0 基建（Modal/ConfirmDialog/Table 等 8 组件·Button 收敛·z 契约） / v0.81.1 导航重组 创作5·资产7·运维5·分析5·系统1·双壳金条指示 / v0.81.2 核心六页模板化·Scenes 800px 溢出修复·视口验收探针 / v0.81.3 数据诊断页模板化·Logs/Tracing 高度魔法数清除·探针 21 页 42 用例 / v0.81.4 confirmDialog 全局入口·20 处 confirm 迁移·UserMenu 暗色化·Settings 文案删除·探针 23 页 46 用例 / v0.81.5 弹窗限高内滚补齐·Card 圆角契约 / v0.81.6 设置控件作用域统一·页面级错误边界 / v0.82.0 伏笔排序/去重/配额8/预算保护/硬指令（+3 测试） / v0.82.1 WriteTimeBundle 限额 3/1→8/2（续写主路径） / v0.82.2 导演 target_scene 指定场景修订·场景 override·gate 只更新目标 scene / v0.82.3 导演快照场景序号·审批卡目标显示 / v0.82.4 伏笔文本计划「收：第N章」进窗口提示 / v0.82.5 中文数字·多条计划优先逾期 / v0.82.6 V154 伏笔计划文本回填结构化窗口 / v0.82.7 补交遗漏新增文件（仓库完整性修复） / v0.82.8 真机探针 导演场景目标·伏笔实际使用（本机实跑） / v0.82.9 Dashboard/Stories 弹窗迁统一 Modal / v0.83.0 体裁·角色·世界观手写弹窗全迁 Modal / v0.83.1 弹窗 800×600 视口回归探针 / v0.83.2 伏笔清单 blob 紧凑计划摘要（埋第X→收第Y） / v0.83.3 升级·角色编辑·模型编辑弹窗壳迁 Modal / v0.83.4 模型编辑 13 控件迁 Input/Select/Textarea（设置逐字段样板） / v0.83.5 AgentConfig·RouteSimulator·PromptsPanel 控件迁共享组件 / v0.83.6 GeneralSettings 9 控件迁 Input/Select/Textarea / v0.83.7 UnifiedModelManager 控件迁移·设置控件组件化完成 / v0.83.8 移除 Google Fonts 阻塞·loading 挂 React 挂载 / v0.83.9 Login 弹窗迁 Modal·暗色化 / v0.84.0 Cinzel 字体自托管·字体零外部依赖 / v0.84.1 StyleDnaModal·ExportDialog 迁共享 Modal·卷号计划·V155 碎片窗口继承 / v0.84.2 V155 计划线程匹配扩展（收后 3 字专名 + 人称过滤）·碎片窗口 9→19 ·共 15 ignored）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 719 passed / 3 skipped（v0.95.0 +13：拆书 liveStatus 隔离/完成 invalidate、Prompts 草稿不互覆、日志上传票据、Export 审查错误区分、Tasks cancelled 分组与行锁、Training upsert 互斥、向导边界；v0.94.0 数量不变（后端训练链路改动）；v0.93.16 数量不变（Rust-only 修复）；v0.93.15 数量不变（本版无前端改动）；v0.93.14 +3：本地时间格式化（SQLite/UTC/非法串）；v0.93.13 数量不变（CI 改动）；v0.93.12 +2：任务通知「查看任务」写入待聚焦 ID 并切换全部任务页、toast 点击后关闭并聚焦；v0.93.11 数量不变（CI 改动）；v0.93.10 数量不变（CI 改动）；v0.93.9 +2：任务组四项契约、四项跳转独立视图；v0.93.7 数量不变；v0.93.5 数量不变（本轮前端零改动）；v0.93.4 数量不变（agency/上传改动未动契约）；v0.93.3 数量不变（导出/切书由 E2E 覆盖）；v0.93.2 数量不变（任务融合未动契约）；v0.93.0 +3：MCP server/tool 前缀解析、章节工具自动选择、参数模板自动生成；v0.91.1 MCP 前端类型/连接态对齐（数量不变）；v0.90.0 Dashboard 契约随 AgencyRunsPanel 更新；v0.88.0 契约更新：Sidebar IA 23→24（续写训练）；v0.87.0 +10：@提及解析（全角/紧跟文字/叙述不误判/四角色映射）；v0.85.0 +2：创作路径容器布局、Sidebar 窄态分组展开；Panel/Scenes 契约随行为更新；v0.84.1 +5：Modal testid 契约 1、ExportDialog Modal 壳/步骤 4；v0.81.4 +2：confirmDialog 回退/Portal 确认取消；v0.81.1 +1：Sidebar IA 契约按新五组重写；v0.81.0 +19：Modal 4、ConfirmDialog 3、Field/controls 3、Table 4、Skeleton/EmptyState 2、Button 别名与 token 回归 3；v0.80.7 +4：中文数字标题展示 1、下拉就地改名 2、大标题铅笔入口 1；v0.79.0 +3：story-selected seq/origin 契约 2、生成中切换集成 1；v0.75.0 +1：代理工作室情感页签契约/张力/债务/读者面板；v0.74.3 徽章文案改为冷缓存预估（断言更新，数量不变）；v0.74.2 +2：大提示词徽章显示/阈值以下不显示；v0.74.0 +3：语料页列表/详情/AI 提炼与交给导演；AgencyStudio +11：四角色/黑板/时间线/run 选择/错误提示/去重/观察/资产跳转/发送消息/审批确认；v0.73.14 +5：`StyleDnaModal` 单一风格先选后确认 / 关闭不落库 / 弹窗限高 / 混合面板固定按钮与保存 / 混合配置异步回填；v0.73.13 +7：`useExecutionState` 取数与降级 3、`loggedInvoke` 失败日志 4；v0.73.12 +2：文思活跃火焰/被动静态图标契约；+11 幕前切章正文同步 / 幽灵保留 / 懒加载可重开；+2 章纲重规划预览/勾选与忽略；+判官一致率卡；+章法偏好渲染与重置；+2 卷纲页签与合同展示；v0.68.1 分章切换用例 CI 超时加固：`asyncUtilTimeout` 5s + 三处显式等待 10s）
- `npx playwright test` ✅ 144 passed / 5 skipped（v0.95.0 数量不变（状态机改造后全量 E2E 复跑通过）；v0.94.0 数量不变（后端训练链路改动，视口/功能基线）；v0.93.16 数量不变（Rust-only 修复，视口/功能基线）；v0.93.15 数量不变（本版无前端改动）；v0.93.14 数量不变（诊断页时间格式改动，视口/功能基线复跑通过）；v0.93.13 数量不变（CI 改动）；v0.93.12 数量不变（训练状态横幅/任务聚焦由 Vitest 契约覆盖，视口验收 training-lab 复跑通过）；v0.93.11 数量不变（CI 改动）；v0.93.10 数量不变（CI 改动）；v0.93.9 +6：guidebook-distillation 与 training-lab 加入三档视口验收；agency wiring 改用「全部任务」入口；v0.93.7 基线不变；v0.93.5 本轮前端零改动；v0.93.4 基线不变；v0.93.3 +3：真机懒加载路径切书、update_scene 挂起 5s 超时继续、快速连点停最后点击；v0.93.2 基线不变；v0.93.0 +1：MCP 页自动展示已保存服务器与全部工具；v0.91.1 MCP 页无新增 E2E，基线不变；v0.91.0 +4：级联交给导演、Tasks 代理任务重试、SceneEditor 派发、执行面板 AI 续写；v0.90.0 +3：仪表盘代理运行+创世、生成链路 agency 步骤、四角色用量；v0.85.2 +1：九场景五档几何/滚动/缩放验收；v0.85.0 +29：三档 23 视图扩 23、真实内容布局回归 6；日志遮挡用例改为 Sidebar 普通布局入口；v0.83.1 +3：Modal 800×600 boundingBox+内滚探针；v0.81.4 +14：探针 23 视图 46 用例；v0.81.3 +20：视口探针扩到全导航 21 页；v0.81.2 +12：核心六页 800×600/1400×900 视口验收探针（无横向溢出断言+截图）；v0.80.7 +1：章节下拉就地改名 E2E；v0.80.0 +1：多故事切换 E2E；v0.74.0 +2：语料页加载/选中/AI 提炼；代理工作室剧组/对话/现场与 `agency_chat_send`；v0.73.9 +1：故事卡片点击同步 `notify_frontstage_story_selected`；v0.73.5 +1：日志页「复制」按钮不被幕前悬浮启动器遮挡（1680px trial click）；v0.73.1 +2：模型页签「超时」子页签直达超时设置、写作页签在 `list_workflows` 返回 null 时不崩溃；v0.72.3 修掉一处既有 flake：`e2e/mock-tauri.ts` 补 `check_update`/`get_current_version` handler——`useUpdater(true)` 挂载 5s 后自动检查，mock 未实现时慢机/并行负载下页面加载类用例会在「无 console 错误」断言上偶发红；门禁仍非阻塞，见未关闭）
- `cargo +nightly-2026-10-05 fmt` ✅（固定版本，避免 nightly 漂移）
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.73.13 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 起瘦身，v0.77.0 归档 v0.74.2；根文件只保留最近 5 个版本与关键教训）。

### v0.95.0 - 隐式状态机治理（51 个条目收敛到 3 个核心边界）

**为什么**：全仓审计发现同一状态在 DB 列、内存、事件、日志/前端 store 各存一份；终态可被迟到的执行结果覆盖，取消只写 DB 不中断执行，中文/英文阶段词汇各判一套。用户要求按「更少更深模块、更小接口」治理，并永久记住三条原则（状态机治理、入口校验一次、分析透彻建议大胆）。

**改动**：①新增 `src-tauri/src/lifecycle/mod.rs`：统一转移表（Task/AgencyRun/AgentTask/ChatMessage/DirectorAction）、终态集合、RAII `CancelHub/CancelScope`；任务进度/完成/失败只允许从 running 写入，取消后迟到的 executor 结果不再复活终态；手动 trigger 显式复位 pending；调度器复用同一把执行锁。②agency：`finish_run` 返回写入行数，完成事件只在真正写入后发送；导演 cancel_run 同时落 DB 并让批量逐章读 DB 终态；聊天占位成功回填 done；审批动作复用同一行。③前端：`state/generation.ts` 成为唯一解释器（后端 phase 改 snake_case + label）；活动聚合读当前 store；章节选择 epoch、幽灵 Esc/Tab、保存态、切书失败恢复。④流程：拆书/Prompts/日志上传/导出/Tasks/Training/向导/技能执行状态收敛；删除 8 个死前端状态。⑤其他：MCP Unknown 权限、updater 多源、重复日志包、projection 前缀、style-learning/recompute RAII、PostProcess 解析失败、订阅过期门控、拆书终态重试。

**验证**：Rust **2130 / 17**；tsc 0；Vitest **719 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。审计文档 `docs/plans/2026-11-10-implicit-state-machine-audit.md`。**未关闭**：真机复测取消/审批/训练/日志上传；租约续期与 DB CHECK、P2/P3 窗口/自动化/叙事阶段/视图单源。

### v0.94.0 - 续写训练资产重建 + 编审/对比闭环（按现有续写流程重构）

**为什么**：v0.93.16 已修掉基准章泄漏与空稿守卫，但训练沙盒仍只有灌入章节，没有世界观/故事大纲/角色/关系/逐章梗概；writer 会在「创作资产为空」时拒绝，或走散文回退另起新故事。用户要求训练按现有续写流程：前文 → 资产管理（重建世界观/大纲/角色/故事线）→ 续写 → 编审 → 与原后续章对比 → 沉淀经验并优化下一轮。

**改动**：①新增 `agency/training_assets.rs`：训练执行器在续写前用上下文章（不含末位基准章）重建世界观（concept/history/rules）、故事大纲（核心冲突/转折点/下一节点）、角色卡（含情感四字段）、角色关系、逐章梗概（写 `scenes.outline_content`）；复用 `CharacterRepository` / `WorldBuildingRepository` / `StoryOutlineRepository` / `SceneRepository` / `materialize::upsert_seed_relationship`，未接地人名一律跳过；JSON 解析失败时复用 `asset_refresh::parse_refresh_payload` salvage 世界观/大纲/角色。②资产 prompt 只使用前文实际出现的人名/地名；长章采样保留开场 + 结尾窗口，末章结尾用于 `next_node`。③续写仍走既有 `run_continue_batch_with_cancel`（writer + gate/revision/assembly），资产就绪后 `ensure_assets` 不再走空资产拒绝路径。④判官增加「上一章结尾衔接基准」，专门检查 AI 是否紧接现场，`improvement` 必须含承接修正；报告固定记录「资产重建 → 编审 → 对比」三段流程与本轮资产计数。⑤训练进度新增 assets 阶段；报告/日志可见资产重建计数与告警。

**验证**：Rust **2107 passed / 17 ignored**（+6）；tsc 0；Vitest **706 / 3**、Playwright **144 / 5** 基线不变；fmt/clippy/guard 全绿。**未关闭**：升级后真机跑第 97 章训练，确认资产重建计数、writer 不再拒绝、gate/revision 正常、报告含承接基准与真实五维分；旧沙盒被空稿覆盖的正文仍需从黑板/源章节恢复。

### v0.93.16 - 续写训练连贯性根修 + 空稿覆盖守卫（本机真机沙盒复核）

**为什么**：本机 2026-11-09 训练报告与沙盒复核发现：①沙盒 1109-1821（《阴脉先生》第 97–106 章）生成的「铜钱烙掌破门而出」正文是「陈默/铜钱/阴脉」现代都市故事，与第 106 章末尾「桐人镇魇」现场脱节；②沙盒 1109-1434 的 writer 直接输出「我无法按当前输入完成这一章。原因是：提示中「创作资产」区域为空…」409 字元评论，gate 判死整轮；③`build_style_anchor` 取 `chapters` 末两章（末位是真实后续基准章）导致基准章末段泄漏进续写指令；④上轮训练建议跨章节区间注入（第 1–10 章的建议污染第 97 章训练）；⑤两个沙盒的已生成/上下文正文被幕前切书的 `<p></p>` 空文档覆盖成 7 字符空稿（`update_scene` 旧守卫只挡字面空串）。

**改动**：①新增 `context_chapters`/`build_continuity_anchor`：文风锚点与承接锚点只用上下文章（末位基准章绝不进入指令）；训练指令始终注入上一章结尾原文 900 字并硬性要求同地点/同批人物/同时代承接、未完结事件先落地、禁止新主角/新委托/新场景；`inject_director_directive` 上限 1200→4096。②`load_prior_insights` 只取与当前承接区间重叠的报告（`start_chapter <= 当前起点 < start_chapter + context_chapters`）。③batch/revision writer prompt 追加「资产缺失兜底」硬规则；`writer_prose_fallback` 在有前文时改用新增 `assemble_continue_prose_fallback`（续写 system/task + 【前文】），不再复用创世开篇 prompt。④`validate_generated_prose` 判分前拒绝 `<p></p>`/空白、<300 字、拒绝话术（「无法按当前输入完成/创作资产区域为空/作为 AI」），任务失败、沙盒保留、报告不落；判官输入统一 `content_to_plain_text`。⑤`SceneRepository::update_in_tx` 把视觉空 HTML 视为 None，不再覆盖已有正文（字面空串守卫保留）。

**验证**：Rust **2101 passed / 17 ignored**（+5）；tsc 0；Vitest **706 / 3**、Playwright **144 / 5** 基线不变；fmt/clippy/guard 全绿；`cargo check` 零错误。**未关闭**：安装 v0.93.16 后真机重跑第 97 章训练，确认生成正文承接第 106 章现场且判官给出真实五维分；沙盒 1109-1821 第 11 章与 1109-1434 第 62 章的原始正文已被空稿覆盖，需从黑板草稿或源章节恢复后再对比（本轮未直接改用户库）。

### v0.93.15 - 续写训练网络韧性 + 判官评分闸门（本机真机日志根修）

**为什么**：本机 2026-11-09 17:58 续写训练任务 `a2090aa1`（《阴脉先生》第 97 章起 10 章）在取到第 100 章后报 `调用 get_chapter_content 失败：传输错误: 读取响应失败: Connection reset by peer (os error 54)`，整条训练直接失败；重试后于 18:28 完成，但报告 `1a36e1d9` 是 `{"overall":0.0,"scores":null}`、五维全「-」的假 0.00 报告（判官 718 字输出缺 scores），且报告把「沙盒第 11 章」与「真实第 107 章」混在同一行；闭环提示词还输出「上一1轮训练建议」。

**改动**：①`mcp/client.rs::McpClient::call_tool` 对 Read/Network 权限工具在瞬时 `TransportError`（ECONNRESET/os error 54、拒绝、broken pipe、读取/写入失败）或 `Timeout` 时重试两次（150/400ms 退避），Write/Private 绝不重发；②`task_system/continuation_training.rs` 新增 `parse_judge_report`：五维必须齐全且为 1-10 数值（宽容数字字符串与 `"7/10"`），`overall` 缺失按五维均值折算、0-10 归一；判官输出不可用时严格重试一次（提示词只给纯 JSON），仍不可用则降级落 `{"scores":null,"overall":null,"judge_error":...}` 报告（overall=null，前端趋势跳过；正文与沙盒保留，对齐质检失败降级保产出）；③报告与判官提示词标清「沙盒第 N 章（对应真实第 M 章）」，任务摘要同口径；④`load_prior_insights` 修「上一1轮/上一2轮」为「更早一轮/最近一轮训练建议」。

**验证**：Rust **2096 / 17**（+10）；fmt/clippy/guard 全绿；前端未改（Vitest **706/3**、Playwright **144/5** 基线不变）；本机 Legado live probe（11 工具、list_groups）通过；本地 96–111 章直连探针全部 200。**未关闭**：安装 v0.93.15 后真机重跑第 97 章训练，确认取章遇到抖动不再整轮失败、报告含真实五维分（判官可用时）。

### v0.93.14 - 任务进度/日志时间统一（反复被报的时间不一致根修）

**为什么**：用户再次报「任务进度时间与日志时间不一致」，并明确要求永久记住。查证：`TaskExecutionContext::update_progress` 只写 tasks.progress/updated_at（`update_status`），`task_logs` 由 `create_log` 另写自己的 `Local::now()`，`last_heartbeat_at` 又要等执行器下一次主动 heartbeat——三者时间可相差分钟级；前端日志只显示 `HH:MM:SS`，跨天/跨轮次聚合时与「上次运行」的完整日期错位；诊断页还有多处对原始 UTC ISO 串做 `slice`（AgencyLearning/AgencyEval/Maintenance），与本地任务时间差 8 小时。这是与 v0.73.7「日志本地时间」同类的循环复现口径错误。

**改动**：①`TaskRepository::record_progress` 新增：一次进度更新把 `tasks.progress/updated_at/last_heartbeat_at` 与 `task_logs.created_at` 写成同一个 `now`（毫秒级一致），进度日志同条落库；`update_progress` 与 `TaskProgressEvent.timestamp` 共用该 now。②`TaskStatusChangedEvent` 增 `timestamp`；`TaskExecutionContext::log` 同步刷新心跳（日志也是存活信号）。③前端新增 `utils/datetime.ts`（`formatLocalDateTime/formatLocalTime`，按 Date 解析本地时区、SQLite `YYYY-MM-DD HH:MM:SS` 归一），Tasks 页的最近更新/上次运行/心跳/每条日志全部改为完整本地日期时间；TrainingLab 状态横幅显示更新与最新日志时间；AgencyLearning/AgencyEval/Maintenance 的 raw `slice` 全部改本地格式化。④永久记忆 `storymoss-task-log-time-consistency.md`：任务 DB 用 Local、应用日志/traces 用 UTC、UI 禁止 slice 原始时间串。

**验证**：Rust **2086 / 17**（+1：`record_progress` 三时间一致）；tsc 0；Vitest **706 / 3**（+3：SQLite/UTC/非法时间串格式化）；Playwright **144 / 5**（复跑）；fmt/clippy/guard 全绿。**未关闭**：真机升级后在 Tasks 页对照「最近更新/心跳/日志」确认同一毫秒时间链。

### v0.93.13 - CI 预热 AppImage 打包工具（修 GitHub 504 连累构建）

**为什么**：v0.93.10 的 Linux tauri-build 红在 AppImage 打包——`tauri-bundler` 运行时从 GitHub Releases 拉 `AppRun-x86_64` 得到 504，bundler 自身无重试，整条 tag 构建与官网同步失败。

**改动**：tauri-build 在 Ubuntu 上新增「Prefetch AppImage bundler tools」步骤：curl 多轮重试预热 `~/.cache/tauri/` 的 AppRun、linuxdeploy、gtk/gstreamer 插件脚本（chmod 770）；命中缓存后 bundler 不再联网，预热失败只告警交由 bundler 兜底。

**验证**：workflow YAML 解析通过；v0.93.13 tag 实测 Linux AppImage 打包路径。**未关闭**：v0.93.8 错误 Release 清理待确认。

### v0.93.12 - 续写训练失败根修（修订熔断兜底 + DNA 小数解析）+ 任务状态可见性

**为什么**：真机续写训练任务 `0f3ca151`（《阴脉先生》第 53 章起 10 章上下文）2026-11-09 14:37:59 失败，错误为「主创 Agent 被熔断（达到最大轮数），修订轮未完成」。日志链：06:35 首轮主创熔断 → prose 兜底取稿；06:37 编审熔断 → 散文回退；06:37:59 第二道主创（修订轮）熔断 → `handle_gate` 直接把整条续写管线判死（首轮草稿其实可用）。同期风格分析失败 `invalid type: floating point 22.6, expected u32`——LLM 把平均句长写成小数，整份 StyleDNA 解析报废、风格材料静默失效。训练页只有报告列表，任务失败/执行中时用户停留在本页没有任何提示；任务完成/失败 toast 只在 Tasks 页挂载时才有，也没有跳转具体任务的入口。

**改动**：①`agency/coordinator.rs::handle_gate` 修订轮调用失败/熔断不再判死整轮——保留首轮草稿、采用首道 gate 裁决继续装配；黑板有更完整新稿则优先采用；修订后取稿失败也回退保留原稿（对齐 v0.30.30「质检失败降级保产出」）。②`domain/style.rs::SyntaxProfile.avg_sentence_length` 宽容反序列化：小数四舍五入、数字字符串可解析、负数/非数字回退 0（不再因一个数字形态整份 DNA 报废）。③训练 MCP 未连接错误带可操作提示（确认栗子读书已启动并在 MCP 页连接）。④训练页新增常驻「训练任务状态」横幅：执行中进度/最新日志、失败错误全文 +「重试」+「查看任务」；App 顶层监听 `task-created` / `task-status-changed`，创建/完成/失败/取消均弹带「查看任务」的 toast；`pendingTaskFocusId` 令 Tasks 页展开、高亮并滚动到该任务行（拆书/指导书/训练创建同样走该入口）。

**验证**：Rust **2085 / 17**（+2：小数/数字字符串平均句长）；tsc 0；Vitest **703 / 3**（+2：任务通知跳转契约）；Playwright **144 / 5**（视口验收 training-lab 复跑 75 用例通过）；fmt/clippy/guard 全绿。**未关闭**：安装升级后真机复测该训练任务（同书 once 行）应完成并写出报告；LLM 连续空响应/网关超时仍会重试与兜底，不属本版根修范围。

### v0.93.11 - CI 加固（protoc 下载 504 重试）

**为什么**：v0.93.10 tag 构建的 macOS rust-check 红在 `arduino/setup-protoc`：GitHub Release 下载 protoc 连续 504，动作内部重试耗尽即失败，与仓库代码无关；重跑即恢复，但一次网络抖动不应让整条发布链变红。

**改动**：`rust-check` 与 `tauri-build` 两处 protoc 安装改为外层三步重试：首次 `continue-on-error: true`，`outcome == 'failure'` 时依次重试两次，最后 `Verify protoc` 确保二进制可用；三次都失败才让 job 红。

**验证**：v0.93.10 重跑 macOS rust-check 的 `Install protoc` 通过；随后以 v0.93.11 新 tag 实测。**未关闭**：v0.93.8 错误 Release 清理待确认。

### v0.93.10 - 发布门禁修复（docs-guard 失败不再打包/上传）

**为什么**：v0.93.6 与 v0.93.8 的 tag 构建都红在 `docs-guard`（发布未更新 8 份必需文档），但 `tauri-build`、`upload-to-website` 不依赖它，失败后照样打包、创建 GitHub Release、写官网；v0.93.8 因 tag 打在未 bump 版本的清理提交上，把 0.93.7 产物当 v0.93.8 上传。

**改动**：①`docs-guard` 增「tag 与 Cargo.toml / tauri.conf.json / package.json / landing FALLBACK_VERSION 四处版本一致」校验（tag 去掉 `v` 与 `-nightly` 后比较），不一致直接失败。②`tauri-build` 的 `needs` 加入 `docs-guard`，并用 `always() && rust-check/frontend-check success && (docs-guard success || skipped)`——非 tag 放行，tag 门禁失败即不打包。③`upload-to-website` / `verify-updater-manifest` 的 `needs` 加入 `docs-guard`，`if` 要求 `docs-guard.result == 'success'`，彻底堵住「门禁红、发布照走」。

**验证**：本地 dry-run v0.93.10 版本校验通过；workflow YAML 解析通过；随后以 v0.93.10 实测新 tag 路径。**未关闭**：v0.93.6/v0.93.8 历史红 run 不重写 tag；v0.93.8 错误 Release 是否清理待用户确认。

### v0.93.9 - 侧栏「任务」提升为一级菜单（拆书/指导书提炼/续写训练归位）

**为什么**：用户要求把续写训练与拆书、指导书提炼放到一起并提升可发现性。现状：拆书在「创作」组、指导书提炼只是拆书页内页签、续写训练在默认折叠的「分析」组——三件同属「从作品/方法论练习写作」的事分散且两处在折叠区。

**改动**：①`Sidebar.tsx` 五组→六组，新增一级「任务」组排在创作之后（默认展开）：全部任务 / 拆书 / 指导书提炼 / 续写训练；拆书移出创作组、续写训练移出分析组、原运维组「任务」子项删除。②`ViewType` 新增 `guidebook-distillation`，`App` 路由渲染 `<BookDeconstruction initialTab="guidebooks" />` 并纳入 FULL_HEIGHT_VIEWS；`BookDeconstruction` 新增 `initialTab`，页内页签切换同步 `setCurrentView`，侧栏高亮跟随。③`Sidebar.ia.test` 重写为六组/25 项 + 四项跳转契约；视口验收加入指南书/训练两页并改 agency wiring 入口。

**验证**：Rust **2083 / 17**（无 Rust 改动）；tsc 0；Vitest **701 / 3**（+2）；Playwright **144 / 5**（+6）；prettier/guard 全绿；1400×900 真渲染截图确认「任务」四入口与指导书页签高亮。**未关闭**：真机升级复测六组布局与窄态分组手感。

### v0.93.7 - StyleDNA 宽容解析（DNA 失效真根因）+ 训练任务行复用

**为什么**：用户要求对本机几个训练实例再分析并修复。任务日志抓到风格 DNA 失效真根因：`JSON 解析失败: missing field 'temporal_quality'`——LLM 漏一个字段即整份 DNA 报废（`StyleDNA` 八结构体无 serde default），可复现、与 LLM 故障无关；且 v0.93.2 的「立即跑」无计划时每次仍新建 once 行（真机 legacy「（手动）」行永不被复用）。

**改动**：①`domain/style.rs` `StyleDNA` + 七个子 profile 加容器级 `#[serde(default)]`，LLM 缺字段/缺整块回退默认（存量 DNA 含全字段不受影响）。②`TrainingLab.findTrainingTaskByBook(title, schedule)` 按书名+调度类型双向复用：立即跑先复用同书 once 行（update payload + trigger），不再堆积。

**验证**：Rust **2083 / 17**（+2）；tsc 0；Vitest **699 / 3**；Playwright **138 / 5**；fmt/clippy/check/guard 全绿。**未关闭**：真机下一轮训练验证 DNA 生效与 overall 回升。

### v0.93.5 - 续写质量评估落地 + deploy-server 编译修复

**为什么**：用户要求对本机续写训练做质量评估并应用成果提高续写质量。《阴脉先生》首轮报告 overall 0.32；逐字对照生成文与真实第11章实锤：①第10章结尾悬停点（看房/血红豆眼老鼠）就在【前文】tail 里，模型仍跳切新委托（承接断裂）；②风格 DNA 静默失效（style_dna_applied=0），生成无任何风格材料。另 v0.93.4 的 src-server 限流 SQL 被 CI Docker 真实编译抓出 E0308（sqlx 把 COUNT 推断为可空，本地无 PG 编译不到）。

**改动**：①`coordinator.rs` 新增 `CONTINUITY_HARD_RULE` 常量，三处续写 writer 指令模板统一注入「必须紧接【前文】末尾现场、未完结事件先落地、禁止跳切」。②`continuation_training.rs::build_style_anchor`：DNA 失败时取最后 2 章末尾各 600 字做 few-shot 文风锚点注入指令 + 任务日志。③src-server 限流 SQL 改 `COALESCE(COUNT(*),0) AS "n: i64"` 强制非空。

**验证**：Rust **2081 / 17**（+3）；tsc 0；Vitest **699 / 3**；Playwright **138 / 5**（前端零改动）；fmt/clippy/check/guard 全绿。**未关闭**：真机下一轮训练对比 overall 回升幅度；deploy-server 重部署后匿名上传验证。

### v0.93.4 - agency 错误入日志 + 修订熔断兜底 + 日志上传免登录自动授权

**为什么**：用户真机在代理工作室派发「修订场景 #11」报「修订主创熔断（达到最大轮数），且黑板无修订稿」，并要求①agency 错误记入日志②修复该错误③日志上传免登录+自动上传（弹窗同意才传）。查证：agency 全 crate 零 log::error（失败只在 DB/聊天）；修订路径只有精确 key 捞稿一层兜底（续写路径的 prose fallback 与 v0.80.4 任意 key 捞稿都没接）；网关路径「error sending request」被判 Fatal 0 重试（v0.86.0 分类死代码）；上传在应用端与服务端两跳都强制登录。

**改动**：①失败链补 log::error（run 失败臂×3 处共用、run_task 主臂/父等待臂、finish_directed_task、update_chat_task_status 最终串、导演回合失败）+ 派发 log::info。②`revision_draft_with_fallback` 三层兜底（精确 key→任意 key→prose 直出），writer_task 拆 base_task。③`AppError::LlmRequestFailed`（Retry）+ wrap 点分类，网关候选闪断先重试一次。④上传免登录：应用删 Device 拒绝、token 改 Option、服务端 `Option<AuthClaims>` + 1h>50 包限流（需 deploy-server）。⑤自动上传：启动+24h 循环检查「新 ERROR 且 >24h 未传且今天未拒绝且 24h 未提示」→ 打包发 `log-upload-requested` → App.tsx backstage 窗口 confirmDialog 同意才传，拒绝记当天。

**验证**：Rust **2078 / 17**（+4）；tsc 0；Vitest **699 / 3**；Playwright **138 / 5** 基线不变；fmt/clippy/check/guard 全绿。**未关闭**：deploy-server 后真机复测熔断兜底/日志 ERROR 可见/免登录上传全路径。

### v0.93.3 - 导出格式修复（中文字体/段落规范化）与幕后切书死端加固

**为什么**：用户真机报告①「小说导出多种格式乱码，MD 正确其他都有问题，EPUB 要严格按幕前显示的样式」——EPUB/PDF/TXT 全是字面 `</p><p>`，PDF 还有 ``` 围栏残留且中文不可读（TimesRoman 无中文字形 + 空白切分折行）；②「幕后点击非当前小说，幕前停留在旧的最后章节而非新书的最新章，重启才正常」。根因：①`scenes.content` 是 tiptap `getHTML()` 存量（`<p>` 连写无换行），导出按 `\n\n` 分段整段转义；②切书链有多个静默死端（IPC 挂起永久卡死 / 懒加载失败留半切换态 / selectStory 静默未落地），既有 E2E 的 mock 章节自带 content，从未覆盖真机懒加载路径。

**改动**：①新增 `export/content.rs`（唯一清洗入口：块级标签切段、行内剥壳、实体解码、去 ``` 围栏、nbsp 转空格），MD/HTML/TXT/模板上下文统一走规范化段落。②EPUB 样式对齐 `frontstage.css` `.ProseMirror`（衬线、两端对齐、缩进 2em、行高 1.8、段距 0.75em），标题页/大纲/目录补 html_escape。③PDF 重写：内嵌 Noto Serif SC（OFL，`src-tauri/assets/fonts/`，`font_subsetting` 子集化）+ 按显示宽度折行 + 标题/章名居中 + 段首全角缩进。④切书加固：flush 5s 超时、失败降级继续、`selectStory` 返回落地结果未落地重试一次、懒加载两跳皆败按空正文应用元数据。⑤mock 补真机语义（aggregated 聚合 extra 场景、`hangNextInvoke` 注入）+3 回归 E2E。

**验证**：Rust **2074 / 17**（+16：content 12 + pdf 4）；tsc 0；Vitest **699 / 3**；Playwright **138**（+3）；fmt/clippy/check/guard 全绿；PDF 内嵌字体端到端测试 + QuickLook 视觉探针通过。**未关闭**：真机复测 EPUB/PDF 导出（WPS 阅读）与幕后切书全路径。

### v0.93.1 - CI 分章用例超时加固

**为什么**：v0.93.0 CI frontend-check 在 `FrontstageApp.split-auto-switch.test.tsx` 的「分章命中当前章」用例失败——内部 `waitFor` 上界 10s，但 Vitest 默认测试超时 5s，CI 负载抖动下先触发 5s 超时；本地连跑 3 次均通过。

**改动**：该用例显式传 20000ms 测试超时，与既有 CI 抖动加固口径一致。

**验证**：Vitest **699/3** 全绿；Rust **2055/17** 不变；tsc/prettier/guard 全绿；未本地打包。**未关闭**：v0.93.0 功能真机复测（见下）。

### v0.91.1 - MCP 映射层版本/权限契约 + Legado 工具列表与回环代理修复

**为什么**：栗子读书（Legado 0.14.24）真机接入暴露工具列表 0（标准 `inputSchema` 与内部 `parameters` 不兼容，解析失败被静默吞掉）与回环请求经系统代理被判 400「请求不完整」；源系统字段/权限变化缺少明确失效与报错规则。

**改动**：①新增 `mcp/mapping.rs`（`mcp-mapping/1.0.0`）：协议白名单 2024-11-05/2025-03-26/2025-06-18、Legado 0.14.x 源版本门、基础 10 读工具、Network/Write/Private 权限快照；单工具 schema drift 隔离，基础工具缺失整机失效；查询被撤销权限工具报 `MCP_PERMISSION_DENIED`/`MCP_PERMISSION_CHANGED`。②`McpTool` 兼容 `inputSchema`、`McpResource` 兼容 `mimeType`；`refresh_tools/resources` 不再静默吞错。③回环 HTTP（localhost/127/::1）改走单次 write 的原生 HTTP/1，绕开 reqwest 分次写与系统代理；非回环仍用 reqwest。④前端修 `McpTool.parameters` 类型、保存并连接置位、外部工具前缀名规范化与执行错误展示。⑤新增结构化 `MCP_*` 错误码（`AppError::McpMapping`）。

**验证**：Rust **2047 / 16**（+12）；live Legado probe 通过直连 1122（11 工具、`get_chapter_content` 有 schema、`list_groups` 调用成功）；tsc/Vitest **696/3**；fmt/clippy/check/guard 全绿；未本地打包。**未关闭**：CI 安装包真机复测 MCP 页工具列表与权限撤销报错。

### v0.90.0 - 主工作流观测闭合

**为什么**：清查发现 agency 四角色主工作流在幕后几乎是「隐形的」——生成链路只追旧编排器（agency 不开 trace）、仪表盘运行记录读已废弃的 `genesis_runs` 表且创世无 UI 入口、用量只有总量没有角色维度。

**改动**：①`coordinator.rs` 新增 run 级 trace 注册表（`RUN_TRACES` + `RunTraceGuard`），genesis/continue/role_task/batch_body 四个入口登记 trace；`AgencyLlm` 每次调用记录角色 step 并透传 trace_id；导演回合开 `director.plan` 短 trace。②新增 `AgencyRunsPanel`（读 `agency_list_runs` + 发起 `agency_start_genesis`），删除废弃 `GenesisPanel`。③`get_llm_usage_by_role` 按 `llm_calls.purpose` 聚合角色用量，数据洞察加角色卡。

**验证**：Rust 新增 trace 注册表单测；Playwright **130 / 5**（+3 观测闭合探针）；Vitest **696 / 3**；tsc/prettier/guard 全绿。**未关闭**：原生安装包真机复测创世/追踪。

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

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **StoryMoss** (31423 symbols, 66977 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "main"})`.
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
