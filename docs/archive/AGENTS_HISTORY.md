*归档于 2026-10-06（v0.59.3）：v0.56.2 摘要。*

### v0.64.8 - 称号幻影行随死者一并排除（v0.64.7 收尾）

**真机**：KG 里除 `明成公主` 外还有只有称号的角色行 `公主`、`镇北王`（无 `characters` 行、不在生死列），只按行名排除时它们会以「活人」进 cast——症状与死人复活一样。**修复**：`life_status::dead_names` 按 v0.64.6 的解析策略（`resolve_character_id`：精确名 → 别称表 → 唯一同人形态命中）把归到死者名下的称呼一并算已死，解析不出来不猜。

- **验证**：`cargo test --lib` 1731 passed / 4 ignored（+14）；vitest 607 / 3 skipped；clippy 0 error；fmt / prettier / guard / tsc 全绿。真机探针 dead 名单：`[公主, 明成公主, 苏会山, 镇北王]`。
- **契约**：`dead_names_expand_to_registered_aliases`。
- **未关闭**：同 v0.64.7（历史段落里活着的明成公主仍在正文；state/index 投影 writer schema 不匹配）。

### v0.64.7 - 死人不得复活（角色生死状态持久化）

**真机**：《帝国的烟火》第 2 章明成公主被一拳打死（「七窍喷血……登时气绝」「明成公主的尸体躺在原处」），自动续写到第 10、11 章又让她走路、说话、夺印、抓人手腕——同一场景里她的尸体还停在门板上。**根因是死亡从来没落库**：生死唯一来源是 `dead_names_in_text` 对**当前章末 1500 字**（`PRIOR_CAST_CHAR_CAP`）的扫描，第 2 章的死亡到第 9 章早已滑出窗口；抽取的死亡信号只落在 `kg_entities.attributes.status`（不进任何注入路径），`characters` 无生死列，她的 `character_states.physical_state` 为空。**修复**：V142 加 `characters.life_status` / `death_chapter` 并回填存量（章序扫描正文 + KG `status=Dead` 兜底）；文本判定下沉 `utils::death_text`、列读写与回填在 `db::character_life`、策略在 `story_system::life_status`（架构守卫「db 不得依赖 story_system」继续通过）；`auto_commit` 与 `update_scene` 按整章正文即时落库；`beat_card` dead 名单与「下一拍」并入持久化已死，`WriteTimeBundle` 给已死角色卡注入「已死（第 N 章），不得作为活人行动」，导演锁渲染「已死」+ 禁重演；单调写回（post_process 不得改回活人）+ 人物页「已死」徽标与一键改回存活（假死情节，命令 `set_character_life_status`）。

- **验证**：`cargo test --lib` 1730 passed / 4 ignored（+13）；vitest 607 / 3 skipped（+3）；clippy 0 error；fmt / prettier / guard / tsc 全绿。**真机验收探针**（对真实库副本，未触碰原库）：V142 把 `苏会山`、`明成公主` 标为第 2 章已死；用真机第 10 章正文编译节拍卡，她不在 cast 且在 dead 名单；写作包角色卡为「已死（第2章），不得作为活人行动」。
- **契约**：`refresh_marks_written_death_and_stays_monotone`；`refresh_skips_negated_death_sentences`；`dead_marker_annotates_state_and_revive_strips_it`；`mark_dead_is_monotone_and_keeps_first_chapter`；`annotate_and_strip_roundtrip`；`backfill_scans_chapters_in_order_and_uses_kg_signal`；`revive_clears_marker_and_allows_second_death`；`v142_backfills_dead_character_with_death_chapter`；`v142_marks_physical_state_for_character_card`；`v142_is_idempotent`；`v142_keeps_distinct_people_alive`；`persisted_dead_beats_local_window_for_cast_and_next_node`；`load_sync_marks_persisted_dead_character_in_card`；`real_machine_probe_resurrect_is_blocked`（ignored，手动跑）。
- **未关闭**：已写的第 10、11 章正文里「活着的明成公主」仍在正文里（升级后续写会把她当已死，历史段落需作者重写或删除）；本机库标记依赖升级后 V142 迁移自动完成；`state` / `index` 投影 writer 长期 schema 不匹配报 error（本次未动）。

### v0.64.6 - 人物称呼归一（称呼不再长出幻影人物）

**真机**：《帝国的烟火》里 `景亲王` 与 `景亲王曹元寿`、`苏世子` 与 `苏亦铁`、`奉乾帝` 与 `奉乾皇帝` 各占一行。**根因是数据层缺「称呼 → 人物」**：抽取 prompt 要求 `name` 取文本中出现的名字，`asset_bridge` 又按名精确匹配建行——中文小说「称人不说名」，每个新称呼都长一行；`characters` 无别称层，`same_person` 只认「称号+本名」不认「称号+名」与纯称号，称号词表仅 11 个硬编码词。**修复**：V140 别称表 + 建行前解析（精确名→别称→称号形态）+ 命中即合并（改线状态/关系/场景关联/行为/知情/持有，关系与场景关联去重，改写正文名字 token，补齐保留行空字段）+ 抽取 prompt 新增 `aliases` 与「称呼归并」段 + 称号词表扩充与「称号在前」形态 + V141 启动迁移。**本机数据已修**（苏世子→苏亦铁、奉乾皇帝→奉乾帝、景亲王→景亲王曹元寿，别称已登记，备份 `cinema_ai.db.bak-v0.64.6-*`）。

- **验证**：`cargo test --lib` 1717 passed / 3 ignored（+8）；vitest 604 / 3 skipped；clippy 0 error；fmt / prettier / guard / tsc 全绿。
- **契约**：`resolve_by_title_form`；`resolve_by_alias_and_absorb_phantom`；`resolve_returns_none_for_unknown`；`merge_dedups_scene_links_and_relations`；`v141_merges_title_form_rows`；`v141_keeps_distinct_people`；`same_person_title_first_form`；`test_sync_character_appellation_merges_into_person`。
- **未关闭**：字/号/官职靠 LLM 的 aliases 归并（抽取质量决定归并质量）；同名候选多于一个时不猜（宁可新建）。

### v0.64.5 - 同章重复续写不落 commit（一章一条的 UPSERT）

**真机事故**：《帝国的烟火》第 2 章首次提交后，每次「续写下一段」保存都报 `[SceneCommit] auto_commit failed … UNIQUE constraint failed: scene_commits.story_id, scene_commits.chapter_number`。**根因**：`scene_commits` 有 `UNIQUE(story_id, chapter_number)`（一章一条），而 `SceneCommitService::init_commit` 一律 INSERT → 自动提交第一步就失败，那一轮的 review / 章节摘要 / 合同履行度 / KG 提取 / 状态增量 / 全部投影 writer 都不跑（记忆金字塔停在首次提交）。**修复**：新增 `SceneCommitRepository::{get_by_story_chapter, upsert_pending}`——同章复用既有行（保留 id，投影幂等重跑）、重置 pending、刷新 scene/chapter 挂载、清空派生字段（避免重新提交期间旧摘要被当当前事实）；`init_commit` 与前端可调命令一并幂等。

- **验证**：`cargo test --lib` 1709 passed / 3 ignored（+4：3 项单元 + 1 项端到端探针，后者随 master 提交、下次打包生效）；vitest 604 / 3 skipped；clippy 0 error；fmt / prettier / guard / tsc 全绿。
- **契约**：`init_commit_reuses_row_for_same_chapter`；`recommit_resets_status_and_clears_derived_fields`；`recommit_refreshes_scene_mount_when_provided`。
- **未关闭**：本机第 2 章 commit 需新版本安装后下一次保存/续写才重算；真机端到端未复跑。

### v0.64.4 - 模型失败可见性 + 探测超时韧性（含 v0.64.3 全部内容）

**真机事故**：加正文后续写崩在 `write_beat_once 过短（0 字符），续写回退仍失败`。**环境**：四个端点里两个自建（`127.0.0.1:11500`、`10.62.239.13:17092`）不可达、远程 deepseek 401，只剩 `10.62.239.13:17098` 健康，而它在 5s 起跑探测里超时（单槽服务忙）被跳过 → 全候选失效。**两处产品缺陷**：①`write_beat_once` 把模型失败吞成空文本再由「过短」报错，真实原因（不可达/超时/鉴权）丢失且白跑一次回退；②唯一健康端点作为**最后一个候选**被探测超时跳过 = 整轮必失败。**修复**：模型失败立即带原因失败（`续写模型调用失败（未产出正文）：…`）；探测超时若已是最后一个候选则真打一次（`should_attempt_after_probe_timeout`，后面还有候选时保持快速回退）；`StreamOutput` 渲染前合并悬挂闭合引号。本版同时包含 v0.64.3（下引号孤行根除 + V139 迁移），v0.64.3 未上传安装包。

- **验证**：`cargo test --lib` 1705 passed / 3 ignored（+1）；vitest 604 / 3 skipped；clippy 0 error；fmt / prettier / guard / tsc 全绿。
- **契约**：`probe_timeout_still_attempts_last_candidate`。
- **未关闭**：模型端点需用户自行恢复（两个自建不可达 + 远程 key 401）；真机端到端未复跑。

### v0.64.2 - 续写人物关系错乱修复（真机事故）+ 关系不变量守卫

**事故**：《帝国的烟火》续写第二章台词角色错位（「景亲王江顾然」两人并成一体；景亲王对大执事说「苏爱卿，你儿子娶的是公主」）。根因两条、都在 `agency`：①`continue_director` 的配偶启发式用整串 `kin.contains("配偶")` 判定，一处「甲与乙一并坐下」把该角色与**全部在场者**写成夫妻（真机 15+12 行）；②`coordinator::persist_inferred_relationships` 的 `should = dirty || 旧类型 != 推导类型` 把「类型不同」也当覆盖理由，父子/手足/兄妹/同僚被逐条改写成夫妻。同一份 `lock.relations` 既进提示词（【本拍人物关系】+ 硬规则）又落库 → 模型被明确告知「父子是夫妻」。**修复**：逐段匹配对方姓名 + 配偶信号、去掉 `（配偶向）` 标记泄漏、落库只允许修脏/填空；新增不变量 `sanitize_relations` / `sanitize_bundle_relations`（血亲对上的夫妻行、单人 ≥3 配偶整批拦下，别名先归一），覆盖【本拍人物关系】与【角色情感关系】两条注入路径，拦下的行入质量债 `continue_relations`；探针遗留缺口入质量债 `continue_probe`（此前只 log）。**数据修复**：《帝国的烟火》32 行误写夫妻 → 回填 6（父子/兄弟/兄妹/同僚/母子）、删除 26，清除幻影人物「景亲王江顾然」（备份 `cinema_ai.db.bak-v0.64.2-*`）。

- **验证**：`cargo test --lib` 1698 passed / 3 ignored（+7：推导不扩散 / 不变量 4 / 关系表路径去污 / 覆盖保护）；vitest 594 / 3 skipped（无前端改动）；clippy 0 error；fmt / prettier / guard / tsc 全绿。
- **契约**：`spouse_inference_stays_on_the_named_pair`；`sanitize_drops_spouse_on_kin_pair`；`sanitize_drops_person_with_three_spouses`；`sanitize_keeps_two_spouses_and_non_kin_lines`；`bundle_relations_drop_poisoned_spouse_rows`；`bundle_relations_canonicalize_alias_pairs`；`persist_does_not_overwrite_established_type_with_inferred`。
- **未关闭**：运行中的 0.64.1 需升级才生效（否则再续写会重新污染）；第二章正文错乱台词仍在正文里；`景亲王/景亲王曹元寿`、`奉乾帝/奉乾皇帝` 同名重复行未合并；真机端到端未复跑（**不得宣称续写质量已修复**）。

### v0.64.1 - 发布链路恢复：0.64.0 上线 + 兜底版本 + 用例加固

**发布确认**：Apple 协议签署生效 → v0.64.0 完整发布（latest.json=0.64.0；dmg/msi/AppImage/**deb** 全 200）。**landing** `FALLBACK_VERSION` 0.58.0→0.64.0（最近一次确认在线版本；保留策略 5 版内有效）。**加固**：`FrontstageApp.split-auto-switch` 分章切换用例三处等待显式 5s（默认 1000ms 在 CI 高负载下超时，曾致 v0.64.0 首轮构建失败；纯测试时序）。**流程修正**：v0.60.0–v0.63.0 此前只推到 Cursor 代理，GitHub master 停在 v0.59.4、CI 未运行——已补推，后续每版双 remote 推送。

- **验证**：Rust 1691 passed / 3 ignored（无改动）；vitest 594 / 3 skipped；landing 24；clippy 0 error；fmt/prettier/guard 全绿。
- **未关闭**：真机端到端未复跑（**不得宣称续写质量已修复**）。

### v0.64.0 - 运行维护页 + 声明式投影路由表（P3-F 落地与 CI 修复）

**P3-F 声明式投影路由表**（`story_system::projection_writers`）：`CommitArtifact` × `ProjectionWriterKind` 纯数据表 `PROJECTION_ROUTES`；`get_projection_writers` 与 `projection_status_keys()` 均从表派生（漏接线告警、新增 writer 自动进状态键），每次 commit 记录路由摘要便于审计；契约测试 6 项（产物全覆盖/顺序与 name 一致/无孤儿/状态键含异步/空值判定/摘要 on-off）。**幕后「运行维护」页**：质量债、待确认、文风偏好、成本四 Tab（此前四处的后端命令均无界面）；新增 `list_style_preferences` / `set_style_preference_status` 命令与 4 项 vitest。**修复**：`llm::cost` 的 `clippy::redundant_comparisons`（deny 级）阻塞了 v0.63.0 的 CI 发布链路——修复并把 `cargo clippy` 纳入每版验证。

- **验证**：`cargo test --lib` 1691 passed / 3 ignored（+6）；`npx vitest run` 594 passed / 3 skipped（+4）；`cargo clippy` 0 error；tsc / nightly fmt / prettier / guard 全绿。
- **契约**：`routing_table_covers_every_artifact`；`sync_kinds_are_constructible_and_match_writer_names`；`status_keys_include_deferred_writers`；`artifact_has_content_treats_empty_payload_as_absent`；`route_summary_reports_on_off_per_artifact`；`Maintenance` 4 用例。
- **未关闭**：真机端到端未复跑（**不得宣称续写质量已修复**）；v0.63.0 tag 保留（CI 已失败，不回改）；网站发布待本版 tag 跑通。

### v0.63.0 - 工程纪律：质量债 / 时间旅行 / 终局指南针 / 待确认队列（P0–P3 收官）

P3 阶段，也是 P0–P3 四阶段路线图的收官版本。**V138** 三张表：`quality_debts`（质检降级不再静默：RevisionRequired / salvage / 异常降级入账，含建议回收窗口，幂等 upsert）、`story_checkpoints`（每 10 章随段摘要写连续性快照：角色状态/物品归属/未回收伏笔/段摘要/全书纲要）、`pending_reviews`（规则类新增物待确认队列）。**时间旅行**：`story_system::checkpoint::query_as_of` 基于 append-only 知情流水与 `reveal_chapter` 给出「截至第 N 章」的角色已知与真相揭示状态（物品账本非追加式，结果标注 current_only）。**终局指南针**：`story_system::compass` 确定性派生（核心冲突行 / 活跃伏笔 / 进度）并注入续写，零额外 LLM。**待确认**：ingest 中 importance ≥7 的世界规则进队列。**三把尺子组合契约**：`tests::three_rulers_contract_test` 在同一故事内组合验证玉佩/知识边界/级联三条主线。5 个新命令：`list_quality_debts` / `resolve_quality_debt` / `query_story_as_of` / `list_pending_reviews` / `resolve_pending_review`。

- **验证**：`cargo test --lib` 1685 passed / 3 ignored（+14）；前端无改动（vitest 590 / 3 skipped）；nightly fmt / guard 全绿。
- **契约**：`record_is_idempotent_and_lists_open_debts`；`as_of_filters_knowledge_and_reveals_by_chapter`；`checkpoint_upsert_and_lookup_by_chapter`；`compass_derives_direction_threads_and_scale`；`note_is_idempotent_and_resolve_removes_from_pending`；`three_rulers_hold_on_one_story`。
- **未关闭**：P2-B/P2-D/P3-A/P3-B 的后端与命令已就绪但**无 UI**；P3-F 声明式投影路由表未实施；真机三把尺子端到端复跑未做（**不得宣称续写质量已修复**）；网站发布待 Apple 公证解阻。

### v0.62.0 - 文本质量与成本：prose_lint / 文风逆向学习 / 伏笔增强 / 成本账本

P2 阶段。**P2-A** `story_system::prose_lint` 纯 Rust 两档规则（blocking：注入术语泄漏 + 否定排比 + 章尾预告腔；advisory：破折号密度/重复句/极短收尾/开篇时间跳跃），接入 `auto_commit`（并入 review 记录）与 editor_qc 预注入块。**P2-B** V137 `style_preferences`：`update_scene` 人类编辑 → 防抖 120s + 单故事单处理器 → LLM 提炼可执行文风规则（资产 `style_delta_extraction`，标签「后台风格提炼」）→ 续写注入【作者文风偏好】。**P2-C** V137 给 foreshadowing_tracker 增 evidence/strength/subtlety/related_foreshadow_ids；ingest 自动登记上限 5 条；注入话术按计划回收窗口分档（临近「请勿提前回收」/过期「尽快回收」）。**P2-D** `llm::cost` 按故事聚合 + 阈值提示 + 零增量计费盲区检测（连续 ≥5 次零记账告警），命令 `get_story_cost_summary`。

- **验证**：`cargo test --lib` 1671 passed / 3 ignored（+18）；前端无改动（vitest 590 / 3 skipped）；nightly fmt / guard 全绿。
- **契约**：`flags_pipeline_header_leak_as_blocking`；`clean_literary_text_produces_no_blocking`；`parse_style_delta_validates_and_dedupes`；`style_signal_gate_filters_noise`；`service_hints_annotate_planned_payoff_window`；`zero_token_streak_is_flagged_but_normal_usage_is_not`。
- **未关闭**：真机验证；P2-B/P2-D 无前端 UI（命令已就绪）；**不得宣称续写质量已修复**；网站发布待 Apple 公证解阻。

### v0.61.0 - 分层记忆金字塔：语义摘要 + 段摘要 + 全书纲要 + 自适应窗口

P1 阶段（docs/plans/2026-10-06-p0-p3-roadmap-implementation.md）。**P1-A**：`scene_commits.summary_text` 从「前 1000 字截断」升级为 LLM 语义摘要（100-150 字，资产 `chapter_summary`；`parse_summary_response` 拒绝 JSON/过短/过长，失败回退截断）。**P1-B**：V136 `story_segment_summaries`（segment/book 两级，UNIQUE(story_id,level,segment_index)）——每 10 章由逐章摘要压缩段摘要（数据不足一半跳过），段数 ≥3 生成全书纲要；commit 成功后 `spawn_refresh_after_commit` 后台补齐（后台闸门 + LLM 失败仅告警）。**P1-C**：`adaptive_summary_window`（≤15→10 / 16-50→5 / >50→3）替换写死的「近 3 章」，工作记忆注入最近段摘要 + 全书纲要；续写资产新增【故事纲要】（长篇带段摘要 + 全书纲要，禁止直接复述）。

- **验证**：`cargo test --lib` 1653 passed / 3 ignored（+10）；前端无改动（vitest 590 / 3 skipped）；nightly fmt / architecture_guard 全绿。
- **契约**：`adaptive_summary_window_shrinks_with_book_length`；`segment_math_covers_expected_ranges`；`upsert_segment_summary_is_idempotent_per_index`；`story_so_far_block_includes_segments_only_for_long_books`；`parse_rejects_json_too_short_and_too_long`。
- **未关闭**：真机 10+ 章验证摘要质量；**不得宣称续写质量已修复**；网站发布待 Apple 公证解阻。


### v0.56.2 - 下引号不再单独成段；编辑审计顶栏不再报「已完成失败」

幕前对话句号后空行 + 全角缩进会把闭合引号排成带段首缩进的孤段。根因：悬挂合并只认「换行后立刻是引号」，夹着全角空格就漏；空行分段路径还不跑 HTML 孤段合并。现跳过换行与引号之间的空白并丢掉缩进，空行/存量 HTML 都并回上一句。后台编辑审查本是 fail-open（章节已落库），却把 `Err` 标成「后台审查失败」，顶栏拼成「编辑审计已完成后台审查失败」。现 done 固定「后台审查」，失败只走 toast/日志；`friendlyText` 对失败/超时不再加「已完成」。

- **验证**：`cargo test --lib` 1572 passed / 2 ignored（+1）；`npx vitest run` 606 passed / 3 skipped（+4）。
- **契约**：`blank-line path: indented hanging closing quote`；`失败/超时不得拼成「已完成…失败」`；`editor_qc_done_detail_is_fail_open_not_failure`。
- **未关闭**：已落库旧章下次打开会并回孤引号。真机须再续写确认；**不得宣称续写质量已修复**。


---

*归档于 2026-10-06（v0.59.2）：v0.56.0 / v0.55.0 摘要；v0.56.2 见本文件顶部。*

### v0.55.0 - 资产渐进展开 / 续写冻结 / 短操作合同

第二至四期对照 grok-bot 控制面：Producer/Editor 工具目录只注入名+一行，schema 走原生 `tools[]`；新增 `asset_read` 按名取全卡。续写仍零工具，全卡只给在场/冲突，其余半卡。一次 `write_beat_once` 冻结节拍卡与阵容，返回后解冻。`CONTINUE_BEAT_SYSTEM` 改为 11 行合同 + 三条对错范例。不改三档路由、不把主创拉回 ToolLoop。

- **验证**：`cargo test --lib` 1549 passed / 2 ignored（+7）。
- **契约**：`catalog_for_role_is_name_and_one_line`；`asset_read_returns_full_card_and_refuses_unknown_name`；`continue_user_omits_asset_read_and_keeps_present_full_cards`；`pin_keeps_first_shot_when_later_cast_changes`；`continue_freeze_pin_ignores_later_db_mutation`；`continue_beat_operational_contract_has_three_examples`。
- **未关闭**：真机须再跑创世/续写；**不得宣称 ToolLoop JSON 熔断或续写质量已修复**。

### v0.56.0 - 续写导演锁 / 管理 Agent 必写人物关系

对照 `docs/plans/2026-08-27-continue-director-lock-design.md`：续写主创仍单次 `complete()`、零工具。拍前 Rust 编译人物锁（头衔+名合并、近文亲缘、本拍关系），可选 Producer `complete_json` 约 20s enrich；冻结含锁。探针去掉「丢掉已在场者 / 点名不足 2 人」，改拦同一人双身体与亲缘写反。管理 Agent 写角色后必须 upsert `character_relationships`（同对一行）；≥2 角色且关系表空则 `ensure_relationships` 补写。不自动删/合并角色表脏行。

- **验证**：`cargo test --lib` 1569 passed / 2 ignored（+20）；`npx tsc --noEmit` / `cargo +nightly fmt` / `architecture_guard.py` 全绿。
- **契约**：`same_person_title_plus_given_name`；`probe_rejects_cao_split_bodies`；`probe_rejects_nephew_when_lock_is_father`；`probe_does_not_gap_silent_present`；`pin_keeps_director_lock`；`test_materialize_relationship_after_characters_even_if_listed_first`；`test_materialize_relationship_updates_existing_pair`；`ensure_assets_upserts_missing_relationships_for_two_characters`。
- **未关闭**：真机须从「飞身扑上」同一开头再跑；**不得宣称续写质量已修复**。角色表脏行仍不自动删除。


---

*归档于 2026-10-06（v0.59.2）：v0.56.1 摘要。*

### v0.56.1 - 拆人探针认抱衣角；死人不得再用眼睛锁定

真机从「飞身扑上」续写：琬公主曹元佩抱着曹元佩的衣角（同一人两个身体），明成公主已气绝却用眼睛锁定苏亦铁。根因：拆人探针只认「名字+则/蜷缩」，抱衣角漏网；死人探针只拦再刺/再气绝，不拦活人目光。现改：同一人两个称呼独立出现即缺口；已死者点名后 80 字内出现眼睛/锁定/审视则缺口。合同 Wrong 补抱衣角。不改主创零工具。

- **验证**：`cargo test --lib` 1571 passed / 2 ignored（+2）。
- **契约**：`probe_rejects_hugging_own_clothes_as_two_people`；`probe_rejects_dead_princess_living_gaze`。
- **未关闭**：真机须从「飞身扑上」再跑；**不得宣称续写质量已修复**。探针只重试一次，仍失败会 salvage 落库。


---

*归档于 2026-10-06（v0.59.0 AGENTS.md 瘦身）：以下为 v0.30.26–v0.54.0 的逐版本摘要；v0.56.1 见本文件顶部。*

### v0.54.0 - Agency ToolLoop 原生 function calling

创世/管理/编辑的 ToolLoop 把角色白名单工具以 JSON Schema 发给模型；优先执行原生 `tool_calls`，没有则回退现网文本 JSON action。续写 `write_beat_once` 仍是单次 `complete()`，请求不带 tools。不改三档路由、不把主创拉回 ToolLoop。

- **验证**：`cargo test --lib` 1542 passed / 2 ignored（+16）。
- **契约**：`native_tool_calls_preferred_over_text_json`；`text_json_action_still_parses_when_tool_calls_empty`；`generate_request_omits_tools_field_when_none`；`tool_specs_for_role_producer_is_json_schema`；`continue_beat_complete_does_not_require_tools`；既有 tool_loop mock 只实现 `complete` 仍绿。
- **未关闭**：真机须再跑创世/资产路径看管理 Agent 是否少一轮解析失败；**不得宣称 ToolLoop JSON 熔断或续写质量已修复**。第二至四期已在 v0.55.0 落地。
### v0.53.6 - 已写完的行刺/死亡不再被续写重演

大堂行刺、苏会山与明成公主已死之后，节拍卡仍把死人当在场、下一拍仍是行刺，主创会把刺杀再写一遍。本版：死人退出行动阵容；已完成高潮不再当下一拍；末句锚点与探针拦重演。

- **验证**：`cargo test --lib` 1526 passed / 2 ignored（+7）。
- **契约**：`wedding_climax_marks_king_and_princess_dead_not_son`；`wedding_climax_dead_are_not_acting_cast_or_conflict`；`compile_next_node_skips_stab_already_written_in_prose`；`probe_rejects_replay_of_completed_stab`。
- **未关闭**：须用「飞身扑上」同一开头再跑真机；**不得宣称唱反调/续写质量已修复**。
### v0.53.5 - 大纲重写弹出确认框（确认 / 取消 / 重写）

生成后续故事大纲 / 场景大纲后，幕前弹出可编辑对话框。确认才写入幕后；取消废弃；重写再生成一轮。纸面不改。

- **验证**：`cargo test --lib` 1519 passed / 2 ignored（+2）；`npx vitest run` 602 passed / 3 skipped（+5）。
- **契约**：`execute` 确认前不写库；`confirm_persists_user_edited_outlines_as_given`；幕前确认/取消/重写。
- **未关闭**：须用原句再跑真机；不得宣称续写质量已修复。
### v0.53.4 - 「写后续的故事大纲」不再进续写正文

真机该句走 Agency Append。形状检测曾要求「正文」；现写/生成的宾语是大纲即覆盖续写兜底。

- **验证**：`cargo test --lib` 1517 passed / 2 ignored（+2）。
- **契约**：`looks_like_write_subsequent_outlines_without_prose_keyword`；`write_subsequent_outlines_is_not_append_continue`。
- **未关闭**：须用原句再跑真机；不得宣称续写质量已修复。
### v0.53.3 - 按正文重写大纲在幕前弹出落库预览

真机「根据正文内容重新写后续的故事大纲，同时生成场景大纲」已写库，幕前只闪 toast。现把大纲正文弹「已按正文重写设定」；后续指令从章末往下写。

- **验证**：`cargo test --lib` 1515 passed / 2 ignored（+3）；`npx vitest run` 597 passed / 3 skipped。
- **契约**：`persist_summary_includes_written_outline_text`；`parse_targets_subsequent_story_and_generate_scene`；`refresh_prompt_asks_for_next_beat_not_opening_recap`；幕前弹窗含【故事大纲】/【场景大纲】，纸面不变。
- **未关闭**：须用原句再跑真机看弹窗；不得宣称续写质量已修复。
### v0.53.2 - 键值散文与场景大纲也能按正文重写

真机 Gemma 返回 `story_outline:… scene_outline：…` 无 JSON。现解析中英冒号键值；仅场景大纲也可 salvage。

- **验证**：`cargo test --lib` 1512 passed / 2 ignored（+7）。
- **契约**：真机两段原文可解析；「根据正文重新生成下一章」不是本作业。
- **未关闭**：须用换说法 / 场景大纲原句再跑真机；不得宣称续写质量已修复。
### v0.53.1 - 按正文重写不再因对象 JSON / 散文丢掉

真机 Gemma 4 `agency_producer` 1194 token / 82 字。`parse_refresh_payload` 接受对象大纲与中文键；仅故事大纲时 salvage 散文。

- **验证**：`cargo test --lib` 1505 passed / 2 ignored（+6）。
- **契约**：对象 `story_outline` 经 `normalize_outline`；短垃圾仍 `parse_fail` 不写库。
- **未关闭**：须用同一句再跑真机；不得宣称续写质量已修复。
### v0.53.0 - 按正文重写生产资产（不改正文）

幕前「将故事大纲按照现有正文重新写过」只更新 `story_outlines`，纸面不变。同一路由可按指令改角色 / 世界观 / 当前场大纲。`smart_execute` 在 Agency Append 之前走 `run_asset_refresh`（Producer 一次 JSON，Tool 档，无 writer）。

- **验证**：`cargo test --lib` 1499 passed / 2 ignored（+19）；`npx vitest run` 597 passed / 3 skipped（+1）。
- **契约**：`parse_asset_refresh_targets`；`apply_asset_refresh_override` 非续写/非 prose；persist 不碰 `scenes.content`；金敏秀不进表；`user_created` 情感核保留；ingest 可精炼；场景前缀 + `【当前场大纲】`；`result_kind=asset_refresh` 不进编辑器。
- **未关闭**：真机须用同一开头输入原句，看幕后大纲变、幕前字数不变；不得宣称续写质量/唱反调已修复。
### v0.52.0 - 续写拍级分层与准入轨迹（章末近文 1500）

对照 OpenViking 只引进分层加载与轨迹，不嵌其进程。章末近文窗口 500→1500，避免刚写完的债被漏掉；大纲/配额点名的角色仍准入；在场与冲突双方用全卡，其余准入半卡；故事大纲回流只留核心冲突+最近 5 个转折。日志写出「为什么是这几人」。

- **验证**：`cargo test --lib` 1480 passed / 2 ignored（+7）。
- **契约**：`prior_tail_for_cast` 1500；`mentioned_from_continue_tasks`；`format_admission_trace`；L1 无情感内核；`cap_story_outline_content` 最多 5 个转折点。
- **未关闭**：真机须用同一开头再点续写，看日志 `continue_assets: shot=1500`；不得宣称续写质量/唱反调已修复。
### v0.51.6 - 工具档/后台档不再被创作模型挤到同一台机

幕后已把主创/管理/回流分到 Qwren127、Gemma 4、Qwen 3.8，续写仍全打到 Qwren127。根因：`generate()` 在角色置顶之后又把当前活跃模型抬回链头。

- **验证**：`cargo test --lib apply_active_front` 3 passed。
- **契约**：工具档/后台档 `apply_active_model_front` 不得盖掉已指定模型；创作档仍置顶。
- **未关闭**：真机须用同一三档再点续写，看管理/回流是否打到 Gemma / Qwen 3.8；不得宣称续写质量已修复。
### v0.51.5 - 进行中的续写不再弹前台中断卡

底栏已在「Agency 续写中」时，二次点击/自动续写撞上同一 run，不再盖住纸面。等底栏结束或点底栏取消即可。

- **验证**：vitest 弹窗对 `active_run` 渲染空；FrontstageApp 不出现「需要您先处理」。
- **契约**：`isActiveCreativeRunConflict` 的弹窗路径 return null；二次点击不 `setShowInterruptionModal`。
- **未关闭**：真机须再点续写确认不再弹卡；不得宣称续写质量已修复。
### v0.51.4 - 「已有创作任务」不再打发去设置

二次续写撞上正在进行的 Agency run 时，弹窗曾写「需要您先处理 / 前往设置」。现改为「正在续写中」：等底栏结束，或取消当前续写。二次点击不再清掉真正那次生成状态。

- **验证**：`npx vitest run` 596 passed / 3 skipped（+3）；`cargo test --lib` 1470 passed / 2 ignored。
- **契约**：`AgentInterruptionModal` 不渲染「前往设置」；`isActiveCreativeRunConflict` 认 `active_run` 字段。
- **未关闭**：真机须再点续写确认弹窗文案；不得宣称续写质量已修复。
### v0.51.3 - 续写规划污染不再把 600 秒耗尽

v0.51.2 清空节拍卡泄露后立刻重试。Qwren127 用 390s 写出大纲归纳规划，净化后再开一轮，前端 600s 取消，网关还点下一候选。本版：大纲注入前切断规划；剩余 <90s 不重试；取消即停候选链。

- **验证**：`cargo test --lib` 1470 passed / 2 ignored（+4）。
- **契约**：`condense_outline_strips_planning_dump_glued_to_turning_point`；`writer_retry_has_time_requires_ninety_seconds`；`candidate_chain_stops_on_cancellation_not_timeout`；`test_detect_and_strip_bare_cot_outline_planning_dump_returns_empty`。
- **未关闭**：真机须用同一开头再点续写；不得宣称续写质量/唱反调已修复。库内旧规划大纲靠读取时切断，不自动改表。
### v0.51.2 - 续写切断节拍卡/约束规划泄露

本地 Qwen 主创把节拍卡、状态网、约束清单写进正文（有时接在复述已有场面之后）。`detect_and_strip_bare_cot` 增加节拍卡行话全文切断；规划在文首则清空触发重试。续写 system 禁止输出任务分析。

- **验证**：`cargo test --lib` 1466 passed / 2 ignored（+3）；既有 DeepSeek CoT 用例仍绿。
- **契约**：`test_detect_and_strip_bare_cot_qwen_beat_card_dump_returns_empty`；`test_detect_and_strip_bare_cot_strips_beat_card_after_prose`；`test_sanitize_novel_output_drops_qwen_beat_card_leak`。
- **未关闭**：真机须用同一开头再点续写；不得宣称续写质量/唱反调已修复。GitNexus 索引在 StoryForge→StoryMoss 改名后需 `node .gitnexus/run.cjs analyze`。
### v0.51.1 - 幕前取消键去掉系统原生凸起

生成中底栏 X 被 macOS WKWebView 画成 Aqua 灰凸块。取消键与发射键加 `appearance-none border-0 shadow-none`；取消键透明底 + 顶栏同款陶土 hover；flush 路径 CSS 双杀 UA 按钮。

- **验证**：`npx vitest run` 取消键 / 发射键 `appearance-none` 契约。无 Rust 逻辑变更。
- **契约**：`FrontstageBottomBar` 取消生成无脉冲红块且 `appearance-none`；`AiPromptBar` 发射键去掉系统原生按钮外观。
### v0.51.0 - 手写/粘贴正文触发三角色观察

幕前写满一段并停手 30 秒后，代理工作室会出现「观察」run：管理回流资产、主创编译当前场大纲与下一拍（不改正文）、编辑审查只写审查区。与自动分章同一空闲窗口；该场多出 ≥200 字才重跑；创世/续写进行中则让路只做 Ingest。观察不用 `running`，以免挡住点续写。

- **验证**：`cargo test --lib` 1463 passed / 2 ignored（+9）；`npx vitest run` 592 passed / 3 skipped（+1）。
- **契约**：`should_observe_requires_two_hundred_char_growth`；`apply_observe_writer_writes_outline_not_prose_or_beats`；`has_blocking_creative_run_ignores_observe`；`observe_editor_label_is_silent_background`；工作室「观察中」轨迹。
- **未关闭**：真机须粘贴 ≥200 字停手 30s 核对时间线；整章替换未多 200 字不重跑；分章新章等下次保存。
### v0.50.2 - 自动分章后章节名跟随章号，续写不再把切走的正文写回

下拉出现「第7章」后面还挂「第6章」，第 3–6 章开头同一段验尸。根因：重排只改 `chapter_number`；续写客户端旧全文比截断后的 DB 长，被当成底稿写回。本版派生标题跟随新号；幕前按章号显示；Append 在 DB 为前缀且多出 ≥200 字时用 DB；V130 修存量标题。

- **验证**：`cargo test --lib` 1454 passed / 2 ignored（+4）；`npx vitest run` 591 passed / 3 skipped（+1）。
- **契约**：`generic_chapter_title_matches_arabic_and_chinese_numerals`；中间章切开旧章标题变 `第4章`；`append_base_prefers_db_when_client_is_pre_split_superset`；`v130_retitles_stale_generic_and_keeps_custom`；`displayChapterTitle` 第6章@8 → 第8章。
- **未关闭**：已重复的第 3–6 章正文不自动删。真机须用同一开头再点续写。
### v0.50.1 - 自动分章后续写不再误报「请先打开一个章节」

第 6 章已打开、文思活跃立刻续写，却弹出 `VALIDATION_FAILED`「请先打开一个章节」。根因：`SCENES_PAGE_SIZE=5`，分章切到第 6 章后分页首页没有新章 scene，`sceneId` 回落 `chapter.id`，Agency Append 在 scenes 表找不到。本版分章补拉 `get_chapter_scenes`；后端 `resolve_append_scene_id` 把 chapter id 解析成关联 scene。

- **验证**：`cargo test --lib` 1450 passed / 2 ignored（+1）；`npx vitest run` 590 passed / 3 skipped。
- **契约**：`append_chapter_id_resolves_to_linked_scene`；分章自动切换 `sceneId === 'scene-2'` 且调用 `get_chapter_scenes`。
- **未关闭**：真机须用同一开头再点续写；不得宣称唱反调已修复。角色表脏行不自动删除；ContextPrioritizer 未接 Agency。
### v0.50.0 - 续写三角色闭环（资产可见 / 当场大纲 / 审查进下一拍）

幕后工作室续写后资产栏空、管理只有 start 没有 done、编辑 revise 不进下一拍。本版：后台 spawn 落活动日志且必有 done；本拍阵容与当前场大纲投影到当前 run 资产栏（可点）；Append 写入 `【当前场大纲】`；未解决 revise 最多 2 条进入下一拍节拍卡。

- **验证**：`cargo test --lib` 1449 passed / 2 ignored（+13）；`npx vitest run` 590 passed / 3 skipped（+2）。
- **契约**：`bg_done_detail_covers_all_exits` / `persist_activity_writes_start_and_done`；`production_change_projects_to_current_run_asset_zone`；`append_writeback` 含 `【当前场大纲】`；`compile_beat_card_injects_prior_run_revise_issues`；工作室资产卡点击 `setCurrentView`。
- **未关闭**：真机须用同一开头再点续写；不得宣称唱反调已修复。角色表脏行不自动删除；ContextPrioritizer 未接 Agency。
### v0.49.1 - 卸掉幕前划词润色浮条

划词弹出的「润色 / 扩写 / 指令」条挡住打字、选区也塌不下去。v0.48.1 改成够长才出仍无用。本版从幕前编辑器卸掉，并删除 `AiSelectionActions`。改写走底部指令栏。

- **验证**：`npx vitest run` 588 passed / 3 skipped（−17）；`tsc` / `format:check` / `architecture_guard.py` 全绿。无 Rust 逻辑变更。
- **契约**：划选长句不渲染 `ai-selection-actions`，也没有润色/扩写/指令按钮。
### v0.49.0 - 续写大纲以正文为真相源

有章节正文时禁止按书名发明角色/大纲；空方法论落库场景结构规范；管理 Agent 熔断改为 salvage + 后台续跑，不挡住续写。未接地书大纲不当下一节点；探针拦截场外开篇。

- **验证**：`cargo test --lib` 1436 passed / 2 ignored（+18）；`npx vitest run` 605 passed / 3 skipped；`tsc` / `format:check` / `architecture_guard.py` / `cargo +nightly fmt` 全绿。
- **契约（设计 §11）**：`title_inventions_dropped_when_absent_from_prose` / `test_materialize_drops_names_absent_from_prose`；`ferdinand_outline_is_not_grounded` / `compile_next_node_ignores_ungrounded_book_outline`；`test_ensure_methodology_default_writes_scene_structure_when_empty`；`test_ensure_assets_with_prose_does_not_require_producer_loop`；`probe_rejects_offshot_pov_opening`。
- **未关闭**：真机须用同一开头再点续写；**不得宣称「唱反调」已修复**。角色表脏行不自动删除；ContextPrioritizer 未接 Agency。
### v0.48.1 - 划词浮条不再挡住手工写作

v0.39.0 划词 AI 操作条（润色/扩写/改写）在任意选区、拖选过程中弹出，自带输入框抢焦点，mouseup 落在浮条上后选区塌不下去。改为 ≥4 字且鼠标松开后才出现；idle 默认无输入框；Esc 收起。

- **验证**：`npx vitest run` 605 passed / 3 skipped（+4）；`tsc` / `format:check` / `architecture_guard.py` 全绿。无 Rust 逻辑变更。
### v0.48.0 - 续写按镜头在场、禁止旧快照覆盖、未确认幽灵先写入

对照 v0.47.0 真机失败（《帝国的烟火》2026-08-16 晨）：在场窗口过大、角色表补位、书大纲未落地当前席、连续续写丢幽灵并用旧快照覆盖。P0：镜头 500 字阵容、去掉补位、下一节点须点名本拍在场者、落库取更长底稿、末句禁复述已完成动作、NewScene 不罚丢人、连续续写先 `appendAiContent` 未确认幽灵。

- **验证**：`cargo test --lib` 1418 passed / 2 ignored（+5）；`npx vitest run` 601 passed / 3 skipped；`tsc` / `format:check` / `architecture_guard.py` / `cargo +nightly fmt` 全绿。
- **未关闭**：真机 8 次须在 0.48.0 上重跑；v0.47.0 真机已失败，不得宣称五症状已修复。角色表脏行不清理；ContextPrioritizer 未接 Agency。
### v0.47.0 - 续写质量闭合（债务/节点/阵容/状态网）

对照 `docs/plans/2026-08-15-continue-quality-closure-design.md` 落地 P0–P3：债务只在正文兑现时刷新；下一节点不回绕；冲突看本拍阵容；写回事实出场与别名点名；续写过短回退保留节拍卡；拍级状态网 + 一次探针重试；CI `eight_beat_append_quality_contract`。NextChapter 装配先算 SceneUpdate 再开写事务，避免 SQLite unlock_notify 死锁。

- **验证**：`cargo test --lib` 1413 passed / 2 ignored（+22）；`tsc` / `format:check` / `architecture_guard.py` / `cargo +nightly fmt` 全绿。无前端逻辑变更。
- **未关闭**：真机 8 次幕前续写未跑，不得宣称三症状已修复；ContextPrioritizer 未接 Agency；不叠更早几章全文。
### v0.46.0 - 传统色主题（纸帘印 + 幕前幕后分选）

十二套写作向传统色替换旧四套。幕前顶栏色点只改纸面；设置页两列分选幕前/幕后。旧 id 自动迁到朱红/群青/藤黄/黛紫。印色即锚色，`--ai-accent-tint` 跟随当前窗。

- **验证**：`npx vitest run` 601 passed / 3 skipped（+11）；`tsc` / `format:check` / `architecture_guard.py` 全绿。无 Rust 逻辑变更。
- **未关闭**：空资产 trim 金标；ToolLoop head 双构造；v0.42.0 §8 真机探针；全界面截图回归。
### v0.45.1 - 续写前文改为开篇+近文双窗

长章续写只带末 800 字，模型看不见本章前面的情节。改为短章全文、长章开篇 600 + 近文 1800；先剥 HTML；「末段已在场」只看近文；预算裁前文时保住章末。

- **验证**：`cargo test --lib` 1391 passed / 2 ignored（+6）；无前端逻辑变更。
- **未关闭**：仍不叠更早几章全文；空资产 trim 金标；ToolLoop head 双构造；v0.42.0 §8 真机探针。
### v0.45.0 - 提示词运行时组装（创世/续写/ToolLoop）

创世首章、续写 `write_beat_once`、ToolLoop 头部改走 `prompts/assembly.rs` 哑拼接器。幕后提示词页场景预览改报 Agency 续写/创世。`board_read`/`board_write`/`asset_query` 自带用法行。内置 agency/writer/scene_outline 残留 `{{ident}}` CI fail-closed。

- **验证**：`cargo test --lib` 1385 passed / 2 ignored（+18）；`npx vitest run` 590 passed / 3 skipped；`tsc` / `format:check` / `architecture_guard.py` 全绿。
- **未关闭**：空资产/空末句 trim 金标未锁；ToolLoop head 双构造路径；v0.42.0 §8 真机探针仍未跑；P3 producer/concept_pack 未接线。
### v0.44.1 - 幕前输入框去掉系统原生描边

v0.44.0 拆掉底栏卡片后，macOS WKWebView 仍给 `<textarea>` 画原生 inset 边，用户看见输入区一圈细线。`AiPromptBar` 加上 `appearance-none border-0 shadow-none`；flush 路径 CSS 双杀 UA 样式。契约测试锁 class，避免再只查外壳。

- **验证**：`npx vitest run` 590 passed / 3 skipped（+1）；`tsc` / `format:check` 全绿。无 Rust 逻辑变更。
- **未关闭**：全界面截图回归未做；v0.42.0 §8 真机探针仍未跑。
### v0.44.0 - 墨纸 / 机械视觉定向进化补齐

对照 `docs/plans/2026-08-14-ink-paper-mechanical-deepen-design.md` 补齐 v0.43.0 未做满的缺口：幕前输入无框、Medium 分文件、纸 chroma / 选区 22%、顶栏 press、暖金内芯同色相、Panel 高光 + 弹簧 500ms、侧栏去金框。

- **验证**：`npx vitest run` 589 passed / 3 skipped（+11）；`tsc` / `format:check` / `architecture_guard.py` 全绿。无 Rust 变更。
- **未关闭**：设计全界面人工目视清单未做成截图回归；v0.42.0 §8 真机探针仍未跑。霞鹜 v1.250 无 Medium 文件，本版用同 tag Bold 映射为 CSS 500（README 已写明）。
### v0.43.0 - 墨纸 / 机械视觉定向进化

幕前输入条 `flush`：一层纸面、陶土淡彩发射、取消去 pulse。霞鹜文楷本地 woff2。幕后色板/阴影收软、press 曲线、EmptyHint。

- **验证**：`npx vitest run` 578 passed / 3 skipped（+22）；`tsc` / `format:check` / `architecture_guard.py` 全绿。无 Rust 变更。
- **未关闭**：设计全界面人工目视清单未做成截图回归；v0.42.0 §8 真机探针仍未跑。
### v0.42.0 - 续写按拍选取创作资产

长篇续写提示词不再全表倾倒。节拍卡当准入名单（≤8 人完整卡）；其余本故事相关角色一行名单；脏名不进；大纲去重；前文只留当前章末 800 字。`to_prompt()` 不动。

- **验证**：`cargo test --lib` 1367 passed / 2 ignored（+13）；无前端逻辑变更。
- **未关闭**：规格 §8 真机探针未跑；`story_outlines` 表内无界追加；ContextPrioritizer 未接 Agency。
### v0.41.2 - 续写 600s 超时：跳过超窗候选、散文失败不再进 tool_loop

推理模型空正文后，网关按窗口跳过装不下的本地模型（如 Gemma 8k）；单章 `write_beat_once` 散文回退失败直接报错，不再 `write_chapter` tool_loop 重烧候选链至前端看门狗。

- **验证**：`cargo test --lib` 1354 passed / 2 ignored（+4）；无前端逻辑变更。
- **未关闭**：本地连接超时仍可能 60s×2；`story_outlines` 膨胀与跨故事角色串入；设计 §13 真机探针未跑。
### v0.41.1 - Agency 续写上线核验加固

对照设计核验 v0.41.0 后补齐：主创 `sanitize_novel_output` + 8% 自重复重试；改写永不选 TimeSliced/TriShot；划词不走 Append；测试环境跳过 finalize LLM 摘要；文思活跃断言 `scene_id`。

- **验证**：`cargo test --lib` 1350 passed / 2 ignored（+5）；`npx vitest run` 556 passed / 3 skipped；`tsc` / `format:check` / `architecture_guard.py` 全绿。
- **未关闭**：设计 §13 连续 8 次幕前续写真机探针（需 LLM），不得宣称四症状已修复。
### v0.41.0 - Agency 唯一续写路径 + 幕前同章追加

创世与幕前/幕后续写只走 Agency 三角色；幕前续写/文思活跃为同章追加（`PersistMode::Append`），划词改写仍走 PlanExecutor。SceneBeatCard 把资产编译成这一拍的硬任务；落库写回出场/冲突/地点；债务按拍计数。切断 TimeSliced/TriShot 续写路由。

- **PersistMode**：Append 接到当前章（需 `scene_id`，增量 ≥200 才落库，返回 `increment`）；幕后「续写一章」仍 NextChapter。LeadWriter 默认单次 `complete()`，Editor `spawn_editor_qc` 后台，装配后立即 `finish_run`。
- **资产强关联**：Bundle 情感四元组+关系；SceneBeatCard 0 LLM Rust 编译；双锚点 writer prompt；写回 `characters_present` / `character_conflicts` / `setting_location`；`BeatCounters` 在 expansion 层（避免 `creative_engine → agency`）。
- **切断旧路由**：`smart_execute` 续写 → Agency Append；`execute_writer` 遇续写/创世 Err；设置 `generation_mode` 仅管改写（auto/fast/full），UI 去掉 time_sliced/tri_shot。
- **验证**：`cargo test --lib` 1345 passed / 2 ignored（+17）；`npx vitest run` 556 passed / 3 skipped（不变）；`tsc` / `format:check` / `architecture_guard.py` 全绿。
- **已知债务**：ContextPrioritizer 未接 Agency 热路径；`characters_present` id/名字混杂旧数据。
### v0.40.0 - AI 原生组件库 P3（数据展示六件套）+ P4（项目收尾）

**beautifului AI 原生组件库收官（P3 数据展示 + P4 收尾）**：6 个数据展示组件适配为受控组件入库 `src-frontend/src/components/ui/ai/` 并替换幕后落点；P4 完成替换残留清理、视觉修正与浅色页令牌化。只引用 `--ai-*` 语义令牌（契约扩至 17 变量），不引新依赖，纯前端无后端改动。

- **P3 数据展示**：AiSearchList（PromptsPanel 搜索计数区）、AiCodeBlock（六文件七处裸 pre/JSON 块）、AiDiffTable（AgencyEval 检查点对比，metrics_json 解析补基准/对比列）、AiFilterTable（UsageStats 分组筛选 + 最近调用表；AiFilterChipsBar 可选接 Logs 级别筛选）、AiRecordsTable（PromptsPanel 分组列表 + AgencyEval 双表）、AiInsightCards（UsageStats/AgencyEval 统计卡，内嵌 MiniLineChart）；AiChat 经勘察关闭（ChatComposer 为 AiPromptBar 严格子集）。
- **P4 清理**：P1-P3 替换残留 TS 13 处 + frontstage 死 CSS 约 40 类；历史死件 8 件（AiSuggestionBubble/AiHintOverlay/HelpPanel/ZenModeExit/useLlmStream/useStudioConfig/hetiAddon/Toggle）。
- **P4 修正与令牌化**：新增 `--ai-on-accent` 令牌替换四组件 text-white 直写；`/N` 透明度修饰符失效 13 处改 color-mix；Tasks 裸 pre → AiCodeBlock；AiDiffTable testid 改 per-row key；AgencyEval/AgencyStudio/AgencyLearning 浅色页令牌化（AgencyLearning 裸表 → AiRecordsTable）。
- **验证**：`npx vitest run` 556 passed / 3 skipped（P3 564 − 死件自带测试 8）；`cargo test --lib` 1328 passed / 2 ignored（无 Rust 改动）；`tsc` / `format:check` / `architecture_guard.py` 全绿。
### v0.39.0 - AI 原生组件库 P1+P2（共 10 组件）+ 保存 UNIQUE 修复

**beautifului AI 原生组件库（P1 生成体验 + P2 代理与任务）**：10 个组件适配为受控组件入库 `src-frontend/src/components/ui/ai/`，逐点接入幕后/幕前落点；只引用 `--ai-*` 语义令牌（双窗口各自定义），不引新依赖，纯前端无后端改动。

- **P1 生成体验**：令牌桥（16 个 `--ai-*` 变量 + tailwind ai 色组/9 动画工具）；AiLoading（幕后 3 处加载指示）、AiThinking（AgencyStudio 当前执行轨迹）、AiStreamingText（幕前幽灵续写，Intl.Segmenter 中文词级分词）、AiPromptBar（幕前底部指令条 + / 命令菜单）、AiApprovalCard（创建向导四选项步骤）；删除幕前死代码 `StreamingText.tsx` + `useStreamingGeneration.ts`。
- **P2 代理与任务**：AiContextCards（PromptCoverageBar 槽位清单）、AiToolChips（Tasks/Skills 筛选条）、AiRecommendationCard（级联改写逐段确认卡）、AiTaskRows（Tasks 任务行外壳）、AiSelectionActions（幕前划词浮条，smartExecute + insertContentAt 选区替换）；frontstage.css 补 `--shadow-float`。
- **保存修复**：幕前保存 UNIQUE 失败（scenes.story_id, sequence_number）根因——自愈补建逻辑在章节已有关联 scene 时盲目 INSERT 重复行、序号被占时硬撞约束；`heal_missing_scene_in_tx` 改为重定向既有关联 scene + 序号被占取 MAX+1。
- **验证**：`cargo test --lib` 1328 passed / 2 ignored（+2）；`npx vitest run` 523 passed / 3 skipped（+68）；`tsc` / `format:check` / `architecture_guard.py` 全绿。
### v0.38.2 - 幕后深色调主题 + 代理工作室实时动态持久化

**幕后主题底座（beautifului AI 原生改造 P0）**：4 套深色调主题（暖金 warm/冷青 cool/琥珀 amber/靛紫 indigo），与幕前色调同 id、同 localStorage key（`storymoss-color-theme`）双向同步。

- **主题定义**：`src-frontend/src/styles/backstageThemes.ts`——`BACKSTAGE_THEME_VARS`（16 个 `--cinema-*`/`--status-*` 变量）+ `backstageThemes` + `applyBackstageTheme`（运行时重写 `documentElement` 同名变量，未知 id 回退 warm）；warm 与 tokens.css 现状值全量一致（零视觉回归）。
- **全局接线**：`src-frontend/src/hooks/useBackstageTheme.ts`——挂载即应用 + storage（跨窗口）/ Tauri `color-theme-changed`（同窗口）双通道；listen unlisten 竞态加 cancelled 标志。在 `App.tsx` 顶层调用一次。
- **设置页入口**：`GeneralSettings.tsx` 的 `ColorThemeSelector`（已 export）——幕前/幕后双预览色点（`theme-swatch-frontstage-/backstage-{id}`），选择即同时 `applyColorTheme` + `applyBackstageTheme`。
- **清理**：删除死代码 `frontstage/hooks/useWritingStyle.ts`（注意：`hooks/useWorldBuilding.ts` 有同名 hook，不受影响）。
- **验证**：vitest 新增 11 项，全套 455 passed / 3 skipped；tsc / prettier 全绿。

**代理工作室实时动态持久化**：v0.38.0 将 agency 事件监听提升到常驻 `App.tsx` 顶层 + 全局 `agencyActivityStore`，接线正确但用户仍看不到实时动态。根因：活动事件（`agency-agent-activity` / `agency-run-progress`）纯内存（Zustand store 无 persist），macOS 隐藏 WKWebView 窗口事件送达不可靠，事件丢失即永久丢失。本版将活动事件持久化到 DB + 前端 3s 轮询，使实时显示不再依赖 Tauri 事件到达隐藏窗口。

- **DB 持久化**：新增 `agency_activity_log` 表（V129 迁移），`emit_activity` / `emit_progress` 在 `app.emit()` 后 `tokio::task::spawn_blocking` fire-and-forget 写 DB（不阻塞创世流程，失败仅 `log::warn!` 不致命）；测试环境（`app_handle=None`）不进入此块。
- **后端命令**：新增 `agency_list_activities` Tauri 命令（`run_id` -> 按 `id ASC` 返回 `Vec<AgencyActivityLogEntry>`，limit 200）。
- **前端轮询**：`AgencyStudio.tsx` 新增 `useQuery(['agency-activities', activeRunId], listActivities, { refetchInterval: 3000 })`，DB 活动事件为主源，live store 事件补充轮询间隔内新事件（按业务键 `role|action|detail` / `phase|status|message` 去重）。
- **live 事件监听保留**：`App.tsx` 事件监听 + `agencyActivityStore` 不变，提供轮询间隔内的即时更新（双保险）。
- **验证**：`cargo test --lib` 1326 passed / 2 ignored（+1：`test_log_and_list_activities`）；`npx vitest run` 455 passed / 3 skipped（无前端测试变更）；`cargo +nightly fmt` / `tsc` / `format:check` 全绿。
### v0.38.1 - 修复续写伏笔账本多字节中文切片 panic（文思活跃模式）

用户报告文思活跃模式续写弹 Fatal：`[TimeSliced] bundle 加载任务失败: ... "end byte index 30 is not a char boundary; it is inside '指' (bytes 29..32)"`。根因：`foreshadowing_service.rs` 构造伏笔账本 title 预览用 `&content[..30]` 按字节切片，中文 content 的 byte 30 落在三字节字符「指」内部 -> Rust UTF-8 panic -> 续写 bundle 加载失败。文思活跃连续续写读伏笔账本（`load_write_time_bundle -> pending/overdue_foreshadowings`），每次必炸。

- **主修复·`foreshadowing_service.rs`**：title 截取从字节语义改字符语义（`chars().count() > 30` + `chars().take(30).collect()`）；伏笔 title 是用户预览，按 30 字符比 30 字节（10 汉字）更合理。
- **同类预防·`post_process.rs`**：两处 `&draft_content[..8000/6000]` 改 `floor_char_boundary`--保留字节预算（控制上下文长度），切点回退最近字符边界。
- **同类预防·`intent.rs`**：JSON 解析失败日志 `&content[..min(200)]` 改 `floor_char_boundary`。
- **回归测试**：`service_ledger_title_multibyte_no_panic`--用报错原文验证 `get_ledger` 不 panic。
- **验证**：`cargo test --lib` 1325 passed / 2 ignored（+1）；`cargo +nightly fmt` ✅。纯 Rust 修复。
### v0.38.0 - 代理工作室实时显示修复与三 Agent 完善

修复幕后代理工作室（AgencyStudio）未打开时创世/续写事件丢失、打开后空白等待的问题——事件监听此前挂在条件挂载的页面上，随卸载销毁。

- **实时显示修复**：agency 事件监听提升到常驻 `App.tsx` 顶层；新增全局 `src-frontend/src/stores/agencyActivityStore.ts`（activities/progress cap 200，对标 backendActivityStore 单例无 persist），页面未开不再丢实时动态，打开即见；跨故事切换时 activeRunId 按 storyId 校正。
- **三 Agent（主创/管理/编辑审计）事件信号补齐**（`agency/coordinator.rs`）：概念/资产/首章/资产补齐/装配的 start/done 全路径配对（含 legacy 与快速路径单点覆盖）；修复 legacy 概念完成信号角色标注（LeadWriter→Producer）；后台质检黑板写入实时推 `agency-board-changed`。
- **前端打磨**：幕前 DETAIL_VERB 补全（概念/装配/资产补齐/资产回流/第N章草稿/审查第N章等）；幕后时间线去重改业务键（role|action|detail|phase|status），同源重复事件不再显示两次。
- **续写熔断不丢稿（测试补齐）**：行为已由 65d90b5（v0.30.30）实现（草稿 ≥600 字符降级放行/<600 丢稿），本档补齐流程级测试。
- **验证**：`cargo test --lib` 1306 passed / 2 ignored（+5）；`npx vitest run` 421 passed / 3 skipped（+17）。
### v0.37.0 - 资产回流：后台资产 agent 对已生成正文生效

修复 IngestPipeline 从正文提取的角色/关系只写 kg 记忆层、续写 writer 只读生产资产表两不相通的问题（且提取 prompt 字段名与 schema 错配、新登场角色被丢弃、Agency 续写路径不跑提取）。

- **提取 prompt 写作级升级**（`resources/prompts/memory/memory_content_analysis.md`）：字段与 schema 严格对齐——角色画像（情感内核/触发/创伤/需求）、双向情感关系、世界观增量（规则/历史/文化）、场景大纲、故事增量（核心冲突/转折点）。
- **新增资产桥**（`src-tauri/src/memory/asset_bridge.rs`）：提取结果 upsert 进生产资产表（characters / character_relationships / world_buildings / scenes.outline_content / story_outlines），新角色自动注册；源感知合并——只精炼机器来源（ingest/agency/auto_placeholder），手工编辑（user_created/manual）永不覆盖。
- **Agency 续写接入**：每章正文落库后后台自动跑提取（`spawn_asset_ingest`，含 KG 持久化）；orchestrator/TriShot 路径经 `run_ingest` 自动生效。
- **并发安全**：per-story 进程内锁 + `BACKGROUND_LLM_SEMAPHORE` 后台串行化；失败不致命，绝不影响正文落库。
- **验证**：`cargo test --lib` 1301 passed / 2 ignored（+14）；`npx vitest run` 404 passed / 3 skipped（无前端逻辑变更）。
- **已知问题（backlog）**：story_outlines 无 source 列（机器提取追加进手写大纲、content 无界增长）；关系按有向去重（反向建第二行）；ingest tokens 不计入 AgencyBudget；agency 取消不传播给已 spawn 的 ingest。
### v0.30.48 - 创世持久化链路审计修复 + issue #13/#14/#15 批量修复

- **v0.30.46 创世正文未即时保存与资产缺失**：前端两条创世路径 `selectChapter(skipContent)` 后补 `setTimeout(flushSceneSave, 0)`（auto-accept 时 sceneId 未就绪导致 flush 被跳过）；`agency/coordinator.rs` 场景装配 create+update 合成单事务 + 空正文校验；`generate_chapter_outline` 写黑板身份 Producer→LeadWriter（修 `scenes.outline_content` 恒 None）；`orchestrator.rs` 创世成功臂回读空正文即报错；`scene_repository.rs` 空串 content 归一 None 防 COALESCE 覆盖；`scene_commands.rs` 吞错上抛；`agency/materialize.rs` 新增 foreshadowing 落库（纯文本/JSON 数组/对象三形态）+ item_type 别名归一化 + characters upsert。
- **v0.30.47 角色谱静默失败 + llm_calls 空表（issue #13/#14）**：`agents/novel_creation.rs` 角色谱/文风/首场景三路径改 `extract_and_sanitize_json` 健壮解析 + 去 unwrap + warn 日志；`llm/service.rs` `prompt[..200]` 字节切 UTF-8 panic（llm_calls 永不落库根因）改 `chars().take(200)`；创世向导三卡片加 `isGenerating` 防重入；`BookDeconstruction.tsx` 4 处 toast 改 `extractMessage`。
- **v0.30.48 向导策略加载误报 + 快速创作空输入（issue #15）**：策略推荐加载中误显「策略加载失败」改为转圈动画；快速创作简介为空时先确认"仅根据标题自由发挥"。
### v0.30.45 - 修复文思活跃模式续写提示词泄露（LLM 思维链泄露到正文）

用户报告"开启了文思活跃模式后，出现提示词泄露问题"--续写返回的不是小说正文，而是 LLM 的思维链（CoT）："这是一个小说续写任务，需要我以专业作者身份..."。四层防线全部失守导致 deepseek-v4 推理模型的 CoT 被当作正文返回。

- **根因 1·`resolve_content` 错误回退（`llm/openai.rs`）**：v0.30.25 假设推理模型可能把实际内容放在 reasoning_content，content 为空时回退。但 reasoning_content 是思维链不是正文。现移除回退，content 为空返回空 + warn。
- **根因 2·`max_tokens: 2048` 太小（`agents/orchestrator.rs`）**：推理模型 CoT 消耗 1500-2500 token，2048 留给正文预算为 0 -> content 空 -> 触发回退。三处 `Some(2048)` -> `Some(4096)`。
- **根因 3·裸 CoT 检测（`agents/orchestrator.rs` `sanitize_novel_output`）**：新增 `detect_and_strip_bare_cot` 纯函数--扫描前 2000 字符非空行，≥3 行命中 CoT 信号词（40+ 个）判定泄露，尝试提取正文起点，找不到返回空。作为 step 0e 插入。
- **根因 4·prompt 禁止输出思考过程（`resources/prompts/writer/`）**：`writer_system.md` + `orchestrator_timesliced_writer.md` 新增"不要输出思考过程/分析/规划"+"禁止以分析性语句开头"。
- **验证**：`cargo test --lib` 1091 passed / 2 ignored（+4）；`npx vitest run` 352 passed / 3 skipped；`cargo +nightly fmt` / `cargo clippy --lib`（539 零新增）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.44 - 修复文思活跃模式续写报"生成过程异常结束，未收到有效内容"

用户报告"开启了文思活跃模式后，出现了报错的诊断信息"。诊断数据显示 LLM（deepseek-v4）成功返回 2460 字符，但前端 `generatedText` 仅剩 3 字符（"正文续"），打字机动画显示 18 字符增长（12->15->18）后被中断，最终弹出"生成过程异常结束，未收到有效内容"。根因：`smartExecuteInFlightRef.current = false` 在 smartExecute resolve 后、内容处理前被提前清除--后台活动同步回调（100ms 防抖）在内容处理期间把 `isGenerating` 置 false，触发安全网 effect（`!isGenerating && smartExecuteNeedDiagnosticRef.current`）误报。`handleRequestGeneration` 的活跃模式分支还错误地走了打字机幽灵文本（3 字符/帧），而非直接 `appendAiContent` 追加到编辑器正文。

- **主修复·`handleRequestGeneration` 提前清除 flight 标志（`FrontstageApp.tsx`）**：移除 smartExecute resolve 后的 `smartExecuteInFlightRef.current = false`。改为在各退出路径统一清除：打字机完成时、displayText 空 bail、background bootstrap、genesis 首章、aborted、active mode 追加后。确保内容处理期间 `isGenerating` 不被后台活动同步干扰。
- **主修复·`handleSmartGeneration` 同类根因（`FrontstageApp.tsx`）**：移除 smartExecute resolve 后的 `smartExecuteInFlightRef.current = false`，与 `handleRequestGeneration` 同理。在各内容交付路径（aborted / isAlreadyPresent / isBootstrapCompleted&&delivered / active mode append / isFirstChapterReady / ghost text）统一清除 `smartExecuteInFlightRef` + `smartExecuteNeedDiagnosticRef`；`finally` 块在 `setIsGenerating(false)` 之后兜底清除 flight 标志防泄漏。
- **活跃模式直追（`FrontstageApp.tsx` `handleRequestGeneration`）**：在打字机之前新增活跃模式分支--`wensiModeRef.current === 'active'` 时直接 `appendAiContent(displayText, 'auto')` + 清除两标志 + `setIsGenerating(false)`，绕过打字机（与 `handleSmartGeneration` 活跃模式行为一致）。
- **回归测试（`FrontstageApp.wensi-active.test.tsx`）**：+2 测试。①活跃模式续写内容直接追加到编辑器正文，不走打字机幽灵文本；②`smartExecuteNeedDiagnosticRef` 被清除，不触发"生成过程异常结束"诊断。测试 mock 修复：RichTextEditor mock 的 `getHTML()` 此前返回 stale `props.content`，改为用 mutable ref 跟踪编辑器内部 HTML（对齐真实 TipTap `getHTML` 返回实时 DOM 行为）。
- **验证**：`npx tsc --noEmit` ✅；`npx vitest run` 352 passed / 3 skipped（+2）；`cargo +nightly fmt` / `cargo clippy --lib`（538 零新增）/ `architecture_guard` / `npm run format:check` 全绿。纯前端修复，无 Rust 变更。
### v0.30.43 - 修复续写内容丢失根因：flushSceneSave 读取滞后的 latestContentRef + onChapterUpdated 覆写未保存内容

v0.30.33/v0.30.34 的关闭前 flush + 序列化持久化仍未能完全解决续写内容丢失。深入诊断定位两个根因：①`flushSceneSave` 读取 `latestContentRef.current` 而非编辑器实际 HTML--RichTextEditor 的 `onChange` 有 200ms 防抖（`htmlDebounceRef`），`latestContentRef` 可能比编辑器实际内容滞后 200ms，关闭应用/切换章节时若读 `latestContentRef`，最后 200ms 内的输入会丢失；②`onChapterUpdated`（后台 auto_commit 触发）用 DB 旧内容 `setContent` 覆写编辑器但不更新 `latestContentRef`，若用户有尚未落库的输入（防抖窗口内），编辑器被 DB 旧内容覆写后用户再输入，旧输入从编辑器消失且 `latestContentRef` 被新输入覆盖，造成不可逆丢失。

- **主修复·flushSceneSave 直接读编辑器（`FrontstageApp.tsx`）**：`flushSceneSave` 从 `editorRef.current?.getHTML()` 读取编辑器实际 HTML，`editorRef` 不可用时回退 `latestContentRef.current`；读后回写 `latestContentRef.current = content` 保持一致。覆盖关闭前 flush（`frontstage-flush-requested` 事件）、章节切换（`selectChapter`）、AI 追加（`appendAiContent`）、修稿（`handlePipelineRefine`/`onReviseResult`）全部 flush 路径。消除 200ms HTML 防抖窗口导致的内容丢失。
- **Root Cause #2·onChapterUpdated 保护未保存内容 + 同步 latestContentRef（`FrontstageApp.tsx`）**：`onChapterUpdated` 在 `setContent(formatted)` 前新增守卫--若 `latestContentRef`（会被 flush 保存的内容）非空且与 DB 内容不同，说明用户有尚未落库的输入（200ms HTML 防抖窗口内或 2000ms 自动保存防抖未出火），此时绝不用 DB 旧内容覆写编辑器，直接 `return` 跳过；`setContent` 后补 `latestContentRef.current = formatted` 同步刷新后的内容，使后续 flush 保存 onChapterUpdated 刚加载的 DB 内容而非旧值。
- **附带·setContent('') 清空 latestContentRef（`FrontstageApp.tsx`）**：无章节时 `setContent('')` 后补 `latestContentRef.current = ''`，避免 flushSceneSave 保存已清空的旧内容。
- **验证**：`cargo test --lib` 1087 passed（无 Rust 变更）；`npx tsc --noEmit` ✅；`npx vitest run` 350 passed / 3 skipped（+1：close-flush 保存编辑器实际内容而非滞后 latestContentRef 回归测试）；`cargo +nightly fmt` / `cargo clippy --lib`（538 零新增）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.42 - 修复世界观生成失败（LLM 返回 markdown 代码块包裹的 JSON + 未转义引号 + 静默失败 + prompt 字段名不匹配）

issue #14 用户报告"世界观生成失败，请重试"，但日志显示 LLM API 调用成功返回内容（7636 字符），失败发生在下游 JSON 解析且完全无错误日志。根因三层：①模型将 JSON 包裹在 ` ```json ... ``` ` 代码块中、或在字符串值内直接换行/使用裸双引号，`serde_json::from_str` 静默失败；②`novel_creation.rs` 严格解析全量响应（含围栏）直接失败，`agency/coordinator.rs::parse_lenient` 用 `rfind('}')` 会被尾部杂散 `}` 误导且无法修复字符串内裸换行；③`novel_creation_world_options.md` prompt 要求"concepts 数组"但代码读 `parsed["world_buildings"]`，即使解析成功也找不到数组；prompt 缺少格式约束。

- **Fix 1·`parse_lenient` 复用健壮提取器（`agency/coordinator.rs`）**：`parse_lenient` 改为先调 `crate::narrative::extract_and_sanitize_json`（剥离 markdown 围栏 / 推理链、括号深度匹配跳过尾部杂散 `}`、修复字符串内未转义换行、移除 BOM / 注释 / 尾随逗号），失败再回退旧的首尾花括号截取。覆盖 agency 全部 JSON 解析路径（concept_pack / producer_depth_assets 世界观 / editor 裁决 / retrieval plan）。`extract_and_sanitize_json` 已存在于 `narrative` 且被 memory/analysis 等模块使用，`agents` 已有 `crate::narrative::strip_reasoning_blocks` 先例，无新跨层依赖。
- **Fix 2·`novel_creation.rs` 世界观选项解析健壮化**：提取 `parse_world_options_response` 纯函数（便于单测，无需 mock LlmService），先 `extract_and_sanitize_json` 剥离围栏再 `serde_json::from_str`；解析失败时 `log::warn!` 记录错误 + raw 长度 + 200 字片段（此前完全静默）；`world_buildings` 缺失时错误信息明确指出"缺少 world_buildings 数组"；元素反序列化 `unwrap` 改 `map_err` 不再 panic。
- **Fix 3·prompt 字段名修正 + 格式约束（`novel_creation_world_options.md` + `narrative_world_building_generate.md`）**：`novel_creation_world_options.md` "concepts 数组" -> `world_buildings`（与代码一致）并补全完整 schema 示例；两份 prompt 新增格式约束--禁止 markdown 代码块包裹、字符串值内引用用中文引号「」或转义 `\"`、禁止 JSON 外输出任何文字。
- **验证**：`cargo test --lib` 1087 passed / 2 ignored（+5：parse_lenient 剥围栏/修复裸换行 +2，novel_creation 解析 +3）；`npx tsc --noEmit` ✅；`npx vitest run` 349 passed / 3 skipped；`cargo +nightly fmt` / `cargo clippy --lib`（538 零新增）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.41 - 修复续写内容被假阳性去重静默丢弃（模型回显指令 + 短文本假阳性 + 内容丢失）

用户诊断报告显示续写生成时 LLM（deepseek-v4）成功返回 2511 字符，但前端仅显示 6 字符（"续写\n黑暗。"），随后报"生成过程异常结束，未收到有效内容"。根因链：①模型在生成内容开头回显用户指令"续写"（非正文）；②打字机动画首帧仅 3 字符（"续写\n"），归一化后 2 字符"续写"几乎必然出现在 9656 字已有正文中；③`isTextDuplicate` 假阳性返回 true，`setGeneratedText` 跳过赋值并 `markAccepted` 存入 2 字符指纹；④生成内容被静默丢弃。两层修复：

- **Fix 1·`isTextDuplicate` 最小长度守卫（`textCleanup.ts`）**：归一化后 < 30 字符的生成文本直接返回 false，不进行去重检查。打字机首帧（3 字符）、短回显前缀（2 字符）等短文本在长篇正文中几乎必然命中 `includes()` 造成假阳性；只有生成文本足够长（≥30 归一化字符）时才检查是否为已有内容的子串。全量内容（2511 字符）仍正常评估去重。
- **Fix 2·`stripInstructionEcho` 指令回显剥离（`textCleanup.ts` + `FrontstageApp.tsx`）**：新增 `stripInstructionEcho(generated, userInput)` --归一化比较生成文本开头与用户指令，若开头匹配则裁掉原始文本中对应前缀及紧随的分隔符（换行/冒号/逗号等），剩余内容过短（<10 字符）则保留原文防误剥。在 `handleRequestGeneration` 和 `handleSmartGeneration` 的 `sanitizeContinuationOutput` 后调用，覆盖打字机路径与 smart_execute 直接路径。
- **测试**：`isTextDuplicate.test.ts` +2 测试（短文本假阳性守卫 + 长文本真阳性）；`textCleanup.test.ts` +7 测试（`stripInstructionEcho` 7 场景）；更新 2 既有测试（前缀检测改用 ≥40 字符 + `isTextDuplicate` 用 ≥30 字符）。
- **验证**：`npx tsc --noEmit` ✅；`npx vitest run` 349 passed / 3 skipped（+13）；`npm run format:check` ✅；`architecture_guard` ✅。纯前端修复，无 Rust 变更（cargo 基线不变）。
### v0.30.40 - 修复代理工作室不显示活动记录数据（activeRunId 仅从事件捕获 + 无 list_runs 命令）

用户报告"前端后台的代理工作室，没有显示代理活动的记录数据"。根因：`AgencyStudio.tsx` 的 `activeRunId` **仅从实时事件捕获**（`agency-agent-activity` / `agency-run-progress` / `agency-board-changed` 三个 `listen`），IPC 查询 `getRun`/`listBoard` 的 `enabled: !!activeRunId`--如果用户在 run 启动后或完成后才打开代理工作室，没有事件到达，`activeRunId` 恒为 `null`，页面永远显示"暂无活动"。此外无 `agency_list_runs` 命令发现已有 run，activity/progress 事件 fire-and-forget 不持久化（时间线数据页面卸载即丢失）。

- **后端·新增 `agency_list_runs` 命令（`agency/repository.rs` + `agency/commands.rs` + `handlers.rs`）**：`AgencyRepository::list_runs_for_story(story_id, limit)` 按 `created_at DESC` 列出某 story 的全部 run（利用已有 `idx_agency_runs_story` 索引）；`agency_list_runs` Tauri 命令（limit=20）注册到 `handlers.rs`。前端可通过 IPC 发现已有 run，不依赖实时事件。
- **前端·activeRunId 水合（`AgencyStudio.tsx`）**：新增 `useQuery(['agency-runs', currentStory?.id], () => listRuns(currentStory.id))` 查询（10s 轮询）；`useEffect` 在 `runs` 数据到达且 `!activeRunId` 时取 `runs[0].id`（最新 run）水合。实时事件仍可覆盖（新 run 启动时事件到达，切到新 run）。
- **前端·历史时间线重建（`AgencyStudio.tsx`）**：时间线从仅 live 事件改为三源合并--①Live 事件（activities + progress）；②历史重建（board items 的 `created_at` + `producer` + `zone` + `key` + `summary` 生成时间线条目，如"管理 创建 资产：世界观 - 双星系统"）；③Run 生命周期（`created_at` 启动 + `updated_at` 终态）。合并后按 `(at, text)` 去重、时间倒序、截断 100 条。无需新表/迁移，从已持久化的 `agency_board_items` 重建。
- **前端·Run 选择器（`AgencyStudio.tsx`）**：标题栏右侧新增 `<select>` 下拉框，每个 option 显示 `[status] phase - premise 前30字 (时间)`，用户可切换浏览历史 run。切换时 `setActiveRunId` -> react-query 自动刷新 board + run 数据。
- **附带·clippy 冗余修复（`agency/repository.rs`）**：`list_runs_for_story` + `list_checkpoints` 的 `Ok(rows.collect::<Result<Vec<_>, _>>()?)` 改为 `rows.collect::<Result<Vec<_>, _>>()`（`Ok` + `?` 冗余，clippy `needless_question_mark`）。
- **验证**：`cargo test --lib` 1082 passed（+1：`test_list_runs_for_story`）；`cargo check` / `npx tsc --noEmit` / `npx vitest run`（339 passed / 3 skipped，+3：水合 / run 选择器 / 历史时间线）/ `cargo +nightly fmt` / `cargo clippy --lib`（538，baseline 540 零新增且 -2 修复既有）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.39 - 修复续写不按故事大纲推进剧情（TimeSliced 路径缺失 build_progression_anchor）

用户报告"续写和故事大纲仍然缺乏强关联"、"没有按照故事大纲来写剧情和推进剧情"。根因：v0.30.31 引入的 `build_progression_anchor`（确定性注入剧情推进方向锚点）**只在 TriShot 路径（`execute_trishot`）调用，从未移植到 TimeSliced 路径（`execute_time_sliced`）**。而 TimeSliced 是默认续写路径（`generation_mode = "auto"` 路由续写到 TimeSliced 而非 TriShot）。TimeSliced 的 writer 通过 `bundle.to_prompt()` 得到完整故事大纲，但缺少"已推进进度"指针（最近 3 章 `scenes.outline_content`），无法判断当前在故事大纲的哪个节点，因此无法按节点推进剧情，导致续写偏离大纲、原地踏步或仅复述设定。

- **根因·build_progression_anchor 未在 TimeSliced 调用（`agents/orchestrator.rs`）**：v0.30.31 新增 `build_progression_anchor` 函数，注入①本次创作指令（创作方向）；②故事大纲前1200字（硬约束）；③本章场景大纲前800字（硬约束）；④已推进进度（最近3章 outline_content，进度指针）；⑤世界观规则前600字（硬约束）；⑥显式调和指令（在硬约束内落实指令核心意图，推进到下一节点）。但该函数**仅在 `execute_trishot` 的 `!synthesis.is_fallback` 分支调用**（line ~1617），`execute_time_sliced`（默认续写路径，line 842-1068）从未调用。TimeSliced writer 只有 `bundle.to_prompt()`（含故事大纲）+ `build_continuation_context`（前文回顾）+ `build_ending_anchor`（末句硬锚点），**无进度指针、无显式调和指令**。
- **Fix·TimeSliced 路径注入 build_progression_anchor（`agents/orchestrator.rs` `execute_time_sliced`）**：在 prompt 模板渲染后、`ending_anchor` 注入前，插入 `build_progression_anchor(&bundle, pool.inner(), &task.context.story.story_id, chapter_number, &user_instruction)` 调用，与 TriShot 路径完全对齐。writer 现在收到完整的推进方向锚点：故事大纲硬约束 + 已推进进度指针 + 显式调和指令（"推进到故事大纲下一节点、承接已推进进度，不得原地踏步"）。注意 `story_id` 在 `spawn_blocking` 闭包中被 move，改用 `&task.context.story.story_id`。
- **验证**：`cargo test --lib` 1081 passed；`cargo check` / `npx tsc --noEmit` / `npx vitest run`（336 passed / 3 skipped）/ `cargo +nightly fmt` / `cargo clippy --lib`（539，baseline 540 零新增）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.38 - 修复续写输出被编辑器元评论污染（is_prose_request 被 serde 默认 false 导致 sanitize 跳过）

用户报告"第三次续写时出的错"--续写产出正文后紧接一段 AI 文学编辑元评论（"好的，作为一名专业的文学编辑，我将根据您提供的问题列表和总体评分，对您的文本进行深度重塑…请粘贴您的《永夜神骸》第一章内容"）。续写误路由 bug 第 6 次复发（v0.30.9-14 各堵一条路径，但分类层根因未修）。

- **根因（三层叠加）**：①分类提示词"继续写"示例省略 `is_prose`，LLM 若遵循示例返回合法 JSON 但缺该字段，`#[serde(default)]` 填 `is_prose_request=false`；②serde 默认值（false）与 LLM 失败兜底值（true）相反--partial-but-valid JSON 走 `parse_classification_json` 成功解析、`is_fallback=false` 被缓存，后续相同输入持续返回毒化 false；③`sanitize_plan_for_prose_request` 门控仅检查 `is_prose_request`，false 时跳过全部净化 -> SING 多步计划 `[writer, inspector, builtin.style_enhancer]` 未拦截 -> `final_content` = style_enhancer 元评论覆盖 writer 正文。
- **Fix 1·后置不变量（`intent.rs` `parse_classification_json`）**：成功反序列化后若 `is_continuation || is_new_novel` 但 `is_prose_request=false`，强制设 true（续写/创世本质是 prose，逻辑必然）。
- **Fix 2·提示词示例补全（`intent.rs` `build_classification_prompt`）**："继续写"示例补 `is_prose=true`，消除 LLM 省略该字段的源头。
- **Fix 3·sanitize 门控扩展（`planner/mod.rs` `sanitize_plan_for_prose_request`）**：门控从 `is_prose_request` 扩展为 `is_prose_request || is_continuation`--纵深防御，即使 Fix 1 未生效，`is_continuation=true` 也触发净化+塌缩。
- **验证**：`cargo test --lib` 1081 passed（+4 回归）；`cargo check`/`tsc`/`vitest`（336/3 skipped）/`fmt`/`clippy`（540->539 零新增）/`architecture_guard`/`format:check` 全绿。
### v0.30.37 - 修复创作生成失败时 toast 显示 "[object Object]"（issue #12）

用户反馈 issue #12：创作/生成失败时错误提示显示 `[object Object]`。根因与 issue #11（v0.30.31 修复的"获取模型列表"路径）同源：后端 `AppError` 自定义 `Serialize` 产出普通 JSON 对象 `{ code, message, severity, data? }`，Tauri v2.4 作为普通对象（非 JS `Error` 实例）投递到前端 catch 块；前端用 `String(err)` 或 `err instanceof Error ? err.message : String(err)` 转字符串，对普通对象产出 `[object Object]`，可读 `message` 被丢弃。v0.30.31 的 `extractMessage` helper 只覆盖"获取模型列表"，**创作/生成错误路径未迁移**--幕前 smart_execute、幕后快速创作/AI 向导、生成草稿/大纲、文思生成、管线修稿/审稿/定稿仍显示 `[object Object]`。

- **主修复·统一改用 `extractMessage`（10 个前端文件）**：所有创作/生成错误路径的 `String(err)` / `instanceof Error ? err.message : String(err)` / `err?.message || String(err)` 统一替换为 `extractMessage(err)`（`src/utils/errorHandler.ts`，依次尝试结构化 AppError 对象取 `.message` -> `Error.message` 内嵌 JSON 解析 -> 普通 Error `.message` -> 字符串 -> 带 `.message` 对象 -> 兜底 `'Unknown error'`）。
  - `FrontstageApp.tsx`（5 处）：smart_execute 主 catch（`structured?.message ?? extractMessage(error)`，复用已计算 `structured`）+ 第二 smart_execute catch + 修稿/审稿/定稿。
  - `SceneEditor.tsx`（2 处）：生成大纲/草稿失败。
  - `Stories.tsx`（4 处）：幕后快速创作/向导创作/风格混合保存/风格样本生成。
  - `RichTextEditor.tsx`（2 处）：文思内联建议生成/智能排版。
  - `WenSiPanel.tsx`（2 处）：自动续写/自动修改。
  - `usePipeline.ts`（6 处）：修稿/审稿/定稿/修复/合并/加载。
  - `CharacterStatePanel.tsx`（1 处）、`Skills.tsx`（7 处）、`PromptsPanel.tsx`（5 处）、`useUpdater.ts`（2 处）。
  - 不动 `main.tsx` / `ErrorBoundary.tsx`：已优先取 `.message`，`String()` 仅最后兜底，对带 `.message` 的 AppError 对象不会产出 `[object Object]`。
- **回归测试（`src/utils/__tests__/errorHandler.test.ts`，+8）**：AppError 普通对象提取 `message`（断言不等于 `[object Object]`）/ 带 `data` / `parseStructuredError` 识别 / `Error.message` 内嵌 JSON / 普通 Error / 字符串 / 带 `.message` 对象 / 兜底文案。
- **验证**：`npx tsc --noEmit` ✅；`npx vitest run` 336 passed / 3 skipped（+8）；`npm run format:check` ✅；`architecture_guard` ✅。纯前端，无 Rust 变更（cargo 基线 1077 不变）。
### v0.30.36 - 修复首次创世指令不保存到输入历史（按↑调取不到）

用户报告"输入框的历史输入内容也没有保存，按向上方位键调取不到历史输入"。根因：`handleInputSubmit` 保存输入历史时读取 `sid = currentStory?.id`，首次创世（无已有故事）时 `currentStory=null` -> `sid=undefined` -> `if (sid) saveInputHistory(...)` 跳过，创世指令从未持久化；随后 isBootstrap 分支 `setCurrentStory(null)` 触发 useEffect 清空 `inputHistory`，创世成功后 `setCurrentStory(新故事)` 再次触发 useEffect 从 localStorage 加载（空）。新故事输入历史始终为空，按↑无响应（无历史 + 无章节时 `fetchSmartHint` 也因 `!currentChapter` 提前返回）。v0.30.23 修复意图分类（创世不再被误判为续写）后，创世指令正确走 isBootstrap 路径，暴露了此前被续写误分类掩盖的首次创世不保存缺陷。

- **主修复·创世成功后补存创世指令（`FrontstageApp.tsx`）**：`handleSmartGeneration` 的 `story_created` 处理块（`setCurrentStory(targetStory)` 之后）新增同步写入--`loadInputHistory(storyId)` 读取新故事现有历史，若不含 `userInput` 则 `saveInputHistory(storyId, [userInput, ...existing].slice(0, MAX))` 持久化。关键时序：此写入在 `setCurrentStory` 触发 `useEffect[currentStory?.id]` 之前同步执行（同一同步块无 await），useEffect 随后 `loadInputHistory(storyId)` 即可读到创世指令。
- **不动 `handleRequestGeneration`**：文思活跃续写路径（`user_input: context || '续写'`），非用户创世指令，无有意义输入可存。
- **验证**：`npx tsc --noEmit` ✅；`npx vitest run` 328 passed / 3 skipped（+2：创世指令持久化 + 按↑召回）；`npm run format:check` ✅；`architecture_guard` ✅。纯前端，cargo 基线 1077 不变。
### v0.30.35 - editor 质检后台异步化：首章立即显示 + 后台质检 + toast 反馈

用户报告创世顶满 600s 超时无产出。根因：editor 质检（`review_and_assemble` 中的 `evaluate_gate`）在 Scene 装配落库**之前**同步执行，被 `tokio::time::timeout(600s)` 包裹。producer（深度资产 ~30-60s）+ writer（tool_loop ~4-5min）花约9分钟后 editor 只剩约1分钟，而 editor 的 `editor_verdict_prose_fallback` 用固定 300s timeout 发起 LLM 调用，34s 后被硬 600s 砍掉，既未完成质检也无法走 `salvage_failed_gate` 保产出，整 run 超时无任何首章返回。本版本把 editor 质检从同步硬阻塞改为后台异步 spawn：writer 完成首章 + 装配落库后立即返回前端显示首章（约5-6min 即可见），editor 在后台独立 spawn 质检（独立 300s deadline，不受 smart_execute 600s 限制），结果通过 `genesis-qc-result` 事件 + toast 通知用户。

- **后端·装配与质检分离（`coordinator.rs`）**：①新增 `assemble_only`（pub(crate)）-- 从 `review_and_assemble` 提取纯装配部分（`update_phase("assembly")` + `cleanup_prose_for_persist` 抗重复三件套 + `SceneRepository::create/update` 落库 + `emit_activity`），不含 editor 质检与修订，返回 `(BoardItem, scene_id)`。②新增 `spawn_editor_qc`--测试环境 `app_handle=None` 时 no-op；生产环境 `tokio::spawn` 后台任务，构造全新 `AgencyLlm(EditorAuditor)` / `AgencyBudget` / `BlackboardService` / `ToolRegistry`，用 `Some(Instant::now() + 300s)` 独立 deadline 调 `evaluate_gate_impl`，结果三态分支：`Passed` -> `{passed:true,salvaged:false}`；`RevisionRequired` -> `{passed:false,issues}`；`Failed` -> 先 `salvage_failed_gate`（草稿≥600字合成 pass 裁决保产出）-> 成功 `{passed:true,salvaged:true}` / 失败 `{passed:false,issues:[reason]}`；`Err` -> 降级放行 `{passed:true,salvaged:true}`。`emit_activity(EditorAuditor,"start"/"done","后台审查")` + emit `genesis-qc-result` 事件。③`genesis_fastpath` / `run_genesis_legacy_inner` Phase C 由 `review_and_assemble` 改为 `assemble_only` + `spawn_editor_qc`，返回 `revised:false, verdict:EditorVerdict::pending()`。④删除已无用的 `review_and_assemble` 方法（其 helper `build_revision_task`/`evaluate_gate` 仍被续写路径复用）。⑤`EditorVerdict` 新增 `pending()` 构造函数（verdict="pending"，comments="后台质检进行中"）。⑥新增事件常量 `EVENT_GENESIS_QC_RESULT = "genesis-qc-result"`。
- **前端·后台质检结果 toast（`FrontstageApp.tsx`）**：`setupEventListeners` 新增 `genesis-qc-result` 监听，三态反馈：质检通过（`passed && !salvaged`）-> `toast.success('编辑审计质检通过')`；降级放行（`passed && salvaged`，审计超时/失败但首章已保留）-> `toast.warning('质检降级放行（审计超时/失败，首章已保留）')`；不合格（`!passed`）-> `toast.warning('质检不合格，建议重新创世。问题：' + issues)`。后台 editor 不影响 `isGenerating`（agency 事件不进 `backendActivityStore`），用户可在质检期间继续写作；不自动重新创世，由用户手动决定。
- **producer 深度资产保持前台**：审计后发现 `producer_depth_assets` 已是单次 `complete_json` 调用（非 tool_loop，约30-60s），非瓶颈；且保障首章不脱节（v0.30.29 专门修复的"首章在无大纲/无世界观下写就脱节"问题）。主要瓶颈是 writer tool_loop（4-5min）+ editor tool_loop，移 editor 后台后用户在 writer 完成即可见首章。
- **验证**：`cargo test --lib` 1077 passed（+2：`test_editor_verdict_pending_defaults` / `test_assemble_only_persists_scene_without_qc`；移除 3 个已不适用的 genesis 同步质检测试，`test_editor_verdict_prose_fallback` 改为直接测 `evaluate_gate` 保留 prose-fallback 覆盖）；`cargo check` / `npx tsc --noEmit` / `npx vitest run`（326 passed / 3 skipped，+4：`genesis-qc-result` 注册 + passed/salvaged/failed 三态 toast）/ `cargo +nightly fmt` / `cargo clippy --lib`（539，baseline 540 零新增）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.34 - 修复续写内容丢失根因：序列化场景持久化 + 修稿 bypass 修复 + 关闭超时提升

v0.30.33 的关闭前 flush + AI 追加立即落库仍未能完全解决续写内容丢失。深入诊断定位三个收敛根因：①`flushSceneSave` 无序列化--文思活跃连续续写时多次 `void flushSceneSave()` 并发 fire-and-forget，`update_scene` 全量覆写在 `spawn_blocking` 线程池上 SQLite 写锁获取顺序非 FIFO，较早的小内容可能在较晚的大内容之后提交，静默覆写（编辑器显示正确但 DB 被回退，重启才发现）；②close-flush 3s 超时 < SQLite `busy_timeout` 5s，写锁竞争下 close-flush 的 `update_scene` 被 kill；③`handlePipelineRefine` 的 `setContent` / `onReviseResult` 的 `insertText` 绕过 `appendAiContent`，不更新 `latestContentRef`，关闭时 flush 保存旧内容。

- **主修复·序列化场景持久化（`FrontstageApp.tsx`）**：新增 `saveChainRef`（Promise 链）+ `persistSceneContent(sceneId, content, title)` -- 每次调用捕获快照后排队，`await prev` 等待前一次完成再 `update_scene`，`finally release()` 释放链。所有 `update_scene` 调用（`flushSceneSave` / `handleContentChange` 防抖 saveFn / 保护性保存）统一走此函数，保证串行提交、最后一次写总是最新内容。`flushSceneSave` 改为 `cancelAutoSave + persistSceneContent`。
- **Root Cause #2·关闭超时 3s -> 6s（`lib.rs`）**：`CloseRequested` 超时兜底线程从 `sleep(3s)` 提升到 `sleep(6s)`，超过 SQLite `busy_timeout=5s`，确保写锁竞争下 close-flush 的 `update_scene` 仍能提交。
- **Root Cause #3·修稿 bypass 修复（`FrontstageApp.tsx`）**：①`handlePipelineRefine` 的 `editorRef.setContent(refined_content)` 后补 `getHTML -> setContent(store) -> latestContentRef = html -> void flushSceneSave()`（`setContent` 抑制 `onChange` 不更新 ref，此前关闭时 flush 保存修稿前旧内容）；②`onReviseResult` 的 `editorRef.insertText(html)` 后同理补同步 + `void flushSceneSave()`。
- **保护性保存 + handleContentChange saveFn 统一（`FrontstageApp.tsx`）**：新建小说前保护性保存从手写 `cancelAutoSave + loggedInvoke` 改为 `await flushSceneSave()`（序列化）；`handleContentChange` 防抖 saveFn 从手写 `loggedInvoke('update_scene')` 改为 `await persistSceneContent(payload.sceneId, payload.content, payload.title)`（序列化）。消除所有非序列化的 `update_scene` 直调。
- **验证**：`cargo test --lib` 1078 passed；`npx tsc --noEmit` ✅；`npx vitest run` 322 passed / 3 skipped；`cargo +nightly fmt` / `cargo clippy --lib`（baseline 540 零新增）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.33 - 修复关闭应用时续写内容丢失（关闭前 flush + AI 追加立即落库 + 章节切换 flush）

用户报告"多次续写后关闭应用再重启，续写内容丢失，没有得到及时保存"。根因：幕前续写 `appendAiContent` 追加 AI 内容后仅调度 2000ms 防抖保存（`scheduleAutoSave(..., 2000)`），文思活跃连续续写时每次 `cancelAutoSave()` 重置定时器，间隔 <2s 则永不出火；关闭应用时后端 `CloseRequested` 直接 `graceful_shutdown -> std::process::exit(0)` 不给前端 flush 机会，防抖窗口内的内容随进程退出丢失。三层修复：

- **主修复·关闭前 flush 协调（`lib.rs` + `FrontstageApp.tsx`）**：后端 `CloseRequested` 由直接 `graceful_shutdown` 改为 `api.prevent_close()` + emit `frontstage-flush-requested` 事件 + 3s 超时兜底线程；前端 `useEffect` 监听该事件 -> `await flushSceneSaveRef.current()`（取消防抖 + 立即 `update_scene` 落库 `latestContentRef.current`）-> `invoke('graceful_quit')` 命令触发优雅关闭（WAL checkpoint 确保刚写入的数据落盘）。`graceful_shutdown` 加 `AtomicBool` 幂等守卫防 flush 完成与超时兜底竞争。3s 超时兜底覆盖前端无响应/已崩溃/flush 卡住。
- **纵深·AI 追加立即落库（`FrontstageApp.tsx` `appendAiContent`）**：`scheduleAutoSave(..., 2000)` 替换为 `void flushSceneSave()`（立即 fire-and-forget 落库）。AI 内容是离散完整块（非高频打字），立即落库合适；消除文思活跃连续续写 cancelAutoSave 反复重置定时器导致永不出火的丢失窗口；即使应用崩溃（非优雅关闭）内容也已落库。wordCount 已在上方 `setWordCount` 更新无需重复。
- **附带·章节切换前 flush（`FrontstageApp.tsx` `selectChapter`）**：`cancelAutoSave()` 替换为 `void flushSceneSaveRef.current()`，切换章节前将当前场景未保存内容落库，避免防抖窗口内的续写/编辑内容在切换章节时丢失（flush 内部已 cancelAutoSave 无需重复）。
- **提取 `flushSceneSave`（`FrontstageApp.tsx`）**：此前保护性保存（新建小说前 `cancelAutoSave + sync update_scene`，line 3888）与自动保存 saveFn 各自重复同一套 `update_scene` 逻辑；现提取为共享 `flushSceneSave` useCallback（cancelAutoSave -> 读 store sceneId/title + latestContentRef -> `loggedInvoke('update_scene')` -> setIsSaved + justSavedRef），通过 `flushSceneSaveRef` 暴露给 effect 监听器与 selectChapter。
- **验证**：`cargo test --lib` 1078 passed；`npx tsc --noEmit` ✅；`npx vitest run` 322 passed / 3 skipped；`cargo +nightly fmt` / `cargo clippy --lib`（baseline 540 零新增）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.32 - 增强性指令纳入世界观/故事大纲/场景大纲/上下文强关联

承接 v0.30.31 让世界观/故事大纲/场景大纲/进度彼此强关联后，用户指出增强性指令（logline 后缀）未被纳入这套强关联--增强后缀生成时不看世界观，进入管线后又与资产各居一隅、互不交叉引用，"失去了增强性指令的意义"。本版本补齐两个缺口：增强生成纳入世界观，指令与资产在 writer prompt 显式调和（资产=硬约束，指令=创作方向，在硬约束内落实指令核心意图，冲突时调整指令具体表现以符合约束但保留核心意图）。

- **P0-A·增强生成纳入世界观（`commands/orchestrator.rs` + `agency_logline_suffix_contextual.md`）**：`build_logline_context_sync` 此前只拉 story_outline/scene_outline/characters/current_content，**完全不读 `world_buildings`**，增强后缀可能在不知世界规则下提出违反世界观的设定。现 `LoglineContext` 新增 `world_setting` 字段，拉 `WorldBuildingRepository` 渲染 concept + rules 前3 + history（截断 1000），`build_contextual_logline_system` 注入 `world_setting` var；`agency_logline_suffix_contextual.md` 新增 `## 世界观设定` 段 + 输出要求"后缀须与世界观规则一致，不得提出违反世界观的设定或角色"。
- **P0-B·TriShot 指令纳入 `build_progression_anchor` + 显式调和（`agents/orchestrator.rs`）**：v0.30.31 的 `build_progression_anchor` 注入 story_outline/scene/progress/world 标记"最高优先级，不得偏离"，但**不接收指令参数、从不引用用户指令**；指令被 Call1 LLM 抽象进 `synthesized_prompt`，与资产各居一隅、无调和。现签名加 `user_instruction: &str`，指令非空时作为首个段【本次创作指令（你的创作方向，须与下方硬约束协调一致）】注入；收尾指令改为显式调和："本次创作指令是你的创作方向；故事大纲/场景大纲/世界观/已推进进度是硬约束。须在硬约束内落实指令核心意图--推进到故事大纲下一节点、遵循世界观规则、承接已推进进度。若指令与某硬约束冲突，调整指令的具体表现以符合约束，但保留指令核心意图；不得因约束丢弃指令，也不得因指令违反约束。"调用点传 `&task.input`（raw 指令，空则跳过指令段走原推进约束）；仅有指令无资产时输出指令段 + "推进剧情向前发展"。
- **P1-C·创世指令-资产调和（`agency/coordinator.rs`）**：`writer_first_chapter`/`writer_prose_fallback` 写作要求增"故事前提是你的创作方向；创作资产（世界观/大纲/伏笔）是硬约束，须在硬约束内落实前提核心意图，不得自相矛盾"；`writer_prose_fallback` 补回"资产区为准"系统提示。
- **P1-D·TimeSliced 指令-资产调和（`orchestrator_timesliced_writer.md` + `agents/orchestrator.rs` fallback）**：要求段加"写作指令须与故事上下文中的世界观、故事大纲、场景大纲协调一致；若冲突，在遵循上下文硬约束的前提下落实指令核心意图"。
- **验证**：`cargo test --lib` 1078 passed（+1：`test_build_progression_anchor_directive_only_no_assets` 边界；现有 2 测试更新为断言指令段 + 调和约束）；`cargo check` / `tsc` / `vitest`（322 passed / 3 skipped）/ `cargo +nightly fmt` / `cargo clippy --lib`（baseline 540 零新增）/ `architecture_guard` / `format:check` 全绿。
### v0.30.31 - 续写链路修复：世界观/故事大纲/场景大纲注入与剧情推进方向

用户报告"世界观设定没有体现在续写中，世界观和故事大纲、场景大纲结合不紧密，续写内容剧情推进不够紧凑，迷失剧情推进方向"。全面审计定位五类根因，聚焦幕前续写实际路径（Legacy TriShot）+ 共享生成端/prompt 资产 + Agency 注入函数顺带修复。进度指针用现有 `scenes.outline_content` 回读最近 3 章，无 DB 迁移、无 schema 变更。

- **P0-A·Legacy TriShot 确定性注入（最关键）**：根因--TriShot `final_prompt = Call1 LLM 合成的 synthesized_prompt`，manifest 不含 story_outline、synthesizer 不透传 bundle_prompt 关键段，故事大纲/场景大纲 outline_content/world_buildings 三者均不到达 writer（v0.30.15 注释声称修了 TimeSliced/TriShot，实际只修了 TimeSliced）。①`write_time_bundle.rs` load_sync 读 world_buildings 表（concept+rules前5+history+cultures前3，截断 2000）为 `world_setting`；`domain/write_time_bundle.rs` WriteTimeBundle 新增 `world_setting: Option<String>`；`to_prompt` 增【世界观设定】段。②`manifest.rs` build 增加 story_outline+world_setting 清单项（hard_constraint），scene_outline one_line 纳入 outline_content。③`orchestrator.rs` 新增 `build_progression_anchor`，在 `final_prompt = synthesized_prompt` 后确定性注入【剧情推进方向（最高优先级）】段（故事大纲1200+场景大纲800+已推进进度+世界观600+推进约束），`!is_fallback` 时注入。
- **P0-B·writer prompt 推进约束**：`writer_system.md`/`orchestrator_timesliced_writer.md`/`trishot_synthesizer.md` 各加"剧情必须推进到下一节点，不得原地踏步、不得仅复述设定或复述前文"。
- **P0-C·scene_outline.md 修伪前提 + 加 world/progress 变量**：删"按序号定位节点"伪前提（故事大纲是散文无编号节点），改为"根据【已推进进度】定位"；Legacy `creation_commands.rs generate_scene_outline` 加载 world_buildings + 最近 3 章 outline_content 注入 task.parameters，`service.rs build_outline_prompt` 读取注入 vars；Agency `generate_chapter_outline` vars 同步注入。
- **P1-A·Agency build_continue_writer_context 修复（顺带修）**：世界观全字段（concept+rules前5+history+cultures前3），此前只 concept+history 且超 6000 整段丢弃，现超预算截断降级注入；前文阈值倒挂修复（>8000->>12000 且保底最近 1 场正文 1500 字）；新增【已推进进度】段；`write_chapter` 三分支加推进约束+点名世界观。
- **P1-C·world_buildings 生成端填全字段**：`ensure_world_building` concept 存全文（此前截 500）；prompt 增"正文末尾用【核心规则】列出 3-5 条世界规则"，best-effort 解析存 rules；history 不再冗余存储（concept 全文已含）。
- **P1-D·editor 质量门预注入参照资产**：`evaluate_gate_impl` editor task 预注入参照资产（世界观红线+世界观设定+故事大纲），使"合同兑现/连续性/世界观一致性/推进方向"维度可校验。
- **验证**：`cargo test --lib` 1077 passed（+2：build_progression_anchor 全段注入 + 空场景返回空）；`cargo check`/`npx tsc --noEmit`/`npx vitest run`（322/3 skipped）/`cargo +nightly fmt`/`cargo clippy --lib`（baseline 540 零新增）/`architecture_guard`/`npm run format:check` 全绿。
### v0.30.30 - Agency 创作链路结构性优化：抗重复闭环 + 质量门宽松度 + 熔断不丢稿

承接 v0.30.29 内容质量根因修复后显式推迟的 D/E 两类结构性优化。聚焦 Agency 创世/续写链路三个"产出被白白丢弃"的结构性缺口：①创世装配写 RAW 正文不经清理（续写在 v0.30.29 已接清理三件套，创世没有）；②质量门 model 分对 scoreless pass 兜底 0.85 过宽松；③editor 完全评不出裁决时整 run 失败、writer MaxTurns/Deadline 熔断直接丢稿。把"熔断不等于丢稿"哲学（v0.30.19 salvage + 散文回退）补齐到 writer 与 gate Failed 两个剩余缺口，并把创世装配纳入与续写一致的清理管线。

- **D1·抗重复提示词补齐 + 创世装配接入清理三件套（`coordinator.rs` + 两份 agency 提示词资产）**：①提取共享 helper `cleanup_prose_for_persist(raw, story_id)`（`spawn_blocking` 内 `trim_self_repetition` -> `strip_existing_overlap`（取最新场景全文，无则跳过）-> `trim_dangling_tail`，join 失败回退原文）；创世 `review_and_assemble` 装配 Scene 前调此 helper（此前写 RAW `draft.content`）；续写 `handle_gate` 内联清理块替换为调此 helper（行为等价去重）。②`agency_lead_writer_system.md` 创作红线新增"禁止重复输出"一条；`agency_editor_auditor_system.md` 审查维度新增第 6 维"重复与复述" + `dimension_scores` 模板加 `"repetition":1-5`。③内联 writer prompts（`writer_first_chapter`/`writer_prose_fallback`/`write_chapter` 三分支/`build_revision_task`）各加一句禁止重复指令。
- **D2·失效 prompt_id 核查（结论：by-design，仅加注释）**：`agency/roles.rs` 中 `Writer/Inspector/OutlinePlanner/StyleMimic` 引用 `agency_writer_system` 等占位 ID（无 bundled 文件），但运行时回退 `default_role_prompt`（`roles.rs` 注释已明确），且这些角色不在创世/续写主流程（只用 LeadWriter/Producer/EditorAuditor）。9 个"orphan" prompt 文件已被 WalkDir 注册供用户覆盖/未来用，非 bug。仅在 `writer.rs`/`inspector.rs`/`outline_planner.rs`/`style_mimic.rs` spec 处补注释说明占位 ID 回退，无功能改动。
- **E1·模型分宽松 - scoreless pass 兜底 0.85 -> 0.7（`coordinator.rs` `ModelGraderReport::from_verdict`）**：editor 不给数值分只给 `verdict:"pass"`（本地模型常见）时 `model_score` 由 0.85 降到 0.7。Gate v2 加权 `0.2*code+0.3*rule+0.5*model` 阈值 0.75：0.85 时单 model 项贡献 0.425，code+rule 仅需 65% 满分即过门（太宽松）；0.7 低于阈值，须 code+rule 达 80% 满分才放行，code/rule 满分时仍可过（0.85）不误伤优质稿。`"revise"=0.4`/兜底 `0.5` 不动。
- **E2·editor 连累整 run - GateOutcome::Failed 降级放行（`coordinator.rs`）**：新增 helper `salvage_failed_gate(draft, reason) -> Option<EditorVerdict>`：草稿 `chars().count() >= 600`（substantive）-> 合成 `verdict:"pass"` 裁决（`comments` 透明记录降级原因），`log::warn!` 标记；草稿过短返回 `None`（不救垃圾稿）。4 个 Failed arm（genesis 首门/复审、续写首门/复审）由直接 `return Err` 改为先尝试 salvage：救回则继续装配（仍走 D1/C3 清理三件套），救不回才 Err。对齐 v0.30.19 salvage 哲学"熔断不等于丢稿"。
- **E3·writer 熔断丢稿 - MaxTurns/Deadline 先取黑板草稿（`coordinator.rs`）**：genesis 与续写 writer abort 处理原先仅在 `reason == "连续解析失败"` 时触发 `writer_prose_fallback`，`MaxTurns`/`Deadline` 直接 `return Err`。但 MaxTurns/Deadline 熔断前 writer 可能在早期轮次已 `board_write` 产出草稿到黑板 Draft 区（`LoopResult.output` 是占位串不含正文，黑板里有）。现统一：MaxTurns/Deadline 先 `latest_draft`/`latest_draft_by_key` 取回已产出草稿（`>= 200` 字符才用），取不到/过短才落 `writer_prose_fallback` 散文回退，仍失败才 Err。连续解析失败路径行为不变（黑板通常无稿 -> 直接散文回退）。
- **验证**：`cargo test --lib` 1069 passed（+4：scoreless pass 阈值 / salvage_failed_gate 长短稿边界 / cleanup_prose_for_persist 自重复清理 / 续写 writer MaxTurns 黑板取回）；`cargo check` / `npx tsc --noEmit` / `npx vitest run`（322 passed / 3 skipped）/ `cargo +nightly fmt` / `cargo clippy --lib`（baseline 540 零新增）/ `architecture_guard` / `npm run format:check` 全绿。
### v0.30.29 - 内容质量根因修复：强模型结构化大纲不再被丢弃 + 大纲/世界观约束到生成链路

- **P0 根因·DepthAssets 结构化 outline（`coordinator.rs`）**：`outline: String` -> `serde_json::Value`（兼容 String/Object/Array）；新增 `normalize_outline` 将结构化对象（core_conflict/three_act_structure{act1,act2,act3}/turning_points）渲染为可读文本（【核心冲突】【三幕结构】【关键转折点】），未知字段 fallback `to_string()`；新增 `outline_value_is_empty` 判空；散文兜底 `outline` 改 `Value::Null`；内联 prompt 鼓励整书三幕+转折点大纲。**实证根因**：强模型返回结构化整书大纲对象时 serde `String` 类型不匹配 -> `parse_lenient` 返回 `None` -> 走散文兜底（`outline=空`）-> 大纲不写 `story_outlines` 表 -> 首章与续写都看不到大纲（模型越强、大纲越完整，越被丢弃）。下游零改动（`content` 仍 TEXT，消费者已当纯文本处理）。
- **P1·创世首章注入 world/outline（`coordinator.rs`）**：Phase B 编排由多模型 `tokio::join!(writer, producer)` 并行改串行 producer-first（producer 先写深度资产到黑板 Asset 区，writer 后读资产写首章）；新增 `build_assets_ctx_brief` helper（读 Asset 区 3000 字符预算）注入 `writer_first_chapter`/`writer_prose_fallback`，system prompt 增补"人设/世界观/伏笔以资产为准不得自相矛盾"。消除首章在无大纲/无世界观下写就的脱节；任一失败仍上抛回退 legacy。
- **C1·续写注入 MASTER_SETTING 红线（`coordinator.rs` `build_continue_writer_context`）**：上下文最前注入合同红线（`StoryContractRepository::get_by_type` + `extract_redline_text` 800 字截断），对齐 C 链路 `WriteTimeBundle.to_prompt` "红线最前最突出"不变量；Agency 续写此前完全绕过红线。
- **C3·续写落库前抗重复三件套（`coordinator.rs` `handle_gate`）**：装配 Scene 前对 `draft.content` 应用 `TextUtils::trim_self_repetition` -> `strip_existing_overlap`（取最新场景全文比对尾部 3000 字）-> `trim_dangling_tail`，与 C 链路 `orchestrator.rs` 同款；此前自重复/复述/截断半句直接入库回灌污染后续章节。
- **C4·章节大纲改用 scene_outline.md（`coordinator.rs` `generate_chapter_outline`）**：硬编码 prompt 替换为 DB-backed `resolve_prompt_with_vars(pool, "scene_outline", &vars)`（`spawn_blocking`，支持用户在提示词管理界面覆盖），vars 单独查库（story_outline/scene_number/characters/scene_info）；`unwrap_or_else` 保留硬编码 fallback。章节大纲受"禁止发明新角色、定位故事大纲节点"强约束。
- **验证**：`cargo test --lib` 1065 passed（+5：normalize_outline 对象/字符串/空/部分/未知 fallback；C1 红线注入扩展）；`cargo check`/`npx tsc --noEmit`/`npx vitest run`（322 passed / 3 skipped）/`cargo +nightly fmt`/`cargo clippy --lib`（baseline 540 零新增）/`architecture_guard`/`npm run format:check` 全绿。
### v0.30.28 - UI 双模式设计系统重塑；落地页下载自动同步；幕前交互打磨

- **双模式设计系统**：幕前「墨纸」（`--paper-*`/`--ink-*`/`--terracotta*`，无阴影扁平）与幕后「机械」（`--cinema-*`/`--cinema-gold*`，多层阴影仪表板）落地为统一 token；`tailwind.config.js` 暴露 `paper`/`ink`/`terracotta`/`cinema`/`status` 调色板与 `rounded-paper`/`rounded-panel`/`shadow-panel`。幕后仪表盘外壳、机械设置页、导航轨去重 + 无障碍、墨纸↔机械模式切换按 `docs/plans/2026-07-27-ui-redesign-design.md` 实现；硬编码颜色全量 token 化，`@apply` 不透明度改 `color-mix(in oklch,...)` 修 Vite 构建。
- **落地页自动同步**：`landing/src/hooks/useLatestRelease.ts` 运行时 fetch `latest.json` 拼下载链接（模块级 cache + in-flight promise + `FALLBACK_VERSION` 兜底），发版自动跟随无需重部署；AGENTS.md 新增用户级规则 #7（兜底版本随发版 bump、bundle 命名变更校验）。
- **幕前交互打磨**：恢复 UI 重塑误删的 `.frontstage-input-ghost*` CSS（`frontstage.css`），logline 后缀灰色 + 13px 小字号、前缀占位对齐消除层叠；`FrontstageBottomBar.tsx` 剥离冲突 Tailwind 工具类让 CSS 为唯一源。发射/取消按钮扁平化（`rounded-md` 淡彩底 + 陶土色图标，移除 `active:scale`，修无效 `text-cinema-50`）。移除 ghost-chrome 静止蒙版：删 `useGhostChrome` hook + 测试，`FrontstageHeader`/`FrontstageBottomBar` 不再鼠标静止 3s 淡出至 `opacity 0.08`，常驻完整不透明。
- **验证**：`cargo test -p storymoss` 1060 passed（无 Rust 变更）；`npx vitest run` 322 passed / 3 skipped；tsc / fmt / clippy（540 零新增）/ prettier / architecture_guard 全绿；landing 24 passed。
### v0.30.27 - 上下文感知 Logline 后缀；输入框自适应高度

- `generate_logline_hint` 在提供 `story_id` 时拉取故事大纲、当前章节大纲、角色列表与最近正文，渲染新 prompt 资产 `agency_logline_suffix_contextual`，生成贴合上下文的后缀；无上下文时回退原 `agency_logline_suffix`。
- `FrontstageBottomBar.tsx` 通过 `textareaRef` + `useEffect` 根据 `inputValue`/`ghostHint`/`loglineHint` 动态调整 textarea 高度（上限 200px），幽灵层不再固定 `max-height: 60px`。
### v0.30.26 - 统一 Logline 增强提示为内联幽灵文本；修复分时预检缺少角色

- **根因**：v0.30.24 的 logline 增强提示以独立 div 显示在输入栏下方，与统一的幽灵提示系统不一致；用户按 `->` 后直接用完整 logline 替换原输入，导致意图分类可能将增强后的长文本误判为续写，进而进入 TimeSliced 路径触发“缺少角色”预检失败。
- **Fix 1（UI 统一·FrontstageBottomBar.tsx + frontstage.css）**：移除 `.frontstage-logline-hint` 独立建议条；改为在 `frontstage-input-ghost-wrapper` 内渲染内联幽灵文本——前缀用 `visibility: hidden` 占位以与输入框文本对齐，后缀以灰色透明样式显示在已输入内容之后。loading 状态仍以内联形式提示“正在生成增强版指令…”。
- **Fix 2（交互·FrontstageApp.tsx）**：按 `->` 时执行 `setInputValue(inputValue + loglineHint)` 并清空 `loglineHint`，随后按 Enter 提交“原输入 + 增强后缀”组合文本。移除 `originalInputForLoglineRef` 与 `intentClassificationInput` 透传，`handleSmartGeneration` 恢复只接收 `userInput`，意图分类统一基于当前输入框文本。
- **Fix 3（Prompt 资产·orchestrator.rs）**：新增 `resources/prompts/agency/agency_logline_suffix.md`，要求 LLM 只输出应追加到原输入后的后缀；`generate_logline_hint` 改用该 prompt，返回后缀字符串。
- **Fix 4（分时预检缺少角色·intent.rs + preflight.rs + FrontstageApp.tsx）**：后端意图分类兜底路径增加按输入文本判断创世意图；`QuickPreflightChecker` 在角色表为空时自动创建占位主角（仅一次 DB 写入，不触发 LLM auto_contract），避免空角色表阻塞生成；前端接受 logline 提示后用原输入做意图分类。
- **验证**：`cargo test -p storymoss` 1060 passed；`npx vitest run` 310 passed / 3 skipped；fmt / clippy（baseline 549 零新增）/ tsc / prettier / architecture_guard 全绿。
### v0.30.25 - 修复续写 600s 超时（auto_contract 阻塞 + reasoning_content 丢失 + 无超时）

- **根因（三层叠加）**：用户输入"续写"后卡死 600s。①前端 `FrontstageApp.tsx` 在调用 `smart_execute` 前 `await autoCreateMissingContracts`，`auto_fill` 串行 4 次 LLM 调用（~6 分钟），v0.26.22 的 `is_silent_background` 只隐藏了 `isAnyBackendActive` 但 `await` 仍阻塞且 `isGenerating=true` 触发 600s 看门狗；后端 TimeSliced 续写路径本已跳过 auto_contract，但前端从未调用到。②DeepSeek 推理模型把思维链放在 `reasoning_content` 字段，`openai.rs` 的 `Message` 结构体不捕获该字段 -> serde 丢弃 -> `content=""`（0 字符）但 `tokens=2643`，auto_contract 收到空内容静默失败，合同永远补不齐。③`auto_contract.rs` 的 `auto_fill` 每个 `build_*` 调用无超时，单个慢模型调用阻塞数分钟。
- **Fix 1（主修复·`FrontstageApp.tsx`）**：续写请求不再阻塞 auto_contract。`handleSmartGeneration` 中 `classification.is_continuation` 时后台 fire-and-forget `autoCreateMissingContracts`（不 await，直接进入 smart_execute）；`handleRequestGeneration`（仅续写入口）同理。新增 `fireAutoContractInBackground` helper + `autoContractInProgressRef` 防并发 + 非阻塞 toast（成功/失败）。非续写请求（rewrite/audit）保持原有阻塞 await 行为。
- **Fix 2（次修复·`openai.rs`）**：`Message` 结构体加 `#[serde(skip_serializing, default)] reasoning_content: Option<String>`；`OpenAiDelta` 同理。非流式/流式提取在 `content` 为空时 fallback 到 `reasoning_content`，并 `log::warn!`。提取纯函数 `resolve_content` 供单测。
- **Fix 3（三修复·`auto_contract.rs`）**：`auto_fill` 的 4 个 `build_*` LLM 调用各包 `tokio::time::timeout(30s)`，超时与 Err 同处理（warn + 跳过 + 继续）。总上限 120s（4×30s），远低于 600s。
- **验证**：`cargo test --lib` 987 passed（+5：resolve_content fallback + Message 反序列化）；`npx vitest run` 311 passed；fmt / clippy（baseline 549）/ tsc / prettier / architecture_guard 全绿。
### v0.30.24 - Logline 幽灵提示（用户输入简单创世指令时实时生成增强版 logline）

- **功能**：用户在输入栏输入简单创世指令（如"写一部现代间谍的长篇小说"）后，后台用 v0.30.22 的 PROBLEM logline 生成功能产出一句话强力 logline，以幽灵提示形式显示在输入栏下方，用户按 `->` 即可用 logline 替换原始简单指令再执行。避免用户简单指令得不到好的生成结果而反复试，同时为用户提供故事创意的体验和技能锻炼。
- **后端（`commands/orchestrator.rs` + `handlers.rs`）**：新增 `generate_logline_hint` 命令--输入为空或 ≥ 100 字符返回 `None`（与 v0.30.22 `< 100 字符` 触发条件对齐）；复用 `agency_problem_logline` prompt 资产（用户在幕后编辑后自动生效）；`LlmService::generate_for_task_with_system_prompt` + 15s 超时；失败/超时静默返回 `None`。提取纯函数 `should_skip_logline_generation` / `is_valid_logline` 供单测。
- **前端状态（`FrontstageApp.tsx`）**：新增 `loglineHint` / `loglineHintLoading` state + `loglineHintTimerRef` / `loglineHintReqIdRef` 防抖 ref；`useEffect` 监听 `inputValue` 变化，1.5s 防抖后调 `generateLoglineHint`，请求 ID 防竞态（仅接受最新请求结果）；`handleInputKeyDown` 扩展 `->` 接受 logline（输入非空时，区别于 ghost hint 的空输入场景）+ `Esc` 清除；`handleInputSubmit` 清理 logline hint。
- **UI（`FrontstageBottomBar.tsx` + `frontstage.css`）**：输入框下方新增 `.frontstage-logline-hint` 建议条--loading 时显示旋转图标 + "正在生成增强版指令…"；就绪后显示 Lightbulb 图标 + logline 文本 + "按 -> 使用"提示；点击建议条也可接受（等同于 `->`）。CSS 含淡入动画 + hover 高亮。
- **不干扰现有幽灵提示系统**：现有 `ghostHint` 仅在输入为空时显示（placeholder 式），logline 提示在输入非空时显示（suggestion 式），是独立的 UI 层。两个 `->` 处理互斥（ghost hint 要求 `!inputValue`，logline hint 要求 `inputValue`）。
- **验证**：`cargo test --lib` 982 passed（+4：should_skip / is_valid 纯函数守卫）；`npx vitest run` 311 passed（+4：logline 渲染 / loading / 点击接受 / 空输入不渲染）；fmt / clippy（baseline 550，实际 549）/ tsc / prettier / architecture_guard 全绿。
### v0.30.23 - 意图分类 Bug 修复（LLM 分类去偏 + 失败兜底上下文化）

- **根因（LLM 分类本身被破坏）**：用户输入"写一部现代间谍的长篇小说"被分类为续写 -> `VALIDATION_FAILED: 请先在左侧选择或创建一个作品`。5 层防线失守：①提示词注入 `已有故事=true` 上下文偏差使 LLM 倾向续写；②`仅当"明确要求新开一部"` 过于保守；③无正例；④兜底 `conservative_fallback()` 恒返回 `is_new_novel=false` 无视 DB 状态；⑤失败结果被缓存。
- **Fix A（提示词去偏·主修复·intent.rs）**：`build_classification_prompt` 移除 `上下文：已有故事={story}` 上下文注入行（偏差来源）；移除 `仅当` 保守措辞；新增 3 个正例（"写一部科幻小说" -> is_new_novel=true）。LLM 不再受 DB 状态偏差，基于用户输入本身判定意图。
- **Fix B（上下文感知兜底·intent.rs）**：新增 `conservative_fallback_with_context(has_existing_story)`--LLM 失败时无故事返回创世（不可能续写不存在的作品），有故事返回续写。3 个兜底路径全部改用。原 `conservative_fallback()` 标记 `#[deprecated]`。
- **Fix C（不缓存失败·intent.rs）**：仅 LLM 成功解析的结果写入缓存，兜底结果不缓存。缓存键简化为仅 `user_input`（提示词不再使用上下文）。
- **Fix D（前端兜底上下文化·FrontstageApp.tsx）**：catch 块和 null 防御两处 LLM 失败兜底从硬编码 `is_new_novel: false` 改为 `is_new_novel: stories.length === 0`。`isBootstrap` 判定不变（尊重 LLM 结果）。
- **设计原则**：LLM 是意图判断的唯一权威；不回到硬编码关键词匹配；不用 `|| !has_existing_story` 覆盖 LLM 结果；DB 状态仅在 LLM 失败兜底时使用。
- **验证**：`cargo test --lib` 978 passed（+4）；`npx vitest run` 307 passed；fmt / clippy（baseline 550）/ tsc / prettier / architecture_guard 全绿。
### v0.30.22 - PROBLEM 七元素框架集成（Logline 生成 + 故事大纲增强）

- **背景**：用户输入简单指令（如"写一部科幻小说"）直接作为 `premise` 透传到 `concept_pack` -> `genesis_fastpath`，全程无方向约束。`concept_pack` 虽生成 `logline` 字段但从未使用，`ensure_story_outline` 提示词宽松，缺乏结构化创意质量检验。
- **核心**：将 Erik Bork 的 PROBLEM 七元素（Punishing/Relatable/Original/Believable/Life-Altering/Entertaining/Meaningful）编码为可编辑 prompt 资产，在创世和续写两个关键点注入。
- **Phase 1（Prompt 资产）**：新增 `agency_problem_logline.md` + `agency_problem_outline.md`，WalkDir 自动注册。
- **Phase 2（DB）**：V114 迁移 `stories ADD COLUMN logline TEXT`；`Story` model + `StoryRepository`（3 SELECT 加列 + `update_logline`）。
- **Phase 3（Logline 生成）**：`generate_logline` 单次 Producer LLM 调用；`run_genesis_inner` 在 `concept_pack` 前检测简单前提（< 100 字符）生成 logline 替换原 premise；genesis 成功后 `update_logline` 持久化。
- **Phase 4（大纲增强）**：`ensure_story_outline` system prompt 从 registry 加载 PROBLEM 大纲提示词 + logline 上下文；`producer_depth_assets` outline 字段增强 PROBLEM 指引。
- **Phase 5（Writer 上下文）**：`build_continue_writer_context` 追加 `【故事Logline】` 注入。
- **验证**：`cargo test --lib` 974 passed（+3：logline 生成 / 跳过 / 持久化）；fmt / clippy（baseline 550）/ tsc / prettier / architecture_guard 全绿。
### v0.30.21 - 续写资产层级生成（世界观 -> 故事大纲 -> 章节大纲 -> 正文）

- **根因**：续写路径 `ensure_assets`（`coordinator.rs`）仅检查 `characters` 表行数，角色存在即返回--不检查、不生成 world_buildings / story_outlines。`build_continue_writer_context` 不注入故事大纲，`write_chapter` task 仅"续写第N章"无方向约束，导致生成内容缺乏方向和情节推进。
- **Fix A（ensure_assets 扩展）**：角色检查后追加 world_buildings / story_outlines 检查；缺失时调 `ensure_world_building` / `ensure_story_outline` 单次 Producer LLM 调用（不跑 tool_loop，不抢主创 LLM）生成并落库。失败时 `log::warn` + `Ok(())` 不阻断续写。
- **Fix B（build_continue_writer_context 注入故事大纲）**：读 `story_outlines.content` 注入 writer task（4000 字符预算），为 writer 提供整体推进方向。
- **Fix C（generate_chapter_outline）**：writer tool_loop 前单次 Producer LLM 调用生成章节大纲（服从故事大纲），写入黑板 Draft 区 `key=outline-{chapter_key}`。无故事大纲时跳过（返回空串）。strict writer task 含故事大纲 + 本章大纲 + 写作要求（起伏/转折/冲突）。
- **Fix D（handle_gate 存储 outline_content）**：装配 Scene 时从黑板读取章节大纲，存入 `scenes.outline_content`。
- **层级约束**：世界观构建 -> 故事大纲 -> 章节大纲 -> 正文，每一层基于上一层生成，形成从世界观到正文的约束链。
- **验证**：`cargo test --lib` 971 passed（+4：ensure_world_building / ensure_story_outline / generate_chapter_outline / skip_without_story_outline）；fmt / clippy（baseline 550）/ tsc / architecture_guard 全绿。
### v0.30.20 - Agency 续写效率优化与质量门硬化

- **背景**：对照创世路径（`run_genesis`）审计续写路径（`run_continue` / `run_continue_batch`），发现三项结构性缺口（无 run 级 deadline / 无资产预注入 / 无散文回退）+ editor 质量门 deadline 仍为 None（v0.30.4 有意豁免，但 v0.30.19 的 salvage + prose_fallback 已使 deadline 安全可行）。
- **P0-1 续写 run_deadline**（`coordinator.rs`）：`run_continue` / `run_continue_batch` 入口调 `setup_run_deadline()`，与创世一致。`setup_run_deadline` 在 `app_handle=None`（测试环境）时 no-op。deadline 设置后 `run_role_with_llm_and_budget` 自动读取并传给 tool_loop（剩余 <30s 熔断保产出）。
- **P0-2 续写 writer 散文回退**（`coordinator.rs`）：参数化 `writer_prose_fallback` 新增 `chapter_key: &str`（替换硬编码 "第1章"），prompt "第一章正文" -> "章节正文"（generic）；`write_chapter` 熔断时 reason == "连续解析失败" 回退散文单调用（与 genesis legacy 同理），MaxTurns/Deadline 仍 Err。
- **P0-3 续写 writer 上下文预注入**（`coordinator.rs`）：新建 `build_continue_writer_context` 从 DB 读角色（`CharacterRepository`）/世界（`WorldBuildingRepository`）/最近 2 场景（`SceneRepository`），格式化截断 8000 字符注入 writer task，消除多轮 board_read/asset_query 轮询（tool_loop 从 3-7 轮降到 1-2 轮）。空则保留原 task（writer 自轮询）。
- **P1-1 Editor 质量门 deadline**（`coordinator.rs`）：`evaluate_gate_impl` 新增 `deadline: Option<Instant>` 参数替换原 `None`；`evaluate_gate` 方法传 `self.current_deadline()`；`GateRunner` 结构体新增 `deadline` 字段，`gate_runner()` 工厂设 `self.current_deadline()`，`GateRunner::evaluate` 传 `self.deadline`。v0.30.19 的 salvage + prose_fallback 使 deadline 安全（熔断后仍有两次兜底）。
- **P1-2 Editor 草稿预注入**（`coordinator.rs`）：`evaluate_gate_impl` editor task 从 "审查 draft 区的最新章节草稿" 改为注入 `draft.content`（截断 8000 字符），editor 无需 board_read 即可审查（tool_loop 从 2 轮降到 1 轮）。
- **P1-3 连接超时调优**（`config/settings.rs`）：`llm_connect_timeout_secs` 默认 60s -> 15s（TCP 连接建立超时，非响应超时；模型不可达时浪费从 240s 降到 60s）。
- **验证**：`cargo test --lib` 967 passed（+2：续写散文回退 + 上下文预注入）；fmt / architecture_guard 全绿；clippy 549（零新增）；tsc 通过。
### v0.30.19 - 质量门编辑审计 Agent 熔断修复（本地模型 JSON 不遵从，散文回退）

- **根因**：`evaluate_gate_impl`（`coordinator.rs`）中 editor_auditor 的 ReAct tool_loop 在本地模型（Qwen 3.6）不遵从 JSON action 格式时连续解析失败/达到最大轮数（6 轮）熔断。原实现 `if editor_out.aborted` 直接返回 `GateOutcome::Failed`，既不 salvage 末轮输出也不尝试替代路径，导致整 run 失败。与 v0.30.3 writer 熔断同类（本地模型 JSON 不遵从），但 editor 路径此前无散文回退。
- **Fix（两层兜底，`coordinator.rs`）**：①salvage--移除 `editor_out.aborted` 早返回，熔断时仍先 `parse_lenient::<EditorVerdict>` 尝试从末轮输出提取裁决 JSON；salvage 成功则用之，熔断则 break 进散文回退（同模型重试必同败，不重试）。②散文回退--新增 `editor_verdict_prose_fallback` 自由函数，单次 `llm.complete()` 直接请求裁决 JSON（不经 tool_loop/工具），复用 editor 系统提示词审查标准 + 追加「直接输出 JSON、不走工具循环」强约束。与 `writer_prose_fallback`（v0.30.3）同理。回退失败才降级 `Failed`。
- **验证**：`cargo test --lib` 965 passed（+1 `test_editor_verdict_prose_fallback` 正向回归；2 个现有熔断测试更新为显式验证回退也失败时 run 仍 failed）；fmt / clippy（baseline 550 零新增）/ architecture_guard 全绿。
### v0.30.18 - 修复幕前意图分类 null 崩溃（v0.30.16 CI E2E PAGEERROR 根因）

- **根因**：`handleSmartGeneration`（`FrontstageApp.tsx`）调用 `classifyIntent` 后直接读 `classification.is_new_novel`。`classifyIntent` 走 `loggedInvoke`（出错即 throw），但 **resolve 为 null 时不抛异常**--catch 块只拦抛出异常，无法拦截 null。E2E 环境 `e2e/mock-tauri.ts` 对未注册命令默认 `return null`（`classify_intent` 未 mock），导致 `classifyIntent` resolve 为 null -> `null.is_new_novel` -> `TypeError: Cannot read properties of null (reading 'is_new_novel')` PAGEERROR，幕前崩溃，连带 6 个 E2E（设置页/自动保存/创世重复）失败。v0.30.16 master 与 tag 两次 CI 均 hit；v0.30.15 未触发（非确定性）。真实用户若后端序列化异常返回 null 也会崩。
- **Fix（`FrontstageApp.tsx`）**：① catch 块之后新增 post-catch null 兜底--`if (!classification)` 时填充续写兜底对象（与 catch 同语义：`is_new_novel=false`，因误判续写为创世会启动 Agency 全流程覆盖工作，故默认偏向续写），避免 `null.is_new_novel` 崩溃；② 不再缓存 null 结果（`if (classification)` 守卫 cache.set），避免缓存 null 导致每次重入重调。
- **macOS 构建失败（v0.30.16 tag）**：`Failed to create Info.plist: Io(code 5, "Input/output error")`--GitHub macOS runner 瞬时磁盘 I/O 错误，同代码 master 构建成功（39m18s）。属 flake，已 `gh run rerun --failed` 重建，非代码问题。
- **验证**：`npx tsc --noEmit` ✅；`npx vitest run` 307 passed / 3 skipped（含 genesis-duplicate 14 项）；`npm run format:check` ✅。纯前端，cargo 基线 964 不变。
### v0.30.17 - 幕前顶部创世状态显示三 Agent 动作/进度

- **背景**：用户反馈幕前顶部创世流程状态提示信息不足，看不出「主创在干嘛、做完了什么工作」。Agency 创世的后端 `agency-agent-activity` 事件（`coordinator.rs` emit_activity，role=lead_writer/producer/editor_auditor，action=start/done，detail=概念/首章/深度资产/审查/装配）早已存在，但此前仅幕后 `AgencyStudio` 消费，幕前未订阅。底部 LLM 连接状态本次不动。
- **新增 `useAgencyAgentActivity` hook（`src-frontend/src/frontstage/useAgencyAgentActivity.ts`）**：幕前订阅 `agency-agent-activity`，按 主创/管理/编辑审计 顺序聚合各角色最新一条活动，产出 `{ text, done }[]` 文案（进行中「主创正在写第一章」，已完成「管理已完成深度资产」）；订阅 `agency-run-progress`，run 结束（completed/failed/cancelled/error）时清空，避免创世结束后残留陈旧进度。
- **接线 `FrontstageHeader.tsx`**：顶部状态栏在 `orchestratorStatus` 之后渲染各 Agent 进度条目（进行中琥珀 `saving` 态、已完成绿色 `saved` 态，title 标注「创世多 Agent 进度（主创 / 管理 / 编辑审计）」），无活动时不占位。
- **附带（用户级永久指令）**：`AGENTS.md` 强制构建规则 #2 改为「本地构建仅在用户明确要求时执行」--推送后由 GitHub Actions 负责全平台构建，本地仅跑 `cargo test` / `tsc` / `vitest` 等验证命令，省略耗时 `cargo tauri build` 打包。
- **验证**：`npx tsc --noEmit` ✅；`npx vitest run` 307 passed / 3 skipped（+2：三 Agent 进度渲染 + run 结束清空）；`npm run format:check` ✅。纯前端，无 Rust 变更（cargo 基线 964 不变）。
### v0.30.16 - 故事资产手动编辑（补齐编辑缺口）

- **背景**：审计后台发现 故事大纲/故事摘要 只读展示（`useUpdateStoryOutline`/`useUpdateStorySummary` hook 零调用），伏笔无内容编辑+删除，角色关系无编辑。角色/世界构建/场景已有完整编辑，无需改动。
- **Gap 1 故事大纲编辑（`pages/Stories.tsx`）**：只读 `<p>` 改为 查看/编辑 切换（textarea + 保存/取消），保存调 `useUpdateStoryOutline`（后端命令已就绪）。
- **Gap 2 故事摘要编辑（`pages/KnowledgeGraph.tsx`）**：抽取 `SummaryCard` 组件，查看/编辑 切换 + 保存，调 `useUpdateStorySummary`。
- **Gap 3 伏笔内容编辑+删除（后端+前端）**：`ForeshadowingTracker` 新增 `update_foreshadowing`/`delete_foreshadowing` 方法；新增 `update_foreshadowing`/`delete_foreshadowing` Tauri 命令并注册（`handlers.rs`）；前端新增 `useUpdateForeshadowing`/`useDeleteForeshadowing` hook，`Foreshadowing.tsx` 卡片加 编辑表单（内容/重要性/设置场景）+ 删除按钮。
- **Gap 4 角色关系编辑（前端）**：新增 `useUpdateCharacterRelationship` hook（后端 `update_character_relationship` 已存在），`Characters.tsx` 的 `RelationshipCard` 加 编辑表单（关系类型/描述）。
- **验证**：`cargo test --lib` 964 passed；`npx vitest run` 305 passed；tsc / `cargo +nightly fmt` / clippy（零新增，baseline 550）/ architecture_guard 全绿。
### v0.30.15 - 场景围绕故事大纲生成（创作原则加固）

- **创作原则**：有故事大纲时，场景必须围绕故事大纲展开。用户报告续写内容与故事大纲"两张皮"（场景大纲写"金敏秀"，续写跑偏到核电站，与故事大纲"韩雪/李明在首尔"脱节）。
- **根因 A（场景大纲生成用错提示词）**：`generate_scene_outline` 复用故事级 `outline_planner.md`（要求三幕式/章节划分/角色弧线），`task.input` 几乎为空且**不注入 story_outlines.content** -> 模型幻觉新角色"金敏秀"（不在角色卡），场景大纲与故事大纲冲突。
- **根因 B（writer 看不到故事大纲）**：续写走 TimeSliced/TriShot，prompt 只用 `WriteTimeBundle.to_prompt()`；故事大纲只在 Full/Fast 路径计算，**从未到达 writer** -> 内容偏离大纲。
- **Fix A（场景大纲生成锚定故事大纲，`creation_commands.rs` + `agents/service.rs` + 新提示词）**：新增场景级提示词 `resources/prompts/planner/scene_outline.md`（强制复用已登场角色、禁止发明新角色、围绕故事大纲对应节点展开）；`generate_scene_outline` 加载 `story_outlines.content` + 场景序号注入 `task.parameters`；`build_outline_prompt` 分流（场景模式用 `scene_outline`，workflow 故事级仍用 `outline_planner`）。
- **Fix B（writer 锚定故事大纲，`domain/write_time_bundle.rs` + `creative_engine/write_time_bundle.rs`）**：WriteTimeBundle 新增 `story_outline` 字段，`load_sync` 加载 `story_outlines.content`，`to_prompt()` 在世界观红线**之后**插入权威段【故事大纲（本场景必须围绕此大纲展开，禁止偏离）】（保持红线第一不变量）；冲突时以故事大纲为准并使用已登场角色。一处覆盖 TimeSliced+TriShot。
- **验证**：`cargo test --lib` 964 passed（+4）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。
- **注意**：现有"金敏秀"场景大纲需用户重新点"生成大纲"覆盖（Fix A 修生成器）；Fix B 让 writer 即使面对旧毒大纲也锚定故事大纲。
### v0.30.14 - 续写返回风格增强模板修复（多步 plan 尾部非 writer 覆盖正文）

- **根因（结构性）**：`execute_plan`（`planner/executor.rs:685-687`）用**最后产出 `content` 的步骤**作为 `final_content` 返回用户。force-correction（防线 2）只修正**首步**，无法拦截多步 plan **尾部**的 `style_enhancer`/`inspector`--尾部非 writer 的模板/报告会覆盖 writer 已产出的正文。用户报告"增强第二章"得到 `[inspector, style_enhancer]` 多步 plan，style_enhancer 收到 inspector 报告后抱怨"这是一份质量检查报告而非章节原文"。这是该误路由 bug **第 5 次复发**（v0.30.10/11/12/13 各堵一条路径：模板重放/朴素子串/inspector 漏拦/SING 绕过，但多步尾部漏网）。
- **Fix（防线 3，`planner/mod.rs` + `planner/executor.rs`）**：新增 `PlanGenerator::sanitize_plan_for_prose_request`，在 plan 执行咽喉点 `execute_with_context`（force-correction 之后）对所有 `is_prose_request` plan 统一净化：①移除 `builtin.style_enhancer`/`text_formatter`/`character_voice`/`emotion_pacing` 等绝不产出正文的技能步骤；②续写（`is_continuation`）塌缩为单 writer 步；③其余 prose 请求弹出尾部非 writer 步骤，**保证末步为 writer**（`final_content` = 正文），保留 `[inspector, writer]` 等 Rule 9 合法流；④净化后空则补 writer 步。非 prose 请求（显式审查 `Audit`/`is_prose_request=false`）不净化。
- **验证**：`cargo test --lib` 960 passed（+12 sanitize 回归）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。
### v0.30.13 - 续写返回风格增强模板修复（SING 路径绕过 force-correction）

- **根因**：planner force-correction（防线 2）只在 `PlanGenerator::generate_plan` 内施加，而 `PlanExecutor::execute_with_context` 的 SING（IntentionGraphPlanner）路径直接返回 plan（`planner/executor.rs:148-178`）、**完全绕过** `generate_plan`。当 SING 把续写路由到 `builtin.style_enhancer`（Skill 资产）作为首步时，force-correction 从不执行，style_enhancer 收到空 content 返回"请提供需要增强的原始文本"模板。v0.30.11 禁用模板重放消除了模板路径，但 SING 路径的绕过漏洞仍在。
- **Fix（结构修复，`planner/mod.rs` + `planner/executor.rs`）**：提取 `PlanGenerator::force_correct_first_step_to_writer` 为 `pub(crate)` 方法（封装 swap + understanding/purpose 标注），在 `generate_plan` 与 **plan 执行咽喉点** `execute_with_context`（所有 plan 来源 SING/PlanGenerator/fallback 必经，`execute_plan` 之前）**统一施加**。SING 路径产生的 `builtin.style_enhancer`/`inspector`/`outline_planner` 等首步经咽喉点修正为 `writer`。幂等：已为 writer 的首步不受影响，两处重复调用安全。
- **验证**：`cargo test --lib` 948 passed（+4 咽喉点回归）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550 -> 549）。
### v0.30.12 - 续写返回审查报告修复（force-correction 漏拦 inspector）

- **根因**：planner force-correction（`planner/mod.rs` 防线 2）的"强制改 writer"capability 列表含 `outline_planner`/`style_mimic`/`plot_analyzer`/`builtin.*`，**漏掉 `inspector`**；提示词 Rule 9 允许"有内容时用 inspector 先审后写"、Rule 21 never-use 列表也漏 inspector。本地模型（Gemma-4-31B）把"继续写当前这部小说"误判为"审查/改进已有文本"路由到 `inspector` -> force-correction 不拦 -> inspector 运行 `inspector_system` 提示词 -> 产出"总体评分 0.85 / 具体问题清单"审查报告作为生成结果。
- **Fix A（force-correction 主修复）**：提取纯函数 `PlanGenerator::should_force_correct_to_writer`（可单测），将 `inspector` 纳入 swap-to-writer 列表，按 LLM 分类分流：续写（`is_continuation`）/ 创世 / 无分类 / 审查且 `is_prose_request=true`（分类矛盾兜底）强制 `writer`；仅纯审查（`Audit` 且非 prose）与改写润色（`Rewrite`，Rule 9 流，最终输出仍是 writer 正文）保留 `inspector`。
- **Fix B（提示词）**：Rule 9 澄清"继续写/续写/往下写"是续写而非 refine，必须直接 `writer`、绝不用 `inspector`；Rule 21 将 `inspector` 加入 prose 请求 never-use 列表。
- **验证**：`cargo test --lib` 944 passed（+8 force-correction 回归）；`npx vitest run` 305 passed；tsc / fmt / clippy / format:check 全绿。
### v0.30.11 - 全面整改：用 LLM 解析器替换朴素子串意图匹配

- **背景**：审计全项目发现 ~30 处 `.contains()`/`.includes()` 朴素子串匹配，其中 6 处高危直接在用户自然语言输入上做意图路由（`find_match`、`is_novel_creation_intent` 前后端、`from_instruction_and_context`、force-correction、`synthesize_query_rule_based`），是 v0.30.10 `PlanTemplateLibrary` bug 的同类。用户指示用 LLM 解析器替代。
- **核心：`IntentParser::classify_writing_intent`（intent.rs）**：一次 LLM 调用产出 `WritingIntentClassification`（is_new_novel / is_continuation / task_type / is_prose_request / input_clarity / detected_genre / confidence）。最快模型 + 8s 超时 + 保守兜底（is_new_novel=false=续写）+ 会话 LRU 缓存。误判代价不对称：误判续写为创世会启动 Agency 全流程覆盖工作（灾难），故默认偏向续写。
- **Site 4**：`smart_execute` 用 `classification.is_new_novel` 替代 `is_novel_creation_intent`；前端 `classify_intent` IPC 先行，payload 透传分类，后端信任不重复调用。
- **Site 1**：`find_template` 禁用（恒返回 None，patterns 来自 LLM understanding 切词噪声）；`find_match` 标 `#[allow(dead_code)]`。
- **Site 4b/5**：TriShot 守卫 + 续写绕过 + force-correction 读 `PlanContext.intent_classification`（无新 LLM 调用）。
- **Site 3**：`from_instruction_and_context` 修运算符优先级 bug + 移除单字 pattern + `hint` 参数经 `task.parameters["task_type_hint"]` 透传。
- **Site 8**：`build_writer_prompt` 题材优先 LLM `detected_genre` > `extract_genre`（加否定窗口 + 长度降序）> 故事 genre。
- **Site 7**：`detect_input_clarity` 移除单字信号；调用方读 `classification.input_clarity`。
- **Site 2**：`intention_graph::builder` LLM 主路径硬化（JSON 子串截取 + raw_input 推断）+ 规则兜底默认 `generate prose`。
- **前端**：新增 `classifyIntent` API；`handleSmartGeneration` 入口调分类（缓存 + 兜底）；删除 `isNovelCreationIntent`/`isContinuationIntent`。
- **字段名 bug**：prompt 指示返回 `"is_prose"` 但 struct 字段 `is_prose_request` 无 alias 致恒 false；加 `#[serde(alias = "is_prose")]` 修复（单测捕获）。
- **不适用 LLM（诚实标注）**：Site 9 `derive_model_role_from_label`（内部 label）、Site 10 `discover_from_outputs`（LLM 输出，需结构化 findings 改造）保留为后续。
- **验证**：`cargo test --lib` 936 passed（+1）；`npx vitest run` 305 passed；tsc / fmt / format:check / clippy / architecture_guard 全绿。
### v0.30.10 - 续写返回风格增强模板修复（模板匹配误路由 + content 空兜底）

- **根因**：`PlanTemplateLibrary::find_match` 用朴素 substring 匹配（`user_input.contains(pattern)`），之前记录的 style_enhancer 计划的触发词（如"这部小说"）会匹配"继续写当前这部小说"，导致续写请求**跳过 planner LLM 和所有安全规则**，直接重放 style_enhancer 计划。style_enhancer 收到空 content 后返回"在您提供文本后，我将从以下几个方面进行增强"模板而非续写正文。
- **Fix A（executor.rs 主修复）**：`execute_with_context` 在 `find_template` 前检测续写意图词（继续/续写/接着写/往下写/接下来/后续/接着），命中则跳过模板匹配，强制走 planner LLM 路径，确保续写请求由 Rules 8/19/21 正确路由到 writer。
- **Fix B（mod.rs 防线 2 扩展）**：force-correction 从仅捕获 `outline_planner` 扩展到 `style_mimic` / `plot_analyzer` / `builtin.style_enhancer` / `builtin.text_formatter` / `builtin.character_voice` / `builtin.emotion_pacing`，当首步为这些 capability 且输入含写作/续写关键词时强制改为 `writer`。
- **Fix C（executor.rs content 兜底）**：新增 `inject_content_fallback` 静态方法，为 `style_mimic` / `plot_analyzer` / `builtin.*` 技能在 content 为空时按 depends_on -> step_outputs -> plan_context.current_content_preview 顺序注入文本，与 v0.30.9 inspector draft 兜底同理。
- **Fix D（mod.rs Rule 21 强化）**：Rule 21 新增"继续"/"续写"关键词和"这部"/"当前"故事相关主语，并明确禁止 `style_mimic` / `plot_analyzer` / `builtin.style_enhancer` 用于 prose 请求。
- **验证**：`cargo test --lib` 929 passed（+5：content 兜底注入 5 场景）；fmt / clippy 无新增告警。
### v0.30.9 - 续写返回 Inspector 审查模板修复（draft 空内容兜底注入）

- **根因**：legacy planner 的 LLM 生成的 ExecutionPlan 中 inspector 步骤常遗漏 `"draft": "{{step_N}}"` 参数。`execute_inspector` 仅从 `params["draft"]` 读取待检查正文，缺失时 `task.input` 为空串，`build_inspector_prompt` 渲染出"【待检查内容】部分为空"的模板文本，Inspector 直接将该模板作为"审查结果"返回，用户看到审查模板而非续写正文。
- **Fix A（主修复·executor.rs）**：`resolved_params` 块新增 inspector draft 兜底注入--当 `capability_id == "inspector"` 且 `draft` 为空时，按 `depends_on` 顺序查找 writer 步骤的 `step_outputs["content"]`，找不到则扫描全部 `step_outputs`，自动注入非空 content 作为 `draft`。提取为可测静态方法 `inject_inspector_draft_fallback`。
- **Fix B（提示词·mod.rs）**：planner 提示词 Rule 9 强化--明确要求 inspector 必须使用 `"draft": "{{step_id}}"` 传参，否则 inspector 收到空内容只返回请求输入的模板；JSON 示例增加 inspector 步骤示范 `"draft": "{{step_1}}"` + `depends_on: ["step_1"]`。
- **验证**：`cargo test --lib` 924 passed（+5：inspector draft 兜底注入 5 场景）；fmt / clippy 无新增告警。
### v0.30.8 - 全面修复 nullable 列读取（Invalid column type Null 系列）

- **根因**：`world_buildings.cultures`（index 5）和 `rules`（index 3）在基础 schema 为 nullable TEXT，旧数据该列为 NULL，repository 用 `row.get(N)?` 读非空 `String` 即报 `Invalid column type Null`。与 v0.30.6 `dynamic_traits` NULL 同类。
- **全面排查**：系统性审查全部 27 个 repository 文件，发现并修复所有 nullable 列被当作非空 `String` 读取的问题（共 8 个文件、31 处）：`world_building_repository`（cultures/rules）、`scene_repository`（characters_present/character_conflicts × 4 方法）、`scene_version_repository`（同上 × 2 方法）、`studio_config_repository`（llm_config/ui_config/agent_bots）、`writing_style_repository`（custom_rules）、`knowledge_graph_repository`（attributes × 4 / evidence × 2）、`user_preference_repository`（6 列 × 2 方法）。全部改为 `Option<String>` + `unwrap_or_default`/`unwrap_or_else` 兜底。
- **迁移**：V112 回填 `world_buildings.cultures/rules`；V113 全面回填 scenes/scene_versions/studio_configs/writing_styles/kg_entities/kg_relations/user_preferences 的所有 nullable JSON/TEXT 列。
- **验证**：`cargo test --lib` 919 passed（+2：world_buildings NULL 兜底 + 合法 JSON 解析）；fmt / architecture_guard 全绿。
### v0.30.7 - 计划执行失败修复（LLM 在 depends_on 写入上下文名）

- **根因**：LLM 生成的 ExecutionPlan 在 `depends_on` 中混入上下文名（如 `"Story Context"`、`"writer"`）而非 plan 内 step_id。`topological_sort`（swarm.rs）已正确跳过非 step_id 依赖，但 `PlanExecutor::execute` 的依赖校验未对齐--遇到非 step_id 依赖直接判 `not found`，导致 step_1 被跳过 -> step_2 依赖 step_1 也 not found -> step_3 链式失败，整 plan 崩溃。
- **Fix（executor.rs）**：依赖校验前收集 `plan_step_ids` 集合，对不在集合中的依赖（非 step_id）跳过校验并 `log::warn`，与 `topological_sort` 行为一致；仅校验真实 step_id 依赖是否已产出。参数引用 `{{step_id}}` 由 `resolve_parameters` 兜底处理缺失键。
- **Fix（mod.rs）**：Rule 3 强化--明确 `depends_on` MUST ONLY contain step_id values of OTHER steps in this same plan，NEVER put context names / capability names / free text，并举例 `"Story Context"` / `"writer"` 为错误值。
- **验证**：`cargo test --lib` 917 passed（+2：topological_sort 非 step_id 依赖跳过 + 混合依赖排序）；fmt / tsc / architecture_guard 全绿。
### v0.30.6 - 获取角色失败修复（dynamic_traits 列 NULL）

- **根因**：`characters.dynamic_traits` 列在基础 schema 为 nullable TEXT（无 `NOT NULL`/`DEFAULT`），StoryForge 数据迁移导入的旧角色行该列为 NULL。`get_by_story`/`get_by_id` 用 `row.get::<_, String>(9)` 读非空类型，遇 NULL 即报 `Invalid column type Null at index: 9, name: dynamic_traits`，续写/创世获取角色失败弹 Fatal 诊断卡片。
- **修复（双层）**：读取层 `get_by_story`/`get_by_id` 改读 `Option<String>` 兜底 `"[]"`（NULL 行返回空 `dynamic_traits`）；数据层 V111 迁移回填 `characters.dynamic_traits NULL -> '[]'` 保证一致。
- **验证**：`cargo test --lib` 915 passed（+2：NULL 兜底 + 合法 JSON 解析回归）；fmt/clippy 无新增告警。
### v0.30.4 - 幕前输入历史持久化（按故事隔离）

- **功能**：幕前底部输入框已输入内容现长久保留，关闭窗口/重启后不丢失，与编码工具一致。每条提交按故事 ID 隔离存入 `localStorage`（`frontstage:inputHistory:<storyId>`，最近 20 条），切换故事自动加载该故事的历史。
- **UX**：保留既有 ghost-hint 交互（↑/↓ 切换 LLM 建议 <-> 历史记录，-> 确认填充），持久化对导航无侵入。localStorage 不可用时静默降级为内存态。
- **实现**：`src-frontend/src/frontstage/FrontstageApp.tsx`（模块级 `loadInputHistory`/`saveInputHistory` + `useEffect` 加载 + `handleInputSubmit` 同步持久化）。
- **验证**：`npx vitest run` 297 passed（+2：持久化写入 + 重载召回）；tsc / prettier 通过。纯前端，无 Rust 变更。
### v0.30.5 - 创世流程严重超时修复（600s 顶满 + 前端先杀后端）

- **根因（对照 `creative_workflow.log` 2026-07-20 08:37–08:47）**：Agency 创世 5 阶段慢，producer tool_loop 5.5min + writer tool_loop 4.5min（含本地模型连接超时 60s×4 候选=240s）顶满 600s；前端 `Promise.race` 600s 到了先 `llm_cancel_all_generations` 杀掉后端，创世被 CANCELLATION 砍掉无产出 + 僵尸 run 卡死故事续写；writer 在 tool_loop 中盲目 board_read 轮询 7-10 轮。
- **Fix 1**：`config/commands.rs` 放开 `smart_execute_total_timeout_secs` / `frontend_timeout_secs` clamp 上限 600->1800（默认仍 600s）；`GeneralSettings.tsx` 输入框 max 同步到 1800。
- **Fix 2**：`FrontstageApp.tsx` 创世路径前端超时 = 后端 + 30s 缓冲（主超时 + 看门狗 + 诊断卡片三处统一）；提取纯函数 `utils/genesisTimeout.ts`。
- **Fix 3（核心）**：`coordinator.rs` 新增 `asset_retrieval_plan`--writer 前置单次 LLM 调用从资产 catalog 选出必需 key（30s 超时 + 失败兜底全量 + `RetrievalPlan` 别名兼容），消除 writer 多轮 board_read 轮询。
- **Fix 4**：`coordinator.rs` 新增 `build_writer_assets_context`--检索规划后按 key 过滤资产全文预注入 writer task（8000 字符预算截断），tool_loop 轮次从 7-10 降到 1-2。
- **Fix 5**：`tool_loop.rs` 新增 run 级 deadline 感知（`with_deadline` + 每轮检查，剩余 <30s 熔断保产出）；新增 `LoopAbortReason` 枚举，`circuit_break_reason` 识别 deadline 熔断返回"剩余时间不足"，coordinator writer 路径据此快速失败而非回退 legacy（避免 legacy 又跑一遍超时）。
- **验证**：`cargo test --lib` 913 passed（+14）；`npx vitest run` 305 passed（+8：genesisTimeout 纯函数）；tsc / fmt / clippy / format:check / architecture_guard 全绿。
### v0.30.3 - 创世主创 Agent 熔断修复（本地模型 JSON 不遵从）

- **根因**：本地模型（Qwen/Gemma）对 `producer_depth_assets` 的 `complete_json` 返回散文而非 JSON -> 快速路径失败回退 legacy -> legacy writer tool_loop 要求 JSON action 而模型写散文 -> 连续 3 轮解析失败熔断，首章未完成。
- **Fix A（主修复）**：`producer_depth_assets` 在 `parse_lenient` 失败时兜底 salvage 散文为 world 资产，快速路径继续，避免回退 legacy。
- **Fix B（可诊断性）**：`tool_loop.rs` 此前零条日志，解析失败 raw 响应只存在内存 run 结束即丢弃。现每轮解析失败 + 熔断点 + max-turns 均 `log::warn!`（含 role、轮次、截断 raw 500 字）。
- **Fix C（纵深防御）**：legacy writer "连续解析失败"熔断时回退自由体散文单调用（新 `writer_prose_fallback`），"达到最大轮数"仍直接 Err。
- **验证**：`cargo test --lib` 899 passed（+2 新测试）；fmt/clippy 通过。
### Agency 多代理创作框架 P1 — 创世 2.0 骨架（串行端到端）

- **新模块**：`src-tauri/src/agency/`（board 黑板 / tool_loop ReAct 工具循环 / roles 三角色 / coordinator 协调器（P2 起含并行稳态循环 gate(n-1)∥writer(n)）/ repository+models 持久化 / bus 消息总线（P2 已接线：修订提案 proposal）/ budget 角色预算 / commands IPC）。
- **IPC**：`agency_start_genesis` / `agency_get_run` / `agency_list_board` / `agency_cancel_run` / `agency_continue_chapter` / `agency_continue_batch`。
- **提示词目录**：`resources/prompts/agency/`。
- **依赖边界**：agency 允许依赖 db/llm/router/prompts，不允许被反向依赖。
- **教训**：迁移文件必须与引用它的代码同一 commit 提交（P3 T5 教训：V109 与测试被并行 CI 提交拆散导致断档）。
- 设计：`docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（P1-P3 已完成，除真机验收外）。
### v0.26.59 — StoryForge → StoryMoss 品牌收尾，官网落地页上线

- **品牌重命名**：完成仓库文档、CI、Tauri 配置与 GitHub Release 的 StoryForge → StoryMoss 全局替换。
- **官网落地页**：`landing/` 站点部署至 `https://ai.91z.net`，重写产品介绍并加入 Logo；下载按钮按平台自动匹配安装包。
- **验证**：landing 19 tests passed。
### v0.26.58 — 修复 OpenAI/Deepseek 模型因 top_p=0 健康检测失败

- **根因**：OpenAI 兼容 API（含 Deepseek）不接受 `top_p = 0.0`，会返回 `Invalid top_p value`。
- **修复**：`OpenAiAdapter` 在序列化前过滤 `top_p`，仅保留 `(0, 1.0]` 的合法值；非法值自动省略，让服务端使用默认值。
- **验证**：新增 `llm::openai` 单元测试；`cargo test --lib` 770 passed。
### v0.26.57 — 自动划分章节、本地导出保存与提示词目录

- **自动划分章节**：后台设置新增「按字数 / 按情节」分章策略；字数上限留空默认 3000 字；场景保存空闲 30s 后仅对最新章自动切分。
- **本地导出保存**：导出结果通过系统保存对话框落盘；文本写 UTF-8，二进制（pdf/epub）复制后端临时文件；取消时不关闭弹窗。
- **提示词目录**：提示词注册表新增「打开目录」按钮，直接用系统文件管理器打开 bundled prompts 目录；编辑器改用原生 textarea 避免 CSP 导致 Loading。
- **验证**：`cargo test --lib` 769 passed；`npx vitest run` 292 passed；tsc / fmt / format:check 全绿。
### v0.26.56 — 网关契约测试串行化（mock app_data_dir）

- **修复**：写 AppConfig 的 executor 契约测试加进程锁，避免并行污染导致 `creative_x_overrides` 偶发失败。
- **验证**：`creative_x_overrides` / `demoted_degraded` / `sticky_unhealthy` / `disabled_model` 并行 `--test-threads=8` 通过。
### v0.26.55 — 幕后模型列表开启/关闭开关

- **UI**：模型卡片「开启/关闭」；仅轮询已启用模型。
- **运行时**：复用 v0.26.54 fail-closed；`is_promotable` 要求仍在注册表。
- **验证**：ModelCard vitest + disable/probe Rust 契约。
### v0.26.54 — 修复创作模型被粘性降级绕过

- **根因**：Deepseek 已是创作/活跃，但连续失败 demotion 让 `resolve_role_model` 丢弃显式角色，Call3 长期用 MN-Oblivion。
- **修复**：显式角色不受粘性 demotion；Unhealthy 在 resolve 清一次再探；`set_active_model`/`save_settings` 清 demotion；`generate()` 用 `is_promotable`；禁用模型 fail-closed（持久化 enabled、不探测、活跃自动回退）。
- **验证**：gateway/health/commands 契约通过；architecture_guard。
### v0.26.53 — 故事名取消单击回幕后（双击改名可用）

- **修复**：故事名不再单击打开幕后（与双击改名冲突）；回幕后走设置按钮（禅模式也保留）。
- **验证**：Header 单击不调 `onOpenBackstage`；设置按钮可回幕后；双击仍进编辑。
### v0.26.52 — 修复模型新增与默认创作模型即时生效

- **幕前连接状态**：`model_config`/`app_settings` 刷新同步失效 `gateway-status`；状态栏含 `Unknown`。
- **创作模型**：用户显式角色允许 Unknown 置顶；`set_active_model(creative)` / `save_settings` 同步 `active_llm_profile`。
- **验证**：Rust 4 + vitest 5；tsc/fmt/architecture_guard。
### v0.26.51 — 幕前故事名与章节名内联改名

- **故事名**：草苔/未命名展示；有正文自动建「未命名」故事；双击改名。
- **章节名**：编辑器上方 + 顶栏状态统一双击改名；空标题 `第N章`；`update_scene` 持久化。
- **验证**：displayStoryTitle/ChapterTitle + Header/EditableChapterTitle 相关测试；tsc/format/architecture_guard。
### v0.26.50 — 修复打字触发后台运行与深度思考假超时

- **AutoIngest 防抖**：打字自动保存不再立刻抢本地模型（30s + BACKGROUND_LLM_SEMAPHORE）。
- **合同补齐静默**：不再用 `contract-auto-progress` 拉高 `isGenerating`。
- **活动同步**：后台活动不得单独禁用输入栏；`isGenerating` 超时看门狗强制弹诊断。
- **验证**：scene_service 6；contract gate 2。
### v0.26.49 — 修复续写与正文脱节（末句硬锚点）

- Call3/TimeSliced 在 prompt 最末尾注入末 2 句硬锚点，覆盖「开场」类大纲指令；抗 Lost-in-the-Middle。
- **验证**：ending_anchor 相关 3 passed。
### v0.26.48 — 修复自动更新（GitHub Releases + latest.json）

- 开启 `createUpdaterArtifacts`；CI 产出签名更新包与 `latest.json`；Linux AppImage；下载进度累加与 404 提示。
- **验证**：`cargo test --lib updater::` 2 passed。
### v0.26.47 — CI 热修复（Rust fmt）

- `cargo +nightly fmt` 修复 v0.26.46 rust-check 失败；无逻辑变更。
### v0.26.46 — 创世方法论全链路、题材 match-or-create 与拆书持久化

- **方法论**：5 个 background 模板恢复 `strategy_section`；Genesis 分步注入 + `methodology_step` 推进；ID 归一化；Selector 预填 `recommended_methodology_id`。
- **题材**：`EnsureGenreProfileStep` match-or-create；概念保真硬化。
- **拆书**：StoryArc/作者/伏笔落库；分块超时与并发止血；前端按书过滤进度。
- **验证**：genesis/methodology/prompt 契约 20+ passed。
### v0.26.45 — Genesis 人物卡强制落地（姓名 + 欲望/阻力）

- **人物卡**：merge/render/probe 纯函数；first_scene + Call3 双重注入；真名≥80%、欲/阻信号探针；软重试 fail-open。
- **验证**：narrative 61；protagonist_card 6。
### v0.26.44 — Genesis 首章质量：开篇骨架与提示词加厚

- **开篇骨架**：quick_phase 四步（概念→策略→骨架→开篇）；10s 超时 fail-open；概念字段规则映射降级。
- **提示词**：概念加厚（主角/冲突/世界锚点）；strategy_selector 中文化；first_scene 纪律单源化。
- **四元组 + 占位角色**：Genesis 接入 `infer_narrative_quartet`；TriShot 占位用骨架主角，去掉「异星末世」硬编码。
- **验证**：`narrative::genesis` 12 passed；骨架解析契约 +1。
### v0.26.43 — 修复底部状态栏 emoji 显示为方框

- **根因**：阶段文案嵌入 emoji + 解析正则拆碎中文/代理对；WebView 缺字显示 □□。
- **修复**：纯文案 + `StatusIcon`（Lucide）；解析前剥 emoji。
- **验证**：StatusIcon / FrontstageBottomBar 相关 18 passed。
### v0.26.42 — 修复续写 Tab 提示可见但无幽灵文本

- **根因**：Tab 接受后 30s `hideGhostUntil` / `postAcceptHideUntilRef` 未在新续写时清零；幽灵树仍渲染 Tab 条，幽灵段落被压住。
- **修复**：续写入口与 `setGeneratedText` 清零父级锁；RichTextEditor 新幽灵到达时清零本地锁（接受中不解除）。
- **验证**：`RichTextEditor.duplicate.test.tsx` 6 passed（+1）。
### v0.26.41 — 记忆统一读模型与 Finalize scene_id 根治

- **Finalize**：`scene_id` 贯穿 drafts/IPC/UI；直写编辑场景。
- **记忆**：`story_memory_facts` VIEW + `kg_entity_id` 链接；`list_unified_facts`；表不 DROP。
- **验证**：cargo 701；facade 7；finalize 3；vitest 261。
### v0.26.40 — 幕后资产闭环 P0–P3

- **P0**：侧栏热/温/冷/配徽章；合同/KG 生成影响说明；诊断组默认折叠。
- **P1**：SceneEditor 管线轨；KG 摘要进 WriteTimeBundle；MCP→设置扩展；Wizard 幂等+KG（既有）。
- **P2**：MemoryFacade；quality_gate 永不热路径 LLM。
- **P3**：TracingPanel 资产→prompt 覆盖率。
### v0.26.39 — 幕后信息架构全面重排

- **侧栏五组**：创作 / 故事资产 / 创作工具 / 洞察与运维 / 系统；中文重命名。
- **数据洞察**：合并用量/写作/功能使用；设置七 Tab 重组；拆书设置就近；账号死链修复。
- **验证**：`npx vitest run` 249 passed / 3 skipped；tsc/format 通过。
### v0.26.38 — 提示词面板修复与组合智能化

- **修复 Loading / 打开目录 / 导出**：Monaco CDN → textarea；`open_prompts_directory` 原生打开；dialog+fs 导出覆盖/完整包。
- **接通 FrameworkSelections**：methodology + contextual_injectors 确定性回灌 Call 3（0 额外 LLM）。
- **场景组合预览**：`preview_prompt_composition` + 面板分层跳转。
- **验证**：`cargo test --lib` 690 passed；`npx vitest run` 244 passed / 3 skipped；fmt、format、architecture_guard 均通过。
### v0.26.37 — 修复幕前「保存中」常亮与字数不更新

- **根因**：`update_scene` IPC 参数形状错误 + `appendAiContent` 不刷新字数/不调度保存。
- **修复**：`buildUpdateSceneIpcArgs`；追加后同步 `wordCount` + `scheduleAutoSave`。
- **验证**：vitest 242 passed / 3 skipped。
### v0.26.36 — 后台配置变更即时生效（超时/字体/主题）

- **超时热重载**：`save_settings` → `reload_config` + `app_settings` sync；幕前立刻用新超时。
- **首字节超时**：`llm_first_chunk_timeout_secs` 接入三适配器。
- **字体/主题跨窗口**：Tauri 事件 `editor-config-changed` / `color-theme-changed`。
- **验证**：cargo test 685；vitest 240 passed / 3 skipped。
### v0.26.35 — 全面落地幕后工作室审计残留 R1–R11

- **R1**：`list_stories` 返回真实 `scene_count`；Dashboard「场景」统计对齐。
- **R2**：CreationPathGuide 快速创作绑定 `runCreationWorkflow`；`App` 导航统一 `appStore.currentView`。
- **R3**：后端 `apply_wizard_to_story`（角色去重、首场景 upsert、KG 摄取）；前端单 IPC。
- **R4**：幕后 `App`/`GenesisPanel` 监听 `genesis-warnings`。
- **R5/R6**：PipelinePanel / SceneEditor 标注场景序号语义。
- **R7–R11**：世界构建文风 Tab、UsageStats 启发式、伏笔 Kanban、角色→场景跳转、拆书转故事导航。
- **验证**：`cargo test --lib` 685 passed；`npx vitest run` 237 passed / 3 skipped；fmt、format、architecture_guard、tsc 均通过。
### v0.26.34 — 修复提示词导入参数并新增「打开本地目录」功能

- **修复批量导入静默失败**：`PromptsPanel.handleImportAll` 调用 `save_prompt_override` 时参数键由错误的 `promptId` 修正为 `prompt_id`，与后端 `rename_all = "snake_case"` 对齐。
- **新增「打开目录」功能**：后端新增 `get_prompts_directory` 命令暴露当前 prompts 资源目录；前端标题栏新增「打开目录」按钮，使用系统文件管理器打开目录。
- **新增「刷新」功能**：支持重新加载提示词列表与目录路径。
- **改善错误展示**：加载失败时在页面上方显示具体错误信息。
- **导出/导入按钮归位**：将「导出」「导入」按钮从「全部重置」确认弹窗移至页面标题栏。
- **验证**：`cargo test --lib` 685 passed；`npx vitest run` 237 passed / 3 skipped；fmt、format、architecture_guard 均通过。
### v0.26.24 — 修复续写重复、截断与跨内容复述（5 项根因）

对照 `creative_workflow.log` 2026-07-07 08:44–09:05 续写会话（新写 → 多次续写）：

- **散布式句子块重复**：新增 `trimInterspersedRepeatedBlocks`（Rust + TS 对齐，golden 双跑），处理单次生成内意象循环重复。
- **跨内容重叠复述**：新增 `stripExistingOverlap`，剥离 Writer 复述已有正文段落（`startsWith` / `isTextDuplicate` 无法拦截的路径）。
- **截断末句污染**：新增 `trimDanglingTail`，裁掉 60s 超时硬截断留下的极短半句。
- **续写 8% 重试闸门**：TriShot 续写路径补齐 anti-repeat 重试（对齐 Genesis）。
- **前端管线统一**：`sanitizeContinuationOutput` 覆盖 smart_execute / appendAiContent / handleRequestGeneration。
### v0.26.23 — 修复续写卡死与幽灵文本混乱（4 项根因）

对照 `creative_workflow.log` 2026-07-07 续写会话时间线定位 4 个根因：

- **Bug B（卡死主因）**：`auto_contract` 4 个 LLM label（master_setting/chapter/scene_outline/default_character）加入 `is_silent_background`，后台补齐合同不再阻塞 `isAnyBackendActive`（原 6 分钟阻塞续写）。
- **Bug D（混乱主因）**：`handleSmartGeneration` 入口加重入守卫——存在未接受幽灵时先丢弃并提示，避免新旧续写结果竞争。
- **Bug A**：`RichTextEditor` 新增 `bodyForceHideGhost` state 镜像 `force-hide-ghost` 类，移除类时触发重渲染，消除幽灵 10s 渲染延迟。
- **Bug C**：续写（非创世首章）call3 超时上限 120s→60s，慢模型 fail-fast 回退到快模型（Gemma4 10s vs MN-Oblivion 198s）。
### v0.26.21 — 修复 Windows MSI 构建（迁移文件名重命名）

- v0.26.17 起打包 `src/db/migrations/` 为 Tauri resource，但 24 个迁移文件名含中文/全角逗号/破折号且最长 102 字符，导致 WiX `light.exe` 标识符生成失败（v0.26.14/v0.26.16 resources 引入前 Windows MSI 曾成功）。
- 重命名 24 个迁移文件为 ASCII 短名（保留 `V###` 前缀与排序）。`schema_migrations` 按 version 跟踪，已应用迁移不受影响；`parse_filename` 仅解析 `V###` 前缀，无逻辑变更。
- v0.26.20 尝试的 `wix.language: zh-CN` 无效（问题在标识符生成而非代码页）。
### v0.26.20 — 修复 v0.26.19 CI 格式检查失败

- `ParallelWorldOutlineCharacterStep` doc 注释超 `max_width=100`，`cargo +nightly fmt` 自动换行。仅注释格式变更。
- macOS 公证随 Apple Developer 协议续签恢复成功。
### v0.26.19 — Genesis 创世流程全面审计与测试加固

对照项目文档对「智能创作流程-创世」全面审计，分 Phase 1–4 执行：

- **Phase 1（P0 竞态与契约）**：
  - **Gap B**：`isFirstChapterReady` 路径在 `finalContent` 为空时不锁 `delivered`，避免编辑器永久空白。
  - **P0-2 角色世界观上下文**：`ParallelWorldOutlineCharacterStep` 中 character 提示词读取 `bundle.world_building` 恒为空（闭包捕获竞态），改为先 await world 拿真实 `world_concept` 再构造 character；提取 `world_concept_for_character_prompt` 纯函数 + 单测。
  - **P0-3 ChapterSwitch delivered 时序**：`selectChapter` 懒加载失败时不标记 `delivered`（`markDeliveredOnLoad` 仅在 `setContent` 成功后标记）。
- **Phase 2（P1 架构对齐）**：后台错误可观测性（`GenesisContext.errors` → `genesis_runs.steps_json` + `genesis-warnings` 事件 → 前端 toast）；mutex 中毒锁加固；策略移入 quick phase 经评估暂缓（记录为债务）；`window/mod.rs` 与 `FrontstageEvent.ts` 注释对齐 auto-accept 真实路径。
- **Phase 3（测试加固）**：8% 重试闸门 + ChapterSwitch payload 提取纯函数 + 契约测试；前端 Gap C + 状态机端点测试；**跨层共享 trim golden fixture**（`tests/fixtures/trim_golden.json`，Rust + TS 双跑锁定 `trim_self_repetition` 跨层一致性）。
- **Phase 4（代码整洁）**：`*_future` → `*_gen` 重命名；`AppConfig::load` 去重；`appendAiContent` skip 路径不 `markAccepted`；Gap C 重复入站也跳过 setContent；`isGenesisSettingUpRef` 合并评估——不合并（覆盖窗口不同）。
- **验证**：`cargo test --lib` 655 passed（+10）；`npx vitest run` 183 passed（+17）；`npx tsc --noEmit` 零错误；fmt 通过。
### v0.26.18 — Genesis 第一章重复：竞态路径加固

- **Gap A**：ChapterSwitch `auto_accept=true` 但 content 为空时 `skipContent=true` 且不标记 `delivered`，让 smart_execute 投递。
- **Gap B**：`isFirstChapterReady` 路径仅在已 append 或编辑器已有内容时标记 `delivered`。
- **Gap C**：`selectChapter` 咽喉点新增 `delivered` + 编辑器已有内容守卫，跳过 `setContent`。
- **回归测试**：新增 Gap A 竞态路径单测，vitest 167 passed。
### v0.26.17 — Issue #4 启动加固：打包 SQL 迁移与 init_db 诊断增强

- **打包 SQL 迁移**：Release 安装包包含 `$RESOURCE/db/migrations/`。
- **init_db 加固**：启动前确保 app data 目录；失败日志含 DB 路径；新增 fresh init 回归测试。
### v0.26.16 — 根治 Genesis 第一章重复、Issue #4 启动稳定性与代码格式修复

- **根治 Genesis 第一章内容重复**：替代 v0.26.7–v0.26.14 的散布布尔守卫补丁模式，从两个独立根因进行结构性修复。
  - **R2 生成侧验证闸门（`src-tauri/src/narrative/genesis.rs`）**：检测 LLM 输出自重复比例，≥8% 时用更强 anti-repeat 指令重试一次；prompt 模板新增「结构纪律」段，明确禁止首尾回环与整章重复。
  - **R1 前端单写者状态机（`src-frontend/src/frontstage/FrontstageApp.tsx`）**：将 `genesisAutoAcceptedRef` 布尔替换为 `idle → generating → delivered` 三态状态机；`generating` 态阻塞 `onChapterUpdated` 与 `loadStories` 自动选择；`delivered` 态阻塞 `setGeneratedText` 幽灵文本恢复。
  - `textCleanup` 提升到 `src-frontend/src/utils` 共享；Rust `trim_self_repetition` 对齐前端 KMP 最长 border 检测；全路径统一调用 `trimSelfRepetition`。
- **修复 Issue #4：init_db 失败时启动 panic/Windows 闪退**：`GatewayExecutor::new` 改为显式接收 `pool`，`setup` 仅在 pool 可用时初始化网关执行器，避免 `state::<DbPool>()` 在启动时 panic；新增不可写目录回归测试。
- **修复 CI 格式检查失败**：`cargo +nightly fmt -- --check` 与 `npm run format:check` 现已通过。
### v0.26.14 — 修复 Genesis 第一章模型输出自重复与幕前诊断日志过载

- **修复 v0.26.13 仍被用户感知的「新写小说第一章内容重复」**：通过分析 `creative_workflow.log` 中 13:43 的完整链路，确认前端 `append_ai_done` 只触发一次、`append_text_check.occurrences=1`，**不是前端把内容追加了两次**；重复来自 LLM 生成的 613 字正文自身首尾段落重复。
- 新增 `trimSelfRepetition` 工具（`src/frontstage/utils/trimSelfRepetition.ts`）：
  - 段落级：检测「后半段 == 前半段」或「末段 == 首段」并裁剪。
  - 字符级：对归一化文本做 KMP 最长 border 检测，保守阈值（重复长度 ≥30 字符且 ≥ 全文 8%）下裁掉尾部重复前缀。
- 在 `FrontstageApp` 的 `appendAiContent` 以及 `smart_execute` 返回的 `finalContent` 进入编辑器/幽灵文本前调用自重复清理，覆盖 Genesis 自动接受、Tab 接受、ContentUpdate/AppendContent 等全部路径。
- **缓解「写完后过会儿页面崩溃」**：`RichTextEditor` 的 `frontstage:rich_editor_diag` 渲染诊断日志从每帧都记改为仅前 20 次渲染 + 幽灵文本/隐藏锁状态变化时记录，并将 IPC 日志节流从 50ms 收紧到 200ms，降低长时间写作或文思活跃模式下的 IPC 与日志压力。
- 新增 `trimSelfRepetition` 单元测试，覆盖首尾段落重复、整章重复、单段内 suffix 重复、短文本不裁剪等场景。
### v0.26.13 — 修复 Genesis 第一章渲染层视觉重复（幽灵容器残留）

- 修复 v0.26.12 仍偶发的「新写小说第一章内容重复」视觉问题：数据层只写一次，重复来自渲染层幽灵文本/空幽灵容器与正文同框。
- `RichTextEditor` 的 `shouldShowGhostTree` 改为仅在 `generatedText` 非空时渲染，避免生成中状态的空幽灵容器残留或复用旧内容。
- `FrontstageApp` Genesis 自动接受路径先 `setIsGenerating(false)`，确保幽灵树条件立即失效。
- 增强 `frontstage:rich_editor_diag` 诊断字段：`isGenerating`、`isHidingGhost`、`bodyHidingGhost`、`generatedTextLen`。
- 增强 Playwright E2E 回归测试，新增自动接受后 `ghost-paragraph` 必须隐藏的断言。
### v0.26.12 — 修复角色列表为空/未加载时的幕前崩溃与订阅状态空值

- 修复 `useCharacters` 返回 `null` 或未加载完成时，`RichTextEditor`「角色名点击」effect 访问 `characters.length` 导致白屏崩溃的问题。
- `useSubscription` 增加空值防护，避免 `getSubscriptionStatus()` 返回 `null` 时产生错误日志。
- 新增 Playwright E2E 回归测试 `e2e/genesis-duplicate.spec.ts`，覆盖「已有故事 + 新写末世小说」完整流程。
- `frontstage/main.tsx` 与 `ErrorBoundary` 增强崩溃诊断输出。
### v0.26.11 — 修复 Genesis 第一章 store-editor 失步与崩溃隐患

- 修复 Genesis 自动接受第一章后，store 依赖 200ms onChange debounce 回写导致的 store-editor 失步。
- `appendAiContent` 追加后立即用 `editorRef.getHTML()` 同步 store 与 `latestContentRef`。
- `RichTextEditor.appendText` 空文档分支标记外部同步并更新 `lastExternalContentRef`，防止 content prop 被再次 setContent。
- `RichTextEditorRef` 新增 `getHTML()` 方法。
- 确认 `tauri.conf.json` `devUrl` 指向 dev server，避免开发时加载陈旧 dist 崩溃。
### v0.26.10 — 强化 Genesis 第一章重复防护（双重基准与追加最终防线）

- 修复 v0.26.9 单一 `latestContentRef` 基准与编辑器 DOM 短暂失步时，重复检测仍可能失效的问题。
- `isTextAlreadyInEditor`、`appendAiContent` 采用 `latestContentRef` + `editorRef.getText()` 双重基准。
- `appendAiContent` 增加正文前缀剥离安全网，并在追加后用 DOM 文本校准 `latestContentRef`。
- `RichTextEditor.appendText` 增加最终防线：编辑器尾部已包含待追加内容则直接跳过。
### v0.26.9 — 根治 Genesis 第一章重复（DOM 竞态与追加去重）

- 修复 TipTap DOM 状态滞后于 React `content` prop 时，重复检测依赖 `editorRef.getText()` 导致失效的问题。
- `isTextAlreadyInEditor`、`handleRequestGeneration`、`handleSmartGeneration`、`appendAiContent` 统一改用 `latestContentRef` 作为内容基准。
- `appendAiContent` 追加后立即同步 `latestContentRef`，避免 onChange debounce 窗口期内重复追加。
- `RichTextEditor` 幽灵文本直接包含检测剥离 HTML 标签，覆盖 ContentUpdate/AppendContent 路径。
- 新增 DOM 滞后竞态单元测试。
### v0.26.8 — 彻底修复 Genesis 第一章重复（竞态路径覆盖）

- 修复 `genesisAutoAcceptedRef` 无法覆盖 pipeline-complete 先加载 DB 正文竞态的问题。
- 新增 `isTextDuplicate` 归一化去重工具与 `isTextAlreadyInEditor` helper。
- `handleRequestGeneration` / `handleSmartGeneration` 设置幽灵文本前检测编辑器是否已包含生成内容。
- `pipeline-complete` 加载正文后标记 Genesis 已自动接受。

---

_最后更新: 2026-08-29 - v0.58.0_

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **StoryMoss** (22262 symbols, 46734 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

  - **v0.23.66 模型角色分配 × 后台并发根治** (2026-06-28) — 两层修复解决模型过载与前端页面崩溃问题。核心变更：
    - **模型角色分配**：新增三种模型角色——创作模型（正文生成/Writer/改写）、工具模型（Call 1 路由/探测/JSON 提取）、后台任务模型（BGP 审计/入库/洞察/Genesis 后台流水线）。`AppConfig` +3 字段 + `GatewayRequest.model_role` + `resolve_role_model` 解析方法。网关 `select_candidates`/`select_fastest_profile` 按角色选择对应模型并强制置顶；未设置时自动分配（快→工具，闲置→后台，创作回退 active）。前端 `UnifiedModelManager` 顶部新增「模型角色分配」卡片（三个下拉框 + "自动分配"选项），`ModelCard` 显示角色徽章（琥珀=创作/蓝=工具/紫=后台）
    - **后台并发过载根治**：`ParallelWorldOutlineCharacterStep` 从 `tokio::join!` 3 路并发改为串行 `.await` + `BACKGROUND_LLM_SEMAPHORE` 保护；BGP-4 `run_insight` 前加信号量；Genesis 后台 spawn 入口加信号量。任何时刻最多 1 个后台 LLM 调用，根治单模型过载 → `INTERNAL_ERROR` 洪流 → 前端崩溃的链路
    - 验证：`cargo test --lib` **582 passed / 0 failed / 2 ignored**；`cargo check` ✓；`npx tsc --noEmit` ✓；`cargo +nightly fmt -- --check` ✓

  - **v0.23.65 提示词工程全链路修复** (2026-06-27) — 对提示词工程进行深度审计，修复 80+ 高质量提示词在默认续写路径（TimeSliced）被系统性旁路的问题，全部改动零新增 LLM 调用。核心变更：
    - **P0-1/P0-2：`writer_system` 全链透传**。`writer_system`（7 条写作准则）此前仅在 Full 路径（`build_writer_prompt`）生效，TriShot Call 3 和 TimeSliced（默认续写）完全旁路。现在从 orchestrator → `generate_for_task*` → `GatewayRequest` → `execute_generation` → `GenerateRequest` → 适配器全链路透传 `system_prompt`。`GatewayRequest` +`system_prompt` 字段；`execute_generation` 三级优先级（每模型 > adapter 默认 > `writer_system` 注册表默认）；Ollama 适配器 `system_prompt` 前置拼接
    - **P0-3：选中资产正文回灌**。Call 1 返回 `selected_asset_ids`（桥段卡/引擎/高压关系等），此前只被转为路由标签用于模型网关，资产内容（`function`/`when_to_use`/`remix_hint`/`avoid`/`core_payoff`）从未到达 Writer。新增 `render_selected_asset_guidance`——从 `AssetCapabilityManifest` 回查资产完整内容，格式化为紧凑创作指导文本，注入 TriShot Call 3 的 system_prompt（限 5 条，控制 token 预算）
    - **P1-2：LivingAuthorGuard 注入所有路径**。`render_writer_system_from_bundle` 内置在世作者名清除 + 手工艺滑块（5 维 × 3 档），此前只在 Full 路径 `build_writer_prompt` 生效
    - **P1-3：Anti-AI cliché 避免指令**。27 个 AI 高频陈词滥调（不言而喻、总而言之、嘴角微微上扬…）注入所有 Writer 路径的 system_prompt
    - **P2-1/P2-5：`orchestrator_timesliced_writer` 模板充实 + `continuation` 变量声明补全**。模板新增 `{{continuation}}` 变量使用；注册表补全 `continuation` 变量声明（此前代码注入了但注册表未声明，前端编辑器不可见）
    - 验证：`cargo test --lib` **582 passed / 0 failed / 2 ignored**；`cargo check` ✓；`npx tsc --noEmit` ✓；`cargo +nightly fmt -- --check` ✓

  - **v0.23.64 续写内容质量根因修复** (2026-06-27) — 用户续写得到完全不可用的小说（4 个不同故事拼接 + 每个重复两次 + 规划 markdown 泄漏）。三路并行调查确认 3 个根因。核心变更：
    - **P0 根因：Writer LLM 看不到前文正文**。TimeSliced（默认续写）`execute_time_sliced` 构建 prompt 时完全不读取 `task.parameters["current_content"]`，Writer 只收到 `WriteTimeBundle` 结构化约束 → 每次续写生成全新故事。TriShot 路径的 `current_content` 截断 6000→600 字只给 Call 1 做意图检测，Call 3 Writer 看不到任何原始正文
    - **P0 修复**：新增 `build_continuation_context` 函数（结合记忆系统）——从 `scene_commits.summary_text` 读取近 3 章摘要（每章 300 字）+ 当前章尾部 2000 字预览，注入 TimeSliced 和 TriShot（非创世第一章）prompt。`MemoryWriter::extract_summary` 从前缀 200 字改为尾部 300 字（续写需要"最近发生了什么"而非开头）
    - **P1 根因：`sanitize_novel_output` 清洗不足**。推理模型在正文后输出 `<thinking>...</thinking>` 思考链 + `+++++` 文件分隔符 + 编号规划块（`1. 世界观设定`等）+ markdown 代码块，`sanitize_novel_output` 均未剥离 → 规划 markdown 泄漏到正文
    - **P1 修复**：`sanitize_novel_output` 新增第 0 步清洗（调用 `strip_reasoning_blocks` 剥离思考链 + `+++++` 分隔符截断 + 编号规划块连续 3 行检测截断 + markdown 代码块剥离）。`strip_reasoning_blocks` 从 `narrative/mod.rs` 改为 `pub(crate)` 复用
    - **P2 根因：前端幽灵文本与编辑器内容同时渲染**。`AppendContent` 事件 `setContent(prev => prev + formatted)` 无去重守卫 → 同一正文可被拼接两次。bootstrap 回退路径 `setContent(autoFormatText(final_content))` 不清空 `generatedText` → 同一文本同时作为幽灵（纯文本）和编辑器内容（HTML）
    - **P2 修复**：`AppendContent` 追加前检查 formatted 尾部 200 字是否已存在于 prev 中；bootstrap 回退路径两个 `setContent` 前加 `setGeneratedText('')`
    - 验证：`cargo test --lib` **578 passed / 0 failed / 2 ignored**；`cargo check` ✓；`npx tsc --noEmit` ✓

  - **v0.23.63 byte-slice panic 根治 + 探测日志静默化** (2026-06-27) — 解决创世静默卡死 600s 超时 + 日志洪流两个问题。核心变更：
    - **根因（日志确认）**：`trishot.call3.done`（Call 3 作家模型生成完成）正常发射，但第二个 `trishot.call3.done`（Call 3 完成，正文已生成）从未出现。两者之间的 `&content[..content.len().min(120)]` 在多字节 UTF-8 边界 **panic**（Rust 字符串字节索引必须落在 char 边界），`execute_trishot` tokio task 静默崩溃，`smart_execute` 600s 超时
    - **P0 修复**：全局 9 处 `&str[..str.len().min(N)]` 字节切片改为 `str.chars().take(N).collect::<String>()`，根治 UTF-8 边界 panic。覆盖 orchestrator/genesis/audit_executor/analyzer/migrations/service
    - **P1 探测日志静默化**：keepalive 探测每 10s 调用 3 个模型，虽然 `is_silent_background=true` 跳过了 emit_llm_progress，但 `workflow_log`（llm.generate.start/completed/heartbeat/record_call/emit_completed/return_ok）和 `record_llm_call` 仍在执行 → 27361 行日志中 17832 行是 probe 噪声。现在 `is_silent_background=true` 时跳过全部 workflow_log 和 record_llm_call
    - 验证：`cargo test --lib` **578 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` ✓

  - **v0.23.61 系统提示词可配置 + 第一章注册表化 + 框架级智能路由** (2026-06-27) — 三个缺口全面修复。核心变更：
    - **Gap 1 第一章注册表化**：`genesis.rs` 硬编码 `format!()` 改为 `first_chapter_prompt()`，走 PromptRegistry 的 `narrative_first_chapter_generate` 条目（15 个模板变量），后台可编辑覆盖
    - **Gap 2 系统提示词覆盖**：`LlmProfile` +`system_prompt_override` → `GenerateRequest` +`system_prompt` → OpenAI/Anthropic adapter 去硬编码英文 "You are a professional…"，改为 `req.system_prompt`。前端 ModelModal 新增多行文本框。优先级：每模型 > adapter 默认
    - **Gap 3 框架级智能路由**：新增 `FrameworkSelections`（methodology/quality_gate/contextual_injectors/prompt_hints）。Call 1 最快模型收到 `build_prompt_framework_catalog()`（方法论文/质量门/注入器目录），输出 `framework_selections`。各步骤消费选择结果
    - 验证：`cargo test --lib` **578 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` ✓；`npx tsc --noEmit` ✓

  - **v0.23.60 网关探测异步化 + 调度退避 + 并发限流 + 卡死诊断** (2026-06-27) — 基于本机日志分析的精益优化。核心变更：
    - **网关探测异步化**：后台 keepalive 每 10s 刷新健康模型（5s 超时），`is_health_fresh()` → true 时网关跳过内联 5s 探测，正常运行时 0ms 附加延迟
    - **死模型退避**：连续失败 ≥3 次 → 指数退避 30→60→120→…→3600s
    - **后台 LLM 并发限流**：`BACKGROUND_LLM_SEMAPHORE(1)`，Call 3 后 BGP-1/BGP-3 串行化
    - **卡死诊断**：`execute_trishot` 返回前 → `orchestrator.generate` 返回前 → genesis DB 保存前后加 `log::warn!`
    - 验证：`cargo test --lib` **578 passed / 0 failed / 2 ignored**

  - **v0.23.59 全面修复并强化模型网关调度** (2026-06-27) — 审计发现创世流程 5 个 LLM 调用中 4 个绕过网关、真实调用失败对调度器不可见两个系统性缺陷，全面修复。核心变更：
    - **根因 A（审计确认）**：创世流程 5 个 LLM 调用中，只有 TriShot Call 3 经过网关（带 5s 预探测 + 候选 fallback）。故事概念生成、Call 1 路由合成、5 个后台 pipeline 步骤全部绕过网关——直接走 `select_profile_for_request` + 单适配器，死模型挂起 300s 直到 LLM 超时，无候选切换
    - **根因 B（审计确认）**：候选循环中真实调用失败只 `continue` 到下一候选，不更新健康注册表。只有预探测失败才标记 Unhealthy。预探测成功≠真实调用成功——模型能说 "OK" 但无法生成正文时，下次 `generate()` 又强制置顶该模型（`+1000` 分）。`Degraded` 状态从未被任何路径写入
    - **Fix 1 路由到网关**：`generate_for_request_with_context_and_pipeline`（service.rs）改为委托 `generate_for_request_with_request_id`（网关路径），单点覆盖概念生成 + 5 个后台 pipeline。新增 `derive_intent_from_label` 从 `context_label` 派生 `intent_verb`/`intent_object` 激活意图感知分类
    - **Fix 2 `generate_with_fastest` 探测+fallback**：新增 5s 预探测（`probe_profile_quick`），探测通过才直接调用（保留速度优势），失败标记 Unhealthy + 递增失败计数，回退到网关候选链。新增 `GatewayExecutor::mark_unhealthy`/`record_success_public` 公开接口
    - **Fix 3 连续失败降级**：`HealthRecord` +`consecutive_failures` 字段 + `record_failure`/`record_success`/`consecutive_failures` 方法；网关候选循环 5 处跟踪真实调用成败（真实调用失败标记 `Degraded`——让"能说 OK 但无法生成正文"的模型对调度器可见）；新增 `active_model_demoted()` + `ACTIVE_MODEL_DEMOTION_THRESHOLD=2`，3 个强制置顶点（`select_candidates`/`generate`/`select_fastest_profile`）连续失败≥2 时跳过强制置顶，成功 1 次即清零恢复
    - **Fix 4 TimeSliced 写作策略**：`WriteTimeBundle::load_sync()` 的写作策略约束字符串是硬编码（`运行模式：标准\n冲突强度：0.5\n...`），用户在后台设置修改的策略从未在 TimeSliced 续写路径生效。`execute_time_sliced()` 从 `AppConfig` 读取真实 `WritingStrategy` 并覆盖 `bundle.writing_strategy_constraints`（`format_writing_strategy_constraints` 新增于 `write_time_bundle.rs`）
    - 验证：`cargo check` 零错误；`cargo test --lib` **578 passed / 0 failed / 2 ignored**（571 基线 + 7 新增）；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误

  - **v0.23.49 推理模型思考链导致 JSON 提取出空对象修复** (2026-06-26) — 解决推理模型创世时报 `missing field 'title' at line 1 column 2` 的问题。核心变更：
    - **根因（日志确认）**：MN-Oblivion-26B 等推理模型在正文前输出 `önh...` / `<thinking>...</thinking>` 思考链，思考链里含花括号（如 "用 {} 格式表示"）。`extract_first_json_object` 用 `content.find('{')` 找第一个 `{`，它落在思考链里那个 `{}` 上，括号匹配返回空对象 `{}`，serde 反序列化 `StoryMetaElement` 时找不到必填 `title`。LLM 实际成功返回 5191 字符，失败在 JSON 提取阶段
    - **修复**：新增 `strip_reasoning_blocks`（在 `extract_and_sanitize_json` 第一步剥离配对思考链块，标签以字节数组构造避免源码字面量被误处理）；`extract_first_json_object` 跳过空对象 `{}` 继续向后扫描。`extract_and_sanitize_json` 是 genesis/ingest/analysis/auto_contract 共用 JSON 提取咽喉点，一次性受益
    - 验证：`cargo test --lib` **571 passed / 0 failed / 2 ignored**

  - **v0.23.48 JSON 提取用括号匹配修复 trailing characters 解析失败** (2026-06-25) — 解决 LLM 返回 JSON 后附带含 `}` 文本导致的解析失败。核心变更：
    - **根因**：`extract_and_sanitize_json` 用 `rfind('}')` 找 JSON 结尾，LLM 在 JSON 后输出包含 `}` 的额外文本时，`rfind` 误提取过多内容 → serde_json "trailing characters" 错误
    - **修复**：新增 `extract_first_json_object` 用括号匹配（跟踪 `{`/`}` 深度 + 跳过字符串字面量）精确提取第一个完整 JSON 对象
    - 验证：`cargo test --lib` **568 passed / 0 failed / 2 ignored**

  - **v0.23.47 调用模型前实时连接探测（5s），跳过失效死模型** (2026-06-25) — 解决死模型浪费 30-300s 超时。核心变更：
    - **根因**：模型列表里可能存在已失效但健康状态仍为 Healthy 的死模型（本地服务已停止但缓存未更新）
    - **修复**：`GatewayExecutor::generate` 候选循环中，每个候选模型在实际 LLM 调用前先执行 5s 超时实时探测；探测失败/超时则标记 `HealthStatus::Unhealthy`，跳到下一候选

  - **v0.23.46 顶部流程进度和底部AI进程状态统一使用模型名称** (2026-06-25) — 状态栏文案追加模型名称。

  - **v0.23.45 IngestPipeline LLM 调用静默化，根治正文后活动卡死与页面崩溃** (2026-06-25) — 解决创世正文返回后前端崩溃问题。核心变更：
    - **根因（日志确认）**：创世正文返回后，IngestPipeline 并发发起多个"记忆-内容分析"LLM 调用，`context_label` 未匹配 `is_silent_background` 静默列表，导致进度事件覆盖前端主活动状态（"准备上下文"卡住）。本地模型无法处理并发请求返回 `INTERNAL_ERROR`，大量错误事件涌入导致前端页面崩溃空白
    - **修复**：将 IngestPipeline 的三个 `context_label`（`"记忆-内容分析"`、`"记忆-生成知识"`、`"记忆-叙事事件提取"`）加入 `is_silent_background` 静默列表
    - 验证：`cargo check` 零错误

  - **v0.23.44 AI 状态提示使用模型名称** (2026-06-25) — 状态栏文案追加模型名称。核心变更：
    - `generation-status` 和 `llm-generating-progress` 心跳事件的状态文案追加模型名称（格式：`准备上下文... · gemma4-e2b (OpenAI) (15s)`）
    - 模型名称来自 `lastLlmModelRef`（在 `llm-prompt-sent` 事件中设置为 `model_name (provider)` 格式）

  - **v0.23.43 前端诊断日志 + log_frontend_event 命令** (2026-06-25) — 前端可写入 WorkflowLogger。核心变更：
    - 新增 `log_frontend_event` Tauri 命令，前端关键路径（`setContent`/`selectChapter`/`ChapterSwitch`）可写入 `creative_workflow.log`
    - 诊断卡片自动收集这些前端日志

  - **v0.23.42 根治创世卡在"最终输出"：BGP-4 自死锁修复** (2026-06-25) — 根治 v0.23.40-41 中创世 Call 3 完成后卡死 600s 超时的问题。核心变更：
    - **根因（日志确认）**：`execute_trishot` 在 Call 3 成功返回后用 `spawn_blocking().await` 同步等待 BGP-4 `should_trigger` DB 查询，与 BGP-1/BGP-3 后台任务竞争 `std::sync::Mutex` 导致自死锁，`execute_trichot` 永不返回，DB 保存和 ChapterSwitch 事件从未执行
    - **修复**：BGP-4 从 `spawn_blocking().await` 改为 `tokio::spawn`（fire-and-forget），`execute_trichot` 在 Call 3 完成后立即返回
    - **诊断日志**：`trishot.call3.done` → `trishot.bgp4.spawn` → `trishot.bgp4.done` 全部在 1ms 内完成
    - 验证：`cargo test --lib` **563 passed / 0 failed / 2 ignored**

  - **v0.23.40 参照现有诊断机制添加 WorkflowLogger 日志点** (2026-06-25) — 在关键缺失点补充日志。核心变更：
    - **Bug A 日志点**：`genesis.first_chapter.generated`、`genesis.chapter_switch.sent`、`genesis.final_content`（记录是摘要还是完整正文）
    - **Bug B 日志点**：`smart_execute.start`、`trishot.call3.done`、`trishot.bgp4.start`/`bgp4.done`
    - 前端 `[DEBUG-dup]` / `[DEBUG-act]` console.warn 诊断日志

  - **v0.23.37 Genesis 活动清理 + 前端正文重复修复尝试** (2026-06-25) — 修复 Genesis 完成后活动卡死。核心变更：
    - **Bug B-1**：Genesis 成功路径补发 `smart-execute-progress` 的 `completed` 事件，错误路径补发 `error`
    - **Bug B-2**：`smart-execute-progress` 处理器把 `timeout`/`error` 也映射为 `failed`（此前只认 `completed`）
    - v0.23.39 回滚了激进的前端修改（selectChapter guard / 跳过 selectChapter / setGeneratedText('')），保留后端修复

  - **v0.23.36 创世正文清洗 + 后台作业不阻塞输入** (2026-06-24) — 解决创世流程最终输出的两个瑕疵。核心变更：
    - **TriShot Call 3 输出纪律段**：`NOVEL_OUTPUT_DISCIPLINE` 常量追加到 Call 3 的 `final_prompt` 末尾，约束模型只输出纯小说正文，禁止元评论/markdown 格式/`【】`小节标题/`（幕结束）`批注
    - **`sanitize_novel_output` 后处理兜底**：Call 3 返回后对内容做四步清洗（逐行去 markdown 符号 → 截断尾部元评论 → 剥离前导过渡语 → 去整行小节标题/批注），7 个单元测试覆盖
    - **Genesis 后台阶段不阻塞输入**：后台完善世界观/角色/场景时 `pipeline-progress` 事件打 `metadata:{background:true}` 标记，前端 `useBackendActivityListener` 检测到后跳过注册 running activity，输入框不再被禁用；状态文案仍由 `novel-bootstrap-progress` 监听器独立更新
    - 验证：`cargo test --lib` **563 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误

  - **v0.23.35 采摘 Step1 JSON 解析容错** (2026-06-23) — 修复采摘作业 Ingest Step1 `missing field entity_type` 错误。核心变更：
    - `memory/ingest.rs` 中 6 个反序列化结构体补 `#[serde(default)]` 容错字段（AnalyzedEntity/AnalyzedRelation/AnalyzedEvent/SentimentAnalysis/Foreshadowing）
    - LLM 返回 JSON 缺失这些字段时不再解析失败，3 次重试均失败的采摘错误消除
    - 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

  - **v0.23.34 修复 select_candidates 中 std::sync::Mutex 自死锁（根因彻底查明）** (2026-06-23) — v0.23.31-33 全链路 15 个诊断标记精确定位：`select_candidates` 第125行 `health_registry.lock()` 获取 MutexGuard，变量存活到函数末尾不释放，第188行 `is_model_available` 再次 `lock()` 同一 Mutex → `std::sync::Mutex` 不可重入 → 自死锁 → 600s 超时。Call 1 走 `select_fastest_profile` 不受影响。修复：health 锁移入嵌套块作用域，块结束时自动释放。
    - 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过

  - **v0.23.30 Genesis 全线阻塞点修复：genesis_default + select_candidates spawn_blocking + Chapter 保存 spawn_blocking** (2026-06-23) — 全线同步 DB 操作从 tokio worker 线程移出。核心变更：
    - **`GenerationMode::genesis_default()`**：显式化 Genesis 模式选择，替代写死的 TriShot。Genesis 始终用 TriShot（需资产选择 + 快速出章），用户模式设置影响日常续写/改写
    - **`select_candidates` spawn_blocking 预加载能力档案**：`GatewayExecutor::generate` 中同步 DB 查询 `CapabilityStore::load_all()` 用 `spawn_blocking` 包裹，修复 Call 3 卡死
    - **Chapter 保存 spawn_blocking**：`FirstChapterGenerationStep` 中 `ChapterRepository::get_by_story/update/create` 移入 `spawn_blocking`
    - **Genesis 跳过 Call 2 精修器**：第 1 章 + 无已有内容时直接进 Call 3
    - **前端显示 "[创世]"**：Genesis 期间底部栏显示创世状态而非"三击模式"
    - 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npx vitest run` 126 passed

  - **v0.23.28 select_candidates spawn_blocking + v0.23.29 Chapter 保存 spawn_blocking** (2026-06-23) — 最后两个同步 DB 阻塞点修复。Call 3 不再卡在 gateway 路由；第一章内容写入不再卡在 DB。

  - **v0.23.24 setContent 内容比较 + v0.23.23 RichTextEditor isExternalSyncRef** (2026-06-23) — 从根源和入口双重杜绝伪"保存中"：`setContent` 和新内容相同时跳过；编辑器外部 `setContent` 触发的 `onUpdate` 跳过 `onChange` 回调。

  - **v0.23.21 根治 TriShot auto_fill 5 次 LLM 调用耗尽预算** (2026-06-22) — v0.23.19 仍 600s 超时，日志显示 `try_state` 到 `spawn` 卡 4 分钟：`record_llm_call` 虽然把 DB 写入移入 spawn_blocking，但 `try_state` + `count_tokens` + `get_active_profile` + 数据收集仍在 tokio worker 线程同步执行。同时用户反馈"保存中"卡死——`update_chapter` 是同步 Tauri command，连接池满时 `pool.get()` 阻塞。核心变更：
    - **Fix 4 整个 record_llm_call 放入 spawn_blocking**：async 线程只 clone owned 数据，所有工作（token 计数、try_state、DB 写入）在阻塞线程池执行，tokio worker 线程零阻塞
    - **Fix 3 update_chapter 改 async + spawn_blocking**：同步 `pub fn` → `pub async fn`，DB 操作用 `spawn_blocking` 包裹，连接池满时不再阻塞前端"保存中"
    - **连接池扩容 20 → 50**：缓冲 auto_commit/ingest/projection writers 并发占用
    - **新增 `get_db_pool_status` 命令**：返回 `{max_size, connections, idle, in_use, connection_timeout_secs}`，前端可实时监控连接池状态
    - **前端 DB 连接池指示器**：`useDbPoolStatus` Hook 5s 轮询，FrontstageHeader 状态栏 ≥80% 黄色预警 / ≥95% 红色告警；诊断卡片新增 `DB连接池` 字段
    - 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异；`vitest run` 126 passed

  - **v0.23.19 根治 record_llm_call 阻塞 tokio worker 导致 600s 超时** (2026-06-22) — v0.23.18 行级工作流日志精确定位卡点：概念生成 LLM 调用 1.1s 完成，但随后的 `record_llm_call` 同步 DB INSERT 卡住 600s 永不返回。根因是 `record_llm_call` 在 async 上下文中直接执行同步 `pool.get()` + `conn.execute()`，而生产连接池未配置 `connection_timeout`，连接池满时 `pool.get()` 无限阻塞 tokio worker 线程，`tokio::time::timeout` 无法 poll。核心变更：
    - **生产连接池加 `connection_timeout(5s)`**：`init_db` 的 `Pool::builder()` 补 `.connection_timeout(Duration::from_secs(5))`，与测试池一致，防止 `pool.get()` 无限阻塞
    - **`record_llm_call` 改为 fire-and-forget**：收集 owned 数据后 `tokio::task::spawn_blocking` 提交 DB 写入到阻塞线程池，立即返回不等待结果。指标记录是审计用途，失败不影响生成结果，永不阻塞主流程
    - 移除 `record_llm_call` 内部的 `llm.record_call.db_write` / `db_done` 工作流日志（DB 写入已异步化，无法在主流程观察到完成时刻），新增 `llm.record_call.spawn` 标记提交点
    - 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

  - **v0.23.16 Genesis 快速阶段卡死修复 + E2E 集成测试** (2026-06-22) — 根治 v0.23.15 中概念 LLM 完成后 pipeline 阻塞 600s 的问题。根因是 `StoryRepository::create()` 为同步 r2d2 调用，在 async 上下文中直接执行，若 DB 锁或连接池满则阻塞 tokio worker 线程，导致 `tokio::time::timeout(600s)` 无法 poll。核心变更：
    - `story_repo.create()` 改用 `tokio::task::spawn_blocking` 异步化
    - `ConceptGenerationStep` / `FirstChapterGenerationStep` / `smart_execute` 关键路径添加 `log::warn!` 诊断日志
    - 新增 `scripts/test_trishot_e2e.py` E2E 集成测试，用真实 LLM 模拟完整 Call 1-3
    - E2E 验证：Gemma4-e2b 真实模型 **73.2s 完成，2270 字符，1852 中文字，全部检查通过**
    - 验证：`cargo test --lib` **551 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过

  - **v0.23.15 TriShot 管线 4 处缺陷修复** (2026-06-22) — 审查 `execute_trishot` Call 1-3 全路径发现 P0/P1/P2 共 4 处缺陷。核心变更：
    - **P0**: `execute_trishot` 预检用 `QuickPreflightChecker` 不触发 auto-fill，Genesis 新故事必然失败。修复：预检失败时调 `AutoContractBuilder::auto_fill` 补齐角色后重试
    - **P1**: 前端 `novel_bootstrap_background_started` 消息导致第一章正文被当幽灵文本。修复：改名 `novel_bootstrap_first_chapter_ready`
    - **P2**: Call 1 预算守卫 `t_synth` 刚创建 `elapsed≈0` 永远不触发；Call 2 硬编码 `total_budget=180`；Call 3 无超时覆盖可跑满 300s。修复：用 `total_start` 计算已耗时间、读配置 budget、Call 3 超时 30-120s + 空内容检查
    - 移除 `strategy_selection_step()` 死代码
    - 验证：`cargo test --lib` **551 passed / 0 failed / 2 ignored**

  - **v0.23.14 干净健康的模型池 + 两阶段 Genesis** (2026-06-22) — 建设干净的模型池并重构 Genesis 为快速阶段（30-60s 返回正文）+ 后台阶段。核心变更：
    - **模型池净化 L1-L4**：启动归零清除历史 `llm_calls` + 过滤 `HealthRegistry` 残留；删除/更新模型级联清理；拒绝 disabled 设为活跃；清理硬编码死模型 IP；健康报告数据源从历史表切换为实时探测快照
    - **Genesis 两阶段**：`quick_phase_steps()` = 概念 + 第一章（TriShot 模式）；`background_steps()` = 策略选择 + 世界观/大纲/角色等
    - `FirstChapterGenerationStep`: `Full` → `TriShot` 模式（270s → 30-60s）
    - 验证：`cargo test --lib` **551 passed / 0 failed / 2 ignored**

  - **v0.23.13 强制所有生成路径使用活跃模型** (2026-06-22) — 彻底解决”当前模型是 A，实际调用 B”导致的 600 秒超时。核心变更：
    - `LlmService::select_profile_for_request` 无条件优先返回用户设置的 `active_llm_profile`
    - `GatewayExecutor::select_candidates` 将健康活跃模型强制置顶为 primary，避免被三维打分/算力档案绕开
    - `GatewayExecutor::select_fastest_profile` 只要活跃模型健康（Healthy/Degraded）就优先使用，不再受 TTFB 阈值限制
    - Genesis 故事概念、TriShot Call 1/Call 3、普通路由生成全部走活跃模型
    - `create_model` 保存后即时刷新网关注册表并执行健康探测，新模型立即进入可用池
    - 验证：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

  - **v0.23.12 彻底修复长超时：活跃模型优先 + 智能创作流程日志** (2026-06-22) — 根因是模型网关连接了非当前设置的模型，导致实际调用的模型挂起/不可用。核心变更：
    - `GatewayExecutor::generate` 把用户当前设置的活跃模型强制提升到候选链首位
    - `select_fastest_profile` 在活跃模型无算力档案时也优先使用活跃模型
    - 新增 `WorkflowLogger`，记录 TriShot 每个阶段、LLM 调用起止、模型网关候选链等，写入 `logs/creative_workflow.log`
    - 诊断卡片新增工作流日志路径与最近日志
    - 验证：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

  - **v0.23.11 诊断提示词过滤探测/静默调用** (2026-06-22) — 修复诊断卡片里“最后发给模型的提示词”被 `model_gateway_probe` 的 `Respond with exactly the word OK.` 覆盖的问题。核心变更：
    - `LlmService::execute_generation` 仅在非静默/非探测调用时更新 `DiagnosticStore` 和 `llm-prompt-sent` 事件
    - 过滤范围：probe、input_hint、intent_detection、后台审计/洞察、tri-shot-router/refiner、bg-auto-rewriter、bg-ingest
    - 验证：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

  - **v0.23.10 模型网关优先使用当前活跃模型** (2026-06-22) — 修复“AI 连接了以前的模型 ID，没有连接当前设置的模型”的问题。核心变更：
    - `select_fastest_profile` 在选最快模型前先读取当前 `active profile`；若活跃模型健康且 TTFB 不比最快模型差太多，优先使用活跃模型
    - `select_candidates` 保证活跃模型始终出现在候选链中，避免路由结果完全脱离用户预期
    - 验证：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

  - **v0.23.9 运行时创作资产能力清单 + TriShot 路由增强** (2026-06-22) — 解决“组合提示词不顺利”的根因：Call 1 原本只能看到当前故事约束，看不到系统级创作资产。核心变更：
    - 新增 `AssetCapabilityManifest` Tauri State，启动时自动生成全部系统资产（methodology、genre_profile、skill、beat_card、story_engine、pressure_relationship 等）的紧凑目录
    - `PromptSynthesizer` Call 1 prompt 注入【系统可用创作资产目录】，让模型知道可调用的资产
    - TriShot Call 3 通过 `generate_for_task_with_tags` 把 Call 1 选中的资产透传给 `ModelGateway`
    - `ModelGateway` dispatcher 识别更多创作资产标签并归类为 `HeavyCreation`
    - 修复 TriShot `request_id` 被错误赋值为模型名、Call 1 无预算守卫的问题
    - 验证：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

  - **v0.23.8 AI 进度指示精细化 + 提示词诊断可靠性提升** (2026-06-22) — 让 LLM 生成过程可见：连接模型 ID/提供商、组合提示词规模、等待回应、模型回应 token 数、解析结果。核心变更：
    - `LlmGeneratingProgress` 新增 `model_id`、`provider`、`prompt_chars`、`prompt_tokens`、`response_tokens`
    - 心跳文案从“构思故事”改为具体阶段描述，并实时显示模型 ID 与提示词规模
    - 新增 `diagnostics::DiagnosticStore` Tauri State 与 `get_last_llm_prompt` 命令，避免大提示词事件丢失导致诊断卡片“未捕获”
    - 验证：`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

  - **v0.23.7 诊断信息增强 + 超时文案去硬编码** (2026-06-22) — 修复诊断卡片版本号仍显示 `0.16.0`、超时文案硬编码 200/180 的问题，并补充 AI 生成模式、当前模型、最后发给 LLM 的提示词全文。核心变更：
    - `src-frontend/src/main.tsx` / `src/frontstage/main.tsx` 从 `package.json` 动态注入 `__STORYFORGE_VERSION__`
    - `FrontstageApp.tsx` 的 `handleRequestGeneration` / `handleSmartGeneration` 从 `settings` 读取实际超时时长
    - 诊断卡片新增 `AI生成模式`、`当前模型ID/名称/提供商/端点`、`最后调用模型`、`最后发给模型的提示词`
    - 后端 `LlmService` 调用模型前发射 `llm-prompt-sent` 事件，前端监听并缓存最后一次 prompt
    - 验证：`cargo check` 零错误；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

  - **v0.23.6 修复 macOS 启动崩溃（VectorStore State 初始化顺序）** (2026-06-22) — 修复启动时 `state() called before manage() for Arc<dyn VectorStore>` panic 导致的 macOS 崩溃。核心变更：
    - 将 `LanceVectorStore` 的创建与 `app.manage(vector_store)` 提前到 `init_task_system_and_automation` 之前
    - 仅调整 State 注入时序，异步 `init()` 保留在原地
    - 验证：`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npm run format:check` / `npm run type-check` 通过；`python3 scripts/architecture_guard.py` 通过

  - **v0.23.5 CI 格式化修复** (2026-06-21) — 修复 Rust nightly `cargo fmt` 格式化差异（import 顺序、函数参数折行、单行化）与前端 Prettier 差异（`GeneralSettings.tsx` 类型断言单行化）。无业务逻辑变更，仅代码风格修复，使 GitHub Actions `rust-check` / `frontend-check` 通过。

  - **v0.23.4 智能层闭环落地** (2026-06-21) — TriShot 管线与架构债务清偿后，补齐智能创作层最后一环。核心变更：
    - LLM JSON mode：`llm::adapter::ResponseFormat::JsonObject`，OpenAI/Ollama 适配器原生结构化输出，`GatewayRequest` 透传 `response_format`
    - Review/Refine Pipeline 调用 JSON mode 并解析 `refinement_notes`
    - `MemoryBudget::for_task_type` 强类型化预算参数（`MemoryTaskType { Write, Plan, Review }`）
    - 拆书存储统一：删除 `reference_characters` / `reference_scenes`，数据汇入 `narrative_*` 表；迁移 `V100__拆书存储统一_删除_reference_表.sql`
    - 验证：`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`python3 scripts/architecture_guard.py` 通过

  - **v0.23.3 测试基线修复 + 工程化（48 个 V092 失败清零）** (2026-06-21) — 修复迁移框架 bug 与 narrative 表 schema 不匹配，让 `cargo test --lib` 首次全绿。核心变更：
    - **MigrationRunner 交错执行**：`run_with_legacy` 改为按版本将 SQL 文件 migration 与 inline Rust migration 交错执行，避免高版本 SQL 文件跳过低版本 inline migrations；新增 `MAX_INLINE_MIGRATION_VERSION` 约束与注释
    - **SING migration 版本上调**：`V095__意图图_SING_数据层.sql` → `V099__...`，确保其跑在所有 inline migrations 之后
    - **`narrative_*` 表补 status 列**：`narrative_characters` / `narrative_scenes` / `narrative_world_buildings` 加入 `status TEXT NOT NULL DEFAULT 'active'`，并新增 inline Migration 98 为已存在表补列
    - **ElementSource/ElementStatus round-trip 修复**：`domain/narrative_elements.rs` 新增 `as_str()` / `from_str()`（snake_case 英文）；`db/repositories_narrative.rs` 存储与解析统一使用英文键，新增 3 个 repository round-trip 测试
    - **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**（新增 3 个测试，零回归）；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

  - **v0.23.2 事件总线与状态同步治理** (2026-06-21) — 在 v0.23.1 架构清理基础上，补齐后端提交事件流并收敛前端编辑器状态源。核心变更：
    - **后端 `SyncEvent::ChapterCommitted`**：`state_sync/events.rs` 新增 `ChapterCommitted` 变体，携带 `projection_status`；`SceneCommitService::apply_commit` 在 projections 完成后统一发射，替代零散的 `dataRefresh("knowledgeGraph")`
    - **前端 `content/isSaved` 迁移到 `frontstageStore`**：`FrontstageApp.tsx` 移除本地 `useState(content/isSaved)`，改为 `useFrontstageStore` 读写；保留 `isSaved` + editor focus 双重保护，后台同步事件不会覆盖未保存编辑内容
    - **清理遗留事件/hack**：删除所有 `backstage-data-refreshed` 废弃注释，更新 `CONTEXT.md` 数据刷新说明；`useWebViewRedrawFix` 改为 `FIXME` 标记，待真实场景验证后再移除
    - **验证**：`cargo check` 零错误；`cargo test --lib` 487 passed / 48 failed（新增 1 个序列化测试，基线一致，零新回归）；`npx tsc --noEmit` 零错误；`npx vitest run` 126 passed / 3 skipped

  - **v0.23.1 架构债务清偿：全局单例治理 + 模块依赖解耦** (2026-06-21) — 为 TriShot 之后的长期可维护性清理架构底层，零业务行为变更。核心变更：
    - **全局单例清零**：彻底移除 14 个全局 `static`/缓存（`VECTOR_STORE` / `DB_POOL` / `LLM_SERVICE` / `APP_CONFIG` / `SKILL_MANAGER` / `CHAPTER_COMMIT_DEBOUNCE` / `PENDING_VECTOR_INDEXES` / `WRITER_*` 缓存 / `APP_CONFIG_CACHE` 等），全部改为 Tauri State 注入或每次调用重新加载
    - **domain 领域层扩展**：新增 `agent_context` / `agent_types` / `foreshadowing` / `search` / `write_time_bundle` / `asset_snapshot` / `continuity` / `adaptive` / `prompt_synthesis` / `agent_service` / `creative_engine` 等共享类型与端口，统一跨模块数据契约
    - **模块循环依赖斩断**：`memory → agents`、`narrative → memory`、`narrative → creative_engine` 数据类型下沉到 `domain`；`agents ↔ creative_engine` 行为依赖通过 `CreativeEnginePort` / `AgentServicePort` 双向反转，彻底消除循环导入
    - **架构守卫收紧**：`scripts/architecture_guard.py` 的 `KNOWN_VIOLATIONS` 清空，`architecture_guard.py` 报告 **0 known violations / 14 enforced global singletons removed**
    - **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed / 48 failed（与 TriShot 基线一致，零新回归）

  - **v0.23.0 TriShot 三击生成管线** (2026-06-21) — 全面实施「最多 3 次 LLM」三击生成架构。核心变更：

    - **TriShot 三击管线**：新增 `GenerationMode::TriShot` 模式，Call 1 用最快模型选资产+合成提示词 → Call 2(可选) 精修 → Call 3 Writer 生成。关键路径最多 3 次 LLM，质检/改写/入库/洞察全部下沉后台静默执行
    - **prompt_synthesis 模块**：`manifest.rs` 资产清单（4000 字符预算）+ `synthesizer.rs` 路由合成器（最快模型选资产+合成）+ `refiner.rs` 精修器（可选，预算守卫）
    - **最快模型选取**：`GatewayExecutor::select_fastest_profile()` 按 CapabilityProfile TTFB 升序选最快可用模型 + `LlmService::generate_with_fastest()`
    - **PlanExecutor 快速路径**：TriShot 跳过 SING/PlanGenerator（Call 1 替代），`PlanStep::long_running` 跳过 90s 步超时
    - **后台 agent 体系**：BGP-1 质检（复用 AuditExecutor）→ BGP-2 自动改写器（新 `auto_rewrite_executor.rs`，分严重度）→ BGP-3 入库（补 smart_execute 缺口）→ BGP-4 洞察（复用 InsightExecutor）
    - **SyncEvent 扩展**：`ContentAutoRevised`（HIGH 自动改写通知，可撤销）+ `RevisionSuggested`（LOW 建议审阅面板）
    - **配置**：`AppConfig.generation_mode` 新增 `"tri_shot"` + `auto_rewrite_severity_threshold`；前端设置下拉新增「三击模式」
    - **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed（新增 TriShot 相关 19 测试全部通过，零回归）；`npx tsc --noEmit` 零错误

  - **v0.22.4 「异星球末世生存」智能创作流程优化** (2026-06-21) — 针对复合题材（如「异星球末世生存」）解析断链、意图图资产发现不足、模型网关调度未感知资产标签、TimeSliced 默认续写路径缺失次要题材画像等问题，进行系统性补强：
    - **GenreResolver 题材解析服务**：新增 `strategy/genre_resolver.rs`，支持精确/别名/子串/同义词/复合题材解析；将「异星球末世生存」解析为末世+科幻/星际机甲等多画像
    - **StrategySelector 链路改造**：`exact_genre_match`、`build_selected_strategy`、`story_concept_prompt` 均接入 GenreResolver，LLM 输出标准化 `genre_profile_ids`
    - **意图图资产发现增强**：`AssetNode` 支持 tags；资产同步注入标签；`IntentionGraphPlanner::discover_assets` 用 GenreResolver 补充复合题材相关 `genre_profile`
    - **模型网关资产感知调度**：`GatewayRequest` 新增 `asset_tags`/`discovered_asset_ids`，`TaskClassifier` 按标签校准任务类别，`GatewayExecutor` 按标签重叠加分
    - **TimeSliced 续写复合题材补强**：`WriteTimeBundle` 新增 `secondary_genre_profile_strategy`，复合题材时把次要题材画像摘要注入默认续写 prompt
    - **验证**：新增 targeted tests 全部通过（genre 5/5、selector 6/6、write_time_bundle 13/13、dispatcher 5/5、intention_graph 19/19 passed，2 ignored）；`cargo check` 零错误；`npx tsc --noEmit` 零错误

  - **v0.22.3 钥匙串彻底移除 + 模型健康报告自动刷新 + 配置加载优化** (2026-06-21) — 根据用户反馈实施 3 项关键改进：
    - **钥匙串彻底移除**：删除 `keyring` crate、`secure_storage` 模块、`store_api_keys_securely` 配置项；API Key 直接存 SQLite；移除 `load()/save()` 中全部钥匙串读写逻辑（共~260 行），启动/操作时不再弹出 macOS 钥匙串密码提示
    - **模型健康报告自动刷新**：前端 `refetchInterval: 30_000` 每 30 秒自动刷新；后端改为 async 命令不阻塞 IPC
    - **冗余 load 消除**：`execute_writer` 2→1 次、`FirstChapterGenerationStep` 3→1 次、`book_deconstruction` 死代码移除
    - **验证**：`cargo check` 零错误，`cargo test --lib` 425 passed，`npx tsc --noEmit` 零错误 — 根据测试反馈实施 4 条建设性意见。关键变更：
    - **GenreProfile 推荐种子**：`seed_genre_recommendations()` 为末世/科幻/修仙/都市/悬疑/历史 6 个题材写入推荐风格+方法论+技能映射
    - **策略选择器硬约束**：`build_selected_strategy` 中体裁画像有推荐时跳过 LLM 直接使用
    - **算力档案默认值修正**：capability_score 未测试时默认 0.0（避免虚假质量分基准）

  - **v0.22.1 提示词与后台资产深度结合** (2026-06-21) — 根据测试报告实施 5 条建设性意见。关键变更：
    - **StrategySelector 题材推荐映射**：`get_genre_recommendations()` 覆盖末世→余华等 7 种题材→风格推荐
    - **StyleDNA 句长偏差检测**：`execute_time_sliced` 生成后检测句长偏差，>30% 记录建议
    - **Inspector 方法论动态 prompt**：按 methodology_id 选择 prompt（5 种方法论全覆盖）
    - **GenreProfile 推荐字段**：4 新列 + Migration 96 + Repository SQL 更新

  - **v0.22.0 提示词与后台资产完整结合** (2026-06-21) — 修复 5 个系统性缺口：TimeSliced 全资产注入 / Inspector 全资产注入 / 意图感知调度接线 / 算力档案消费闭环 / 资产→生成参数规则映射。关键变更：
    - **Phase A**: WriteTimeBundle 新增 4 字段，`to_prompt()` 追加 4 个 section
    - **Phase B**: `build_inspector_prompt` 追加题材画像/方法论/角色状态/冲突/四元组
    - **Phase C**: `generate_for_request_with_request_id` 新增 intent 参数，agent_type 自动推导意图
    - **Phase D**: `select_candidates` 加载 CapabilityProfile 参与候选排序
    - **Phase E**: 新增 `asset_params.rs` —— StyleDNA→temperature / methodology→max_tokens / genre→max_tokens
    - **验证**：cargo check 零错误，真实模型 6/6 通过，tsc 零错误

  - **v0.21.0 提示词全量可配置化：从"聊胜于无"到"全面可控"** (2026-06-21) — 审计发现现有"提示词覆盖"仅覆盖 14 个 key，15 个假接入（走 resolve_prompt_default 旁路 DB），40+ 个活跃硬编码提示词完全旁路 registry。全面修复后所有提示词均可在后台设置页面查看、编辑、保存。关键变更：
    - **Phase 1 registry 扩展**：新增 6 个 PromptCategory，注册 ~50 个新提示词条目，新增 resolve_prompt_with_vars
    - **Phase 2 假接入修复**：snowflake 10 个 + multi_agent 5 个 key 改为 resolve_prompt（含 DB 覆盖）
    - **Phase 3 旁路接线**：40+ 个硬编码提示词全部接入 registry（narrative/pipeline/planner/agents/memory/audit/strategy/deconstruction/methodology/intention_graph）
    - **Phase 5 前端升级**：PromptsPanel 编辑器升级为 Monaco，新增批量导出/导入
    - **验证**：cargo check 零错误，intention_graph 18/18 通过，真实模型测试通过，tsc 零错误

  - **v0.20.1 SING 意图图集成审计修复：5 处致命断环 + 理论对齐** (2026-06-21) — 对 v0.20.0 SING 集成进行深度审计后发现 5 处致命断环导致意图图路径运行时从未生效（静默回退到 PlanGenerator），系统性修复全部问题。关键变更：
    - **P0-1 资产同步接通**：`lib.rs` setup 阶段调用 `AssetSyncEngine::full_initialize` + `warm_up_cache`，将 CapabilityRegistry/SelectableAsset/Agent/系统命令同步到意图图表；IntentionGraphRepository 注册为 Tauri state 供共享缓存
    - **P0-2 模型网关意图感知生效**：`GatewayRequest` 新增 `intent_verb`/`intent_object` 可选字段，`classify_task` 优先使用 `classify_by_intention` 进行意图感知分类
    - **P0-3 执行图持久化**：`execute_with_react` 接受 `invoke_fn` 回调实现真实步骤执行（替代硬编码假输出），执行图 + 执行节点持久化到数据库供诊断面板查询；`record_execution_graph` 在意图图计划生成成功后调用
    - **P0-5 LLM 意图合成**：`IntentSynthesisPipeline::synthesize_query` 新增 LLM 增强版（JSON 结构化输出提取动词-宾语），失败时优雅降级到规则匹配
    - **P1-1 评分权重对齐论文**：`discover_tool_level` 从 0.3/0.4/0.2/0.1 改为论文 λ=1 等权（desc + intent + ppr）
    - **P1-2 PPR 图传播生效**：`discover_server_level` 从一跳邻域冒充改为真正调用 `GraphScorer::ppr_propagate`，构建异构图邻接表从根意图种子节点传播
    - **P1-4 语义嵌入生成**：AssetSyncEngine 为所有节点（资产/意图）调用 `embed_text` 生成语义嵌入，使描述匹配走余弦相似度而非 Jaccard 词重叠
    - **验证**：`cargo check` 零错误，`cargo test --lib intention_graph` 16/16 通过，`npx tsc --noEmit` 零错误

  - **v0.20.0 SING 意图图集成：动态 ReAct + 分层发现** (2026-06-21) — 全面集成 arXiv:2606.16591v2 论文的意图-工具异构图理论，实现从"关键词匹配"到"意图驱动"的智能创作调度范式升级。关键变更：
    - **新模块 `intention_graph/`**（11 文件）：`models.rs` 核心数据结构 + `graph.rs` SQLite+内存混合存储 + `builder.rs` 离线意图合成 + `discovery.rs` 分层发现 + `reactor.rs` 动态 ReAct + `planner.rs` 包装 PlanGenerator + `commands.rs` IPC 诊断命令
    - **Migration 95**：6 张新表（intention_nodes / asset_nodes / intention_asset_edges / asset_asset_edges / execution_graphs / execution_nodes）
    - **PlanExecutor 四级回退**：模板匹配 → IntentionGraphPlanner → PlanGenerator → 直接 Writer，零回归风险
    - **模型网关意图感知**：`classify_by_intention()` 将 SING 意图动词映射到 TaskClass（LightTool/BalancedWork/HeavyCreation）
    - **前端诊断面板**：`IntentionGraphDiagnostics.tsx` —— 统计卡片 + 最近执行记录 + 执行图详情钻取，侧边栏新增「意图图」入口
    - **验证**：`cargo check` 零错误，`cargo test --lib intention_graph` 16/16 通过，`npx tsc --noEmit` 零错误

  - **v0.19.0 提示词全面可配置化：70+ 硬编码提示词注册表 + 前端完整覆盖** (2026-06-18) — 彻底消灭所有硬编码提示词，全部纳入统一注册表。关键变更：
    - **注册表扩展**：`prompts/registry.rs` 从 8 个内置 prompt 扩展至 35+，覆盖 15 个分类（Writer / Inspector / Commentator / Planner / Analyzer / Probe / System / Memory / Knowledge / Skill / Methodology / World / Character / Narrative / Other）
    - **雪花法 10 步注入**：`methodology_snowflake_step1` ~ `step10` 全部进入注册表，`prompt_instruction()` 优先查注册表、回退硬编码
    - **技能提示词映射**：`skill_id_to_prompt_id()` 将 5 个内置技能（style_enhancer / plot_twist / text_formatter / character_voice / emotion_pacing）映射到注册表 prompt ID，执行时动态读取覆盖
    - **Memory / Knowledge / MultiAgent 接入**：`extract_narrative_events`、`build_knowledge_graph_prompt`、`multi_agent` 等模块全部改用 `resolve_prompt()`
    - **前端 PromptsPanel 重写**：15 分类折叠面板 + 实时搜索 + 分类筛选 + 批量重置 + 默认内容预览 + 模板变量标签高亮
    - **GeneralSettings 精简**：移除旧版 2 个 textarea 覆盖，改为「提示词注册表」链接卡片
    - **新增 IPC**：`reset_all_prompt_overrides` 一键恢复全部默认
    - **验证**：`cargo check` 零错误，`cargo test --lib` 392/392 通过，`npx tsc --noEmit` 零错误，`vitest run` 126/126 通过

  - **v0.18.1 设置超时修复：数字输入体验 + 配置读取路径** (2026-06-20) — 修复两个关键问题：
    - **前端数字输入过快保存**：后台设置「超时设置」数字输入框从 `onChange` + 300ms 防抖改为本地 state + `onBlur` 保存，用户输入多位数字时不再中途弹出「设置已保存」
    - **后端超时配置不生效**：修复 3 处 `AppConfig::load` 错误使用 `std::env::current_dir()` 而非 `app_handle.path().app_data_dir()` 的问题（`smart_execute` 总超时、`executor_step_timeout` 单步超时、`model_gateway` 探测提示词），用户设置的 600 秒总超时现在真正生效
    - **验证**：`cargo check` 零错误，`cargo test --lib` 444/444 通过，`npx tsc --noEmit` 零错误，`vitest run` 126/126 通过

  - **v0.18.0 后台资产深度审计 × 智能创作流程全面优化** (2026-06-20) — 对后台资产与智能创作流程的关联进行全面深度审计，发现核心矛盾：默认续写路径（TimeSliced）绕过约 90% 后台资产。系统性修复 P0-P3 共 14 项：
    - **P0 断环修复**：ingest→伏笔自动追踪闭环（`persist_foreshadowings`）、character_states 写入闭环（`persist_character_states`）、内置 MCP 自动注册进 CapabilityRegistry、四元组资产完整 payload 展开
    - **P1 核心优化**：TimeSliced 接入精选资产子集（叙事阶段+伏笔+风格摘要+四元组，解决"资产黑洞"）、接通 3 个休眠技能（character_voice/plot_twist/text_formatter 场景智能触发）、Full Inspector 接入全量上下文、审计触发自动 Rewrite 建议（`AuditRewriteSuggested` SyncEvent）
    - **P2 死代码清理**：删除 `prompts/methodologies/`、`evolution/`、`state/`、`PromptManager`、`PromptEvolver`；评点家改用 registry 模板；Migration 94 删除 beat_cards/story_engines/pressure_relationships 死表
    - **P3 架构优化**：Pro/Free 精细化分层（单 StyleDNA+写作风格+作品简介移出 is_pro）、按 genre 自动匹配 GenreProfile、新建 `CreativeAssetSnapshot` 统一资产注入网关
    - **CI 修复**：删除重复的 V092/V093 SQL 迁移文件（修复 48 个测试失败）
    - **文档**：新增审计报告 `docs/AUDIT_后台资产与智能创作流程.md` + 参考文档 `docs/CREATION_FLOW_AND_ASSETS_REFERENCE.md`
    - **验证**：`cargo check` 零错误，`cargo test --lib` 444/444 通过，`npx tsc --noEmit` 零错误，`cargo +nightly fmt -- --check` 通过

  - **v0.17.1 提示词注册表 + 两个紧急 Bug 修复 + 智能后台预访谈 + Anti-AI 闸骨架 + 在世作者保护** (2026-06-19) — 一个综合性版本：
    - **🔴 Bug 1 修复：超时设置保存失败 undefined**：`AppSettingsData` 长期缺失 v0.16.0 引入的 13 个高级字段（frontend_timeout_secs / executor_step_timeout_secs / smart_execute_total_timeout_secs / llm_connect_timeout_secs / llm_first_chunk_timeout_secs / style_weight / narrative_weight / skip_rewrite_threshold / keep_revision_history / context_budget_ratio / generation_mode / writer_system_prompt_override / probe_prompt_override），任何尝试调整超时数字都触发 IPC 反序列化失败。修复：后端补全字段 + 全部 `#[serde(default)]` + 前端 `SettingsContext` mutationFn 读取 query 缓存合并 patch 后下发完整对象。
    - **🔴 Bug 2 修复：模型健康报告不可用**：`ModelHealthReport` 新增 `total_calls` / `last_called_at` / `generated_at`；前端 `useModelHealthReports` 关掉缓存（`staleTime: 0` / `gcTime: 0` / `refetchOnMount: 'always'`）；ModelHealthPanel 头部显示「数据更新于 X」+ 每个模型的「近期调用次数 / 最近一次调用」，让数据新鲜度可见。
    - **🟡 提示词注册表（核心新增）**：把分散在 `prompts/engine.rs` / `llm/prompt.rs` / `task_system/audit_executor.rs` 的硬编码 prompt 全部抽取到统一注册表。关键：
      - **Migration 93** `prompt_overrides` 表（prompt_id PK + overridden_content + updated_at）
      - **`prompts/registry.rs`**：8 个内置 prompt（writer_system / writer_continue / writer_rewrite / inspector_system / style_checker_system / outline_planner / commentator_system / model_gateway_probe），分 6 类（写作核心 / 审校与质量 / 评点 / 规划 / 分析 / 探测）
      - **IPC 命令**：`list_prompt_entries` / `save_prompt_override` / `reset_prompt_override` / `resolve_prompt_content`
      - **前端 PromptsPanel**：Settings 新增「提示词」标签页，按分类折叠分组，每条 prompt 可展开编辑，显示已覆盖 / 未保存状态徽章 + 模板变量列表 + 保存覆盖 / 恢复默认按钮
      - **运行时接入**：`AgentService::resolve_prompt(id)` 优先查 DB override，否则回退默认；Writer / Inspector / OutlinePlanner / Model Gateway 探测全部经 registry 读取
    - **InputClarity 三档判定**：`intent.rs` 新增 `detect_input_clarity()` 启发式（Vague / WithSeed / WithFullConcept），不调 LLM，用字符长度 + 故事元素信号词（角色 / 动作 / 冲突 / 场景 / 关系 / 目标 6 类，约 45 个词）轻量分类。
    - **NarrativeQuartet 透明推断**：新建 `strategy/quartet_inference.rs` —— 当输入处于 Vague/WithSeed 时，后端透明补全 5 元组（emotional_payoff / pressure_relationship / conflict_arena / story_engine / beat_card），不弹卡片，全部走默认 + GenreProfile.reader_promise。
    - **Writer Prompt 注入**：`PlanExecutor::execute_writer` 把序列化后的四元组写入 `task.parameters["narrative_quartet"]`；`build_writer_prompt` 在最终组装阶段调用 `render_narrative_quartet_section()` 追加中文渲染段。
    - **Anti-AI cliché 词表 +7**：`anti_ai/mod.rs::ai_cliches` 新增 关键在于 / 值得注意的是 / 综上所述 / 让我们 / 在某种程度上 / 与此同时 / 这一切的背后。
    - **AntiAiRewriter 骨架**：`anti_ai/rewriter.rs`（新文件）—— `RewriteStrategy`（LocalReplace / ParagraphRewrite / ChapterRewrite）+ `AntiAiRewriter::should_trigger`（overall_score < 60 或任一 high severity）+ `rewrite()` 异步入口（v0.17.1 直接返回原文，v0.17.2 接 LLM）+ 4 个单测。
    - **OpeningClarityGate 骨架**：`audit/opening_clarity.rs`（新文件）—— 6 要素门（Danger / Humiliation / Loss / Puzzle / PhysicalAnchor / GenreSignal），按前 200 字检查；`signal_for_genre()` 为 5 种主流题材（赘婿 / 修真 / 末世 / 悬疑 / 校园）提供差异化检测词；5 个单测。
    - **AuditExecutor 7→11 维**：`task_system/audit_executor.rs` prompt 扩 4 维（desire / payoff / aftertaste / opening_clarity），`dimension_priority` / `dimension_label` 同步更新，2 个新单测。
    - **LivingAuthorGuard**：`creative_engine/style/living_author_guard.rs`（新文件）—— 在世作者黑名单 41 位（中文 26 + 外文 15），命中即替换为「具备相同手工艺特征的写作风格」+ 自动追加「手工艺滑块」段（5 维 × 3 档：句长偏好 / 对话比例 / 比喻密度 / 内心独白比例 / 视角粘度）；`build_writer_prompt` 在最终组装后调用 `sanitize_style_brief()` 自动清洗；6 个单测。
    - **不接入生产的骨架模块**：rewriter.rs / opening_clarity.rs 仅做接口预定义和轻量启发式实现，主创作流程暂未引用，预留 v0.17.2 接入。
    - **遗留诊断**：用户报告「写第二章」200s 假超时（plan-executor-step inspector 事件 + heartbeat elapsed=0）已定位为前后端超时间隔过窄（200s vs 180s）+ 计划步骤超时不受 smart_execute outer timeout 进度保护，转交 v0.17.2 集中修复。
    - **验证**：`cargo check` 零错误（33 warnings 全为既有），`cargo test --lib` 396 passed / 48 failed（48 为 v0.17.0 起的 V092 测试 DB 基线问题，零新回归），新增 28 个单测全部通过（含 5 个提示词注册表单测）。

  - **v0.17.0 中文叙事增强：桥段卡 / 剧情引擎 / 高压关系 / 读者承诺四件套** (2026-06-19) — 引入业界共识级的四类中文叙事创作资产，与既有的方法论 / 体裁画像 / Style DNA 三轴互补：
    - **31 张经典桥段卡**：`creative_engine/beat_cards/`（mod.rs + registry.rs），分 7 大类——跌落与回归 / 公开证明与打脸 / 身份与识别 / 悬疑与真相重构 / 情感拉扯 / 制度与规则压力 / 后台视角与组织讽刺。每张卡含可复用功能 / 何时使用 / 重构提示 / 反例 / 标签五要素，全部使用通用化中文，不绑定特定作品。
    - **21 种剧情引擎**：`creative_engine/story_engines/mod.rs`，正交叙事动力库可组合 2-4 个。每种引擎含核心 payoff / 最佳收束 / 反例 / 适合搭配。
    - **13 种高压关系**：`creative_engine/pressure_relationships/mod.rs`，冲突放大器（真假继承人 / 师徒宗门 / 后台执行者与台前英雄 等）。
    - **体裁读者承诺**：`creative_engine/reader_promise.rs` + Migration 92 `genre_profiles.reader_promise` 字段，9 种基础情绪（爽 / 甜 / 虐 / 恨 / 惊 / 燃 / 怕 / 痛 / 治愈）+ 衍生爽点。43 个内置体裁全部映射，启动期回填，已设置值不会被覆盖。
    - **架构升级**：`AssetKind` +3 变体（BeatCard / StoryEngine / PressureRelationship）；`SelectedStrategy` +5 字段（emotional_payoff / pressure_relationship_id / conflict_arena / story_engine_ids / beat_card_ids）；`StrategyOverrides` 同步扩展支持 UI 锁定；`strategy/asset_catalog.rs` 新增 3 个工厂函数自动并入资产路由。
    - **下一步迭代**：v0.17.1（智能后台预访谈 LLM prompt 扩四元组）/ v0.17.2（反 AI 味自动改写闸 + 开篇清晰度门 + 11 维质量门）/ v0.17.3（在世作家风格信号翻译）。
    - **验证**：`cargo check` 零错误，`cargo test --lib` 357 passed（基线 344 + 新增 13）零回归，`npx tsc --noEmit` 零错误。

  - **v0.16.2 修复后台审计（AuditExecutor）LLM 调用误导前端假超时** (2026-06-18) — 用户输入"写第二章"后诊断卡弹出"最终输出 / async-audit-inspector 完成"：TimeSliced 模式正文生成后 spawn 的 `AuditExecutor` 未列入 silent 白名单，其 `emit_llm_progress` 覆盖了主流程的 "已完成" 事件，让前端误以为主流程仍在跑，最终 200s 假超时。关键变更：
    - **后端扩展 silent_background**：`async-audit-inspector` / `async-insight` / `async-deep-insight` / `background-summary` 纳入静默白名单，跳过 emit_llm_progress 与心跳
    - **前端 mainGenerationCompletedRef**：`handleGenerationStatus` 在 phase="已完成"/"出错"/"已取消" 时置位，后续后台 events 不再重置 sinceLastEvent、不再触发 tick
    - **验证**：`cargo check` 通过，`cargo test --lib` 392/392 通过，`npx tsc --noEmit` 通过

  - **v0.16.1 修复"距上次响应 80006 秒"计数 Bug** (2026-06-18) — `lastEventTimeRef` 初始化 `Date.now()` 若首事件延时较长，其与 `Date.now()` 的差值被解释为"距上次响应"，产生天文数字。修复：`lastEventTimeRef` 初始为 null + tick null guard。

  - **v0.16.0 智能创作参数全面可配置** (2026-06-18) — 所有超时/创作/提示词覆盖参数可从前端设置。GeneralSettings 新增 3 张卡片（创作参数/超时设置/提示词覆盖），AppConfig 扩展对应字段，AppSettings 接口同步更新。

  - **v0.15.2 修复"已完成"事件在错误检测前发射** (2026-06-18) — `emit_progress("completed")` 移至成功路径末尾，仅在 `result.success == true` 且内容非空时发射。失败路径改发 "error"。

  - **v0.15.1 生成阶段提示汉字化** (2026-06-18) — `GenerationPhase::as_str()` 返回中文（准备上下文/候选生成/内容审校/润色改写/最终输出/已完成），底部状态栏与诊断卡均以中文显示。

  - **v0.15.0 模型网关智能调度器** (2026-06-17) — 新增 Capability Profile 系统（`model_capability_profile` 表 + CRUD），Streaming TTFB 基准测试（长+短），TaskClassifier（LightTool/BalancedWork/HeavyCreation），3D 智能评分路由（能力 50%+偏好 30%+拟合 20%），所有 LLM 调用统一经网关路由。

  - **v0.14.4 修复"应用启动后自动进入生成进程"假象** (2026-06-18) — `model_gateway_probe` / `input_hint` / `intent_detection` 纳入 `is_silent_background`，跳过心跳与 emit_llm_progress；前端 `llm-generating-progress` 监听器添加 `isGenerating` 守卫。

  - **v0.14.3 智能创作生成内容根因修复——场景智能路由** (2026-06-17) — 在 v0.14.2 超时防线之上深入查找根因，发现"准备上下文阶段长时间延时后退出"的真正症结：`smart_execute` 路径写死 `GenerationMode::Full`，导致每次续写需要 1 Writer + 2 Inspector + 2 Rewrite = 最多 5 次同步 LLM 调用，对本地 Qwen 累计 250-335 秒**必然超时**。`docs/plans/2026-06-14-time-sliced-intervention-design.md:456` 明确指定 `smart_execute` 默认 `TimeSliced`，但实施时只改了任务系统入口，**漏改了 `smart_execute` 实际走的 `PlanExecutor::execute_writer` 路径**。关键变更：
    - **场景智能路由**：`PlanExecutor::execute_writer` 根据场景动态选择模式——`selected_text` 非空（重写选中）→ Full（含质检），续写或新章首段 → TimeSliced（单次 LLM，30-60s）
    - **AppConfig.generation_mode**：新增配置字段，可选 `auto`/`time_sliced`/`fast`/`full`，前端 GeneralSettings 暴露下拉选择
    - **优先级**：plan 参数 > AppConfig.generation_mode > 场景智能默认
    - **超时优化**：`DEFAULT_LLM_TIMEOUT_SECONDS` 240→120，LlmProfile `max_tokens` 8192→2500
    - **预期效果**：续写从"200-300s 必超时"变为"30-60s 稳定生成"，智能创作功能从不可用变为可用
    - **验证**：`cargo check` 零错误，`cargo test --lib` 392/392 通过，`npx tsc --noEmit` 零错误，`vitest run` 126/126 通过

  - **v0.14.2 智能创作超时退出根因修复——多层超时防线** (2026-06-17) — 从根本上修复智能创作"准备上下文阶段长时间延时后退出且不弹诊断卡片"的系统性问题。经全面检视，根因是多层超时缺失：后端 `smart_execute` 无整体超时、LLM 生成超时按 chunk 刷新可无限挂起、Full 模式 270s 预算是死代码、准备上下文阶段同步 DB 阻塞 worker。关键变更：
    - **smart_execute 整体超时**：函数体提取为 `smart_execute_inner`，外层包裹 180s `tokio::time::timeout`，超时调用 `cancel_all_generations()` 取消所有 LLM 生成
    - **PlanExecutor 单步超时**：`execute_step` 单步 90s 超时，超时记为 failed 但不中断后续批次
    - **激活 Full 模式预算**：Inspector/Rewrite 受 `remaining_budget_secs()` 约束，剩余 <30s 跳过质检
    - **LLM 首字节超时 + 绝对超时**：`read_body_with_generation_timeout` 首字节 min(240s, 60s)，绝对上限 generation_timeout × 1.5，修复 vllm 半挂
    - **spawn_blocking 包裹同步 DB**：`build_agent_context` 中 `CanonicalStateManager::get_snapshot`、`ForeshadowingTracker`、`StoryRepository`；Step 4 风格查询和 `build_selected_strategy`
    - **前端超时 330s→200s**：确保前端总在后端之后超时；超时调用 `llm_cancel_all_generations` 通知后端取消
    - **useBackendActivityListener 状态保护**：`plan-executor-step` failed 状态保持 activity running，避免 invoke reject 前清空 isGenerating
    - **验证**：`cargo check` 零错误，`cargo test --lib` 392/392 通过，`npx tsc --noEmit` 零错误，`vitest run` 126 passed

  - **v0.14.1 后台设置即时更新重构** (2026-06-17) — 引入 `SettingsProvider` 统一后台设置状态层，消除本地 state 与 server state 的双向漂移。关键变更：
    - **统一状态层**：新增 `src/contexts/SettingsContext.tsx`/`settingsContextBase.ts`/`hooks/useSettingsContext.ts`，在 `main.tsx` 全局挂载；
    - **乐观更新与回滚**：所有设置写操作（保存通用设置、创建/更新/删除/激活模型、更新 Agent 映射）均内置 `onMutate` 乐观更新 + `onError` 回滚 + `onSettled` 统一失效；
    - **组件重构**：`GeneralSettings`、`MethodologySettings` 移除本地 `useState/useEffect`，直接绑定 TanStack Query 数据；`AgentConfig`、`UnifiedModelManager`、`ModelModal` 接入 Context；
    - **跨状态同步**：`useUpdateStory` 乐观更新同时刷新 query 缓存与 Zustand `currentStory`；
    - **统一失效范围**：设置 family 内 `settings`/`models`/`agent-mappings`/`model-health-reports` 一并失效，确保跨标签页/跨组件即时同步；
    - **验证**：`npx tsc --noEmit` 通过，修改文件 `eslint --max-warnings 0` 通过，`cargo +nightly fmt -- --check` 通过，`cargo check` 通过。

  - **v0.13.3 诊断卡片安全网：修复「准备上下文」长时间延时后退出但未弹诊断卡片** (2026-06-17) — 根因：诊断卡片仅在 `catch` 块触发，但存在多条静默退出路径（成功路径中返回空内容 / `success: false`、状态流转异常等）。关键变更：
    - **防御性诊断**：在 `handleRequestGeneration` 与 `handleSmartGeneration` 的成功路径中，遇到 `final_content` 为空或 `success: false` 时立即调用 `captureDiagnosticInfo` 并弹诊断卡片；
    - **全局安全网**：新增 `smartExecuteNeedDiagnosticRef` 与 `lastGenerationCancelledRef`，监听 `isGenerating` 从 `true` 到 `false` 的转换，若本次生成曾启动且未被用户主动取消，则兜底弹出诊断卡片；
    - **修复响应判定**：`startElapsedTimer` 不再在启动时就将 `backendEverRespondedRef` 设为 `true`，仅在实际收到后端事件后才标记，提升诊断信息准确性；
    - **验证**：`cargo check` 通过，`cargo test --lib` 392/392 通过，`npx tsc --noEmit` 通过，`NODE_ENV=test npx vitest run` 126/126 通过。

  - **v0.13.2 诊断卡片增强 + 前端自救计时器** (2026-06-17) — 根据首次诊断报告修复多个问题：版本号显示、已用时被意外清空、后端响应判定逻辑 bug；新增 `smartExecuteInFlightRef` 防止 activityStore 提前清空生成状态；`scheduleFallbackPrompt` 改为自我循环的前端自救计时器，即使后端心跳中断也能每 10s 更新已用时；后端心跳改为 `log::warn!` 级别输出，确认心跳是否运行；诊断提示去 Ollama 化，适配 vllm/Qwen 用户。验证：`cargo check` 通过，`npx tsc --noEmit` 通过，`NODE_ENV=test npx vitest run` 126/126 通过。

  - **v0.13.1 修复智能创作卡死在「准备上下文」阶段** (2026-06-15) — 根因：能力进化反馈环 `evolve_capability_descriptions` 未清洗 LLM `<think>` 思考链，被污染的 `when_to_use` 描述注入 PlanGenerator prompt，导致计划生成 LLM 卡死、前端 300s 超时退出。关键变更：
  - **写入清洗**：`capabilities/evolution.rs` 新增 `sanitize_evolved_description()`，剥离 `<think>...</think>` 标签（含未闭合情况）、去 markdown 代码块、300 字符上限、<20 字符拒绝；新增 5 个单元测试
  - **加载防御**：`capabilities/mod.rs` `load_evolved_descriptions()` 过滤含 `<think>` 或超 300 字符的条目，丢弃并告警，防止历史污染数据再次注入
  - **数据清理**：用户机器 `evolved_descriptions.json` 已重置为 `{}`，立即恢复
  - **验证**：`cargo check` 零错误，`cargo test --lib` 392/392 通过（原 387 + 新增 5），零回归

- **v0.9.7 技能与设置参数对智能创作真正生效** (2026-06-13) — 全面修复"项目丰富的技能与后台参数设定没有真正影响小说内容生成"问题，关键变更：
  - **WorkflowConfig 统一从 AppConfig 读取**：`rewrite_threshold` / `max_feedback_loops` / `style_weight` / `narrative_weight` / `skip_rewrite_threshold` / `keep_revision_history` 全部用户可配置，创作路径不再写死
  - **Agent 模型映射前端可用**：新增 `AgentConfig` 组件，可为 8 个 Agent 单独配置 chat / embedding / multimodal 模型；后端 `get_agent_llm_params` 按 Agent 读取模型 profile 的 `temperature` / `max_tokens`
  - **技能参数真正生效**：`SkillParameter.default` 自动合并；`SkillManifest.config` 支持 `temperature` / `max_tokens`；内置技能补充默认 config
  - **Genesis / 创作向导读取配置**：概念生成与第一章使用 active profile 参数；第一章注入 `writing_strategy` 与可配置目标字数；创作工作流 `review_threshold` / `max_iterations` 从配置读取
  - **模型高级参数持久化**：`LlmProfile` 新增 `top_p` / `frequency_penalty` / `presence_penalty`，前端 `ModelModal` 可编辑，OpenAI / Anthropic / Ollama 适配器传递
  - **通用/隐私设置持久化**：`theme` / `language` / `auto_save` / `font_size` / `line_height` / `share_usage_data` / `store_api_keys_securely` 真正保存到 `AppConfig`
  - **风格与场景参数补全注入**：`build_writer_prompt` 注入写作风格详细字段与作品简介；`format_scene_structure` 显式渲染 `setting_atmosphere`
  - **验证**：`cargo test --lib` 323/323 通过，`npm run type-check` 通过，`vitest run` 116 passed / 3 skipped

- **v0.9.4 智能创作进度感知与幕前界面精简** (2026-06-12) — 修复"智能创作进度提示长时间卡住"问题，并进一步精简幕前界面，关键变更：
  - **全局进度监听**：`orchestrator-step` 监听从局部改为全局，智能输入栏（`handleSmartGeneration`）与 `Ctrl+Enter`（`handleRequestGeneration`）均能实时显示写作进度
  - **初始阶段提示细化**：`smart_execute` 上下文加载阶段新增"读取故事信息 / 章节与场景结构 / 世界观、角色与伏笔 / 风格配置"等细粒度事件，避免初始阶段无反馈
  - **意图识别文案优化**：识别明确续写意图时显示"正在续写..."，通用指令显示"正在理解创作意图并执行..."
  - **删除"我学到这些"卡片**：接受/拒绝续写后的学习反馈改为统一 toast 进程提示
  - **完全删除左侧边栏**：移除修订模式、生成古典评点、打开幕后工作室按钮；`FrontstageSidebar` 组件及相关样式已删除
  - **设置入口移至顶部**：在顶部色调设置旁新增设置图标，点击打开幕后工作室
  - **采摘图标与右键菜单重绘**：采摘（Ingest）改为统一 VI 风格漏斗+下箭头 SVG；编辑器右键菜单仅保留剪切/复制/粘贴/全选，并继承全局色调
  - **编译测试通过**：`cargo check` 零错误，`npx tsc --noEmit` 零错误，`vitest run` 116 passed

- **v0.9.4 采摘状态指示器 VI 风格再优化** (2026-06-12) — 针对截图反馈的"灰色 pill 像橡皮擦"问题，进一步美化采摘（Ingest）状态 UI：
  - **移除灰色 pill 容器**：改为与设置/文思/禅模式一致的 28px 圆形透明按钮
  - **简化状态表达**：漏斗图标 + 右下角微型状态点（绿/琥珀/灰）替代双图标并排
  - **全局色调继承**：hover、active、面板背景、边框、文字全部使用 `--parchment`、`--warm-sand`、`--stone-gray`、`--charcoal` 等 CSS 变量
  - **面板风格统一**：下拉面板改为暖色纸张质感圆角卡片，替代原来的深色 slate 面板
  - **图标线条优化**：漏斗 SVG 路径更柔和，stroke-width 调整为 1.75，视觉上更纤细
  - **前端验证**：`npx tsc --noEmit` 零错误，`vitest run` 116 passed，`npm run build` 通过

- **v0.9.4 CI 调整：E2E 不再阻塞整体工作流** (2026-06-12) — 解决 master 构建整体被 E2E 测试标红的问题：
  - 给 `e2e-check` job 添加 `continue-on-error: true`
  - 移动并更新注释：E2E 在缺少真实 Tauri 后端的 Vite dev server 上运行，settings 页 IPC 调用会挂起，因此不作为发布阻塞项
  - 单元测试与构建检查（rust-check / frontend-check / tauri-build）仍是可靠质量门

- **v0.9.5 智能创作补齐采摘（Ingest）闭环** (2026-06-12) — 修复智能创作（`smart_execute` / `AgentOrchestrator::generate`）生成成功后未触发完整采摘的问题：
  - **现状**：查询侧已正常调用（`StoryContextBuilder` → `MemoryOrchestrator::build_memory_pack`），但生成后只写入 `memory_items` / `scene_commits` 摘要，未调用 `IngestPipeline` 提取实体/关系并更新知识图谱
  - **修复**：在 `AgentOrchestrator::generate` 的 `MemoryWriter::write` 成功后，异步启动 `IngestPipeline::ingest`，并将提取到的实体/关系批量保存到知识图谱（`KnowledgeGraphRepository::save_entities_batch` / `save_relations_batch`）
  - **影响**：智能创作续写/生成的内容与 `auto_write` 保持一致，都会进入知识图谱和向量索引，后续查询能检索到最新实体与关系
  - **本地验证**：`cargo check` 零错误，`cargo clippy` 通过，`cargo test --lib` 318/318 通过

- **v0.9.4 构建修复：固定 Rust 1.95.0 并提交 Cargo.lock** (2026-06-12) — 修复 GitHub Actions 在 latest stable Rust 下的 E0119 编译失败，关键变更：
  - **根因**：Rust 1.96（latest stable）与 `time` crate 0.3.47/0.3.48 存在 coherence 冲突，导致 `tracing-subscriber`、`tantivy-common`、`cookie`、`tauri-utils` 等 crate 报 `From<HourBase>` 冲突实现错误
  - **修复**：新增 `rust-toolchain.toml` 固定 Rust 版本为 **1.95.0**；将 `Cargo.lock` 从 `.gitignore` 移除并纳入版本控制，锁定 `time` 在 0.3.47
  - **影响**：CI 与本地构建依赖解析一致，避免 future Rust 版本导致 transitive crate 编译失败
  - **本地验证**：`cargo clippy` 通过（301 warnings 均为既有历史 warning），`cargo test --lib` 318/318 通过

- **v0.9.2 自动创作性能优化** (2026-06-11) — 全面优化自动创作速度与后台任务感知，重点解决"后台任务多"与"创作速度慢"问题，关键变更：
  - **后端并行化**：PlanExecutor 同 batch 步骤 `join_all` 并行；GenesisPipeline 后台阶段将世界观/大纲/角色合并为单一并行步骤，使用 `tokio::join!` 同时调用 LLM
  - **共享状态线程安全**：`GenesisContext.bundle` 升级为 `Arc<RwLock<NarrativeBundle>>`
  - **上下文查询去重**：`StoryContextBuilder` 同一次构建内只查一次 scenes
  - **LLM 调用层优化**：Adapter 缓存复用、读取 `timeout_seconds`、指数退避重试
  - **数据库调优**：SQLite WAL + busy_timeout + synchronous=NORMAL，连接池 5 → 10
  - **前端收敛**：`useBackendActivityListener` 将 6 类事件聚合为单一主 activity；`FrontstageApp` 的 `isGenerating` 与 `backendActivityStore` 对齐
  - **全量测试通过**：`cargo test --lib` 318/318，`vitest run` 124 passed

- **v0.9.1 架构拆分与全面测试覆盖** (2026-06-10) — 完成 Phase 3 架构拆分 + Phase 4 测试覆盖，`cargo check` 零警告，`cargo test` 318/318 通过，前端 `vitest run` 124/124 通过，E2E 32/32 通过。关键变更：
  - **后端架构拆分**：`repositories.rs` 6198 行 → 183 行（24 个 Repository 独立文件）；`models.rs` → 8 个领域子模块；移除 3 个 RESERVED 幽灵模块
  - **前端架构拆分**：`FrontstageApp.tsx` 提取 5 个自定义 hooks（useFrontstageData/Editor/Generation/Wensi/Panels）+ 2 个子组件（HelpPanel/ZenModeExit）
  - **前端单元测试 71 例**：hooks ×4、组件 ×2、工具函数 ×2，全部通过
  - **Rust 核心测试 21 例**：utils/text ×7、utils/file ×3、pipeline/refine ×3、pipeline/review ×3、story_system/scene_service ×5，全部通过
  - **E2E 测试重写 36 例**：从截图驱动转为行为驱动，新建 3 个 spec 文件 + 共享 mock 工具

- **v0.9.0 Brooks-Lint 代码质量重构** (2026-06-08) — 完成第一轮代码质量重构，`cargo check` 接近零警告，前端 `tsc --noEmit` 零错误，Rust 测试 297/297 通过。关键变更：
  - 新增 `db/dto.rs`：18+ 个请求/响应 DTO 从 `models.rs` 独立
  - 新增 `story_system/chapter_service.rs` 与 `scene_service.rs`：业务编排从 Command 层下沉到领域服务
  - 前端 `services/tauri.ts`（1,340 行）拆分为 `services/api/` 下 17 个按域子模块
  - 自定义 `MigrationRunner` + 21 个版本化 `.sql` 迁移文件（V007 ~ V027）
  - `lib.rs` setup 逻辑拆分为独立初始化函数，全局单例补充 SAFETY 注释
  - 为 Repository、Cascade 删除、规范状态等核心模块铺设回归测试，总测试数 264 → 297

- **v0.8.0 模型管理重构 + 浮点数精度修复 + 连接状态增强** (2026-05-29) — 统一模型管理入口 `UnifiedModelManager`；temperature 序列化规范化；连接测试步骤可视化 + 全局连接状态 Store；`commands.rs` 辅助函数提取。

- **v5.6.4 Tauri v2 IPC `rename_all = "snake_case"` 根本修复** (2026-05-08) — 彻底消灭 camelCase↔snake_case 参数不匹配导致的 IPC 静默失败。根因：Tauri v2 默认将 Rust snake_case 自动转换为 camelCase 传给 JS，前端改为 snake_case 后未同步禁用转换，参数全部静默丢弃。修复：157 个 `#[tauri::command]` 全部添加 `rename_all = "snake_case"`（`lib.rs` 63 + `commands_v3.rs` 92 + `subscription/commands.rs` 2）。`cargo check` 零错误，`cargo test` 217/217 通过。
- **v5.6.3 IPC 参数一致性全面修复 + Bootstrap 序列化修复** (2026-05-08) — 修复幕后界面功能不可用的根本原因。Bootstrap 进度卡死：`CharacterElement`/`SceneElement` 添加 `#[serde(default)]` 容错 LLM 省略字段；`BootstrapProgressEvent` 新增 `status` 字段。IPC 参数全面审计：修复 7 处 camelCase↔snake_case 不匹配（前端传参修复）。后端命令参数补全：`run_creation_workflow` mode 映射、`update_story` genre、`create_character`/`update_character` 扩展字段。`cargo check` 零错误，`npm run build` 通过。
- **v5.6.2 设计-实现对齐全面修复 v5** (2026-05-08) — 全面检视并修复 5 项设计-实现差距
  - **前端缓存同步精确化**: `writingStyle` case 同时刷新 `writing_style` 缓存（修复只刷新 `world_building` 的遗漏）；`chapterUpdated` 补充 `['chapters', storyId]` 精确刷新
  - **update_scene 向量索引闭环**: `update_scene` 内联 Ingest 补充 `embed_text_async` → `VectorRecord` → `add_record`，Scene 内容变更后语义搜索可检索；`VECTOR_STORE`/`embeddings` 可见性提升为 `pub(crate)`
  - **storySelected 关联数据自动刷新**: `case 'storySelected'` 补充 8 项关联数据 `invalidateQueries`，消除切换故事时的时序依赖
  - **dataRefresh 完整覆盖**: 补充 `knowledgeGraph`/`characterRelationships` 单独 case
  - **编译优化**: 5 处 dead_code 警告清理，warnings 113→109
  - **编译**: `cargo check` 零错误，`npm run build` 通过

- **v5.6.1 设计-实现对齐全面修复 v4** (2026-05-08) — 全面检视并修复 8 项设计-实现差距
  - **幕前幕后自动关联补全**: `sceneCreated`/`sceneDeleted` 同步刷新 `chapters` 缓存，消除场景-章节关联状态滞后
  - **自适应学习真实反馈**: `record_feedback` 返回 `Vec<LearningPoint>`，同步挖掘真实偏好；前端使用返回结果替代硬编码 mock
  - **前端缓存同步完整覆盖**: `useSyncStore` 新增 `writingStyle`/`storyOutlines`/`foreshadowings` case，所有数据类型修改后自动刷新
  - **Pending vector SQLite 持久化**: Migration 42 创建表，替代 JSON 文件持久化
  - **Workflow 幂等性**: `schedule_execution` 入队前检查 queue/running，防止重复执行
  - **编译**: `cargo check` 零错误，`cargo test` 217/217 通过，`npm run build` 通过

- **v5.5.0 设计-实现对齐全面修复** (2026-05-07) — 全面检视并修复 10 项设计-实现差距
  - **幕前幕后自动关联补全**: `create_world_building`/`update_world_building` 正确发射 `WorldBuildingUpdated` 同步事件；`ChapterRepository::delete` 添加事务清理 `scenes.chapter_id` 外键；`characterDeleted` 按 `storyId` 精准失效缓存
  - **后台自动化闭环**: `auto_ingest_chapter` 成功后写入 LanceDB 向量存储（`embed_text_async` → `VectorRecord` → `add_record`），语义搜索可检索最新写作内容；WorkflowEngine 支持数据库持久化（Migration 41 + `with_pool` + 自动 save/load）；能力进化反馈环闭合（`evolve_capability_descriptions` 自动保存 + `build_default_registry` 加载进化描述 + PlanExecutor 后台触发）
  - **技术债务清理**: 移除 `src-core` 幽灵 crate（54 文件零引用）；同步 `FEATURES.md`/`ROADMAP.md`/`ARCHITECTURE.md` 版本号至 v5.4.1
  - **编译**: `cargo check` 零错误，`cargo test` 217/217 通过，`npm run build` 通过

- **v5.4.1 Bootstrap 编辑器内容丢失修复** (2026-05-07) — 修复创世流程"小说已创建但编辑器无文字"的竞态条件问题
  - `FrontstageEvent::ChapterSwitch` 新增 `content` 字段，后端直接传递生成内容
  - 前端优先使用事件中的 `payload.content`，绕过 DB 查询竞态
  - `chaptersRef` 为空时自动重新查询数据库
  - `final_content` 兜底机制
  - `loadStories` 在生成期间禁止自动 `selectStory`
  - **编译**: `cargo check` 零错误，`npm run build` 通过
- **v5.4.0 向量检索语义化 + QueryPipeline 端到端集成** (2026-05-04) — 从关键词匹配到语义理解的检索升级
  - **OllamaEmbeddingAdapter**: 新增 `embeddings/provider.rs` 中 `OllamaEmbeddingProvider`，支持通过 Ollama API（`nomic-embed-text` / `all-minilm` / `mxbai-embed-large`）获取真实语义嵌入
  - **全局语义嵌入路由**: `embeddings/embedding.rs` 中 `embed_text_async()` 优先查询全局 `EmbeddingProvider`（Ollama/OpenAI），失败 graceful fallback 到本地 FNV-1a 哈希；全局 provider 使用 `tokio::sync::Mutex` 保证跨 async 边界 `Send` 安全
  - **QueryPipeline 语义搜索融合**: `memory/query.rs` 四阶段管线扩展为五阶段——1a token_search + 1b semantic_search（embedding 生成 → `search_with_embedding`）+ 1c `fuse_results` 加权融合（token 权重 0.4 / 语义权重 0.6）+ 2 图谱扩展 + 3 预算控制 + 4 上下文组装
  - **Graceful 降级**: 若用户未配置 Ollama/OpenAI embedding，或 `DbVectorStore` 不支持 `search_with_embedding`，自动回退到纯 token 搜索，零额外配置即可运行
  - **LanceDB 真实向量索引**: `vector/lancedb_store.rs` 已接入 IVF-PQ + Cosine 距离语义检索，`VectorStore` trait 扩展 `search_with_embedding` 接口
  - **测试覆盖**: 新增 6 个 `fuse_results` 单元测试（双侧/仅token/仅语义/去重/空输入/截断），Rust 总测试数 211→217
  - **编译**: `cargo check` 零错误，`cargo test` 217/217，`npm run build` 通过

- **v5.3.1 Bootstrap体验修复 + 幕后数据刷新** (2026-05-03) — 修复四个关键体验问题
  - **Bootstrap重复显示小说开头**: `handleSmartGeneration` Bootstrap完成时不再设置 `generatedText` 幽灵文本，避免与 `ChapterSwitch` 加载的 `chapter.content` 正文叠加
  - **幕后结构要素不显示**: `useSyncStore` 中 `invalidateQueries` 的 queryKey 与 hooks 实际使用的 key 不一致（`world-building`≠`world_building`、`story-outlines`≠`story-outline`），修复后 TanStack Query 缓存正确过期，幕后自动刷新世界观/大纲/角色/场景/伏笔数据
  - **Bootstrap解析失败**: 给所有 `NarrativeElement` 结构体的 `id`/`story_id`/`source` 等字段添加 `#[serde(default)]`，允许 LLM 返回的 JSON 省略后端生成字段，修复 `missing field id` 反序列化错误
  - **Bootstrap生成中断（幕前无正文+幕后无结构要素）**: `StoryContextBuilder::build` 中数据库查询在 Bootstrap 时返回 `Err` 导致 `FirstChapterGenerationStep` 失败；LLM 返回 JSON 缺少 `relationships`/`rules`/`key_locations` 等字段导致后台阶段中断。修复：build 方法查询失败时返回默认值；给所有可能缺失的字段添加 `#[serde(default)]`
  - **续写时重复生成小说开头**: `current_content_preview` 从头部截断 2000 字符，续写后 LLM 看不到续写内容只能看到第一章，于是重新生成开头。修复：从尾部截断 6000 字符保留最新内容
  - **后台数据刷新统一通道**: 后台阶段完成后通过 `StateSync::emit_data_refresh()` 发射标准 `sync-event` 事件
  - **编译**: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过，`cargo tauri build` Windows `.exe`/`.msi`/`-setup.exe` 生成

- **v5.3.0 叙事元素模型重构：创世-拆书同构架构** (2026-05-02) — 将 Bootstrap（生成小说）和拆书（分析小说）统一为可逆的 NarrativePipeline 架构
  - **统一数据模型**: 新建 `narrative/` 模块 — `CharacterElement/SceneElement` 等 + `ElementSource` 枚举区分 Generated/Extracted/UserCreated/Imported
  - **GenesisPipeline**: 7步正向流程（概念→世界观→大纲→角色→场景→伏笔→知识图谱）
  - **AnalysisPipeline**: 7步逆向流程（元数据→世界观→角色→场景→故事线→伏笔→知识图谱）
  - **统一进度系统**: `usePipelineProgress.ts` Hook 替代两套进度系统
  - **统一存储层**: `repositories_narrative.rs` 生产表和参考表数据汇聚到统一表
  - **编译**: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v5.2.2 Bootstrap 两阶段架构重构** (2026-05-02) — 核心体验优化：用户等待从10+分钟缩短到2-3分钟
  - **两阶段执行模型**: `bootstrap.rs` `run()` 拆分为 `run_quick_phase()`（同步：概念+正文，2-3分钟）+ `run_background_phase()`（异步 `tokio::spawn`：世界观/大纲/角色/场景/伏笔/知识图谱，5-8分钟）
  - **即时返回正文**: 生成第一章后立即返回给前端，用户可以开始写作，无需等待后台完善
  - **后台进度感知**: 前端状态栏显示"后台正在完善小说世界..."，完成后 toast "创世完成！所有卡片已生成"
  - **编译**: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v5.2.1 超时修复与白屏修复** (2026-05-02) — 消灭用户报告的两个紧急问题
  - **Bootstrap 超时延长**: 前端 `handleSmartGeneration` 创建新小说超时从 180 秒延长至 **600 秒**，匹配本地大模型多步 LLM 调用实际耗时
  - **进度密度增强**: `bootstrap.rs` 每个 LLM 调用（概念/世界观/大纲/角色/场景/伏笔）前后增加细粒度进度事件，用户实时看到"调用AI→已生成→解析中"
  - **LLM 心跳加速**: `llm/service.rs` 心跳间隔 3 秒→**2 秒**，上限 40 次→**300 次**，消息优化为"正在深度思考中..."
  - **后台窗口白屏修复 v5.2.1**: `show_backstage` 双重维度尺寸微调（width+height）+ JS `html+body` 双重重排 + **800ms 延迟**（原 300ms）+ 延迟期间再次微调；`App.tsx` 立即+300ms 延迟两次 `setRenderKey` 强制 React 重挂载
  - **编译**: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v5.2.0 设计-实现对齐全面完成** (2026-05-02) — 通用 Workflow 引擎 + 能力进化闭环 + 双向同步
  - **`WorkflowScheduler::run_instance` 完整 DAG 执行**: 从空实现到支持 Start→WriteChapter→Inspect→Revise→VectorIndex→AnalyzePlot→End 全节点类型，拓扑有序执行（同层可并行）+ 状态管理 + 上下文变量传递
  - **通用 Workflow IPC 命令**: `register_workflow` / `create_workflow_instance` / `start_workflow_instance` / `get_workflow_instance_status`，setup 时自动注册 `standard_writing_workflow` 模板
  - **能力进化反馈环闭合**: `ExecutionRecordStore` JSON 持久化 + `PlanExecutor` 自动记录每次能力执行 + `evolve_capability_descriptions` LLM 分析生成改进建议
  - **幕前↔场景内容双向同步**: `useSyncStore` chapterUpdated 刷新 scenes 缓存 + `FrontstageApp` 监听 chapter-updated 自动刷新编辑器内容（3 秒防循环保护）
  - **QueryPipeline 降级感知**: 后端 `context-degraded` 事件 + 前端 toast "正在使用简化上下文生成内容..."
  - **废弃组件清理**: `FrontstageToolbar` 从索引导出中移除
  - **编译**: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v5.1.1 设计-实现对齐全面修复** (2026-05-01) — 消灭 P0 差距，补齐 P1 差距，全面达到设计目标
  - **`update_chapter` 保存后自动触发 IngestPipeline**: `auto_ingest_chapter()` 异步后台执行，5 分钟冷却期 + 内容哈希去重，防止 API 成本失控
  - **`state_sync` 空 story_id 修复**: character/chapter update/delete 先查询 `story_id` 再发射同步事件，前端缓存精准刷新
  - **`FrontstageToolbar` story_id 传递**: 废弃组件修复 `show_backstage` 参数传递
  - **`WorkflowScheduler` 队列机制**: 从空实现改为 `VecDeque` 内存队列 + `execute_next()` 串行执行
  - **`PromptLibrary` 扩展**: 新增 StyleChecker + Commentator 系统提示词模板
  - **方法论模板库**: 新建 `prompts/methodologies/` — 雪花法 10 步 + 英雄之旅 12 阶段 + 场景结构 3 变体
  - **编译**: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过，`cargo tauri build` Windows `.exe`/`.msi`/`-setup.exe` 生成

- **v5.1.0 幕前幕后自动关联对齐** (2026-05-01) — 从"各自为战"到"自动联动"的全面升级
  - **Phase 1.1 Chapter↔Scene 双向映射**: `chapters.scene_id` + `scenes.chapter_id` 外键关联，ChapterRepository 自动创建/关联 Scene
  - **Phase 1.2 统一实时状态中心**: 后端 `state_sync` 模块 + 16 种 `SyncEvent`，所有数据修改命令自动发射同步事件
  - **Phase 1.3 Bootstrap 自动加载**: `smartExecute` 返回后前端自动加载新故事并切换到第一章，Bootstrap 完成后双重 `ChapterSwitch` 保险
  - **Phase 1.4 幕前→幕后快速跳转**: `Ctrl+Shift+B` 快捷键 + 标题栏点击，幕后自动定位当前故事
  - **Phase 2.2 自适应学习闭环修复**: `record_feedback` 成功后异步触发 `mine_preferences`，偏好挖掘自动激活
  - **Phase 2.4 AgentOrchestrator 闭环接入**: `execute_writer` 集成 `AgentOrchestrator::execute_write_with_inspection`，Writer→Inspector→StyleChecker→Writer 质检改写生效
  - **Phase 3.1 Zustand↔TanStack Query 同步**: `App.tsx` 监听 `currentStory` 变化，自动刷新关联数据缓存
  - **Phase 3.2 窗口通信事件标准化**: `DataRefresh` 统一由 `useSyncStore` 处理，消除重复刷新
  - **编译**: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v5.0.0 创世引擎：一键创世，万物关联** (2026-04-30) — 从"一键生成开头"到"一键生成完整小说世界"
  - **故事大纲自动生成**: `story_outlines` 表 + LLM 生成 3 幕结构大纲（标题/摘要/情节点/预估场景数）
  - **角色完整性格小传**: `characters` 表新增 appearance/gender/age，bootstrap 完整填充 personality/goals/appearance
  - **角色关系图谱**: `character_relationships` 表 + 前端"关系"标签页，展示角色间朋友/敌人/恋人/师徒等关联
  - **伏笔自动埋设**: Bootstrap 基于大纲识别 3-5 个核心伏笔，自动关联第一章场景
  - **知识图谱自动构建**: 创世时为角色/场景/伏笔自动创建 KG 实体和关系
  - **前后台智能联动**: Bootstrap 完成后自动发送 `NavigateTo` 事件，幕后切换到 Stories 并高亮新故事
  - **故事概览面板**: Stories.tsx 新增"概览"视图，展示大纲/角色/场景/伏笔总览
  - **7步创世工作流**: 构思故事 → 撰写开篇 → 构建世界 → 生成大纲 → 塑造角色 → 铺设场景 → 埋设伏笔 → 编织关联
  - **Migration 34/35/36**: story_outlines / characters增强+relationships / scenes.foreshadowing_ids
  - **Bug 修复 v3（热修复）**: 后台窗口白屏 + 后台卡片显示修复
    - **白屏根因**: WebView2 窗口 `hide()` 后重新 `show()` 时渲染表面可能丢失；JS 强制重排不够可靠
    - **白屏修复 v3**: `show_backstage` 命令**微调窗口大小再恢复**（`width+1` → `width`），强制 WebView2 重新创建渲染表面；配合 JS 强制重排；延迟 300ms 发射 `backstage-shown` 事件确保前端监听器就绪
    - **卡片不显示根因 v3**: (1) Bootstrap 完成时 backstage 被隐藏，事件丢失; (2) `DataLoader` 与 `App.tsx` 同时加载 stories 造成**竞态条件**; (3) Bootstrap LLM 调用失败时错误被 `log::warn` 吞掉，前端完全不可见
    - **卡片修复 v3**: `DataLoader` **移除 stories 查询**，完全由 `App.tsx` 控制数据加载，消除竞态 → `App.tsx` 引入 `useQueryClient`，`handleWindowShown` 中主动 `invalidateQueries` 强制刷新所有页面数据 → `bootstrap.rs` LLM 调用失败时发射 `novel-bootstrap-error` 事件到前端，让错误可见
  - 编译: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v4.5.0 进程提示栏超时深度修复：消灭"系统仍在处理中"黑洞** (2026-04-30) — 从"不知道在等什么"到"每一步都可见"
  - **根因定位**: `build_writer_prompt` 是同步函数，内部包含大量数据库查询 + `block_on` 调用，但**零事件输出**。用户在 0.15→0.20 之间等待 5-30 秒无反馈，前端 fallback timer 超时退出
  - **async 化**: `build_writer_prompt` → `async fn`，移除危险的 `tauri::async_runtime::block_on`（在 Tauri 异步运行时中可能导致死锁/线程阻塞）
  - **密集事件**: 在 `build_writer_prompt` 内部插入 15+ 个新事件（0.150→0.195），覆盖：策略配置→模板变量→系统提示词渲染→策略约束注入→方法论→风格 DNA→个性化偏好→叙事状态快照（故事/场景/冲突/伏笔/角色）→最终组装
  - **`tokio::task::yield_now().await`**: 每个子步骤之间 yield，确保事件循环有机会将 IPC 事件发送到前端
  - **AdaptiveGenerator 细分**: 0.281"查询用户反馈历史"、0.285"计算生成策略"
  - **前端图标映射**: 补充"读取/渲染/准备/查询/计算"→Brain，"注入/组装"→Cog
  - **状态栏宽度**: `generation-status-text` max-width 200px→600px + `flex-shrink: 0`；最终移到输入框 pill 下方独立行 `generation-status-row`，占满 900px 宽度
  - 编译: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v4.4.0 3风格三角框架：通用风格混合系统** (2026-04-28) — 从单一风格到多风格融合的创作革命
  - **通用风格混合系统**: `StyleBlendConfig` 支持任意 2-5 个 StyleDNA 按权重组合，主导/辅助角色自动分配，权重实时归一化
  - **3风格三角创作框架**: 新增普鲁斯特（意识流/长句/内心独白70%）+ 马尔克斯（魔幻现实/全知视角）内置风格，与现有海明威形成完整三角
  - **混合风格 Prompt 注入**: 主导风格完整注入，辅助风格仅注入关键差异维度；融合规则明确"主导定基调，辅助渗精神"
  - **防漂移自检清单**: 5项检查（句长/对话比/比喻密度/内心独白/情感外露），加权平均目标 ± 容差，总体匹配度评分
  - **章节级风格控制**: `scenes.style_blend_override` 支持每章独立配置，前端 Stories.tsx 双标签页（单一风格/风格混合）
  - **数据层**: Migration 30/31 新增 `story_style_configs` 表 + `scenes` 覆盖字段；4 个新 IPC 命令
  - 编译: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v4.3.0 智能交互创作流程深度优化** (2026-04-27) — 从"能创作"到"懂创作"的全面升级
  - **一键创作体验升级**: Bootstrap前端实时显示5步进度（构思→世界观→角色→场景→撰写）；创建完成后自动切换新故事并加载第一章；Chapter/Scene双轨同步确保前端零延迟加载
  - **模型驱动编排全面落地**: 彻底移除`detect_and_route_intent`关键词匹配，所有用户输入交由PlanGenerator自由理解；PlanContext增强注入世界观摘要、角色列表、活跃伏笔、风格DNA、MCP可用工具
  - **设定修改智能响应**: 新增`update_character`/`update_world_building`/`update_scene`能力，LLM解析用户修改意图自动更新后台设定；场景修改自动标记`needs_rewrite`，续写时自动重写受影响内容
  - **MCP与技能自动化**: CapabilityRegistry注册MCP工具，PlanGenerator知道何时调用外部工具；内置技能（style_enhancer/character_voice/emotion_pacing）可由模型自主编排
  - **PlanGenerator Prompt进化**: 新增技能调用指南、设定修改指南、MCP工具使用指南、伏笔处理指南（Rule 12-18）
  - 编译: `cargo check` 零错误零警告，`cargo test` 183/183，`npm run build` 通过
  - 新增测试: planner/bootstrap 7个（JSON提取/概念序列化）、planner/executor 4个（参数解析）、planner/mod 4个（PlanContext/PlanStep）
  - 修复: bootstrap.rs 编译警告、第一章 prompt 增强（注入题材/基调/简介）

- **v4.2.0 智能交互设计重构 V2：模型驱动的编排范式** (2026-04-23) — 从程序式编排转向模型式编排
  - **核心理念**: 人类只定义能力能做什么（自然语言描述），模型负责编排（什么时候用、怎么用、按什么顺序）。移除所有关键词匹配、意图分类枚举、if/else 分支判断用户意图。
  - **CapabilityRegistry（能力自描述系统）**: Agent 和 Skill 用自然语言描述自己（`description` / `when_to_use` / `input_description` / `output_description`），模型阅读描述自主选择。人类不再写死 Agent 映射规则。
  - **PlanGenerator（模型计划生成器）**: 取代 IntentParser + IntentExecutor。LLM 接收系统状态 + 用户输入 + 能力清单，自主输出执行计划（自由文本理解 + 步骤列表 + 参数 + 依赖关系）。
  - **PlanExecutor（计划执行引擎）**: Dumb executor，忠实执行 LLM 生成的计划。按顺序执行步骤、传递输出、处理失败。所有决策已在计划中。
  - **PromptEvolver（提示词进化器）**: LLM 根据故事上下文（题材、叙事阶段、用户偏好）自由改写整个 prompt。不是模板变量替换，而是真正的"进化"。
  - **AiLearningIndicator（记忆显性化）**: 前端组件，每次 AI 交互后展示"系统学到了什么"。让"越写越懂"对用户可见。（注：v0.9.4 已移除该卡片式提示，改为统一 toast 进程提示。）
  - **CapabilityEvolutionEngine（能力进化反馈环）**: 记录能力调用结果，长期优化能力描述准确性。
  - **PlanTemplateLibrary（计划模板学习）**: 记录成功执行计划，类似请求复用或微调。
  - **移除的程序式规则**: IntentType 枚举（11 类预设分类）、前端正则关键词检测、IntentExecutor.map_agents 写死映射、`if (!currentStory)` 强制报错流程。
  - **前端简化**: `handleSmartGeneration` / `handleRequestGeneration` 统一走 `smart_execute`，用户任何输入都交给模型决定。
  - 编译: `cargo check` 零错误零警告，`cargo test` 160/160，`npm run build` 通过

- **v4.1.0 幕前界面深度重构：化整为零，萤火随行** (2026-04-22) — P0+P1+P2 全流程体验重构
  - **设计理念**: 从 20+ 可见 UI 元素缩减至 <5 持久元素。AI 功能以萤火暗示（firefly hints）形式按需浮现，用完即隐。"创作者不应在工具中花费精力标注自己的创作"——移除所有显式注释/评论创建 UI。
  - **P0 核心重构 (4 项)**:
    - 顶栏精简: 44px 细线设计，小说标题（点击进入幕后）、字数统计、字号调节、🔥 文思三态切换（off·/passive✨/active🔥）、禅模式。移除汉堡菜单、订阅徽章、"开启文思"按钮、"AI 续写"按钮、主行动按钮。
    - 底栏删除: 彻底删除底部聊天工具栏（chat input、模型状态点、WenSiPanel 嵌入、Slash textarea 菜单）。AI 结果以幽灵文本（ghost text）内联呈现，Tab 接受/Esc 拒绝。
    - 侧边栏精简: 5 按钮→2 按钮：修（修订模式）/ 批（生成古典评点）/ 幕（幕后）。移除注释和评论显式 UI。（注：v0.9.4 已进一步完全移除幕前左侧边栏，设置入口并入顶部状态栏。）
    - 键盘快捷键: `Ctrl+Enter` / `Cmd+Enter` 全局触发续写，`Ctrl+Space` 循环文思模式，`F11` 禅模式。
  - **P1 萤火系统 (3 项)**:
    - 幽灵文本: 编辑器末尾灰色斜体段落（`opacity: 0.35`），附带萤火操作栏（Tab 接受 / Esc 拒绝）。
    - 右边缘萤火: `smartGhostText` 从右侧淡入（0.8s）→ 停留 → 淡出（1.2s），不打扰写作流。
    - 空态引导: 编辑器无内容时居中显示诗意提示"开始写下第一句话，文思将随你而行"。
  - **P2 体验优化 (4 项)**:
    - 内联 `/` 命令菜单: 8 命令（续写/润色/古风/场景/自动续写/审校/评点/排版），光标处触发，方向键导航，回车执行，Esc 关闭，自动删除 `/` 字符。
    - WenSiPanel 浮动化: 从底栏嵌入改为 FrontstageApp 右下角浮动卡片，通过 `/` 菜单高级命令触发。
    - 修订横幅精简: 从多行可展开缩减为 32px 单行，变更列表可滚动，默认折叠。
    - 古典评点保留: AI 生成的段落评点（金圣叹式朱批）保留为内联段落，朱红色 `oklch(55% 0.18 25)`，霞鹜文楷字体，左边框红色，※ 前缀，缩进 3em。
  - **移除（设计决策）**:
    - 显式注释系统: sidebar "注"按钮、注释/评论面板、选中文本弹窗创建按钮、右键菜单项、所有相关 hooks（`useTextAnnotations`、`useCommentThreads`）。
    - 原因: AI 写作工具不需要创作者标注自己的作品；AI 反馈应以幽灵文本或古典评点形式自然呈现。
  - 编译: `cargo check` 零错误零警告，`cargo test` 160/160，`npm run build` 通过

- **v4.0.1 全面代码审计与空实现修复** (2026-04-22) — Phase A+B
  - **Phase A: 代码审计与 P0 修复 (15+ 项)**:
    - 综合审计: 扫描 40+ 模块，输出 `CODE_AUDIT_REPORT_V4.md`（5 严重/17 参数/9 空实现）
    - IPC: 统一 17 处 camelCase→snake_case 参数名，修复 Tauri v2 反序列化静默失败
    - 空实现补全: `analytics` 真实统计、`agents/commands` 真实状态、`skills/executor` 真实 MCP 调用、`export/import_from_text` 正则解析、`workflow/scheduler` 执行日志、`evolution/updater` manifest CRUD、`mcp/server` 缺失 `.await`
    - 前端修复: `settings.ts` 移除硬编码密钥、`useCollaboration.ts` WebSocket 真实发送、`useStreamingGeneration.ts` 移除 mock、`textAnalyzer.ts` 增量分析
    - UI: 聊天工具栏从 absolute 改为正常流、编辑器 padding 优化
    - 类型统一: `skills/mod.rs` 移除重复 `McpServerConfig`
  - **Phase B: 内存模块 SQLite 持久化 (3 模块)**:
    - Migration 26/27/28: `chat_sessions`/`chat_messages`、`story_runtime_states`、`collab_sessions`/`collab_participants`
    - `chat/mod.rs`: `ChatManager` 改为 `DbPool` 持久化
    - `state/manager.rs`: `StoryStateManager` 改为 `DbPool` 持久化
    - `collab/mod.rs` + `websocket.rs`: `CollabManager` 持久化 + 完整消息处理闭环（Join/Leave/Operation/Cursor/Participants）
  - 编译: `cargo check` 零错误零警告，`cargo test` 160/160，`npm run build` 通过

- **v4.0.0 借鉴 AI-Novel-Writing-Assistant 全面优化** (2026-04-22) — Phase 1+2+3 共 9 项新功能
  - **Phase 1: P0 核心能力 (3 项)**:
    - Canonical State: 新增规范状态系统，统一聚合 StoryContextBuilder/character_states/foreshadowing/KG 等分散状态，AI 续写时准确知道"当前处于故事哪个阶段"
    - Payoff Ledger: 升级 ForeshadowingTracker 为伏笔账本，新增时间窗口追踪(target_start/target_end)、逾期检测、风险信号、回收时机智能推荐
    - Execution Panel: 新增章节执行面板，智能推荐下一步行动（"处理逾期伏笔"/"续写"/"运行审校"），集成到 Scenes.tsx 和 FrontstageApp
  - **Phase 2: P1 质量与控制 (3 项)**:
    - Narrative Phase Detection: 增强叙事阶段检测（逾期伏笔→ConflictActive、高置信度长内容→Climax、主要伏笔回收→Resolution），注入 Writer prompt
    - Structured Outline: Scene 模型新增 execution_stage/outline_content/draft_content，SceneEditor 重写为 6 标签页（规划/大纲/起草/审校/定稿/批注）
    - Audit System: 新增统一审计模块，整合 ContinuityEngine/StyleChecker/QualityChecker/PayoffLedger，五维评分（连续性/人物/风格/节奏/伏笔），支持 light/full 审计
  - **Phase 3: P2 体验优化 (3 项)**:
    - Novel Creation Wizard: 新增 5 步小说创建向导（创意→世界观→角色→文风→首个场景），每步提供 AI 生成选项
    - Enhanced Streaming: StreamOutput 组件增强（Markdown 渲染、实时字数、停止按钮、打字机效果），接入 FrontstageApp/WenSiPanel/CreationWizard
    - Strategy Configuration: Settings 新增写作策略配置（运行模式/冲突强度/叙事节奏/AI 自由度），动态注入 Writer prompt
  - 编译: `cargo check` 零错误，`cargo test` 160/160，`npm run build` 通过

- **v3.7.1 智能化创作系统 5 阶段重构深度修复** (2026-04-22) — Phase A+B+C 共 15 项修复
  - **Phase A: P0 核心断裂修复 (5 项)**:
    - QueryPipeline: `graph_expansion` 内容分词后逐 token 匹配实体，修复图谱扩展永不命中的 bug
    - QueryPipeline: `budget_control` 修复内层 break 只跳出内层循环的预算泄漏 bug
    - ContinuityEngine: `check_world_rules` 修复检查方向——从"检测规则描述片段"改为"提取禁止条款后检测"
    - ContinuityEngine: `get_character_states` 效率优化（O(N×M)→O(N+M)），`check_character_locations` 增强跨场景位置检测
    - PreferenceMiner: `record_feedback` 成功后异步触发 `mine_preferences`，自适应学习闭环激活
    - StyleChecker: 接入 `AgentOrchestrator` 闭环，Writer→Inspector→StyleChecker→Writer 风格校验生效
    - Ingestion: 实现真正的内容保存（Chapter 创建/更新）+ 简化知识图谱实体提取，工作流闭环完成
  - **Phase B: P1 功能补全 (6 项)**:
    - 方法论: Migration 22 添加 `methodology_id`/`methodology_step`，Settings 页面新增创作方法论配置
    - 创作模式: `CreationWorkflowEngine` 按 `CreationMode` 分支（AI全自动/AI初稿+精修/人工初稿+润色）
    - 进度反馈: 前端 `useWorkflowProgress` Hook + Stories.tsx 进度弹窗（阶段名称+百分比+指示器）
    - Orchestrator 事件: 前端监听 `orchestrator-step` 实时状态（生成→质检→改写），Settings 暴露阈值/循环数配置
    - AdaptiveGenerator: `calculate_temperature` 累加而非覆盖，pacing/style 偏好微调生效
    - 反馈记录: AiSuggestionNode + WenSiPanel 接入 `record_feedback`，覆盖内联建议/自动续写/自动修改
  - **Phase C: P2 优化 (4 项)**:
    - StyleAnalyzer: 新增 `analyze_with_llm` + `analyze_style_sample` IPC，Stories.tsx 新增"从文本生成风格"
    - QualityChecker: 新增 `check_with_llm`，Review 阶段优先 LLM 评估、回退规则评估
    - PhaseWorkflow: 硬编码阶段逻辑迁移到配置驱动，`PhaseWorkflow` 配置系统激活
    - 增量 Context: 每阶段完成后关键产出回注 `AgentContext`（Conception→world_rules, Outlining→scene_structure）
  - 编译: `cargo check` 零错误，`cargo test` 145/145，`npm run build` 通过

- **v3.6.1 全面功能审计与深度修复** (2026-04-22) — P0+P1+P2 共 30 项修复
  - **P0 紧急修复 (10 项)**:
    - DB: Migration 21 补全 scenes/kg_relations `confidence_score` 缺失列，消除运行时崩溃
    - IPC: 统一 25 处 camelCase→snake_case 参数名，修复 Tauri v2 反序列化失败
    - 场景: `create_scene` 后端扩展参数，前端传参不再静默丢弃
    - Orchestrator: 修复 Rewrite 事件错误携带初稿分数的 bug (`writer_result.score` → `rewrite_result.score`)
    - 技能: `execute_skill` 注入真实 `StoryContext`，`SkillExecutor` 实现真正 LLM 调用
    - 自适应学习: FrontstageApp accept/reject 接入 `record_feedback`，FeedbackRecorder 数据源激活
    - 审计: `LlmService::generate` 完成后调用 `log_ai_usage`，AI 调用日志写入数据库
    - 配额: auto_write/auto_revise 错误处理识别配额关键字，触发 Toast 提示
  - **P1 功能补全 (8 项)**:
    - ContinuityEngine: 补全 timeline + character_emotion + relationship 检查，5/5 全部实现
    - 一键创作: `CreationWorkflowEngine` 每阶段发射 `workflow-progress` 事件 + QualityReport 填充
    - SceneRepository: 新增 5 个单元测试（create/get/update/delete/reorder），Rust 测试 139→144
    - hooks/index.ts: 补全 `useCommentThreads` 等 6 个 Hook 导出
    - 类型: `ChangeTrack.scene_id` 改为 `string | undefined`，与后端 `Option<String>` 对齐
    - 评论: RichTextEditor 已解决评论支持「重新打开」
    - 变更追踪: 修订模式增加单条 change 独立接受/拒绝按钮
    - 清理: 移除弃用 `check_ai_quota` IPC 注册
  - **P2 优化 (6 项)**:
    - 概念统一: Sidebar `chapter_count` 显示从"场景"改为"章"
    - 滑块: SceneEditor 置信度 `step` 从 0.05 改为 0.1
    - 拆书转故事: 人物 background 合并 personality + appearance，场景 summary 保存为 content
    - 伏笔看板: 幕后新增 Foreshadowing 页面，支持 setup/payoff/abandoned 状态管理
    - 技能 Hook: 6 个关键业务点（create_chapter/character/scene、AI write、world_building update）激活 Hook 调用
    - 孤儿表: 评估 `world_rules`/`settings`/`character_states`，保留兼容
  - 编译: `cargo check` 零错误，`cargo test` 144/144，`npm run build` 通过

- **v3.5.2 全功能落地：剩余 7 项修复完成** (2026-04-22)
  - #17 auto_revise 取消/进度事件：后台任务模式 + 4 阶段进度 + 取消支持
  - #20 confidence_score：Scene 类型补全 + SceneEditor 置信度滑块
  - #16 MCP 持久连接：全局连接池 + disconnect/get_connections + DuckDuckGo 真实搜索
  - #19 一键创作按钮：Stories 页面入口 + run_creation_workflow 调用
  - #18 StyleDNA UI：stories 表 style_dna_id + 前端选择模态框 + 创作注入
  - #15 技能系统补全：execute_skill 异步 LLM 调用 + 2 个缺失技能（角色声音/情感节奏）
  - #14 意图引擎接入：RichTextEditor 聊天栏 parseIntent → 路由 → executeIntent
  - 139 Rust tests + 前端构建全部通过，版本号统一 3.5.2

- **v3.5.1 全面功能审计与修复** (2026-04-22) — 13 项关键修复
  - 自动修改: 结果应用到编辑器 + 保存到数据库
  - 拆书: 书名/作者持久化、convert_to_story story_id 修复、store_embeddings、进度 100%、心跳闪烁修复
  - 场景模型: scene_versions 表生产环境补建、conflict_type 列索引修复、版本快照全字段检测
  - AI 核心: AgentOrchestrator 闭环集成、ContinuityEngine/ForeshadowingTracker 写作流集成、AdaptiveGenerator 动态参数应用、auto_write Ingest 触发
  - Inspector: JSON 结构化输出 + 三层解析增强
  - LLM: 取消机制实现、useLlmStream 真实流式
  - StyleDNA: 内置风格自动种子化、CreationWorkflowEngine 暴露命令
  - 测试: Rust 139 全部通过，前端构建通过，已推送 GitHub

- **v3.5.0 拆书体验升级** (2026-04-21) — 进度提示 + 取消支持
  - 后端: `BookAnalyzer` 5 步 Pipeline 每个子步骤发送详细进度，人物/章节逐块汇报
  - 前端: `AnalysisProgress` 8 步骤指示器 + 百分比 + 块处理信息，告别"只见转圈"
  - 取消: `TaskExecutionContext.is_cancelled()` + analyzer 循环检查 + `cancel_book_analysis` IPC
  - 数据库: `reference_books` 新增 `task_id` 字段 + Migration 18
  - 测试: Rust 139 全部通过，前端构建通过

- **v3.4.0 智能化创作系统** (2026-04-18) — 5 阶段重构
  - Phase 1 地基重构: `StoryContextBuilder` 真实 DB 上下文, `QueryPipeline` 四阶段检索, `ContinuityEngine`, `ForeshadowingTracker` — 27 tests ✅
  - Phase 2 方法论注入: 雪花法/场景节拍/英雄之旅/人物深度 + `MethodologyEngine` + `AgentOrchestrator`(Writer→Inspector 闭环) — 34 tests ✅
  - Phase 3 风格深度化: `StyleDNA` 六维模型, `StyleAnalyzer`, `StyleChecker`, 10 经典作家 DNA, `StyleDnaRepository` — 45 tests ✅
  - Phase 4 自适应学习: `FeedbackRecorder`, `PreferenceMiner`(5 维启发式), `AdaptiveGenerator`(动态 temperature/top-p), `PromptPersonalizer` — 54 tests ✅
  - Phase 5 工作流闭环: `CreationWorkflowEngine`(7 阶段), `QualityChecker`(4 维评估) — 63 tests ✅
  - 版本号统一 3.3.0→3.4.0，Logo 生成全平台图标包

- **Freemium 付费系统** (2026-04-18)
  - 后端: `subscriptions`/`ai_usage_logs` 表 + `SubscriptionService`（v0.7.3 移除配额计量，改为功能订阅开关）+ Tauri IPC 命令
  - 前端: `useSubscription` Hook + `SubscriptionStatus` 指示器 + `UpgradePanel` 付费引导 + 功能解锁提示
  - 策略: "功能订阅制" — Free 用户可用基础写作/场景/角色/知识图谱；Pro 解锁 Pipeline（Refine/Review/Finalize）/ 拆书 / 自动续写 / 自动修改
  - Agent 分层: 免费版 max_tokens 1000 + 简化 prompt；专业版完整能力
  - 优化: `has_feature_access` 细粒度权限 / `AppError::SubscriptionRequired` 错误码 / session 冷却 / 离线缓存 / 防抖修复 — 9 项

- **幕前排版与 AI 续写优化** (2026-04-17)
  - 段落间距收紧 + 首行缩进 2em，底部栏 padding-bottom 增至 10rem
  - 自动续写：接受 AI 生成后自动触发下一轮续写
  - Zen 模式绝对纯净：隐藏所有 AI UI 元素

- **TaskService 全局共享修复 + 集成测试建设** (2026-04-19)
  - 关键 Bug: `TaskService` 未全局共享 → 每个 command 新建实例 → `BookDeconstructionExecutor` 丢失 → 拆书功能不可用
  - 修复: `TaskService<R: Runtime>` 泛型化 + 手动 `Clone` + `app.manage(task_service)` + `State<'_, TaskService>`
  - 缓存修复: `useSetActiveModel` `invalidateQueries({ queryKey: ['settings'] })`
  - 单元测试: Rust 71 新增（settings 16 + task_system 13 + repositories 14 + validation 20）+ 前端 21 新增
  - 集成测试: Rust 5 新增（executor registry 共享、任务生命周期、调度器、无执行器失败、拆书去重）
  - 测试总计: Rust 139 + 前端 21 = 160 tests 全部通过

- **拆书功能 + 任务系统 + 向量化存储** (2026-04-19)
  - 后端: `book_deconstruction` 模块 — parser/chunker/analyzer/repository/service/commands
  - 前端: `BookDeconstruction` 页面 + 6 个子组件 + `useBookDeconstruction` Hooks
  - 任务系统: `task_system` 模块 — models/repository/scheduler/heartbeat/executor/service/commands (8 IPC 命令)
  - 拆书改为 `BookDeconstructionExecutor` 任务执行，心跳保活 + 进度推送
  - 向量化: 场景/人物 embedding 自动生成并入库 LanceVectorStore
  - 数据库: 5 张新表 (tasks + task_logs + reference_books + reference_characters + reference_scenes) + 9 个索引 + Migration 16/17

- **拆书功能** (2026-04-19)
  - 后端: `book_deconstruction` 模块 — parser/chunker/analyzer/repository/service/commands
  - 前端: `BookDeconstruction` 页面 + 6 个子组件 + `useBookDeconstruction` Hooks
  - 支持 txt/pdf/epub 解析，三层 LLM 分块分析策略，生成小说类型/世界观/人物/章节/故事线
  - 一键转为故事项目，参考素材库独立存储，向量化接口预留
  - 新增 3 张数据库表 + 4 个索引 + Migration 16，6 个单元测试

- **任务系统 + 拆书改任务 + 向量化存储** (2026-04-19)
  - 后端: `task_system` 模块 — models/repository/scheduler/heartbeat/executor/service/commands (8 IPC 命令)
  - 前端: `Tasks` 页面 + `useTasks` Hooks，状态分组/心跳指示器/进度条/执行日志
  - tokio::time 调度器支持 once/daily/weekly/cron，每任务互斥锁防重叠，心跳检测60秒扫描
  - 拆书分析改为 `BookDeconstructionExecutor` 任务执行，每步分析后心跳保活
  - 向量化存储接入 LanceVectorStore：场景/人物 embedding 自动生成并入库
  - 新增 2 张数据库表 (tasks + task_logs) + 5 个索引 + Migration 17

### 编译状态

- `cargo check` ✅ | 警告: 0（新增 `LearningPoint` 结构体，`RecordFeedbackRequest` 字段 `scene_id`/`chapter_id` 预留未读警告已存在）
- `cargo check --release` ✅ | 警告: 0
- `cargo test` ✅ 217/217
- `npm run build` ✅
- `npm run build` ✅
- `cargo test` ✅ 193/193

---

## [v5.1.0] - 幕前幕后自动关联对齐

### 核心升级
- **Chapter↔Scene 双向映射**: 自动关联，幕前切换章节同步切换场景
- **统一实时状态中心**: 所有数据修改自动同步，前后台零延迟对齐
- **Bootstrap 自动加载**: 创世完成后幕前自动加载新故事并切换到第一章
- **AgentOrchestrator 闭环**: Writer 生成后自动质检→风格检查→改写

### 技术细节
- Migration 37: `chapters.scene_id` + `scenes.chapter_id`
- 后端 `state_sync` 模块: `SyncEvent` 枚举 + `StateSync` 发射器
- 前端 `useSyncStore` Hook: 监听 `sync-event`，自动 `invalidateQueries`
- `show_backstage` 接收 `story_id` 参数，自动导航定位

### 编译状态
- `cargo check` ✅ 零错误
- `cargo test` ✅ 193/193
- `npm run build` ✅

---

## [v5.0.0] - 创世引擎：一键创世，万物关联

### 核心升级
- **一键生成完整小说世界**: 输入"写一部都市玄幻小说"，自动生成故事概念 + 第一章正文 + 完整大纲 + 角色性格小传 + 场景规划 + 伏笔埋设
- **7步创世工作流**: 构思 → 开篇 → 世界 → 大纲 → 角色 → 场景 → 伏笔 → 关联
- **自动幕后卡片创建**: 所有生成内容自动在幕后对应栏目创建卡片

### 新增功能
- **故事大纲系统**: `story_outlines` 表 + 3幕结构自动生成 + 前端概览面板
- **角色系统增强**: appearance/gender/age 字段 + 角色关系图谱
- **伏笔自动生成**: Bootstrap 自动埋设 3-5 个核心伏笔
- **知识图谱自动构建**: 角色/场景/伏笔自动创建 KG 实体
- **前后台智能联动**: 完成后自动导航到 Stories 并高亮新故事

### Bug 修复
- **后台窗口白屏修复**: 隐藏后重新显示时出现空白/白屏
- **后台卡片显示修复**: Bootstrap 后大纲/角色/场景/伏笔卡片不显示
- **根因**: (1) 后台隐藏时无法接收事件 (2) 前后台独立 Zustand store 未同步 (3) 页面未监听 DataRefresh (4) 无自动加载
- **修复**: App.tsx 自动加载 + FrontstageApp 通知同步 + 页面监听 backstage-data-refreshed 并 invalidate queries

### 数据库迁移
- Migration 34: `story_outlines` 表
- Migration 35: `characters` 增强 + `character_relationships` 表
- Migration 36: `scenes.foreshadowing_ids`

### 编译状态
- `cargo check` ✅ 零错误
- `cargo test` ✅ 193/193
- `npm run build` ✅
- `cargo tauri build` ✅ — Windows `.exe` (36MB) + `.msi` (14MB) + `-setup.exe` (10MB) 已生成

---

*最后更新: 2026-06-21 - v0.22.3 钥匙串彻底移除*
---

*归档于 2026-07-04。后续版本摘要请见根目录 AGENTS.md。*

---

*归档于 2026-10-06（v0.60.0）：v0.59.0 / v0.58.0 摘要。*

### v0.59.0 - 验收证据链、数据层治理与续写质检闭环

对照 `docs/audits/2026-10-06-project-review-v0.58.0.md`。三批实施：CI 恢复阻塞（弃用「49 个 V092 基线失败」过时注释）、迁移记 checksum + 集合水位线（低于水位的补丁迁移可执行）、V133 六索引与删故事级联、后台 ingest/QC 纳入 run 预算与取消传播、续写质检可行动（事件带 mode/chapter + 幕前「按审查意见修订本章」走 `auto_revise(revision_type=editor_qc)`）、补齐 4 个占位 prompt 资产、server JWT/DEV_UPGRADE 缺省加固、导出 ZIP 默认剔除 API key、发布白名单补 .deb、AGENTS.md 1208→190 行、FrontstageApp 抽 `useScenePersistence`。

- **验证**：`cargo test --lib` 1624 passed / 3 ignored（+41）；`npx vitest run` 609 passed / 3 skipped（+2）；`tsc` / `architecture_guard` / `cargo +nightly fmt` / prettier 全绿；Playwright 39 passed / 5 skipped。
- **契约**：`test_apply_pending_backfills_lower_versioned_migration`；`test_record_migration_stores_content_checksum`；`test_pick_migrations_dir_ignores_build_output_copy_even_when_newer`；`export_strips_api_keys_by_default`；`test_prune_activity_log_keeps_recent_rows`；`revise_task_description_injects_editor_qc_issues`；续写质检不合格不提示「重新创世」且出现可修订操作条；`rejects_known_insecure_defaults`（server，独立提取运行）。
- **未关闭**：真机续写仍未复跑，**不得宣称续写质量已修复**；新发现载入期空保存竞态（E2E 门禁暂留非阻塞）；`src-server` 无 DB 环境不可编译、CI 未覆盖；golden harness 需真机基线。

### v0.58.0 - 戏剧工艺 + 短剧格式（AI-drama-pound）

对照 `docs/plans/2026-08-29-drama-craft-fusion-design.md`。工艺来源 [AI-drama-pound](https://github.com/POUND0423/AI-drama-pound)（MIT），不 vendoring 对方 skill，不把主创拉回 ToolLoop。节拍卡增加本拍必须改变项；续写短合同禁止原地踏步；编辑审计可读 impact/fix；探针只在复述近文且未兑现改变项时 gap。V131 `story_format` 默认 novel；显式短剧词才切竖屏剧本组装。幕后新建可选长篇/短剧。幕前仍写 `scenes.content`。

- **验证**：`cargo test --lib` 1583 passed / 2 ignored（+11）；`npx vitest run` 607 passed / 3 skipped（+1）；`tsc` / `architecture_guard.py` 全绿。
- **契约**：`change_delta_from_hostile_cast`；`continue_system_has_stall_example`；`editor_issue_parses_impact_and_fix`；`probe_gaps_when_increment_is_tail_recap`；`probe_does_not_gap_literary_aside_when_not_recap`；`looks_like_short_drama_defaults_novel`；`continue_beat_complete_does_not_require_tools`；制作限制只在短剧显示。
- **未关闭**：真机须再跑创世/续写/短剧；**不得宣称续写质量已修复**。不分镜、不自动删角色脏行。

---

*归档于 2026-10-06（v0.61.0）：v0.59.1 摘要。*

### v0.59.1 - 构建修复：对齐新版 nightly rustfmt

v0.59.0 的 CI 卡在「Check Rust formatting」（tauri-build 被跳过，安装包未产出）：浮动 nightly 由 2026-07-17 升到 2026-10-05 后中文注释折行规则变化。已整仓按新规则格式化（106 文件，纯折行无逻辑改动）。

- **验证**：`cargo +nightly fmt -- --check` 0 diff；`cargo test --lib` 1624 passed / 3 ignored；`npx vitest run` 609 passed / 3 skipped（均不变）。
- **复发处置**：CI 若在格式步失败 → `rustup update nightly && (cd src-tauri && cargo +nightly fmt)` 后提交。

---

*归档于 2026-10-06（v0.62.0）：v0.59.2 摘要。*

### v0.59.2 - 修静默清空、清死代码、归档旧文档

载入期空文档保护：ProseMirror 空文档是 `<p></p>`（真值），旧 `if (!content)` 守卫挡不住 → 正文未到时编辑器自带空文档被 2s 防抖保存落库、覆盖整章（e2e 稳定复现）。现 `markSceneContentLoaded` 布防、`isEmptyEditorHtml` 判定、首次非空保存自动解除。另修 JSON 尾随逗号换行形态（模型几乎总把闭合括号另起一行）。E2E 去掉 `continue-on-error` 提升为阻塞门；删 5 个零引用编辑器扩展 + 4 个孤儿 hook（前端测试 −24）；根目录 33 份陈旧 .md 归档到 `docs/archive/root-legacy/`；landing 字体 CDN 上锁 `@3.0.0` + SRI。

- **验证**：`cargo test --lib` 1626 passed / 3 ignored（+2）；`npx vitest run` 585 passed / 3 skipped（净 −24：删 27 孤儿测试 + 新增 3 项空文档判定）；Playwright 39 passed / 5 skipped（连续两轮）；landing 24 passed + build 通过。
- **契约**：`isEmptyEditorHtml` 空文档判定；`test_extract_fenced_json_trailing_comma_newline`；`test_strip_whitespace_trailing_commas_keeps_string_literals`；`frontstage-editing` 自动保存持久化用例（3 轮稳定）。
- **未关闭**：真机续写未复跑（**不得宣称续写质量已修复**）；src-server 无 DB 不可编译、CI 未覆盖；withGlobalTauri + 宽松 CSP、FTP 明文、`story_outlines` 机器覆盖手写大纲、Agency↔agents 环依赖与 coordinator 巨石拆分待办。

---

*归档于 2026-10-06（v0.63.0）：v0.59.3 摘要。*

### v0.59.3 - 手写大纲不再被机器改写

V134 给 `story_outlines` 加 `source`（存量 unknown，保持「机器仍可精炼」语义）。作者手写/弹窗确认（`user_created`）时：创世 `materialize` 的 upsert 带 `WHERE source <> 'user_created'` 不覆盖；资产回流 `sync_story_delta` 直接跳过不追加；`StoryOutlineRepository::update` 仅在带内容时打标（只改 structure_json 不改来源）。另删死模块 `memory/hybrid_search.rs`（410 行）与 capability 死权限 `http:default`。

- **验证**：`cargo test --lib` 1628 passed / 3 ignored（+5）；vitest 585 / 3 skipped；tsc / guard / nightly fmt / playwright 全绿。
- **契约**：`test_materialize_does_not_overwrite_user_created_outline`；`test_materialize_still_updates_machine_outline`；`test_sync_story_delta_skips_user_created_outline`；`update_with_content_marks_user_created`；`update_without_content_keeps_source`。
- **未关闭**：真机续写未复跑（**不得宣称续写质量已修复**）；withGlobalTauri + CSP 需真机运行时验证；FTP 明文；Agency↔agents 环依赖 / coordinator 拆分 / llm_calls 保留 / src-server CI。

---

*归档于 2026-10-06（v0.64.0）：v0.59.4 摘要。*

### v0.59.4 - 发布纪律门禁与网站链路修复

起因：v0.59.1–v0.59.3 连续三版漏更 `ARCHITECTURE.md`（文档更新脚本无断言、静默失配），且线上 `latest.json` 仍停在 0.58.0。新增 `docs-guard` 作业（tag 推送时机械校验 8 份必需文档都有改动，缺失即 fail）；补齐 ARCHITECTURE.md 的 v0.59.1–v0.59.3 记录；landing 兜底版本回退 0.58.0（0.59.x 线上 404，兜底不得指向不存在版本）。

- **阻塞点（需人工）**：macOS `tauri-build` 失败于 Apple 公证 `403 A required agreement is missing or has expired`；`upload-to-website` 依赖三平台全成功 → 网站未更新（Windows/Linux 构建成功，但未上传）。
- **验证**：`cargo test --lib` 1628 passed / 3 ignored；`npx vitest run` 585 passed / 3 skipped；landing tsc + 24 tests；build.yml YAML 解析通过；本地按 docs-guard 同款命令预演通过。
- **契约**：`docs-guard`（发布必需文档门禁）。
- **未关闭**：签署 Apple 协议后重跑 macOS 构建 → upload-to-website 才会发布 0.59.x（含 0.58.0 缺失的 `.deb`）；真机续写未复跑，**不得宣称续写质量已修复**。

---

*归档于 2026-10-07（v0.64.1）：v0.59.3 摘要。*

### v0.60.0 - 三把尺子：知识边界 / 物品归属 / 改稿级联影响报告

对照外部五项目对比报告（docs/audits）落地 P0 阶段，把「防吃书」从提示词叮嘱变成可校验机制。**V135** 新增四张表：`story_timeline_events`（世界真相 / 读者认知 / 揭示状态机双栏建模）、`character_knowledge_log`（知情变更审计流水）、`item_holdings`（关键物品持有者账本）、`cascade_impacts`（改稿影响报告）。**知识边界**：ingest 新增 `knowledge_updates`/`timeline_events` 抽取，修掉 secrets 被 COALESCE 永久冻结的断链；续写资产注入【本拍信息差】【未公开真相】禁令（计划内揭示自动豁免）；`detect_knowledge_leaks` 接入续写探针与 editor_qc 疑点清单；Agency 快照不再丢弃 secrets。**物品归属**：`item_holdings` 按 (story,item) upsert，续写注入【在场物品】，`detect_possession_conflicts` 拦「非持有者使用/遗失物再现」（当场转手豁免）。**级联**：场景 re-ingest 后自动跑确定性影响分析（下游章、无处不在实体过滤）＋ LLM 冲突扫描（提示词资产 `cascade_conflict_scan`），发 `SyncEvent::CascadeImpactDetected`，新增 4 命令与幕后「级联中心」页（去查看/重跑分析/触发改写/忽略）——**只报告不改写后文**。

- **验证**：`cargo test --lib` 1643 passed / 3 ignored（+15）；`npx vitest run` 590 passed / 3 skipped（+5）；tsc / nightly fmt / prettier / architecture_guard 全绿。
- **契约**：`test_edit_early_chapter_creates_downstream_impacts_only_for_shared_entities`（帖主测试③）；`test_persist_knowledge_updates_moves_secret_from_unknown_to_known`；`test_knowledge_boundary_detects_unknown_secret_leak` / `..._hidden_truth_reveal`；`test_possession_conflict_flags_absent_holder_but_allows_transfer` / `..._lost_item_reuse`；`test_continuity_gaps_reads_db_and_respects_planned_text`；`test_ubiquitous_entity_is_filtered_out`；`CascadeCenter` 5 用例。
- **未关闭**：真机三把尺子端到端复跑（P3 三测试套件收口）；**不得宣称续写质量已修复**；网站发布仍待 Apple 公证解阻。
