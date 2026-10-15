# 决策与选择环节全量盘点 + 改造判定（2026-10-15）

> **最终决策（2026-10-15，作者）：本项目全部环节都不接 Laya。** 本盘点保留为「已评估、不采用」的记录与后续同类评估的模板；
> 落地的是与 Laya 无关的**后台 LLM 链条效率改造**（见 §三/§五 与 CHANGELOG v0.73.0）。
>
> 缘起：Laya Decision API（`10.62.239.13:17100`，非自回归 System-1 决策引擎，choice/score/noul 三类判断）接入评估。
> 方法：三路只读扫描（LLM 调用点 / 确定性判定 / 后台触发链，覆盖 src-tauri 与 src-frontend）+ **约 100 次 Laya 实测**（真实故事数据与真机库候选）。
> 结论先行：**Laya 的可靠区很窄**（短 state、单点、极性明确的二值判断，失败模式以「弃权」为主，40–260ms）；
> 它接不了复合语义判定与生成。真正的大头效率改进在**后台 LLM 链条本身**（与 Laya 无关）——即 v0.73.0 落地的部分。

---

## 一、决策环节全景（五类）

### 1.1 路由与角色判定（纯规则，已瞬时）

| 判断 | 位置 | 机制 |
|---|---|---|
| 请求→TaskClass | `model_gateway/dispatcher.rs:34` | 意图优先→TaskType→尺寸→标签 级联 |
| verb/object→类别 | `dispatcher.rs:75/95` | 关键词表 |
| TaskType+agent→类别 | `dispatcher.rs:105` | 白名单 |
| 尺寸升级 | `dispatcher.rs:127` | 输入>4000 或输出>1500 升档 |
| 角色模型解析 | `model_gateway/executor.rs:152` | 显式>TaskClass→健康→自动分配 |
| 候选链打分 | `executor.rs:517-765` | 健康 −1000/−20/×0.5、标签 +3、TaskClass 权重、TTFB/TPS 加权 |
| 健康/降级/退避 | `executor.rs:28/418`、`health.rs`、`scheduler.rs:32/224` | 连续失败≥2 降级；退避门槛 3 |
| label→意图/角色 | `llm/service.rs:249/267` | 关键词表（第五套） |
| 路由打分量表 | `router/router.rs:232-349` | 任务分/复杂度/成本/速度惩罚 |

**判定：保持规则。** 全部是查表/加权，0 LLM、微秒级；换成任何引擎都只会更慢。唯一值得做的是**阈值校准**（见 §四 C 类）。

### 1.2 意图与规划判定（LLM 支撑，失败有规则兜底）

| 判断 | 位置 | 现机制 | 频次 |
|---|---|---|---|
| 写作意图分类（6 字段） | `intent.rs:464`（prompt 见 :524） | LLM（`intent_classify`→BalancedWork→**创作档 deepseek**），8s 超时，三路兜底 + 会话缓存 | 每次用户输入 |
| 意图解析（11 类） | `intent.rs:319` | LLM + `Intent::unknown` 兜底 | 每次输入 |
| verb/object 意图 | `intention_graph/builder.rs:97` | LLM(100tok) + 关键词兜底 | 每次路由 |
| 执行计划（步骤/能力选择） | `planner/mod.rs:787` | LLM，**无兜底（Err）** | 每次规划 |
| 资产选择（beat_plan） | `planner/executor.rs:1757` | LLM 混合（选资产=决策） | 每章 |
| 方法论/风格/技法选择 | `strategy/selector.rs:96` | LLM + 精确匹配 merge 兜底 | 每次策略 |
| 输入兜底启发式 | `intent.rs:234/270/692` | 关键词（LLM 失败时） | 兜底路径 |

### 1.3 质量门与打分（LLM + 规则合成）

