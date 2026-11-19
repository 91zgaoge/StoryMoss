# v0.99.1 续写训练实跑复盘与优化实施计划

- 日期：2026-11-19
- 实跑任务：`a2090aa1-2592-42ae-bfa7-e97356889ace`
- 训练沙盒：`[训练]《阴脉先生》·1119-1438`（story `5c29e08b-0934-4ba7-8e73-dce6480f3ba7`）
- 报告：`321a0a75-dad5-46c2-927c-e556eed1d0c4`
- 代码基线：v0.99.2（P0-1～P0-5 与 P1 核心项已落地：资产分阶段/V163 摘要缓存、JSON mode 截断补全、GPU prompt-limit 路由、确定性中段摘要、批内 progress、continuity/reference_gap 分列、blind 反馈模式）

---

## 0. 结论摘要

1. **v0.99.1 的“长程记忆”结构有效**：承接从上一轮 3.1 → **7.2**，加权总分 0.40 → **0.63**；文笔 7.3、口吻 6.6、节奏 6.2、钩子 6.9；前五章承接 10/10/9/10/10，且未出现「引魂蛆/骨刺/阴脉起手式」等新体系名词。`continue_assets` 日志证实 `brief=10`、`ledger=0..9` 已真实注入。
2. **但这轮暴露了 6 个系统性问题**，其中最关键的不是 writer 的写作能力，而是：
   - **资产重建失败 → brief 质量塌方 → writer 缺人物卡/白名单/逐章语义摘要**；
   - **本地模型网关对超大提示词的容量判断错误 → Qwren127 反复 GPU OOM → deepseek-v4 CoT-only 空正文 → 多次候选空转**；
   - **编审评分把“原书后续差距”混进“承接”，导致 16/17/20 承接被误判为 1/2/2**；
   - **训练阶段进度/心跳停在 40% 45 分钟**；
   - **质量门 3 章降级/1 章修订未完成**；
   - **truth hint 回灌下一轮，训练闭环存在基准泄漏风险**。
3. 下一轮不应只继续“加 prompt”，应做 **资产摘要分阶段化 + 网关容量路由 + 评分口径分离 + 进度可观测** 四件 P0 事。

---

## 1. 本次实跑数据

### 1.1 时间线

| 时间 | 事件 |
|---|---|
| 14:38:46 | 任务开始，取 20 章（缓存命中 20） |
| 14:38:54 | 风格 DNA 绑定完成 |
| 14:40:19–14:44:39 | 资产重建 4 次候选调用（deepseek-v4 ×3、Qwren127 ×1 失败、Ds10 ×1），最终失败 |
| 14:44:39 | 前情提要确定性兜底：10 章 / 1 线索 / 0 白名单 / 0 人物态 / 2042 字 |
| 14:52:05–15:29:17 | 沙盒第 11–20 章落库（10 章，共 53 分钟） |
| 15:29:33–15:32:01 | 逐章编审对比 10 对 + 跨章总评 |
| 15:32:18 | 报告落库，任务 completed |

### 1.2 报告分数（对比上一轮）

| 指标 | 11-18（v0.99.0） | 11-19（v0.99.1） | 变化 |
|---|---|---|---|
| 加权总分 | 0.40 | **0.63** | +0.23 |
| 承接 | 3.1 | **7.2** | +4.1 |
| 情节推进 | 3.3 | **5.3** | +2.0 |
| 文笔风格 | 5.6 | **7.3** | +1.7 |
| 人物口吻 | 4.9 | **6.6** | +1.7 |
| 节奏密度 | 4.3 | **6.2** | +1.9 |
| 钩子悬念 | 5.1 | **6.9** | +1.8 |
| 生成质量门 | 3 pass / 0 rev / 2 degraded | 6 pass / 0 rev / 3 degraded / 1 unrevised | — |

逐章承接：`10, 10, 9, 10, 10, 1, 2, 10, 8, 2`。

### 1.3 证据：brief 已注入，但质量弱

应用日志（本地 14:44:39）：

```text
前情提要就绪：10 章逐章事实｜1 条未结线索｜0 项设定白名单｜0 人人物态｜2042 字
│已替换被规划文档污染的故事大纲
资产重建失败（继续训练）：资产重建输出无法解析（已重试）；raw 前 240 字:
```

`continue_assets` 日志：

