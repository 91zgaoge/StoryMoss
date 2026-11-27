# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.99.28
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

**并行批量取稿必须按章号（v0.99.0，续写训练 13/14、16/17、19/20 正文逐字相同）**：

- 并行设计是 gate(n) 与 writer(n+1) 并发，黑板**最新** draft 通常是下一章。任何「熔断兜底按任意 key 取最新稿」都会把第 n+1 章正文装进第 n 章；writer(n+1) 随后又装配同一稿 → 相邻章字节相同，而真实稿（draft11/15/18）从未落库。报告里的「承接断裂/原地踏步/整段复制上一章」大多由此产生，不要先怀疑模型幻觉。
- **铁律**：批量路径取稿必须 `item_type=chapter` + 严格解析章号（`第N章`/`第N章修订`；禁止用 `第1章` 前缀匹配，否则第 10 章会串到第 1 章）；修订三层兜底第 2 层也只能同章；落库前裁掉正文首行章名，判分/装配前检测跨章复制。

**长程记忆必须全量注入（v0.99.1，10 章训练承接 1.2–3.1 根因）**：

- **只注入「上一章结尾 1400 字」不够**：它只解决首章落笔点；前 10 章的逐章事实、未结线索、人物当前态、设定白名单必须作为整批固定的《前情提要》进入 writer **与 planner**。实机反例：第 142 章「赵开来赠送福摩萨夜总会道场/去见邵卫江」在正文 81% 处，writer 进度块只取最近 3 章末 220 字，完全看不到；生成第 11 章全文 grep「邵卫江/福摩萨」=0，模型自造「闭口燕」危机。
- **铁律**：`progress_lines_from_parts` 不得再用 `take(3)` 或 `outcomes` 整体替换上下文；`generate_chapter_outline` 必须单独接收前情提要与上一章结尾（不能依赖 3200 字 directive）；训练批内必须维护滚动账本（gate∥writer 并发时 n-1 在途稿先以「刚写定稿」入账）；训练路径禁止 `ensure_story_outline` 用「正文头 600+尾 1800」生成「单章回顾+下一拍规划」伪大纲（`with_assets_seeded(true)` 跳过 ensure_assets）。
- **前情提要要保在 protected 区**：`ASSET_CHAR_BUDGET=6000` 会从尾部压缩动态块；有 brief/账本时预算放宽到 9000，brief 放在红线之后、故事大纲之前，不得被 `prior/progress` 挤掉。

**资产重建必须两阶段 + 模型容量路由（v0.99.2，本机 v0.99.1 实跑 0.63/承接 7.2 但 asset 失败）**：

- **一次 26k prompt + 8192 max_tokens 的「全资产 JSON」不可靠**：deepseek-v4 真机 completion 23,877/23,921/23,914 tokens 全烧在 reasoning、content 为空；Qwren127 报 `Prompt (16781 tokens) requires ~9439MB GPU memory but only ~3387MB available`；Ds10 同样空。改「逐章摘要（单章小 prompt 并发 3、可缓存）→ 聚合 brief（只喂结构化摘要）」后，本地模型可承载，聚合仍可走远端；失败用确定性事件摘要兜底。
- **确定性摘要不能只取首尾 70 字**：第 142 章「邵卫江/福摩萨夜总会」在正文 81% 处，首尾窗口都漏；用「首句 + 中段冲突/引号对句 + 尾句」事件摘要，把 `key_terms` 带进 brief 的 canon 白名单兜底。
- **本地 GPU OOM 要从错误里学习容量上限**：解析 `Prompt (N tokens) requires ~X MB ... only ~Y MB` → model health `prompt_limit_tokens≈N*Y/X`；`resolve_role_model`/`select_candidates`/候选循环都要按 prompt 大小跳过（Qwren 实测约 4–6k tokens，不再反复 OOM）。空 content 的 HTTP 200 不得记 success（llm_calls 成功率要真实）。
- **评分口径要分离**：`continuity` 只相对上一沙盒章；`reference_gap` 单独表示与原书后续差距。16/17/20 章「承接 1/2/2」是 judge 把 truth 差异混进 continuity 的误判，不能再把 truth 当承接基准。
- **批内进度要回调**：`run_continue_batch_with_cancel` 需 per-chapter progress sink，训练任务把 done/total/chars 写 task progress/log/heartbeat；否则任务在 40% 停 45 分钟、UI 日志不动。

**日志保留与训练主角/修订（v0.99.12，重启丢 tracing + 主角错位 + 7/10 章修订未完成）**：

- 日志清理只在启动执行，条件反转极难在单测之外被发现：`cleanup_old_logs` 曾把 `mtime < cutoff` 当成「未过期」继续保留，实际每次启动删掉刚写入的当日 tracing、留下 7 天前旧文件（真机训练窗口日志整段消失）。此类代码必须用真实 mtime 回归测试锁语义（保留当日、删除过期、非日志前缀不动、超大文件删除）。
- 训练 roster 不得假定「角色表第一项=主角」：角色模型漏返回时确定性 cast 兜底按出场顺序建角色，真机首项是配角「大波浪卷女人」。主角只认原文自称句式（我叫X/我姓X/以X身份/X这个身份/X自报/自称X），并把「我/主角/主人公/阴脉先生」合并到权威姓名；证据不足或并列宁可不标注。
- 修订完成必须拿到「与首稿不同、≥300 字、非工具 JSON」的新稿；只判断 tool_loop 正常结束会把「模型没写黑板」误记成已修订（真机 7/10 章判修订但 0 章完成）。训练批量改用「完整正文 + 逐条问题」的直出修订，写回本章 key 后再走第二轮 gate。

**推理模型的额度/思考强度/探测（v0.99.11，空 content 2,377 次 + 连续 8 次主创熔断）**：

- **`max_tokens` 是 CoT 与正文共享的预算**。DeepSeek-V4.1-Flash 默认 `effort=high`，writer 只申请 8192 时 CoT 吃满、`finish_reason=length`、`content` 为空。适配器必须解析 `finish_reason`/`reasoning_tokens`，在「空正文 + length + 有 reasoning」时放大额度重试一次（×4、封顶 32768）；`stop` 空响应不重试。
- **`reasoning_effort` 必须真的发出去**。API 校验值域 `none/minimal/low/medium/high/xhigh/ultra/max`，模型卡支持 low/high/max、默认 high。profile 显式值优先；JSON/结构化任务对声明 `Reasoning` 能力的模型默认 `low`；未声明 reasoning 的本地端点不得发送（未知参数会 400）。
- **健康探测不能把「busy/CoT-only」当「down」**。4 tokens 探针会让推理模型必然空正文；5s 超时对大模型首字（20–60s）必然误判。探测给 64 tokens/30s，空正文 WARN 做 60s 节流，否则 10 秒一次的探测刷屏会埋掉真实生成失败。
- **冷却不能连坐唯一候选**。`candidate_count=1` 时一次空正文把唯一模型冷却 10 分钟，会让后续所有请求直接报「所有候选模型都因连续空正文进入冷却」；全部候选被剔除时必须恢复候选兜底，而不是硬失败。

**幕前 CAS 基线必须按 scene 绑定（v0.99.13，10 次假冲突 + 上一章末次编辑丢失）**：

- `persistSceneContent` 曾在 `await prev` 之后才读共享的 `sceneBaselineContentRef`（只存一段裸正文）。切章时 `selectChapter` 把该 ref 换成新章 DB 正文，上一章在途/排队的保存便拿**新章指纹**校验旧 scene → 后端正确判 `scene_conflict`，前端误报「另一窗口已更新」并停止重试（真机 10 次冲突全部发生在切章后 10–50ms，冲突对象都是上一章 scene，最末几次编辑因此从未落库）。
- **铁律**：CAS 基线必须带 `sceneId`，并在保存**入口同步捕获**；跨 scene 时宁可不发送指纹（null），也不得挪用别的 scene 的指纹；成功回写只在 ref 仍跟踪同一 scene（或尚未跟踪）时进行，避免在途保存覆盖新章基线。
- 视觉空稿（`<p></p>`/`<p><br></p>`/`<p>&nbsp;</p>`）一律不写库：后端 v0.93.16 会忽略正文但仍返回命中 1 行，前端若把基线更新成空稿，之后每次保存都会永久冲突。切章瞬间编辑器短暂空文档正是这类写入的来源。

**更新检查多源裁决（v0.99.13，12 次 check_update + 3 次 install_update 假失败）**：

- GitHub 源被网络阻断（本机实测请求返回空）、官网 `latest.json` 正常且明确「无更新」时，旧 `conclude_update_checks` 仍因「任一源失败」整体报错——用户每次启动都看到「无法完成更新」。多源更新里「有源成功给出确定答案」就是可用的答案；另一源失败降为 `warning` 诊断字段（前端只记 warn 日志、不弹错）。只有**所有**源都失败才报错（此时确实无法判断，不能误报「没有更新」）。

**训练沙盒章号必须沿用原书真实章号（v0.99.14，143 章后续写首章被编成第 11 章）**：

- 训练沙盒不是独立新书：`scenes.sequence_number` / `chapters.chapter_number` 必须就是原书章号（如 134–143），续写从真实后续基准首章（144）起。禁止用 `(i+1)` 灌入或 `context_n+1` 当续写起点——否则 143 章后起续写会落成「第 11 章」，报告写「沙盒第 11 章 vs 原书第 144 章」，两套章号错位。
- 报告/编审总评里的沙盒章号必须由真实后续章号（`truths[0]` 或 batch 生成的 `chapter_number`）派生，不能再用 `context_n+1+i` 内部序号。回归：`sandbox_continuation_start_uses_source_chapter_numbers`、`build_report_uses_source_chapter_numbers_for_sandbox_range`。

**续写训练主线/文风必须逐章硬注入（v0.99.15，原著主线被弃 + 冷硬 Noir 稀释）**：

- **故事大纲的「下一节点」会压倒上一轮报告里的一行主线建议**。资产重建只读上下文 134–143 章，`next_node` 会归纳成「就地审问伪装护士」；writer 严格按硬大纲写，把原书 144 章「去福摩萨找邵卫江」推成「明日」，随后另起老曹—法林寺平行支线。truth_hints 训练必须用**本轮真实后续章**逐章摘要生成 `MainlineProjection`（逐章 goal/cast/location/key_terms/must_terms），写进沙盒故事大纲（删旧 `【下一节点】`）并按章注入章纲、writer、gate；`must_terms` 只取真实章出现频次前二人物 + 首地点，匹配要允许多字专名去掉通用中心语的别名（「福摩萨夜总会」→「福摩萨」、「大波浪卷女人」→「大波浪卷」），否则会把已落地的章误判缺失、反复强制修订。回归：`agency::mainline` 6 条、`SceneBeatCard::align_to_mainline`。
- **StyleDNA 六维指标不能替代原文语体样本**。`force_style_anchor = !style_applied || style < 6.0` 在 style≈7 时恒 false，10 章批量每章只喂上一章生成稿，风格逐章向冷硬 noir 自我稀释。必须每章注入原文语体样本 + 量化基线（句长/对白/第一人称/口语词/比喻/短句占比），并输出文风漂移与 AI 腔探针；v0.99.17 加「叙述长句样本」（全章节按得分取最高 2 段口语 + 1 段叙述，不再只看首/中/尾章），v0.99.18 再加单章 AI 腔硬核对（咯噔≥2/后脊梁发麻/比喻>4/千字）与真实章 `voice_cues`/`key_turn`（关键台词/转折机制进 writer）。回归：`agency::style_probe` 7 条、mainline `voice_cues` 1 条。
- **节拍卡/章契约会沿用上一章的具名阻力**。第 144 章主线是邵卫江，卡里仍是杨晓雯，规则复检「具名阻力未出现」会把正确主线的章打回修订；训练主线存在时必须 `align_to_mainline` 覆盖 cast/parties/location/next_node。地点核对要允许场景同义词（夜总会↔包厢/吧台/门厅），否则 146 这类正确章也会被判缺失。

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
cd src-frontend && npx vitest run     # 必须在 src-frontend 下跑
npm test                              # Playwright E2E（仓库根）
node scripts/cdp-inspect.js           # CDP 截图

