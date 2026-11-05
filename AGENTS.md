# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.82.6
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
- `cargo test -p storymoss` ✅ 2007 passed / 13 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划 / 判官一致率 / 手法回灌 / 卷纲 / 稳定性回归门·概念体检 / 语料校准 / 真机功能探针（4 项含换模矩阵）/ 后台链效率（同内容 ingest 复用·摘要+快评合并）/ 创世熔断 salvage·单调用回退 / 超时语义拆分 / 空正文冷却·上下文自动探测 / 续写跨档兜底 / 角色身份·改名去重·实体链接 / v0.74 语料仓储·V149 lane/task/lease·导演角色/分区·异步调度·租约·聊天/动作·IPC 契约 / v0.74.1 导演 ToolLoop 拒绝·动作校验·空正文候选剔除·capability 归一化·关系别名 / v0.74.2 大提示词超时豁免·稳定前缀排序·语料分段摘要 / v0.74.3 stable/dynamic 前缀拆分·冷预填充超时校准 / v0.75.0 情感回路·V150·OCC·压力场·读者面板 / v0.76.0 批量续写章节情感弧·gate/revision 同计划·persist_chapter_plan / v0.77.0 多 write 并行·V151 章号序列·pinned 语料·真机导演/语料探针 / v0.77.1 资产引导租约·图片语料元数据/摘要 / v0.78.0 视觉多模态·图片提炼/转资产·真机视觉探针 / v0.79.0 叙事契约 P0·场简报·契约门禁·情感最小链·active story 切换·姓名校验 / v0.80.0 章契约·1–3 场投影·legacy 方法论/叙事策略接线·回滚开关·多故事切换 E2E / v0.80.1 SceneOutcome 落库·姓名账本/称呼块 / v0.80.2 复合姓·候选池·作者称呼规则 / v0.80.3 批量 LLM 章契约·确定性兜底·环境开关 / v0.80.4 多代理熔断 salvage·调度器状态语义·网关 tool_calls·ingest 宽容解析 / v0.80.5 导演 LeadWriter 派发接续写管线·原子章号·GateRunner 装配·导演指令注入 / v0.80.6 导演 target_chapter 修订既有章·原 scene 更新·修订仍过 gate / v0.80.7 章节下拉就地改名·中文数字标题展示 / v0.80.8 幕后窗口关闭白屏修复（prevent_close + from_config 重建） / v0.81.0 幕后 UI 重设计 P0 基建（Modal/ConfirmDialog/Table 等 8 组件·Button 收敛·z 契约） / v0.81.1 导航重组 创作5·资产7·运维5·分析5·系统1·双壳金条指示 / v0.81.2 核心六页模板化·Scenes 800px 溢出修复·视口验收探针 / v0.81.3 数据诊断页模板化·Logs/Tracing 高度魔法数清除·探针 21 页 42 用例 / v0.81.4 confirmDialog 全局入口·20 处 confirm 迁移·UserMenu 暗色化·Settings 文案删除·探针 23 页 46 用例 / v0.81.5 弹窗限高内滚补齐·Card 圆角契约 / v0.81.6 设置控件作用域统一·页面级错误边界 / v0.82.0 伏笔排序/去重/配额8/预算保护/硬指令（+3 测试） / v0.82.1 WriteTimeBundle 限额 3/1→8/2（续写主路径） / v0.82.2 导演 target_scene 指定场景修订·场景 override·gate 只更新目标 scene / v0.82.3 导演快照场景序号·审批卡目标显示 / v0.82.4 伏笔文本计划「收：第N章」进窗口提示 / v0.82.5 中文数字·多条计划优先逾期 / v0.82.6 V154 伏笔计划文本回填结构化窗口 ·共 13 ignored）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 679 passed / 3 skipped（v0.81.4 +2：confirmDialog 回退/Portal 确认取消；v0.81.1 +1：Sidebar IA 契约按新五组重写；v0.81.0 +19：Modal 4、ConfirmDialog 3、Field/controls 3、Table 4、Skeleton/EmptyState 2、Button 别名与 token 回归 3；v0.80.7 +4：中文数字标题展示 1、下拉就地改名 2、大标题铅笔入口 1；v0.79.0 +3：story-selected seq/origin 契约 2、生成中切换集成 1；v0.75.0 +1：代理工作室情感页签契约/张力/债务/读者面板；v0.74.3 徽章文案改为冷缓存预估（断言更新，数量不变）；v0.74.2 +2：大提示词徽章显示/阈值以下不显示；v0.74.0 +3：语料页列表/详情/AI 提炼与交给导演；AgencyStudio +11：四角色/黑板/时间线/run 选择/错误提示/去重/观察/资产跳转/发送消息/审批确认；v0.73.14 +5：`StyleDnaModal` 单一风格先选后确认 / 关闭不落库 / 弹窗限高 / 混合面板固定按钮与保存 / 混合配置异步回填；v0.73.13 +7：`useExecutionState` 取数与降级 3、`loggedInvoke` 失败日志 4；v0.73.12 +2：文思活跃火焰/被动静态图标契约；+11 幕前切章正文同步 / 幽灵保留 / 懒加载可重开；+2 章纲重规划预览/勾选与忽略；+判官一致率卡；+章法偏好渲染与重置；+2 卷纲页签与合同展示；v0.68.1 分章切换用例 CI 超时加固：`asyncUtilTimeout` 5s + 三处显式等待 10s）
- `npx playwright test` ✅ 94 passed / 5 skipped（v0.81.4 +14：探针 23 视图 46 用例；v0.81.3 +20：视口探针扩到全导航 21 页；v0.81.2 +12：核心六页 800×600/1400×900 视口验收探针（无横向溢出断言+截图）；v0.80.7 +1：章节下拉就地改名 E2E；v0.80.0 +1：多故事切换 E2E；v0.74.0 +2：语料页加载/选中/AI 提炼；代理工作室剧组/对话/现场与 `agency_chat_send`；v0.73.9 +1：故事卡片点击同步 `notify_frontstage_story_selected`；v0.73.5 +1：日志页「复制」按钮不被幕前悬浮启动器遮挡（1680px trial click）；v0.73.1 +2：模型页签「超时」子页签直达超时设置、写作页签在 `list_workflows` 返回 null 时不崩溃；v0.72.3 修掉一处既有 flake：`e2e/mock-tauri.ts` 补 `check_update`/`get_current_version` handler——`useUpdater(true)` 挂载 5s 后自动检查，mock 未实现时慢机/并行负载下页面加载类用例会在「无 console 错误」断言上偶发红；门禁仍非阻塞，见未关闭）
- `cargo +nightly-2026-10-05 fmt` ✅（固定版本，避免 nightly 漂移）
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.73.13 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 起瘦身，v0.77.0 归档 v0.74.2；根文件只保留最近 5 个版本与关键教训）。