```text
continue_assets: admitted=主角 l2=主角 roster= chars=9000 truncated=true brief=10 ledger=0
continue_assets: ... brief=10 ledger=4
...
continue_assets: ... chars=9000 truncated=true brief=10 ledger=9
```

说明：
- `brief=10` 与 `ledger=n` 结构已在 writer 资产块中生效；
- 但 `admitted=主角`、`roster=-`、`chars=9000 truncated=true`：没有人物卡/名册，资产块仍被 9000 字上限截断；
- 第 142 章的「福摩萨夜总会/邵卫江」没有进入 2042 字 brief（brief 是首尾 70 字确定性摘要）；
- 生成第 11–20 章 `邵卫江 / 福摩萨 / 夜总会` 出现 **0 次**；`引魂蛆/骨刺/阴脉起手式` **0 次**；`小护士` 3 次。

---

## 2. 问题清单（按严重度）

### P0-1 资产重建：单次巨型 JSON + 推理模型 CoT-only + 本地 GPU OOM

**证据**：

- 资产 prompt 约 25,758 字符（~12,879 tokens），调用 `generate_for_task_with_format(..., max_tokens=8192, JsonObject)`；
- `llm_calls`：
  - deepseek-v4 3 次，completion **23,877 / 23,921 / 23,914** tokens，content 为空；
  - Qwren127 1 次，`Prompt (16781 tokens) requires ~9439MB GPU memory but only ~3387MB available`；
  - Ds10 1 次，completion 23,914，仍无可用 JSON；
- 结果：`rebuild_from_context` 两次尝试均 parse 失败，`persist_training_bundle` 没有落任何 LLM 语义摘要，brief 走确定性首尾兜底。

**根因**：

1. 一次请求同时做「10 章归纳 + 世界观 + 角色 + 关系 + 章梗概 + 线索 + 白名单」；输出 JSON 巨大，推理模型把预算烧在 reasoning；
2. `max_tokens=8192` 对 23k completion 的推理模型不够，`finish_reason=length` 时 content 为空；
3. 本地 Qwren127 实际 GPU 上限约 4–6k tokens（4,275 tokens 已经 OOM 一次：4,924MB vs 4,590MB），却被用于 12.9k–16.8k tokens prompt；
4. JSON mode 的 reasoning→JSON 提取只尝试“最后一个完整 JSON”，对截断的 JSON 没有补全/二次提取，content 空即失败。

**影响**：brief 只有首尾 70 字 + 0 白名单/0 人物态；writer 没有人物卡/名册，第 142 章中段铺垫缺失，模型只能自造新悬疑线（铁盒/赤脚印/檀香道姑）。

---

### P0-2 网关容量路由错误：本地模型反复 OOM，候选空转

**证据**（`llm_calls` 14:44–15:32）：

| purpose | model | calls | success | fails | 主要错误 |
|---|---|---|---|---|---|
| agency_writer | Qwren127 | 74 | 19 | **55** | GPU OOM / network |
| agency_writer | deepseek-v4 | 69 | 69 | 0 | — |
| agency_editor | Qwren127 | 35 | 19 | **16** | GPU OOM / network |
| agency_editor | deepseek-v4 | 32 | 32 | 0 | — |
| agency_producer | Qwren127 | 25 | 10 | **15** | GPU OOM / network |

典型错误：

```text
Prompt (13240 tokens) requires ~9223MB GPU memory but only ~3093MB is available
Prompt (4275 tokens) requires ~4924MB GPU memory but only ~4590MB is available
```

**根因**：
- `candidate_fits_prompt` 只看模型声明的 `max_context_length`，没有看实际 GPU 显存预算；
- 用户指定的 Creative 模型在失败后仍被反复置顶/尝试；
- `record_gateway_failure` 标 Degraded，但没有按「prompt 大小」记住容量上限；
- empty-content 冷却只对空正文生效，GPU OOM 没有等价的“大 prompt 冷却”。

**影响**：147 次 writer 调用中 55 次失败（37%），10 章续写总时长 53 分钟；资产重建失败直接由 Qwren OOM 与 deepseek CoT-only 叠加导致。

---

### P0-3 brief 质量塌方：确定性摘要是首尾 70 字，不是事件摘要

