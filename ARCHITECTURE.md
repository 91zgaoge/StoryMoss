# StoryMoss (草苔) v0.65.0 架构文档

> **v0.65.0**：**人类文笔基线**——`story_system::human_voice`（纯函数、0 LLM）把「AI 写多了 / 人写少了」的分布偏移量化为 `VoiceMetrics` + `VoiceFinding`（excess/deficit 两组）：`flat-rhythm`（段内连续 ≥3 句近等长，唯一跨语言方向一致的句法指标）、`mood-particle-absent`（有对话零语气词，中文人类密度 5 倍）、`embodied-only`（具身化 ≥3 且零平直命名）、`connective-stack` / `disyllabic-padding` / `fancy-speech-tags` / `abstract-wrapper` / `uniform-paragraphs`；**方向矛盾项不入规则**（标点密度、平均句长、段落数、词汇多样性）。注入三处：`agency::beat_card::render_writer_user_prompt`（`render_guidance_block_for_progress`：7 条固定准则 + 8 条手法池按已写篇幅轮换，置于末句锚点之前）、`agents::service::render_writer_system_from_bundle`（常驻准则追加，不受用户模板覆盖影响）、`agency::coordinator` 编辑器资产（`render_audit_block`，与 `prose_lint` 并列）。记录：`story_system::commit_service::auto_commit` 把发现并入 `review_result.issues`。校准：`prose_lint::lint_text` 的 `not-x-but-y` 改为聚集（≥2 处）才 blocking；`anti_ai::{check_emotion, check_dialogue, check_syntax, check_narrative}` 按实测方向重写（平直命名情绪不是缺陷、重复「说/道」是常态、感官密度低不是 AI 指纹、句法看离散度），并修复对话提取只认弯引号导致「」体例失效。
>
> **v0.64.12**：**自动后台重算 + 指纹感知**——V145 `scene_commits.summary_source_hash` + `chapter_summary::content_fingerprint`（剥标记后的 FNV-1a）；`recompute::{recompute_scoped, recompute_after_commit, should_auto_recompute, spawn_auto_recompute_if_stale}`（后台闸门 + 同故事并发去重 + 单轮章数上限 + `AppConfig::auto_recompute_after_edit` 开关），挂载点 `story_system::scene_service::schedule_commit_and_split` 的 auto_commit 成功分支；`SceneCommitRepository::set_summary_source_hash` 在 `auto_commit` 落库。
>
> **v0.64.11**：**物料失效重算 / 关系归一 / 摘要可见**——V143 `story_material_staleness` + `story_system::recompute::{mark_stale, list_stale, recompute_from}`（章节摘要按当前正文、分层摘要与全书纲要删旧重建、快照重写、`relation_retract::audit_unsupported_relations` 入质量债），`update_scene` 编辑即标记；`db::relation_kind::RelationClass`（受控词表 + 位标志）+ V144 两列回填，接入 `beat_card::compile_conflict` 与 `continue_director::{rel_is_kin, rel_is_spouse}`；`story_system::relation_retract::retract_relations_for_scene` 按 `kg_relations.evidence` 摘证据/删行；`chapter_summary::summarize_chapter_with_quality` 与 `segment_summary::SegmentRefreshReport` 把总结类失败并入质量债。
>
> **v0.64.10**：**冲突阶梯**——`agency::beat_card::{ConflictStage, conflict_line, previous_conflict_line}` 四阶梯（加压→升级→结账→余波），`compile_beat_card_located_prev(pool, story_id, content, loc, previous_outline)` 以当前场景的 `outline_content`（上一拍卡块）为阶梯依据，Append / observe / 批量三条路径接入；`ConflictMove.stage` 驱动 `compile_change_delta` 的必须改变项；`agency::beat_state::{conflict_outcome_landed, conflict_repeats_prior}` 两条探针缺口（升级未兑现 / 对峙原地复述，后者用新增的 `utils::text::TextUtils::char_bigram_similarity`）。
>
> **v0.64.9**：**续写不重演 + 投影恢复**——`agency::continue_assets::chapter_plan_from_scene_outline` （只剥节拍卡块形态，保留手写/刷新出的场景大纲）与 `chapter_plan_missing`；`agency::prose_ground::is_generic_next_node`（方法论兜底句不落库、不被下一拍采信）；`agency::persist::scene_fields_from_facts` 改为 `merge_current_scene_outline`（章纲前缀保留，卡块并入）；`agency::observe::set_chapter_plan_prefix`（Append 新章补章纲落库为前缀，coordinator `needs_plan` 触发 `generate_chapter_outline`）；`story_system::projection_writers::normalize_delta_items`（state/index 兼容 KG 实体/关系视图等四种 delta 形态，替代无生产者的历史反序列化目标）。
>
> **v0.64.7**：**角色生死状态持久化（死人不复活）**——`utils::death_text`（纯文本判定，`name_is_dead_in_text`/`dead_names_in_text`，含否定句豁免；从 agency 下沉为叶子模块，db 与 story_system 共用）+ `db::character_life`（`characters.life_status`/`death_chapter` 列读写、`mark_dead_on`/`revive_on`、`backfill_story` 存量回填、`annotate_physical_state` 标记）+ `story_system::life_status`（`refresh_after_text` 整章扫描、`dead_names`/`dead_marker_map` 供注入路径、`revive` 作者改判）。接入点：`commit_service::auto_commit` 与 `scene_commands::update_scene` 即时刷新；`agency::beat_card`（cast 排除 + 下一拍候选）与 `creative_engine::write_time_bundle`（角色卡「已死」标记）消费；`post_process` 单调守卫。V142 迁移加列并回填。
>
> **v0.64.6**：**人物称呼归一**——V140 `character_aliases`（UNIQUE(story_id, alias)）登记「称呼 → 人物」；`db::character_identity::{resolve_character_id, record_aliases, merge_characters}` 提供建行前解析（精确名 → 别称 → 称号形态）、同名幻影行自愈与合并（改线 character_states / character_relationships / scene_characters / scene_character_actions / character_knowledge_log / item_holdings，关系与场景关联去重，改写 scenes/scene_versions 的 `characters_present`/`character_conflicts` 名字 token，补齐保留行空字段）；`continue_director` 称号词表扩充 + `same_person` 新增「称号+名」形态；抽取 prompt 增「人物称呼归并」段与 `aliases`；V141 迁移按形态合并存量。

> **v0.64.5**：**一章一条 commit 的 UPSERT**——`SceneCommitRepository::upsert_pending`（配 `get_by_story_chapter`）取代 `init_commit` 里的裸 INSERT：同章已有 commit 时复用该行（保留 id，投影按 story+chapter 幂等重跑）、重置 `pending`、`COALESCE` 刷新 scene/chapter 挂载、清空派生字段（摘要 / review / 状态增量 / 投影状态），避免重新提交窗口内旧摘要被下游当当前章事实。修复真机 `UNIQUE constraint failed: scene_commits.story_id, scene_commits.chapter_number`。

> **v0.64.4**：**模型失败可见性与探测韧性**——`agency::coordinator::write_beat_once` 的模型调用失败不再降级成空文本（此前被「过短」分支接管，真实原因丢失），改为立即带原因失败；`model_gateway::executor` 新增 `should_attempt_after_probe_timeout`——起跑前 5s 探测超时的候选若已是**最后一个**则不再跳过、真打一次（自带 60s 超时与重试），后面还有候选时保持快速回退（单槽推理服务在忙时探测会排在生成后面，超时≠不可用）。`components/StreamOutput.tsx` 渲染前合并悬挂闭合引号。

> **v0.64.3**（未上传安装包，内容并入 v0.64.4）：**段首收尾符修复**——`format.ts::splitChineseSentences` 让句末标点后紧跟的收尾符归属本句（段首孤引号的源头）；新增段落级 `mergeLeadingClosingPunctParagraphs`（前端）与 `TextUtils::merge_leading_closing_punct_paragraphs`（Rust，供 V139 迁移），把段首闭合标点并回上一段，只认有方向的收尾符（ASCII `"` `'` 可能是开引号，保持原样）。`autoFormatText` 三条路径与 `textToParagraphsHtml` 统一走「孤段 + 段首」合并。

> **v0.64.2**：**关系不变量守卫**——`agency::continue_director::{sanitize_relations, sanitize_bundle_relations}` 在注入前拦下矛盾关系行（同一对人物既是血亲又判夫妻；单人被写成 ≥3 人配偶），人物锁【本拍人物关系】与关系表【角色情感关系】两条注入路径共用同一判定，`DirectorLock.relations_dropped` 承载被拦下的行并计入质量债（`continue_relations`）。配套修掉配偶启发式的整串匹配 bug（改为逐段匹配对方姓名 + 配偶信号）与落库覆盖策略（只允许修脏 / 填空，不覆盖既有的父子 / 兄妹 / 同僚）。探针遗留缺口改入质量债（`continue_probe`）。

> **v0.64.1**：无架构变更（landing 兜底版本与测试加固；发布流程修正为 GitHub + Cursor 代理双 remote 推送）。

> **v0.64.0**：**声明式投影路由表**（P3-F）——`projection_writers::{CommitArtifact, ProjectionWriterKind, PROJECTION_ROUTES}` 把「提交产物 → 投影 writer」变成纯数据表；`get_projection_writers` 按表构造（漏接线告警）、`projection_status_keys()` 派生 commit 状态键（新增 writer 自动进 pending 初始化）、`route_summary` 记录每次提交的路由摘要（on/off + 激活 writer）供审计。异步 writer（vector / kg）在表内以 `deferred` 声明，执行仍由 `apply_commit` 的 future 完成——行为保持的重构。前端新增「运行维护」页（`pages/Maintenance.tsx` + `hooks/useMaintenance.ts`）：质量债 / 待确认 / 文风偏好 / 成本四 Tab，配套命令 `list_style_preferences` / `set_style_preference_status`。修复 `llm::cost` 的 deny 级 clippy 问题（曾阻塞 v0.63.0 发布）。

> **v0.63.0**（P3 工程纪律）：**V138** 三表——`quality_debts`（UNIQUE(story,chapter,detail) 幂等；`story_system::quality_debt`，coordinator 的 spawn_editor_qc 在 RevisionRequired/salvage/异常降级时记账）、`story_checkpoints`（UNIQUE(story,chapter)；`story_system::checkpoint` 在段摘要刷新的 10 章边界写快照，`query_as_of` 用 append-only 知情流水 + `reveal_chapter` 做「截至第 N 章」回溯，物品账本标注 current_only）、`pending_reviews`（UNIQUE(story,kind,subject)；ingest 对 importance ≥7 的世界规则入队，确认前不作硬约束）。`story_system::compass` 确定性派生终局方向/活跃长线/进度并经 continuity_blocks 注入。命令：list/resolve_quality_debts、query_story_as_of、list/resolve_pending_reviews。组合契约见 `tests::three_rulers_contract_test`。**未实施**：P3-F 声明式投影路由表。

> **v0.62.0**（P2 文本质量与成本）：`story_system::prose_lint`（纯规则引擎，blocking/advisory 两档）在 `auto_commit` 并入 review 记录、在 editor_qc 预注入核对块。**V137**：`foreshadowing_tracker` 增 `evidence/strength/subtlety/related_foreshadow_ids`（`ForeshadowingRecord` 同步暴露 target_start/end 与 evidence，注入话术按计划窗口分档）；新表 `style_preferences`（UNIQUE(story_id, pattern)）承载作者文风偏好——`story_system::style_learning` 在 `update_scene` 人类编辑后防抖 120s、单故事单处理器（PENDING/RUNNING 两个进程内表）提炼（资产 `style_delta_extraction`，标签「后台风格提炼」），`render_style_block` 经 continuity_blocks 进入续写资产。`llm::cost` 按故事聚合 llm_calls 并提供零增量计费盲区检测，命令 `get_story_cost_summary`。

> **v0.61.0**（P1 记忆质量）：**V136** `story_segment_summaries`（level segment/book，UNIQUE(story_id,level,segment_index)）承载段摘要与全书纲要；`story_system::chapter_summary` 把 commit 摘要升级为 LLM 语义摘要（`parse_summary_response` 形态校验，失败回退截断，标签 `background-summary`）；`story_system::segment_summary` 负责段数学（`completed_segment_count` / `segment_bounds`）、区间章摘要收集、段/书摘要生成与 `spawn_refresh_after_commit` 后台补齐（挂接 `schedule_commit_and_split` 的 commit 成功分支）。`memory::orchestrator::adaptive_summary_window`（10/5/3）替换写死的 3 章窗口，工作记忆按书长注入最近段摘要与全书纲要；`render_story_so_far_block` 经 `write_beat_once` 的 continuity_blocks 进入续写资产【故事纲要】。

> **v0.60.0**：P0「三把尺子」。**V135** 四张表：`story_timeline_events`（objective_fact / reader_knowledge / reveal_status 三层信息分离）、`character_knowledge_log`（知情变更审计）、`item_holdings`（物品归属账本）、`cascade_impacts`（改稿影响，batch 分组 + decision/stale 状态）。**知识边界**：ingest 新增 `knowledge_updates`/`timeline_events` 抽取并落库（`memory::continuity::persist_*`），secrets 不再是 COALESCE 冻结值；续写侧 `render_continuity_blocks` 注入【本拍信息差】【未公开真相】（`planned_text_for_card` 豁免计划内揭示），`detect_knowledge_leaks` 经 `continuity_probe_gaps` 接入 `write_beat_once` 探针，editor_qc 预注入疑点清单；`domain::asset_snapshot::CharacterStateSnapshot` 保留 secrets，adapter 不再丢弃。**物品归属**：`persist_item_holdings` 按 (story,item) upsert；`detect_possession_conflicts` 探针（使用动词窗口 + 当场转手豁免 + 遗失物再现）。**级联**：`creative_engine::cascade_rewriter::impact_report`——`compute_mention_impacts`（下游 + 无处不在实体过滤，`score = Σconf × √count`）→ `persist_impacts` → `scan_conflicts_with_llm`（提示词资产 `cascade_conflict_scan`，label「后台级联扫描」静默）→ `SyncEvent::CascadeImpactDetected`；`SceneIngestor::spawn_ingest_now` 完成后自动 spawn，四命令（list/ignore/reanalyze/trigger_for_impact）经 handlers 注册；前端「级联中心」页只呈现与决策，**不自动改写后文**。

> **v0.59.4**：无架构变更，修正发布纪律：发布 tag 增加「必需文档已更新」门禁；landing 兜底版本回退到网站上真实存在的版本（Apple 公证协议未签署 → 0.59.x 尚未发布，兜底不得指向 404）。

> **v0.59.3**：`story_outlines` 加 `source`（V134）。作者手写/确认（`user_created`）时：创世 `materialize` upsert 带 `WHERE source <> 'user_created'` 不覆盖；资产回流 `sync_story_delta` 跳过不追加；`StoryOutlineRepository::update` 仅在带内容时打标。存量行 `unknown`（机器仍可精炼）。删死模块 `memory/hybrid_search.rs` 与 capability 死权限 `http:default`。

> **v0.59.2**：`useScenePersistence` 增加载入期空写保护（`markSceneContentLoaded` + `isEmptyEditorHtml`）：后端非空正文载入后，编辑器自带空文档（`<p></p>`）不得落库覆盖；首次非空保存自动解除。`narrative::strip_whitespace_trailing_commas` 修 `,` + 换行 + 闭合括号。E2E 提升为阻塞门。

> **v0.59.1**：无架构变更（对齐新版 nightly rustfmt，106 文件纯折行）。

> **v0.59.0**：迁移框架记内容校验和 + 集合水位线（`schema_migrations.checksum`，V132）；迁移目录只在源码树取（构建产物路径被剔除）。后台资产回流/质检注册 run 级附属取消标志并入 run 预算（`AGENCY_ANCILLARY_FLAGS`）。续写质检结果事件带 `mode`/`chapter_number`，幕前可一键走 `auto_revise(revision_type=editor_qc)` 修订本章。删故事级联清理 `agency_*`/`ingest_jobs`/`llm_calls`；`agency_activity_log` 30 天保留。`FrontstageApp` 保存链抽为 `useScenePersistence`。v0.58.0 的戏剧工艺与短剧格式不变量不变。

> **v0.58.0**：戏剧工艺来自 AI-drama-pound（MIT），不嵌对方进程。`SceneBeatCard.change_delta` 0 LLM 编译。`assemble_continue_beat_for` 按 `stories.story_format` 选 `CONTINUE_BEAT_SYSTEM` 或 `DRAMA_BEAT_SYSTEM`。`probe_increment_ex` 复述近文 + 未兑现改变项才 gap。`CreateStoryRequest` 不加字段；创世后 `update_story_format`。续写 `tools=None`、`scenes.content` 真相源不变。v0.56.2 孤引号 / fail-open 不变量不变。
>
> **v0.56.2**：`mergeHangingClosingPunct` / `merge_hanging_closing_punct` 跳过换行与闭合标点之间的空白/全角缩进/零宽字符。`autoFormatText` 空行路径也跑 `mergeLoneClosingPunctParagraphs`。后台 `editor_qc_done_detail` 固定「后台审查」，不合格走 toast。v0.56.1 探针不变量不变。
>
> **v0.56.1**：`subject_split_gaps` 改为同一人 ≥2 个称呼独立出现即缺口（不再只认「则/蜷缩」）。`dead_acting_gaps`：锁里已死之人点名后 80 字内眼睛/锁定/审视则缺口，尸体窗口除外。续写 `tools=None` 不变。v0.56.0 导演锁不变量不变。
>
> **v0.56.0**：续写 `write_beat_once` 在 `complete()` 前编译 `DirectorLock`（Rust 必跑，Producer enrich 可选、失败回退 Rust）。主创 `tools=None`。冻结快照含锁。`probe_increment` 拦拆身/亲缘写反，不罚沉默在场。`materialize_assets` 先角色后关系、同对 upsert。`ensure_assets` 在 ≥2 角色且关系表空时 `ensure_relationships`。不 DELETE 角色行。v0.55.0 短合同 / 冻结不变量不变。
>
> **v0.55.0**：`catalog_for_role` 只注入工具名+一行，JSON Schema 仍走 `GenerateRequest.tools`。`asset_read` 按名取本故事全卡。`ContinueFreezeMap` 在一次 `write_beat_once` 内冻结节拍卡/user。`CONTINUE_BEAT_SYSTEM` 为 11 行操作合同 + 三条对错范例。续写 `tools=None` 不变量不变。v0.54.0 原生 tool_calls 不变量不变。
>
> **v0.54.0**：Agency ToolLoop 原生 function calling。`GenerateRequest.tools` 经网关透传到 OpenAI / Ollama `/api/chat` / Anthropic；`resolve_loop_action` 原生优先、文本 JSON 回退。续写 `write_beat_once` 的 `tools=None`，适配器请求体不含 `tools` 字段。不改 PersistMode / 三档路由 / `scenes.content` 真相源。v0.53.6 高潮不重演不变量不变。
>
> **v0.53.6**：近文死亡检测把已死之人移出节拍卡行动阵容；`compile_next_node` 跳过已在正文完成的行刺高潮；`probe_increment` 拦把已死者再刺一次。不改 PersistMode / Agency 唯一续写路径。v0.53.5 大纲确认框不变量不变。
>
> **v0.53.5**：含大纲的 `asset_refresh` 返回 `asset_refresh_draft`，确认前不写 `story_outlines` / `scenes.outline_content`。`confirm_asset_refresh` 按手改落库。重写再走 `smart_execute`。纸面只读不变量不变。v0.53.4 写/生成宾语是大纲的形状检测不变。