### v0.82.6 - V154 伏笔计划文本回填结构化窗口

**为什么**：127 条未回收伏笔 target 全空，计划只在文本里；看板/逾期/续写都拿不到结构化窗口。

**改动**：新增 V154 Rust 迁移——扫描 target_end_scene 为空的 setup 伏笔，从 content 解析
「收/回收：第N章」（阿拉伯/中文数字），取最早计划章号回填；无计划不写、幂等。

**验证**：Rust **2007 / 13**（+1）；cargo check / nightly fmt 全绿；前端基线不变。

**未关闭**：迁移只在启动跑一次；后续新增伏笔仍靠文本解析兜底；真机复测。

### v0.82.5 - 伏笔文本计划解析补齐（中文数字+多条优先）

**为什么**：v0.82.4 只解析阿拉伯数字、且清单多条计划盲取第一条。

**改动**：①`parse_chapter_number` 支持中文数字（一/二/两…十/百/千）；②
`parse_planned_payoff_chapters` 收集全部「收：第N章」，优先逾期（≤当前场）最小的一条，
否则取最近未来窗口。

**验证**：Rust **2006 / 13**（+1）；cargo check / nightly fmt 全绿；前端基线不变。

**未关闭**：跨行拆字的计划不解析；真机复测。

### v0.82.4 - 伏笔文本回收计划进窗口提示

**为什么**：127 条未回收伏笔 target 全空，计划只写在文本里（「埋：第1章；收：第9章」），
结构化窗口为空时 prompt 拿不到回收窗口。

**改动**：`get_writing_hints` 在 target_end 为空时解析 content 的「收/回收：第N章」作为计划
回收章号，窗口提示同结构化分档（逾期尽快回收/临期禁止提前/远期只可铺垫）。仅阿拉伯数字。

**验证**：Rust **2005 / 13**（+1）；cargo check / nightly fmt 全绿；前端基线不变。

**未关闭**：中文数字不解析；清单 blob 多条计划只取第一条；真机复测。

### v0.82.3 - 导演场景调度可用化（快照+审批卡）

**为什么**：v0.82.2 支持 target_scene，但导演巡检快照没有场景编号，LLM 无法可靠填写；
审批卡也不显示目标场景。

**改动**：①`CrewSnapshot` 新增 `scenes`（最近 24 场：序号/章号/标题/字数）；
`render_director_user_prompt` 输出 Scenes 区块（标注可作 target_scene）。②`AgencyStudio`
动作摘要显示 `场景N`（优先）或 `第N章`。③Crew 快照场景测试 + 导演 prompt 场景行断言。

**验证**：Rust **2004 / 13**（+1）；vitest **679 / 3**；playwright **94 / 5**；fmt/clippy/guard 全绿。

**未关闭**：真机复测；快照只取最近 24 场。

### v0.82.2 - 导演 target_scene 场景级修订

**为什么**：v0.80.6 的导演只能按章修订；作者需要「只改第 N 场」的调度能力。

**改动**：①`DirectorActionSpec.target_scene`（scenes.sequence_number，仅 lead_writer，与 target_chapter 互斥）。
②执行器写 meta；调度器校验场景存在并按 chapter_id 映射章号，走 `run_revise_scene_with_cancel`。
③批量管线支持显式 scene override：writer 以目标场景正文为基底（key「第N章场景修订」，
prompt 标注「指定场景」），gate 通过后 `assemble_existing_chapter` 只更新该 scene。
④导演提示词升 0.82.2 并补规则/示例。

**验证**：Rust **2003 / 13**（+1 集成测试）；cargo check / nightly fmt 全绿；前端基线不变。

**未关闭**：导演快照未列场景编号；仅 lead_writer 支持；真机复测。

### v0.82.1 - 续写主路径伏笔限额修正