| 判断 | 位置 | 现机制 |
|---|---|---|
| 质量门加权 | `agency/gate.rs:20/24` | 0.2 code + 0.3 rule + 0.5 model ≥ 0.75 |
| code/rule grader | `agency/graders.rs:33-142` | 规则（自重复/字数/禁则/追读力公式） |
| editor 裁决 | `agency/coordinator.rs:6426` + tool_loop | LLM（ReAct + 散文回退） |
| mini_review | `story_system/mini_review.rs:121` | LLM（**每次 commit**）+ 关键词回退(:158) |
| 审稿报告 / Inspector / PlotAnalyzer / quality_evaluation | `pipeline/review.rs:99`、`agents/service.rs:1444/1564`、`creative_engine/workflow/quality.rs:294` | LLM + 规则兜底 |
| 级联冲突扫描 | `creative_engine/cascade_rewriter/impact_report.rs:634` | LLM + (0,0) 跳过 |
| 判官一致率引用幸存 | `agency/editor_agreement.rs:25/75` | 规则（bigram ≥0.8） |
| 追读力/伏笔紧迫度 | `graders.rs:233`、`foreshadowing_service.rs:478` | 公式/阈值 |

### 1.4 语义探针（规则做语义，漏报风险区）

| 判断 | 位置 | 现机制 |
|---|---|---|
| 死亡判定 | `story_system/life_status.rs:134` | 关键词 + 否定豁免，**每次保存 + 每次 commit 各一遍整章** |
| 知识泄密 | `memory/continuity.rs:647` | ≥8 字片段匹配 + 本拍规划排除 |
| 物品归属 | `memory/continuity.rs:703` | 物品名 ±40 字窗口 + 使用动词 |
| 每拍探针缺口（推进/复述/死者行动） | `agency/beat_state.rs:265/185/221/359` | 关键词 + bigram ≥0.62 + 在场匹配 |
| 人物指称/身份聚类/亲缘推断 | `agency/continue_director.rs:158-423` | 并查集 + 称号词表 + 粘连名启发式 |
| 关系不变量清洗 | `continue_director.rs:602/676/701` | 硬规则（血亲×夫妻等） |
| 实体去重 | `memory/ingest.rs` link_entities | **name 全等**（无语义相似） |

### 1.5 资产选择与后台分流（触发链，见 §三）

`scene_service.rs:463-632`（30s 防抖 / 自动分章 / Observe vs Ingest 分流 / auto_recompute 闸门）、
`commit_service.rs:55-547`（commit 链）、`agency/learning.rs:61/241/273/512-625`（观察→分析→晋升阈值）、
`story_system/style_learning.rs:31-91`（120s 防抖 + 前后差异门）、
`memory/ingest.rs:545`（importance≥7 入待确认）、`retention.rs`（遗忘曲线分档）。

### 1.6 前端本地感知（每次输入跑启发式）

`src-frontend/src/frontstage/ai-perception/textAnalyzer.ts`（段落类型/句式单调/词汇重复/节奏曲线，含 O(n²) 句块检测）、
`suggestionEngine.ts:267-324`（模板打分）、`utils/textCleanup.ts`（去重/自重复裁剪/幽灵判定）。

---

## 二、Laya 实测矩阵（约 100 次调用，真机数据）

延迟基线：**单问 40–74ms；6 问一次调用 50–250ms；2978 字 state 的 2 问 259ms；串行 10 次中位 45ms**（对照：`intent_classify` 现路径 deepseek **2.1–3.2s**、本地 Qwen3.5 **2.7–18.9s** 且在 8s 超时威胁下）。

| 任务形态 | 结果 | 判定 |
|---|---|---|
| 单句情绪极性（choice 3 类） | **4/5**；真机段落逐段 39–56ms | ✅ 可用（展示型） |
| 死亡标记判定（noul，6 例真值） | **5/6**，且 2 个「错」都是**弃权（未知）**而非反判 | ✅ 可作预筛（弃权交回正则） |
| 人物指称同一性（noul，别称表真值 8 例） | 6/8，1 个假阳(0.880) | ⚠️ 仅提案制 |
| 世界规则近重复（noul，真机 pending_reviews 4 例） | 3/4，1 个假阳(0.875) | ⚠️ 仅提案制 |
| 完整意图分类（6 字段一次调用） | **23/36 = 64%**（task_type 系统性偏 rewrite） | ❌ 不能直接替换 |
| 输入分流：创作 vs 闲聊（noul） | **4/7**（「继续写」判否） | ❌ |
| 文笔质量四级打分（score） | 具象样本 1.36 vs 抽象样本 1.41 — 无区分度 | ❌ |
| 检索相关性（noul，结构化 state） | **3/3 反向** | ❌ |
| 敏感内容（noul） | 1/3 | ❌ |