> **v0.53.4**：`looks_like_asset_refresh_shape` 在写/生成的宾语是大纲时成立，不再要求出现「正文」。`apply_asset_refresh_override` 仍盖掉续写兜底。「写后续」「按照故事大纲写后续」「根据正文重新生成下一章」不进本作业。v0.53.3 幕前弹窗与纸面只读不变量不变。
>
> **v0.53.3**：`summarize` 把落库大纲正文带回 `final_content`；幕前 `result_kind=asset_refresh` 弹「已按正文重写设定」，仍不 `appendAiContent`。`build_refresh_prompts` 对「后续」要求从章末下一拍写，禁止复述开场。v0.53.2 键值散文解析不变量不变。
>
> **v0.53.2**：`parse_refresh_payload` 在 JSON 失败后解析无花括号键值散文（`story_outline:` / `scene_outline：`）。salvage 覆盖仅场景大纲。`looks_like_asset_refresh_shape` 认「重新生成」且须点名资产。v0.53.1 对象大纲不变量不变。
>
> **v0.53.1**：`parse_refresh_payload` 不再要求 `story_outline` 为字符串。对象走 `normalize_outline`；中文键别名；仅故事大纲时散文 salvage。解析失败打 warn 预览。v0.53.0 路由与纸面只读不变量不变。
>
> **v0.53.0**：幕前「按正文重写设定」在 `smart_execute` 判断 Agency Append **之前**走 `agency/asset_refresh.rs`。`AssetTaskType::AssetRefresh`；Producer `complete_json`（WorldBuilding/Tool 档），无 writer、无 tool_loop、不创建挡住续写的 run。落库只写点名靶：`story_outlines` 整份替换 + cap；角色/世界观源感知合并；场景大纲 `merge_current_scene_outline`。禁止 `update_scene.content`。前端 `result_kind=asset_refresh` 不 `appendAiContent`。v0.52.0 拍级分层不变量不变。
>
> **v0.52.0**：拍级编译器引进 OpenViking 式分层与轨迹，不嵌其运行时。`PRIOR_CAST_CHAR_CAP=1500`；任务文案点名准入；在场/冲突 L2、其余准入 L1；`story_outlines` 回流封顶。不改 `WriteTimeBundle::to_prompt()`。
>
> **v0.51.6**：`GatewayExecutor::generate` 在 `select_candidates` 角色置顶之后，不再把当前活跃/创作模型抬回工具档、后台档链头。创作档仍置顶。主创→Creative，管理→Tool，编辑审计与资产回流→Background。无落库变更。
>
> **v0.51.5**：进行中的 Agency 续写（`active_run`）不是 UserAction 中断。`AgentInterruptionModal` 对此错误不渲染；二次点击不盖纸面。无路由/落库变更。
>
> **v0.51.3**：`condense_story_outline` 切断规划提纲后再注入续写；`write_beat_once` 过短重试受剩余 deadline（90s）约束；网关 `CANCELLATION` 终止候选链。v0.51.2 节拍卡切断不变量不变。
>
> **v0.51.2**：`detect_and_strip_bare_cot` 增加节拍卡/约束清单强切断（全文扫描，从首个命中行切到文末）。续写 `CONTINUE_BEAT_SYSTEM` 禁止输出任务分析。sanitize 清空后既有 write_beat 过短重试仍生效。无路由/落库契约变更。v0.51.1 墨纸按钮不变量不变。
>
> **v0.51.1**：幕前取消键 / 发射键卸掉 macOS WKWebView Aqua 原生按钮外观（`appearance-none` + flush CSS 双杀）。无架构/生成链路变更。v0.51.0 观察不变量不变。
>
> **v0.51.0**：手写/粘贴正文触发三角色观察。`agency/observe.rs` 与自动分章同一 30s 空闲窗口；`decide_post_commit_work` 分流 Observe / Ingest / Skip。观察 run id=`observe-{story_id}`，status=`observing`/`idle`（不用 pending/running，以免撞 V109）。主创不写 `scenes.content`、不加续写拍数。编辑 `bg-observe-editor` 静默且不发 `genesis-qc-result`。禁止 `observe` → `story_system`（由 scene_service 调观察）。验证：`cargo test --lib` 1463 passed / 2 ignored（+9）；`npx vitest run` 592 passed / 3 skipped（+1）。v0.50.2 分章标题/Append 前缀守卫不变量不变。
>
> **v0.50.2**：自动分章重排后派生标题跟随 `chapter_number`（`is_generic_chapter_title`）；幕前 `displayChapterTitle` 对「第N章 / 第一章」按章号显示。`append_base_content`：DB 为客户端前缀且多出 ≥200 字时用截断后的 DB，避免把溢出写回。V130 修存量标题。不自动删除已重复正文。验证：`cargo test --lib` 1454 passed / 2 ignored（+4）；`npx vitest run` 591 passed / 3 skipped（+1）。
>
> **v0.50.1**：自动分章后幕前 `sceneId` 曾回落 `chapter.id`，Agency Append 在 scenes 表找不到并误报「请先打开一个章节」。分章切章补拉 `get_chapter_scenes`；`resolve_append_scene_id` 与 `update_scene` heal 同口径（id 是 scene 原样，id 是 chapter 则取该章关联 scene），贯穿 `run_continue_inner` 与 `persist_append`。禁止猜最新有内容场景。验证：`cargo test --lib` 1450 passed / 2 ignored（+1）；`npx vitest run` 590 passed / 3 skipped。v0.50.0 三角色闭环不变量不变。
>
> **v0.50.0**：续写三角色闭环。`agency/continue_loop.rs` 把后台活动写入 `agency_activity_log`（spawn 即 start，退出必 done）；Append 后把 BeatCard 投影到当前 run Asset 区，ingest 再补正文接地的角色/故事/世界观。`scenes.outline_content` 写成 `【当前场大纲】`，`compile_next_node` 优先读 `下一拍`；ingest 见该标记不覆盖。`load_open_review_issues` 跨 run 取最多 2 条 `gate:revise` 进节拍卡。不改 `WriteTimeBundle::to_prompt()`，不接 ContextPrioritizer，不 DELETE 脏角色行，不自动改写已落库章节。验证：`cargo test --lib` 1449 passed / 2 ignored（+13）；`npx vitest run` 590 passed / 3 skipped（+2）。真机未跑，不得宣称唱反调已修复。
>
> **v0.49.1**：卸掉幕前划词浮条。`RichTextEditor` 不再挂载 `AiSelectionActions`；组件从 `components/ui/ai/` 删除。选区只服务右键菜单与 `insertText` 当前选区替换。底部 `AiPromptBar` 仍是改写入口。v0.49.0 续写正文真相源不变量不变。验证：`npx vitest run` 588 passed / 3 skipped（−17）。无 Rust 逻辑变更。
>
> **v0.49.0**：有实质正文时资产真相源是 `scenes.content`，不是书名。`agency/prose_ground.rs` 姓名门闩 / `outline_is_grounded` / 默认 `scene_structure`。`ensure_assets` 跳过标题发明 Producer tool_loop；`producer_out.aborted` 不 Err。`ensure_story_outline` 有正文走方法论归纳。`compile_next_node` 未接地当空。`probe_increment` 拦场外开篇。不改 `WriteTimeBundle::to_prompt()`，不接 ContextPrioritizer，不 DELETE 脏角色行。验证：`cargo test --lib` 1436 passed / 2 ignored（+18）。真机未跑，不得宣称唱反调已修复。
>
> **v0.48.1**：划词浮条 `AiSelectionActions` 改为 ≥4 字且 `mouseup` 后才挂载；idle 默认无 `<input>`（点「指令」才展开）。Esc 塌选区。写作主路径不再被改写条挡住。v0.48.0 续写镜头/落库不变量不变。
>
> **v0.48.0**：续写按镜头在场。`PRIOR_CAST_CHAR_CAP=500` 与散文近文 1800 分离；`compile_beat_card` 不再按角色表补位；`compile_next_node` 吃当前正文，跳过不点名本拍在场者的书大纲句；`persist_append_inner` 底稿取 DB 与客户端更长者；`probe_increment` 在 `NewScene` 时不罚丢掉已在场者；幕前连续续写先 `appendAiContent` 未确认幽灵。验证：`cargo test --lib` 1418 passed / 2 ignored（+5）。v0.47.0 债务兑现 / 节点不回绕 / 事实写回不变量仍成立。
>
> **v0.47.0**：续写质量闭合。`touch_refresh_beats` 只在正文兑现冲突/阵容/地点时刷新；`compile_next_node` 禁止回绕大纲首句；`compile_conflict` 只点名本拍 cast；`persist` 写回增量点名（`match_character_names` 含别名）；`write_beat_once` 过短回退复用 `assemble_continue_beat`，不调用创世 `writer_prose_fallback`。新增 `agency/beat_state.rs`：状态网头尾双锚、末句锚点降权、探针至多一次 `complete()`。`render_continue_assets` 拼接 `active_conflicts`/`character_goals`。NextChapter 装配先算 `SceneUpdate` 再开写事务（避免持 tx 时 `pool.get()` 读 scenes 触发 SQLite unlock_notify 死锁）。不改 `WriteTimeBundle::to_prompt()`、`ExpansionDebt::quota_text()` 原文、`PersistMode`。验证：`cargo test --lib` 1413 passed / 2 ignored（+22）。v0.46.0 传统色不变量不变。

> **v0.46.0**：传统色主题。12 id（`zhuqing`…`hanxiulv`）各一份亮纸（幕前）+ 一份暗机械（幕后）。存储 `storymoss-color-theme-front` / `-back`；事件 `{ surface, id }`。不变量：幕前 `gold === terracotta`；幕后 `--cinema-gold` 为该色暗面 brand，印色只进 `--cinema-velvet`；改幕前不写 back、不 `applyBackstageTheme`。`--ai-accent-tint` / `--ai-on-accent` 跟随当前窗。不引入 `--dsw-*`。验证：`npx vitest run` 601 passed / 3 skipped（+11）。v0.45.1 续写前文双窗不变量不变。

> **v0.45.1**：续写前文窗口。`slice_prior_prose` 先 `strip_editor_markup`；短章全文，长章开篇 600 + 近文 1800。`compile_beat_card`「末段已在场」改看 `prior_tail_for_cast`。`ending_anchor` 用纯正文。`apply_asset_budget` 裁【前文】从尾截。不叠更早几章正文。验证：`cargo test --lib` 1391 passed / 2 ignored（+6）。v0.45.0 组装器不变量不变。

> **v0.45.0**：提示词运行时组装。`prompts/assembly.rs` 的 `assemble()` 只按槽位拼接 `Layer` 文本，不依赖 `agency`。创世 `writer_first_chapter` / `writer_prose_fallback`、续写 `write_beat_once`、`ToolLoop::run` 的 head 经工厂接线；上下文仍由 BeatCard / `render_writer_user_prompt` 编译。`WriteTimeBundle::to_prompt()` 不动。场景预览默认 `agency_continue`，`timesliced`/`trishot_call3` 映射热路径。architecture_guard：`prompts` ↛ `agency`。验证：`cargo test --lib` 1385 passed / 2 ignored（+18）；`npx vitest run` 590 passed / 3 skipped。v0.44.1 输入无原生描边不变量不变。

> **v0.44.1**：幕前 flush 输入去掉 WKWebView `<textarea>` 原生 inset 边（`appearance-none border-0 shadow-none` + CSS 双杀）。v0.44.0 只拆了外壳卡片，测试未锁输入框本身。验证：`npx vitest run` 590 passed / 3 skipped（+1）。无 Rust 逻辑变更。

> **v0.44.0**：墨纸 / 机械视觉定向进化补齐。幕前输入壳去掉顶边与毛玻璃；霞鹜 Medium 独立 woff2（v1.250 Bold 映射 CSS 500）；纸 `--parchment*` hue 95；选区陶土 22%；顶栏图标按钮 press/淡彩。幕后 warm cinema 850–500 三方同色相；Panel inset 顶高光；`--transition-spring` 0.5s，接线带 `motion-reduce`；侧栏选中无金框。纯前端，`--ai-*` 17 变量名不变。验证：`npx vitest run` 589 passed / 3 skipped（+11）。v0.43.0 flush / 本地 Regular / 陶土发射不变量不变。

> **v0.43.0**：墨纸 / 机械视觉定向进化。幕前 `AiPromptBar` 增加 `flush` 变体（底栏一层纸面、陶土淡彩发射、取消无 pulse）；霞鹜文楷 `@font-face` 本地 woff2，去掉 HTML 字体 CDN。幕后 token / `backstageThemes` 对比与阴影收软；`--transition-press`；空态 `EmptyHint`。纯前端，`--ai-*` 17 变量名不变。验证：`npx vitest run` 578 passed / 3 skipped（+22）。v0.42.0 续写资产选取不变量不变。

> **v0.42.0**：续写按拍选取资产。Agency 热路径不再调用 `WriteTimeBundle::to_prompt()` 全量倾倒；新增 `agency/continue_assets.rs`（0 LLM）按 SceneBeatCard 准入名单渲染完整卡（≤8）、未上场一行名单、大纲去重、前文 800 字、资料预算 6000。`to_prompt()` 仍供改写 Full 路径使用。验证：`cargo test --lib` 1367 passed / 2 ignored（+13）。v0.41.2 超窗跳过与单章不回落 tool_loop 不变量不变。

> **v0.41.2**：续写超时止血。`GatewayExecutor::generate` 候选循环用 `candidate_fits_prompt`（`prompt_chars/2` + 256 token 补全预留）跳过装不下当前提示的模型，补上路由器 `estimated_input_tokens==0` 与活跃模型插回首位后的漏洞。`write_beat_once` 散文回退失败直接返回，单章不再回落 `write_chapter` tool_loop（批量续写仍走该路径）。验证：`cargo test --lib` 1354 passed / 2 ignored（+4）。v0.41.1 核验不变量不变。