**为什么**：v0.82.0 改了 snapshot/adapter/continue/contract 的配额，但续写 writer 实际走
`WriteTimeBundle::load_sync`，bundle 层仍有 `snapshot.pending_foreshadowings(3)` /
`overdue_foreshadowings(1)`，真实路径仍只拿 3 条。

**改动**：`write_time_bundle.rs` 改 8/2，与 CreativeAssetSnapshot/Adapter 对齐。

**验证**：Rust **2002 / 13**；cargo check / nightly fmt 全绿；前端不变。

**未关闭**：结构化回收窗口样本全空；真机复测伏笔使用。

### v0.82.0 - 伏笔进入续写热路径

**为什么**：作者反馈「埋了一堆伏笔、做了回收计划，续写正文完全没伏笔的事」。本机 DB 实证：
主故事 127 条未回收伏笔（importance 全 5、目标窗口全空），旧实现只取 ORDER BY importance,
created_at 的前 3 条 → 最老 3 条永久占位；伏笔块在动态区末尾，预算裁剪第一个清空；指令
「适时推进」是可选项。

**改动**：①`get_writing_hints` 排序（逾期/已入窗 > 临期 > 未来窗口 > 无窗口）＋去重＋截断
（普通 60 字 / 计划清单 240 字）。②pending 3→8、overdue 1→2（snapshot/adapter/continue/
chapter_contract 同步）。③伏笔块前置到动态区首位、900 字上限；文案改【本拍伏笔（必须使用）】
＋「本段正文必须至少推进其中 1 条」硬指令＋未来窗口只可铺垫。

**验证**：Rust **2002 / 13**（+3）；cargo check / nightly fmt 全绿；前端基线不变。

**未关闭**：结构化回收窗口仍依赖伏笔看板「目标窗口编辑」保存（样本全空）；旧计划清单只作文本
提示；需真机复测 3 章内伏笔使用。

### v0.81.6 - 设置控件作用域统一 + 页面级错误边界

**为什么**：91 个设置原生输入散落 13 面板、视觉不一致；单页运行时崩溃会整屏空白（仅根级
ErrorBoundary）。

**改动**：①`Settings` 内容区加 `.settings-panel-scope`，`index.css` 作用域样式一次覆盖
input/select/textarea（range/checkbox/radio/color/file 豁免），对齐 controlClass。②App 每个视图包
`<ErrorBoundary key={currentView}>`，单页崩溃可切走恢复。

**验证**：tsc / prettier 全绿；vitest **679 / 3**；playwright **94 / 5**（46 视口探针）。

**未关闭**：自定义向导弹窗保持自绘；真机复测设置输入外观。

### v0.81.5 - 弹窗限高内滚补齐 + 圆角契约

**为什么**：还有 8 处手写弹窗无 max-height/内滚，600px 最小窗口下会撑出视口；Card 默认
rounded-2xl 与圆角契约（控件 lg / 卡片 xl）冲突。

**改动**：①8 处弹窗补 `max-h-[85dvh] overflow-y-auto` + 遮罩 p-4（WorldBuilding×2、
Dashboard×2、Characters、Stories×3、GenresTab）。②Card 默认圆角 xl；UpgradeModal /
ErrorBoundary / BookUploadPanel 同步；幕后 rounded-2xl 清零。

**验证**：tsc / prettier 全绿；vitest **679 / 3**；playwright **94 / 5**。

**未关闭**：Settings 面板 Field/Input 逐面板迁移；真机复测弹窗滚动。

### v0.81.4 - P3 确认体系收口 + UserMenu 暗色化 + Settings 文案

**为什么**：`window.confirm`/裸 `confirm()` 共 20 处散落各页，样式不可控且与 ConfirmDialog 组件脱节；
UserMenu 浅色嵌在暗色 Sidebar；设置面板仍有附录 A 的机制说明段；Agency 三页标题带故事名、Learning
按钮用原生 indigo。

**改动**：①新增 `ui/confirm.tsx`：`confirmDialog()` Promise 式全局入口 + 根节点 `ConfirmPortal`，
未挂载时回退 window.confirm（既有单测不破）；20 处确认全部迁移，危险文案逐字保留。②UserMenu
全令牌化暗色。③Settings 七处文案删除/压缩（约束句保留）。④Agency 三页页头对齐、indigo 按钮改 Button。
⑤视口探针扩到 23 视图 ×2 档 = 46 用例（窄态折叠组程序化展开）。

**验证**：tsc / prettier 全绿；vitest **679 / 3**（+2）；playwright **94 / 5**（+14）。

**未关闭**：设置 13 面板原生输入未逐面板替换为 Field/Input（组件已就绪）；自定义向导弹窗保持自绘。

### v0.81.3 - P2 数据诊断页模板化 + 文案删除

**为什么**：P2 落地设计文档 §4/§8 剩余页面：Logs/Tracing 仍有 100vh 魔法减法；
诊断页页头自造、科普段冗余；内联 hex 尚存。

**改动**：①Logs/Tasks/Corpus/Maintenance/Cascade/Skills/StorySystem/Tracing/NarrativeAnalysis/
IntentionGraphDiagnostics 接 PageHeader（meta 承载计数与安全承诺句）。②Logs/Tracing 撤 100vh
魔法数改 flex 链；NarrativeAnalysis 栅格加断点；SVG hex→token；Logs 未定义 cinema-1xx→ai-ink。
③空态/骨架统一；附录 A 对应文案删除（约束句保留）。④视口探针扩到 21 页 ×2 档。