**API 使用约束（实测踩坑，写适配器必须遵守）**：
1. `noul` = P(是)，阈值 0.65/0.35 三段（是/未知/否）可用；**问句必须短、单一、无否定从句**——带「…都不是」的对照说明会把 0.695 打成 0.168。
2. `state` 只传**纯文本**；传 dict 明显偏置（「继续写」被判 is_new_novel=0.761）。
3. `score` **必须**带 `criteria`（分级描述列表），否则 HTTP 500（与官方示例不一致）；返回 `score`（索引空间连续值）+ `probabilities` + `legend`。
4. `choice` 返回命中的 criterion **原文**（需映射回标签）；用 `answer_confidence`/`noul` 做阈值，不用 `confidence`。
5. 一次调用多问是最大效率点；固定 `laya-multilingual`（换 model 触发重载）。

**可靠区画像**：短 state + 单点判断 + 答案可由关键词锚定（气绝/尸首/写一部）+ 语义极性明确。
**失败区画像**：结构化/大 state、多字段复合、近义细粒度区分、排序/打分区分度、短祈使句的三分类。

---

## 三、真正的运行效率热点（不依赖 Laya，按收益排序）

后台链条现状（扫描结论）：

1. **IngestPipeline（3 次 LLM，各最多 3 次重试）挂在 5 条触发路径上**（commit、scene ingest、每拍后 assets ingest、Observe、空角色表 hot-path），且受 `concurrency.rs:8` 的 **`BACKGROUND_LLM_SEMAPHORE = 1`** 全局串行——每章最多约 18 次后台 LLM 调用排队。**合并三次调用为一次**（同一上下文与契约）是最大单点收益。
2. **auto_commit 每章 2 次 LLM**（`chapter_summary` 400tok + `mini_review` 800tok，`commit_service.rs:115/141`）且与续写同 30s 窗抢模型；两者都有确定性兜底 → 可合并为一次调用，或按正文指纹/分钟条件短路。
3. **每次保存两遍整章死亡扫描**（`update_scene` 一遍 + `auto_commit` 一遍）在用户保存路径内（await）→ 合并为一次、并挪出关键路径。
4. **`maybe_enrich_director_lock` 每拍一次 LLM（20s 超时）**（`coordinator.rs:4771`）——其结果只是别名/亲属富化，可用别名表 diff 判定「有新增称呼才触发」。
5. **前端 `textAnalyzer`（复核后修正：本项无需改造）** —— 实际接线是 `SmartHintSystem`：
   停止输入 **1.5s 防抖** + 同内容跳过 + **Web Worker** 内执行（`asyncTextAnalyzer`），
   主线程不受 O(n²) 句块检测影响；盘点初稿写「每次输入全量跑」是**误判**，已在 v0.73.0 复核修正。
6. 网关 keepalive 10s × N 模型常驻探测（`scheduler.rs:32`）→ 空闲期惰性化（有请求才探测）。

---

## 四、改造判定表（逐环节）

### A. 保持规则（不改造）
路由/角色关键词表（§1.1 全部）、指纹与单调状态机（`chapter_summary.rs:103`、`character_life.rs:86`、`continuity.rs:262`）、bigram 相似度（`beat_state.rs:185/221`、`editor_agreement.rs:75`）、关系不变量硬规则（`continue_director.rs:602`）、gate 合成加权（`gate.rs:20`）、前端文本清洗（`textCleanup.ts`）。
**理由**：确定性、微秒级、可复现——任何「更聪明的判断」在这里都是净损失。