> **v0.41.1**：对照设计核验后的续写加固。`write_beat_once` 在单次 `complete()` 后走 `sanitize_novel_output`，自重复 ≥8% 再 complete 一次；`execute_writer` 经 `resolve_rewrite_generation_mode` 只选 Fast/Full，历史 time_sliced/tri_shot 映射过去，永不选 TimeSliced/TriShot；`should_agency_append_continue` 在有划词选区时禁止 Append。测试环境 `finalize_session` 跳过 LLM 摘要。验证：`cargo test --lib` 1350 passed / 2 ignored（+5）；`npx vitest run` 556 passed / 3 skipped。v0.41.0 路径不变量不变（创世/续写只走 Agency；幕前 Append；幕后 NextChapter）。
> **v0.40.0**：AI 原生组件库 P3（数据展示）+ P4（收尾），设计文档 P1-P4 四阶段全部收官。①P3：6 个数据展示组件（AiSearchList/AiCodeBlock/AiDiffTable/AiFilterTable/AiRecordsTable/AiInsightCards）适配为受控组件入库 `src-frontend/src/components/ui/ai/`，逐点替换幕后落点（PromptsPanel 搜索计数区与分组列表、UsageStats 分组筛选/最近调用表/统计卡、AgencyEval 检查点对比/双表/统计卡、Logs 级别筛选、TracingPanel/Mcp/Skills/IntentionGraphDiagnostics 裸 pre/JSON 块）；沿用 P1 令牌桥零扩令牌，liveline 以组件内嵌 SVG MiniLineChart 静态快照替代；「AiChat」经勘察关闭（ChatComposer 为 AiPromptBar 严格子集、无多轮对话场景）。②P4：删除 P1-P3 替换残留 TS 13 处与 frontstage 死 CSS 约 40 类、历史死件 8 件；新增第 17 个语义令牌 `--ai-on-accent`（替换四组件 text-white 直写），`/N` 透明度修饰符失效 13 处改 color-mix 内联；AgencyEval/AgencyStudio/AgencyLearning 浅色页令牌化（gray 映射固化为约定）。纯前端，无后端改动。验证：`cargo test --lib` 1328 passed / 2 ignored（不变）；`npx vitest run` 556 passed / 3 skipped（+33）。
> **v0.39.0**：AI 原生组件库 P1+P2 共 10 组件 + 保存 UNIQUE 修复。①P1 生成体验五件套（AiLoading/AiThinking/AiStreamingText/AiPromptBar/AiApprovalCard）与 P2 代理与任务五件套（AiContextCards/AiToolChips/AiRecommendationCard/AiTaskRows/AiSelectionActions）适配为受控组件入库 `src-frontend/src/components/ui/ai/`，逐点接入幕后/幕前落点；全部组件只引用 `--ai-*` 语义令牌（幕后 tokens.css / 幕前 frontstage.css 各自定义），不引新依赖，纯前端无后端改动。②保存修复：幕前自动分章等场景持过期 `chapter.id` 作 sceneId，自愈补建逻辑盲目 INSERT 撞 `UNIQUE(story_id, sequence_number)`；改为章节已有关联 scene 时重定向 update、序号被占时取 MAX+1 补建（`scene_repository.rs::heal_missing_scene_in_tx`）。验证：`cargo test --lib` 1328 passed / 2 ignored（+2）；`npx vitest run` 523 passed / 3 skipped（+68）。
> **v0.38.2**：代理工作室实时动态持久化 + 前端轮询。v0.38.0 将 agency 事件监听提升到常驻 `App.tsx` 顶层 + 全局 `agencyActivityStore`（Zustand，cap 200，无 persist），接线正确但用户仍看不到实时动态。根因：活动事件（`agency-agent-activity` / `agency-run-progress`）纯内存，macOS 隐藏 WKWebView 窗口事件送达不可靠，事件丢失即永久丢失。修复：①新增 `agency_activity_log` 表（V129 迁移），`emit_activity` / `emit_progress` 在 `app.emit()` 后 `tokio::task::spawn_blocking` fire-and-forget 写 DB（不阻塞创世流程，失败仅 `log::warn!`）；②新增 `agency_list_activities` Tauri 命令（`run_id` -> 按 `id ASC` 返回 `Vec<AgencyActivityLogEntry>`，limit 200）；③前端 `AgencyStudio.tsx` 新增 `useQuery(['agency-activities', runId], listActivities, { refetchInterval: 3000 })` 3s 轮询，DB 活动事件为主源，live store 事件补充轮询间隔内新事件（按业务键 `role|action|detail` / `phase|status|message` 去重）；④`App.tsx` 事件监听 + `agencyActivityStore` 保留不变（双保险）。验证：`cargo test --lib` 1326 passed / 2 ignored（+1）；`npx vitest run` 455 passed / 3 skipped。
> **v0.38.1**：修复续写伏笔账本多字节中文切片 panic（文思活跃模式）。`foreshadowing_service.rs` 构造伏笔账本 title 预览时 `&content[..30]` 按字节切片，中文 content 的 byte 30 落在三字节字符内部 -> Rust UTF-8 panic -> 续写 bundle 加载失败（文思活跃连续续写读伏笔账本，每次必炸）。改为按字符数截取（`chars().take(30)`）；同类 `post_process.rs` 两处 `&draft_content[..8000/6000]` + `intent.rs` `&content[..min(200)]` 改 `floor_char_boundary`（保留字节预算，切点回退最近字符边界）。验证：`cargo test --lib` 1325 passed / 2 ignored（+1）；纯 Rust 修复。
> **v0.38.0**：代理工作室实时显示修复与三 Agent 完善。①修复幕后 AgencyStudio 未打开时创世/续写事件丢失、打开后空白等待：agency 事件监听从条件挂载的页面提升到常驻 `App.tsx` 顶层，新增全局前端 store `src-frontend/src/stores/agencyActivityStore.ts`（对标 backendActivityStore 的单例无 persist 模式，activities/progress cap 200），页面未开不再丢实时动态，打开即见；跨故事切换时 activeRunId 按 storyId 校正。②三 Agent（主创/管理/编辑审计）事件信号补齐（`agency/coordinator.rs`）：概念/资产/首章/资产补齐/装配 start/done 全路径配对（含 legacy 与快速路径单点覆盖）；修复 legacy 概念完成信号角色标注（LeadWriter→Producer）；后台质检黑板写入实时推 `agency-board-changed`。③前端打磨：幕前动词映射补全；幕后时间线去重改业务键。④续写熔断不丢稿：行为已由 65d90b5（v0.30.30）实现，本档补齐流程级测试。验证：`cargo test --lib` 1306 passed / 2 ignored（+5）；`npx vitest run` 421 passed / 3 skipped（+17）。
> **v0.37.0**：资产回流——后台资产 agent（IngestPipeline）对已生成正文生效。此前提取结果只写 kg 记忆层，续写 writer 只读生产资产表，两不相通。①提取 prompt（`resources/prompts/memory/memory_content_analysis.md`）升级为写作级字段并与 schema 严格对齐（角色情感画像 / 双向情感关系 / 世界观增量 / 场景大纲 / 故事增量）。②新增**资产桥** `memory/asset_bridge.rs`——memory 层 → 生产资产表的单向桥：提取结果 upsert 进 characters / character_relationships / world_buildings / scenes.outline_content / story_outlines，新登场角色自动注册；源感知合并——只精炼机器来源（ingest/agency/auto_placeholder），用户手工编辑（user_created/manual）永不覆盖。③Agency 续写路径接入：每章正文落库后后台 `spawn_asset_ingest` 自动跑提取（含 KG 持久化）；orchestrator/TriShot 路径经 `run_ingest` 自动生效。④并发安全：per-story 进程内锁 + `BACKGROUND_LLM_SEMAPHORE` 后台串行化；提取失败不致命，绝不影响正文落库。验证：`cargo test --lib` 1301 passed / 2 ignored（+14）；`npx vitest run` 404 passed / 3 skipped（无前端逻辑变更）。
> **v0.30.46-48**：创世持久化链路审计修复 + issue #13/#14/#15 批量修复。**v0.30.46**（创世正文未即时保存与资产缺失）：前端两条创世路径补 `setTimeout(flushSceneSave, 0)` 补偿保存；`agency/coordinator.rs` 场景装配 create+update 合成单事务 + 空正文校验；`generate_chapter_outline` 写黑板身份 Producer→LeadWriter；`orchestrator.rs` 创世成功臂回读空正文即报错；`scene_repository.rs` 空串 content 归一 None 防覆盖；`agency/materialize.rs` 新增 foreshadowing 落库 + item_type 别名归一化 + characters upsert。**v0.30.47**（issue #13/#14 角色谱静默失败）：`agents/novel_creation.rs` 角色谱/文风/首场景改 `extract_and_sanitize_json` 健壮解析 + 去 unwrap + warn 日志；`llm/service.rs` `prompt[..200]` 字节切 UTF-8 panic（llm_calls 永不落库根因）改 `chars().take(200)`；向导三卡片 `isGenerating` 防重入；拆书页 4 处 toast 改 `extractMessage`。**v0.30.48**（issue #15）：向导策略加载中误显失败文案改转圈动画；快速创作简介为空先确认。验证：`cargo test --lib` 1098 passed / 2 ignored；`npx vitest run` 352 passed / 3 skipped；`tsc`/`fmt` 全绿。
> **v0.30.45**：修复文思活跃模式续写提示词泄露（LLM 思维链泄露到正文）。根因四层叠加：①`llm/openai.rs` 的 `resolve_content`（v0.30.25 引入，当时为修复 DeepSeek 推理模型空 content 静默失败而加的 `reasoning_content` fallback：`Message`/`OpenAiDelta` 结构体加 `reasoning_content: Option<String>` 字段，非流式/流式在 `content` 为空时 fallback 到 `reasoning_content`）在 `content` 为空时错误回退到 `reasoning_content`（CoT），把思维链当正文返回--当推理模型把全部 token 预算花在 CoT 上时 `content` 恒为空，fallback 恰好把 CoT 当作"正文"投递给前端；②`max_tokens: 2048` 对推理模型过小，CoT 耗尽全部 token 预算导致 `content` 恒为空、整段被 CoT 占据；③`sanitize_novel_output`（v0.23.36 引入，逐行去 markdown/截断尾部元评论/剥离前导过渡语/去整行小节标题批注）无法识别裸 CoT 思维链（无固定围栏/标记，纯推理行）；④writer 提示词从未显式禁止推理输出。修复：①移除 `resolve_content` 的 `reasoning_content` 回退（`content` 为空即返回空，不再用 CoT 兜底--v0.30.25 的 fallback 本是为避免"空 content 静默失败"，但推理模型空 content 多因 CoT 耗尽预算，fallback 反而把 CoT 当正文，弊大于利）；②`max_tokens` 2048 -> 4096，给推理模型留足正文预算；③新增 `detect_and_strip_bare_cot`（检测 ≥3 条 CoT 信号行触发剥离），接入 `sanitize_novel_output` 后处理；④writer 提示词新增反推理指令（禁止输出思考过程/推理链）。验证：`cargo test --lib` 1091 passed / 2 ignored（+4）；`npx vitest run` 352 passed / 3 skipped；`cargo clippy --lib` 539（零新增）；`tsc`/`fmt`/`architecture_guard`/`format:check` 全绿。
> **v0.30.44**：修复文思活跃模式续写报"生成过程异常结束，未收到有效内容"。根因：`smartExecuteInFlightRef.current = false` 在 smartExecute resolve 后、内容处理前被提前清除（`handleRequestGeneration` line ~3231 + `handleSmartGeneration` line ~4194）--后台活动同步回调（`useBackendActivityStore` 订阅，100ms 防抖，`startTransition` 包裹 `setIsGenerating`）在内容处理期间（isAlreadyPresent 检查 / active mode 追加 / appendAiContent 同步步骤）把 `isGenerating` 置 false，触发安全网 effect（`!isGenerating && smartExecuteNeedDiagnosticRef.current && !lastGenerationCancelledRef.current` -> `captureDiagnosticInfo('生成过程异常结束，未收到有效内容')`）误报。`handleRequestGeneration` 活跃模式分支还错误地走了打字机幽灵文本（`requestAnimationFrame` 3 字符/帧），而非直接 `appendAiContent` 追加到编辑器正文。修复：①移除两处 smartExecute resolve 后的提前 `smartExecuteInFlightRef.current = false`；改为在各内容交付退出路径（active mode append / isFirstChapterReady / ghost text / aborted / isAlreadyPresent / isBootstrapCompleted&&delivered / 打字机完成 / displayText 空 bail / background bootstrap / genesis 首章）统一清除 `smartExecuteInFlightRef` + `smartExecuteNeedDiagnosticRef`；`handleSmartGeneration` 的 `finally` 块在 `setIsGenerating(false)` 之后兜底清除 flight 标志防泄漏（确保 `setIsGenerating` 触发安全网检查时 flight 仍为 true）。②`handleRequestGeneration` 活跃模式分支在打字机之前直接 `appendAiContent(displayText, 'auto')` + 清除两标志 + `setIsGenerating(false)`，绕过打字机（与 `handleSmartGeneration` 活跃模式行为一致）。纯前端修复（`FrontstageApp.tsx`），无 Rust 变更。验证：`npx vitest run` 352 passed / 3 skipped（+2）；`tsc`/`fmt`/`clippy`（538 零新增）/`architecture_guard`/`format:check` 全绿。
> **v0.30.43**：修复续写内容丢失根因--flushSceneSave 读取滞后的 latestContentRef（RichTextEditor 200ms HTML 防抖窗口）而非编辑器实际 HTML，关闭/切章时最后 200ms 的输入丢失；onChapterUpdated 用 DB 旧内容覆写编辑器但不更新 latestContentRef，用户未保存输入被覆写后不可逆丢失。修复：flushSceneSave 改为直接读 editorRef.getHTML()（编辑器实际内容），回写 latestContentRef 保持一致；onChapterUpdated 新增守卫--latestContentRef 与 DB 内容不同时跳过覆写，覆写后同步 latestContentRef。纯前端修复（FrontstageApp.tsx），无 Rust 变更。验证：cargo test --lib 1087 passed；npx vitest run 350 passed / 3 skipped（+1）；tsc/fmt/clippy（538 零新增）/architecture_guard/format:check 全绿。
> **v0.30.42**：修复世界观生成失败（LLM 返回 markdown 代码块包裹的 JSON + 未转义引号 + 静默失败 + prompt 字段名不匹配）。根因（issue #14）：用户报告"世界观生成失败，请重试"，但日志显示 LLM API 调用成功返回内容（7636 字符），失败发生在下游 JSON 解析且完全无错误日志。模型将 JSON 包裹在 ` ```json ... ``` ` 代码块中、或在字符串值内直接换行/使用裸双引号，`serde_json::from_str` 静默失败。①`agents/novel_creation.rs:118` 用严格 `serde_json::from_str` 解析全量响应（含围栏）直接失败；②`agency/coordinator.rs::parse_lenient` 用 `rfind('}')` 会被尾部杂散 `}` 误导且无法修复字符串内裸换行；③`novel_creation_world_options.md` prompt 要求"concepts 数组"但代码读 `parsed["world_buildings"]`，即使解析成功也找不到数组；prompt 缺少格式约束。三层修复：①`parse_lenient` 改为先调 `crate::narrative::extract_and_sanitize_json`（剥离 markdown 围栏 / 推理链、括号深度匹配跳过尾部杂散 `}`、修复字符串内未转义换行、移除 BOM/注释/尾随逗号），失败回退旧首尾花括号截取--覆盖 agency 全部 JSON 解析路径（concept_pack / producer_depth_assets 世界观 / editor 裁决 / retrieval plan）；`extract_and_sanitize_json` 已存在于 `narrative` 且被 memory/analysis 等模块使用，`agents` 已有 `crate::narrative::strip_reasoning_blocks` 先例，无新跨层依赖（architecture_guard 仅约束 db/domain，agency→narrative 无环）。②`novel_creation.rs` 提取 `parse_world_options_response` 纯函数先剥离围栏再 `serde_json::from_str`；失败时 `log::warn!` 记录错误 + raw 长度 + 200 字片段（此前完全静默）；元素反序列化 `unwrap` 改 `map_err` 不再 panic。③两份 prompt 修正字段名（`concepts` -> `world_buildings`）+ 补全 schema + 新增格式约束（禁 markdown 围栏、引号转义、JSON 外无文字）。验证：`cargo test --lib` 1087 passed / 2 ignored（+5）；`npx vitest run` 349 passed / 3 skipped；`tsc`/`fmt`/`clippy`（538 零新增）/`architecture_guard`/`format:check` 全绿。

> **v0.30.41**：修复续写内容被假阳性去重静默丢弃（模型回显指令 + 短文本假阳性 + 内容丢失）。根因：用户诊断报告显示续写生成时 LLM（deepseek-v4）成功返回 2511 字符，但前端仅显示 6 字符（"续写\n黑暗。"），随后报"生成过程异常结束，未收到有效内容"。根因链：①模型在生成内容开头回显用户指令"续写"（非正文）；②打字机动画首帧仅 3 字符（"续写\n"），归一化后 2 字符"续写"几乎必然出现在 9656 字已有正文中；③`isTextDuplicate`（`src-frontend/src/utils/textCleanup.ts`）假阳性返回 true，`setGeneratedText` 跳过赋值并 `markAccepted` 存入 2 字符指纹；④生成内容被静默丢弃。两层修复：①`isTextDuplicate` 新增最小长度守卫--归一化后 < 30 字符的生成文本直接返回 false，不进行去重检查（短文本几乎必然是已有正文的子串，去重无意义且会误杀首帧）；②新增 `stripInstructionEcho(generated, userInput)`（`textCleanup.ts`）剥离模型回显的用户指令前缀（按归一化前缀匹配 + 换行/标点边界剥离），在 `handleRequestGeneration` 和 `handleSmartGeneration` 的 `sanitizeContinuationOutput` 后调用。纯前端修复，无 Rust 变更。验证：`npx vitest run` 349 passed / 3 skipped（+13）；`tsc`/`format:check`/`architecture_guard` 全绿。

> **v0.30.40**：修复代理工作室不显示活动记录数据（`activeRunId` 仅从事件捕获 + 无 `list_runs` 命令）。根因：`AgencyStudio.tsx`（幕后代理工作室页面）的 `activeRunId` **仅从实时事件捕获**（`agency-agent-activity` / `agency-run-progress` / `agency-board-changed` 三个 `listen` 回调），IPC 查询 `getRun`/`listBoard` 的 `enabled: !!activeRunId`。用户在 run 启动后或完成后才打开页面时，无事件到达，`activeRunId` 恒 null，页面永远显示"暂无活动"。且 `agency_runs` 表虽有持久化数据（`idx_agency_runs_story` 索引），但**无 `list_runs` 命令**供前端发现已有 run；`agency-agent-activity` / `agency-run-progress` 事件 fire-and-forget（`app.emit` only，不写 DB），时间线数据页面卸载即丢失。修复三层：①后端新增 `AgencyRepository::list_runs_for_story(story_id, limit)` + `agency_list_runs` Tauri 命令（`commands.rs`，limit=20，注册到 `handlers.rs`），按 `created_at DESC` 列出 story 的全部 run；②前端 `AgencyStudio.tsx` 新增 `useQuery(['agency-runs', storyId], listRuns)` + `useEffect` 在 runs 到达且 `!activeRunId` 时取 `runs[0].id` 水合（实时事件仍可覆盖）；③时间线从仅 live 事件改为三源合并（live 事件 + board items 历史重建 + run 生命周期），`board items` 的 `created_at`/`producer`/`zone`/`key`/`summary` 生成历史时间线条目，`run` 的 `created_at`/`updated_at` 生成生命周期条目，`(at, text)` 去重排序截断 100 条，无需新表/迁移。附带：新增 run 选择器 `<select>` 下拉框（option `[status] phase - premise(时间)`，切换 `setActiveRunId` -> react-query 自动刷新）；`agency/repository.rs` 两处 `Ok(rows.collect::<Result<Vec<_>, _>>()?)` -> `rows.collect::<Result<Vec<_>, _>>()`（clippy `needless_question_mark`）。

> **v0.30.39**：修复续写不按故事大纲推进剧情（TimeSliced 路径缺失 `build_progression_anchor`）。根因：v0.30.31 引入的 `build_progression_anchor`（确定性注入剧情推进方向锚点：本次创作指令 + 故事大纲硬约束 + 本章场景大纲硬约束 + 已推进进度指针 + 世界观规则硬约束 + 显式调和指令）**只在 TriShot 路径（`execute_trishot`）的 `!synthesis.is_fallback` 分支调用，从未移植到 TimeSliced 路径（`execute_time_sliced`）**。而 TimeSliced 是默认续写路径（`generation_mode = "auto"` 路由续写到 TimeSliced 而非 TriShot）。TimeSliced writer 通过 `bundle.to_prompt()` 得到完整故事大纲，但缺少"已推进进度"指针（最近 3 章 `scenes.outline_content`），无法判断当前在故事大纲哪个节点，导致续写偏离大纲、原地踏步或仅复述设定。修复（`agents/orchestrator.rs` `execute_time_sliced`）：在 prompt 模板渲染后（line ~1013）、`ending_anchor` 注入前（line ~1014），插入 `build_progression_anchor(&bundle, pool.inner(), &task.context.story.story_id, chapter_number, &user_instruction)` 调用，与 TriShot 路径完全对齐。注意 `story_id` 在 `spawn_blocking` 闭包（line 910 `move ||`）中被 move，改用 `&task.context.story.story_id`（`task` 参数仍在作用域内）。`build_progression_anchor` 函数本身不变（line 3716-3803），已由 v0.30.31 测试覆盖。

> **v0.30.38**：修复续写输出被编辑器元评论污染（is_prose_request 被 serde 默认 false 导致 sanitize 跳过）。根因：分类提示词"继续写"示例省略 `is_prose` 字段，LLM 若遵循示例返回合法 JSON 但缺该字段，`WritingIntentClassification` 的 `#[serde(default)]` 填 `is_prose_request = false`。serde 默认值（false）与 LLM 失败兜底值（true）相反--partial-but-valid JSON 走 `parse_classification_json` 成功解析、`is_fallback=false` 被缓存，后续相同"续写"输入持续返回毒化 false。`sanitize_plan_for_prose_request`（`planner/mod.rs`）门控 `Some(c) if c.is_prose_request => c, _ => return`--false 时跳过全部净化（移除 builtin.* 技能 / 续写塌缩单 writer / 弹出尾部非 writer），SING 多步计划 `[writer, inspector, builtin.style_enhancer]` 未拦截：writer 产出正文、inspector 产出问题列表+评分、style_enhancer 收到 inspector 输出后产出编辑器元评论，`execute_plan` 的 `final_content` = 最后产出 content 的步骤 = style_enhancer 元评论覆盖 writer 正文。三层修复：①`intent.rs` `parse_classification_json` 后置不变量--`is_continuation || is_new_novel` 但 `is_prose_request=false` 时强制设 `true`（续写/创世本质是 prose，逻辑必然）；②`intent.rs` `build_classification_prompt` "继续写"示例补 `is_prose=true`；③`planner/mod.rs` `sanitize_plan_for_prose_request` 门控从 `is_prose_request` 扩展为 `is_prose_request || is_continuation`（纵深防御）。+4 回归测试。