**验证**：tsc / prettier 全绿；vitest **677 / 3**；playwright **80 / 5**（+20）。

**未关闭**：P3 设置 13 面板 + Agency 工作台 + UserMenu 暗色化 + window.confirm 18 处迁移
ConfirmDialog。

### v0.81.2 - 核心六页模板改造 + 文案删除 + 视口验收探针

**为什么**：P1-2 落地设计文档 §4 模板与 §8 文案处置：六页各有自造页头/教学句空态；
Scenes 双栏 780px 下限在 800px 最小窗口必然溢出；Dashboard 还在引用未定义 cinema-50/300。

**改动**：①Characters/Stories/WorldBuilding/Foreshadowing 接入 PageHeader（新增 meta 插槽，
WorldBuilding 保留「保存中...」状态词）。②Scenes 改 `grid lg:grid-cols-[minmax(280px,360px)_minmax(0,1fr)]`
+ <lg 堆叠（右栏 70dvh 保底）；Foreshadowing/Dashboard/Stories 统计栅格加断点。③7 处空态改
EmptyState；附录 A 对应文案删除（约束半句全保留）。④Dashboard cinema-50/300→ai-ink、rounded-2xl→xl。
⑤新增 12 用例视口验收探针（无横向溢出断言+截图）。

**验证**：tsc / prettier 全绿；vitest **677 / 3**；playwright **60 / 5**（+12；角色页 3 处 e2e 断言
同步新页头文案）。

**未关闭**：P2 数据诊断页；P3 设置+工作台；ConfirmDialog/Table 随 P2 落地。

### v0.81.1 - 侧边导航重组 + 双壳视觉统一

**为什么**：诊断组 10 项混杂运维与分析；创作工具孤组只有 2 项；两套导航壳 active 样式不一致。

**改动**：①`Sidebar` NAV_GROUPS 改为创作5/故事资产7/运维5/分析5/系统1，运维/分析默认折叠，
23 条目 label/icon/impact/view key 零改动。②Sidebar 与 StudioNavRail 统一 active 指示
（左 2px 金条 + 金淡彩 + aria-current）；Sidebar gray-\* 色全部改 ai-ink 令牌；删「打开幕前
写作」下冗余说明句。③Sidebar IA 契约测试按新五组重写；日志页 e2e 分组入口同步。

**验证**：tsc / prettier 全绿；vitest **677 / 3**（+1）；playwright **48 / 5**。

**未关闭**：P1-2 核心页模板改造与文案删除（Dashboard/Stories/Characters/WorldBuilding/Scenes/
Foreshadowing）+ 800×600 截图验收。

### v0.81.0 - 幕后 UI 重设计 P0 基建

**为什么**：幕后 27 个页面长期双壳并行、组件分裂（91 个裸输入框、20+ 手写弹窗、无 sticky 表头）、
800px 窗口下多页溢出、38 条教学式冗余文案。先出设计文档（`docs/plans/2026-11-04-backstage-ui-redesign-design.md`，
含六类诊断实证与四条铁律），经批准后按 P0→P3 四阶段实施。

**改动**：①新增共享组件 Modal（限高 85dvh 内滚/Esc/danger）、ConfirmDialog、PageHeader、Field、
Input/Select/Textarea（共享样式）、Table（横滚+吸顶+骨架/空态）、EmptyState、Skeleton、layers（z 契约）。
②Button 收敛为 primary/secondary/ghost/danger 四规范变体，paper/cinema/cinema-outline 转兼容别名，
默认 primary，修复未定义 cinema-100/300/50 引用。③Toaster 令牌化 + zIndex 70；tokens.css 增
--z-nav…--z-toast。④设计决策入档：双壳保留、导航重组清单（创作5/资产7/运维5/分析5/系统1）。

**验证**：tsc / prettier 全绿；vitest **676 / 3**（+19）；playwright **48 / 5** 基线不变。

**未关闭**：P1 导航重组+双壳视觉统一+核心页模板；P2 数据诊断页；P3 设置+工作台；文案处置随各页落地。

### v0.80.8 - 幕后窗口关闭后重开白屏

**为什么**：用户从幕前点设置打开幕后，关闭幕后窗口后再点设置，第二次只剩白窗。根因是
`on_window_event` 的 `backstage` CloseRequested 分支只 `hide()`、没有 `api.prevent_close()`：
窗口仍按默认行为被销毁。下一次 `show_backstage` 找不到窗口，重建的 WebView 在
Windows/WebView2 上是空白页。启动时隐藏的同一个窗口首次 `show()` 是正常的，说明问题不在
显示路径，而在窗口被销毁。

**改动**：①`backstage` 关闭先 `api.prevent_close()` 再 `hide()`，保留同一个 WebView，后续
打开走既有尺寸微调/JS 重排恢复路径。②`show_backstage` 的缺失窗口重建改用
`tauri.conf.json` 的 backstage 配置（`WebviewWindowBuilder::from_config`），与启动时一致；
配置缺失才回退手写 builder，并记录 warn 日志。

**验证**：Rust **1999 passed / 13 ignored**（行为修复，无新增用例）；`cargo check` /
nightly fmt 全绿；前端未改动（vitest 657 / 3、playwright 48 / 5 基线不变）。