**证据**：
- `deterministic_chapter_summaries` 对每章取开头 70 字 + 结尾 70 字（`training_assets.rs:762`）；
- 第 142 章「邵卫江」在正文 1892/2325（81%），首尾窗口都没有；
- `progress_lines_from_parts` 在没有 `outline_content` 时取正文末 180 字（`coordinator.rs:10342`），同样漏中段；
- brief 只有 1 条通用 main thread，0 白名单/0 cast；
- `continue_assets` 的 `roster=-` / `admitted=主角`：没有人物卡可用。

**影响**：即使结构上 brief 注入，writer 也缺中段铺垫、人物关系和设定边界；模型另起「收据—寄存处—赤脚印—檀香道姑」线，与前五章形成的医院现场脱节。

---

### P0-4 评估口径错误：原书后续差距被当成“承接断裂”

**证据**：逐章承接 `10,10,9,10,10,1,2,10,8,2`；报告对 16/17/20 写“完全无视基准章/承接断裂”，而这几章其实紧接了自己的上一章（沙盒第 15/16/19 章）。

`review_prompt`（`continuation_training.rs:1777`）同时给出：
- `【上一章结尾（承接基准；只用于判断续写是否紧接该现场，不是评分基准）】`
- `【原作者的后续真实章（基准）】`
但 continuity 指令又说“AI 续写是否紧接上一章结尾”，而 truth 被标为“基准”，模型容易把“与 truth 的差异”写进 continuity，造成 16/17/20 低分。

**影响**：
- 报告把“与原书后续不同”误读为“承接断裂”，下一轮改进项也围绕“基准章此刻人在哪”；
- 对真实续写（没有 truth 可看）不公平；plot 分数被 truth 差异污染（5.3）；
- 无法区分“沙盒内部连贯”与“与原书后续距离”。

**修正口径**：
- `continuity` 只相对**上一沙盒章**；
- 新增 `reference_gap`（原书后续差距，1–10，越高越接近）作为诊断，不混入承接；
- `plot` 只评本章内部状态变化与推进，不以 truth 匹配度扣分；
- 报告列改为「承接（沙盒）」+「原书差距」。

---

### P0-5 训练进度/心跳不可见：40% 停了 45 分钟

**证据**：
- 任务 `progress=40` 从 14:44:39 一直到 15:29:33（45 分钟），`last_heartbeat_at` 同样是 14:44:39；
- 期间 writer/editor 实际持续调用（日志 writer prompt 17–20k 字符，editor 10–12k）；
- v0.98.9 的 `useTaskProgressSync` 只在收到 `task-progress` 时刷新；训练批量阶段没有发进度事件；
- UI/Tasks 只能看到「进度 40% · 正在按原文风格连续续写…」不动。

**根因**：`ContinuationTrainingExecutor` 只在阶段切换调用 `ctx.update_progress`；`run_continue_batch_with_cancel` 没有 per-chapter progress 回调；`TaskExecutionContext::log` 只在阶段日志时更新心跳。

**修正**：批内每章装配后回调 training executor，更新 progress（40 + n*3）与日志（第 n/10 章完成，字数、gate 裁决）；资产分阶段（summary/aggregate）也发进度；每 30s 心跳。

---

### P1-1 质量门可靠性：3 章 degraded / 1 章 unrevised

**证据**：报告「6 章通过 / 0 章修订 / 3 章质检降级 / 1 章编审判修订但修订未完成」。gate editor 调用 75 次，56 成功，Qwren OOM/网络失败 20+，deepseek 也偶发 `LLM_RESPONSE_TIMEOUT`。

**根因**：editor prompt 10–12k tokens，本地模型 OOM；deepseek CoT-only/JSON 解析失败；gate 的 fallback 只能降级放行。

**修正**：
- 网关按 prompt 容量路由；
- editor gate 使用 `response_format=JsonObject` + 严格 JSON 提示 + prose fallback；
- 空 content 走最终 JSON 提取/二次提取；
- gate degraded 指标进报告与日志（已有一部分）。

---

### P1-2 truth hint 回灌下一轮：训练闭环有基准泄漏风险

**证据**：报告的 next-round improvements 含：

```text
真实后续节点：周成主动破局、立威（叫人畏我惧我）
真实后续节点：“高潮收尾+新人物引入”
真实后续节点：静态对话博弈，确立主角与黄玄然的合作契约...
```

