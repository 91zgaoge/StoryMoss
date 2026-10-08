# StoryMoss Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryMoss (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryMoss`
- **版本**: v0.65.1
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
- `cargo test -p storymoss` ✅ 1778 passed / 5 ignored（迁移治理 / 级联 / 取消传播 / 质检闭环 / 提示词资产 / 导出加固 / 网关故障注入 / golden harness / JSON 尾随逗号 / 知识边界·物品归属·级联影响 / 分层摘要 / 文本质检·文风学习·成本哨兵 / 质量债·时间旅行·指南针·待确认·三把尺子 / 投影路由 / 关系不变量 / 段落收尾符 / 生死状态 / 人类文笔基线）
- `npx tsc --noEmit` ✅
- `npx vitest run` ✅ 609 passed / 3 skipped（+2 物料重算页签）
- `npx playwright test` ✅ 39 passed / 5 skipped（新增幕前续写 spec 3 用例；门禁仍非阻塞，见未关闭）
- `cargo +nightly fmt` ✅
- `cargo clippy` ✅ 0 error（v0.64.0 起纳入每版验证：CI 用不带 -D warnings 的 cargo clippy，deny 级 lint 会阻塞发布）
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅
- `src-server` ⚠️ 无本地 PostgreSQL 无法编译（sqlx 宏），仅以独立提取的单测验证纯函数

## 最近完成的功能

> v0.30.26–v0.54.0 的逐版本摘要已移入 `docs/archive/AGENTS_HISTORY.md`（v0.59.0 瘦身：根文件只保留最近 5 个版本与关键教训）。

### v0.65.1 - v0.65.0 收尾（叙事架构层 + 改写纪律）

**v0.65.0 的 tag 先推、构建已启动**（安装包内容与当时的 master 一致），按「不覆盖已有 tag」的规则单独发版本补齐余下改动。**叙事架构层**（sepia 三层协议最深、也最易被识别的一层）写进 `agency_outline_planner_system`：回声测试（同一前提重生成二十次还会出现的转折 = 机器手笔，换成需要本故事特有人物与细节才成立的转折）、允许因果松动一次、不要预支答案（最要紧的信息后置揭示）、不做「主角理解+接受+成长」的收束、关系网稀疏、主题不直说、至多加一条与主线斜向呼应的次要线索。**改写纪律**写进 `writer_rewrite`：删优于加（能删就删、能换就换），改完不得比原文更长/更华丽，不得把平直命名改成身体反应或把「说」改成低语/咕哝/嗤笑，保留原文具体细节（名字/物件/数目）。

- **验证**：`cargo test --lib` 1778 passed / 5 ignored（+1，v0.65.0 基线 1777）；fmt / 格式守卫通过。
- **契约**：`test_v0650_human_voice_doctrine_in_prompts`（六个写作/规划/审查提示词必须分别携带人类文笔基线要素，防后续编辑悄悄改掉）。
- **未关闭**：同 v0.65.0（叙事架构层无确定性检查；真机端到端未复跑）。

### v0.65.0 - 人类文笔基线（sepia / StoryScope 方法落地）

**为什么**：主创的文笔「明显是 AI 写的」不是词句问题，而是**分布偏移**——把公认的 AI 特征反向拉满会造出新的机器感（sepia：时间非线性人类中位 2.4/5，不是 5/5；中文人类语料 30% 的文章含「不是…而是」，单例是正常语域）。**做法**：`story_system::human_voice`（纯函数，0 LLM）只收**跨语言/跨模型世代方向一致**的形态——句长离散度（唯一方向一致的句法指标）、语气词缺失（中文人类密度 5 倍）、情绪具身化独大（机器 81% 用身体反应承载情绪，人类 38%，人类反而更常直说「她害怕」）、连词堆叠、双音节填充、花式对话标签轮换、抽象包装、段落过匀；**方向矛盾项一律不入规则**（标点密度、平均句长、段落数、词汇多样性）。写作端 `agency::beat_card::render_writer_user_prompt` 注入 7 条固定准则 + **8 条手法池按已写篇幅轮换**（Select, don't accumulate：多样性来自每篇选 3–5 个手法，不是堆满；置于末句锚点之前，锚点保持最后的高显著位）；`render_writer_system_from_bundle` 追加常驻准则（用户覆盖模板也生效）；editor 资产加【人类文笔基线】块 + 诊断纪律（按两组逐条核查——一次通读只看得到最扎眼的一两项；每条阻断引用一处原文；白名单：语法干净/单个破折号/单个「不是…而是」/正式语域都不是 AI 证据；修正优先删换其次加）；commit 把发现并入 `review_result`。**校准既有模块**：`prose_lint` 的「不是…而是」改聚集（≥2）才报；`anti_ai` 三处方向错误的判定修正（平直命名情绪不是缺陷、重复「说」是常态、感官密度低不是 AI 指纹），并修复对话提取只认弯引号导致「」体例下整块漏检。