# 真机训练自动触发（仅 debug 构建，避免 UI 自动化抢前台）
cd src-tauri && cargo build --bin storymoss
STORYMOSS_TRIGGER_TASK=<task_id> ./target/debug/storymoss   # 启动后 8s 触发已注册任务
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
6. **网站 Release 保留策略**：`.github/scripts/upload-releases-ftp.mjs` 每次上传前/后会自动清理 `/releases` 目录，仅保留最近 **3** 个版本的安装包（`RELEASE_RETENTION_COUNT=3`，可通过环境变量覆盖；每版约 660MB，5 版本会在本机磁盘触发 552 Disk full——v0.99.2 官网上传即因此失败），防止服务器空间不足。禁止删除 `latest.json` 与无版本号文件（如 `StoryMoss_aarch64.app.tar.gz`）。v0.30.50 起上传**前**也会先清理（磁盘满 552 自愈）；磁盘满等紧急情况可手动运行 `Cleanup Releases` 工作流（`.github/workflows/cleanup-releases.yml`，workflow_dispatch，默认保留数 3），它调用脚本的 `--cleanup-only` 模式，不上传只清理。
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
- `cargo test -p storymoss` ✅ 2319 passed / 17 ignored（v0.99.28 +5：cron 5 字段补秒/日程 09:00/6 字段保持/非法仍报错 4、关系类型复合与中文归一 1（原「同门师兄妹降级」用例改写）；v0.99.27 +1：reporter 心跳落库 drain 回归；v0.99.26 +5：保活闲置/活跃节奏 3、真实活动仅由真实调用写入 1、探测提示词缓存 TTL 1；v0.99.25 +4：探测失败判定/退避翻倍封顶 2、追加日志旧 ERROR 不计 1、生产布局行级过滤 1（替换 1 条旧计数用例）、未知关系类型降级 1；v0.99.24 +5：训练规则提升 4、V165 迁移 1；v0.99.23 +9：生产复用 production_context 4、StyleDNA 基线 1、writer/gate/batch/genesis 复用 4；v0.99.22 数量不变（文档验证版）；v0.99.21 数量不变（创作 effort 回滚、封顶空稿重试、文风门禁建议化）；v0.99.20 数量不变（创作 reasoning_effort 默认 low，断言更新）；v0.99.19 +1：第一人称 POV + 动作钩；v0.99.18 +4：地点同义词、voice_cues 抽取、单章 AI 腔、risk 证据词；v0.99.17 +2：文风严重漂移判定、compact 语体样本保留叙述长句段；v0.99.16 +4：GPU 容量上限按客户端估算单位学习、估算含 system_prompt、遗留 running trace 启动清扫、真实报告跨词短语探针回归（含 v0.99.15 的 +11）；v0.99.15 +11：主线投影/别名匹配 6、原文语体样本/漂移/AI 腔探针 4、节拍卡主线对齐 1；v0.99.14 +2：续写沙盒首章号沿用原书真实章号 1、训练报告沙盒区间真实章号 1；v0.99.13 +2 净增：多源裁决 3 + warning 序列化 1（替换 2 条旧契约）；v0.99.12 +10：日志保留真实 mtime 1、主角自称识别 1、主角误检回归 1、alias cast 合并 1、探针器物/称谓/重叠碎片 1、修订候选可用性 1、直出修订 prompt 1、LeadWriter 章节写入上限判定 1、LeadWriter 空轮提前结束 1、修订变差回退判定 1；v0.99.11 +8：撞额度空正文重试判定/放大值封顶 2、finish_reason+reasoning tokens 解析 1、reasoning_effort 三态映射 3、全冷却恢复兜底 1、创作角色输出上限断言 1；v0.99.10 +1：白名单碎片/重叠窗口/方位窗口专名过滤（含质量门文案断言扩充）；v0.99.9 +6：对象 suggestions、字符串分值/null 维度、诊断维度不进改进、报告不列诊断短板、线索可读标签、跨词垃圾专名过滤；v0.99.8 +7：中文数字转换、阿拉伯/中文/空格/后缀章号解析、标题格式化/源标题、通用占位识别、V164 旧库标题迁移、V130/splitter 契约更新；v0.99.7 +6：deadline 扩容/上限、编辑审计直连 JSON、失败门禁落 editor_gate_records、确定性摘要裁章名、世界观确定性兜底、角色 grounded cast 兜底；v0.99.6 +1：批量 trace 三态；v0.99.5 +4：llm_calls 无 cost 列统计聚合回归 2、telemetry 功能使用窗口聚合 1、get_feature_usage_stats IPC 注册表契约 1；v0.99.4 数量不变（脚本/CI）；v0.99.3 +5：compact 渲染 2、cache TTL 1、memory probes 2；v0.99.2 +12：确定性中段摘要/摘要缓存/单对象摘要解析/聚合 prompt/brief 字段 roundtrip、GPU OOM prompt-limit 解析/健康档 2、judge reference_gap/blind 过滤 2、batch progress 1；v0.99.1 +12：StoryBrief 渲染/截断 3、V162 建表 1、前情提要/账本渲染 3、资产 brief 落库/确定性兜底/伪大纲替换 3、writer 资产块全量注入 1、planner brief+锚点 1；v0.99.0 +10：跨章熔断兜底回归、章节 key 解析、首尾梗概兜底、资产字段顺序、跨章复制检测、gate 计数、正文首行裁切 4；v0.98.9 数量不变（前端-only）；v0.98.8 +1：日志自动上传错误计数的生产布局回归（`storymoss.YYYY-MM-DD` + JSON level / legacy 文本 / workflow 三源）+ 扫描目录契约守卫；v0.98.7 +8：在途上一章上下文/门禁裁决截断/风格 DNA 注入/幻觉人名过滤/真实后续节点/降级门计数 5 + 编审截断修复 1 + 权威名册 1 + 门禁降级判定 1；v0.98.5 +2：V161 章节缓存幂等建表、缓存 roundtrip/刷新/跨书隔离；v0.98.4 +3：Legado 回环 HTTP 400/-32600「请求不完整」未执行判定、只读工具请求不完整重试、写工具不重发；v0.98.3 +14：JSON 截断/CoT 提取工具 7、风格 DNA 部分 salvage 3、资产 bundle 截断修复/空对象拒绝 2、OpenAI JSON mode CoT 提取 2；v0.98.2 数量不变（预算熔断暂停：check 恒放行，原熔断用例语义反转）；v0.98.1 +1：本地书籍源 1-based 起始章切片边界；v0.98.0 +7：多章训练 payload 默认值/自定义、编审 continuity+verdict、结构化跨章总评、低分维度修正、确定性多章报告、改进项去重限长、V160 迁移幂等；v0.97.0 +4：V159 gate run_id 迁移、QC 单一权威 3；v0.96.1 +2：update_scene no-rows 语义映射·其余错误保持 Internal；v0.96.0 +9：V158 状态守卫 3、租约续期/回收、agency waiting/stale、训练阶段单调、CAS 指纹、叙事 Falling；v0.95.0 +23：lifecycle 转移/取消 RAII 5、MCP Unknown 权限与重试边界、style-learning/recompute RAII、updater 多源、log_report 重复包、订阅过期门控、PostProcess 解析失败、拆书终态重试、projection 前缀；v0.94.0 +6：训练资产重建落库全链路、资产 prompt 契约、键值散文 salvage、未接地人名过滤、逐章梗概映射、判官承接基准；v0.93.16 +5：基准章不泄漏×2（style/承接锚点）/空稿与拒绝话术校验/空稿覆盖守卫；v0.93.15 +10：MCP 只读重试×4（分类/权限/重置重试/写不重发）；判官评分×6（五维齐全/拒绝 null/拒绝缺维/字符串与 7/10 宽容/overall 均值与 0-10 归一/章号标签）；v0.93.14 +1：进度/日志/心跳同源时间；v0.93.13 数量不变（CI 预热）；v0.93.12 +2：StyleDNA 小数/数字字符串平均句长宽容解析；v0.93.11 数量不变（CI 重试）；v0.93.10 数量不变（CI 发布门禁）；v0.93.9 数量不变（纯前端侧栏 IA）；v0.93.7 +2：StyleDNA 缺字段/缺块回退默认；v0.93.6 测试数不变（服务端非宏 SQL）；v0.93.5 +3：风格锚点取尾/短章边界/硬承接常量措辞；v0.93.4 +4：LLM_REQUEST_FAILED 重试分类、错误行计数、自动上传条件判定×2；v0.93.3 +16：导出规范化 content 12（tiptap 分段/实体解码/围栏清理/边界回退）+ pdf 4（CJK 宽度折行/内嵌字体端到端）；v0.93.2 +3：取消监听器置位·共享执行锁互斥语义·UpdateTaskRequest payload 更新；v0.93.0 +3：自动连接状态/训练自动选工具与参数、MCP utils；v0.92.0 +6：instinct 块 promoted 过滤/预算、skill_execute 白名单、写作策略覆盖、叙事节奏块、追读债空返回、导演洞察 prompt；v0.91.1 +12：MCP 映射契约/字段 alias/permission/loopback raw HTTP 测试；v0.91.0 +1：导演 prompt 质量债区块；v0.90.0 +1：run trace 注册表 roundtrip/guard drop；v0.89.9：task_system/narrative/style_dna 三族 IPC 参数契约修复（rename_all=snake_case），测试数不变；v0.89.0 +3：sha256 确定性·上传历史读写去重·manifest 序列化；v0.88.0 +5：HttpTransport SSE 解析·is_http·旧配置兼容·V156 建表幂等·V157 建表索引；v0.87.0 +4：is_chat 缺省/别名解析·人设覆盖运行时生效·四角色人设注册；v0.86.0 +8：target_end_chapter 校验·快照完整 ID·换算边界·2 章批量端到端·越界拒绝·短 ID 解析与失败隔离·reqwest 重试分类·peek 一致性；v0.85.1 +3：GitHub 主源顺序、网络/安全重试边界、备用源同版本；迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线 / 续写检索门 / 章纲重规划 / 判官一致率 / 手法回灌 / 卷纲 / 稳定性回归门·概念体检 / 语料校准 / 真机功能探针（4 项含换模矩阵）/ 后台链效率（同内容 ingest 复用·摘要+快评合并）/ 创世熔断 salvage·单调用回退 / 超时语义拆分 / 空正文冷却·上下文自动探测 / 续写跨档兜底 / 角色身份·改名去重·实体链接 / v0.74 语料仓储·V149 lane/task/lease·导演角色/分区·异步调度·租约·聊天/动作·IPC 契约 / v0.74.1 导演 ToolLoop 拒绝·动作校验·空正文候选剔除·capability 归一化·关系别名 / v0.74.2 大提示词超时豁免·稳定前缀排序·语料分段摘要 / v0.74.3 stable/dynamic 前缀拆分·冷预填充超时校准 / v0.75.0 情感回路·V150·OCC·压力场·读者面板 / v0.76.0 批量续写章节情感弧·gate/revision 同计划·persist_chapter_plan / v0.77.0 多 write 并行·V151 章号序列·pinned 语料·真机导演/语料探针 / v0.77.1 资产引导租约·图片语料元数据/摘要 / v0.78.0 视觉多模态·图片提炼/转资产·真机视觉探针 / v0.79.0 叙事契约 P0·场简报·契约门禁·情感最小链·active story 切换·姓名校验 / v0.80.0 章契约·1–3 场投影·legacy 方法论/叙事策略接线·回滚开关·多故事切换 E2E / v0.80.1 SceneOutcome 落库·姓名账本/称呼块 / v0.80.2 复合姓·候选池·作者称呼规则 / v0.80.3 批量 LLM 章契约·确定性兜底·环境开关 / v0.80.4 多代理熔断 salvage·调度器状态语义·网关 tool_calls·ingest 宽容解析 / v0.80.5 导演 LeadWriter 派发接续写管线·原子章号·GateRunner 装配·导演指令注入 / v0.80.6 导演 target_chapter 修订既有章·原 scene 更新·修订仍过 gate / v0.80.7 章节下拉就地改名·中文数字标题展示 / v0.80.8 幕后窗口关闭白屏修复（prevent_close + from_config 重建） / v0.81.0 幕后 UI 重设计 P0 基建（Modal/ConfirmDialog/Table 等 8 组件·Button 收敛·z 契约） / v0.81.1 导航重组 创作5·资产7·运维5·分析5·系统1·双壳金条指示 / v0.81.2 核心六页模板化·Scenes 800px 溢出修复·视口验收探针 / v0.81.3 数据诊断页模板化·Logs/Tracing 高度魔法数清除·探针 21 页 42 用例 / v0.81.4 confirmDialog 全局入口·20 处 confirm 迁移·UserMenu 暗色化·Settings 文案删除·探针 23 页 46 用例 / v0.81.5 弹窗限高内滚补齐·Card 圆角契约 / v0.81.6 设置控件作用域统一·页面级错误边界 / v0.82.0 伏笔排序/去重/配额8/预算保护/硬指令（+3 测试） / v0.82.1 WriteTimeBundle 限额 3/1→8/2（续写主路径） / v0.82.2 导演 target_scene 指定场景修订·场景 override·gate 只更新目标 scene / v0.82.3 导演快照场景序号·审批卡目标显示 / v0.82.4 伏笔文本计划「收：第N章」进窗口提示 / v0.82.5 中文数字·多条计划优先逾期 / v0.82.6 V154 伏笔计划文本回填结构化窗口 / v0.82.7 补交遗漏新增文件（仓库完整性修复） / v0.82.8 真机探针 导演场景目标·伏笔实际使用（本机实跑） / v0.82.9 Dashboard/Stories 弹窗迁统一 Modal / v0.83.0 体裁·角色·世界观手写弹窗全迁 Modal / v0.83.1 弹窗 800×600 视口回归探针 / v0.83.2 伏笔清单 blob 紧凑计划摘要（埋第X→收第Y） / v0.83.3 升级·角色编辑·模型编辑弹窗壳迁 Modal / v0.83.4 模型编辑 13 控件迁 Input/Select/Textarea（设置逐字段样板） / v0.83.5 AgentConfig·RouteSimulator·PromptsPanel 控件迁共享组件 / v0.83.6 GeneralSettings 9 控件迁 Input/Select/Textarea / v0.83.7 UnifiedModelManager 控件迁移·设置控件组件化完成 / v0.83.8 移除 Google Fonts 阻塞·loading 挂 React 挂载 / v0.83.9 Login 弹窗迁 Modal·暗色化 / v0.84.0 Cinzel 字体自托管·字体零外部依赖 / v0.84.1 StyleDnaModal·ExportDialog 迁共享 Modal·卷号计划·V155 碎片窗口继承 / v0.84.2 V155 计划线程匹配扩展（收后 3 字专名 + 人称过滤）·碎片窗口 9→19 ·共 15 ignored）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 732 passed / 3 skipped（v0.99.28 数量不变（仅任务页/训练页 cron 文案「5 字段」，无契约断言）；v0.99.27 数量不变（Rust-only）；v0.99.26 数量不变（Rust-only）；v0.99.25 数量不变（Rust-only）；v0.99.24 数量不变（Rust-only）；v0.99.23 数量不变（Rust-only）；v0.99.16 数量不变（Rust-only）；v0.99.15 数量不变（Rust-only）；v0.99.14 数量不变（Rust-only）；v0.99.13 +4：切章 CAS 基线 4；v0.99.12 数量不变（Rust-only）；v0.99.10 数量不变（Rust-only）；v0.99.9 数量不变（Rust-only）；v0.99.8 +3：中文数字、标题格式化、幕前标题展示；v0.99.7 数量不变（Rust-only）；v0.99.6 数量不变（Rust-only）；v0.99.5 数量不变（Rust-only）；v0.99.3 数量不变（Rust-only）；v0.99.2 数量不变（Rust-only）；v0.99.1 数量不变（Rust-only）；v0.99.0 数量不变（Rust-only）；v0.98.9 +2：进度事件同时刷新任务列表与对应日志、缺失 `task_id` 不刷新；v0.98.8 数量不变（Rust-only）；v0.98.7 数量不变（Rust-only）；v0.98.5 数量不变（Rust-only）；v0.98.4 数量不变（Rust-only）；v0.98.3 数量不变（Rust-only）；v0.98.2 数量不变（Rust-only）；v0.98.0 数量不变（训练页配置/逐章表为展示改动）；v0.97.0 +3：导航清单六组唯一/布局派生/废弃别名排除；v0.96.0 +1：update_scene CAS 参数与指纹；v0.95.0 +13：拆书 liveStatus 隔离/完成 invalidate、Prompts 草稿不互覆、日志上传票据、Export 审查错误区分、Tasks cancelled 分组与行锁、Training upsert 互斥、向导边界；v0.94.0 数量不变（后端训练链路改动）；v0.93.16 数量不变（Rust-only 修复）；v0.93.15 数量不变（本版无前端改动）；v0.93.14 +3：本地时间格式化（SQLite/UTC/非法串）；v0.93.13 数量不变（CI 改动）；v0.93.12 +2：任务通知「查看任务」写入待聚焦 ID 并切换全部任务页、toast 点击后关闭并聚焦；v0.93.11 数量不变（CI 改动）；v0.93.10 数量不变（CI 改动）；v0.93.9 +2：任务组四项契约、四项跳转独立视图；v0.93.7 数量不变；v0.93.5 数量不变（本轮前端零改动）；v0.93.4 数量不变（agency/上传改动未动契约）；v0.93.3 数量不变（导出/切书由 E2E 覆盖）；v0.93.2 数量不变（任务融合未动契约）；v0.93.0 +3：MCP server/tool 前缀解析、章节工具自动选择、参数模板自动生成；v0.91.1 MCP 前端类型/连接态对齐（数量不变）；v0.90.0 Dashboard 契约随 AgencyRunsPanel 更新；v0.88.0 契约更新：Sidebar IA 23→24（续写训练）；v0.87.0 +10：@提及解析（全角/紧跟文字/叙述不误判/四角色映射）；v0.85.0 +2：创作路径容器布局、Sidebar 窄态分组展开；Panel/Scenes 契约随行为更新；v0.84.1 +5：Modal testid 契约 1、ExportDialog Modal 壳/步骤 4；v0.81.4 +2：confirmDialog 回退/Portal 确认取消；v0.81.1 +1：Sidebar IA 契约按新五组重写；v0.81.0 +19：Modal 4、ConfirmDialog 3、Field/controls 3、Table 4、Skeleton/EmptyState 2、Button 别名与 token 回归 3；v0.80.7 +4：中文数字标题展示 1、下拉就地改名 2、大标题铅笔入口 1；v0.79.0 +3：story-selected seq/origin 契约 2、生成中切换集成 1；v0.75.0 +1：代理工作室情感页签契约/张力/债务/读者面板；v0.74.3 徽章文案改为冷缓存预估（断言更新，数量不变）；v0.74.2 +2：大提示词徽章显示/阈值以下不显示；v0.74.0 +3：语料页列表/详情/AI 提炼与交给导演；AgencyStudio +11：四角色/黑板/时间线/run 选择/错误提示/去重/观察/资产跳转/发送消息/审批确认；v0.73.14 +5：`StyleDnaModal` 单一风格先选后确认 / 关闭不落库 / 弹窗限高 / 混合面板固定按钮与保存 / 混合配置异步回填；v0.73.13 +7：`useExecutionState` 取数与降级 3、`loggedInvoke` 失败日志 4；v0.73.12 +2：文思活跃火焰/被动静态图标契约；+11 幕前切章正文同步 / 幽灵保留 / 懒加载可重开；+2 章纲重规划预览/勾选与忽略；+判官一致率卡；+章法偏好渲染与重置；+2 卷纲页签与合同展示；v0.68.1 分章切换用例 CI 超时加固：`asyncUtilTimeout` 5s + 三处显式等待 10s）
- `npx playwright test` ✅ 144 passed / 5 skipped（v0.99.28 数量不变（Rust-only + cron 文案，全量复跑通过）；v0.99.27 数量不变（Rust-only，全量复跑通过）；v0.99.26 数量不变（Rust-only，全量复跑通过）；v0.99.25 数量不变（Rust-only，全量复跑通过）；v0.99.24 数量不变（Rust-only，全量复跑通过）；v0.99.23 数量不变（Rust-only，全量复跑通过）；v0.99.16 数量不变（Rust-only，全量复跑通过）；v0.99.15 数量不变（Rust-only，全量复跑通过）；v0.99.14 数量不变（Rust-only，全量复跑通过）；v0.99.13 数量不变（Rust-only，全量复跑通过）；v0.99.12 数量不变（Rust-only，全量复跑通过）；v0.99.11 数量不变（Rust-only；全量 141 通过 + 3 个 1024×768 视口用例与 Rust 单测并行抢 CPU 时 5s 超时，空闲后 `backstage-viewport-acceptance.spec.ts` 75/75 复跑通过）；v0.99.10 数量不变（Rust-only，全量 E2E 复跑通过）；v0.99.9 数量不变（Rust-only，全量 E2E 复跑通过）；v0.99.8 数量不变（displayChapterTitle/章节下拉/latest-chapter 契约更新，全量 E2E 复跑通过）；v0.99.7 数量不变（Rust-only，全量 E2E 复跑通过）；v0.99.6 数量不变（Rust-only，全量 E2E 复跑通过）；v0.99.5 数量不变（Rust-only，全量 E2E 复跑通过）；v0.99.3 数量不变（Rust-only+UI 契约，全量 E2E 复跑通过）；v0.99.2 数量不变（Rust-only，全量 E2E 复跑通过）；v0.99.1 数量不变（Rust-only，全量 E2E 复跑通过）；v0.99.0 数量不变（Rust-only，全量 E2E 复跑通过）；v0.98.9 数量不变（全量 E2E 复跑通过）；v0.98.8 数量不变（Rust-only，全量 E2E 复跑通过）；v0.98.7 数量不变（Rust-only，全量 E2E 复跑通过）；v0.98.5 数量不变（Rust-only）；v0.98.4 数量不变（Rust-only）；v0.98.3 数量不变（Rust-only）；v0.98.2 数量不变（Rust-only）；v0.98.0 数量不变（training-lab 视口验收复跑通过）；v0.97.0 数量不变（导航 registry/Gate 收口后全量 E2E 复跑通过）；v0.96.0 数量不变（入口互锁/CAS/租约改造后全量 E2E 复跑通过）；v0.95.0 数量不变（状态机改造后全量 E2E 复跑通过）；v0.94.0 数量不变（后端训练链路改动，视口/功能基线）；v0.93.16 数量不变（Rust-only 修复，视口/功能基线）；v0.93.15 数量不变（本版无前端改动）；v0.93.14 数量不变（诊断页时间格式改动，视口/功能基线复跑通过）；v0.93.13 数量不变（CI 改动）；v0.93.12 数量不变（训练状态横幅/任务聚焦由 Vitest 契约覆盖，视口验收 training-lab 复跑通过）；v0.93.11 数量不变（CI 改动）；v0.93.10 数量不变（CI 改动）；v0.93.9 +6：guidebook-distillation 与 training-lab 加入三档视口验收；agency wiring 改用「全部任务」入口；v0.93.7 基线不变；v0.93.5 本轮前端零改动；v0.93.4 基线不变；v0.93.3 +3：真机懒加载路径切书、update_scene 挂起 5s 超时继续、快速连点停最后点击；v0.93.2 基线不变；v0.93.0 +1：MCP 页自动展示已保存服务器与全部工具；v0.91.1 MCP 页无新增 E2E，基线不变；v0.91.0 +4：级联交给导演、Tasks 代理任务重试、SceneEditor 派发、执行面板 AI 续写；v0.90.0 +3：仪表盘代理运行+创世、生成链路 agency 步骤、四角色用量；v0.85.2 +1：九场景五档几何/滚动/缩放验收；v0.85.0 +29：三档 23 视图扩 23、真实内容布局回归 6；日志遮挡用例改为 Sidebar 普通布局入口；v0.83.1 +3：Modal 800×600 boundingBox+内滚探针；v0.81.4 +14：探针 23 视图 46 用例；v0.81.3 +20：视口探针扩到全导航 21 页；v0.81.2 +12：核心六页 800×600/1400×900 视口验收探针（无横向溢出断言+截图）；v0.80.7 +1：章节下拉就地改名 E2E；v0.80.0 +1：多故事切换 E2E；v0.74.0 +2：语料页加载/选中/AI 提炼；代理工作室剧组/对话/现场与 `agency_chat_send`；v0.73.9 +1：故事卡片点击同步 `notify_frontstage_story_selected`；v0.73.5 +1：日志页「复制」按钮不被幕前悬浮启动器遮挡（1680px trial click）；v0.73.1 +2：模型页签「超时」子页签直达超时设置、写作页签在 `list_workflows` 返回 null 时不崩溃；v0.72.3 修掉一处既有 flake：`e2e/mock-tauri.ts` 补 `check_update`/`get_current_version` handler——`useUpdater(true)` 挂载 5s 后自动检查，mock 未实现时慢机/并行负载下页面加载类用例会在「无 console 错误」断言上偶发红；门禁仍非阻塞，见未关闭）
- `cargo +nightly-2026-10-05 fmt` ✅（固定版本，避免 nightly 漂移）
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.73.13 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 起瘦身，v0.77.0 归档 v0.74.2；根文件只保留最近 5 个版本与关键教训）。