**未关闭**：原生窗口关闭/重开无法被自动化测试覆盖，需安装包真机复测（幕前点设置 → 关闭幕后
→ 再点设置），macOS / Windows 均需重启应用后验证。

### v0.80.7 - 章节列表就地改名 + 手写中文数字标题

**为什么**：用户截图反馈「AI 有时给章节取名，有些不会，用第几章取名；想把章节名改成第一章，发现
不可修改」。根因有两条：①章节下拉列表只有「选中切换」，没有改名入口，改名只藏在双击标题的
手势里；②`displayChapterTitle` 把所有 `第X章`（含作者手写的「第一章」「第八章」）都当成章号
派生占位，改名后展示又被 `chapter_number` 覆盖，看起来没生效。

**改动**：①`FrontstageHeader` 章节下拉每行新增铅笔按钮，任意章节就地改名（Enter/失焦保存、
Esc 取消、不切换当前章、菜单保持打开）；新增 `onRenameChapterById`。②`FrontstageApp` 抽出
`handleRenameChapterById`：当前章仍优先 `update_scene`（store 有 sceneId），其他章走
`update_chapter`（后端同步 scene 标题），统一用 `applyChapterTitleLocally` 刷新章节/场景/store。
③`EditableChapterTitle` 大标题旁新增铅笔按钮（双击保留）。④`displayChapterTitle` 只让
阿拉伯数字占位标题跟着 `chapter_number`，中文数字手写标题原样显示。

**验证**：Rust 未改动（**1999 / 13** 基线）；vitest **657 / 3**（+4）；playwright **48 / 5**（+1）；
tsc / prettier / guard 全绿。

**未关闭**：自动取名与自动分章仍把中文数字标题视为占位（重排时可能跟随章号）；真机安装包需
重启后复测。

### v0.80.6 - 导演修订既有章（target_chapter）

**为什么**：v0.80.5 的 lead_writer 派发只能续写下一章；导演无法修订既有章节。v0.80.6 增加
`target_chapter` 语义，让同一派发动作既能续写也能修订，且质量门与章号语义不互相绕过。

**改动**：①`DirectorActionSpec` 增加 `target_chapter: Option<i32>`；`validate_action` 只允许
lead_writer 使用且要求 ≥1。②`DirectorExecutor` 把 target_chapter 写入 task meta；
`TaskScheduler` 校验目标章存在后跳过新章号分配，调用
`run_revise_chapter_with_cancel`。③`write_revision_chapter` 以目标章现有正文为修订基底，
加载本章大纲/资产/导演指令，写入 board key `第N章修订`（不生成新章纲）；④`handle_gate`
收到 `existing_scene_id` 后走 `assemble_existing_chapter`，gate 通过才更新原 scene 的
content/outline，不创建新 scene、不推进 beat 计数；gate 失败不覆盖。⑤导演提示词增加
target_chapter 规则与示例。

**验证**：Rust **1999 passed / 13 ignored**（+1：target_chapter 修订既有章、gate 装配、
原 scene 更新；校验用例补 3 断言）；vitest **653 / 3**；playwright **47 / 5**；
tsc / clippy / fmt / guard 全绿。

**未关闭**：target_chapter 只到章级（不支持指定 scene/版本）；修订装配后不重新 ingest KG
（避免重复写入）；真机复测需重启安装包；人类 A/B、设置页灰度、真机样章。

### v0.80.5 - 导演派发 LeadWriter 接入质量门与装配

**为什么**：v0.80.4 修好了熔断不丢稿与状态语义，但导演派发的 LeadWriter 仍只写黑板草稿，
用户看不到故事正文；若直接落库又会绕过 GateRunner 与章号分配。

**改动**：①`DirectorExecutor` 对 lead_writer dispatch 写入 `directed_chapter=true`；
`TaskScheduler` 识别后走 `run_directed_chapter_task`：检查 write lane 上限 →
`story_chapter_sequences::allocate_chapter_numbers` 原子分配下一章 → 调用
`run_continue_batch_with_cancel`（count=1）走 GateRunner/handle_gate/装配。
②新增
`run_continue_batch_with_instruction`/`with_cancel`；`write_chapter` 接受 director directive，
在章契约与情感块之后注入【导演派发指令】，不得覆盖世界观/本章大纲/情感契约；空指令时
普通批量 prompt 逐字不变。③任务摘要带章号/scene/verdict，成功 follow-up；失败/取消回填
聊天并同步 cancel flag。④导演提示词明确「派发主创 = 原子章号 + GateRunner + 装配」。

**验证**：Rust **1998 passed / 13 ignored**（+2：导演指令注入+gate 装配、调度器原子章号+gate）；
vitest **653 / 3**；playwright **47 / 5**；tsc / clippy / fmt / guard 全绿。

**未关闭**：导演派发尚无显式目标章参数字段，lead_writer 派发按「下一章」处理（**v0.80.6 已闭环**）；
真机复测需重启安装包；人类 A/B、设置页灰度、真机样章。

### v0.80.4 - 按本机多代理日志修复熔断/状态语义