- **验证**：`cargo test --lib` 1778 passed / 5 ignored（+15）；vitest 609 / 3 skipped；clippy 0 error；fmt / prettier / guard / tsc 全绿。**验收探针**（ignored 手动跑）：机器腔样本命中 5 条（embodied-only / fancy-speech-tags / connective-stack / disyllabic-padding / abstract-wrapper），人类腔样本 **0 误报**；句长 SD 6.24 对 14.97、语气词 0 对 4、平直命名 0 对 2。
- **契约**：`flat_rhythm_flags_uniform_sentence_run_but_not_varied_prose`；`embodied_only_is_a_deficit_not_plain_naming`；`mood_particle_absence_only_with_dialogue`；`fancy_tags_flagged_only_when_plain_said_is_absent`；`connective_stack_and_disyllabic_padding_are_detected`；`abstract_wrapper_needs_cluster`；`clean_literary_text_produces_no_findings`；`technique_rotates_by_chapter_and_guidance_is_compact`；`audit_block_is_bounded_and_empty_when_clean`；`writer_prompt_carries_human_voice_guidance_before_ending_anchor`；`plain_emotion_naming_is_not_flagged_but_embodied_only_is`；`repeated_plain_speech_tag_is_not_flagged_but_fancy_rotation_is`；`sentence_rhythm_uses_dispersion_not_mean_length`；`flags_not_x_but_y_only_when_clustered`；`test_v0650_human_voice_doctrine_in_prompts`。
- **未关闭**：叙事架构层（主题不解释 / 结局非「主角选择+接纳+成长」三脚架 / 配角不互识）只落在提示词与审查清单，无确定性检查（需跨章语义，确定性规则做不了，单次 LLM 自评天然盲——sepia 实测自评塌缩到一两个维度，留待分组多次诊断）；真机端到端未复跑（本机库无章节正文），**不得宣称续写质量已修复**。

### v0.64.12 - 自动后台重算 + 指纹感知

**为什么**：v0.64.11 的手动重算会把「第 N 章以后每一章」都重算（长书=几十次调用），自动跑不可接受。**修复**：**V145** `scene_commits.summary_source_hash`（正文剥标记后的 FNV-1a 指纹，`auto_commit` 写）+ `recompute_scoped(summary_from, material_from, cap)` 指纹感知（只重算正文变过的章；段摘要只重建摘要变过的段）；`recompute_after_commit`（章节摘要从 committed+1、材料从 committed）；`spawn_auto_recompute_if_stale` 挂在 `schedule_commit_and_split` 的 auto_commit 成功分支——后台闸门 + 同故事并发去重 + 单轮 12 章上限（超出保留标记续算）+ `AppConfig::auto_recompute_after_edit`（默认开）开关；`should_auto_recompute` 判定「有失效且起点 ≤ 刚提交章」。

- **验证**：`cargo test --lib` 1764 passed / 4 ignored（+5）；vitest 609 / 3 skipped；clippy / fmt / prettier / guard / tsc 全绿。**真机探针**（库副本）：`mark_stale(9)` → 一轮重算（4 条重写/段摘要待模型/1 条快照）→ **二轮 0 条重写、4 条未变**；`should_auto_recompute(12)` = `Some(9)`。
- **契约**：`fingerprint_ignores_markup_but_detects_prose_change`；`recompute_skips_chapters_whose_prose_unchanged`；`recompute_cap_defers_rest_and_keeps_flag`；`should_auto_recompute_only_when_stale_at_or_before_committed`；`v145_adds_hash_column_idempotently`。
- **未关闭**：自动重算只在提交防抖后触发；分层摘要仍需可用模型。

### v0.64.11 - 物料失效重算 + 关系归一 + 摘要失败可见