### v0.99.28 - 日志巡检：cron 5 字段任务从不执行 / 心跳 0 秒刷屏 / 关系类型复合值降级

**为什么**：本机 v0.99.27 的 `storymoss.2026-11-27`（8,166 行：2 ERROR / 960 WARN）暴露三处应用侧问题：①两条 ERROR 都是 `Invalid cron pattern '0 9 * * *'` / `'0 1 * * *'`——前端训练页 `DEFAULT_CRON='0 9 * * *'`、任务页占位符都用标准 5 字段，`cron` crate 却要求 6/7 字段（多一个秒字段）；`parse_cron_schedule` 直接 `parse()`，导致 UI 创建的定时任务全部注册失败。库里 `续写训练 · 阴脉先生` 任务 `enabled=1`、`cron_pattern='0 1 * * *'`、`status=pending`，创建后从未执行，只留一条 ERROR。②`[Heartbeat] fired: elapsed=0s` **431 条**（writer 146 / producer 145 / editor 82 / 训练章评 42 / 其余 16），占当日 WARN 45%：`tokio::time::interval` 第一次 tick 立即完成，循环未消费首个 tick，每次 LLM 调用都立即 emit「已等待 0 秒」并写 WARN。③`[Ingest Step2] 未知关系类型「…」，降级为 RelatedTo` **31 次**：v0.99.25 的宽容解析只避免整段 JSON 失败，但 `Family/Partner`、`Friend/Informant`、`Negotiation/Rival`、`PotentialEnemy`、`敌对/控制`、`同伙分账/推诿`、`仇敌`、`合谋`、`主仆` 等可归一值全被降级，知识图谱丢语义；Step2 prompt 仍用 `Friend/Enemy/Family/...` 诱导复合输出。

**改动**：①`task_system/scheduler.rs` 新增 `normalize_cron_pattern`：解析前把 5 字段补 `0` 秒，6/7 字段与 `@daily` 保持原样，错误信息仍显示用户原始表达式；训练页/任务页文案明确「5 字段」。②`llm/service.rs` 心跳 spawn 先 `interval.tick().await` 消费立即 tick，再记 `start`；首条心跳在 10s 后发出。③`memory/ingest.rs` 新增 `normalize_relation_type`：按 `/`、`|`、`、`、`与`、`和` 拆词匹配英文枚举/同义词与中文关键词（`敌对/控制`→Enemy、`同伙分账/推诿`→Ally、`主仆`→MemberOf、`同门师兄妹`→Ally），英文复合词按核心后缀识别（`PotentialEnemy`→Enemy、`SecretAlly`→Ally），仍无法识别才降级 `RelatedTo`；Step2 prompt 改为显式枚举并新增「不要组合或自造」约束。

**验证**：Rust **2319 / 17**（+5：5 字段补秒/5 字段日程 09:00/6 字段保持/非法仍报错 4、复合与中文关系归一 1；原「同门师兄妹降级 RelatedTo」用例改写为归一化 + 真未知降级）；tsc 0；Vitest **732/3**（不变）；Playwright **144/5**（全量复跑）；fmt/clippy/guard/prettier 全绿；`gitnexus impact`：`TaskScheduler::parse_cron_schedule`、`de_relation_type_lenient` 均 **LOW**。**未关闭**：安装 v0.99.28 后确认既有 `0 1 * * *` 训练任务启动即注册并按时触发、`elapsed=0s` 心跳归零、31 类复合关系落到对应枚举；本机 Qwen3.5 keepalive 失败/超时 124 次与 `model-a62431a7` GPU OOM 属环境（端点不可达/显存不足），只记录不修。

### v0.99.27 - 修复 reporter 心跳与 finish 的落库竞态（v0.99.26 CI macOS 唯一红）

**为什么**：v0.99.26 tag 构建 `rust-check (macos-latest)` 唯一失败用例 `agency::reporter::tests::test_reporting_llm_heartbeats_and_finish`（`left: 0, right: 1`，2312 passed / 1 failed）。这不是测试写错，而是真实时序：`TaskReporter::heartbeat` 用 `spawn_blocking` fire-and-forget，`finish` 另起一个 `spawn_blocking` 写完成状态，两者在阻塞线程池上的执行顺序不确定；`TaskRepository::heartbeat` 的 `UPDATE … WHERE status IN ('running','waiting')` 在后到时会静默丢弃心跳，`turn_count` 少计。历史多轮 tag 构建的「reporter heartbeat flake」即此。

**改动**：`TaskReporter` 增加 `pending_heartbeats: Arc<Mutex<Vec<JoinHandle<()>>>>`：`heartbeat` 登记句柄（顺手清理已完成的），`finish` 先 `drain_heartbeats()` 逐个 await（每个上限 5s，超时 warn 后继续）再写完成状态。心跳仍是 fire-and-forget，不阻塞 LLM 调用；只是完成前保证已发出的心跳落库。

**验证**：Rust **2314 / 17**（+1：8 次心跳后 finish，`turn_count == 8` 确定性回归）；tsc 0；Vitest **732 / 3**（不变）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿；`gitnexus impact`：`TaskReporter::finish` / `heartbeat` **MEDIUM**（直接调用方各 5 个：scheduler 任务收尾、chat 收尾、director 派发；签名不变、仅收尾前 drain，无 HIGH/CRITICAL）。**未关闭**：v0.99.27 tag 构建需确认 macOS rust-check 不再红。

### v0.99.26 - 日志巡检：保活探测闲置降频 / 探测提示词缓存（单日 15,584 次探测 vs 45 次真实生成）

**为什么**：本机 v0.99.25 日志（`storymoss.2026-11-26`）全天 **0 ERROR**，但 33,819 行里约 29,000 行（86%）是健康探测：`[LLM] Starting sync generation … prompt_len=33` 15,529 行（其中付费远端 `deepseek-v4` 10,413 次）、`Sync generation completed`（`response_len=2`）13,700 行、`[AppConfig] Loaded from SQLite` 3,324 行；真实生成只有 **45 次**。探测每小时稳定 ~1,000 次，凌晨 2–8 点无人使用也照跑。WARN 907 条中 685 条是 `[GatewayScheduler] keepalive … 失败`（两个端点 5–7% 抖动）：v0.99.25 已修好退避重试（日志可见 30→60→120 递增），但 10s 保活仍对所有健康模型无条件轮询，且每次探测都重新解析提示词（DB 忙时回退 `AppConfig::load` 新建连接池）。

**改动**：①`model_gateway/scheduler.rs` 新增 `keepalive_due`（纯函数）：只保活 Healthy/Degraded，连续失败 ≥3 的死模型交给退避重试；**闲置模型（2 分钟内无真实生成）5 分钟一次，活跃模型 10s 一次**；`last_checked_at` 缺失时立即探测。②`model_gateway/health.rs` 的 `HealthRecord` 增加 `last_real_activity_at`，仅由 `record_success`/`record_failure`/`record_empty_content` 写入（探测结果与快照回填不写），新增 `HealthRegistry::last_real_activity`。③`model_gateway/executor.rs` 新增 `ProbePromptCache`（60s TTL、`Arc` 跨 clone 共享）：探测提示词不再每次查 SQLite/AppConfig，消除 DB 忙时的连接池新建与 `[AppConfig] Loaded from SQLite` 刷屏；用户保存覆盖后 60s 内生效，探测内容不变。

**验证**：Rust **2313 / 17**（+5：保活跳过 Unhealthy/死模型/未探测、闲置 5 分钟、活跃 10s、真实活动仅由真实调用写入、探测提示词缓存 TTL）；tsc 0；Vitest **732 / 3**（不变）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿；`gitnexus impact`：`run_keepalive_probe`/`apply_probe_result`/`record_success`/`record_failure` LOW，`probe_model` LOW（直接调用方 3 个）；`detect_changes` 仅命中既有生成流程 Generate→Registry / Select_fastest_profile→Registry（改动为字段与访问器新增，不改签名）。**未关闭**：安装 v0.99.26 后观察一天，确认探测降到 1,000 次/天量级、`[AppConfig] Loaded from SQLite` 大幅下降、两个抖动端点的 WARN 不再占日志主体。

### v0.99.25 - 日志巡检：健康探测退避失效 / 自动上传旧错误误计 / relation_type 宽容解析