**为什么**：本机 2026-11-02 日志里 4 次 `agency tool_loop: LeadWriter 熔断（达到最大轮数 10）`；
黑板 draft 已有 2097 字草稿，但 task 被标 cancelled、不追加 follow-up，导演据此反复重派
同一主创任务（用户看到「主创都待命了」）。同时 156 次 `content 为空但 reasoning_content 有 N 字符`
说明网关把「空 content + 有效 tool_calls」误判为空正文候选；ingest 还因 `Companion` 与浮点
整数字段连续 3 次失败。

**改动**：①`run_role_task_reported` 对 LeadWriter/Writer 熔断从黑板 Draft 区取回 ≥200 字
active 草稿，标 completed 并回报聊天；`Scheduler::run_task` 只在 cancel 令牌命中时标 cancelled，
MaxTurns/Deadline/ParseFailures 落 failed 并带熔断主因，失败/取消回填聊天占位消息。
②`run_role_task`（auto_write/auto_revise）熔断时取回完整草稿，取不到返回 Err，不再把
「（达到最大轮数，已熔断）」拼进正文。③`gateway_content_is_empty` 认可非空 `tool_calls`。
④`RelationType::Ally` 接受 `Companion` 别名；`importance`/`age` 接受浮点与数字字符串。

**验证**：Rust **1996 passed / 13 ignored**（+5）；vitest **653 / 3**；playwright **47 / 5**；
tsc / clippy / fmt / guard 全绿。

**未关闭**：导演派发的 LeadWriter 仍只写黑板草稿，未自动装配进故事正文——**v0.80.5 已闭环**；
真机复测需重启安装包；人类 A/B、设置页灰度、真机样章。

### v0.80.3 - 批量续写接入 LLM 章契约（确定性兜底）

**为什么**：v0.80.0 的章契约仍是确定性编译；章级方向只来自 LLM 章纲，没有结构化的
「戏剧问题/欲望/反对者/赌注/转折/不可逆结果」规划。

**改动**：①新增 `resources/prompts/planner/chapter_contract.md`；`generate_chapter_contract_llm`
用章纲、上一章末段、角色目标、关系、伏笔、下一节点生成 JSON，解析后覆盖确定性章契约；
失败/通用字段/空字段回退确定性契约。②只在批量 `write_chapter` 调用（每章 1 次）；单拍 Append
仍用确定性契约，避免每拍额外 LLM。生产（有 app_handle）默认开启，测试默认关闭；
`STORYMOSS_LLM_CHAPTER_CONTRACT=0/1` 可强制关闭/开启。

**验证**：Rust **1991 / 13**；vitest **653 / 3**；playwright **47 / 5**；tsc/clippy/fmt/guard 全绿。

**未关闭**：人类 A/B；设置页正式灰度开关；真机样章；LLM 契约生成质量需多题材复测。

### v0.80.2 - 复合姓解析 + 姓名候选池 + 作者编辑称呼规则

**为什么**：账本把「欧阳修远」拆成姓「欧」；作者看不到/改不了字、号、小名、绰号与公开/私下称呼；
候选池仍是 0。

**改动**：①`split_formal_name` 按复姓表切分（欧阳/上官/诸葛/慕容…）；账本与称呼规则用完整复姓。
②`suggest_candidates(formal, count)` 保留姓氏、按语义域生成候选并避开模型陈词组合。
③新增 IPC `get_character_name`/`update_character_name`；旧角色首次打开称呼面板按需落账；
`CharacterEditModal` 新增字/号/小名/绰号/公开/私下/敌对称呼/取名故事并写回 `AddressRule`。

**验证**：Rust **1991 passed / 13 ignored**；vitest **653 / 3**；playwright **47 / 5**；
tsc / clippy / fmt / guard 全绿。

**未关闭**：LLM 章契约生成/校验；称呼 UI 只编辑「众人」一条规则；候选池仍是小样本；
人类 A/B；设置页灰度开关；真机样章。

### v0.80.1 - SceneOutcome 结构化落库 + 姓名账本/称呼块

**为什么**：v0.80.0 的进度块仍从正文末段推断，没有「结果/状态变化/未决问题」的结构化真源；
姓名只有生成后校验，没有账本、没有称呼图进入 writer prompt。

**改动**：①V152 `scene_outcomes` + `SceneOutcomeRepository`；Append/NextChapter/批量 handle_gate
装配后写回；`progress_lines_from_parts` 优先读最近 3 章结果。②V153 `character_names` +
`NameLedgerRepository`；角色创建同事务落账；旧故事按需从 characters 回填；`render_parts`
生成【本场称呼（公开/私下）】并注入 writer 资产区。

**验证**：Rust **1988 passed / 13 ignored**；vitest **653 / 3**；playwright **47 / 5**；
tsc / clippy / fmt / guard 全绿。

**未关闭**：LLM 章契约生成/校验；完整字/号/绰号候选池与作者编辑界面；称呼规则仍为默认全名；
人类 A/B；真机样章。

### v0.80.0 - 章契约 + 1–3 场投影 + legacy 资产接线 + 回滚开关 + 多故事切换 E2E

**为什么**：v0.79.0 只做了场契约；章级方向仍由自由 Markdown 章纲承担，批量续写只拿一个场契约；
方法论/叙事策略资产在 bundle 里加载但 Agency 主路径不消费；契约/姓名没有一键回滚；多故事切换只有单测没有 E2E。