`load_prior_insights` 会把上一轮报告 improvement 注入本轮 writer。若同一区间复用，writer 在“下一轮”拿到了原书后续的剧情提示，评估不再是盲测。

**影响**：连续几轮分数可能被人为抬高；无法判断 true continuation ability。

**修正**：
- training payload 增加 `feedback_mode`: `truth_hints`（训练用）/ `blind`（评估用）；
- blind 时过滤 `真实后续节点`，只保留结构性建议（“先读基准章梗概/禁止另开线索”）；
- A/B 使用不同区间（反馈区 vs 留出区），报告标注 `truth_hint_injected`；
- 未来加 `holdout_interval`。

---

### P1-3 writer prompt 膨胀与本地模型不匹配

**证据**：
- `continue_assets chars=9000 truncated=true`；writer prompt 17.9–20.5k 字符；
- Qwren 在 13,240 tokens OOM；deepseek 能处理但 CoT 慢；
- 资产块 9000 字含 brief 2042 + ledger + outline 1206 + style + cards，导致部分 dynamic 块被裁；
- `admitted=主角`、roster 空。

**修正**：
- 训练按模型容量分 **compact/full** 两种 prompt：
  - compact：brief ≤2500、ledger ≤600、progress ≤1000、style ≤800、cards ≤1200；目标 ≤10k chars；
  - full：deepseek 使用完整 brief + ledger；
- 网关按模型 `max_prompt_tokens` 选择模式；
- 本地 Qwren 仅用于 ≤4k tokens 的小任务（摘要/健康探测），大任务走 deepseek。

---

## 3. 根因链（本次实跑的因果图）

```text
资产单次 26k prompt + 8192 max_tokens
  → deepseek-v4 CoT 23,877 tokens，content 空
  → Qwren127 GPU OOM (16,781 tokens / 9,439MB vs 3,387MB)
  → 资产重建失败（4 次调用，约 4 分钟）
  → brief 只用首尾 70 字确定性兜底（10 章/1 线索/0 白名单/0 cast）
  → writer 缺人物卡/名册/关键中段铺垫
  → 第 16 章起模型自造收据/赤脚印/檀香线
  → 编审把原书后续差距误判为承接断裂（16/17/20 承接 1/2/2）
  → gate editor 在 Qwren OOM/deepseek CoT 空转下降级 3 章
  → 任务进度停在 40%，心跳停在 14:44（45 分钟不可见）
```

**注意**：这不否定 v0.99.1 的成果。结构上 brief/ledger 已生效：承接 7.2、style 7.3、总分 0.63，前五章 10/10/9/10/10。问题是**brief 质量**与**评分口径**在资产失败时暴露出来。

---

## 4. 解决方案设计

### 方案 A：资产重建分阶段化（P0，最高优先）

#### A1. 逐章摘要（Stage 1）

- 每章单独一次小调用：
  - prompt 只放单章正文采样（短章全文；长章 2,000–2,600 字头尾窗口） + 现有世界观骨架（可选）；
  - `response_format=JsonObject`，`max_tokens=800–1200`；
  - 输出结构化：
    ```json
    {"chapter":134,"summary":"...","location":"...","cast":["..."],"result":"...","open_thread":"...","turning_point":"...","key_terms":["福摩萨夜总会","邵卫江"]}
    ```
  - 并发 3–4，单章失败 2 次后走确定性兜底；
  - 缓存（book/chapter/prompt_version），复用 V161 或新表。
- 目标：单章 prompt ≤4k chars（≤2k tokens），Qwren 本地 4.6GB 可承载；不再依赖 deepseek CoT。

#### A2. 聚合 brief（Stage 2）

- 输入 10 条结构化摘要（约 1.5–2.5k 字）+ 世界观骨架（可选）；
- 一次小 JSON 调用（prompt ≤8k chars，max_tokens 4096）输出：
  - `arc_summary`（总脉络）
  - `threads`（3–8 条）
  - `canon_terms`（8–20 条）
  - `cast_states`（4–8 人）
  - 可选 `characters`/`relationships`
- 若失败：
  - `arc_summary = 首章 result + 末章 result`；
  - threads = 每章 open_thread 去重；
  - cast = 上下文出现的人名（字符卡/实体表）；
  - canon_terms = 摘要 key_terms + 世界观规则名；
  - **不接受散文/规划文档**。

#### A3. 持久化与兜底