> **v0.30.37**：修复创作生成失败时 toast 显示 "[object Object]"（issue #12）。根因与 issue #11（v0.30.31 修复的"获取模型列表"路径）同源：后端 `AppError` 自定义 `Serialize` 实现产出普通 JSON 对象 `{ code, message, severity, data? }`，Tauri v2.4 将其作为**普通对象**（非 JS `Error` 实例）投递到前端 `catch` 块；前端用 `String(err)` 或 `err instanceof Error ? err.message : String(err)` 转字符串，对普通对象产出 `[object Object]`，可读的 `message` 字段被丢弃。v0.30.31 引入的 `extractMessage` helper（`src/utils/errorHandler.ts`）只覆盖了"获取模型列表"一条路径，创作/生成相关错误路径未迁移。修复：将 10 个前端文件共 36 处 catch 块统一替换为 `extractMessage(err)`--`FrontstageApp.tsx`（5 处：smart_execute 主 catch 用 `structured?.message ?? extractMessage(error)` 复用已计算 `structured` + 第二 smart_execute catch + 修稿/审稿/定稿）/ `SceneEditor.tsx`（2 处：生成大纲/草稿）/ `Stories.tsx`（4 处：快速创作/向导创作/风格保存/风格生成）/ `RichTextEditor.tsx`（2 处：文思内联建议/智能排版）/ `WenSiPanel.tsx`（2 处：自动续写/修改）/ `usePipeline.ts`（6 处）/ `CharacterStatePanel.tsx`（1 处）/ `Skills.tsx`（7 处）/ `PromptsPanel.tsx`（5 处）/ `useUpdater.ts`（2 处）。`extractMessage` 依次尝试：①结构化 AppError 普通对象取 `.message`；②`Error.message` 内嵌 JSON 解析取 `.message`；③普通 `Error` 取 `.message`；④字符串原样返回；⑤带 `.message` 字段的对象取之；⑥兜底 `'Unknown error'`。不动 `main.tsx`/`ErrorBoundary.tsx`（两者已优先取 `.message`，`String()` 仅作最后兜底，对带 `.message` 的 AppError 对象不会产出 `[object Object]`）。新增 `src/utils/__tests__/errorHandler.test.ts`（+8 回归）。纯前端修复，无 Rust 变更。

> **v0.30.36**：修复首次创世指令不保存到输入历史（按↑调取不到）。根因：`handleInputSubmit` 保存输入历史时 `sid = currentStory?.id`，首次创世（无已有故事）时 `currentStory=null` -> `if (sid) saveInputHistory(...)` 跳过；isBootstrap 分支 `setCurrentStory(null)` 触发 `useEffect[currentStory?.id]` 清空 `inputHistory`，创世成功后 `setCurrentStory(新故事)` 再次触发 useEffect 从 localStorage 加载（空）。新故事历史始终为空，按↑无响应。v0.30.23 修复意图分类后创世指令正确走 isBootstrap 路径，暴露了此前被续写误分类掩盖的缺陷。修复（`FrontstageApp.tsx` `handleSmartGeneration` story_created 块）：`setCurrentStory(targetStory)` 之后同步 `loadInputHistory(storyId)` 读取新故事现有历史，若不含 `userInput` 则 `saveInputHistory(storyId, [userInput, ...existing].slice(0, MAX))` 持久化。关键时序：此写入在 `setCurrentStory` 触发 useEffect 之前同步执行（同一同步块无 await），useEffect 随后 `loadInputHistory(storyId)` 即可读到创世指令。纯前端修复。

> **v0.30.35**：editor 质检后台异步化--首章立即显示 + 后台质检 + toast 反馈。根因：editor 质检（`review_and_assemble` 的 `evaluate_gate`）在 Scene 装配落库**之前**同步执行，被 `tokio::time::timeout(600s)` 包裹；producer（深度资产 ~30-60s）+ writer（tool_loop ~4-5min）花约9分钟后 editor 仅剩约1分钟，其 `editor_verdict_prose_fallback` 用固定 300s timeout 发起 LLM 调用，34s 后被硬 600s 砍掉，既未完成质检也无法走 `salvage_failed_gate` 保产出，整 run 超时无首章返回。修复（`agency/coordinator.rs`）：①新增 `assemble_only`（pub(crate)）从 `review_and_assemble` 提取纯装配部分（`cleanup_prose_for_persist` 抗重复三件套 + `SceneRepository::create/update` 落库 + `emit_activity`），不含 editor 质检与修订，返回 `(BoardItem, scene_id)`；②新增 `spawn_editor_qc`--测试环境 `app_handle=None` 时 no-op，生产环境 `tokio::spawn` 后台任务，构造全新 `AgencyLlm(EditorAuditor)`/`AgencyBudget`/`BlackboardService`/`ToolRegistry`，用 `Some(Instant::now() + 300s)` 独立 deadline（不受 smart_execute 600s 限制）调 `evaluate_gate_impl`，结果三态分支（`Passed`->{passed,salvaged:false}；`RevisionRequired`->{passed:false,issues}；`Failed`->先 `salvage_failed_gate` 保产出->{passed:true,salvaged:true}或{passed:false,issues}；`Err`->降级放行{passed:true,salvaged:true}），emit `genesis-qc-result` 事件 + `emit_activity(EditorAuditor,"后台审查")`；③`genesis_fastpath` / `run_genesis_legacy_inner` Phase C 由 `review_and_assemble` 改为 `assemble_only` + `spawn_editor_qc`，返回 `revised:false, verdict:EditorVerdict::pending()`；④删除无用 `review_and_assemble`（helper `build_revision_task`/`evaluate_gate` 仍被续写路径复用）；⑤`EditorVerdict` 新增 `pending()`；⑥新增 `EVENT_GENESIS_QC_RESULT`。前端 `FrontstageApp.tsx` `setupEventListeners` 新增 `genesis-qc-result` 监听三态 toast（通过/降级放行/不合格建议重新创世），后台 editor 不影响 `isGenerating`。producer 深度资产保持前台（单次 `complete_json` ~30-60s，非瓶颈，保障首章不脱节）。修复后流程：concept -> producer -> writer -> 装配 -> 立即返回显示首章（~5-6min 可见），editor 后台 300s 独立质检 -> toast 反馈。

> **v0.30.34**：修复续写内容丢失根因--序列化场景持久化 + 修稿 bypass 修复 + 关闭超时提升。v0.30.33 的关闭前 flush + AI 追加立即落库仍未能完全解决续写内容丢失。三个收敛根因：①`flushSceneSave` 无序列化--文思活跃连续续写时多次 `void flushSceneSave()` 并发 fire-and-forget，`update_scene` 全量覆写在 `spawn_blocking` 线程池上 SQLite 写锁获取顺序非 FIFO，较早的小内容可能在较晚的大内容之后提交，静默覆写（编辑器显示正确但 DB 回退，重启才发现）；②close-flush 3s 超时 < SQLite `busy_timeout` 5s，写锁竞争下 close-flush 被 kill；③`handlePipelineRefine` `setContent` / `onReviseResult` `insertText` 绕过 `appendAiContent` 不更新 `latestContentRef`，关闭时 flush 保存旧内容。修复：①新增 `saveChainRef`（Promise 链）+ `persistSceneContent(sceneId, content, title)` 序列化所有 `update_scene`（`flushSceneSave` / `handleContentChange` saveFn / 保护性保存统一走此函数）；②`CloseRequested` 超时 3s -> 6s；③`setContent`/`insertText` 后补 `getHTML -> store.setContent -> latestContentRef = html -> void flushSceneSave()`。

> **v0.30.33**：修复关闭应用时续写内容丢失。根因：幕前续写 `appendAiContent` 追加 AI 内容后仅调度 2000ms 防抖保存（`scheduleAutoSave(..., 2000)`），文思活跃连续续写时每次 `cancelAutoSave()` 重置定时器导致永不出火；关闭应用时后端 `CloseRequested`（`lib.rs`）直接 `graceful_shutdown -> std::process::exit(0)` 不给前端 flush 机会。三层修复：①关闭前 flush 协调--后端 `CloseRequested` 改为 `api.prevent_close()` + emit `frontstage-flush-requested` 事件 + 3s 超时兜底线程；前端 `FrontstageApp.tsx` `useEffect` 监听该事件 -> `await flushSceneSaveRef.current()`（取消防抖 + 立即 `update_scene` 落库 `latestContentRef.current`）-> `invoke('graceful_quit')` 命令触发优雅关闭（WAL checkpoint 确保落盘）；`graceful_shutdown` 加 `AtomicBool` 幂等守卫防 flush 完成与超时兜底竞争。②AI 追加立即落库--`appendAiContent` 的 `scheduleAutoSave(..., 2000)` 替换为 `void flushSceneSave()`（立即 fire-and-forget 落库），消除防抖窗口丢失风险，即使崩溃内容也已落库。③章节切换前 flush--`selectChapter` 的 `cancelAutoSave()` 替换为 `void flushSceneSaveRef.current()`。提取 `flushSceneSave` 共享 `update_scene` 落库逻辑（cancelAutoSave -> 读 store sceneId/title + latestContentRef -> loggedInvoke -> setIsSaved + justSavedRef），通过 `flushSceneSaveRef` 暴露给 effect 监听器与 selectChapter。

> **v0.30.31**：续写链路修复--世界观/故事大纲/场景大纲注入与剧情推进方向。幕前续写走 Legacy TriShot，但 `final_prompt = Call1 LLM 合成的 synthesized_prompt`，manifest 不含 story_outline、synthesizer 不透传 bundle_prompt 关键段，导致故事大纲/场景大纲 outline_content/world_buildings 三者均不到达 writer（v0.30.15 注释声称修了 TimeSliced/TriShot，实际只修了 TimeSliced）。`orchestrator.rs` 新增 `build_progression_anchor` 在 `final_prompt` 后确定性注入【剧情推进方向（最高优先级）】段（故事大纲1200+场景大纲800+已推进进度+世界观600+推进约束），`!is_fallback` 时注入（fallback 时 synthesized_prompt=to_prompt 已含这些段，避免重复）。`write_time_bundle.rs` load_sync 读 world_buildings 表（concept+rules前5+history+cultures前3，截断2000）为新增 `world_setting: Option<String>` 字段，`to_prompt` 增【世界观设定】段；`manifest.rs` build 增加 story_outline/world_setting 清单项（hard_constraint）、scene_outline one_line 纳入 outline_content 摘要。进度指针用现有 `scenes.outline_content` 回读最近 3 章作为"已推进到哪"，无 DB 迁移、无 schema 变更。`scene_outline.md` 修"按序号定位节点"伪前提为"按已推进进度定位"，Legacy（`creation_commands.rs generate_scene_outline` 加载 world_buildings+最近3章 outline_content 注入 task.parameters，`service.rs build_outline_prompt` 读取注入 vars）与 Agency（`generate_chapter_outline` vars 注入 world+progress）双路径注入。Agency `build_continue_writer_context` 世界观全字段（此前只 concept+history 且超6000整段丢弃，现截断降级注入）+ 前文保底（阈值倒挂修复 >8000->>12000 且保底最近1场正文1500字）+ 进度指针；`write_chapter` 三分支加推进约束+点名世界观。`ensure_world_building` concept 存全文（此前截500）+ prompt 增"正文末尾用【核心规则】列出3-5条世界规则"best-effort 解析存 rules；history 不再冗余存储（concept 全文已含）。`evaluate_gate_impl` editor task 预注入参照资产（世界观红线+世界观设定+故事大纲），使"合同兑现/连续性/世界观一致性/推进方向"维度可校验。

> **v0.30.30**：Agency 创作链路结构性优化--抗重复闭环 + 质量门宽松度 + 熔断不丢稿。`agency/coordinator.rs` 五点修复：①D1 抗重复提示词补齐（`agency_lead_writer_system.md` 创作红线 + `agency_editor_auditor_system.md` 新增"重复与复述"审查维度 + 内联 writer prompts 各加禁止重复指令）+ 抽取共享 helper `cleanup_prose_for_persist`（`trim_self_repetition` -> `strip_existing_overlap` -> `trim_dangling_tail`），创世 `review_and_assemble` 装配接入（此前写 RAW 正文），续写 `handle_gate` 内联块替换为调此 helper 去重；②D2 失效 prompt_id 核查结论 by-design（`roles.rs` 占位 ID 运行时回退 `default_role_prompt`，仅加注释）；③E1 `ModelGraderReport::from_verdict` scoreless pass 兜底 0.85 -> 0.7（Gate v2 加权 `0.2*code+0.3*rule+0.5*model` 阈值 0.75，editor 不给数值分时不再单凭 model 项过门）；④E2 `salvage_failed_gate` helper（草稿 ≥600 字符降级放行保产出），4 个 GateOutcome::Failed arm 不再直接 `return Err` 丢稿；⑤E3 writer MaxTurns/Deadline 熔断先 `latest_draft`/`latest_draft_by_key` 取黑板已产出草稿（`>=200` 字符），取不到才散文回退。把"熔断不等于丢稿"哲学（v0.30.19 salvage + 散文回退）补齐到 writer 与 gate Failed 两个剩余缺口。

> **v0.30.29**：内容质量根因修复--强模型返回的结构化整书大纲对象不再被 `parse_lenient` 丢弃。`agency/coordinator.rs` 五点修复：①`DepthAssets.outline` 由 `String` 改为 `serde_json::Value` + 新增 `normalize_outline` 将结构化对象（core_conflict/three_act_structure/turning_points）渲染为可读文本落库（实证根因：serde `String` 类型不匹配 -> `parse_lenient` 返回 `None` -> 散文兜底 outline=空 -> 大纲不写 `story_outlines` 表，模型越强大纲越完整越被丢弃）；②创世 Phase B 由 `tokio::join!(writer, producer)` 并行改串行 producer-first，新增 `build_assets_ctx_brief` 注入首章；③续写 `build_continue_writer_context` 最前注入 MASTER_SETTING 红线；④`handle_gate` 落库前接入抗重复三件套（trim_self_repetition/strip_existing_overlap/trim_dangling_tail）；⑤`generate_chapter_outline` 改用 DB-backed `scene_outline.md` 提示词。