**改动**：①新增 `ChapterContract` 与 `compile_chapter_contract`：从节拍卡 + 章纲切 1–3 场，
上一场 `outcome` 成为下一场 `entry_state`；`render_chapter_contract` 注入稳定资产区。
②`write_chapter` 改为章契约 + 1–3 场简报，gate 取第一场核对；`BatchChapterArtifacts` 保留第一场契约。
③`render_continue_assets` 稳定区新增 `methodology_extension`（本章方法论）与 `narrative_quartet`（叙事策略/幕结构）。
④新增回滚开关：`STORYMOSS_NARRATIVE_CONTRACT=0` 关闭契约、`STORYMOSS_NAMING=0` 关闭姓名校验。
⑤扩展 `e2e/mock-tauri.ts` 支持 `extraStories` + `emitSyncEvent`；新增 `e2e/frontstage-story-switch.spec.ts`。

**验证**：Rust **1984 passed / 13 ignored**；vitest **653 passed / 3 skipped**；playwright **47 passed / 5 skipped**；
tsc / clippy / fmt / guard 全绿。

**未关闭**：章契约仍是确定性编译（LLM 生成/校验在后续）；完整 NameLedger/称呼图/DB 落库（M4）；
`SceneOutcome` 结构化落库；legacy 追读力/参考场景全量映射；人类 A/B；真机样章。

### v0.79.0 - 叙事契约 P0 + 多作品切换 + 姓名校验

**为什么**：续写被诊断出四类结构问题（情节无因果、无戏剧冲突、情感只有标签、进展缓慢），
同时幕后选中另一部小说时幕前因「生成中」守卫拒绝切换；姓名塌缩（二字全名 + 小说姓 + 冷色用字）是独立资产问题。

**改动**：①新增 `agency/narrative_contract/`：`SceneContract`/`StateDelta`/`EmotionChain`、
确定性编译、场简报、契约核对；单拍 writer prompt 用「本场契约」替代四行节拍卡，批量 `write_chapter`
也注入场契约；`GateRunner`/`evaluate_gate_impl` 接收契约，具名阻力缺席/正文过短/契约无效强制修订。
②情感最小链默认启用：无作者契约时 `compile_emotion` 返回「事件→评估→情绪→行动→代价」，
`load_emotion_plan`/`load_chapter_emotion_plan` 不再无契约就关闭。
③多作品切换：后端 `set_active_story/get_active_story` 持久化 `active_story_id` + `seq`，
幕前 `StorySwitchController` 先 flush → 处置在途生成 → 按 seq 加载 → 原子应用/清理；删除生成中
toast-return 守卫；旧故事事件用 story_id/epoch 过滤；修复 `chaptersRef` 未更新时的跨故事空正文懒加载。
④`naming` 模块校验重名/同用字/同姓扎堆/长度塌缩/模型陈词组合；创世角色谱逐组校验；
修正「禁止常见姓/禁止单字名」的反模式提示词。
⑤`progress_lines_from_parts` 改为最近 3 章实际正文末段。

**验证**：Rust **1981 passed / 13 ignored**；vitest **653 passed / 3 skipped**；playwright **46 passed / 5 skipped**；
tsc / clippy / fmt / guard 全绿。

**未关闭**：章契约 LLM 生成与 1–3 场投影（M3）；完整 NameLedger/称呼图与 DB 落库（M4）；
legacy 方法论/追读力资产映射；人类 A/B；双窗口多故事切换 E2E。不得宣称「已解决全部叙事质量」。

### v0.78.0 - 图片语料接入视觉模型

**为什么**：v0.77.1 的图片语料只能读标题/摘要/标签；用户确认 `deepseek-v4.1-flash` 具备视觉输入，
应把原图真正送进提炼/转资产。

**改动**：①`LlmAdapter` 新增 `generate_with_images`（默认忽略图片，保持 Anthropic/Ollama 兼容）；
`OpenAiAdapter` 把 data URL 作为 `image_url` 内容块与文本一起发送。②`CorpusService::distill` /
`promote` 对图片语料优先选视觉 OpenAI 兼容 profile（后台档优先；`capabilities=Vision` 或
`deepseek-4.1/v4/flash`），读取 data URL 走视觉调用；解析/调用失败自动回退摘要+标签文本路径。
③新增真机视觉探针 `real_machine_probe_vision_image_input`。

**验证**：Rust **1953 passed / 13 ignored**；vitest 647 / 3；playwright 45 / 5；tsc / clippy / fmt /
guard / IPC manifest 全绿。真机 `deepseek-v4.1-flash` 3.9s 正确描述 `logo.png`。

**未关闭**：视觉仅 OpenAI 兼容 provider；图片提炼质量依赖模型视觉能力；摘要/标签仍是回退。

### v0.77.1 - 资产引导租约 + 图片语料可用化

**为什么**：v0.77.0 放开多 write 后，同故事多个 run 可能同时 `ensure_assets`，重复生成世界/角色/关系/大纲；
图片语料没有正文，提炼/转资产/固定上下文只能看到标题。

**改动**：①`ensure_assets` 获取 `write:assets` 租约（TTL 600s），只让一个 run 生成资产；其余 run 最多等
120s，超时返回 `resource_busy`，不再“等不到就重复生成”。②图片导入纯 Rust 读取 PNG/GIF/JPEG/WebP/BMP
尺寸，生成可编辑摘要；`create(kind=image)` 无摘要时也有兜底说明。③`distill`/`promote` 无正文时使用
标题/摘要/标签/文件/类型/大小结构化信息，并明确「未启用视觉识别，不要编造画面」；固定语料块沿用摘要/标签。