**为什么**：Windows v0.99.14 日志包 `15a05cd0`（11-23 08:07 → 11-25 02:32，tracing 0 ERROR）暴露三处应用侧问题：①三个不可用模型（6d1e9636 / 5bac934b / a2a76bfb）跨 24 小时各被 `[GatewayScheduler] 退避重试` 93–97 次，退避值恒为 30s（从未 60/120/…/3600），连续失败涨到 40–61，同时每轮还记「成功，重置退避」；11-23 启动段 3,363 行日志里约 3k 行是探测与 `AppConfig` 读取。②11-23 08:07:53 `log_report` 提议上传时称「检测到 20 条新错误日志」，实际 20 条全部来自 11-08–11-17（`creative_workflow.log` 只追加）。③08:11:45 `[Ingest Step2] Attempt 1: JSON parse error: unknown variant 同门师兄妹`——Step2 prompt 的 `Friend/Enemy/Family/...` 诱导模型自造类型，旧实现让整段知识 JSON 解析失败并白跑一轮 LLM 重试。

**改动**：①`model_gateway/scheduler.rs` 新增 `probe_succeeded`（`Ok(ProbeResult{success:false})` 视为失败——`probe_model` 只对「模型不存在/禁用」返回 `Err`）与 `next_retry_backoff`（失败 30→60→120→…→3600s 封顶，成功重置）；启动全量探测与 keepalive 也按真实 `success` 记 warn。②`log_report.rs` 的 `count_recent_errors` 增加行级时间过滤（tracing JSON `"timestamp"` / 工作流 JSONL `"ts"` / legacy compact 行首 RFC3339；无法解析时间的行保守保留），只统计窗口内新错误。③`memory/ingest.rs` 新增 `de_relation_type_lenient`：枚举外关系类型 warn 并降级为 `RelatedTo`，关系本身保留，不再让整段知识解析失败。

**验证**：Rust **2308 / 17**（+4：探测失败判定/退避翻倍封顶、追加日志旧 ERROR 不计、生产布局行级过滤、未知关系类型降级）；tsc 0；Vitest **732 / 3**（不变）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿；`gitnexus impact`：`run_retry_probe_with_backoff`/`RelationProfile` LOW，`count_recent_errors` HIGH（仅内部计数语义，不改签名）。**未关闭**：安装 v0.99.25 后确认退避按 30→60→…→3600 递增、探测日志量下降；自动上传提示只统计真新错误；`同门师兄妹` 降级相关图谱质量下一轮 ingest 观察。

### v0.99.24 - 训练报告通用规则提升层 + 生产回归探针

**为什么**：v0.99.23 已把训练能力全部接入生产，但训练报告里的具体改进（含书名/人名/情节）仍只回灌下一轮训练，不能跨故事进入生产线。用户要求把训练报告做成通用规则提升层，并让每轮训练的可泛化结论自动进入生产线。

**改动**：①新增 `agency/training_rules.rs`：从 `continuation_training_reports.scores_json` 的维度均分与 `improvement_md` 的通用信号词（主线未落地/文风修正/AI 腔修正/承接/推进/口吻/节奏/钩子）提取 8 类通用规则，按 `rule_key` 去重累计 `promotion_count`；规则文本只含通用约束，不复制书名/人名/章号/具体情节。②V165 `training_rule_promotions` 表持久化；训练报告落库后自动提升，升级时若表为空从最近 8 份报告幂等补齐。③`ProductionStatic` 加载 active 规则；writer/修订/genesis fastpath/auto_write/单角色 writer 任务与 gate 参照文本注入规则块（上限 1800 字/8 条）。④新增生产回归探针：按 active 规则核对文风漂移、AI 腔、主线落地、承接开篇重复、状态变化、对白互动、章长与章末强钩；批量/单章/genesis 装配后写入 `[ProductionProbes]` 日志，失败 warn。⑤规则提升调用在训练报告落库后；生产路径不读训练报告的具体人物/剧情，只读通用规则文本。

**验证**：Rust **2304 / 17**（+5：规则提升幂等/不复制人名、规则块上限、生产探针失败、空规则、V165 迁移）；tsc 0；Vitest **732 / 3**（不变）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.24 后跑一轮训练再跑生产续写，确认规则自动入库、生产 prompt 出现规则块、`[ProductionProbes]` 无回归失败；训练遗留 146/149/147/148 文风问题继续按报告改进项跟进。

### v0.99.23 - 续写训练能力全量复用（创世/智能创作/普通续写/导演/单章）

**为什么**：训练沙盒独享四条能力：原著主线投影、原文语体样本/量化基线、前情提要（StoryBrief）与直连编审 gate；普通创世/续写只拿到部分能力（`with_mainline_projection`/`with_style_exemplar`/`with_style_baseline` 只在训练执行器设置），因此训练出来的质量改进无法进入真实创作路径。用户要求「必须全部复用，否则续写训练就没有意义」。

**改动**：①新增 `agency/production_context.rs`：生产主线投影从故事大纲 `【核心冲突】/【下一节点】`（兼容旧格式 `核心冲突：/推进方向：`）、CHAPTER 合同 `goal/must_cover_nodes`、卷纲 `goal/climax`、Blueprint 逐章目的派生；语体样本/基线从前文 scene 正文抽取，无前文时退到 corpus 或 StyleDNA 六维基线；StoryBrief 从最近 10 章前文确定性构建（逐章事实/人物态/伏笔线索/设定白名单），已有训练 brief 直接复用；全部确定性、无 LLM。②`MainlineProjection` 增加 `MainlineSource`（Truth/StoryPlan），生产文案改为「故事主线」，训练仍为「原著主线」；故事计划只有大纲/卷纲时不做人物硬缺失（避免第一人称主角姓名误报），CHAPTER/Blueprint 的 `must_terms` 仍硬核对。③coordinator 新增 `production_capabilities`（new 默认开、for_test 关）、静态能力缓存与整批主线缓存；`write_chapter`/`write_beat_once`/`write_revision_chapter`/`generate_chapter_outline`/`revision_prose_direct`/`GateRunner`/`evaluate_gate`/genesis fastpath/`spawn_editor_qc` 全部接入主线、语体、前情提要、红线与 gate；批量中「下一节点」只作用于首章，后续章用合同/卷纲 beat。④生产默认直连编审 JSON；genesis 按题材画像推荐风格 DNA 自动绑定 `stories.style_dna_id`，首章起即有 StyleDNA 基线；自动续写/导演派发的单角色 writer 任务（`run_role_task`/`run_role_task_reported`）也注入语体样本、前情提要、红线等章号无关能力；训练显式字段优先、不覆盖训练 LLM brief。⑤新增生产写作红线（9 条，设定/语体/推进/承接/白名单/长程/主角/主线/章末），训练显式指令已含 TRAINING_REDLINES 时不重复注入。

**验证**：Rust **2299 / 17**（+9：production_context 4、StyleDNA 基线 1、writer/gate/batch/genesis 复用 4）；tsc 0；Vitest **732 / 3**（不变）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：真机创世首章与普通续写复测（主线/语体/前情提要/红线/gate 全部生效），以及前一轮训练遗留的 146/149/147/148 文风问题。

### v0.99.22 - v0.99.21 真机验证：总分 0.734 / voice 7.11 / reference_gap 7.07

**验证**（本机 10 章训练报告 `018e787e-2687-415a-ade8-7f976db6f05b`，13:20–14:12）：**加权总分 0.734**（历轮最高：v0.99.16 0.729、v0.99.20 0.689、v0.99.17 0.7175），style **7.01**、plot **7.67**、voice **7.11**、pacing **7.17**、hooks **7.28**、承接 8.9、`reference_gap` **7.07**（最初问题轮 2.9）；编审 **6 pass / 4 revise / 0 unavailable**（上轮 1/9/0）；生成质量门 3 通过 / 7 修订 / 0 降级；主线落地探针 9/10 ✓；10 章全部落库 2.7–3.8k 字。承接近乎无缝、主线节点与真实章顺序高度重合（145/150/151 几乎逐一对应），148「妖孽，看我法宝」与 151 碑线/152 身世按 voice_cues/mainline 落地。**仍未根治**：146 小梅诊室试探被改村口跪求、149 玉碟敕神/宋辨经考据压成三件套、147 周成荤腔纯情化、148/152/153 偶发格言/预告腔；文风漂移探针句长仍 9.6–14.0（原文 19.2）。下一轮按报告改进项继续。测试 **2290/17**（文档验证版，数量不变）。

### v0.99.21 - 创作 effort 回滚 + 封顶空稿降 effort 重试 + 文风门禁建议化

**为什么**：v0.99.20 真机报告 `21b15629` 显示低 effort 让 10 章全部产出、编审 10/10 可用（上一轮 8/10），但总分 0.689（上轮 0.7169）、style 6.75、voice 6.5（上轮 7.125）：editor 判「黄玄然絮叨被削成短刀、陆尘音功能化、第一人称自嘲递减」。144 章 editor 原判 style 8/voice 8，确定性 style gate（句长 10.8<0.75×19.2）强制修订后报告 style 反而 6.5。空 content 仍出现：writer 输出额度已是 32768，`should_retry_truncated_empty` 的 `max_tokens < cap` 条件使重试从未发生。

**改动**：①`default_reasoning_effort` 回滚到结构化 low、创作保持模型/显式 effort（质量优先）。②`llm/openai.rs` 空稿判定改为 `max_tokens <= cap`，封顶时仍重试一次并把 `reasoning_effort` 临时设为 low（首次调用保留原 effort），日志明确「重试 max_tokens=… 并降 reasoning_effort=low」。③文风/AI 腔 `style_issue` 改为建议性：只在 editor 已判 RevisionRequired 且 style<7 时并入 issues；editor 判 pass 不再因确定性句长强制修订。文风漂移仍在报告与下一轮改进中提示。

**验证**：Rust **2290 / 17**（数量不变，封顶重试断言与创作 effort 回 None 更新）；tsc 0；Vitest **732 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：v0.99.21 重跑同一 10 章训练，确认 voice/style 回到 v0.99.18 水平、空稿重试有效、editor pass 的章节不再被文风门禁误改；148–150 主线钩与小梅身份按报告改进项落地。

### v0.99.20 - 推理模型创作调用默认 low（空 content 止血）

**为什么**：v0.99.19 真机训练启动段 12:05:39–12:08:24 连续 5 次 `agency_writer` 空 content，prompt 37,899 tokens、completion 29,773–31,978 tokens、`finish_reason=length`——writer 输出额度已是 `CREATIVE_MAX_OUTPUT_TOKENS=32768`，v0.99.11 的 ×4 重放无余量；第 144 章草稿只能靠 tool_loop 已写出的 board draft（2,691 字）救回。v0.99.18 同类空 content 曾靠多次尝试偶然成功，训练耗时与产出不稳。

**改动**：`llm::service::default_reasoning_effort` 改为 profile 显式值优先；声明 `Reasoning` 能力的模型，结构化与创作散文调用都默认 `low`；未声明 Reasoning 的 Qwren127 仍不发送未知参数。CoT 不再吃满 32k 输出额度，正文有空间落 content；显式 high 配置仍可覆盖。测试断言同步更新（创作也 low）。

**验证**：Rust **2290 / 17**（数量不变）；tsc 0；Vitest **732 / 3**（数量不变）；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：用 v0.99.20 重跑同一 10 章训练，确认 `agency_writer` 空 content 归零、144–153 产出与报告完整。

### v0.99.19 - 训练二次收敛：第一人称 POV/动作钩 + 叙述样本权重

**为什么**：v0.99.18 真机 10 章报告 `29f3107d` voice 从 6.85 升到 7.125（145 章 editor 写「口语文体贴合样本，短句、自嘲、语气词都在线」「术令内讧正对原书路数」），148/151 pass，主线探针 9/10 ✓、`reference_gap` 6.5；但 style 6.875，144/145/147/149/150/151/152/153 句长 11.3–13.0（原文 19.2），149/150 纪年/玉碟、146 时间线仍有硬伤。gate 2 通过/8 修订，其中 144 因「POV 未在正文出现」「结尾没有新问题」被规则复检打回：第一人称正文只有「我」，POV 姓名「周成」不会出现；「路少，把门关上」是动作钩但不命中旧关键词。

**改动**：①`narrative_contract::verify` 的 POV 核对接受第一人称（正文「我」≥3 次且未出现 POV 姓名），钩子证据词扩到关门/把门/锁/插上/威胁/刀/枪/脚步声/来人/纸条/短信/电话等。②`style_probe::build_exemplar` 改为 1 段口语 + 2 段叙述（compact 1+1），叙述长句占多数样本位；`severe_drift_issue` 阈值收紧到句长 <0.75×基线、口语词 <0.65×基线、比喻 >3/千字且 >1.5×基线（真机 148 4.0、149 3.9 会触发）。③新增 1 条回归测试（第一人称 POV + 动作钩）。

**验证**：Rust **2290 / 17**（+1）；tsc 0；Vitest **732 / 3**（数量不变，Rust-only）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：用 v0.99.19 重跑同一 10 章训练，确认句长漂移收敛、gate 不再因第一人称/动作钩误判、149/150 纪年与玉碟按改进项落地；编审 2 章 unavailable 属本地推理模型解析环境。

### v0.99.18 - 训练「声音」修复：真实章关键台词/转折机制 + 单章 AI 腔/地点同义词

**为什么**：v0.99.17 真机 10 章报告 `5a3fb41e` 证明叙述长句样本生效（144 章句长无报警，145 14.2、147 11.6、148 14.2、149 13.4、150 12.8、152 12.4、153 13.3，上一轮 9.5–13.3），主线探针 9/10 ✓、`reference_gap` 6.5、AI 腔聚合探针未检出；但 editor 仍判「文风逐章稀释，冷硬 noir 与铺陈式明喻加重，市井碎嘴/自嘲下降」，voice 均分 6.85。145 立威丢失「吐烟定住保镖」的术法机制；148/151 关键台词与身体反应套语未被逐章 gate 抓住；146 主线地点「夜总会」被写成「吧台/门厅」触发了不必要的强制修订；risk/emotion 证据词过窄让正确章反复判 revise。

**改动**：①`MainlineBeat` 增 `key_turn`（真实章最大转折，必须写出机制）与 `voice_cues`（真实章关键对白，≤3 条/40 字，truth_hints 教学信号）；`render_beat_block` 注入「本章关键转折」「本章关键对白语感」；`term_matches` 增加场景类型同义词（夜总会→包厢/吧台/门厅/场子/会所，医院→病房/走廊/诊室等）。②`style_probe::ai_ism_issue` 单章统计「咯噔≥2、后脊梁/脊背+发麻/发凉、后槽牙、心里一紧≥2、不是…而是≥2、比喻密度>4/千字」，接入 gate `style_issue`；语体样本改为全章节按得分取最高 2 段口语 + 1 段叙述（compact 1+1），不再只看首/中/尾章。③红线语体边界改为「每章至少 3 处市井碎嘴/语气词/自嘲，比喻每章不超过 3 处，同章身体反应套语不得重复」。④risk 证据词扩到刀/枪/逼/命/死/追/动手/翻脸等，emotion 证据词扩到给/送/赠/护/让/面子/人情等。
**验证**：Rust **2289 / 17**（+4：地点同义词、voice_cues 抽取、单章 AI 腔、risk 证据词）；tsc 0；Vitest **732 / 3**（数量不变，Rust-only）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：用 v0.99.18 重跑同一 10 章训练，确认 voice/style 回升、145 立威机制/148「看我法宝」落地、146 不再因地点名误判修订、151 类身体反应套语减少，且 AI 腔强制修订不拖出 deadline。