> **v0.30.26**：统一 Logline 增强提示为内联幽灵文本并修复分时预检缺少角色。`FrontstageBottomBar.tsx` 将 v0.30.24 的独立 `.frontstage-logline-hint` 建议条改为输入框内跟在已输入内容后的幽灵后缀（前缀 `visibility:hidden` 占位，后缀灰色透明），`FrontstageApp.tsx` 按 `→` 追加后缀、Enter 提交“原输入 + 增强后缀”组合文本；新增 `resources/prompts/agency/agency_logline_suffix.md` 让后端 `generate_logline_hint` 只返回追加后缀；简化 `handleSmartGeneration` 恢复只接收 `userInput`，移除 `originalInputForLoglineRef` / `intentClassificationInput` 透传。修复分时预检缺少角色：`intent.rs` 兜底路径按输入文本判断创世意图；`story_system/preflight.rs` 的 `QuickPreflightChecker` 在角色表为空时自动创建占位主角（仅一次 DB 写入，不触发 LLM），避免空角色表阻塞生成。
>
> **v0.30.25**：修复续写 600s 超时（三层根因叠加）。①前端 `FrontstageApp.tsx` 续写请求不再阻塞 `autoCreateMissingContracts`--`handleSmartGeneration` 中 `classification.is_continuation` 与 `handleRequestGeneration` 改为后台 fire-and-forget（新增 `fireAutoContractInBackground` helper + `autoContractInProgressRef` 防并发 + 非阻塞 toast），非续写保持阻塞 await；v0.26.22 `is_silent_background` 只隐藏了 `isAnyBackendActive` 但 `await` 仍阻塞触发 600s 看门狗，后端 TimeSliced 续写路径本已跳过 auto_contract 但前端从未调用到。②`openai.rs` `Message`/`OpenAiDelta` 结构体加 `reasoning_content: Option<String>` 字段（`#[serde(skip_serializing, default)]`），非流式/流式在 `content` 为空时 fallback 到 `reasoning_content`，修复 DeepSeek 推理模型空 content 静默失败（`content=""` 但 `tokens=2643`）；提取纯函数 `resolve_content` 供单测。③`auto_contract.rs` 的 4 个 `build_*` LLM 调用各包 `tokio::time::timeout(30s)`，总上限 120s 远低于 600s 看门狗。
>
> **v0.30.24**：Logline 幽灵提示--用户输入简单创世指令时实时生成增强版 logline。`commands/orchestrator.rs` 新增 `generate_logline_hint` 命令（复用 v0.30.22 `agency_problem_logline` prompt 资产 + `LlmService` + 15s 超时 + 静默降级），提取纯函数 `should_skip_logline_generation` / `is_valid_logline` 供单测。`FrontstageApp.tsx` 新增 `loglineHint` / `loglineHintLoading` state + 1.5s 防抖 effect（请求 ID 防竞态）+ `->` / `Esc` 键盘处理。`FrontstageBottomBar.tsx` 新增 `.frontstage-logline-hint` 建议条（loading 旋转图标 / 就绪 Lightbulb + logline + "按 -> 使用" / 点击接受）。与现有 `ghostHint` 互斥（ghost hint 仅空输入时显示，logline hint 仅非空输入时显示）。
>
> **v0.30.23**：意图分类 Bug 修复--LLM 分类去偏 + 失败兜底上下文化。`intent.rs` `build_classification_prompt` 移除 `已有故事={story}` 上下文注入行（偏差来源，使 LLM 倾向续写），改为基于用户输入本身判定意图；新增 3 个正例（"写一部科幻小说" -> is_new_novel=true）；新增 `conservative_fallback_with_context(has_existing_story)`--LLM 失败时无故事返回创世（不可能续写不存在的作品），有故事返回续写；仅缓存 LLM 成功结果不缓存兜底；缓存键简化为仅 `user_input`。`FrontstageApp.tsx` 两处 LLM 失败兜底从硬编码 `is_new_novel: false` 改为 `stories.length === 0`。设计原则：LLM 是意图判断的唯一权威，不回到硬编码关键词匹配，不用 `|| !has_existing_story` 覆盖 LLM 结果。
>
> **v0.30.22**：PROBLEM 七元素框架集成（Logline 生成 + 故事大纲增强）--引入 Erik Bork 的 PROBLEM 七元素作为后端创作资产。`coordinator.rs` 新增 `generate_logline`（简单 premise < 100 字触发 PROBLEM logline 生成）；`run_genesis_inner` 在 concept_pack 前新增一次 Producer LLM 调用产出 logline，经 `StoryRepository::update_logline` 持久化至 `stories.logline`（V114 迁移 `ALTER TABLE stories ADD COLUMN logline TEXT`，Story 模型加 `logline: Option<String>`）；`ensure_story_outline` 改用注册表 PROBLEM outline 提示词、`build_continue_writer_context` 注入【故事Logline】，两者均从 DB 读 logline；`producer_depth_assets` 增强 PROBLEM 指导。提示词资产 `resources/prompts/agency/agency_problem_logline.md` / `agency_problem_outline.md` 经 WalkDir 自动注册。
>
> **v0.30.21**：续写资产层级生成--`ensure_assets`（`coordinator.rs`）角色检查后追加 world_buildings / story_outlines 检查，缺失时调 `ensure_world_building` / `ensure_story_outline` 单次 Producer LLM 调用生成并落库（不抢主创 LLM，失败不阻断续写）。`build_continue_writer_context` 注入故事大纲（`StoryOutlineRepository`）。`generate_chapter_outline` 在 writer tool_loop 前生成章节大纲（服从故事大纲），写入黑板 Draft 区；strict writer task 含故事大纲 + 本章大纲 + 写作要求（起伏/转折/冲突）。`handle_gate` 装配时从黑板读取章节大纲存入 `scenes.outline_content`。形成"世界观 -> 故事大纲 -> 章节大纲 -> 正文"层级约束链。**v0.30.20**：修复质量门编辑审计 Agent 熔断--`evaluate_gate_impl`（`coordinator.rs`）中 editor_auditor 的 ReAct tool_loop 在本地模型（Qwen 3.6）不遵从 JSON action 格式时连续解析失败/达最大轮数（6 轮）熔断，原实现 `if editor_out.aborted` 直接返回 `GateOutcome::Failed` 导致整 run 失败。Fix（两层兜底）：①salvage--熔断时仍先 `parse_lenient::<EditorVerdict>` 尝试从末轮输出提取裁决 JSON；②散文回退--新增 `editor_verdict_prose_fallback` 自由函数，单次 `llm.complete()` 直接请求裁决 JSON（不经 tool_loop/工具），复用 editor 系统提示词审查标准 + 追加「直接输出 JSON、不走工具循环」强约束，与 `writer_prose_fallback`（v0.30.3）同理。**v0.30.18**：修复幕前意图分类 null 崩溃--`handleSmartGeneration`（FrontstageApp.tsx）调 `classifyIntent` 后直接读 `classification.is_new_novel`，但 `classifyIntent` resolve 为 null 时不抛异常（catch 只拦抛出异常），E2E mock 对未注册命令返回 null 触发 `null.is_new_novel` PAGEERROR（v0.30.16 CI E2E 根因）。Fix：catch 后新增 post-catch null 兜底（续写语义）+ 不缓存 null。**v0.30.17**：幕前顶部创世状态显示三 Agent 动作/进度--新增 `useAgencyAgentActivity` hook 订阅后端已有的 `agency-agent-activity` 事件（此前仅幕后 `AgencyStudio` 消费），`FrontstageHeader` 顶部状态栏在 `orchestratorStatus` 之后渲染 主创/管理/编辑审计 各角色进度（进行中琥珀 `saving`、已完成绿色 `saved`），run 结束自动清空；底部 LLM 连接状态未改动。附带：`AGENTS.md` 强制构建规则 #2 改为「本地构建仅在用户明确要求时执行」。**v0.30.16**：故事资产手动编辑--故事大纲（Stories.tsx 查看/编辑）、故事摘要（KnowledgeGraph.tsx SummaryCard 编辑）、伏笔内容编辑+删除（ForeshadowingTracker update/delete 方法+命令+注册 + hook + UI）、角色关系编辑（hook + RelationshipCard 编辑表单）。**v0.26.57**：自动划分章节——`chapter_splitter` 在 `SceneService` 的 `auto_commit` 防抖窗口内按 `chapter_split_mode`（`word_count`/`plot`）与 `chapter_split_max_chars`（默认 3000 字）仅切分故事最新章；导出以 `scenes.content` 为真相源通过 `assemble_export_chapters` 聚合，并走系统保存对话框落盘；提示词注册表支持「打开目录」与原生 textarea 编辑。**v0.26.56**：executor 写 config 契约测试串行化（mock app_data_dir 锁）。**v0.26.55**：幕后模型列表开启/关闭——`update_model(enabled)` + 列表开关；禁用模型不进 `UnifiedModelRegistry`/`get_gateway_status`/probe；`is_promotable_user_model` 要求仍在注册表。**v0.26.54**：创作模型粘性降级绕过——显式 `creative`/`tool`/`background` 角色不受连续失败 demotion 拦截；粘性 Unhealthy 在 `resolve_role_model` 清一次→Unknown 再探；`set_active_model`/`save_settings` 调 `clear_model_demotion`；`generate()` 再提升用 `is_promotable_user_model`。**v0.26.53**：幕前故事名取消单击→回幕后；回幕后入口为 Header 设置按钮。**v0.26.52**：模型配置热同步——`gateway-status` 失效；`is_promotable_user_model`；`sync_creative_to_active_llm`。**v0.26.51**：幕前顶部故事名/章节名内联编辑——`displayStoryTitle`/`displayChapterTitle` 纯函数管展示；无故事有正文时 `ensureUntitledStory` 建「未命名」+ scene（不走 `selectStory`）；章节改名优先 `update_scene`（title 回写 chapter）。**v0.26.50**：幕前自动保存 → AutoIngest 改为 30s 防抖并受 `BACKGROUND_LLM_SEMAPHORE` 约束；`contract-auto-progress` 不再驱动 `isGenerating`；`isGenerating` 超时看门狗强制诊断。**v0.26.49**：续写连贯——`build_ending_anchor` 将正文末 2 句硬锚点追加到 Call3/TimeSliced prompt **最末尾**（在 `NOVEL_OUTPUT_DISCIPLINE` 之后），覆盖 WriteTimeBundle「开场建立处境」等开篇指令，抗 Lost-in-the-Middle。
>
> **v0.26.48**：自动更新闭环——`bundle.createUpdaterArtifacts=true` 产出签名更新包；端点仍为 GitHub `releases/latest/download/latest.json`；Linux 需 AppImage（deb 仅手动安装）；CI `verify-updater-manifest` 门禁。
>
> **v0.26.47**：无架构变更；Rust fmt 热修复。
>
> **v0.26.46**：创世 background 五步恢复 `strategy_section` 注入（修复 v0.26.28 外部化断链）；`build_strategy_notes_for_genesis_step` 分步注入；ContractSeeding 后 `methodology_step` 推进（雪花→4、HDWB→2）；`normalize_methodology_id` 统一 HDWB 别名；quick phase `EnsureGenreProfileStep` match-or-create；拆书 StoryArc/作者/伏笔持久化 + chunker 墙钟止血。
>
> **v0.26.45**：Genesis 人物卡（`ProtagonistCard`）双重注入 first_scene + TriShot Call3；规则探针真名/欲望/阻力；与 8% 自重复共享一次软重试。
>
> **v0.26.44**：Genesis quick_phase 四步「概念 → 策略 → 开篇骨架 → 撰写开篇」。`OpeningSkeletonStep` 在写正文前填充戏剧槽位（≤10s fail-open）；概念字段加厚；策略选择后启发式注入叙事四元组；TriShot 占位角色取自骨架。
>
> **v0.26.43**：幕前底部状态栏用 `StatusIcon`（Lucide）渲染阶段图标，不再嵌入 emoji（WebView 缺字会显示 □□）。

> **v0.26.42**：续写幽灵渲染锁——Tab 接受后的 `hideGhostUntil` / `postAcceptHideUntilRef` 必须在新续写开始时清零，否则只见 Tab 条不见幽灵正文。

> **v0.26.36**：`save_settings` 热重载 LLM 并广播 `app_settings`；字体/主题经 Tauri 事件跨窗口同步；`llm_first_chunk_timeout_secs` 接入适配器。


> 本文档反映 v0.26.34 最新架构状态（2026-07-09）
> **v0.26.41 债清偿**：`drafts.scene_id` + finalize 直写场景；`story_memory_facts` VIEW 统一 KG/记忆读面；`memory_items.kg_entity_id` 可选链接。物理表不 DROP。
> **v0.26.40 资产闭环**：侧栏 impact 徽章；诊断组默认折叠；MCP→Settings「扩展」；`WriteTimeBundle.related_entity_summaries`（MemoryFacade top-5）；`prompt_coverage` 写入 TraceStore；SceneEditor 内嵌 Pipeline 轨。quality_gate **永不热路径 LLM**。
> **v0.26.39 幕后信息架构**：侧栏五组；`Insights` 三 Tab；Settings 七→八 Tab（+扩展）；拆书设置就近。
> **v0.26.38 提示词组合智能化**：`FrameworkSelections` methodology/injectors 回灌；`preview_prompt_composition`；quality_gate 仅日志。
> **v0.26.34 提示词注册表可观测性**：`prompts/registry.rs` 新增 `get_prompts_directory()` 暴露当前 prompts 资源目录路径；`prompts/commands.rs` 新增 `get_prompts_directory` Tauri 命令；前端 `PromptsPanel` 新增「打开目录」「刷新」按钮，支持在系统文件管理器中打开 prompts 资源目录并重新加载列表；修复批量导入时 `promptId` → `prompt_id` 参数命名不匹配问题。
> **v0.26.28 Phase 4 架构债务与工程体验**：知识图谱手动 CRUD UI、世界构建 AI 生成、角色 AI 扩展、叙事分析图表；`genesis.rs` 策略选择步骤从后台阶段前移至 Quick Phase（v0.26.44 起 quick_phase 为四步，含开篇骨架）；`background_steps` 为 5 步；`prompts/registry.rs` 中 95 个内置提示词外部化至 `resources/prompts/{category}/{id}.md`，运行时从 Tauri 资源目录加载；`db/connection.rs` 中 ~2,650 行 inline `run_migrations` 拆分为 `src/db/migrations/V028__*.rs` … `V099__*.rs` 共 70 个编号 Rust 迁移文件，`MigrationRunner` 新增 `RustMigration` trait 统一执行 SQL 与 Rust 迁移。
> **v0.26.27 依赖解耦与文档补全**：前端 `components ↔ stores ↔ hooks ↔ frontstage` 通过新增 `types/editor.ts`、`stores/contracts/*` 解耦（`hooks/contracts/*` 仍待补齐），`appStore.ts` 不再依赖 `components/*` / `hooks/*`；Tauri `creative_engine ↔ llm` 已无互相 import，`model_gateway ↔ router` 仍有少量直接 import（后续继续向 `ports/` 迁移）；`USER_GUIDE.md` 补全 L4 诊断页（生成链路 / 意图图 / 日志）并修正 Phase 1–3 实现漂移。
> **v0.26.24 续写后处理**：TriShot 续写路径新增三层后处理（`trim_self_repetition` → `strip_existing_overlap` → `trim_dangling_tail`）+ 8% 自重复重试闸门；前端 `sanitizeContinuationOutput` 对齐。
> **v0.26.19 创世流程审计与测试加固**：对照文档全面审计「智能创作流程-创世」，分 Phase 1–4 执行。
> - **Phase 1（P0）**：修复 Gap B（空 finalContent 不锁 delivered）、P0-2（角色世界观上下文闭包捕获竞态——character 提示词读取 `bundle.world_building` 恒为空，改为先 await world 拿真实 `world_concept`）、P0-3（ChapterSwitch delivered 时序——懒加载失败不标记 delivered）。
> - **Phase 2（P1）**：后台错误可观测性（`GenesisContext.errors` 共享集合 → `genesis_runs.steps_json` + `genesis-warnings` 事件 → 前端 toast）；mutex 中毒锁加固（`unwrap_or_else(|e| e.into_inner())`）；策略移入 quick phase 经评估暂缓（已于 v0.26.28 完成）；`window/mod.rs` 与 `FrontstageEvent.ts` 注释对齐 auto-accept 真实路径。
> - **Phase 3（测试）**：8% 重试闸门 + ChapterSwitch payload 提取纯函数 + 契约测试；前端 Gap C + 状态机端点测试；**跨层共享 trim golden fixture**（`tests/fixtures/trim_golden.json`，Rust + TS 双跑锁定 `trim_self_repetition`/`trimSelfRepetition` 跨层一致性）。
> - **Phase 4（整洁）**：`*_future` → `*_gen` 重命名（澄清顺序 await）；`AppConfig::load` 去重；`appendAiContent` skip 路径不 `markAccepted`；Gap C 重复入站也跳过 setContent；`isGenesisSettingUpRef` 合并评估——不合并（覆盖窗口不同）。
> **v0.26.18 稳定性补丁**：加固 Genesis 第一章重复的三个残留竞态缺口（ChapterSwitch 空内容、delivered 误锁、selectChapter 咽喉点缺守卫）。
> **v0.26.17 稳定性补丁**：Issue #4 一级根因加固——生产包打包 SQL 迁移；`init_db` 启动前确保 app data 目录并增强失败诊断。
> **v0.26.16 稳定性补丁**：根治 Genesis 新小说第一章内容重复，并修复 init_db 失败时启动 panic。
> - 生成侧验证闸门：`genesis.rs` 检测 LLM 输出自重复比例，≥8% 时用更强 anti-repeat 指令重试；prompt 模板新增「结构纪律」段。
> - 前端单写者状态机：`FrontstageApp` 将 `genesisAutoAcceptedRef` 布尔替换为 `idle → generating → delivered` 三态状态机，`generating` 态阻塞外部内容投递，`delivered` 态阻塞幽灵文本恢复。
> - Issue #4：`GatewayExecutor::new` 显式接收 `pool`，`setup` 仅在 pool 可用时初始化网关，避免 `state::<DbPool>()` 在启动时 panic。
> **v0.26.14 稳定性补丁**：修复 Genesis 新小说第一章模型输出自重复并降低幕前诊断日志压力。日志证实 v0.26.13 数据层与渲染层均未重复追加内容，用户看到的「首尾段落相同」来自 LLM 生成的正文自身循环。新增 `trimSelfRepetition` 工具，在 `appendAiContent` 与 `smart_execute.finalContent` 进入编辑器/幽灵文本前做段落级与 KMP border 级自重复清理；同时 `RichTextEditor` 的 `frontstage:rich_editor_diag` 渲染日志从每帧记录改为前 20 帧 + 幽灵状态变化 + 200ms IPC 节流，减少长时间写作或文思活跃模式下的 IPC 与日志开销。
> **v0.26.13 稳定性补丁**：修复 Genesis 新小说第一章渲染层视觉重复。数据层已确保只追加一次，但 `RichTextEditor` 的幽灵树条件 `!!(generatedText || isGenerating)` 会在 `generatedText` 为空但 `isGenerating=true` 时渲染空幽灵容器；该容器若残留旧内容或 React 复用 DOM 节点异常，就会造成「正文 + 幽灵文本」同框的虚假重复。改为 `!!generatedText` 后幽灵树只在有实际幽灵文本时渲染；`FrontstageApp` Genesis 自动接受路径先 `setIsGenerating(false)` 确保幽灵树立即卸载；诊断日志增加 `isGenerating`、`isHidingGhost`、`bodyHidingGhost`、`generatedTextLen`。
> **v0.26.12 稳定性补丁**：修复 `RichTextEditor` 角色点击 effect 在 `characters` 为 `null` 时访问 `.length` 导致的幕前白屏崩溃；加固 `useSubscription` 对 `null` 订阅状态的兼容；新增 Playwright E2E 回归测试覆盖「已有故事 + 新写末世小说」完整流程；`frontstage/main.tsx` 与 `ErrorBoundary` 增强崩溃诊断输出。
> **v0.26.11 稳定性补丁**：修复 Genesis 自动接受第一章后 store-editor 失步问题。`appendAiContent` 追加后立即用 `editorRef.getHTML()` 同步 store 与 `latestContentRef`；`RichTextEditor.appendText` 空文档分支标记外部同步并更新 `lastExternalContentRef`，防止 content prop 被外部同步 effect 再次 setContent；`RichTextEditorRef` 新增 `getHTML()`。同时确认 `tauri.conf.json` `devUrl` 指向 dev server，避免开发模式加载陈旧 dist 崩溃。
> **v0.26.9 稳定性补丁**：在 v0.26.8 基础上进一步根治 Genesis 新小说第一章内容重复问题。重复检测与前缀去重统一改用 `latestContentRef.current`（React state 同步快照），避免 TipTap DOM 滞后导致已有正文被再次追加或恢复为幽灵文本；`RichTextEditor` 幽灵文本直接包含检测剥离 HTML 标签，覆盖 ContentUpdate/AppendContent 路径。
> **v0.26.8 稳定性补丁**：在 v0.26.7 基础上彻底修复 Genesis 新小说第一章内容重复问题。新增 `isTextDuplicate` 归一化去重工具与 `isTextAlreadyInEditor` helper，覆盖 pipeline-complete / ChapterSwitch / smart_execute 等多条竞态路径，确保 DB 正文不会与幽灵文本叠加。
> **v0.26.7 稳定性补丁**：修复 `FrontstageApp` pipeline-complete effect 无限循环（React #185）与 Genesis 新小说第一章内容重复问题，核心回调全部稳定化。
> **v0.26.0 重大变更**：数据飞轮 + Harness 可观测性 + 子代理协作。`WorkspaceService` 初始化 `.storymoss/` 工作空间并自动 Git 提交；`PreferencePairExporter` 把用户接受/拒绝反馈导出为 RLHF 成对数据；`TraceStore` 为每次生成请求建立 `trace_id`，在 `GatewayRequest` / `GenerateRequest` / `LlmGeneratingProgress` 全链路透传，前端新增「生成链路」面板；`Subagent` trait 与 `ContinuityAgent` / `StyleAgent` / `WorldAgent` 提供异步协作审查。
> **v0.25.0 重大变更**：Context Rot 显式防御 + 四级错误分类与恢复。`ContextPrioritizer` 按 Critical/High/Normal/Background 排序系统提示词并双重锚定关键约束；`ErrorSeverity` 把错误分为 Fatal/Retry/Degraded/UserAction，后端支持指数退避重试与降级回退，前端 `AgentInterruptionModal` 显式中断 Fatal/UserAction 错误。
> **v0.23.74 重大变更**：场景优先架构迁移完成——`scenes.content` 为唯一叙事真相源，`chapters.content` 为只读聚合投影。创世提示词场景化，幕前编辑器纯正文无缝拼接。