**验证**：Rust **1952 passed / 12 ignored**（+2 图片尺寸/摘要测试）；vitest 647 / 3；playwright 45 / 5；
tsc / clippy / fmt / guard / IPC manifest 全绿。

**未关闭**：未接入视觉模型/OCR，图片像素内容仍不可读；作者需填写摘要才能让 AI 更具体地使用图片。

### v0.77.0 - 多 write 并行 + 固定语料 + 真机端到端

**为什么**：v0.76 之后仍有三项收尾：同故事 write lane 只能单活跃、作者固定的语料没进续写上下文、
导演规划/语料提炼没有真机端到端证据。

**改动**：①V151 把 V149 的 `(story_id, lane)` 唯一索引改为只约束 review/inspect/chat；
write lane 允许多活跃 run，命令层上限 3；`story_chapter_sequences` 原子分配章号区间。
②幕前 Append 对同一场景加 `append:{scene_id}` 租约；beat 计数器/资产历史改为
`BEGIN IMMEDIATE` 读改写。③`CorpusRepository::list_pinned` + `pinned_context_block`，把
固定语料只读注入单拍/批量 writer prompt（≤4 条、≤1200 字）。④新增两个真机探针：
导演规划（CrewSnapshot → 导演提示词 → Tool 档 → DirectorPlan）与语料提炼（同源提示词 →
Background 档 → CorpusDistillation → 写回 distilled_json）。

**验证**：Rust **1950 passed / 12 ignored**；vitest 647 / 3；playwright 45 / 5；
tsc / clippy / fmt / guard / IPC manifest 全绿。真机实跑：导演规划 5.1s（intent=repair、
1 动作）；语料提炼 66.1s（facts/conflicts/hooks/angles 各 5 条并写回）。

**未关闭**：并行 write 的 `ensure_assets` 仍可能重复生成资产（幂等兜底）；固定语料仅文本摘要；
真机探针样本单一，需更多故事/模型复测。

### v0.76.0 - 批量续写接情感回路（章节级 1–3 幕）

**为什么**：v0.75.0 只接了单拍主路径 `write_beat_once`；批量续写 `write_chapter` 是一章一次
tool_loop，既不消费情感契约，gate 也读不到尚未落库的章节计划，装配还把 card 丢掉。

**改动**：①新增 `ChapterEmotionPlan`/`plan_chapter_emotion`/`split_chapter_into_acts`：按章纲
关键事件生成 1–3 幕，每幕复用 `EmotionBeat`；无契约返回 None。②`write_chapter` 返回
`BatchChapterArtifacts`，在写作要求前注入≤900 字【本章情感回路（批量续写）】。③`GateRunner`/
`evaluate_gate`/`evaluate_gate_impl`/`run_rule_grader` 接收章节计划；editor 参照资产追加计划，
`build_revision_task_with_emotion` 让修订保留本章弧与每幕代价。④装配后 `persist_chapter_plan`
写 N 行 `scene_emotion_beats`/状态/张力/探针；card 仅在情感启用时回传，补齐批量 scene 元数据。
⑤`run_batch_inner` 用 `last_written_plan` 把上一章末幕 `to` 作为下一章第 1 幕 `from`。

**验证**：Rust **1946 passed / 10 ignored**（+11）；vitest 647 / 3；playwright 45 / 5；
tsc / clippy / fmt / guard / IPC manifest 全绿。

**未关闭**：人类 A/B 与真实模型端到端批量情感目标达成率待复测；批量探针仍 advisory。

### v0.75.0 - 情感回路：情感驱动情节

**为什么**：AI 小说最大的缺点不是文笔，而是读者不在乎——情感停留在形容词层，没有成为
情节、冲突、对白与人物行为的因果发动机。EmotionCircuits-LLM（arXiv:2510.11328）证明情绪
是模型内部可分离、可稀疏调制、可因果验证的方向/电路；本项目把它映射到叙事可观测层。

**改动**：①新增 `src-tauri/src/emotion_circuit/` 领域模块（12 维情绪向量、OCC 评估、行动
倾向、8 类表达载体、压力场、调制器、确定性探针、读者面板、题材契约建议、白盒适配端口）
与 V150 七张表；②续写前先选最高压「角色-关系-债务」组合，再决定本拍事件的情感读法，
writer prompt 动态区注入【本拍情感回路】；③每拍落库情感链/角色情绪状态/张力累积，ingest
把 `mood` 折成 12 维状态回流，情感债务可兑现/延期/忽略；④编辑审查新增第 9 维 emotion，
rule grader 规则分改为合同 0.4 / 追读 0.3 / 情感 0.3（无计划节拍中性 0.5）；⑤代理工作室
右栏新增「情感」页签，角色编辑弹窗补情感四元组，新增 5 个 IPC 命令与 e2e mock。

**验证**：Rust **1935 passed / 10 ignored**（+45）；vitest **647 passed / 3 skipped**（+1）；
playwright **45 passed / 5 skipped**；tsc / clippy / fmt / guard / IPC manifest 全绿。

**未关闭**：人类 A/B 尚未跑，不得宣称「已能感动读者」；读者面板只做代理指标；批量续写
`write_chapter` 尚未接情感回路；真实模型端到端情感目标达成率待复测。

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