### v0.99.17 - 训练文风硬核对：叙述长句样本 + 严重漂移强制修订

**为什么**：v0.99.16 真机 10 章报告 `7b878237` 证明主线修复生效（`reference_gap` 2.9→6.67、主线落地探针 9/10 ✓、AI 腔探针未检出），但文风漂移探针显示 10 章句长 9.5–13.3（原文基线 19.2）、口语词 1.1–1.4/千字（基线 3.3），第 149 章 editor 直接判 style 5「冷硬 noir 化」。根因：`build_exemplar` 只抽口语/对白段落，样本本身句长短，模型越写越短；editor 对多数章 style 7 放行，确定性漂移没有进入 gate。

**改动**：①`style_probe::build_exemplar` 增加「叙述样本」：从上下文章抽平均句长不低于原文基线、对白占比 <25% 的段落（首/中/尾章各一段），与口语样本一起注入（compact 2 段/900 字，full 最多 4 段/1800 字），并明确要求两类都模仿、不能只写短句。②新增 `severe_drift_issue`：句长 <0.65×基线 + 口语词 <0.6×基线（或对白低 20pt、比喻 >2.5×基线）时生成具体问题；coordinator 新增 `style_baseline` 字段与 `with_style_baseline`，gate 新增 `style_issue` 参数——训练中即使 editor 判 pass，只要严重漂移也强制 `RevisionRequired` 一次（editor 已判 revise 时并入 issues），修订直出 prompt 已带原文语体样本。③`evaluate_gate_impl` 的背景/观察/批量调用点同步扩展；普通续写这些参数仍为 None，提示词逐字不变。

**验证**：Rust **2285 / 17**（+2：`severe_drift_issue_flags_short_sentences_and_low_colloquial`、`compact_exemplar_keeps_both_voice_and_narrative_samples`；`style_probe` 共 6 条）；tsc 0；Vitest **732 / 3**（数量不变，Rust-only）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：用 v0.99.17 重跑同一 10 章训练，确认句长/口语词漂移收敛、149 类「冷硬 noir」章减少，且强制修订不把总时长拖出 deadline；连续两轮观察。

### v0.99.16 - 日志巡检：GPU 容量路由单位 / 记忆探针短语 / trace 启动清扫

**为什么**：日志包 `14356f2f`（v0.99.14 / macOS，11-22～11-24）自身窗口 0 条 ERROR，`check_update`×3 与 `scene_conflict`×1 都是 v0.99.13 修复前的旧记录。真实缺陷三处：①11-24 06:49–07:34 连续 8 次 Qwren127 GPU OOM，每次都记「学习 GPU 容量上限」却挡不住下一次更大 prompt——学习存的是服务端 tokenizer 计数（10,275 字符 → 7,346 tokens，中文约 0.71 token/字），候选闸门比较的是 `prompt_chars / 2`（约 5,137），学到的上限（≈6,790）永远高于下一次估算（≈5,690），v0.99.2 的容量路由从未触发；②v0.99.14 两份训练报告的记忆探针列出 `往桌上一/周老师/明天中午/穿灰夹克/一件事/两半灰/三道横/出一点/半根烟/指甲盖/摸出烟`——全是跨词短语；③日志里 6 个 trace 在应用重启后仍为 `running`（4 个最后一步也 running），Tracing 与日志分析无法区分「进行中」与「已中断」。

**改动**：①`HealthRegistry::record_prompt_limit_failure` 的参数改为 `prompt_tokens_est`，统一按客户端估算单位学习；`executor` 在 OOM 处用本请求的 `prompt_chars / 2` 折算，日志同时保留服务端 `prompt_tokens` 便于对账；新增 `estimate_prompt_tokens`（prompt+system chars/2，`estimated_input_tokens>0` 时优先），`prompt_tokens_est` 与候选闸门同口径。②`suspect_new_terms` 新增 `LEADING_NON_NAME_CHARS`（介词/动词/数词量词开头）、时间词与「指甲」停用词、称谓后缀（老师/医生/队长/主任…）、器物收尾（烟/茶/酒/饭/水/菜/汤/药），并把方位/介词并入 `FUNCTION_CHARS`；长串优先与白名单子串规则不变。③`tracing::sweep_interrupted_traces` 在 `TraceStore::new`（State 就绪前、窗口创建前）同步清扫 mtime 超过 30 分钟仍 running 的 trace：关闭未闭合步骤、写 `failed` 与「应用退出或崩溃导致运行中断（启动时清扫遗留 running trace）」；解析/读写失败跳过，阈值避免多实例误伤活运行。

**验证**：Rust **2283 / 17**（+4：客户端估算单位学习（用真机 5,137/8,068/7,459 与下一轮 5,690 锁住必须跳过）、估算含 system_prompt、刚写入 trace 不清扫/陈旧 running 改写且已完成不动、真实报告 11 个短语不进探针）；tsc 0；Vitest **732 / 3**（数量不变，Rust-only）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿；`gitnexus impact` 三个改动符号均 LOW（直接依赖 3/2/1）。**未关闭**：安装后重跑同一 10 章训练，确认 Qwren 大 prompt 只学一次、后续按容量跳过并落到其他候选；报告探针无上述短语；重启后旧 trace 不再 running。Qwren 需 ~9.1GB、本机可用 8.4–8.9GB 属环境容量。

### v0.99.15 - 续写训练原著主线投影 + 原文语体样本常驻（根治离题与风格稀释）

**为什么**：11-24 两轮 10 章训练（报告 `d5f75064` 0.696 / `900d60ef` 0.706）编审连续点名「原著主线被弃：144 章把福摩萨推明日，之后另起老曹—法林寺—三理教平行支线」「风格逐章冷硬 noir，碎嘴自嘲与市井诙谐持续稀释」。复核沙盒：资产重建的故事大纲 `【下一节点】` 是「就地控制并审问伪装护士」——writer 严格按它写；上一轮 `improvement_md` 的真实后续节点只有 1–2 行，优先级远低于故事大纲/章纲。风格侧 `style_dna_applied=true` 但 `force_style_anchor` 为 false，10 章没有原文语体样本；StyleDNA 六维指标无法复刻语感。另有节拍卡沿用上一章 cast（杨晓雯），正确主线的章被「具名阻力未出现」打回修订。

**改动**：①新增 `agency::mainline`：truth_hints 模式用本轮真实后续章逐章摘要（`training_assets::summarize_chapters`，沿用 V163 缓存）生成 `MainlineProjection`（逐章 goal/cast/location/key_terms/must_terms）；`apply_mainline_projection` 写进沙盒故事大纲（删旧下一节点；`condense_story_outline` 将投影块排 core 后、转折点前）；coordinator `with_mainline_projection` 注入章纲 `scene_info`、writer task 首段与资产区；gate `render_gate_block` 注入编辑核对块，`mainline_missing` 非空时即使 editor 放行也强制 `RevisionRequired` 一次；`SceneBeatCard::align_to_mainline` 覆盖 cast/parties/location/next_node。②新增 `agency::style_probe`：原文语体样本（首/中/尾章、对白密集/第一人称碎嘴/口语词段落；compact 2 段/900 字）+ 量化基线（句长/对白/第一人称/口语词/比喻/短句占比）每章注入 writer/修订 prompt；`drift_report`/`ai_ism_report` 输出漂移与 AI 腔；报告新增主线落地/文风漂移/AI 腔三行并把确定性证据追加进下一轮 `improvement_md`。③训练红线扩到 9 条（市井自嘲/反冷硬/反 AI 腔/开章赌注/单一强钩/主线边界）；逐章编审 `reference_gap` 必须点名被推迟/替换的主线节点，`style` 以真实章语体为基线；跨章总评加「平行支线替代主线 / 语体逐章冷硬化」；批量 `premise` 去掉「第N章起」避免 editor 直连 JSON 纠结章号。

**验证**：Rust **2278 / 17**（+11）；tsc 0；Vitest **732 / 3**（数量不变，Rust-only）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。真机 18:17 训练：第 144 章 draft 从医院自然转到福摩萨，最终 scene 含 福摩萨×3/邵卫江×11/路麻子×7/法不可亵×1，editor 门禁直接写「主线事件全部实现：报赵开来之名、直呼邵卫江、包厢围攻、大波浪卷劝阻、法不可亵术不可显…」；145 章同样落地邵卫江/路麻子/汪必成/卓玉花。**未关闭**：完成本轮 10 章报告，确认主线探针逐章 ✓、漂移/AI 腔下降、总分与 reference_gap 回升；确定性修正需连续两轮观察。

### v0.99.14 - 续写训练沙盒章号沿用原书（143 章后起续写 = 第 144 章）

**为什么**：本机训练沙盒 `1122-1829` 的上下文 scene 1–10 对应原书第 134–143 章，续写落库 scene 却是第 11/12 章（自动取名「第十一章 朱砂印与二八车」「第十二章 半盒烟换一句大河村」），10 份报告 `generated_chapter` 全为 11；`seed_scenes` 用 `(i+1)` 丢弃源章号、续写起点固定 `context_n+1`，报告/总评也按内部序号派生，导致「沙盒第 11 章 vs 原书第 144 章」错位。

**改动**：`seed_scenes` 的 `scenes.sequence_number` / `chapters.chapter_number` 改原书真实章号；新增 `sandbox_continuation_start`（续写首章 = 真实后续基准首章 `truths[0]`，缺基准退化为上下文末章 +1）；训练指令、报告区间、编审总评沙盒章号统一用真实章号。报告表格变为「沙盒第 144–153 章 vs 原书第 144–153 章」。

**验证**：Rust **2267 / 17**（+2）；tsc 0；Vitest **732 / 3**（数量不变）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.14 后重跑同一 10 章训练，确认新沙盒第 144 章起、报告 `generated_chapter=144`；旧沙盒不迁移，仅影响新训练。

### v0.99.13 - 日志巡检：切章 CAS 基线竞态 / 更新检查多源裁决

**为什么**：v0.99.10/macOS 日志包 `db37e769`（11-17～11-22）里 27 条 ERROR 集中在两处：①`update_scene` 的 `VALIDATION_FAILED scene_conflict` 10 次，全部发生在切章后 10–50ms，冲突对象始终是**上一章** scene；②`check_update` 12 次 + `install_update` 3 次「无法通过 GitHub 或 storymoss.top 完成更新」，而本机实测 GitHub 源不可达、官网 `latest.json` 正常并明确返回「无更新」。另有 1,403 次空正文冷却跳过（含 137 次「原 1 → after 0」全冷却）与本地 Qwren 连接拒绝 267 次、GPU OOM 54 次、Ds10 首字节超时 30 次——核对代码确认探针（keepalive/pre-call）**不写**冷却计数，空正文全部来自真实调用，v0.99.11 已用「额度放大重试 + 全冷却恢复」覆盖；本地模型连接/容量类属用户环境，只记录不修。

**改动**：①`useScenePersistence` 的 CAS 基线改为 `SceneBaseline{sceneId, content}`：`persistSceneContent` 在入口同步捕获本 scene 基线（不再等 `await prev` 后读共享 ref），跨 scene 保存不发送指纹，成功回写只在本 scene 仍被跟踪时进行；视觉空稿保存直接跳过并记 `frontstage:persist_skip_empty`（后端会忽略正文但仍算命中，旧实现会把基线写成空稿导致之后永久冲突）。`FrontstageApp` 三处基线写入（selectChapter / 懒加载 scene / 建未命名故事）同步带 sceneId。②`updater` 新增 `decide_update_check`：至少一个源明确返回「无更新」时判已是最新并附 `warning`（`CheckUpdateResult.warning` 可选、缺省不序列化），前端 `useUpdater` 只记 warn 日志；所有源都失败才维持整体报错；`install_update` 在没有可用更新时返回准确的「No update available」。

**验证**：Rust **2265 / 17**（+2 净增：多源裁决 3 条 + warning 序列化契约 1 条，替换 2 条旧契约测试）；tsc 0；Vitest **732 / 3**（+4：切章在途保存沿用旧基线、同 scene 仍带指纹、视觉空稿不写库不改基线、跨 scene 不发指纹）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.13 后连续切章写作，确认日志不再出现 `scene_conflict`、上一章末次编辑不再丢失；官网无更新时「检查更新」不再报错（只留 `[Updater] Partial check warning`）。

### v0.99.12 - 续写训练真机复盘：日志保留 / 主角识别 / 修订直出 / 探针收敛

**为什么**：11-22 的 10 章训练（报告 `aa70a919`，总分 0.688 / 承接 9.1）编审 4 pass / 6 revise，但报告写「3 通过 / 0 修订 / 7 章判修订但修订未完成」——`handle_gate` 只看修订 tool_loop 是否正常结束，模型没写黑板也置 `revised=true`；熔断分支直接保留首稿。12:37 重跑 v0.99.11 时单章修订 prompt 约 30k 字符（15k tokens，含工具目录与资产），24 分钟仍未完成。同日重启后 `logs/storymoss.YYYY-MM-DD` 从新进程第一行开始：`cleanup_old_logs` 把 `modified < cutoff` 当成「未过期」继续保留，反而每次启动删掉刚写入的当日 tracing、留下 7 天前旧文件。12:38 沙盒角色模型未返回，确定性 cast 兜底按出场顺序把「大波浪卷女人」建为第一名，旧 roster 直接把第一项标成主角；brief 里「我/主角/周成」并存，编审把主角姓名判成配角。记忆探针仍报 `蓝漆铁/蓝漆铁门`、`黄铜钥/黄铜钥匙`、`邵先生`、`传呼机`、`冷库门`。

**改动**：①`logging::cleanup_old_logs` 修正保留条件（mtime 早于 cutoff 才删），新增真实 mtime 回归测试。②`agency::story_brief` 新增 `protagonist`、`detect_protagonist_name`（只认 `我叫X/我姓X/以X身份/X这个身份/X自报/自称X` 等自称句式，并列/证据不足不标注）、`canonicalize_protagonist`（「我/主角/主人公/本人/阴脉先生」并入权威姓名，合并人物态与 cast 去重）；训练 roster 按识别结果标主角，报告长程记忆行记录识别结果。③`handle_gate` 要求修订稿「与首稿不同、≥300 字、非工具 JSON」才算完成；训练批量跳过 ReAct tool_loop，用 `build_direct_revision_task`（完整正文+逐条问题）直出并写回本章 key，再走第二轮 gate；tool_loop 正常结束但没写黑板时采用最终输出。④`suspect_new_terms` 长串优先收敛重叠碎片，新增称谓后缀与器物收尾字过滤。⑤训练红线新增主角身份、每章至少一次配角互动、禁止「不是…而是…」排比与比喻堆叠。⑥`agency/tool_loop` 给 LeadWriter 设章节草稿写入上限 2 次（zone=draft/item_type=chapter/content≥1200），第二稿成功即采用并结束循环（真机单章 9 稿、7 分钟）；连续 4 轮只读取/查询不写章稿时提前结束交给兜底（真机 writer 12 曾 10 轮全耗在读取）；`evaluate_gate` 的 round-2 改为跟随 `direct_gate_json`（此前硬编码 false，训练复审又回到 ReAct tool_loop）；首稿裁决 pass、修订稿裁决变 revise 时回退首稿（避免更差修订稿装配）；`ensure_story_brief` 读回旧 brief 时补主角识别。