- 逐章摘要先写 `scenes.outline_content`（“本章梗概：…”），再写 `story_briefs`；
- 若 Stage 1 全部失败，使用升级版确定性摘要（见方案 C）；
- 若 Stage 2 失败，确定性聚合；
- 训练报告记录：`summary_success/total`、`aggregate_success`、`fallback`、`terms/cast/threads` 计数。

#### A4. 代码落点

- `training_assets.rs`:
  - 新增 `summarize_chapters(llm, chapters) -> Vec<BriefChapter>`；
  - 新增 `aggregate_brief(summaries, skeleton) -> StoryBrief`；
  - 保留 `build_asset_rebuild_prompt` 仅用于聚合（或改名为 `build_brief_aggregate_prompt`）；
  - `rebuild_from_context` 改为编排两阶段；
  - `ASSET_PROMPT_CHARS` 26k → `SUMMARY_CHAPTER_CHARS=2_400`、`AGGREGATE_PROMPT_CHARS=12_000`；
- `Cargo`/`tokio` 并发：`futures::stream::FuturesUnordered` 或 `tokio::task::JoinSet`（已有依赖）限制并发 3；
- 缓存：V161 扩展 `summary_json TEXT`/`summary_version TEXT`，或新表 `continuation_training_chapter_briefs`。

### 方案 B：推理模型/JSON 硬化 + 网关容量路由（P0）

#### B1. OpenAI JSON 提取硬化

`llm/openai.rs` `resolve_content_with_options`：
- content 空 + JSON mode：
  1. `extract_last_json_object(rc)`；
  2. 若失败，取 reasoning 中最后一个 `{` 片段，调用 `complete_truncated_json` 补全；
  3. 若仍失败，返回完整 reasoning 作为 `reasoning_content`（新增字段）供调用方二次提取，而不是只返回空字符串；
  4. 对 `finish_reason=length` 的记录，在 `llm_calls.metadata` 标记截断；调用方重试时降低 prompt 或提高 max_tokens。
- `training_assets.rs` 的 `rebuild_from_context` 增加 **final-answer extraction** 第二请求：
  - 输入 reasoning（截取最后 8k 字）+ “只输出合法 JSON”；
  - max_tokens 2048，temperature 0；
  - 失败则走确定性兜底。

#### B2. Gateway 容量路由

- `LlmProfile`/capability profile 增加 `max_prompt_tokens_observed`（或 `gpu_memory_mb`）。
- 在 GPU OOM 错误中解析：
  - `Prompt (N tokens) requires ~X MB ... only ~Y MB`
  - 估算 `max_prompt_tokens ≈ N * Y / X`，写回 model health（如 Qwren ≈ 4k–6k）。
- `candidate_fits_prompt` 增加第二参数 `observed_prompt_limit`：
  ```rust
  fn candidate_fits_prompt(max_context_tokens, prompt_chars) -> bool
  // + candidate_fits_effective_prompt(observed_limit, prompt_tokens)
  ```
- `resolve_role_model`：对用户角色模型增加「大 prompt 不适配」检查；若 prompt tokens > observed_limit，跳过置顶并记录 `role_model_too_small_for_prompt`。
- 候选排序时对 GPU OOM 过的模型按 prompt bucket 降权（不是全局冷却）；小 prompt 仍可用。
- 空正文/GPU OOM 统一记 failure；`llm_calls.success` 不得记 1（当前 asset deepseek 空内容却 success=1）。
- 对 deepseek 等推理模型：JSON/结构化任务的 `max_tokens` ≥16k；或先要求“content 只放 JSON，不在 reasoning 输出”。

#### B3. 模型选择策略

- `TaskType::Analysis` / `continuation_training_assets` 使用 **非推理/结构化模型优先**，而非 Creative。
- 大 prompt（>8k tokens）优先 deepseek-v4；小 prompt（<4k tokens）可用 Qwren127。
- 报告/日志记录 `selected_model`, `prompt_tokens`, `fallback_count`, `gpu_oom`.

### 方案 C：确定性摘要升级（P0）

`training_assets::deterministic_chapter_summaries` 不再只取首尾 70 字：
- 切句（`。！？；` + 段落）；
- 打分：
  - 含冲突/转折动词（“对峙/逼/拒/发现/答应/交换/动手/逃/追/审/藏/夺/算/杀/换/进/退”）；
  - 含引号对白（身份/意图）；
  - 含专名/地点/器物（匹配实体表/角色名/世界观名词）；