**三件（真机问答落点）**：①改旧章后跨章物料无声漂移；②关系类型 46 行 28 种写法；③段摘要 0 行/第 9 章摘要为空都没人知道。**修复**：**V143** `story_material_staleness`（编辑正文即记「自第 N 章失效」）+ `story_system::recompute`（章节摘要按当前正文重算、分层摘要与全书纲要删旧重建、快照重写、关系失去支撑审计）；运行维护页新增「物料重算」页签 + 命令 `list_stale_materials`/`recompute_story_material`。**V144** 关系归一列（`relation_kind` 受控词表 + `relation_flags` 位标志，`db::relation_kind::RelationClass` 纯函数分类）并接消费点（冲突阶梯敌意判定、关系不变量血亲/配偶判定——此前漏判「翁媳/敌对」「夫妻（名分）／仇敌」）；`story_system::relation_retract` 按 `kg_relations.evidence`（`chapter:<故事>:<n>`/`scene:<id>`/`agency:scene:<id>`）摘证据、摘空即删行，编辑路径已接入。`chapter_summary::summarize_chapter_with_quality` + `SegmentRefreshReport`：所有回退/失败路径入质量债。

- **验证**：`cargo test --lib` 1759 passed / 4 ignored（+14）；vitest 609 / 3 skipped（+2）；clippy / fmt / prettier / guard / tsc 全绿。**真机探针**（库副本）：46 行关系全归一（11 种复合写法正确）、失去支撑审计 0 行、`mark_stale(9)` → 重算（4 条章节摘要重写 / 段摘要待模型 / 1 条快照重写）。
- **契约**：`quality_reports_debt_for_every_fallback_path`；`refresh_report_debt_details_cover_all_failure_paths`；`summarize_chapter_quality_marks_empty_and_no_llm`；`mark_stale_keeps_earliest_chapter_per_kind`；`recompute_rewrites_chapter_summaries_and_clears_flags`；`recompute_deletes_stale_segments_only_when_llm_available`；`v143_creates_staleness_table_idempotently`；`classifies_real_machine_compound_types`；`hostile_flag_matches_legacy_keyword_scan_for_old_cases`；`flags_roundtrip_and_kind_vocabulary`；`v144_backfills_kind_and_flags_idempotently`；`retract_drops_rows_whose_only_evidence_is_the_edited_chapter`；`retract_clears_evidence_but_keeps_multi_evidence_rows`；`audit_reports_only_relations_without_prose_support`。
- **未关闭**：分层摘要重算需可用模型（否则保留旧值并提示待模型）；自动后台重算未做（编辑只记零成本失效标记）；手工关系的失去支撑只报告不自动删。

### v0.64.10 - 冲突升级/衰减（同一对峙不得连拍复述）

**真机**：`compile_conflict` 每拍从静态敌意关系返回同一句「加压：甲 与 乙 正面对峙」，模型因此每拍都写同一场对峙（v0.64.9 修掉自注入后剩下的最后一条）。**修复**：`ConflictStage` 四阶梯（加压→升级→结账→余波），位置从**上一拍卡块**的 `冲突：` 行关键词推进；新增 `compile_beat_card_located_prev` 并接线 Append / observe / 批量三条路径；上一对写到余波则轮换到别的对峙对；探针两条缺口入质量债——「冲突未升级」（要求升级/结账却只再对峙一次，`conflict_outcome_landed` 检查代价/不可逆结果）与「冲突原地复述」（涉事句子与前文 bigram 相似度 ≥0.62，新增 `TextUtils::char_bigram_similarity`）；必须改变项随阶梯（Risk/Relationship/Goal）。

- **验证**：`cargo test --lib` 1745 passed / 4 ignored（+7）；vitest 607 / 3 skipped；clippy / fmt / prettier / guard / tsc 全绿。**真机探针**（库副本）：真机第 10 章正文连编译两拍 = Press → Escalate，文本不同、阶梯推进。
- **契约**：`conflict_ladder_advances_and_never_repeats_consecutive_line`；`previous_conflict_line_reads_card_block`；`conflict_line_rungs_are_distinct`；`escalate_stage_flags_confrontation_without_outcome`；`press_stage_does_not_demand_outcome`；`conflict_repeat_against_prior_tail_is_flagged`；`char_bigram_similarity_flags_near_repeat_but_not_new_scene`。
- **未关闭**：阶梯状态存在卡块里（一拍一格），作者手删对峙不会自动回退；已写的第 10–13 章重演段落仍在正文（需作者重写）。

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