**验证**：Rust **2263 / 17**（+10）；tsc 0；Vitest **728 / 3**（数量不变，Rust-only）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.12 后重跑同一 10 章训练，确认报告「主角：周成」、修订完成数 >0、探针无器物碎片、重启后当日 tracing 保留；直出修订成功率需连续两轮观察。

### v0.99.10 - 日志巡检：质量门文案自相矛盾 + 标题写锁 BUSY_SNAPSHOT + 探针窗口碎片

**为什么**：11-21 本机 v0.99.9 两轮 10 章训练（run `e51f6033` 总分 0.65、run `f8119c97` 总分 0.86 / 承接 9.9 / 编审 10/10）暴露三处应用侧问题：①第二轮报告写「0 章质检降级（编辑审计未完成，已放行保产出）」，降级数为 0 却仍带降级说明；②第一轮 07:01/07:06 两次 `[chapter-title] 自动取名失败 ... [DB_LOCKED] database is locked`，第 12/13 章标题缺失；③记忆探针把白名单长词的窗口碎片（宏达投资→宏达投/达投资）、同一串的重叠窗口（深蓝夹克→深蓝夹/蓝夹克）与方位/虚词短语（条案上/像是有/缕青烟）当成疑似新专名。

**改动**：①`render_gate_line` 只在 `degraded > 0` 时追加「编辑审计未完成，已放行保产出」。②`chapter_title::apply_title` 由 Deferred 改为 `TransactionBehavior::Immediate`：先读后写的 Deferred 事务在并发写入后升级写锁会返回 `SQLITE_BUSY_SNAPSHOT`，该错误不走 busy handler，`busy_timeout=5s` 形同失效；Immediate 先取写锁正常等待（与 `agency/persist.rs`、`lease.rs` 既有做法一致）。③`suspect_new_terms`：白名单长词按子串排除；候选重叠窗口只保留更长串；新增方位字收尾（上下里外边前后）与「是/有/像/声/缕」等虚词字符过滤。

**验证**：Rust **2245 / 17**（+1：白名单碎片/重叠窗口/方位窗口；质量门文案断言扩充）；tsc 0；Vitest **728 / 3**（Rust-only）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.10 后重跑同一 10 章训练，确认训练并发落库时自动取名不再 DB_LOCKED；推理模型空 content（2,377 次）、LeadWriter tool_loop 熔断（22 次）、章节大纲 0 字符（11 次）仍属模型环境，降级/兜底策略不变。

### v0.99.9 - 日志巡检：编审裁决宽容解析 + 诊断维度不再冒充短板 + 记忆探针收敛

**为什么**：11-20 15:40–17:49 本机 10 章训练（报告 `0bbac926`）总分升到 0.70（承接 9.1、编审 10/10 可用、资产/前情提要全部就绪），但仍有 2 章质检降级、5 章判修订未完成。日志抓到模型返回了合法 JSON（`verdict:"pass"`、`suggestions` 为 `[{issue,evidence,impact}]`）却被判「直连 JSON 裁决解析失败」——`EditorVerdict.suggestions` 是 `Vec<String>`，对象数组让 serde 整份裁决失败。报告同时把诊断维度「原书差距 2.1」列进短板、生成悬空句「低于阈值：」并把空修正「原书差距（诊断）（2.1/10）：」注入下一轮；记忆探针把跨词窗口（你脸上/把钥匙/周成把/老头儿）当疑似新专名，线索只回传内部 ID（t1/t2/t3/t4）。

**改动**：①`EditorVerdict` 宽容解析：`suggestions` 接受字符串/对象/单值（对象取 issue→suggestion→text→fix→impact 首个非空字段），`score` 接受数字或数字字符串，`dimension_scores` 忽略 null/非数字项；一个坏字段不再丢弃整份裁决。②新增 `is_corrective_dimension`：`reference_gap` 只保留在五维均值诊断行，不再进入短板、跨章阈值句、下一轮修正指令。③记忆探针：线索未推进输出可读描述（12 字）而非内部 ID；疑似新专名过滤含代词/虚词的跨词窗口，并新增「老头/钥匙/护士/病人/包厢/抽屉」子串停用词。

**验证**：Rust **2244 / 17**（+6：对象 suggestions、字符串分值/null 维度、诊断维度不进改进、报告不列诊断短板、线索可读标签、跨词垃圾专名过滤）；tsc 0；Vitest **728 / 3**（Rust-only）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.9 后重跑同一 10 章训练，确认质检降级 → 0、下一轮改进不再出现空「原书差距」行；编审空 content（推理模型 CoT-only）仍属模型环境，降级保产出策略不变。

### v0.99.8 - 章节标题统一：中文数字解析 +「第二章 铜钱与脚印」+ V164 迁移

**为什么**：本机库与训练沙盒并存四种标题形态：`第 134 章`（旧训练只写占位）、`第12章`（自动取名未完成）、`香炉边的银镯与铜钱`（裸标题）、`第二章 铜钱与脚印`（中文数字）。`parse_chapter_number` 只认阿拉伯数字，中文标题无法落门禁记录；`apply_title` 只写裸标题；幕前 `displayChapterTitle` 对空格占位误当真实标题，对裸标题不补章号。用户要求「统一为『第二章 铜钱与脚印』这样的」。

**改动**：①新增 `src-tauri/src/utils/chapter_title.rs`：`chinese_numeral`（1..99,999,999）、`parse_chapter_title/parse_chapter_number`（阿拉伯/中文/空格/`第12章修订`）、`format_chapter_title`、`source_chapter_title`、`normalize_existing_chapter_title`、`is_generic_chapter_title`、`strip_chapter_prefix`。②`agency::graders::parse_chapter_number`、`db::chapter_split::canonical_chapter_title/is_generic_chapter_title` 全部改用该模块；canonical 改为中文数字。③训练 `seed_scenes` 用 `format_source_chapter_title` 从正文首行恢复原书标题（第134章 魑魅魍魉横行），不再只写「第 134 章」。④自动取名 `apply_title` 统一写「第二章 雨夜叩门」，`name_chapter_context` 返回值/事件/日志同步格式化标题。⑤幕前 `displayChapterTitle` 重写为中文数字格式化：裸标题补章号，源章号（第 134 章）保留，阿拉伯占位（第6章）跟当前章号。⑥新增 V164 迁移 `V164__normalize_chapter_titles`：归一旧库 chapters/scenes.title，占位标题优先从正文首行恢复原书标题，裸标题补章号。

**验证**：Rust **2238 / 17**（+7）；tsc 0；Vitest **728 / 3**（+3）；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装后重启确认 V164 迁移当前库；重跑训练确认沙盒上下文「第一百三十四章 …」、生成章「第十一章 …」。

### v0.99.7 - 续写训练真机复盘：编辑审计直连 JSON / 失败门禁可审计 / deadline 扩容

**为什么**：本机 v0.99.6 实跑《阴脉先生》10 章训练（报告 `66f1456f…`）总分 0.683（承接 8.89、情节 7.0），但生成质量门 **0 通过 / 0 修订 / 9 章质检降级 / 1 章修订未完成**；`agency_editor` 失败 40 次 / 成功 9 次，日志为 `editor tool_loop 熔断（连续解析失败）` 后 `editor 散文回退也失败`，Ds10/deepseek-v4 返回空 content（CoT-only），Qwren 报 GPU OOM。10 章只落 1 行 `editor_gate_records`（第 17 章 revise）——失败分支 `record_gate_impl` 后提前 return，绕过了 `persist_gate_record`。第 17 章起日志变为 `run 级 deadline 触发`：10×240s+120s=2520s，09:45 起算约 10:27 到点，18–20 章被整体熔断。每章进度日志都写「0 字」，实际 scene 正文 1.7–3.1k 字（批量结果 `increment` 恒为空串）。资产聚合虽成功但世界观—/角色 0/关系 0；第 13 对编审两次输出无五维。

**改动**：①训练 coordinator `.with_direct_gate_json(true)`，`GateRunner` 把 `prefer_direct_json` 传进 `evaluate_gate_impl`；新增 `editor_verdict_direct_json`（参照资产内嵌 + JSON mode + reasoning JSON 提取 + 2 次重试，4096 tokens），成功跳过 ReAct tool_loop，失败仍回退旧路径；旧批量/导演默认 false，测试语义不变。②新增 `persist_gate_row` 统一 Passed/RevisionRequired/Failed 落 `editor_gate_records`，失败分支也调用。③`batch_deadline_seconds` 改为 600s/章 + 600s、上限 4h（10 章 6600s）。④`AgencyContinueResult.chapter_chars` 记录实际装配字数，批量进度 sink 改读它；Append/修订/新章三处构造同步。⑤资产 `persist_training_bundle` 提前构建逐章事实；缺 world 时 `deterministic_world_from_brief` 用地点/主线摘要兜底；缺 characters 时 `deterministic_characters_from_cast` 用 grounded cast 兜底；`deterministic_chapter_summary` 裁掉首行章名。⑥编审第三次尝试改 2500 字短采样 + `TaskType::CreativeWriting` + 4096 tokens，降低推理模型 CoT-only 概率。

**验证**：Rust **2231 / 17**（+6：deadline、直连裁决、失败门禁落库、摘要裁章名、世界观/角色兜底）；tsc 0；Vitest **725 / 3**；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.7 后重跑同一 10 章训练，确认编辑审计 10/10 可用/质检降级 0、`editor_gate_records` 10 行、18–20 章不再撞 deadline、进度日志真实字数、世界观/角色非空、编审 unavailable=0。

### v0.99.6 - 日志巡检：批量/续写训练 trace 终态收尾

**为什么**：v0.99.0 日志包（11-19 上传）里，11-17 的 250 步、11-18 的 530 步续写训练 trace 最后一步早已 completed（09:50 / 04:53），但顶层 `status` 在包生成时（11-19 00:25Z）仍为 `running`。v0.96.1 的 `RunTraceGuard::finish` 只接在创世/续写/单角色三个入口，`run_continue_batch_body`（批量续写/导演分批/续写训练共用）启动 trace 后从未收尾。

**改动**：批量路径在 `run_batch_inner` 返回后按结果与取消标记写 trace 终态（completed / failed / cancelled），与 `agency_runs` 同源；状态判定抽成 `batch_trace_terminal_status` 纯函数并 +1 单测锁三态。

**验证**：Rust **2225 / 17**（+1）；tsc 0；Vitest **725 / 3**；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.6 后在「运维 → Tracing」跑一次批量续写，确认新 trace 结束时为 completed/failed/cancelled。

### v0.99.5 - 日志巡检：用量统计幽灵 cost 列 + 功能使用命令未注册

**为什么**：日志巡检拿到 v0.99.0 / macOS 日志包（11-19 上传，41 ERROR / 24 WARN）。其中 11-14 两次 `get_llm_call_stats failed: no such column: cost in SELECT COUNT(*), COALESCE(SUM(total_tokens), 0), COALESCE(SUM(cost), 0.0) FROM llm_calls`、一次 `get_feature_usage_stats failed: Command get_feature_usage_stats not found`。核对代码：`llm_calls` 自 V062 建表与 V089 扩列起从未有 `cost` 列（成本账本 `llm::cost` 只记 token），`get_stats_by_story` / `get_usage_by_purpose` 却都查 `SUM(cost)`；`telemetry::get_feature_usage_stats` 只是普通函数，从未注册成 Tauri 命令，而前端 `services/api/genesis.ts` 的 `getFeatureUsageStats` 一直在 `loggedInvoke('get_feature_usage_stats')`。结果数据洞察「用量统计」全局/本故事/角色用量三张卡每次加载都失败，「功能使用」页签恒空。另发现前端 `getLlmCallStats('global')` 传字面 story_id，后端按 `WHERE story_id='global'` 过滤，全局卡恒为 0（`get_llm_usage_by_role` 已把 'global' 归一为全局）。

**改动**：①`LlmCallRepository::get_stats(Option<&str>)` 只聚合 `COUNT`/`SUM(total_tokens)`，`story_id=None` 跨故事；`get_usage_by_purpose` 去掉 cost 列（返回 `(label,count,tokens)`）；`get_llm_call_stats` 把空串/"global" 归一为 None，`LlmCallStats.total_cost`/`LlmUsageByRole.total_cost` 保留恒 0（IPC 类型不变，等定价表接入再填）。②`commands/anti_ai.rs` 新增 `get_feature_usage_stats` 命令（`days` 默认 30、限幅 1–365）并注册到 `handlers.rs`。③测试 +4：仓库统计回归 2（无 cost 列、故事/全局/跨故事隔离、purpose 分组排序）、telemetry 窗口聚合 1、IPC 注册表契约 1。

**验证**：Rust **2224 / 17**（+4）；tsc 0；Vitest **725 / 3**；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier/IPC manifest 全绿。**未关闭**：安装 v0.99.5 后打开「分析 → 数据洞察」确认用量统计与功能使用页签正常；`check_update` 的 GitHub 网络/清单缺 darwin 平台报错属发布基础设施问题（当前 latest.json 已含 darwin-aarch64/app），本轮未改。

### v0.99.4 - 官网 Release 保留策略 5→3（修复 v0.99.2 552 Disk full）

**为什么**：v0.99.2 tag 构建三平台全部成功，但 `upload-to-website` 传到 macOS 更新包 `.sig` 时报 `552 Disk full`；脚本 retention=5，服务器正好 5 版，清理「nothing to clean up」，官网 latest.json 停在 0.99.1。每版约 660MB、5 版约 3.3GB，本机磁盘长期接近满；v0.98.9 实测 keep=3 可成功。

**改动**：`.github/scripts/upload-releases-ftp.mjs` 默认 `RELEASE_RETENTION_COUNT` 5→3；`.github/workflows/cleanup-releases.yml` `keep` 默认 5→3；AGENTS 保留策略同步改为 3（环境变量仍可覆盖）。**验证**：Cleanup Releases keep=3 已成功释放空间；v0.99.2 上传 job 重跑中；v0.99.3/v0.99.4 构建按新脚本上传后官网 latest.json 应恢复最新版本。**未关闭**：确认官网 latest.json 最终版本与 macOS `.app.tar.gz` 版本化文件名签名校验。

### v0.99.3 - 训练 compact/blind/probes + brief 预览 + 模型 GPU 诊断

**为什么**：v0.99.2 已把资产两阶段/网关容量路由/进度/评分口径落地，但 P1/P2 剩余项仍未完成：本地模型需要 compact prompt；训练页没有 blind 开关；资产阶段没有逐章进度；确定性探针（人物/线索/地点/新专名）未接入报告；摘要缓存没有 TTL；报告详情看不到模型实际收到的《前情提要》；模型页不显示 GPU prompt 上限。