- 选：
  - 开头 1 句 + 中间得分最高 2–3 句 + 结尾 1 句；
  - 每章 summary 150–220 字；
- 若实体表可用（`entity_mentions`/`kg_entities`），提取本章人名/地名/器物作为 `key_terms`；
- `build_brief_chapters` 使用升级后的 summary；
- `progress_lines_from_parts` 在 scene `outline_content` 为空时也使用同一摘要（不再只取尾巴 180 字）。

验收：
- 构造第 142 章 fixture（“邵卫江”在 81% 处），确定性 summary 必须包含“邵卫江/福摩萨”；
- 10 章 summary 总计 ≤2,400 字。

### 方案 D：训练进度/心跳（P0）

- `run_continue_batch_with_cancel` 增加进度回调：
  ```rust
  type BatchProgress = Arc<dyn Fn(i32, usize, usize) + Send + Sync>; // chapter, done, total
  run_continue_batch_with_cancel(..., progress: Option<BatchProgress>)
  ```
- `run_batch_inner` 在每章 `handle_gate` 完成后调用 progress；training executor：
  ```rust
  ctx.update_progress("continue", 40 + n * 3, &format!("续写第 {n}/{total} 章完成（{} 字）", chars));
  ctx.log("info", ...); // log 更新 heartbeat
  ```
- 资产两阶段：
  - summary 每完成 1 章 `ctx.log("info", "逐章摘要 n/10")` + progress 35→39；
  - aggregate 完成 progress 40。
- 30s 心跳：writer/editor 工具循环已发 heartbeat（llm service），但 task heartbeat 没有；在 progress callback 后 `ctx.heartbeat`（如果 API 存在）或 `ctx.log`。
- 验收：任务页 progress 从 40→43→46…→72；最新日志不再停 45 分钟；`last_heartbeat_at` 更新间隔 ≤60s。

### 方案 E：评分口径分离（P0）

`review_prompt` 修改：
- 明确：
  - `【上一章结尾（沙盒第 n-1 章；唯一的承接基准）】`
  - `【原书后续真实章（只用于衡量与原书后续差距；不是承接基准）】`
- JSON 字段：
  ```json
  {"style":...,"plot":...,"voice":...,"pacing":...,"hooks":...,
   "continuity": 1-10,  // 只相对上一沙盒章
   "reference_gap": 1-10, // 与原书后续差距，越高越接近
   "overall":..., "verdict":"pass|revise", ...}
  ```
- `plot` 定义改为“本章内部状态变化与推进”，不再以 truth 匹配度扣分；
- 报告新增列「承接（沙盒）」与「原书差距」；
- `ChapterReviewOutcome` 增加 `reference_gap: Option<f64>`；`scores_json` 增加 `reference_gap`；报告与训练页展示；
- `review_synthesis` 也读取 `reference_gap`，区分“内部断裂”与“与原书差距”。
- 测试：fixture 让 judge 输出 continuity 10/reference_gap 2；报告显示两列，不再把 truth 差距写成承接 1。

**补充确定性探针**（P1）：
- `sandbox_continuity_probe`: 生成章开头与前沙盒章末段的重合/人物/地点 → 若紧接，continuity ≥8；
- `reference_gap_probe`: 生成章与 truth 章的实体/事件重合 → 仅用于 reference_gap；
- `canon_probe`: 生成章新专名不在 brief cano_terms/context → 记录；
- `cast_probe`: truth 章/context 人物缺失 → 记录“人物留存率”。

### 方案 F：truth hint 控制与留出评估（P1）

- `TrainingPayload` 增加 `feedback_mode`（`truth_hints` / `blind`），默认 `truth_hints`；
- `load_prior_insights` 在 `blind` 时过滤含“真实后续节点”的行；
- 报告标注 `truth_hint_injected`；下一轮 A/B：
  - 区间 A 用作反馈，区间 B 作为 holdout（不在 `load_training_feedback` 的区间匹配内）；
  - 盲测 B 不注入 truth hints，评估真实能力；
- 未来训练页允许选择“盲测模式”。

### 方案 G：质量门可靠性（P1）

