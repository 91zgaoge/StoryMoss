# AI 长篇小说创作生态横向对比与引进分析报告

> 日期：2026-10-06
> 缘起：X 帖子 [@Passenger0522](https://x.com/Passenger0522/status/2107374585658388660)——长篇写作如何解决「剧情失忆/吃书」。
> 方法：6 个并行研究代理分别实际抓取/克隆 5 个外部仓库的源码逐文件分析，另 1 个代理梳理 StoryMoss 代码库现状。所有结论基于源码级调研，非 README 转述。关键论断已在 StoryMoss 本地代码抽查复核。

---

## 一、帖子核心论点（对比的三把尺子）

作者的核心判断：**大上下文窗口 ≠ 剧情逻辑连续性**。真正能写长篇不崩的系统，核心在三件事：

1. **状态锁**——物品归属、人物状态、时间线的显式管理与校验
2. **角色知识边界**——角色不能「偷看剧本」，说出不该知道的信息
3. **改稿后的级联更新**——回头大修旧章后，系统能发现后文矛盾

并给出三个测试方法（同一份 5 章细纲）：①**玉佩测试**（物品归属是否前后一致）；②**知识边界测试**（角色是否泄露不该知道的信息）；③**级联冲突测试**（大修旧章后能否揪出后文冲突）。

这三把尺子恰好是对 StoryMoss 最锋利的验收标准——按现状自评：**三把尺子里目前一把半能过**（详见第三节）。

---

## 二、五个项目深度画像

### 2.1 oh-story（`zenstory-ai/oh-story-claudecode`）— 7.3k★，MIT，v0.8.4

**形态**：Claude Code/Codex 的 skill 包（13 个 skill + 7 个分档 Agent + 8 个自动化 hook + 100+ 份方法论文档），不是应用，写作用模型即宿主 Agent 的模型。

**核心机制**（帖子说的「三层信息分离/状态锁/级联扫描」原型就是它）：

- **单一权威 + 确定性派生视图**：`追踪/_tracking-state.json` 是唯一可写真相；上下文状态卡、角色快照、伏笔视图、双时间线全部由 1691 行的 `tracking_commit.py` 确定性重渲染，`check` 子命令重渲染逐文件比对——手改派生文件直接报错，「两份状态各说各话」在结构上不可能。
- **三层信息分离的完整落地**：每个时间线事件（E 编号）同时记录 `objective_fact`（世界真相）/ `reader_knowledge` + `reveal_status`（读者已知）/ 角色快照的 `knowledge` 字段（角色所知）。注入策略是关键：**写手 prompt 只下放「要碰与要避」的伏笔禁令，作者侧真相留在主会话**；稳定人设（设定/角色/）与当前认知（追踪/角色状态/）分开读——「两者分开读，模型就不会把设定当成角色知道的事」。
- **状态锁双保险**：`O_EXCL` 互斥文件锁 + `state_revision` 乐观并发控制，事务级「两个并发事务至多一个成功」，锁内重新计数重验证。
- **级联更新半自动**：`mode=revision` 事务从改动章 X 重算到最新已写章 M（伏笔从 X 扫到 M、角色 8 维快照从 X 重算到 M、时间线双视图重建），后文冲突**列出清单由作者裁决**，不自动改写。
- **确定性文本质检**：2000+ 行去 AI 味检测器（blocking/advisory 两档，规则经真人语料校准——20 章 0 命中才升 blocking）；工程词泄漏检测（细纲/字数目标/章首钩子等流水线术语漏进正文即 blocking）。
- **硬容量上限**：状态卡 12KB、活跃角色 6 人、活跃伏笔 8 条、近章 3 章——第 300 章的热上下文与第 30 章同大小。

**硬伤**：无向量/embedding 检索（忘了登记 ID 就找不到）；级联靠人裁决；细纲 9 必填字段流程重；中文网文深度特化。

### 2.2 MuMuAINovel（`xiamuceer-j/MuMuAINovel`）— 3.1k★，GPL-3.0，v1.5.6

**形态**：多用户 Web 工作台（FastAPI + PostgreSQL + ChromaDB + 本地 ONNX embedding + React/AntD），Docker 支持 Apple Silicon。

**核心机制**：

- **伏笔子系统是全场最完整**：`plant_chapter` / `target_resolve_chapter`（计划回收）/ `actual_resolve_chapter`（实际回收）三组章节分离；状态机 pending/planted/resolved/**partially_resolved**（长线分次回收）/abandoned；`strength`(强度)/`subtlety`(隐藏度)/`urgency`(紧急度)；伏笔链 `related_foreshadow_ids`；稳定去重 ID `sha256(chapter_id+content+type)`；每章自动新建上限 5 个；`remind_before_chapters`（默认提前 5 章提醒）。
- **四层伏笔注入话术**：本章必须回收（最详细）/ 超期（最多 3 个）/ **近期待回收——明确标注「请勿在本章回收」** / 本章计划埋入。
- **实体状态闭环**：逐章分析 → 角色状态写回 `characters.current_state/status/status_changed_chapter`（含生死、心理状态）→ 下次生成注入「当前状态：死亡（第 X 章变更）」。
- **改稿重分析**：删该章记忆 + ChromaDB 向量 → 重提取 → 级联更新职业/角色状态/组织/伏笔。
- **正文内标注可视化**：分析时强制 AI 返回原文 8-25 字 `keyword`，三级匹配回填 `chapter_position`，伏笔/钩子/情节点以图标直接标在章节正文上，与记忆侧边栏双向联动。

**硬伤**：世界观只有 4 个自由文本字段且**不进章节生成 prompt**（扁平反面教材）；无知识边界；PG 与 ChromaDB 双写无事务一致性（自认靠关键词匹配删向量）；无 token 预算管理；GPL-3.0。**宣传的「伏笔时间线可视化」在代码中不存在（实为表格+统计卡片）**。

### 2.3 ani-book-skill（`ExplosiveCoderflome/ani-book-skill`）— 95★，Apache-2.0，v0.3.1

**形态**：OpenAI Codex 的 skill + 零依赖 Python 确定性脚本（依赖只有 PyYAML）。「Codex 出创意、YAML 存真相、SQLite 只当可丢弃缓存、SHA-256 锁证据、一切冲突先保护人类编辑」。

**核心机制**：

- **YAML 六域事实库**：`facts / payoffs / resources / character-state / relationships / asset-links` 分文件存续性事实，稳定 ID（FACT-001/CHAR-001），**每条必带 `evidence.text` 原文证据**；纯中文自然写 YAML（不转义、不折行），人类可读可手改。
- **SQLite 仅为可重建索引**：FTS5 + 章号候选过滤，**模块自述「never reads facts back from SQLite」**——索引删除/过期自动退化 YAML 直读，写作永不阻塞。Markdown 台账是生成视图，头部注入 `do not edit` 标记。
- **reconcile 手改协调**：SHA-256 对比产物指纹，用户改过的文件标 `user_edited + protected`，下游依赖标 `stale` 传播，**任何命令不静默覆盖用户文件**。
- **跨书资产库**：fork / sync / conflict 三态链接；选中资产被 `library_hash + local_hash + content_sha256` 三重哈希锁死为固定快照；`canon-check`（正史事件顺序矛盾/环检测）+ `timeline` + `impact` 只读审查（永远 `writes: false`）。
- **质量债台账 + 章节修复上限**：非阻塞问题入 `quality-debt.md`（含建议回收窗口）；`chapter_repair` 每章最多 2 次，防止无限审修循环。
- **Token 账本纪律**：exact/estimated/unavailable 三档，「不得用字符数冒充 Token」。

**硬伤**：事实抽取完全靠 agent 手工回灌（脚本只做结构校验）；中文 FTS 用 unicode61 形同虚设；无 POV 过滤；Codex/Windows 绑定；社区小。

### 2.4 ainovel-cli（`voocel/ainovel-cli`）— 2.1k★，Apache-2.0（README 写 MIT，不一致），v0.7.9

**形态**：Go 写的全自动多 Agent 引擎（TUI + Headless + Docker）。「事实层确定、语义层自主」——`flow.Route` 纯函数决策表 + 万级穷举测试。

**核心机制**：

- **上下文自适应**（`ContextProfile`）：≤15 章近 10 章摘要；16-50 章近 5 章；**>50 章切三级分层摘要（卷→弧→章）+ 近 3 章**。写作上下文四区（working/episodic/reference/selected）+ 上一章尾部 800 字保语气衔接。
- **四维相关章反查**：活跃伏笔埋设章 / 角色最后出场章 / 状态变化章 / 关系变化章——推荐「章号+理由」，写手按需 `read_chapter` 回读原文，不预载。
- **`/sync` 修订级联**（改稿级联的完整工程实现）：SHA-256 检测用户手改已完成章 → LLM 修订分析（新事实 + **`style_delta` 从用户删改逆向提炼文风偏好** + `downstream_issues` 下游冲突）→ Projector 重建派生状态 → 弧/卷摘要与角色快照标失效由 Editor 补建 → 大纲反馈池交 Architect 传播到后续规划。**未 sync 前禁止继续写作**。
- **style_stats 全书确定性统计**：逐字重复句、句式模式章均计数、章尾短句收尾占比、开篇时间词率——弧内评审对「章均几十次的句式 tic」天然失明，只有全书统计能暴露。
- **预算哨兵**：`book_usd` + warn_ratio + 子代理边界优雅停机 + **零增量计费盲区检测**（连续 5 笔零增量记账告警「预算上限不会触发」）。
- **逐章放行门**：`/review on` + `/next` 精确到章号的许可，提交 saga 任何窗口崩溃都不会误耗/重复消费许可。
- Editor 七维审阅，**每维必须引用原文举证**，不接受空泛结论。

**硬伤**：结构性费 Token（草稿全文多次回读）；AI 检测率问题（全自动磨平语气，帖主已点出）；无向量检索；返工范围曾失控（issue #90：改 3 章结果返工 1-10 章）。

### 2.5 webnovel-writer v6（`lingfengQAQ/webnovel-writer` master 分支）— 7.3k★，GPL-3.0，6.2.1

**形态**：Claude Code 插件（Python 3.10+）。v8 是 Windows 首发的 DeepSeek Harness 桌面工作台，与 Claude Code 无关且 v6 无迁移路径——帖主「Mac/Claude Code 用户认准 master 的 v6」与仓库事实一致。

**核心机制**：

- **事实源与只读投影严格解耦**（帖子说的就是它）：`.story-system/`（合同 + CHAPTER_COMMIT + 事件审计）是唯一事实源；`.webnovel/`（state.json / index.db / summaries / memory / vectors）五路全部是派生只读投影。**声明式事件投影路由表**：10 种 event_type × 5 个 writer 的矩阵（如 `character_state_changed → [state, memory, vector]`），`projections replay --from-chapter A --to-chapter B` 按范围重放。
- **三重防直写**：PreToolUse hook（Edit/Bash 直写受保护路径即 deny）+ Agent 职责边界 + write-gate 三自然边界（prewrite/precommit/postcommit，投影五路全 done 才放行）。
- **断点续跑**：run-ledger 记录每步产物，失败只补跑失败步骤；`--fast/--minimal` 降级模式。
- **追读力系统**：钩子强度/爽点/微兑现/债务追踪，`override_contracts`（偏离登记 + 补偿计划 payback_plan + 到期章）。
- **/webnovel-learn**：作者一句话经验 → 文风模式库 → 下次注入任务书。
- `entity_state_at_chapter` 时间旅行查询（从 append-only `state_changes` 流水按章号重放）。
- v7 冻结规格里有完整的「信息差」设计（每条一个文件：知情人/读者已知/关键词；泄密机检=扫描对白比对角色不知道的关键词）——**未随 v6 发布**。

**硬伤**：v6 只修致命 bug（新功能已转 v8）；guard hook 可环境变量绕过且 state.json 不在保护列表；无知识边界落地；重流程高成本；GPL-3.0。

---

## 三、与 StoryMoss 的正面对比

### 3.1 StoryMoss 的领先项（对比时不能妄自菲薄）

| # | StoryMoss 能力 | 外部对照 |
|---|---|---|
| 1 | **LanceDB 混合检索 + KG + QueryPipeline 预算控制**（RRF 融合、四阶段、token 预算分桶） | 5 个项目里 3 个（oh-story / ani-book / ainovel）**完全没有向量检索** |
| 2 | **节拍卡 0-LLM 编译 + 必须改变项（change_delta 五类）+ 末句锚点**——防原地踏步的确定性短合同 | oh-story 的「契约四问」是最近似物，其余项目无对应 |
| 3 | **导演锁**（一人一号、死人不可复活含 DEAD_AGENCY_MARKERS、亲缘反转拦截）——纯 Rust 状态锁 | 帖主说的「状态锁」，多数项目靠 LLM 审查兜底 |
| 4 | **桌面双形态产品**（幕前沉浸 + 幕后工作室） | 5 个全是 CLI / 插件 / 网页 |
| 5 | **源感知资产合并 + V134 `user_created` 保护** | ani-book 的 reconcile 同哲学，MuMu/webnovel 无此层次 |
| 6 | **质检可行动化**（editor_qc 事件带 mode/chapter + 幕前「按审查意见修订本章」`auto_revise(editor_qc)`） | webnovel 的作者友好报告契约最近似 |
| 7 | 伏笔逾期动态阈值 + 情感张力账本 + 扩张配额（RotationLedger/ExpansionDebt） | MuMu 的 urgency 是静态字段 |

### 3.2 帖主三把尺子的现状自评

| 尺子 | 现状 | 判定 |
|---|---|---|
| ①玉佩（物品归属） | KG 有 Item 实体但**无归属状态机**：没有 possession 表，无「某物品此刻在谁手里」跟踪，改稿/续写不更新归属 | ❌ 不过 |
| ②知识边界 | `character_states.secrets_known/secrets_unknown` 字段存在且已注入 prompt（`writer_assets.rs:64`），**但 ingest 用 COALESCE 永久保留旧值**（`ingest.rs:1515-1516`）——秘密字段只能靠手动更新；无「世界真相/读者已知/角色所知」三层区分；无泄密探针 | △ 半过 |
| ③级联冲突 | 编辑旧章自动防抖 re-ingest + `entity_mentions` 重建 ✅；但 `cascade_rewriter`（ChangeDetector/ImpactAnalyzer/RewriteEngine 齐备）是**手动任务**，无「改第 3 章自动标记第 7 章矛盾」的闭环 | △ 半过 |

### 3.3 其余维度缺口

| 维度 | 现状 | 外部最强 |
|---|---|---|
| 章节摘要 | `scene_commits.summary_text` = **前 1000 字截断**（`commit_service.rs:49`）；无 LLM 语义压缩、无章→弧→卷分层金字塔；长篇远期只靠检索概率召回 | ainovel 三级分层摘要 |
| 确定性文本质检 | 只有 prompt 层 genre_antipatterns；无去 AI 味检测器、无工程词泄漏检测。**且 StoryMoss 注入【必须改变】【本拍】等流水线术语，泄漏风险比 5 个外部项目都高** | oh-story 双档检测器 |
| 作者风格学习 | style_dna_summary 有槽位，无从作者修订逆向学习的链路 | ainovel `/sync style_delta` |
| 成本可见性 | llm_calls 有记录；无预算哨兵、零计费盲区检测 | ani-book + ainovel |
| 伏笔精度 | 有 pending/overdue 注入；无计划/实际回收分离、强度/隐藏度、伏笔链、每章新建上限、证据锚定 | MuMu |
| 质检闭环 | 单轮修订（gap 重试一次 / 批量一轮 LeadWriter）；QC 失败 **fail-open 降级放行** | ani-book 质量债 + 修复上限 |

---

## 四、引进路线图

### P0 —— 直接补齐帖主三把尺子

**P0-1 角色知识边界：三层信息分离**（源：oh-story，MIT，实现可参考）

现状锚点：`character_states.secrets_known/secrets_unknown`（V014/V116）注入在但永不更新；`narrative_events` 与 `CanonicalStateSnapshot.timeline` 无读者认知维度。

1. V135 迁移：时间线事件增加 `reader_knowledge` / `reveal_status`（未揭示/部分/已揭示）/ `reveal_chapter` —— oh-story E 编号双栏设计的核心是「客观发生了什么」与「读者此刻知道什么」同记录。
2. ingest 抽取 schema 增加 `information_transfers`（who→whom、secret、chapter）——补上 secrets 的更新链路：角色被告知 → 从 secrets_unknown 迁移到 secrets_known（现在 COALESCE 保留等于永久冻结）。
3. 注入分层：续写资产里区分「设定卡（稳定人设）」与「追踪态（当前所知）」分开渲染（oh-story：「两者分开读，模型就不会把设定当成角色知道的事」）；【本拍角色】卡增加「该角色尚不知道：X」行。
4. 泄密探针：`probe_increment_ex` 新增 gap 类型——在场/POV 角色说出其 `secrets_unknown` 中的内容（secrets 字段 + 对白的关键词确定性匹配可做初版）。
5. editor_qc prompt 增加双栏对照检查「提前泄密」。

验收：帖主测试②固化为回归探针（类似现有 `probe_gaps_when_increment_is_tail_recap` 契约测试）。

**P0-2 物品归属状态机（玉佩账本）**（源：ani-book resources 域 + oh-story abilities_resources）

1. V136 迁移：`item_holdings` 表（story_id、item_entity_id、holder_character_id、acquired_chapter、visibility_state、status、evidence_scene_id）。
2. ingest possession 抽取，**只登记跨章影响行动边界的资源**（ani-book 的克制原则：个人短期状态回写角色档案，防止账本爆炸）。
3. `BeatState` 编译时纳入在场角色持有的关键物品，节拍卡渲染【在场物品】。
4. 归属矛盾探针：正文出现「X 拿出/交出/丢掉 Y」时与 holdings 比对（复用实体别名归一，导演锁 TITLE_TOKENS 已有同类基建）→ gap「物品归属矛盾」。
5. 改稿 re-ingest 时随 entity_mentions 一起重建该章 holdings。

验收：帖主测试①（玉佩连续性）成为探针。

**P0-3 改稿级联：从手动任务到影响报告闭环**（源：oh-story revision 事务 + ainovel `/sync` + MuMu 重分析）

现状锚点：`creative_engine/cascade_rewriter/`（change_detector / impact_analyzer / rewrite_engine）引擎齐备但入口是手动任务（`scene_commands.rs:1004 trigger_cascade_rewrite`）；`entity_mentions`（V072）引用索引已在改稿时重建。

1. re-ingest 完成后**自动**跑 ImpactAnalyzer（零新引擎，把手动的串联起来）→ 产出「受影响后续章节清单 + 影响实体 + 分数」事件推给前端（state_sync 通用频道现成）。
2. LLM 级联冲突扫描（ainovel `downstream_issues` 模式）：新章事实 vs 下游 `scene_commits.summary_text` 比对 → 列出疑似矛盾。
3. 幕后/幕前「级联中心」：每条冲突三动作——去查看 / 标记忽略 / **触发已有的 cascade_rewrite** 改写该场景。
4. 摘要失效标记：受影响章 summary_text 标 stale（ainovel `InvalidateChapterAggregates`），后台重摘要。

哲学取舍：**列出冲突由作者拍板，不自动改写后文**——oh-story 的模式，与 V134 user_created 保护同一精神。不学 oh-story 的全量修订事务（每改一章全流程重跑，StoryMoss 有 DB + 事件可以做得轻得多）。

验收：帖主测试③。

**P0-4 章节语义摘要 + 分层金字塔 + 自适应窗口**（源：ainovel ContextProfile）

1. commit 时 LLM 产出 100-150 字语义摘要替换 1000 字截断（复用 `run_mini_review` 管线时机或并入 ingest Step 1）。
2. 章节数自适应：≤15 章近 10 章摘要；>50 章近 3 章 + 卷/弧两级滚动摘要（新表 `arc_summaries`）。
3. 检索兜底不变——金字塔摘要提供**确定性远期纲要**，「检索概率召回」升级为「检索 + 确定性纲要」双轨。

### P1 —— 高价值独立模块

**P1-5 确定性文风检测器 `prose_lint`**（源：oh-story check-ai-patterns，MIT + ainovel style_stats）

- 纯 Rust 检测器：blocking/advisory 两档（「不是A而是B」、破折号滥用、否定排比、章尾总结/预告、**跨章逐字重复句**、章尾短句率、开篇时间词率）；接进 `run_mini_review` 之后、editor_qc 之前，结果入 `review_result_json`。
- **工程词泄漏检测对 StoryMoss 尤其贴身**：prompt 注入了【必须改变】【本拍】【节拍】等术语，扫描这些 token 是否出现在正文。
- oh-story 的教训要带走：规则必须本地语料校准，「真人语料 20 章 0 命中才升 blocking」。

**P1-6 作者风格逆向学习**（源：ainovel `/sync style_delta` + webnovel `/webnovel-learn`）

- 幕前手改 AI 文本时（自动保存链路已有前后内容），后台提取风格增量（删了什么句式、换了什么词）入 `style_preferences` 表；`WriteTimeBundle.style_dna_summary` 槽位现成可注入；幕后设置页可视化管理。

**P1-7 伏笔系统增强**（源：MuMu 设计思想——GPL，只借思想不抄代码）

- 计划回收章与实际回收章分离（超期检测更准）；`strength`/`subtlety`；伏笔链；每章自动新建上限 5；四层注入话术（**近期待回收明确标注「请勿本章回收」**，防 AI 过早兑现）；ingest 强制返回原文 8-25 字摘录 → 伏笔卡点击跳转正文位置。

**P1-8 成本账本 + 预算哨兵**（源：ani-book Token 纪律 + ainovel BudgetSentinel）

- per-story 聚合视图 + 设置页 `book_usd` 预算 + warn 比例 + 优雅停机；ainovel 的「零增量计费盲区检测」（连续 5 笔零增量记账告警）防静默超支。

### P2 —— 工程纪律与远期

- **质量债台账**（ani-book）：QC fail-open 降级放行时把未解决问题入 `quality_debts` 表（含建议回收窗口），幕后任务中心可见、可批量触发 `auto_revise`——把 v0.59.0 的「质检可行动化」推进到「质检不静默流失」。
- **新增物三级审批**（oh-story）：ingest 自动建实体已有；金手指规则/世界观硬规则类新实体走弹窗确认（复用 V134 user_created 弹窗 UX）。
- **每 10 章检查点快照 + 时间旅行查询**（ani-book checkpoint + webnovel `entity_state_at_chapter`）：StoryMoss 的 `source_chapter` 字段已具备重放条件，补「按章节时点查询实体状态」API 即可。
- **Compass 指南针资产**（ainovel）：ending_direction / open_threads / estimated_scale——很轻，创世生成、卷末更新，防长篇大纲空洞。
- **帖主三测试固化为验收探针套件**：玉佩连续性 / 知识边界 / 级联冲突三个 e2e 场景，作为「防吃书」能力的持续回归门（契合项目契约测试文化）。
- **事件→投影声明式路由表**（webnovel）：commit_service 的隐式投影更新声明化，可测试可审计——架构级，远期。

---

## 五、明确不引进的

| 不引进 | 原因 |
|---|---|
| 文件系统 YAML 权威 + 派生视图（oh-story / ani-book 模式） | StoryMoss 是 SQLite/LanceDB 中心 + 桌面双界面，文件型权威会与 DB 形成双源；**借其「单一权威 + 派生只读 + 一致性校验」思想**（V134 source 字段已是这个方向） |
| 全自动多 Agent 引擎（ainovel 形态） | StoryMoss 定位是作者主导 + 幕前沉浸；帖主自己点出全自动磨平语气的问题。其确定性路由/穷举测试纪律值得学，形态不学 |
| 外部 Embedding API 依赖（webnovel RAG） | StoryMoss 已有本地 384 维 provider，保持离线能力 |
| MuMu 世界观四字段扁平模型 | 反面教材：世界观不结构化、且不进生成上下文 |
| webnovel v7 规格的未落地设计 | 信息差机检等只在其冻结规格里、从未实现验证——可参考思路，不可当成熟经验 |

---

## 六、许可证红线（引进前必读）

- **MuMuAINovel、webnovel-writer 均为 GPL-3.0：一行代码都不能引入**，只能借鉴设计思想（本报告对二者仅做思想层面引进）。
- oh-story（MIT）、ani-book-skill（Apache-2.0）实现可参考，但建议全部用 Rust 重写贴合现有架构。
- ainovel-cli License 文件为 Apache-2.0 但 README 写 MIT（二者不一致），若要参考其代码需先与其澄清。
- **StoryMoss 仓库根目录目前没有 LICENSE 文件**——若计划开源或引入第三方代码，需先明确自身许可证。

---

## 七、优先级总览

| 级别 | 项 | 对应帖主尺子 | 预估规模 |
|---|---|---|---|
| P0-1 | 三层信息分离 + 信息流提取 + 泄密探针 | 尺子② | 迁移 + ingest schema + prompt + 探针 |
| P0-2 | 物品归属状态机 | 尺子① | 迁移 + ingest + BeatState + 探针 |
| P0-3 | 级联影响报告闭环 | 尺子③ | 串联现有引擎 + 前端中心 |
| P0-4 | 语义摘要 + 分层金字塔 | 防吃书根基 | 摘要管线 + 自适应窗口 |
| P1-5 | prose_lint 文风检测器 | 质量确定性 | 纯 Rust 模块 |
| P1-6 | 作者风格逆向学习 | 长期文风 | 表 + 后台提取 |
| P1-7 | 伏笔增强 | 伏笔健康 | 字段 + 话术 |
| P1-8 | 预算哨兵 | 成本 | 聚合 + 设置 |
| P2 | 质量债 / 三级审批 / 快照 / Compass / 三测试套件 | 工程纪律 | 各自独立小项 |

**一句话结论**：StoryMoss 的检索基建与节拍卡/导演锁体系在这 5 个项目里处于第一梯队，真正的差距集中在帖主点名的三件事上——**物品归属状态机（无）、角色知识边界的更新与校验闭环（冻结在半路）、改稿级联的影响报告（引擎齐备但没串起来）**；外加一个外部共识但 StoryMoss 尚未起步的「确定性文本质检层」。P0 四项全部落地后，帖主的三个测试均可固化为可回归的验收探针。