**改动**：①`ContinueAssetsInput.compact` + `ASSET_CHAR_BUDGET_COMPACT=6000`、`BACKSTORY_CHAR_CAP_COMPACT=2200`、`BATCH_LEDGER_CHAR_CAP_COMPACT=600`、`PROGRESS_CHAR_CAP_COMPACT=900`；coordinator `with_compact_prompt`，planner/writer 共用 compact cap；TrainingPayload `prompt_mode`；训练页「紧凑提示词」勾选。②TrainingPayload `feedback_mode=blind` + 训练页「盲测模式」勾选；报告标注 truth_hints/blind。③`rebuild_from_context`/`summarize_chapters` 接收 AssetProgress；训练任务逐章写 assets progress（35→40）+日志+心跳。④`memory_probes_line`：人物留存/线索未推进/地点跳切/疑似新专名（3–4 字重复且上下文/白名单外），报告「记忆探针」行；测试覆盖引魂蛆/骨刺 fixture。⑤V163 摘要缓存加 30 天 TTL。⑥`training_get_story_brief` IPC + TrainingLab 报告详情「前情提要预览」（计数+2400 字）；⑦`ModelHealthReport.prompt_limit_tokens`（GPU OOM 学习值）+ ModelHealthPanel 黄色诊断行。

**验证**：Rust **2220 / 17**（+5：compact 渲染 2、cache TTL 1、memory probes 2）；tsc 0；Vitest **725/3**；Playwright **144/5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：真机复测 v0.99.3（compact prompt Qwren OOM=0、blind A/B、资产逐章进度、brief 预览、模型页 GPU 上限）。

### v0.99.2 - 续写训练资产分阶段 + 网关容量路由 + 进度/评分口径收口

**为什么**：本机 v0.99.1 实跑（报告 `321a0a75`）总分 0.63、承接 7.2（3.1→7.2），但暴露六个系统性问题：①资产重建仍失败——单次 26k prompt + 8192 max_tokens，deepseek-v4 三次 CoT-only（completion 23k，content 空）、Qwren127 GPU OOM（16,781 tokens 需 9,439MB，仅 3,387MB）、Ds10 无内容；brief 只剩 2042 字首尾摘要/1 线索/0 白名单/0 cast；②gateway 147 次 writer 调用 55 次 Qwren OOM/网络失败；③确定性摘要只取首尾 70 字，第 142 章 81% 的「邵卫江/福摩萨夜总会」漏掉；④编审把「与原书后续差距」混进 continuity（16/17/20 承接 1/2/2，实际是沙盒内部承接）；⑤任务 progress 40% 停 45 分钟、heartbeat 停在 14:44；⑥gate 3 章降级/1 章未修订；⑦truth hints 回灌下一轮有基准泄漏风险。

**改动**：①资产重建两阶段：`summarize_chapters` 逐章小 prompt（单章采样 2.6k、JSON mode 1200 tokens、并发 3、V163 摘要缓存）→ `aggregate_brief` 只喂结构化摘要（≤12k chars、8192 tokens）；聚合失败用确定性拼装；逐章摘要/关键术语/cast 兜底写 brief。②`llm/openai.rs` JSON mode 从 reasoning 提取完整 JSON → 补全截断 JSON（取最长合法对象）→ 含 `{` 的 reasoning 交调用方 salvage；资产/聚合 max_tokens 提高。③gateway：解析 GPU OOM `Prompt(N tokens) requires ~XMB … only ~YMB` 学习 `prompt_limit_tokens≈N*Y/X`；`resolve_role_model`/候选打分/候选循环按 prompt 大小跳过；空 content 记 failure（llm_calls success 不再误报）。④`deterministic_chapter_summaries` 改事件句抽取（首句+中段冲突/引号/专名句+尾句），`progress_lines` 复用；brief 从 `key_terms`/cast 兜底 canon/cast。⑤`run_continue_batch` 加 per-chapter progress sink；训练任务每章写 progress/log/heartbeat（40+n*3）。⑥judge：`continuity` 只对上一沙盒章，新增 `reference_gap`（原书差距，诊断）并分列报告；⑦payload `feedback_mode=blind` 过滤「真实后续节点」，报告标注反馈模式。⑧报告新增逐章摘要/缓存/兜底/聚合诊断行。

**验证**：Rust **2215 / 17**（+12：确定性中段摘要/缓存/单对象解析/聚合 prompt/brief roundtrip、GPU OOM prompt-limit/健康档 2、judge reference_gap/blind 过滤 2、batch progress 1）；tsc 0；Vitest **725/3**；Playwright **144/5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.2 后真机重跑 10 章训练，确认资产两阶段成功/缓存命中、brief 10/10 章含中段铺垫、Qwren 不再大 prompt OOM、任务进度逐章推进、report 承接（沙盒）与 reference_gap 分列；blind 模式 A/B 待验证。

### v0.99.1 - 续写训练长程记忆（前情提要 + 本批账本 + 设定白名单全量注入）

**为什么**：v0.99.0 后同一本书 10 章训练仍 0.40（承接 3.1、情节 3.3），编审结论「缺乏长程记忆与逻辑连贯性」。11-17 那轮资产重建成功（10 条逐章梗概都在库里）也只有 0.24/承接 1.2——问题不是资产没生成，而是**生成了没进 prompt**。writer 进度块只读最近 3 章正文末 220 字，第 11 章后 `scene_outcomes` 更把上下文整体替换成最近 3 条 outcome；planner 只读最近 3 条梗概，且 1400 字承接锚点在 3200 字 directive 之外被截掉。11-18 资产重建失败后，`ensure_story_outline` 用「正文头 600 + 尾 1800」+ 场景方法论生成 1794 字「第 134 章单章回顾 + 下一拍规划」，截到 1200 字后以「故事大纲必须围绕此大纲展开」进入 writer——比没有大纲更糟。

**改动**：①新增 `agency/story_brief.rs` 与 V162 `story_briefs`：`StoryBrief`（逐章事实/未结线索/人物当前态/设定白名单），`render` 按优先级截断；`StoryBriefRepository` 落库/清理孤儿。②资产重建 prompt 增加 `open_threads`/`canon_terms` 与逐章 `location/cast/result/open_thread`；落库时同时写 `scenes.outline_content` 与 `StoryBrief`；模型漏返回时用原文首尾窗口确定性兜底；训练执行器再调 `ensure_story_brief` + `ensure_deterministic_outline`，替换被规划文档污染的大纲。③`render_continue_assets` 把前情提要放稳定区最前（红线后、大纲前），protected；有 brief/账本时 `ASSET_CHAR_BUDGET_WITH_MEMORY=9000`；`progress_lines_from_parts` 改为全量逐章梗概（最近 12 章 + 1600 字上限）+ outcomes 补充，不再 `take(3)` 整体替换。④`generate_chapter_outline` 单独注入 `backstory` + 上一章结尾 `prior_tail`；`run_batch_inner` 维护本批账本（gate∥writer 并发时 n-1 在途稿以「刚写定稿」先入账）；编审参照资产同样注入前情提要。⑤训练 coordinator 加 `with_assets_seeded(true)`，`ensure_assets` 跳过伪大纲与 `hot_path_extract_from_prose`；通用 `ensure_story_outline` 改用逐章摘要归纳，模型仍产出规划时用确定性大纲替换。⑥红线 5/6 改为引用前情提要的白名单/未结线索；资产 0 角色时 `build_roster_block` 也始终注入「第一人称不得另起姓名/路人不得命名」。

**验证**：Rust **2203 / 17**（+12：StoryBrief 渲染/截断 3、V162 建表 1、前情提要/账本渲染 3、资产 brief 落库/确定性兜底/伪大纲替换 3、writer 资产块全量注入 1、planner brief+锚点 1）；tsc 0；Vitest **725 / 3**；Playwright 全量复跑（数量不变）；fmt/clippy/guard 全绿。**未关闭**：安装 v0.99.1 后真机重跑《阴脉先生》134–143→144–153 的 10 章训练，确认报告「长程记忆」行显示 10 章全部进入 prompt、承接/情节分回升、无「引魂蛆/骨刺」类新体系名词。

### v0.99.0 - 续写训练「跨章复制/原地踏步」根修（修订兜底跨章取稿 + 训练红线 + 资产兜底）

**为什么**：本机 2026-11-18 12:55 的《阴脉先生》10 章训练报告 0.40（承接 3.1、情节 3.3），编审指出 14–17、20 章「整段复制上一章结尾/原地踏步」，文风漂移到玄幻惊悚，并自创「引魂蛆/骨刺/阴脉起手式」体系。复核 `cinema_ai.db`：scene 13/14、16/17、19/20 正文哈希完全相同；黑板 draft `第11章`–`第20章` 本身是连续链，但落库 scene 依次是 draft12、13、14、14、16、17、17、19、20、20——7/10 章错位，draft11/15/18 从未装配。应用日志实锤 `修订熔断后采用黑板新稿「第12章」` 发生在装配第 11 章时：`handle_gate` 修订轮熔断后按**任意 key** 取黑板最新稿，并行批量里最新稿正是下一章。

**改动**：①`coordinator.rs` 新增 `draft_chapter_number`（严格解析 `第N章`/`第N章修订`，禁止 `第1章` 前缀误吃 `第10章`）与 `latest_draft_for_chapter`（只接受本章 `item_type=chapter` 草稿，排除 outline）；`handle_gate` 熔断兜底与 `revision_draft_with_fallback` 第 2 层都改为同章取稿，并按内容而非 key 判断修订新稿。②训练新增 5 条写作红线（设定边界/语体边界/推进边界/承接边界/设定白名单），放在指令前部（章纲规划前 3200 字可见），承接锚点后再压轴提醒；明确「沙盒里 AI 自造的名词不算既有设定」。③资产重建 JSON 字段顺序改为 `story_outline`/`chapter_outlines` 先于 `world`/`characters`；模型漏逐章梗概时用原文首尾窗口确定性兜底写入 `scenes.outline_content`（报告标注首尾兜底）。④训练读取 scene 后新增跨章复制检测（完全相同或开头 120 字原样来自上一章 → 不可用，不送判官，编审行写明原因）。⑤质量门改为每章单归类（降级 > 修订 > 通过 > 判修订未完成），报告显式「N 章编审判修订但修订未完成（保留首稿装配）」。⑥writer/修订 prompt 增加「一次成稿」纪律（禁止反复 board_read/asset_query），`TextUtils::strip_leading_chapter_heading` 落库前裁掉正文首行章名。

**验证**：Rust **2191 / 17**（+10：跨章熔断兜底回归、章节 key 解析、首尾梗概兜底、资产字段顺序、跨章复制检测、gate 计数、正文首行裁切 4）；tsc 0；Vitest **725 / 3**；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.99.0 后真机重跑同一 10 章训练，确认 scene 11–20 与 draft 11–20 一一对应、无相邻章重复、承接/情节分回升；风格与设定红线需连续两轮观察。

### v0.98.9 - 任务进度与执行日志展示不一致（进度事件同时刷新列表与日志）

**为什么**：真机任务详情头部显示「一次性 · 40% · 心跳正常 执行中」，展开的「执行日志」最新一条却是 12:12:38 的「进度 5% · 正在获取原书章节…」，两者相差 4 分钟；续写训练页状态横幅的「最新日志」同样停在旧进度。查证：任务列表 `useTasks` 每 5 秒轮询，而 `useTaskLogs` 没有任何轮询，只在 App 顶层 `task-status-changed`（完成/失败/取消）时失效；运行中的 `task-progress` 事件只被拆书页 `useBookDeconstruction` 消费，任务页/训练页的日志查询从不刷新，一直显示展开时刻的旧快照。

**改动**：新增 `hooks/useTaskProgressSync.ts` 并在 `App` 顶层挂载——每个 `task-progress` 事件同时 `invalidateQueries(['tasks'])` 与 `invalidateQueries(['task_logs', task_id])`。后端 `update_progress` 本就在同一个 `now` 写 `tasks.progress`、`last_heartbeat_at` 和一条 `task_logs`（v0.93.14），两查询同批刷新后头部进度、更新时间与最新日志一致；事件缺失 `task_id` 时忽略。

**验证**：Vitest **725 / 3**（+2：进度事件同时失效任务列表与对应日志、缺失 `task_id` 不刷新）；tsc 0；Playwright **144 / 5**（全量复跑）；Rust 数量不变（前端-only）；fmt/prettier/guard 全绿。**未关闭**：安装 v0.98.9 后展开运行中的续写训练任务，确认日志随 5% → 20% → 30% → 40% 等阶段实时推进且与头部一致。

### v0.98.8 - 日志自动上传提议链路根修（扫描目录 / 轮转文件名 / JSON 格式三处不匹配）

**为什么**：日志巡检机器人连续多日拿到空的日志包列表（`GET /api/logs/packages` = `[]`，历史 ID 均 404 = 72h 清理正常），本机应用数据目录只有 `log_upload_history.json`（最后一次 2026-11-11 手动上传）而从未出现 `log_upload_state.json`——自动提议链路从未执行到「已提议」这一步。核对代码发现三处与生产布局不匹配：①`maybe_propose_log_upload` 把 `app_data` 根目录传给 `count_recent_errors`，而 tracing 轮转与 `creative_workflow.log` 实际写在 `app_data/logs/`（非递归 `read_dir` 读不到）；②匹配前缀写 `storymoss.log`，而 `tracing_appender::rolling::daily` 生成 `storymoss.YYYY-MM-DD`（点号分隔、无 `.log`）；③生产 tracing 是 JSON（`"level":"ERROR"`），旧代码却按纯文本 `" ERROR "` 匹配。三者叠加 → `error_count` 恒 0 → `should_propose_auto_upload` 恒 false → 后端从不打包/发 `log-upload-requested`，前端弹窗自然不会出现。

**改动**：①新增 `log_scan_dir(app_dir) -> app_data/logs` 作为唯一扫描目录契约，`maybe_propose_log_upload` 改用它；②`count_recent_errors` 识别 `storymoss.*`（daily rolling 与 legacy `storymoss.log*`），按 `"level":"ERROR"` 或 `" ERROR "` 计数（兼容生产 JSON 与开发 compact），`storymoss-startup-trace.log`、`traces/` 与无关文件不计入；③测试改为生产布局回归（`storymoss.YYYY-MM-DD` + JSON、legacy 文本、workflow 三源合计），并新增扫描目录契约守卫。本机 11-17 日志实测：tracing 70 条 ERROR + workflow 30 条 ERROR，旧实现计 0，修复后计 100。

**验证**：Rust **2181 / 17**（+1）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**（首次全量 2 个布局用例并行 flake，单跑 7/7 通过，全量复跑通过）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.98.8 后真机确认启动即弹「上传日志包协助诊断？」且官网列表可见；仅幕前窗口运行时事件无监听者、`last_prompted_at` 已写入 → 提示会延后 24h（已记录改进建议：仅在幕后窗口存活时记账/发送）。

### v0.98.7 - 续写训练报告驱动修复（在途上一章承接 / 质检有效性 / 风格 DNA / 幻觉隔离）

**为什么**：本机 9 份《阴脉先生》训练报告（2026-11-09～11-17）复核发现：加权总分 0.24–0.35，承接 1.2–2.4、情节 2.3–3.1；最新一轮 10 章 8 章编辑审计熔断后「降级放行」、3 章编审输出截断不可用；逐章出现「杨先生/何小满/陈先生/柳小满」等臆造人名并与上一轮真实主线建议矛盾。