- editor gate 10–12k tokens → Qwren OOM；gateway 路由到 deepseek；`agency_editor` 调用 max_tokens 4096，JSON mode；若 content 空，`editor_verdict_prose_fallback`；
- 空正文/解析失败不再算 success；
- gate degraded 指标写入 report；下一轮目标 0 degraded。
- 若 editor 仍失败，gate 降级放行（已实现），但报告必须明确“降级原因”和候选模型。

### 方案 H：训练 prompt 模式（P1）

- `TrainingPromptMode`:
  - `Full`: deepseek/远程，brief 3600 + ledger 1000 + cards/relationships；
  - `Compact`: 本地模型（Qwren），brief 1800（10 章 × 120 + threads 300）、ledger 600、progress 800、style 800、cards 1200；目标 ≤10k chars；
  - `Auto`: 依据选定模型 `max_prompt_tokens_observed`选择。
- `continue_assets` 增加 `compact` 参数；render 时按模式裁剪块，保留 brief/ledger/chapter/next_node；cards/relationships 按名额。
- 验收：Qwren 不再出现 GPU OOM；writer prompt ≤10k chars；brief/ledger 仍全量。

---

## 5. 实施计划（分阶段）

### P0（下一轮训练前，1–2 天）

| 项 | 改动 | 验收 |
|---|---|---|
| P0-1 资产分阶段 | `training_assets.rs` 新增 `summarize_chapters` + `aggregate_brief`；每章小 prompt；聚合 prompt；缓存 | 10/10 章 summary；brief 10 chapters / ≥3 threads / ≥8 terms / ≥4 cast；asset calls 0 empty-content |
| P0-2 推理/JSON | `llm/openai.rs` 提取补全 + final-answer 第二请求；`max_tokens` 16k；空 content 记失败 | deepseek asset 调用无空 content；parse 成功 |
| P0-3 网关容量 | `model_gateway/executor.rs` observed prompt limit；GPU OOM 解析；候选跳过/降权；`llm_calls.success` | Qwren 大 prompt 0 次尝试；writer/editor success ≥95% |
| P0-4 确定性摘要 | `deterministic_chapter_summaries` 事件句抽取；`progress_lines` 使用同一摘要 | 第 142 章 summary 含邵卫江；10 章 summary ≤2.4k |
| P0-5 进度/心跳 | batch progress callback；training executor per-chapter progress/log | 任务 progress 每章推进；心跳 ≤60s |
| P0-6 评分口径 | review prompt + outcome/reference_gap + report columns/tests | continuity vs reference 分离；16/17/20 fixture 不再误判 |
| P0-7 报告指标 | asset stage/empty/GPU/brief 计数 | 报告有 asset attempts、summary counts、model fallback |

### P1（第二轮验证，3–5 天）

- 逐章摘要缓存（V161/新表）与 prompt version 失效；
- compact prompt mode + Auto 选择；
- gate editor reliability + JSON/prose fallback 指标；
- truth_hints/blind + holdout interval；
- 确定性探针（sandbox continuity/reference gap/canon/cast/thread）；
- 训练页展示 brief/摘要/asset 诊断；
- 训练报告 A/B 对比表。

### P2（稳定化，1–2 周）

- 通用 StoryBrief（普通续写）；
- brief 用户可编辑/预览；
- 模型设置页展示 GPU/context 上限与 fallback 建议；
- 训练区间随机化与盲测 harness；
- 训练报告 JSON schema 化（便于对比）。

---

## 6. 验收标准（下一轮真机）

### 资产与 brief

- 资产重建成功（或确定性兜底完整）：
  - chapters = 10（逐章事实）；
  - threads ≥3（不是 1 条 generic main）；
  - canon_terms ≥8（含“福摩萨夜总会/邵卫江/三理教”等上下文实际名词）；
  - cast_states ≥4（周成/张宝山/杨晓雯/黄玄然/陆尘音等上下文实际人物）；
  - 每章 summary 140–220 字，含中段事件（fixture 必须命中第 142 章邵卫江）。
- `continue_assets` 日志：`brief=10`、`ledger=n`，`truncated=false` 或 brief 完整保留。

### 生成与质量