### B. Laya 快车道（有实测支持，P1 可接）
| 接线点 | 形态 | 证据 | 失败代价 |
|---|---|---|---|
| 逐段情绪/基调标注（喂前端情绪曲线，展示型） | choice 5 类，每段 40–60ms | 单句 4/5；真机 8 段延迟 39–56ms | 展示瑕疵，无害 |
| 新章正文死亡标记**预筛**（仅高置信采纳，弃权交回正则） | noul + 分段 | 5/6，错误=弃权 | 弃权即回退，无新增风险 |
| （待影子）世界规则 importance 粗排 | score | 未校准 | 仅排序建议 |

### C. 先影子评估再定（达标才转正）
1. **意图分类快路径**：仅当「高置信 continuation 且非创世」时短路 LLM。当前 64% 不够 → 先影子记录 `decision_records` 的一致率，达到 95% 且弃权率 <20% 才开；创世/改写/资产重写永远走 LLM（误判代价不对称）。
2. **规则/人物去重提案**：真机 `pending_reviews` 里已有真实近重复（「赤雪牛役使规则」vs「赤雪牛—血桉役使」、「血桉剧毒」vs「血桉劇毒」），但 Laya 3/4 且有一个假阳 → **只能作为提案卡片给作者点选，禁止自动合并**。
3. **mini_review 预筛**：本轮未见区分度（对第 3 章全判「需要复审」）→ 先影子。

### D. 保持 LLM（Laya 明确不胜任）
章纲/卷纲/资产生成（工具档主业）、检索相关性判定、文笔质量打分、敏感内容/红线、复合意图分类直接替换、输入分流、导演锁富化。

### E. 阈值校准（先埋点统计，再改数字）
gate 0.2/0.3/0.5 与 0.75、salvage 600 字、候选链打分权重、健康降级 2 次/退避 3 次/10s/15s/30s、成本哨兵 500k/5 次、预算 300k、retention 分档与 λ、伏笔紧迫度 7/5/8、学习晋升 0.8/跨 2 故事、前端 24/120/30/500/100/20 字等——均无实测分布支撑；`human_voice` 的 FLAT_RUN_LEN=4 与 not-x-but-y≥2 是**唯二有语料校准记录**的。

---

## 五、落地计划

**P1（低风险，即可实施）** —— ①决策引擎端口 + ②Laya 快车道接线：**已由作者决策取消（不接 Laya）**；③效率类第 3、5 项中「双扫描合并」与「前端节流」见下。

**P2（需影子数据）**：意图快路径 / 去重提案 / mini_review 预筛——**取消**（前提是接 Laya）。

**P3（工程重构，收益最大）——v0.73.0 已落地**：
1. **IngestPipeline 同内容结果复用**：同一正文被两条后台路径各 ingest 一次时，第二次直接复用结果（0 次 LLM），
   一次性消掉最长 9 次串行调用；正文一变指纹即不同，不复用。附带 Step2/Step3 **并发**（两步只依赖 Step1）。
2. **提交链摘要 + mini review 合并为一次 LLM 调用**（`chapter_commit_digest`）：happy path 每章 2 次 → 1 次；
   解析失败/异常完整回退旧两步，连续失败 3 次本会话停用合并路径。
3. **死亡扫描去重**：同 (story, 章, 正文指纹) 在 300s 内只扫一遍（保存链 + 提交链此前各扫一遍）；
   作者改判存活（`revive`）会失效备忘。

**验收口径（v0.73.0 实际采用）**：单测/端到端回归（计数桩证明「同内容第二次 = 0 次 LLM」、
合并调用的解析/回退/熔断、扫描去重的重标路径）+ 全量 `cargo test --lib` 与探针复跑。

**不做**：工具档生成任务接 Laya；自动合并/自动改写类决策交给 Laya；`TaskClassifier` 等确定性路由改造。
**留待后续（本轮评估后未做）**：`maybe_enrich_director_lock` 每拍 20s 超时 LLM 的门控（判据需更精确：
近文称号词/亲属词命中才跑——误判会丢失人物称呼富化，风险大于收益）；网关 keepalive 惰性化。