**改动**：①批量续写把在途上一章正文/大纲显式传入 writer 与章纲规划（`PriorChapterContext`），修复 gate(n-1)∥writer(n) 并发时 DB latest=n-2 的承接断裂；散文回退同样优先在途正文。②编辑审计 `max_turns` 6→8、输出额度 2048→4096，输入内嵌时直接 final JSON；裁决解析补截断括号；批量 deadline 按章数放大（240s/章、上限 7200s）。③报告区分真实 gate 通过/修订/质检降级与编审判裁 pass/revise/unavailable，修复重复「承接」列。④`render_continue_assets` 注入 StyleDNA/style_slice（此前 style_dna_applied=true 但 prompt 无风格约束）。⑤训练沙盒关闭后台 ingest；指令携带权威人物名册（仅上下文人名）+ 名册外幻觉忽略；上一轮建议只取最近一轮并按上下文接地过滤；从编审 comments 确定性提取「真实后续节点」注入下一轮。⑥逐章编审/总评改 JSON mode + 截断修复。

**验证**：Rust **2180 / 17**（+8）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**（全量复跑）；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.98.7 后真机复跑 10 章训练，确认连续承接、编审 10/10 可用、报告区分质检降级与下一轮分数回升。

### v0.98.6 - docs-guard 重跑比较修复（排除 nightly tag）+ macOS 公证恢复

**为什么**：v0.98.5 在 Apple 公证协议过期后重跑，docs-guard 的 `PREV` 按 `creatordate` 选中了 `v0.98.5-nightly`；它与 `v0.98.5` 指向同一 commit，`nightly..stable` diff 为空，误判 8 份必需文档未更新，导致 tauri-build 全被跳过。

**改动**：`.github/workflows/build.yml` 的 docs-guard `PREV` 改为只选稳定 tag（`^vX.Y.Z$`），排除 nightly/其它后缀；恢复 Apple Developer Program 协议后以 v0.98.6 重新发版。

**验证**：Rust **2172 / 17**（数量不变，CI-only）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**；Swift LegadoCore MCP 29/29。**未关闭**：确认 v0.98.6 tag 构建的 macOS 公证成功、官网 latest.json 同步与 updater 校验完成。

### v0.98.5 - 训练源章节缓存 + 栗子读书 MCP 读竞态服务端修复

**为什么**：v0.98.4 的客户端重试解决了「请求不完整」的偶发中断，但同一本书反复训练仍要重新通过 MCP 取同样的 20 章（重复读取）。同机栗子读书的 HTTP reader 根因也确认了：监听 socket 设置 `O_NONBLOCK`，macOS/BSD 的 `accept()` 会让客户端 socket 继承该标志，header 与 body 分次到达时 `recv` 立即 `EAGAIN` → 400「请求不完整」。

**改动**：①StoryMoss 新增 V161 `continuation_training_chapter_cache`（`server_id + book_ref + chapter_index`，含 content/char_count/fetched_at）；`fetch_chapters` 的 MCP 分支先读缓存，命中跳过调用，未命中取章后回写；任务日志记录命中/新读计数。②栗子读书 v0.14.26：`MCPHTTPServer.configure` 在 accept 后清除 `O_NONBLOCK`，改由 `SO_RCVTIMEO`（180s）阻塞读；新增 `testSplitHeaderAndBodyRequest`（header/body 分离写入延迟 50ms）。③新增 2 条 Rust 测试（V161 幂等、缓存 roundtrip/刷新/跨书隔离）；Swift MCP 测试 29/29。

**验证**：Rust **2172 / 17**（+2）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿；Swift LegadoCore MCP 29/29；栗子读书 v0.14.26 安装后分离写入 1–500ms 实测均 200。**未关闭**：安装 v0.98.5 后真机复跑训练，确认次轮日志「源章节缓存：命中 20 章，新读取 0 章」；缓存暂不自动过期。

### v0.98.4 - Legado 回环 HTTP「请求不完整」重试（续写训练取章竞态）

**为什么**：本机续写训练 2026-11-14 15:26 两次失败，报 `调用 get_chapter_content 失败：传输错误: HTTP 400: {"error":{"code":-32600,"message":"请求不完整"}}`；同一本书有时写到第 136 章、有时第 137 章才失败。实测 Legado MCP 的 HTTP reader 在 TCP accept 后立即读取请求：客户端在 `connect` 与 `write_all` 之间延迟 ≥1ms，body 未到达就被判「请求不完整」，而本机 LLM 高负载时这种调度延迟是偶发的。

**改动**：①`mcp/transport.rs` 新增 `is_request_not_executed_error`：识别 400/-32600「请求不完整」为「JSON-RPC 尚未解析、工具未执行」；回环 `send_raw_http` 对 `tools/call` 也安全重发一次（写工具不会重复副作用），其它方法仍保持原有重试策略。②`mcp/client.rs` 把同类 400 纳入只读工具瞬时故障分类，保留 150/400ms 退避重试；写/隐私工具仍不在客户端层重发。③新增 3 条回归测试（未执行分类、只读重试、写不重发）。

**验证**：Rust **2170 / 17**（+3）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：安装 v0.98.4 后真机复跑该训练任务，确认取章 20 章、编审与报告落库；Legado 端极短读超时的根本行为仍依赖其实现，客户端以重试兜底。

### v0.98.3 - 训练风格 DNA / 资产重建的推理模型 JSON 容错

**为什么**：本机续写训练（2026-11-13 14:21–14:23）出现两条失败：①`style_dna_analysis`（deepseek-v4）返回的 JSON 在 556 字处被截断，报 `JSON 解析失败: EOF while parsing an object at line 5 column 3`；②`continuation_training_assets` 两次调用 `content` 全空、`reasoning_content` 有 7770 字（CoT-only），报「资产重建输出无法解析（已重试）；raw 前 240 字: 」。训练虽继续，但全程无风格 DNA、无世界观/角色/大纲资产，writer 只能走资产缺失兜底。

**改动**：①新增 `utils/json.rs`：`extract_last_json_object`（从思维链取最后一个完整 JSON 对象）与 `complete_truncated_json`（补全未闭合括号、截断键名 `:null`）。②OpenAI 适配器：仅当请求带 JSON mode 时，`content` 为空才从 `reasoning_content` 提取最终 JSON；非结构化请求仍严格不回退 CoT（保持 v0.30.45 语义）。③`StyleAnalyzer::analyze_with_llm` 改 JSON mode + 4096→8192 两次尝试；两次都不完整时采用补全后的部分字段；空壳 `{}`/仅 name 判无效。④`training_assets` 改 JSON mode + 8192；`parse_bundle` 增加截断括号修复、拒绝空 bundle。⑤新增 14 条单测（JSON 修复 7、风格 3、资产 2、OpenAI CoT 提取 2）。

**验证**：Rust **2167 / 17**（+14）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：真机复测同一本书训练的风格 DNA 生成、资产重建成功与报告落库；推理模型仍可能持续 CoT-only，此时风格退化为原文节选锚点、资产退化为空并由 writer 兜底。

### v0.98.2 - agency run 级 token 硬熔断暂停（续写训练 10 章不再中途判死）

**为什么**：本机续写训练（10 章批量）在第 11–12 章时失败，错误为「续写管线失败：编辑审计 Agent 阶段失败: 运行 token 预算耗尽（314545/300000），已熔断」。`run_continue_batch_body` 整批只创建一个 `AgencyBudget(300_000)`，writer/gate/修订/装配全部共用；v0.98.0 起训练一次批量续写 10 章（最多 20 章），固定单章额度在第 2 章前后的 gate 阶段必然耗尽，整条管线判死（已写章节也不出报告）。用户明确要求「暂不需要预算限制」。

**改动**：`AgencyBudget::check()` 改为恒放行（run 级 token 硬熔断暂停）；`record_usage` 与 `tokens_used()` 记账保留，run 检查点/成本观测继续可见真实消耗。恢复预算限制时只需在 `check()` 内按 `token_budget` 比较，调用方无需改动。同步把 `test_budget_exhaustion_blocks_call` 反转为 `test_budget_exhaustion_does_not_block_call`（超额度后仍成功且继续记账）。

**验证**：Rust **2153 / 17**（测试数不变，语义反转 1 条）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：真机复测 10 章训练完成、逐章编审与报告落库；run 检查点中的 `tokens_used` 仍按真实消耗增长。

### v0.98.1 - 本地书籍训练源取章起点修复

**为什么**：`fetch_chapters` 的 `file` 分支把 1-based `start_chapter` 直接当 0-based 切片索引用（`parsed.chapters[start..end]`），「从第 10 章连取」实际从第 11 章取，跳过指定章并整体错一位；影响本地 epub/txt 训练。

**改动**：新增 `ContinuationTrainingExecutor::file_slice_bounds(start, total_want, total)` 做 `start-1` 转换与边界收敛；越界返回可操作错误；章号标签改 `start + i`。MCP 源与默认第 1 章起不受影响。新增回归测试覆盖 1 / 10 / 95 / 越界。

**验证**：Rust **2153 / 17**（+1）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：真机复测本地书籍源 / MCP 源的完整 10 章训练链路。

### v0.98.0 - 续写训练多章化 + 编审 Agent 全面对比 + 报告驱动优化

**为什么**：训练此前只写 1 章、只对比 1 章，无法验证「连续续写」的人物弧/线索经营/风格漂移；用户要求续写 10 章，并调用编审 agent 对照原小说后 10 章做全面对比分析、写续写质量报告，再据此优化续写能力。

**改动**：①`TrainingPayload` 拆 `chapter_count`（上下文，默认 10，2–30）与 `continuation_chapters`（续写，默认 10，1–20）；取章 = 上下文 N + 原书真实基准 M，沙盒灌 N 章后一次批量续写 M 章。②新增逐章编审：每对「沙盒续写章 vs 原书后续真实章」用 `agency_editor_auditor_system`（编审 agent，PromptRegistry 支持 agent.md 覆盖）输出五维 + continuity + verdict；两次重试不可用只降级该章（不落假 0、不判死）。③新增跨章总评（走势/一致性/与真实后续差距/可执行改进）与确定性《续写质量分析报告》（区间、五维均值、逐章表、跨章对比、强项/短板/评语/改进/流程）；总评不可用时降级为逐章汇总。④`load_training_feedback` 读同书同区间上一轮报告的维度均分，`correction_block` 把 <6.0/10 的维度落成本轮硬性修正（承接 900→1400 字锚点、即使 DNA 已绑定也强制风格锚点、情节/口吻/节奏/钩子要求）；改进项写入 `improvement_md` 自动注入下一轮。⑤V160 报告表加 `continuation_chapters`(默认1)/`truth_chapters_json`/`chapter_reviews_json`；命令与训练页补续写章数配置、报告列表与逐章对比表。⑥判官解析扩 `verdict`/`continuity`；`context_slice` 语义改为「后 truth_n 位是基准章」（execute 直接切片，锚点函数只吃上下文）。

**验证**：Rust **2152 / 17**（+7）；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：真机复测完整 10 章训练（逐章编审/跨章总评/报告落库/下一轮修正生效）；长批量续写受模型配额与网关稳定性影响。

### v0.97.0 - Gate 单一权威 + 导航/页面单一 registry

**为什么**：v0.96.0 留了两项债务——run.result.verdict、黑板 gate 条目、editor_gate_records 仍是三副本；Sidebar NAV_GROUPS、renderView switch、FULL_HEIGHT_VIEWS 仍是三份视图清单。

**改动**：①V159 给 `editor_gate_records` 加 `run_id`；`persist_gate_record` 在 `evaluate_gate_impl` 内 await 落库；新增 `apply_gate_records_to_run`，QC 早/晚于 finish 都能幂等回填 run.result（batch 逐章）。②`navigation/views.ts` 统一 NAV_GROUPS/label/icon/impact/fullHeight，FULL_HEIGHT 从分组派生；`navigation/viewComponents.tsx` 用 `Record<ViewType, ComponentType>` 穷举页面；App 删除 switch，Sidebar 迁出 NAV 并兼容导出。③新增 Gate 权威三向测试 + 导航清单测试。

**验证**：Rust **2145 / 17**；tsc 0；Vitest **723 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：真机复测 Gate verdict 回填（batch QC 晚到）、导航 registry 全页点击；V159 存量库迁移需执行一次。

### v0.96.1 - 日志巡检修复：trace 终态收尾 + update_scene 错误语义化

**为什么**：日志分析机器人首轮真实日志包（v0.95.0 / macOS / 2026-11-11）发现：①12 个 traces 中 8 个卡 `status=running`（含已取消/已报错序列）——`RunTraceGuard` 从不调 `finish_trace`，v0.86.0 起的观测缺陷；②两次 `update_scene failed: Query returned no rows`（Fatal）把 v0.30.50 防丢稿的明确拒绝暴露成内部 SQL 语义，用户无从操作。

**改动**：①`RunTraceGuard` 携带 TraceStore + `finish(status, error)`；genesis/continue 结果后按 Ok→completed / Err+cancel→cancelled / Err→failed 收尾，role 任务 completed/failed（取消语义仍由 finish_run 兜底）。②`scene_commands::map_scene_update_error`：`QueryReturnedNoRows` → validation_failed「目标章节/场景不存在（可能已被删除或在其它窗口被替换），请刷新目录后重试」，其余错误保持映射。

**验证**：Rust **2141 / 15**（+2）；clippy/check/fmt/guard 全绿；前端零改动。

**未关闭**：Tracing 面板真机复验；环境类信号（本地模型 CoT-only 探测、端点连接失败）不修，见 `.recovery/logbot-reports/2026-11-11.md`。

### v0.96.0 - 未尽状态机债务收口（租约/DB 守卫/CAS/入口互锁/死模块）

**为什么**：v0.95.0 还剩租约 TTL、DB 无 CHECK、保存无 CAS、智能入口锁不完整、auth/窗口/automation/叙事阶段/视图清单等债务。

**改动**：①`LeaseGuard` 后台续期 + Drop 释放；`acquire` 前置全局过期回收。②agency 子任务依赖等待落 `waiting`，`dispatch_sub_agent` 前置 `reap_stale`。③V158 迁移清理历史未知状态并为 6 张状态表建 INSERT/UPDATE 触发器。④`update_scene` 新增 `expected_content_hash`（FNV-1a 64，Rust/TS 同构），正文 CAS 阻止幕前旧稿覆盖幕后新稿。⑤Request/Smart 共享入口 ref 锁，生成开始后释放。⑥`TrainingStage` 8 阶段显式枚举。⑦删除 auth/oauth、auth/session；窗口 `WindowVisibility` + toggle 锁；automation `AutomationLifecycle` + 单循环；Falling 叙事阶段可达；`navigation/views.ts` 单源布局。

**验证**：Rust **2139 / 17**；tsc 0；Vitest **720 / 3**；Playwright **144 / 5**；fmt/clippy/guard/prettier 全绿。**未关闭**：Gate verdict 三副本、Sidebar 分组与 renderView switch 仍未完全单源；真机复测租约/CAS/入口互锁。

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

This project is indexed by GitNexus as **StoryMoss** (32121 symbols, 66795 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

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