## 架构理念

StoryMoss 采用创新的**剧院式双界面架构 + 场景化叙事 + 增强记忆系统**：

- **幕前 (Frontstage)**: 沉浸式写作界面，如同登台演出
- **幕后 (Backstage)**: 专业工作室，如同后台准备
- **场景 (Scene)**: 戏剧冲突驱动的叙事单位，取代传统章节
- **记忆 (Memory)**: 基于 llm_wiki 的知识图谱，真正的"越写越懂"

---

## 系统架构图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        StoryMoss (草苔) v0.23.13                          │
│              Tauri 2.4 + React 18 + TypeScript 5.8 + Vite 6              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────┐        ┌─────────────────────────┐         │
│  │     🎭 幕前 Frontstage   │        │     🎬 幕后 Backstage    │         │
│  │    (沉浸式写作界面)      │        │    (专业工作室)          │         │
│  ├─────────────────────────┤        ├─────────────────────────┤         │
│  │                         │        │                         │         │
│  │  • 极简阅读写作界面      │◄──────►│  • 故事/场景/角色管理     │         │
│  │  • TipTap 富文本编辑器   │        │  • LLM 模型配置中心       │         │
│  │  • 场景大纲侧边栏        │        │  • 技能系统               │         │
│  │  • AI 续写辅助          │        │  • 知识图谱浏览          │         │
│  │  • 写作风格切换          │        │  • 工作室配置管理        │         │
│  │  • 禅模式全屏           │        │  • 数据导出/分析          │         │
│  │  • 角色卡片弹窗          │        │                         │         │
│  │                         │        │                         │         │
│  │  暖色调 (#f5f4ed)        │        │  深色主题 (Cinema)       │         │
│  │  Claude 阅读体验设计     │        │  电影感专业界面          │         │
│  │                         │        │                         │         │
│  └──────────┬──────────────┘        └──────────┬──────────────┘         │
│             │                                   │                        │
│             └───────────────┬───────────────────┘                        │
│                             ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Tauri Bridge (IPC)                            │   │
│  │           Commands + Events + Window Management                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                             │                                          │
│  ┌──────────────────────────┴──────────────────────────────────────┐   │
│  │                      Backend (Rust) - v0.23.13 Core                │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │                                                                  │   │
│  │  🎪 SCENE SYSTEM (场景化叙事)                                     │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │  • Scene: 戏剧目标、外部压迫、冲突类型、角色冲突         │   │   │
│  │  │  • StoryTimeline: 可视化场景序列、拖拽排序              │   │   │
│  │  │  • SceneGenerator: AI 场景生成建议                      │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  │  🧠 MEMORY SYSTEM (增强记忆系统)                                  │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │  Layer 4: Multi-Agent Sessions (世界观/人物/文风助手)    │   │   │
│  │  │  Layer 3: Knowledge Graph (带权实体关系图谱)             │   │   │
│  │  │  Layer 2: Vector Store (CJK分词语义检索)                 │   │   │
│  │  │  Layer 1: Raw Sources (场景正文、角色设定)               │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │   │
│  │  │   Agents    │  │   Skills    │  │      LLM Adapter        │ │   │
│  │  │  ├─ Writer  │  │  ├─ Loader  │  │  ├─ OpenAI             │ │   │
│  │  │  ├─ NovelCreation│ ├─ Executor│ │  ├─ Anthropic         │ │   │
│  │  │  ├─ Planner │  │  ├─ Registry│  │  ├─ Ollama (本地)      │ │   │
│  │  │  ├─ Style   │  │  └─ Builtin │  │  └─ Azure/DeepSeek...  │ │   │
│  │  │  └─ Plot    │  │             │  │                         │ │   │
│  │  └─────────────┘  └─────────────┘  └─────────────────────────┘ │   │
│  │                                                                  │   │
│  │  📦 STUDIO SYSTEM (工作室配置)                                    │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │  • StudioConfig: 每部小说独立配置                        │   │   │
│  │  │  • Import/Export: ZIP格式导入导出                        │   │   │
│  │  │  • Theme System: 幕前暖色/幕后暗色默认主题              │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                             │                                          │
│  ┌──────────────────────────┴──────────────────────────────────────┐   │
│  │                      Data Layer                                   │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │                                                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │   │
│  │  │   SQLite    │  │  LanceDB    │  │    File System          │ │   │
│  │  │  (r2d2池)   │  │  (向量检索)  │  │  • 技能库               │ │   │
│  │  │  • Stories  │  │  • 场景嵌入  │  │  • 导出文件             │ │   │
│  │  │  • Scenes*  │  │  • 实体向量  │  │  • 工作室配置           │ │   │
│  │  │  • Chapters │  │  • 语义搜索  │  │                         │ │   │
│  │  │  • Characters│ │             │  │  *Scene 为内容真相源     │ │   │
│  │  │  • KG Entities│ └─────────────┘  └─────────────────────────┘ │   │
│  │  │  • KG Relations                                                 │
│  │  │  • WorldBuilding                                                │
│  │  └─────────────┘                                                  │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 核心系统详解

### 🛡️ 上下文健康与错误恢复 (v0.25.0)

v0.25.0 在原有智能创作链路之上增加了两道防御性基础设施：

#### 1. Context Rot 显式防御

长系统提示词容易出现 "Lost in the Middle"——模型对中间段落的约束记忆衰减。`ContextPrioritizer` 把系统提示词拆分为带优先级的 `ContextChunk`：

- **Critical**：合同红线、在世作者保护、反 AI 陈词滥调等绝对不能违反的约束
- **High**：角色核心、伏笔、四元组等强信号上下文
- **Normal**：写作风格、方法论、世界设定等常规上下文
- **Background**：历史参考、弱关联记忆等辅助信息

排序后 Critical 信息同时出现在提示词开头和结尾的轻量摘要中，实现双重锚定。`ContextHealthMetrics` 记录各类别的 token 数与预算使用率，供诊断卡片实时查看。

#### 2. 四级错误分类与恢复

后端错误统一注入 `ErrorSeverity`：

| Severity       | 含义         | 典型场景              | 恢复策略                                    |
| -------------- | ------------ | --------------------- | ------------------------------------------- |
| **Fatal**      | 无法自动恢复 | 未知内部 panic        | 记录并终止，前端弹 `AgentInterruptionModal` |
| **Retry**      | 瞬态错误     | 模型连接超时、DB 锁定 | 指数退避重试，或切换候选模型                |
| **Degraded**   | 可降级继续   | 上下文缺失            | 执行简化路径并提示用户                      |
| **UserAction** | 需要用户处理 | 订阅、校验、模型禁用  | 前端弹 `AgentInterruptionModal` 引导设置。例外：`active_run`（该故事已有进行中的创作任务）不中断纸面，只留底栏。 |

`error_recovery.rs` 提供 `retry_with_backoff` 与 `with_degraded_fallback`；`GatewayExecutor` 与 `smart_execute` 的 DB 加载已接入重试。前端 `errorHandler.ts` 按 `severity` 兜底推荐动作，`AgentInterruptionModal` 对 Fatal/UserAction 直接中断当前流程。

### 🏗️ 分层架构 (v0.23.6)

v0.23.6 在 v0.19.0 分层基础上进一步完成 **全局单例清零**（14 个 `static`/缓存改为 Tauri State 注入）与 **模块循环依赖斩断**，并新增 **PromptSynthesis 提示词合成层** 支撑 TriShot 三击生成管线。当前核心调用链为：

```
Frontend (React)
      │ invoke / listen
      ▼
Tauri Command Layer (commands/*.rs)   ← 薄层：参数校验 + EmitSync
      │
      ├─ DTO (db/dto.rs)              ← 请求/响应序列化对象
      │
      ├─ Domain Service                 ← 业务编排（story_system/*_service.rs）
      │
      ▼
Repository Layer (db/repositories*.rs) ← 数据访问
      │
      ▼
SQLite / LanceDB / File System
```

#### Command 层 (`src-tauri/src/commands/`)

- 按领域拆分为 20 个文件：`story.rs`、`chapter.rs`、`scene_commands.rs`、`character.rs`、`memory.rs`、`story_system.rs`、`pipeline.rs` 等
- 统一使用 `State<'_, DbPool>` 注入，返回 `Result<T, AppError>`
- 通过 `commands/utils.rs` 的 `EmitSync` trait 在变更后发射 `sync-event`

#### DTO 层 (`src-tauri/src/db/dto.rs`)

- v0.9.0 新增：将 18+ 个请求/响应结构体从 `models.rs` 迁出
- 包括：`CreateSceneRequest`、`UpdateSceneRequest`、`CreateStoryRequest`、`UpdateStoryRequest`、`CreateCharacterRequest`、`CreateChapterRequest`、`CreateAiOperationRequest`、`StudioExportRequest` 等
- 原则：贫血模型（`models.rs`）只保留数据库实体，DTO 只承担序列化/反序列化职责

#### 领域服务层 (`src-tauri/src/story_system/`)

- `chapter_service.rs`：章节变更后的伏笔检测、自动化触发（v0.23.74: `ChapterCommitDebouncer` 已由 `SceneCommitDebouncer` 接替）
- `scene_service.rs`：场景内容变更后的 KG Ingest、向量索引、world_building 刷新、**SceneCommitDebouncer**（30s 防抖 auto_commit + 分章 + v0.51.0 观察/Ingest 同窗分流）
- `commit_service.rs`：`SceneCommitService` — 场景级 commit，驱动 5 个 Projection Writer
- `mod.rs`：`StorySystemEngine`、ContractTree / RuntimeContract

#### Repository 层 (`src-tauri/src/db/repositories*.rs`)

- `repositories.rs`：Story / Scene / Chapter / Character 通用仓库
- `repositories_story_system.rs`：StoryContract / SceneCommit / MemoryItem / ReadingPower 等专用仓库
- `repositories_narrative.rs` / `repositories_pipeline.rs` / `repositories_export.rs`：各垂直域仓库

#### PromptRegistry 层 (`src-tauri/src/prompts/`)

- v0.19.0 新增：统一 LLM 提示词注册表，所有硬编码提示词提取为可配置项
- `registry.rs`：35+ 内置 prompt，15 个 `PromptCategory` 分类，支持 `resolve_prompt()` 运行时读取覆盖
- `commands.rs`：IPC 命令（`list_prompt_entries`、`save_prompt_override`、`reset_prompt_override`、`reset_all_prompt_overrides`、`resolve_prompt_content`）
- 消费者：Writer/Inspector/Commentator/Planner/Analyzer/Probe/Memory/Knowledge/Skill/Methodology 等全部模块

---

### 🗄️ 数据库与迁移框架 (v0.19.0)

#### 数据层组件

| 组件                        | 路径                             | 职责                                             |
| --------------------------- | -------------------------------- | ------------------------------------------------ |
| `connection.rs`             | `src-tauri/src/db/connection.rs` | `DbPool`、初始 Schema、`init_db()`、遗留内联迁移 |
| `migrations.rs`             | `src-tauri/src/db/migrations.rs` | 自定义 `MigrationRunner`，扫描 `.sql` 文件       |
| `migrations/V007~V027*.sql` | `src-tauri/src/db/migrations/`   | 21 个版本化 SQL 迁移                             |
| `dto.rs`                    | `src-tauri/src/db/dto.rs`        | 请求/响应 DTO                                    |
| `models.rs`                 | `src-tauri/src/db/models.rs`     | 数据库实体模型                                   |
| `traits.rs`                 | `src-tauri/src/db/traits.rs`     | 仓库 trait 抽象                                  |

#### MigrationRunner 设计

- 因项目使用 rusqlite 0.39，**自定义实现** `MigrationRunner`（未使用 refinery 默认 rusqlite 特性）
- 扫描 `V{version}__{description}.sql` 文件名，按版本排序
- 通过 `schema_migrations` 表追踪已应用版本
- 每个迁移在独立事务中执行；自动忽略 SQL 中的 `BEGIN/COMMIT/ROLLBACK`
- 对 `duplicate column name` / `already exists` 等错误幂等跳过
- `run_with_legacy()` 先跑 SQL 迁移，再跑遗留 Rust inline 迁移，保证平滑升级

---

### 🎪 场景化叙事系统 (Scene System)

#### 场景模型

```rust
pub struct Scene {
    pub id: String,
    pub story_id: String,
    pub sequence_number: i32,      // 场景序号
    pub title: String,

    // 戏剧结构
    pub dramatic_goal: String,      // 戏剧目标
    pub external_pressure: String,  // 外部压迫
    pub conflict_type: ConflictType, // 冲突类型

    // 角色参与
    pub characters_present: Vec<String>,
    pub character_conflicts: Vec<CharacterConflict>,

    // 内容
    pub content: String,
    pub setting: Setting,

    // 关联
    pub previous_scene_id: Option<String>,
    pub next_scene_id: Option<String>,
}

pub enum ConflictType {
    ManVsMan,        // 人与人
    ManVsSelf,       // 人与自我
    ManVsSociety,    // 人与社会
    ManVsNature,     // 人与自然
    ManVsTechnology, // 人与科技
    ManVsFate,       // 人与命运
}
```

#### 场景 vs 章节

| 特性     | 章节 (Chapter) | 场景 (Scene)    |
| -------- | -------------- | --------------- |
| 驱动方式 | 时间/长度驱动  | 戏剧冲突驱动    |
| 结构     | 线性序列       | 网络化关联      |
| AI 理解  | 文本内容       | 戏剧目标 + 冲突 |
| reorder  | 简单排序       | 依赖关系维护    |

---

### 🧠 增强记忆系统 (Memory System)

基于 [karpathy/llm_wiki](https://github.com/karpathy/llm_wiki) 方法论实现。

#### 四层架构

```
┌─────────────────────────────────────────┐
│  Layer 4: Multi-Agent Sessions          │
│  - WorldBuilding Agent (世界观助手)      │
│  - Character Agent (人物助手)            │
│  - WritingStyle Agent (文风助手)         │
│  - Plot Agent (情节助手)                 │
│  - Scene Agent (场景助手)                │
│  - Memory Agent (记忆助手)               │
├─────────────────────────────────────────┤
│  Layer 3: Knowledge Graph               │
│  - Entity (实体)                        │
│  - Relation (关系，带 strength 0-1)      │
│  - 关系强度动态计算                      │
├─────────────────────────────────────────┤
│  Layer 2: Vector Store                  │
│  - CJK Bigram Tokenizer                 │
│  - 语义检索                              │
│  - 相似度搜索                            │
├─────────────────────────────────────────┤
│  Layer 1: Raw Sources                   │
│  - 场景正文                              │
│  - 角色设定                              │
│  - 世界设定                              │
└─────────────────────────────────────────┘
```

#### 两步思维链 Ingest

```rust
impl IngestPipeline {
    pub async fn ingest(&self, content: &IngestContent) -> Result<(), Error> {
        // Step 1: 分析阶段
        let analysis = self.analyze_content(content).await?;
        // 提取：实体、关系、事件、情感、伏笔

        // Step 2: 生成阶段
        let knowledge = self.generate_knowledge(&analysis).await?;
        // 生成：实体档案、关系强度、事件重要性

        // 保存
        self.save_to_graph(&knowledge).await?;
        self.save_to_vector_store(&knowledge).await?;

        Ok(())
    }
}
```

#### 四阶段查询检索

```rust
impl QueryPipeline {
    pub async fn query(&self, query: &str) -> Result<QueryResult, Error> {
        // Stage 1: CJK二元组分词搜索
        let search_results = self.token_search(query).await?;

        // Stage 2: 图谱扩展（基于关系强度）
        let graph_expansion = self.graph_expansion(&search_results).await?;

        // Stage 3: 预算控制（4K-1M tokens可配）
        let selected = self.budget_control(
            &search_results,
            &graph_expansion
        ).await?;

        // Stage 4: 带引用编号的上下文组装
        let context = self.assemble_context(&selected).await?;

        Ok(QueryResult { context, citations })
    }
}
```

---

### 🤖 AI 智能生成系统

#### NovelCreationAgent

```rust
pub struct NovelCreationAgent {
    llm_adapter: Arc<dyn LlmAdapter>,
}

impl NovelCreationAgent {
    /// 根据用户输入生成世界观选项（3个）
    async fn generate_world_building_options(
        &self,
        user_input: &str,
    ) -> Result<Vec<WorldBuilding>, Error>;

    /// 根据世界观生成角色谱选项
    async fn generate_character_profiles(
        &self,
        world_building: &WorldBuilding,
    ) -> Result<Vec<Vec<CharacterProfile>>, Error>;

    /// 生成文字风格选项
    async fn generate_writing_styles(
        &self,
        genre: &str,
        world_building: &WorldBuilding,
    ) -> Result<Vec<WritingStyle>, Error>;

    /// 生成首个场景
    async fn generate_first_scene(
        &self,
        story_context: &StoryContext,
    ) -> Result<Scene, Error>;
}
```

#### 引导式创建流程

```
用户输入类型 → AI生成世界观选项 → 用户选择/编辑 →
AI生成角色谱选项 → 用户选择/编辑 →
AI生成文字风格选项 → 用户选择/编辑 →
AI生成首个场景 → 开始创作
```

---

### 📦 工作室配置系统

#### 配置架构

```
~/.config/storymoss/
├── config.json              # 全局配置
└── studios/
    └── {story_id}/
        ├── studio.json          # 工作室主配置
        ├── llm_config.json      # LLM配置
        ├── ui_config.json       # 界面配置
        ├── agent_bots.json      # Agent配置
        └── ...
```

#### 导入/导出

```rust
pub struct StudioManager;

impl StudioManager {
    /// 导出工作室配置到 .storymoss ZIP
    pub async fn export_studio(
        &self,
        story_id: &str,
        output_path: &Path,
    ) -> Result<()>;

    /// 从 .storymoss ZIP 导入工作室配置
    pub async fn import_studio(
        &self,
        import_path: &Path,
        options: ImportOptions,
    ) -> Result<ImportResult>;
}
```

---

### 📜 场景版本系统 (Phase 3.x)

**版本管理架构**

```
┌─────────────────────────────────────────┐
│         Scene Version System            │
├─────────────────────────────────────────┤
│                                         │
│  SceneVersionRepository                 │
│  ├─ create_version()     # 创建快照     │
│  ├─ get_versions()       # 获取历史     │
│  ├─ get_version()        # 获取特定版本 │
│  └─ delete_version()     # 删除版本     │
│                                         │
│  SceneVersionService                    │
│  ├─ compare_versions()   # 版本对比     │
│  ├─ restore_version()    # 恢复版本     │
│  ├─ get_version_chain()  # 版本链       │
│  └─ get_version_stats()  # 统计信息     │
│                                         │
│  VersionTimeline (React)                │
│  ├─ VersionCard          # 版本卡片     │
│  ├─ DiffViewer           # 差异查看     │
│  └─ ConfidenceIndicator  # 置信度指示   │
│                                         │
└─────────────────────────────────────────┘
```

**版本模型**

```rust
pub struct SceneVersion {
    pub id: String,
    pub scene_id: String,
    pub version_number: i32,        // 版本号 (v1, v2, ...)

    // 内容快照
    pub title: Option<String>,
    pub content: Option<String>,
    pub dramatic_goal: Option<String>,
    pub conflict_type: Option<ConflictType>,

    // 版本元数据
    pub word_count: i32,
    pub change_summary: String,
    pub created_by: CreatorType,    // user/ai/system
    pub confidence_score: Option<f32>,

    // 版本链
    pub previous_version_id: Option<String>,
    pub superseded_by: Option<String>,
}
```

### 🔍 混合搜索系统 (Phase 1.3)

**RRF 融合排序**

```
┌─────────────────────────────────────────┐
│          Hybrid Search                  │
├─────────────────────────────────────────┤
│                                         │
│  Query: "主角与反派的冲突"               │
│                                         │
│  ┌─────────────┐    ┌─────────────┐    │
│  │ BM25 Search │    │Vector Search│    │
│  │  (CJK分词)  │    │(余弦相似度) │    │
│  └──────┬──────┘    └──────┬──────┘    │
│         │                   │           │
│         ▼                   ▼           │
│  ┌─────────────────────────────────┐   │
│  │    RRF Fusion (k=60)            │   │
│  │    score = Σ(1/(k+r))           │   │
│  └─────────────────────────────────┘   │
│                    │                    │
│                    ▼                    │
│         ┌──────────────────┐            │
│         │  Hybrid Results  │            │
│         └──────────────────┘            │
│                                         │
└─────────────────────────────────────────┘
```

### 🧠 记忆保留系统 (Phase 1.4)

**艾宾浩斯遗忘曲线**

```
R(t) = R₀ × e^(-λt) + Σ(强化奖励)

其中:
- R₀: 初始置信度
- λ: 衰减率 (架构级 0.01, 默认 0.05, 瞬态 0.1)
- t: 距离上次访问的天数
- Σ(强化): 每次访问增加的奖励
```

**优先级分级**

```rust
pub enum PriorityLevel {
    Critical,    // > 0.8  - 必须保留
    High,        // 0.6-0.8 - 优先保留
    Medium,      // 0.4-0.6 - 正常保留
    Low,         // 0.2-0.4 - 可压缩
    Forgotten,   // < 0.2  - 可归档
}
```

---

### 🏛️ Story System 合同驱动体系 (v6.0.0)

**四级合同架构**

```
┌─────────────────────────────────────────┐
│           Story System                  │
├─────────────────────────────────────────┤
│                                         │
│  MASTER_SETTING (故事级全局设定)          │
│  └─ Volume (卷级设定)                    │
│     └─ Chapter (章节级设定与预期)         │
│        └─ Review (审阅与修订合同)         │
│                                         │
│  SCENE_COMMIT (写后真源)                 │
│  ├─ state_deltas_json                   │
│  ├─ entity_deltas_json                  │
│  ├─ accepted_events_json                │
│  └─ projection_status_json              │
│                                         │
│  5 Projection Writers                   │
│  ├─ StateProjectionWriter               │
│  ├─ IndexProjectionWriter               │
│  ├─ SummaryProjectionWriter             │
│  ├─ MemoryProjectionWriter              │
│  └─ VectorProjectionWriter              │
│                                         │
│  ContractTree / RuntimeContract         │
│  └─ 动态合并上层合同 → 运行时约束         │
│                                         │
└─────────────────────────────────────────┘
```

**领域服务下沉 (v0.9.0)**

- `story_system/chapter_service.rs`：`ChapterService` 统一处理 `on_chapter_updated` / `on_chapter_created`
  - `ChapterCommitDebouncer`：30 秒 debounce 后调用 `SceneCommitService::auto_commit`
  - `PayoffDetector`：检测逾期伏笔并发射 `PayoffOverdue` 事件
  - `AutomationTrigger`：触发 `ChapterContentUpdated` / `ChapterCreated` 自动化事件
- `story_system/scene_service.rs`：`SceneService` 统一处理 `on_scene_updated` / `on_scene_created` / `on_scene_deleted`
  - `SceneIngestor`：后台 KG Ingest + 向量索引更新
  - `SceneAutomationTrigger`：触发 `SceneContentUpdated` / `SceneCreated`
- 原 `commands/chapter.rs` 与 `scene_commands.rs` 只保留薄封装：参数校验 → 调用 Service → 发射同步事件

**防幻觉三定律**

1. 合同即法律 — 所有生成内容受合同约束
2. 设定即物理 — 世界观设定如物理定律般不可违背
3. 发明需识别 — 新实体必须被明确识别并记录

---

### 🧠 三层记忆编排器 (v6.0.0)

**MemoryOrchestrator 架构**

```
┌─────────────────────────────────────────┐
│      MemoryOrchestrator                 │
├─────────────────────────────────────────┤
│                                         │
│  Working Memory (50% budget for write)  │
│  ├─ 最近 5 章正文摘要                    │
│  ├─ 活跃角色（出场 > 3 次）               │
│  └─ 开放伏笔（未回收）                    │
│                                         │
│  Episodic Memory (30% budget)           │
│  ├─ state_changes 时间线                 │
│  └─ relationships 演变                   │
│                                         │
│  Semantic Memory (20% budget)           │
│  ├─ 长期事实（按优先级过滤）              │
│  │   Critical > High > Medium > Low     │
│  └─ 源章节窗口过滤（最近 30 章）          │
│                                         │
│  MemoryPack 组装                        │
│  └─ 按任务类型分配预算权重               │
│      write: 50/30/20                    │
│      plan:  20/30/50                    │
│      review: 30/40/30                   │
│                                         │
└─────────────────────────────────────────┘
```

---

### 📈 追读力评估系统 (v6.0.0)

**ReadingPowerEvaluator 五维评估**

```rust
pub struct ReadingPowerEvaluation {
    pub overall_score: f32,        // 0-100
    pub hook_count: i32,           // 悬念/冲突/转折
    pub coolpoint_count: i32,      // 打脸/收获/揭秘
    pub micropayoff_count: i32,    // 小承诺兑现
    pub debt_count: i32,           // 未兑现承诺
    pub trend: Vec<f32>,           // 最近 N 章趋势
}
```

**DebtManager 债务追踪**

- 创建债务 → 逾期计息（每日 5%）→ 覆盖合同跳过 → 兑现销账

---

### 📚 体裁模板库 (v6.0.0)

**GenreProfile 外部化**

- 启动时优先读取 `{app_data_dir}/templates/genres.json`
- 内置 37 个网文体裁模板，支持自定义编辑
- 模板五要素：核心基调、节奏策略、反模式清单、参考数据表、典型结构

---

### 🔍 Anti-AI 五维审查 (v6.0.0)

**AntiAiReviewer 架构**

- 词汇维度：Cliché 检测 + 重复用词
- 语法维度：句式多样性 + 被动语态
- 叙事维度：段落均匀度 + 感官密度
- 情感维度：标签化检测 + 展示 vs 告知
- 对话维度：说明性对话 + 标签单调性
- 输出：overall_score + issues + suggestions + flagged_passages

---

### ⏱️ 分时介入架构 (v0.13.0)

**解决的核心矛盾**：AI 长篇小说创作中"质量与速度不可兼得"——强化专业资产介入导致生成过慢，放松则质量低劣。

**根因诊断（B + E）**：

- **B（资产被错误同步化）**：合同、伏笔、Inspector、记忆各有最佳发力时机，却全压在"Writer 一次 LLM 调用"那个点上
- **E（写与审被错误耦合）**：写（快）和审（深）被焊死在一条同步链路，用户全程干等

**第一性原理**：把大灾难变成即时可见的小债务。蚂蚁搬家，不积巨石。

**三条时间线**：

```
┌─────────────────────────────────────────────────────────────┐
│  时间线 1：写作时刻（热路径，< 15s，用户等待）                │
│  QuickPreflightChecker → WriteTimeBundle（红线突出+题材自适应）│
│  → generate_for_task 直连 LLM → 立即返回正文                  │
│  代码：execute_time_sliced (agents/orchestrator.rs)          │
└─────────────────────────────────────────────────────────────┘
          │ 正文已返回，spawn 后台（不阻塞）
          ▼
┌─────────────────────────────────────────────────────────────┐
│  时间线 2：审计时刻（温路径，30-90s，后台异步）                │
│  AuditExecutor → Inspector 7 维审计（memory 优先）            │
│  → create_annotation_with_meta（type=ai_audit）              │
│  → emit SyncEvent::AnnotationCreated → 前端自动渲染标注       │
│  代码：task_system/audit_executor.rs                          │
└─────────────────────────────────────────────────────────────┘
          │ 每 5 段条件触发
          ▼
┌─────────────────────────────────────────────────────────────┐
│  时间线 3：洞察时刻（冷路径，分钟级，跨章节深度）              │
│  InsightExecutor → 追读力趋势 + 债务汇总 + annotation 盘点    │
│  → 整体健康度评分 → story_summaries → NarrativeAnalysis 页    │
│  代码：task_system/insight_executor.rs                        │
└─────────────────────────────────────────────────────────────┘
```

**GenerationMode 三值**（v0.41.0 起仅约束**改写**路径；幕前续写固定 Agency Append，不再读 TimeSliced/TriShot）：

- `Fast`：Ghost Text 等实时补全（原有，不变）
- `TimeSliced`：历史默认三时间线；**续写已切断**（`execute_time_sliced` 不再是续写入口），设置 UI 已移除该选项
- `Full`：同步审计+Rewrite 闭环（划词改写 / Planner / Workflow）

**Phase 0 实测验证**（qwen3.6-35b，3 场景 A/B 盲测）：

- 最小约束 vs 全量资产平均质量差距 **7.9%**（< 30% 阈值）→ 架构成立
- prompt 长 160% 仅耗时多 7% → 证实"慢在同步链路而非 Writer 本身"
- 三条实证改进：红线突出注入、题材自适应 bundle、memory 维度优先审计

**前端可见物**：

- 顶栏 **DebtIndicator**（债务指示器）：未处理 annotation 计数，超阈值红色警告
- 编辑器内 **TextAnnotationMark**：ai_audit 类型按 severity 动态着色（high=红/medium=琥珀/low=蓝）
- 叙事分析页 **深度洞察 section**：健康度仪表盘 + 追读力趋势柱状图

**设计文档**：[`docs/plans/2026-06-14-time-sliced-intervention-design.md`](./docs/plans/2026-06-14-time-sliced-intervention-design.md)

---

### 📊 可观测性系统 (v6.0.0)

**Ingest 作业追踪**

- `ingest_jobs` 表：pending → running → completed/failed
- `ingest-job-updated` Tauri 事件推送状态变更
- 幕前顶栏 🧠 图标实时显示最近 Ingest 健康状态

**功能使用度量**

- `feature_usage_logs` 表：feature_id / action / story_id / metadata
- `telemetry/mod.rs`：本地 SQLite 记录，零网络传输
- Settings 页面「数据统计」标签：30 天柱状图

**Projection 健康检查**

- 解析 `scene_commits.projection_status_json`
- 逐 Writer 展示成功/失败状态与错误原因

---

### 🔒 类型安全基座 (v6.0.0)

**ts-rs 自动生成**

- Rust `SyncEvent` / `FrontstageEvent` / `BackstageEvent` 添加 `#[derive(TS)]`
- 编译时生成 TypeScript 绑定到 `src-frontend/src/generated/`

**前端穷尽匹配**

```typescript
function assertUnreachable(x: never): never {
  throw new Error(`Unhandled case: ${x}`);
}
// default case 中使用，新增 variant 时编译失败
```

**IPC 一致性检查**

- `scripts/verify-ipc-manifest.py` 解析 `generate_handler![]` 与前端 `loggedInvoke`
- 前端调用未注册命令时报 ERROR

---

### 🤝 Agency 多代理创作框架（创世 2.0）

**职责**：多代理创作框架的创世 2.0 实现——黑板模型 + ReAct 工具循环 + 三角色（主创 Writer / 管理 Producer / 编辑审计 Editor），端到端从一句话 premise 生成新故事（世界观/角色/大纲/首章草稿），并支持逐章续写的并行稳态循环。

**模块**：`src-tauri/src/agency/`

- `board.rs`：BlackboardService（资产区/草稿区/审查区分区读写）
- `tool_loop.rs`：ReAct 工具循环（原生 `tool_calls` 优先，文本 JSON action 回退）
- `tools.rs`：工具注册表（按角色白名单，内置黑板/故事工具）
- `roles.rs`：三角色 spec 与系统提示词
- `coordinator.rs`：创世/续写协调器——质量门判定（`evaluate_gate`）、并行稳态循环（编辑审第 N 章与主创写第 N+1 章并发）、request_id 定点取消
- `continue_loop.rs`：续写后台回流/活动日志；`run_asset_ingest` 供续写与观察共用
- `observe.rs`：手写/粘贴正文观察编排（设计 `docs/plans/2026-08-17-prose-observe-agency-design.md`）
- `gate.rs`：质量门规则复检（规则问题归并 + 复检上下文构建）；门径为编辑裁决 + 规则复检 + 至多 1 轮修订，未过门不装配
- `budget.rs`：AgencyBudget——按角色并发信号量（writer/producer/editor）+ run 级 token 预算硬上限（默认 30 万 tokens）+ agency 全局 LLM 并发闸门（跨 run 在途上限 3，request_id RAII 注册，锁序：先 run 级角色预算后全局闸门）
- `materialize.rs`：创作资产自动落库（characters / world_buildings / story_outlines）
- `session.rs`：SessionService——`agency_sessions` 会话快照（机械提取 + Background 档五段摘要双层）与跨会话恢复支撑
- `repository.rs` / `models.rs`：`agency_runs` / `agency_board_items` 持久化
- `bus.rs`：消息总线（P2 已接线，协调器回收代理消息）
- `commands.rs`：IPC 命令

**IPC**：`agency_start_genesis`（立即返回 run_id，进度经 `agency-run-progress` 事件推送）/ `agency_continue_chapter` / `agency_continue_batch`（续写循环）/ `agency_get_run` / `agency_list_board` / `agency_cancel_run`（按 request_id 定点取消该 run 的在途 LLM 调用，不再全局取消）。

**依赖**：db / llm / router / prompts；**被依赖**：无（禁止反向依赖）。

**提示词**：`resources/prompts/agency/`。

**创世入口**：`smart_execute` 检测到小说创建意图即切换到 agency 创世流程，进度镜像到 `smart-execute-progress`；旧 GenesisPipeline 已移除（TriShot 续写路径保留）。

**意图识别 LLM 化（v0.30.11）**：`smart_execute` 的小说创建意图检测由朴素子串匹配（`user_input.contains(pattern)`）改为 LLM 意图分类器--`IntentParser::classify_writing_intent`（`src-tauri/src/intent.rs`）单次 LLM 调用产出 `WritingIntentClassification`（`is_new_novel`/`is_continuation`/`task_type`/`is_prose_request`/`input_clarity`/`detected_genre`/`confidence`），8s 超时 + 保守降级回退 + 会话级 LRU 缓存（64）。分类结果经前端 `classifyIntent` IPC -> `smart_execute` payload -> `PlanContext.intent_classification` -> planner/executor -> `task.parameters`（`detected_genre` + `task_type_hint`）下发到 agents；新增 `classify_intent` Tauri 命令。共替换 6 处高风险子串匹配点（`is_novel_creation_intent`、`find_template` 禁用、`from_instruction_and_context` 优先级 bug 修复 + hint 参数、force-correction 读 `is_prose_request`、`extract_genre` 否定+排序、intention_graph builder LLM 加固）。

**force-correction 覆盖 inspector（v0.30.12）**：planner 防线2「强制改 writer」capability 列表纳入 `inspector`（提取纯函数 `PlanGenerator::should_force_correct_to_writer` 按 LLM 分类分流--续写/创世/无分类/审查+prose 强制 writer，纯 Audit(非prose)/Rewrite 保留 inspector），「继续写当前这部小说」误路由到 inspector 返回质检报告而非续写正文的问题修复，续写现强制改 writer；提示词 Rule 9 澄清续写≠refine、Rule 21 加入 inspector 禁用。

**P3（代币优化 + 记忆持久性）**：

- **角色×任务模型路由**：主创走 Creative 档、管理走 Tool 档、编辑审计走 Background 档（经 ModelRole 体系解析，用户可按角色指派模型），`AgencyLlm::new(app_handle, run_id, role)` 按角色构造。
- **注入预算与三档目录**：上下文注入按 token 预算截断（tiktoken 计数，超预算降级裁剪）；黑板读取分 catalog（key+summary+version）/ summary / full 三档，ToolLoop 内维护会话窗口。
- **会话快照与恢复**：`agency_sessions` 表持久化会话快照；`agency_resume_run` 跨会话恢复——黑板复制到新 run + stale-replay 防护 + `.storymoss/sessions/` 文件归档。
- **V109 并发护栏**：`idx_agency_runs_one_active_per_story` 部分唯一索引（story_id 非 NULL 且 status 进行中）在 INSERT 即原子拦截同 story 并发 run；创作角色落库去重；质量门判定轮次可追溯（`evaluate_gate(..., round)`）。

**P4（验证循环）**：

- **四级 grader**（`graders.rs`）：code（确定性：字数/自重复/合同禁则）→ rule（合同兑现/追读力/规则复检）→ model（rubric 化编辑裁决，1-5 分维度评分且须引证据，旧格式回退兼容）→ human（用户修改率后置信号，字符二元组 Jaccard 距离，不进 gate）。
- **Gate v2 加权评分**：code 0.2 / rule 0.3 / model 0.5 加权总分，阈值 0.75，取代二元判定；gate 判定条目落黑板审查区（`item_type='gate'`，content 含 outcome + gate_score）。
- **V110 里程碑检查点**：`agency_checkpoints` 表按里程碑采集指标快照（chapters_done/words_total/gate_scores/tokens_used/elapsed_s），`agency_compare_checkpoints` 输出现在 vs 当时差值。
- **eval harness**（`eval_harness.rs`）：JSON 场景 + pass@k/pass^k 指标 + baseline.json 回归门，确定性模式随 `cargo test` 纳入 CI。
- **评估仪表盘**：`agency_eval_overview` IPC 五段聚合（gate 历史 + pass_rate + checkpoints + human_signals + 按角色 token 用量）；前端 `AgencyEval` 页（侧栏诊断组「创作评估」，手绘 SVG 加权分趋势，零图表依赖）。

**P5（持续学习 + 代理可视化）**：

- **持续学习双轨**（`learning.rs`）：观察层——创作事件埋点写 `observations.jsonl`（app 数据目录，10MB 轮转，label 过滤防自观察）；后台 analyzer（`agency_analyze_learning`，Background 档模型）把观察提炼为 instinct（trigger/action/confidence 文件层存储）。
- **置信度引擎**：按证据数初始化置信度；用户反馈 `agency_instinct_feedback`（采纳 +0.05 / 纠正 −0.1）、周衰减 −0.02、低置信度 prune。
- **晋升管线**：instinct 置信度 ≥0.8 且跨 story 复现 → 晋升提案（`agency_promotion_candidates`）→ 学习中心人工确认（`agency_confirm_promotion`）→ 物化为 `skill.yaml` 技能；启动时自动 reload 已学技能。
- **学习中心页**：前端 `AgencyLearning`——模式列表与置信度、晋升提案确认/拒绝、观察流、手动「立即分析」；`agency_learning_overview` 聚合 IPC。
- **代理工作室页**：前端 `AgencyStudio`——三角色实时状态卡 + 黑板分区视图（事件驱动刷新）+ 活动时间线。
- **eval 纳入 CI 专用门禁**：CI 新增 `cargo test --lib agency::eval_harness` step；检查点对比 UI（`agency_compare_checkpoints` 前端落地）；`agency_eval_overview` 新增 story 级 token 聚合（每 run `MAX(tokens_used)` 去重后跨 run 求和）；rule grader 追读力口径对齐生产实现。

**创世快速路径（v0.30.1）**：原六阶段串行流程（12-18 次 LLM 调用）压缩为三阶段 4 次调用——Phase A 概念包单调用（Producer 档，一次产出 title/genre/logline/角色卡）→ Phase B 双模式编排（可用生成模型 >1 时主创首章 ∥ 管理深度资产 `tokio::join!` 并行；≤1 即单模型时主创优先先出首章、深度资产随后串行）→ Phase C 编辑质量门/修订/装配（与 legacy 共用 `review_and_assemble`），典型远程模型首章 ≤3 分钟。回退规则：任一单调用解析失败即回退原串行多轮流程（`run_genesis_legacy_inner`，概念包结果复用）；取消信号不属于快速路径失败，直接传播收敛为 cancelled，不进入 legacy、不产生 fallback 遥测。主创模型优先（Tool 档互斥）：`pick_fastest_for_role` 在 TTFB 排序与健康回退两个分支都排除 active/creative 模型，管理/编辑不再与主创同模型；排除后无候选（单模型场景）回退允许 active，不饿死 Tool 档。smart_execute 超时回退统一为 600s（原配置加载失败时回退 180s）。

**PROBLEM 七元素框架集成（v0.30.22）**：引入 Erik Bork 的 PROBLEM 七元素作为后端创作资产，增强 Logline 生成与故事大纲。架构要点：①`run_genesis_inner` 在 concept_pack 之前新增一次 Producer LLM 调用（`generate_logline`）--当用户 premise 为简单前提（< 100 字）时触发 PROBLEM logline 生成，产出后经 `StoryRepository::update_logline` 持久化至 `stories.logline`（V114 迁移 `ALTER TABLE stories ADD COLUMN logline TEXT`，Story 模型加 `logline: Option<String>`）；②`ensure_story_outline` 改用注册表中的 PROBLEM outline 提示词生成故事大纲，`build_continue_writer_context` 注入【故事Logline】段，两者均从 DB 读取 logline 作为权威约束；③`producer_depth_assets` 增强 PROBLEM 指导；④提示词资产 `resources/prompts/agency/agency_problem_logline.md` 与 `agency_problem_outline.md` 经 WalkDir 自动注册进 PromptRegistry。

**设计文档**：`docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（P1-P5 已完成，除真机验收外）。

---

## 目录结构

```
StoryMoss/
├── src-frontend/                    # 前端代码 (React 18 + TypeScript 5.8 + Vite 6)
│   ├── src/
│   │   ├── main.tsx                # 幕后入口
│   │   ├── App.tsx                 # 幕后主应用：路由 + 全局事件监听
│   │   │
│   │   ├── frontstage/             # 幕前界面
│   │   │   ├── main.tsx            # 幕前入口
│   │   │   ├── FrontstageApp.tsx
│   │   │   ├── components/
│   │   │   │   ├── ReaderWriter.tsx
│   │   │   │   ├── RichTextEditor.tsx
│   │   │   │   ├── CharacterPeekCard.tsx
│   │   │   │   ├── IngestHealthIndicator.tsx
│   │   │   │   └── ...
│   │   │   └── styles/
│   │   │
│   │   ├── pages/                  # 幕后页面
│   │   │   ├── Dashboard.tsx
│   │   │   ├── Stories.tsx
│   │   │   ├── Characters.tsx
│   │   │   ├── Scenes.tsx
│   │   │   ├── WorldBuilding.tsx
│   │   │   ├── KnowledgeGraph.tsx
│   │   │   ├── Skills.tsx
│   │   │   ├── Mcp.tsx
│   │   │   ├── BookDeconstruction.tsx
│   │   │   ├── Tasks.tsx
│   │   │   ├── Foreshadowing.tsx
│   │   │   ├── NarrativeAnalysis.tsx
│   │   │   ├── StorySystem.tsx
│   │   │   ├── UsageStats.tsx
│   │   │   ├── WritingStats.tsx
│   │   │   └── Settings.tsx
│   │   │
│   │   ├── components/             # 共享组件
│   │   │   ├── Sidebar.tsx
│   │   │   ├── StoryTimeline.tsx
│   │   │   ├── SceneEditor.tsx
│   │   │   ├── DataLoader.tsx
│   │   │   ├── ErrorBoundary.tsx
│   │   │   └── ...
│   │   │
│   │   ├── services/               # IPC API 层 (v0.9.0 拆分)
│   │   │   ├── tauri.ts           # 兼容入口：barrel re-export
│   │   │   └── api/
│   │   │       ├── index.ts       # barrel export
│   │   │       ├── core.ts        # loggedInvoke
│   │   │       ├── stories.ts
│   │   │       ├── storySystem.ts
│   │   │       ├── skills.ts
│   │   │       ├── settings.ts
│   │   │       ├── intent.ts
│   │   │       ├── annotations.ts
│   │   │       ├── knowledge.ts
│   │   │       ├── memory.ts
│   │   │       ├── pipeline.ts
│   │   │       ├── quality.ts
│   │   │       ├── genesis.ts
│   │   │       ├── stream.ts
│   │   │       ├── subscription.ts
│   │   │       ├── writing.ts
│   │   │       └── wizard.ts
│   │   │
│   │   ├── hooks/                  # 自定义 Hooks
│   │   │   ├── useSyncStore.ts    # 统一 sync-event 监听
│   │   │   ├── useScenes.ts
│   │   │   ├── useWorldBuilding.ts
│   │   │   ├── useWorkflowNodes.ts
│   │   │   ├── useUpdater.ts
│   │   │   └── ...
│   │   │
│   │   ├── stores/                 # Zustand 全局状态
│   │   │   └── appStore.ts
│   │   │
│   │   ├── generated/              # ts-rs 自动生成类型
│   │   │   └── SyncEvent.ts
│   │   │
│   │   ├── types/                  # 前端类型定义
│   │   │   └── index.ts
│   │   │
│   │   └── utils/                  # 工具函数
│   │       └── logger.ts
│   │
│   ├── index.html
│   ├── frontstage.html
│   └── package.json
│
├── src-tauri/                       # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs                 # 可执行入口
│   │   ├── lib.rs                  # crate 根：模块声明 + 全局单例 + run()
│   │   ├── handlers.rs             # generate_handler![] 宏命令注册表
│   │   │
│   │   ├── commands/               # Tauri Command 层（按领域拆分，v0.7.9+）
│   │   │   ├── mod.rs
│   │   │   ├── utils.rs           # EmitSync trait
│   │   │   ├── core.rs
│   │   │   ├── story.rs
│   │   │   ├── chapter.rs
│   │   │   ├── character.rs
│   │   │   ├── story_system.rs
│   │   │   ├── memory.rs
│   │   │   ├── pipeline.rs
│   │   │   ├── skill.rs
│   │   │   ├── mcp.rs
│   │   │   ├── intent.rs
│   │   │   ├── export.rs
│   │   │   ├── anti_ai.rs
│   │   │   ├── audit.rs
│   │   │   ├── reading_power.rs
│   │   │   ├── vector.rs
│   │   │   ├── workflow.rs
│   │   │   ├── sync.rs
│   │   │   ├── ai_op.rs
│   │   │   └── genre.rs
│   │   │
│   │   ├── scene_commands.rs       # 场景命令（v0.7.9 拆分，顶层保留）
│   │   ├── creation_commands.rs    # 创世/创作命令（v0.7.9 拆分）
│   │   ├── revision_commands.rs    # 修订命令（v0.7.9 拆分）
│   │   └── studio_commands.rs      # 工作室命令（v0.7.9 拆分）
│   │   │
│   │   ├── db/                     # 数据层 (v0.9.0 DTO + MigrationRunner)
│   │   │   ├── mod.rs
│   │   │   ├── connection.rs      # DbPool、init_db、遗留内联迁移
│   │   │   ├── migrations.rs      # 自定义 MigrationRunner
│   │   │   ├── migrations/        # V007 ~ V027 .sql 文件
│   │   │   ├── models.rs          # 数据库实体
│   │   │   ├── dto.rs             # 🆕 请求/响应 DTO (v0.9.0)
│   │   │   ├── repositories.rs
│   │   │   ├── repositories_story_system.rs
│   │   │   ├── repositories_narrative.rs
│   │   │   ├── repositories_pipeline.rs
│   │   │   ├── repositories_export.rs
│   │   │   ├── traits.rs
│   │   │   ├── repositories_tests.rs
│   │   │   └── cascade_tests.rs
│   │   │
│   │   ├── story_system/           # 合同驱动故事系统
│   │   │   ├── mod.rs             # StorySystemEngine / SceneCommitService
│   │   │   ├── chapter_service.rs # 🆕 章节领域服务 (v0.9.0)
│   │   │   ├── scene_service.rs   # 🆕 场景领域服务 (v0.9.0)
│   │   │   ├── auto_contract.rs
│   │   │   ├── contract_builder.rs
│   │   │   ├── preflight.rs
│   │   │   └── projection_writers.rs
│   │   │
│   │   ├── state_sync/             # 前后端状态同步
│   │   │   ├── mod.rs
│   │   │   ├── events.rs          # SyncEvent (ts-rs)
│   │   │   └── service.rs
│   │   │
│   │   ├── agency/                 # 多代理创作框架（创世 2.0：黑板/工具循环/三角色/协调器）
│   │   ├── agents/                 # Agent 系统
│   │   ├── memory/                 # 四层记忆系统
│   │   ├── pipeline/               # Pipeline 审校
│   │   ├── book_deconstruction/    # 拆书
│   │   ├── task_system/            # 任务调度
│   │   ├── creative_engine/        # 创意引擎 / 风格 / 连续性
│   │   ├── narrative/              # 叙事元素与管线
│   │   ├── llm/                    # LLM 适配器
│   │   ├── vector/                 # LanceDB 向量存储
│   │   ├── embeddings/             # Embedding 服务
│   │   ├── knowledge_base/         # 知识库
│   │   ├── skills/                 # 技能系统
│   │   ├── mcp/                    # MCP 工具
│   │   ├── automation/             # 自动化事件
│   │   ├── workflow/               # 工作流引擎
│   │   ├── planner/                # 计划生成与执行
│   │   ├── config/                 # 配置管理
│   │   ├── export/                 # 导出
│   │   ├── updater/                # 自动更新
│   │   ├── telemetry/              # 使用统计
│   │   ├── versions/               # 版本管理
│   │   ├── anti_ai/                # Anti-AI 审查
│   │   ├── reading_power/          # 追读力评估
│   │   ├── canonical_state/        # 规范状态
│   │   ├── utils/                  # 工具
│   │   └── tests/                  # 集成测试
│   │
│   └── Cargo.toml
│
├── e2e/                             # Playwright E2E 测试
├── docs/                            # 文档
│   ├── USER_GUIDE.md
│   ├── product-screenshots/
│   └── ...
├── scripts/                         # 工具脚本
└── README.md
```

---

## 数据流

### 场景创建流程 (v0.9.0 分层)

```
用户点击"新建场景" → StoryTimeline
→ invoke('create_scene', { story_id, sequence_number, title })
→ commands/scene_commands.rs 参数校验
→ db::CreateSceneRequest (dto.rs) 反序列化
→ SceneRepository::create()
→ SQLite
→ StateSync::emit_scene_created() 发射 sync-event
→ 前端 useSyncStore 失效 ['scenes'] 查询
→ StoryTimeline 自动刷新列表
```

### AI 场景生成流程

```
用户请求生成 → SceneGeneratorAgent
→ QueryPipeline::query() 获取上下文
→ LLM Adapter 生成场景建议
→ 返回 3 个 SceneProposal
→ 用户选择 → SceneRepository::create()
```

### 记忆 Ingest 流程

```
场景保存 → IngestPipeline::ingest()
→ Step 1: analyze_content() 提取实体关系
→ Step 2: generate_knowledge() 生成知识
→ KnowledgeGraph::save_entities()
→ KnowledgeGraph::save_relations()
→ VectorStore::store()
→ asset_bridge (v0.37.0) → 提取结果源感知 upsert 进生产资产表
  (characters / character_relationships / world_buildings /
   scenes.outline_content / story_outlines，新角色自动注册，
   手工编辑 user_created/manual 永不覆盖)
→ ingest_jobs 表更新状态 (completed/failed)
→ 发射 ingest-job-updated 事件
→ 幕前顶栏 🧠 图标更新
```

### SCENE_COMMIT 投影流程 (v6.0.0 → v0.9.0)

```
场景保存
→ scene_service.rs / chapter_service.rs 领域编排
  → SceneCommitService::auto_commit()
    → StateProjectionWriter     → memory_items (category="state")
    → IndexProjectionWriter     → memory_items (category="entity")
    → SummaryProjectionWriter   → story_summaries
    → MemoryProjectionWriter    → memory_items (category="event")
    → VectorProjectionWriter    → LanceDB VectorRecord
  → projection_status_json 记录各 Writer 状态
→ 发射 sync-event: DataRefresh + IngestionCompleted
→ v0.51.0 正文变更：`lookup_post_commit_work` → Observe（三角色）/ Ingest / Skip
```

> **v0.7.3 变更**：`ChapterCommitService` 重命名为 `SceneCommitService`，`chapter_commits` 表重命名为 `scene_commits`（Migration 70），提交粒度从 Chapter 对齐到 Scene。  
> **v0.9.0 变更**：`SceneCommitService::auto_commit` 的调用从 Command 层下沉到 `story_system/chapter_service.rs` 的 `ChapterCommitDebouncer`，触发逻辑与 HTTP/IPC 层解耦。

---

## 前端 IPC 与状态同步 (v0.9.0)

### API 服务层 (`src-frontend/src/services/api/`)

- v0.9.0 将原 `services/tauri.ts`（1,340 行上帝文件）拆分为 17 个按域子模块
- `core.ts` 仅保留 `loggedInvoke<T>`：参数脱敏 + 耗时日志 + 统一错误抛出
- 历史导入 `import { ... } from '@/services/tauri'` 仍通过 3 行 barrel 兼容
- 新增 `services/api/index.ts` barrel export，未来新模块推荐 `import { ... } from '@/services/api'`

### TanStack Query + Zustand 协作

- `stores/appStore.ts`（Zustand）：保存 `currentStory`、`stories[]`、UI 状态
- `hooks/useSyncStore.ts`：监听 Rust 发射的 `sync-event`，根据事件类型精确失效 TanStack Query 缓存
- `App.tsx`：`currentStory` 变化时批量 `cancelQueries` + `invalidateQueries`，刷新关联数据

### 前后台通信

- Rust `state_sync/events.rs` 定义 `SyncEvent`（`#[derive(TS)]` 自动生成到 `src-frontend/src/generated/SyncEvent.ts`）
- 16+ 种事件覆盖：StoryCreated / StoryDeleted / CharacterUpdated / SceneCreated / ChapterUpdated / WorldBuildingUpdated / StyleDnaUpdated / TaskUpdated / AnnotationCreated / PayoffLedgerUpdated / DataRefresh 等
- 幕前/幕后通过 Tauri 事件（`backstage-update`、`backstage-shown`）联动，逐步替代旧的 DOM CustomEvent

---

## 性能优化

### 前端

- **懒加载**: 幕前/幕后代码分割
- **虚拟列表**: 故事线长列表优化
- **防抖**: 自动保存 2 秒延迟
- **增量更新**: 精确触发重新渲染

### 后端

- **连接池**: r2d2 SQLite 连接复用
- **异步**: Tokio 运行时处理 I/O
- **缓存**: 向量索引内存缓存
- **批量处理**: Ingest 批量写入

---

## 安全考虑

1. **API Key**: 本地存储，界面显示为 `***`
2. **文件访问**: Tauri 能力限制
3. **SQL 注入**: 参数化查询
4. **XSS**: TipTap 内容转义
5. **CORS**: 仅允许本地请求

---

## 开发指南

### 启动开发服务器

```bash
# 前端开发服务器（推荐单独启动）
cd src-frontend && npm run dev
# 默认监听 http://127.0.0.1:5173/

# Tauri 开发模式（会自动启动前端并打开桌面窗口）
cd src-tauri && cargo tauri dev

# 生产构建
cd src-tauri && cargo tauri build
```

### 测试

```bash
# Rust 单元测试
cd src-tauri && cargo test

# 前端类型检查
cd src-frontend && npx tsc --noEmit

# Playwright E2E
npm test
```

### 数据库迁移

- 启动时 `init_db()` 自动调用 `MigrationRunner::run_with_legacy()`
- SQL 迁移文件位置：`src-tauri/src/db/migrations/V###__*.sql`
- 开发环境如需重置，可删除应用数据目录下的 SQLite 文件后重新启动

---

## 相关文档

- [V3 架构计划](docs/plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计文档
- [功能清单](docs/FEATURES.md) - 完整功能列表
- [更新日志](CHANGELOG.md) - 版本变更记录