- 10/10 章生成；writer/editor success ≥95%；Qwren 大 prompt GPU OOM = 0。
- 总分 ≥0.65；承接（沙盒）≥7.5；情节内部推进 ≥6.0；文笔 ≥7.0；无相邻章复制；无新体系名词。
- gate：0 degraded / 0 unrevised（或明确原因 ≤1）。
- reference_gap 单独报告；truth 章不参与 writer 提示词（除下一轮 hints，且 blind 模式禁用）。

### 进度/可观测

- 任务进度从 40→72 按章推进；最新日志每章更新；heartbeat 间隔 ≤60s。
- 训练报告「长程记忆」行显示 10/10 章摘要进入 writer/planner；asset stage 成功/失败原因。

### 测试

- Rust +12/新增：
  - `summarize_chapters_parses_and_caches`；
  - `aggregate_brief_falls_back_deterministically`；
  - `openai_json_mode_extracts_truncated_reasoning_json`；
  - `gateway_skips_model_with_gpu_oom_for_large_prompt`；
  - `deterministic_summary_includes_mid_chapter_term`；
  - `batch_progress_callback_reports_each_chapter`；
  - `review_prompt_separates_continuity_and_reference_gap`；
  - `report_marks_truth_hint_injected`。
- 现有 2203 passed / 17 ignored 保持；tsc/Vitest/Playwright 不变。

---

## 7. 风险与回滚

| 风险 | 影响 | 缓解 |
|---|---|---|
| 逐章摘要增加 10 次 LLM 调用 | 资产阶段变慢 | 并发 3–4 + 缓存；失败 2 章即降级；总预算 5–8 min |
| 本地模型 Qwren 仍 OOM | 生成慢/失败 | observed prompt limit + compact mode；大 prompt 走 deepseek；Qwren 仅小任务 |
| 推理模型 CoT-only | JSON 空 | final-answer 第二请求 + JSON 补全 + max_tokens 提高 |
| 评分口径变更影响报告可比性 | 历史分数不可比 | 报告 schema 增加 `reference_gap`；旧报告保留版本字段；训练页标注 |
| truth hints 泄漏 | 评估失真 | `feedback_mode=blind` + holdout interval；报告标注 |
| 摘要质量仍弱 | brief 仍弱 | 事件句抽取 + 实体表 key_terms；asset success 优先 |

回滚策略：
- 保留旧 `build_asset_rebuild_prompt` 聚合路径作为 `legacy` 开关（`TRAINING_ASSET_LEGACY=1`）；
- 评分口径切换用 `report_schema_version`，旧版报告保持原列；
- 网关容量路由为软策略，若误杀模型可配置 `allow_large_prompt_models`。

---

## 8. 附：本次实跑的代码位置

| 主题 | 位置 |
|---|---|
| 单次资产 prompt（26k） | `src-tauri/src/agency/training_assets.rs:48` |
| `rebuild_from_context` + max_tokens 8192 | `src-tauri/src/agency/training_assets.rs:91`、`:118-128` |
| `build_asset_rebuild_prompt` | `src-tauri/src/agency/training_assets.rs:175` |
| 确定性首尾摘要 70 字 | `src-tauri/src/agency/training_assets.rs:762` |
| `ensure_story_brief` / 兜底 | `src-tauri/src/agency/training_assets.rs:1181`、`:1199` |
| 全量逐章进度 + 1600 字上限 | `src-tauri/src/agency/coordinator.rs:10342` |
| 批内账本 | `src-tauri/src/agency/coordinator.rs:10418` |
| brief/ledger 渲染与 9000 预算 | `src-tauri/src/agency/continue_assets.rs:838`、`:1155` |
| 网关空正文/冷却 | `src-tauri/src/model_gateway/executor.rs:81`、`:1403` |
| 网关上下文窗口过滤 | `src-tauri/src/model_gateway/executor.rs:43` |
| JSON mode reasoning 提取 | `src-tauri/src/llm/openai.rs:142` |
| 编审 prompt（truth 混承接） | `src-tauri/src/task_system/continuation_training.rs:1777` |
| 批量 deadline | `src-tauri/src/agency/coordinator.rs:1799` |

---

## 9. 一句话

**v0.99.1 证明了“长程记忆注入”方向正确（承接 3.1→7.2、总分 0.40→0.63），但下一轮的病根转移到了“brief 生成质量 + 模型容量路由 + 评分口径”**；先做 P0 的四件事，再跑同区间 A/B，才能把「与原书后续差距」和「沙盒内部承接」分开，真正判断续写能力。
