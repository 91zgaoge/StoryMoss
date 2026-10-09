# StoryMoss (草苔) v0.65.3 项目完成状态

> 最后更新: 2026-10-09（**v0.65.3：幕前「续写后正文消失 / 章节点不开」真机缺陷修复**——真机《帝国的烟火》第 2 章续写 → 自动分章出第 3 章 → 幕前自动切换，用户看到续文消失、重启后又有、且第 3 章此后点不开；本机日志留痕三处独立缺陷：①`lazyLoadingChapterIdsRef` 只 add 不 delete（懒加载过一次的章本次会话内永久打不开）→ 在途去重 + 可重试；②`RichTextEditor` 的「有焦点不强制 setContent（W2-F1）/ Tab 接受后 30s 禁同步（v0.24.9）」把无交互的分章自动切换的新章正文挡掉（屏幕停在旧章、章标题已切走）→ 以章 id 为文档身份，切章优先于两道守卫；③`selectChapter` 无条件清除未确认幽灵续文 → 新纯函数 `isGhostDeliveredInContent` 判定是否已随新章正文呈现，未呈现则保留 + 顶栏提示。回归探针修复前逐条复跑为红：vitest 620 passed / 3 skipped（+11）、tsc ✅、playwright 39 passed / 5 skipped ✅。v0.65.2：主续写路径 system 提示词补文笔准则**——核对链路发现主续写的 system 提示词是 `CONTINUE_BEAT_SYSTEM`（assembly.rs 内联常量，用户不可覆盖）而非 `agency_lead_writer_system`：补第 7 条文笔（四模式情绪/句长参差/不解释主题/重复「说」/具体实物）+ 对照示例；`DRAMA_BEAT_SYSTEM` 按剧本语域补「情绪用可拍的动作与台词演」。v0.65.1：v0.65.0 收尾**——叙事架构层写进章纲规划（回声测试 / 因果松动一次 / 不要预支答案 / 不做「理解+接受+成长」收束 / 关系网稀疏 / 主题不直说）、改写纪律写进润色提示词（删优于加），新增六提示词契约测试；v0.65.0：人类文笔基线**——把「AI 写多了、人写少了」的分布偏移做成确定性规则与提示词。方法来源开源项目 sepia / StoryScope，逐条注明实测方向：`story_system::human_voice`（句长离散度 `flat-rhythm`、语气词缺失 `mood-particle-absent`、情绪具身化独大 `embodied-only`、连词堆叠、双音节填充、花式对话标签、抽象包装、段落过匀；全部 advisory + 量化指标）；每拍写作提示词注入 7 条固定准则 + 8 条手法池按篇幅轮换；editor 审查加【人类文笔基线】块与诊断纪律（分组核查 / 引用原文 / 白名单 / 优先删换）；commit 记录落库。校准既有模块：`prose_lint` 的「不是…而是」改为聚集（≥2）才报；`anti_ai` 三处方向错误判定修正（平直命名情绪不是缺陷、重复「说」是常态、感官密度低不是 AI 指纹），并修复「」引号对话整块漏检。验收探针：机器腔样本命中 5 条、人类腔样本 0 误报，句长 SD 6.24 对 14.97。v0.64.8：称号幻影行随死者一并排除——KG 里只有称号的角色行（公主/镇北王）按 v0.64.6 解析策略随死者退场。v0.64.7：**死人不得复活**——角色生死状态持久化。真机《帝国的烟火》第 2 章被打死的明成公主在第 10/11 章被续写成活人：生死此前只由「当前章末 1500 字」窗口临时推断、从未落库。V142 加 `characters.life_status`/`death_chapter` 并按章序扫描正文回填；`auto_commit` 与 `update_scene` 即时刷新；已死角色不进 cast/下一拍，角色卡注入「已死（第 N 章），不得作为活人行动」，导演锁渲染「已死」；单调写回 + 人物页「已死」徽标与一键改判（假死情节）。真机验收探针（库副本）通过：明成公主被排除出 cast、在 dead 名单、角色卡带标记。v0.64.6：**人物称呼归一**——`景亲王`/`景亲王曹元寿`、`苏世子`/`苏亦铁` 这类「同一人多个称呼」不再各占一个角色行：V140 别称表 + 建行前解析 + 命中即合并 + 抽取 aliases + V141 启动迁移。v0.64.5：**同章重复续写不落 commit**——改为复用既有行的 UPSERT。v0.64.4：**模型失败不再伪装成「文本过短」**、探测超时不再跳过最后一个候选。含 v0.64.3：**下引号孤行根除**。v0.64.2：续写人物关系错乱修复 + 关系不变量守卫）
>
> v0.30.43：修复续写内容丢失根因--flushSceneSave 读取滞后 latestContentRef + onChapterUpdated 覆写未保存内容）
> GitHub: https://github.com/91zgaoge/StoryMoss

---

## 🚧 当前迭代

- **Agency 多代理创作框架**：P1（多代理框架骨架）+ P2（质量门 / 并发与 token 预算 / 并行稳态循环 / request_id 定点取消 / 续写循环 / 资产落库 / smart_execute 切换与旧 GenesisPipeline 移除）+ P3（角色×任务模型路由 / 全局 LLM 并发闸门 / 注入 token 预算与黑板三档目录 / agency_sessions 会话快照 / agency_resume_run 跨会话恢复 / V109 并发护栏）+ P4（code/rule/model/human 四级 grader / Gate v2 加权评分阈值 0.75 / V110 里程碑检查点与对比 / eval harness 与 baseline 回归门 / 评估仪表盘页）+ P5（持续学习双轨：observations.jsonl 观察层 + 后台 analyzer + instinct 文件层 / 置信度引擎 / ≥0.8 跨 story 晋升物化 skill.yaml / 学习中心页 / 代理工作室页 / eval CI 门禁）已完成。除真机验收外 P1–P5 已完成，框架收官。**v0.30.1 创世提速（Genesis Fastpath）已发布**：创世压缩为三阶段 4 次 LLM 调用（概念包 -> 主创首章 ∥ 管理深度资产 -> 编辑质量门），典型远程模型首章 ≤3 分钟；主创模型优先（多模型时 Tool 档排除 active/creative，单模型时主创先出首章）；单调用解析失败自动回退 legacy 串行流程，取消信号不误入回退；smart_execute 超时回退统一 600s。

---

## ✅ 最近完成功能

### v0.64.12 - 自动后台重算 + 指纹感知（2026-10-08）

- **V145** `scene_commits.summary_source_hash`（提交时写正文指纹）；`recompute_scoped` 指纹感知：
  只重算正文变过的章、只重建摘要变过的段（长书不再等于一次全量重写）。
- **自动触发**：提交防抖成功后若存在「自第 N 章起失效」且 N ≤ 提交章，后台自动重算——
  后台模型闸门 + 同故事并发去重 + 单轮 12 章上限（超出保留标记续算）+
  `AppConfig::auto_recompute_after_edit` 开关（默认开）。
- **真机探针**：一轮重算 4 条重写 → 二轮 0 条重写 / 4 条未变；自动判定 `Some(9)`。
- **基线**：`cargo test --lib` 1764 passed / 4 ignored（+5）；vitest 609 / 3 skipped。

### v0.64.11 - 物料失效重算 + 关系归一 + 摘要失败可见（2026-10-08）

- **物料失效与重算**：V143 `story_material_staleness`（编辑正文即记「自第 N 章失效」）+
  `story_system::recompute`（章节摘要按当前正文重算、分层摘要/全书纲要删旧重建、快照重写、
  关系支撑审计）+ 运行维护页「物料重算」页签与两个命令。
- **关系归一**：V144 两列（`relation_kind` 受控词表 + `relation_flags`）+ `db::relation_kind`
  分类器，接入冲突阶梯敌意判定与关系不变量血亲/配偶判定；`relation_retract` 按
  `kg_relations.evidence` 摘证据撤回（编辑路径已接）。
- **摘要失败可见**：`summarize_chapter_with_quality` 与 `SegmentRefreshReport` 的所有回退/
  失败路径入质量债。
- **真机探针**（库副本）：46 行关系全归一、失去支撑 0 行、`mark_stale(9)` → 重算通过。
- **基线**：`cargo test --lib` 1759 passed / 4 ignored（+14）；vitest 609 / 3 skipped（+2）。

### v0.64.10 - 冲突升级/衰减：同一对峙不得连拍复述（2026-10-08）

- `ConflictStage` 四阶梯（加压→升级→结账→余波），从上一拍卡块的 `冲突：` 行推进；
  `compile_beat_card_located_prev` 接线 Append / observe / 批量三条路径；上一对到余波
  即轮换到别的对峙对。
- 探针两缺口入质量债：「冲突未升级」（升级/结账级缺代价标记）、「冲突原地复述」
  （与前文 bigram 相似度 ≥0.62，新增 `TextUtils::char_bigram_similarity`）。
- **真机探针**（库副本）：真机第 10 章正文连编译两拍 Press → Escalate。
- **基线**：`cargo test --lib` 1745 passed / 4 ignored（+7）。

### v0.64.9 - 续写重演修复 + 投影 writer 恢复落库（2026-10-08）

- **重演根因四段**：卡块回灌成章纲；`_existing_outline` 参数被忽略导致新建章章纲被覆盖丢弃；
  方法论兜底「下一拍」被落库并被逐拍采纳；自动分章新章从不生成章纲（只有近文阵容 + 静态
  敌意关系）。**修复**：章纲合并 + 兜底句不落库不采信 + 只剥卡块形态的 `chapter_plan_from_scene_outline`
  + `needs_plan` 时 Append 补章纲（落库为前缀）。
- **投影 writer**：`missing field subject` / `entity_id` 的真因是反序列化目标没有生产者
  （实际产物是 KG 实体/关系视图）；`normalize_delta_items` 兼容四形态并落库。
- **真机验收**（库副本）：真机 commit 产物跑两个 writer 全部成功，`memory_items` state+entity
  250 行。
- **基线**：`cargo test --lib` 1738 passed / 4 ignored（+7）。

### v0.64.8 - 称号幻影行随死者一并排除（2026-10-08）

- KG 里的纯称号幻影行（`公主`、`镇北王`，无 `characters` 行）随死者一并排除出 cast：
  `life_status::dead_names` 复用 v0.64.6 的 `resolve_character_id` 解析策略（精确名 →
  别称表 → 唯一同人形态命中），解析不出来不猜。
- 真机探针 dead 名单 `[公主, 明成公主, 苏会山, 镇北王]`；新增契约
  `dead_names_expand_to_registered_aliases`。
- **基线**：`cargo test --lib` 1731 passed / 4 ignored（+14）。

### v0.64.7 - 死人不得复活：角色生死状态持久化（2026-10-08）

- **真机事故**：明成公主第 2 章被一拳打死（「七窍喷血……登时气绝」「明成公主的尸体躺在原处」
  「公主尸身移西院」），自动续写到第 10、11 章又让她走路、说话、夺印、抓人手腕——同一场景里
  她的尸体仍停在门板上。
- **根因**：生死唯一来源是 `dead_names_in_text` 对**当前章末 1500 字**（`PRIOR_CAST_CHAR_CAP`）
  的扫描；第 2 章的死亡到第 9 章早已滑出窗口。抽取的死亡信号只落在
  `kg_entities.attributes.status`（不进任何注入路径），`characters` 无生死列。
- **修复**：V142 两列 + 存量回填（章序扫描正文，KG `status=Dead` 兜底）；判定下沉
  `utils::death_text`、列读写回填在 `db::character_life`、策略在 `story_system::life_status`；
  `auto_commit` / `update_scene` 即时落库；`beat_card` dead 名单与下一拍并入持久化已死；
  `WriteTimeBundle` 角色卡注入「已死（第 N 章），不得作为活人行动」；单调写回 +
  人物页徽标与一键改判（`set_character_life_status`）。
- **真机验收**（库副本，未触碰原库）：`苏会山`、`明成公主` → 第 2 章已死；真机第 10 章正文
  编译节拍卡：她不在 cast、在 dead 名单；写作包角色卡为「已死（第2章），不得作为活人行动」。
- **基线**：`cargo test --lib` 1730 passed / 4 ignored（+13）；vitest 607 / 3 skipped（+3）；
  clippy 0 error；fmt / prettier / guard / tsc 全绿。

### v0.64.6 - 人物称呼归一（2026-10-07）

- **问题**：同一人物的不同称呼各占一个角色行（景亲王/景亲王曹元寿、苏世子/苏亦铁、奉乾帝/奉乾皇帝），关系表与提示词出现"两个同一个人"。
- **根因（数据层，不是模型）**：抽取要求 `name` 取文本中出现的名字 + 建行按名精确匹配 + `characters` 无别称层 + 称号词表仅 11 词。
- **修复**：V140 别称表、建行前解析（精确名→别称→称号形态）、命中即合并（改线六个引用面 + 正文名字 token + 补齐空字段）、抽取 prompt 增 `aliases` 与归并要求、称号词表扩充与「称号在前」形态、V141 启动合并存量。
- **本机数据已修**：苏世子→苏亦铁、奉乾皇帝→奉乾帝、景亲王→景亲王曹元寿；关系 25 行无重复无悬空；备份 `cinema_ai.db.bak-v0.64.6-*`。
- 基线：Rust 1717 / 3 ignored（+8）；vitest 604 / 3 skipped；clippy 0 error。
- **未关闭**：字/号/官职靠 LLM aliases；同名候选多于一个时不猜。

### v0.64.5 - 同章重复续写不落 commit（2026-10-07）

- **事故**：第 2 章首次提交后，每次续写保存都报 `auto_commit failed … UNIQUE constraint failed: scene_commits.story_id, scene_commits.chapter_number`。
- **根因**：一章一条（唯一索引）却被 `init_commit` 用 INSERT 实现 → 第二步起必然冲突，那一轮的 review/摘要/履行度/KG/增量/投影全部不落。
- **修复**：`upsert_pending`（复用行、重置 pending、刷新挂载、清空派生字段，避免旧摘要被当当前事实）；`init_commit` 与命令层一并幂等。
- 基线：Rust 1708 / 3 ignored（+3）；vitest 604 / 3 skipped；clippy 0 error。
- **未关闭**：本机第 2 章 commit 待下次保存/续写重算；真机端到端未复跑。

### v0.64.4 - 模型失败可见性 + 探测超时韧性（2026-10-07）

- **事故**：加正文后续写崩在 `write_beat_once 过短（0 字符），续写回退仍失败`。
- **诊断**：四个端点中本机 `127.0.0.1:11500` 与局域网 `:17092` 不可达、deepseek 401，仅剩 `:17098` 健康（实测 0.68s）——但它在 5s 起跑探测中超时被跳过，全部候选失效。
- **修复**：模型失败立即带原因失败（不再吞成空文本 → 不再报「过短」）；探测超时若已是最后一个候选则真打一次；`StreamOutput` 渲染前合并悬挂闭合引号。
- **含 v0.64.3 全部内容**：下引号孤行根除（句读切分 + 段首并回 + V139 迁移），v0.64.3 未上传安装包。
- 基线：Rust 1705 / 3 ignored（+1）；vitest 604 / 3 skipped；clippy 0 error。
- **未关闭**：模型端点需用户恢复；真机端到端未复跑。

### v0.64.3 - 下引号孤行根除（2026-10-07，未发布，内容并入 v0.64.4）

- **根因**：`splitChineseSentences` 在句末标点处断开，收尾引号被切给下一句；段落组装正好在那里断段 → `<p>”\n正文…</p>`（真机《帝国的烟火》第 2 章两处）。此前三次修复都只覆盖「整段仅闭合标点」，引号后带正文的漏掉。
- **修复**：句末标点后的收尾符归属本句（源头）；新增「段首并回」规则（前端 + Rust 同规则），段首只认有方向的收尾符（ASCII 直引号保持）；`autoFormatText` 三条路径与 `textToParagraphsHtml` 统一走「孤段 + 段首」合并；V139 存量迁移幂等修库。
- 基线：Rust 1704 / 3 ignored（+6）；vitest 604 / 3 skipped（+10）；clippy 0 error。
- **未关闭**：存量修复依赖新版本启动时迁移；真机端到端未复跑。

### v0.64.2 - 续写人物关系错乱修复（2026-10-07）

- **事故**：《帝国的烟火》续写第二章台词错位——「景亲王江顾然」两人并成一体、景亲王对大执事说「苏爱卿，你儿子娶的是公主」。
- **根因**：配偶启发式整串 `contains("配偶")` 致笛卡尔积（一处「一并坐下」把角色与全部在场者写成夫妻）；`persist_inferred_relationships` 的覆盖条件把「类型不同」当覆盖理由，父子/兄妹/同僚被改写成夫妻；同一份清单既进提示词又落库。
- **修复**：逐段匹配 + 去内部标记泄漏 + 落库只修脏/填空；`sanitize_relations`/`sanitize_bundle_relations` 覆盖【本拍人物关系】与【角色情感关系】两条注入路径；拦下行与探针遗留缺口入质量债（`continue_relations` / `continue_probe`）。
- **本机数据修复**：32 行误写夫妻 → 回填 6 / 删除 26；幻影人物「景亲王江顾然」清除（备份 `cinema_ai.db.bak-v0.64.2-*`）。
- 基线：Rust 1698 / 3 ignored（+7）；vitest 594 / 3 skipped；clippy 0 error。
- **未关闭**：运行中的 0.64.1 需升级；第二章正文错乱台词待作者决定；同名重复角色行未合并；真机端到端未复跑。

### v0.64.1 - 发布链路恢复（2026-10-07）

- **0.64.0 已上线**：latest.json=0.64.0；dmg/msi/AppImage/deb 全 200（Apple 公证协议签署生效）。
- landing 兜底版本 0.58.0 → **0.64.0**。
- 分章自动切换用例三处等待显式 5s（CI 抖动曾阻塞发布）。
- v0.60.0–v0.63.0 补推到 GitHub（此前只在 Cursor 代理），CI 恢复运行。
- 基线：Rust 1691 / 3 ignored（无改动）；vitest 594 / 3 skipped；landing 24。

### v0.64.0 - 运行维护页 + 声明式投影路由表（2026-10-06）

- **P3-F**：`PROJECTION_ROUTES` 声明式路由表（产物 × writer），注册顺序与状态键从表派生，commit 记录路由摘要；6 项契约测试。
- **运行维护页**：质量债 / 待确认 / 文风偏好 / 成本 四 Tab（补齐 P2/P3 的 UI 缺口）；+4 vitest。
- **修复**：`clippy::redundant_comparisons`（v0.63.0 CI 失败根因）；`cargo clippy` 纳入每版验证。
- 基线：`cargo test --lib` 1691 passed / 3 ignored（+6）；vitest 594 passed / 3 skipped（+4）；clippy 0 error。
- **未关闭**：真机端到端未复跑；v0.63.0 tag 保留不回改；网站发布待本版 tag 跑通。

### v0.63.0 - 工程纪律（P0–P3 收官，2026-10-06）

- **P3-A 质量债**（V138）：质检降级/未解决问题入账（含建议回收窗口），幂等，可结清/忽略。
- **P3-C 检查点 + 时间旅行**（V138）：每 10 章快照；`query_story_as_of` 回溯「截至第 N 章」的角色已知与真相揭示（物品账本标注 current_only）。
- **P3-D 终局指南针**：确定性派生并注入续写（零额外 LLM）。
- **P3-B 待确认队列**（V138）：ingest 新增规则类资产（importance ≥7）入队待作者确认。
- **P3-E 三把尺子组合契约**：同一故事内组合回归 玉佩/知识边界/级联。
- 基线：`cargo test --lib` 1685 passed / 3 ignored（+14）；vitest 590 / 3 skipped（无前端改动）。
- **未关闭**：P2-B/P2-D/P3-A/P3-B 无 UI；P3-F 路由表未实施；真机端到端未复跑；**不得宣称续写质量已修复**。

### v0.62.0 - 文本质量与成本（2026-10-06）

- **P2-A prose_lint**：确定性两档规则（注入术语泄漏/否定排比/章尾预告腔 blocking；破折号密度/重复句/极短收尾 advisory），接入 commit review 与 editor_qc 预注入。
- **P2-B 文风逆向学习**（V137 `style_preferences`）：手改 → 防抖 120s 提炼 → 续写【作者文风偏好】。
- **P2-C 伏笔增强**（V137 增列）：证据锚定、强度/隐藏度、伏笔链、单章上限 5、注入话术窗口分档。
- **P2-D 成本账本**：按故事聚合 + 阈值提示 + 计费盲区哨兵；命令 `get_story_cost_summary`。
- 基线：`cargo test --lib` 1671 passed / 3 ignored（+18）；vitest 590 / 3 skipped（无前端改动）。
- **未关闭**：真机验证；P2-B/P2-D 无 UI；**不得宣称续写质量已修复**。

### v0.61.0 - 分层记忆金字塔（2026-10-06）

- **P1-A 语义摘要**：`summary_text` 从 1000 字截断升级为 LLM 100-150 字语义摘要（资产 `chapter_summary`，失败回退截断）。
- **P1-B 分层金字塔**（V136 `story_segment_summaries`）：每 10 章段摘要（数据不足一半跳过）+ 段数 ≥3 全书纲要；commit 后后台补齐。
- **P1-C 自适应窗口**：≤15→10 / 16-50→5 / >50→3 章窗口 + 段摘要/全书纲要注入；续写新增【故事纲要】块。
- 基线：`cargo test --lib` 1653 passed / 3 ignored（+10）；vitest 590 / 3 skipped（无前端改动）。
- **未关闭**：真机 10+ 章验证摘要质量；**不得宣称续写质量已修复**。

### v0.60.0 - 三把尺子：知识边界 / 物品归属 / 改稿级联影响报告（2026-10-06）

- **V135 四表**：`story_timeline_events`（世界真相/读者认知/揭示状态机）、`character_knowledge_log`、`item_holdings`、`cascade_impacts`。
- **知识边界**：ingest 抽取 `knowledge_updates`/`timeline_events`（修 secrets COALESCE 冻结断链）；续写注入【本拍信息差】【未公开真相】（计划内揭示豁免）；泄密探针 + editor_qc 疑点清单；Agency 快照保留 secrets。
- **物品归属**：`item_holdings` upsert + 续写注入【在场物品】+ 归属矛盾探针（非持有者使用 / 遗失物再现；当场转手豁免）。
- **级联影响报告**：re-ingest 后自动确定性影响分析（下游章 + 无处不在实体过滤）＋ LLM 冲突扫描 → `SyncEvent::CascadeImpactDetected` → 幕后「级联中心」四动作（去查看/重跑分析/触发改写/忽略）。**只报告不改写后文。**
- 基线：`cargo test --lib` 1643 passed / 3 ignored（+15）；`npx vitest run` 590 passed / 3 skipped（+5）；tsc / nightly fmt / prettier / guard 全绿。
- **未关闭**：真机三把尺子端到端复跑（P3 收口）；网站发布待 Apple 公证解阻；**不得宣称续写质量已修复**。

### v0.59.4 - 发布纪律门禁与网站链路修复（2026-10-06）

- 新增 `docs-guard`（tag 推送校验 8 份必需文档都已更新）；补齐 ARCHITECTURE.md 的 v0.59.1–v0.59.3 记录。
- landing 兜底版本回退 0.58.0（0.59.x 线上 404，兜底不得指向不存在版本）。
- **线上现状**：latest.json=0.58.0；0.59.x 全 404；0.58.0 无 deb。阻塞点＝Apple 公证 403（协议未签署）。
- 基线：Rust 1628 / 3 ignored；vitest 585 / 3 skipped；landing 24 tests；E2E 39 passed / 5 skipped。

### v0.59.3 - 手写大纲不再被机器改写（2026-10-06）

- V134 `story_outlines.source`：作者手写/确认（user_created）的大纲不再被创世 materialize 覆盖、不再被 ingest 追加；存量行为 unknown（保持原语义）。
- 删除死模块 `memory/hybrid_search.rs`（410 行）与 capability 死权限 `http:default`。
- 基线：`cargo test --lib` 1628 passed / 3 ignored（+5）；vitest 585 / 3 skipped；Playwright 39 passed / 5 skipped。

### v0.59.2 - 修静默清空、清死代码、归档旧文档（2026-10-06）

- 修 P1：载入期空文档（`<p></p>`）覆盖整章正文；新增载入后空写保护 + 单测 + E2E 复现用例。
- 修 JSON `,` + 换行 + 闭合括号的尾随逗号解析失败（字符串内部原样保留）。
- E2E 提升为阻塞门；删 5 个零引用编辑器扩展 + 4 个孤儿 hook（前端测试 −24）；根目录 .md 44 → 11。
- landing 字体 CDN 版本上锁 `@3.0.0` + SRI + crossorigin。
- 基线：`cargo test --lib` 1626 passed / 3 ignored；`npx vitest run` 585 passed / 3 skipped；Playwright 39 passed / 5 skipped。

### v0.59.1 - 构建修复：对齐新版 nightly rustfmt（2026-10-06）

- CI 浮动 nightly 升级后中文注释折行规则变化，106 个文件在格式步失败（v0.59.0 安装包因此未产出）；已整仓按新规则格式化，纯折行无逻辑改动。
- 行为与测试基线不变：`cargo test --lib` 1624 passed / 3 ignored；`npx vitest run` 609 passed / 3 skipped。

### v0.59.0 - 验收证据链、数据层治理与续写质检闭环（2026-10-06）

- **CI**：`cargo test --lib` 恢复阻塞；弃用「49 个 V092 基线失败」过时注释。新增 provider 故障注入测试 + golden 续写 harness（`#[ignore]`）。
- **数据层**：`schema_migrations` 内容校验和（V132）+ 集合水位线（低版本补丁可执行）；迁移目录剔除构建产物路径；V133 六个热查询索引；删故事级联 agency_*/llm_calls；活动日志 30 天剪枝。
- **续写质检闭环**：质检事件带 mode/chapter；幕前「按审查意见修订本章」（`revision_type=editor_qc`）；后台 ingest/QC 纳入 run 预算与取消传播。
- **提示词**：补齐 4 个占位资产（writer/inspector/outline_planner/style_mimic）。
- **安全/发布**：server JWT 无缺省密钥、DEV_UPGRADE 默认 false；导出 ZIP 默认剔除 API key；发布白名单补 .deb。
- **工程**：AGENTS.md 1208→190 行；FrontstageApp 抽 `useScenePersistence`（−149 行）；新增全面检视报告 docs/audits/2026-10-06-*。
- **未关闭**：真机续写未复跑（不得宣称质量已修复）；新发现载入期空保存竞态（E2E 门禁暂留非阻塞）；src-server 无 DB 不可编译、CI 未覆盖。

### v0.58.0 - 戏剧工艺 + 短剧格式（2026-08-29）

- 节拍卡 `change_delta`；续写合同禁原地踏步；编辑审计 impact/fix；复述近文探针。
- V131 `story_format` 默认 novel；幕后长篇/短剧选择器。
- **验证**：`cargo test --lib` 1583 passed / 2 ignored（+11）；`npx vitest run` 607 passed / 3 skipped（+1）。
- **未关闭**：真机须再跑创世/续写/短剧；不得宣称续写质量已修复。

### v0.56.2 - 下引号不再单独成段；编辑审计顶栏不再报失败（2026-08-28）

- 悬挂合并跳过换行与引号之间的空白/全角缩进；空行分段与存量 HTML 都并回上一句。
- 后台编辑审查 done 固定「后台审查」；`friendlyText` 失败/超时不加「已完成」。
- **验证**：`cargo test --lib` 1572 passed / 2 ignored（+1）；`npx vitest run` 606 passed / 3 skipped（+4）。
- **未关闭**：真机须再打开该章或再续写；不得宣称续写质量已修复。

### v0.56.1 - 拆人探针认抱衣角；死人不得再用眼睛锁定（2026-08-28）

- 同一人两个称呼独立出现即缺口；已死者点名后 80 字内眼睛/锁定/审视即缺口。
- **验证**：`cargo test --lib` 1571 passed / 2 ignored（+2）。
- **未关闭**：真机须从「飞身扑上」再跑；不得宣称续写质量已修复。

### v0.56.0 - 续写导演锁 / 管理 Agent 必写人物关系（2026-08-27）

- 续写主创仍 `complete()` 零工具。拍前编译人物锁（头衔+名、近文亲缘），冻结含锁。
- 探针拦拆身/亲缘写反；沉默在场不罚「丢掉」。
- 管理 Agent 写角色后 upsert 关系；≥2 角色空表则补写。角色表脏行不自动删。
- **验证**：`cargo test --lib` 1569 passed / 2 ignored（+20）。
- **未关闭**：真机须从「飞身扑上」再跑；不得宣称续写质量已修复。

### v0.55.0 - 资产渐进展开 / 续写冻结 / 短操作合同（2026-08-25）

- Producer/Editor catalog 名+一行；`asset_read` 按名取全卡。续写零工具，在场/冲突全卡，其余半卡。
- `write_beat_once` 冻结节拍卡/阵容，返回后解冻。
- `CONTINUE_BEAT_SYSTEM` 11 行合同 + 三条对错范例。
- **验证**：`cargo test --lib` 1549 passed / 2 ignored（+7）。
- **未关闭**：真机创世/续写；不得宣称 JSON 熔断或续写质量已修复。

### v0.54.0 - Agency ToolLoop 原生 function calling（2026-08-25）

- 角色白名单以 JSON Schema 发给模型；原生 `tool_calls` 优先，文本 JSON 回退。
- 续写 `write_beat_once` 不带 tools。
- **验证**：`cargo test --lib` 1542 passed / 2 ignored（+16）。
- **未关闭**：真机创世/资产路径；不得宣称 JSON 熔断或续写质量已修复。第二至四期未做。

### v0.53.6 - 已写完的行刺不再被续写重演（2026-08-25）

- 死人退出本拍行动阵容；已完成高潮不再当下一拍；探针拦再刺一次。
- **验证**：`cargo test --lib` 1526 passed / 2 ignored（+7）。
- **未关闭**：须用「飞身扑上」同一开头再跑真机；不得宣称唱反调已修复。

### v0.53.5 - 大纲重写确认框（2026-08-25）

- 生成后弹出可编辑对话框；确认才写库；取消废弃；重写再生成。
- **验证**：`cargo test --lib` 1519 passed / 2 ignored（+2）；vitest 602 passed / 3 skipped（+5）。
- **未关闭**：原句真机；不得宣称唱反调已修复。

### v0.53.4 - 写后续大纲不进续写（2026-08-24）

- 「写后续的故事大纲，同时生成后续的场景大纲」走按正文重写，不 Append。
- **验证**：`cargo test --lib` 1517 passed / 2 ignored（+2）。
- **未关闭**：原句真机；不得宣称唱反调已修复。

### v0.53.3 - 按正文重写大纲幕前预览（2026-08-24）

- 该句会落库；幕前弹出「已按正文重写设定」含大纲正文；纸面不变；后续指令从章末往下写。
- **验证**：`cargo test --lib` 1515 passed / 2 ignored（+3）。
- **未关闭**：原句真机弹窗；不得宣称唱反调已修复。

### v0.53.2 - 键值散文与场景大纲（2026-08-23）

- Gemma `story_outline:` / `scene_outline：` 无花括号也会落库。
- **验证**：`cargo test --lib` 1512 passed / 2 ignored（+7）。
- **未关闭**：原句真机；不得宣称唱反调已修复。

### v0.53.1 - 按正文重写 JSON 形状（2026-08-21）

- Gemma 4 把大纲做成对象或散文时不再整段丢弃。
- **验证**：`cargo test --lib` 1505 passed / 2 ignored（+6）。
- **未关闭**：同一句真机；不得宣称唱反调已修复。

### v0.53.0 - 按正文重写生产资产（2026-08-21）

- **路由**：`smart_execute` 在 Append 之前识别 `asset_refresh`，Producer JSON 按靶落库。
- **纸面**：`scenes.content` 只读；无正文 / 续写中拒绝。
- **靶**：点名故事大纲、角色、世界观、场景大纲；未点名请用户说明。
- **验证**：`cargo test --lib` 1499 passed / 2 ignored（+19）；vitest +1。
- **未关闭**：真机同一开头；不得宣称唱反调已修复。

### v0.52.0 - 续写拍级分层与准入轨迹（2026-08-21）

- **近文 1500**：`PRIOR_CAST_CHAR_CAP` 500→1500；散文双窗不变。
- **漏债**：`mentioned_from_continue_tasks` 把大纲/配额/伏笔点名并入准入。
- **膨胀**：回流大纲最多 5 个转折；准入者分层 L2/L1。
- **轨迹**：`format_admission_trace` 写 `continue_assets: shot=…`。
- **验证**：`cargo test --lib` 1480 passed / 2 ignored（+7）。
- **未关闭**：真机续写；不得宣称唱反调已修复。P3 freshness 节流 ingest、P4 ContextPrioritizer 未做。

### v0.51.6 - 工具档/后台档不再被创作模型挤到同一台机（2026-08-20）

- **根因（executed）**：`select_candidates` 已按 Tool/Background 置顶，`generate()` 再把 `active_llm_profile` 插回首位。
- **修复**：`apply_active_model_front` 对工具档/后台档直接返回；创作档行为不变。
- **验证**：`apply_active_front_*` 3 passed。

### v0.51.5 - 进行中的续写不再弹前台中断卡（2026-08-20）

- **根因（inspected）**：同一故事已有 Agency run 被标成 UserAction，中断卡盖住正在写的纸面。续写状态已在底栏。
- **修复**：冲突不 `setShowInterruptionModal`；弹窗对 `active_run` 渲染空；二次点击静默。
- **验证**：vitest 弹窗空渲染 + FrontstageApp 不出现「前往设置」。

### v0.51.4 - 「已有创作任务」不再打发去设置（2026-08-20）

- **根因（inspected）**：`VALIDATION_FAILED` + UserAction 一律走「前往设置」。底栏已在 Agency 续写时，再点一次被当成设置缺项。
- **修复**：进行中冲突专用文案；取消当前续写；二次点击不清理真正那次生成态。
- **验证**：vitest 弹窗契约；Rust `field=active_run`。

### v0.51.3 - 续写规划污染不再把 600 秒耗尽（2026-08-20）

- **根因（executed）**：`creative_workflow.log` 23:55:46 Qwren127 390s 写出 9821 字规划；sanitize 清空后 00:02:16 立刻第二轮；00:02:55 前端 600s 取消后网关仍试 Qwen 3.8。规划粘在 story_outlines 转折点块。
- **修复**：`condense_story_outline` 切断规划；剩余 <90s 不重试；`CANCELLATION` 停候选链。
- **验证**：`cargo test --lib` 1470 passed / 2 ignored（+4）。

### v0.51.2 - 续写切断节拍卡/约束规划泄露（2026-08-19）

- **根因（executed）**：裸 CoT 检测只认旧思维链套话、只扫前 2000 字；Qwen 把节拍卡规划写进 content，或接在复述正文之后。
- **修复**：节拍卡行话全文切断；文首规划清空以重试；续写 system 禁止输出任务分析。
- **验证**：`cargo test --lib` 1466 passed / 2 ignored（+3）。

### v0.51.1 - 幕前取消键去掉系统原生凸起（2026-08-17）

- **根因（executed）**：macOS WKWebView 给未设 `appearance-none` 的 `<button>` 画 Aqua 凸起灰块；取消键休息态无透明底。
- **修复**：取消键 / 发射键 `appearance-none border-0 shadow-none`；取消键 `bg-transparent` + 顶栏陶土 hover；flush CSS 双杀 UA 按钮。
- **验证**：vitest 取消键 / 发射键契约。无 Rust 逻辑变更。

### v0.51.0 - 手写/粘贴正文触发三角色观察（2026-08-17）

- **根因（executed）**：三角色只在创世/点续写建 run；`update_scene` 30s 空闲只跑分章与 Ingest。
- **修复**：`agency/observe.rs` 同窗观察编排；观察 run `observing`/`idle` 不撞 V109；主创不改正文；编辑 `bg-observe-editor` 静默。
- **验证**：`cargo test --lib` 1463 passed / 2 ignored（+9）；vitest 592 / 3 skipped（+1）。
- **未关闭**：真机未跑；整章替换未多 200 字不重跑；分章新章等下次保存。

### v0.50.2 - 自动分章后章节名跟随章号（2026-08-17）

- **根因（executed）**：`split_chapter_in_tx` 重排只 +1 `chapter_number`；`persist_append` 在客户端旧全文更长时用客户端做底稿，把溢出写回已截断章。
- **修复**：派生标题跟随新号；`displayChapterTitle` 按章号显示；Append 前缀守卫；V130 修存量标题。
- **验证**：`cargo test --lib` 1454 passed / 2 ignored（+4）；vitest 591 / 3 skipped（+1）。
- **未关闭**：已重复的第 3–6 章正文不自动删。

### v0.50.1 - 自动分章后续写不再误报「请先打开一个章节」（2026-08-17）

- **根因（executed）**：`SCENES_PAGE_SIZE=5`，自动分章切到第 6 章后分页首页没有新章 scene；`selectChapter` 把 `sceneId` 回落成 `chapter.id`；Agency Append `get_by_id` 在 scenes 表找不到，误报 `no_scene`。文思活跃只是立刻发续写，把这条路径打出来。
- **修复**：分章补拉 `get_chapter_scenes`；`resolve_append_scene_id` 与 heal 同口径解析 chapter→scene，贯穿 `run_continue_inner` 与 `persist_append`。
- **验证**：`cargo test --lib` 1450 passed / 2 ignored（+1）；vitest 590 / 3 skipped。
- **未关闭**：真机同一开头再续写未跑，不得宣称唱反调已修复。

### v0.50.0 - 续写三角色闭环（2026-08-16）

- **根因（executed）**：回流不写当前 run 黑板；spawn 裸 emit 不落库；`start` 在信号量之后；审查 revise 不进下一拍 BeatCard。
- **修复**：活动日志 start/done 全覆盖；节拍卡投影资产栏可点；`【当前场大纲】` 落库并优先作下一拍；revise 最多 2 条进卡，兑现后 resolved。
- **验证**：`cargo test --lib` 1449 passed / 2 ignored（+13）；vitest 590 / 3 skipped（+2）。
- **未关闭**：真机同一开头再续写未跑，不得宣称唱反调已修复。

### v0.49.1 - 卸掉幕前划词润色浮条（2026-08-16）

- **根因**：v0.39.0 划词条挡住打字；v0.48.1 改成够长才出，用户仍觉得完全无用。
- **修复**：幕前编辑器不再挂载；删除 `AiSelectionActions`。改写走底部指令栏。
- **验证**：`npx vitest run` 588 passed / 3 skipped（−17）。无 Rust 逻辑变更。

### v0.49.0 - 续写大纲以正文为真相源（2026-08-16）

- **根因（executed）**：空角色表 → Producer 按标题发明费迪南；`ensure_story_outline` 不读章节；熔断仍 materialize；书大纲点名即可换 POV。
- **修复**：正文门闩；场景结构默认方法论；熔断不 Err；未接地大纲不注入；场外开篇探针；热路径 ingest。
- **验证**：`cargo test --lib` 1436 passed / 2 ignored（+18）。
- **未关闭**：真机同一开头再续写未跑，不得宣称唱反调已修复。

### v0.48.1 - 划词浮条不再挡住手工写作（2026-08-16）

- **根因**：v0.39.0 `AiSelectionActions` 任意选区即出条，拖选中浮条接住 mouseup，输入框抢焦点。
- **修复**：≥4 字 + 鼠标松开后才出；idle 无输入框；Esc 塌选区。
- **验证**：`npx vitest run` 605 passed / 3 skipped（+4）。

### v0.48.0 - 续写按镜头在场、禁止旧快照覆盖、未确认幽灵先写入（2026-08-16）

- **根因（executed）**：v0.47.0 真机《帝国的烟火》三次续写：1800 字近文当本拍在场、书大纲未落地当前席、幽灵丢弃 + 旧快照覆盖上一拍。
- **修复**：镜头 500 字阵容；去掉角色表补位；下一节点须点名本拍在场者；落库取更长底稿；末句禁复述；NewScene 不罚丢人；连续续写先写入未确认幽灵。
- **验证**：`cargo test --lib` 1418 passed / 2 ignored（+5）；vitest 601 / 3 skipped。
- **未关闭**：真机 8 次须在 0.48.0 上重跑；不得宣称五症状已修复。

### v0.47.0 - 续写质量闭合（债务/节点/阵容/状态网）（2026-08-15）

- **P0**：债务旗标、进度行累积、节点不回绕、冲突看阵容、事实写回、续写回退不走创世模板。
- **P1**：别名点名、沉寂/张力入场门闩、地点 shift、活跃冲突/目标进筛选器。
- **P2**：BeatState 状态网、末句降权、探针一次重试、角色 location 规则回写。
- **P3**：`eight_beat_append_quality_contract` CI 替代物。真机 §8.2 未跑。
- **验证**：`cargo test --lib` 1413 passed / 2 ignored（+22）。
- **未关闭**：真机 8 次幕前续写；不得宣称三症状已修复。

### v0.46.0 - 传统色主题（纸帘印 + 幕前幕后分选）（2026-08-15）

- **十二套**：竹青/朱红/群青/藤黄/绛紫/菱锰红/荷叶绿/粉绿/黛紫/鷃蓝/皮弁/汉绣绿；默认朱红。
- **分选**：顶栏色点只改幕前；设置两列；旧四套自动迁移。
- **印=锚色**：`--gold === terracotta`；幕后 cinema-gold 为暗面 brand；`--ai-accent-tint` 跟随当前窗。
- **验证**：`npx vitest run` 601 passed / 3 skipped（+11）。无 Rust 逻辑变更。
- **未关闭**：全界面截图回归；v0.42.0 §8 真机探针。

### v0.45.1 - 续写前文改为开篇+近文双窗（2026-08-15）

- **根因**：v0.42.0 只注入当前章末 800 字；HTML 标签再吃预算；节拍卡按整章点名。
- **修复**：短章全文；长章开篇 600 + 近文 1800；剥 HTML；在场只看近文；预算裁前文保章末。
- **验证**：`cargo test --lib` 1391 passed / 2 ignored（+6）。
- **未关闭**：不叠更早几章全文；v0.42.0 §8 真机探针。

### v0.45.0 - 提示词运行时组装（创世/续写/ToolLoop）（2026-08-14）

- **组装**：`assemble()` 哑拼接；创世 `writer_first_chapter` / `writer_prose_fallback`、续写 `write_beat_once`、ToolLoop head 走工厂。`to_prompt()` 不动。
- **预览**：幕后场景组合默认 Agency 续写；`timesliced`/`trishot_call3` 映射热路径。
- **P1/P2**：三工具「用法:」行；内置模板残留 `{{ident}}` CI fail-closed，运行时仍 fail-open。
- **验证**：`cargo test --lib` 1385 passed / 2 ignored（+18）；`npx vitest run` 590 passed / 3 skipped。
- **未关闭**：空资产/空末句 trim 金标；ToolLoop head 双构造；v0.42.0 §8 真机探针；P3 producer/concept_pack。

### v0.44.1 - 幕前输入框去掉系统原生描边（2026-08-14）

- **根因**：v0.44.0 拆卡片后 WKWebView 仍画 textarea 原生 inset 边；测试只查外壳。
- **修复**：`appearance-none border-0 shadow-none` + flush CSS 双杀。
- **验证**：`npx vitest run` 590 passed / 3 skipped（+1）。无 Rust 逻辑变更。

### v0.44.0 - 墨纸 / 机械视觉定向进化补齐（2026-08-14）

- **幕前**：输入无框、Medium woff2、纸 hue 95、选区 22%、顶栏 press/淡彩。
- **幕后**：warm 850–500 同色相、Panel inset 高光、弹簧 500ms（减动冻结）、侧栏选中无金框。
- **验证**：`npx vitest run` 589 passed / 3 skipped（+11）。无 Rust 变更。
- **未关闭**：全界面截图回归未做；v0.42.0 §8 真机探针仍未跑。

### v0.43.0 - 墨纸 / 机械视觉定向进化（2026-08-14）

- **幕前**：输入条 `flush`、陶土淡彩发射、取消去 pulse、霞鹜文楷本地加载。
- **幕后**：色板/阴影收软、press、Panel bezel、侧栏徽章、EmptyHint。
- **验证**：`npx vitest run` 578 passed / 3 skipped（+22）。无 Rust 变更。

### v0.42.0 - 续写按拍选取创作资产（2026-08-14）

- **筛选**：`continue_assets.rs` 确定性编译；完整卡 ≤8；未上场一行名单；脏名不进；大纲去重；前文 1 场×800 字；资料预算 6000。
- **范围**：Agency `write_beat_once` / `write_chapter`；不改 `to_prompt()`。
- **验证**：`cargo test --lib` 1367 passed / 2 ignored（+13）。
- **未关闭**：规格 §8 真机探针；表内大纲无界追加；ContextPrioritizer 未接 Agency。

### v0.41.2 - 续写 600s 超时：跳过超窗候选、散文失败不再进 tool_loop（2026-08-14）

- **网关**：`candidate_fits_prompt` 在候选循环跳过装不下当前提示的模型（Gemma 8k 不再接约 1.2 万 token 的续写）。
- **主创**：`write_beat_once` 散文回退失败直接报错，单章不再回落 `write_chapter` tool_loop。
- **验证**：`cargo test --lib` 1354 passed / 2 ignored（+4）。
- **未关闭**：本地连接超时仍可能 60s×2；`story_outlines` 膨胀；设计 §13 真机探针未跑。

### v0.41.1 - Agency 续写上线核验加固（2026-08-13）

- **核验缺口**：主创正文 `sanitize_novel_output` + 8% 自重复重试；改写路径 `resolve_rewrite_generation_mode` 永不选 TimeSliced/TriShot；划词选区不走 Append；测试环境 `finalize_session` 跳过 LLM 摘要。
- **验证**：`cargo test --lib` 1350 passed / 2 ignored（+5）；`npx vitest run` 556 passed / 3 skipped。
- **未关闭**：设计 §13 连续 8 次幕前续写真机探针未跑，不得宣称四症状已修复。

### v0.41.0 - Agency 唯一续写路径 + 幕前同章追加（2026-08-13）

- **唯一路径**：创世/幕前/幕后续写只走 Agency 三角色；幕前续写与文思活跃同章 Append（不再新建 `scenes` 行）；划词改写仍走 PlanExecutor Full/Fast。
- **硬任务**：SceneBeatCard（0 LLM）编译本拍必须完成的任务；Bundle 补情感四元组与关系；落库写回出场名/冲突/地点；债务按拍计数。
- **切断**：TimeSliced/TriShot 不再路由续写；设置 `generation_mode` 仅管改写；熔断 ≥200 必写回 / <200 拒落库。
- **验证**：`cargo test --lib` 1345 passed / 2 ignored（+17）；`npx vitest run` 556 passed / 3 skipped。
- **已知债务**：ContextPrioritizer 未接 Agency 热路径；`characters_present` id/名字混杂。

### v0.40.0 - AI 原生组件库 P3（数据展示六件套）+ P4（项目收尾）（2026-08-13）

- **P3 六组件入库** `components/ui/ai/`：AiSearchList（PromptsPanel 搜索计数区）、AiCodeBlock（六文件七处裸 pre/JSON 批量替换）、AiDiffTable（AgencyEval 检查点对比，metrics_json 解析补基准/对比列）、AiFilterTable（UsageStats 分组 tabs + 最近调用表；AiFilterChipsBar 可选接 Logs 级别筛选）、AiRecordsTable（PromptsPanel 分组行列表保留展开编辑器 + AgencyEval 判定历史/token 用量双表）、AiInsightCards（UsageStats/AgencyEval 统计卡，内嵌 MiniLineChart 静态快照替代 liveline）。
- **AiChat 关闭**：ChatComposer 为 AiPromptBar 严格子集、无多轮对话场景，设计文档 P3「AiChat」以勘察结论关闭，差异特性记录备选。
- **P4 清理**：P1-P3 替换残留 TS 13 处 + 死 CSS 约 40 类；历史死件 8 件（AiSuggestionBubble/AiHintOverlay/HelpPanel/ZenModeExit/useLlmStream/useStudioConfig/hetiAddon/Toggle，含自带测试/barrel/级联 CSS）。
- **P4 修正**：`--ai-on-accent` 令牌（契约扩至 17 变量）替换 text-white ×4；/N 透明度失效 13 处 color-mix 修复；Tasks 裸 pre → AiCodeBlock；AiDiffTable testid per-row key。
- **P4 令牌化**：AgencyEval / AgencyStudio / AgencyLearning 浅色切口关闭（AgencyLearning 裸表 → AiRecordsTable）；gray 映射固化为约定（gray-600 → `text-ai-ink-2`、gray-500/400 → `text-ai-ink-3`、表单控件 → `border-ai-line bg-ai-field text-ai-ink`）。
- **验证**：`npx tsc --noEmit` / `npx vitest run`（556 passed / 3 skipped）/ `format:check` / `architecture_guard.py` 全绿；Rust 无改动（1328 passed / 2 ignored 不变）。

### v0.39.0 - AI 原生组件库 P1+P2（共 10 组件）+ 保存 UNIQUE 修复（2026-08-12）

- **P1 生成体验五件套**入库 `components/ui/ai/`：AiLoading（幕后 3 处加载指示）、AiThinking（AgencyStudio 当前执行轨迹）、AiStreamingText（幕前幽灵续写，中文词级分词）、AiPromptBar（幕前底部指令条 + / 命令菜单）、AiApprovalCard（创建向导四选项步骤）；`--ai-*` 语义令牌 16 个双窗口各自定义，tailwind 注册 ai 色组 + 9 个动画工具；删除幕前死代码 `StreamingText.tsx` + `useStreamingGeneration.ts`。
- **P2 代理与任务五件套**入库 `components/ui/ai/`：AiContextCards（PromptCoverageBar 槽位清单）、AiToolChips（Tasks/Skills 筛选条）、AiRecommendationCard（级联改写逐段确认卡）、AiTaskRows（Tasks 任务行外壳）、AiSelectionActions（幕前划词浮条，smartExecute + insertContentAt 选区替换）；frontstage.css 补 `--shadow-float`。
- **保存修复**：幕前保存 UNIQUE 失败（scenes.story_id, sequence_number）根因修复——scene 自愈补建重定向既有关联 scene（不再补建重复行），序号被占时取 MAX+1 避让。
- **验证**：`cargo test --lib` 1328 passed / 2 ignored（+2）；`npx vitest run` 523 passed / 3 skipped（+68）；`npx tsc --noEmit` / `format:check` / `architecture_guard.py` 全绿。

### v0.38.2 - 幕后工作台深色调主题 + 代理工作室实时动态持久化（2026-08-12）

- **幕后主题底座（beautifului AI 原生改造 P0）**：4 套深色调主题（暖金/冷青/琥珀/靛紫）与幕前色调同 id 同 localStorage key 双向同步；`backstageThemes.ts` 16 变量运行时重写 + `useBackstageTheme` 全局接线（storage/Tauri 双通道）+ 设置页双预览色点选择器；warm 与现状色值逐一核对零视觉回归；删除 `useWritingStyle` 死代码。
- **问题**：v0.38.0 将 agency 事件监听提升到常驻顶层 + 全局 store，但用户仍看不到实时动态。根因：活动事件纯内存（Zustand 无 persist），macOS 隐藏 WKWebView 窗口事件送达不可靠，事件丢失即永久丢失。
- **DB 持久化**：新增 `agency_activity_log` 表（V129 迁移），`emit_activity` / `emit_progress` 在 `app.emit()` 后 fire-and-forget 写 DB（`spawn_blocking`，不阻塞创世，失败仅 warn）。
- **后端命令**：新增 `agency_list_activities`（`run_id` -> 按 `id ASC` 返回活动日志，limit 200）。
- **前端轮询**：`AgencyStudio.tsx` 新增 3s 轮询 `useQuery(['agency-activities', runId], listActivities)`；DB 活动事件为主源，live store 事件补充轮询间隔内新事件（按业务键去重）。
- **验证**：`cargo test --lib` 1326 passed / 2 ignored（+1）；`npx vitest run` 455 passed / 3 skipped（无前端测试变更）。

### v0.38.1 - 修复续写伏笔账本多字节中文切片 panic（文思活跃模式）（2026-08-12）

- **问题**：文思活跃模式续写弹 Fatal `[TimeSliced] bundle 加载任务失败: ... "end byte index 30 is not a char boundary; it is inside '指' (bytes 29..32)"`。
- **根因**：`foreshadowing_service.rs` 构造伏笔账本 title 预览用 `&content[..30]` 按字节切片，中文 content 的 byte 30 落在三字节字符「指」内部 -> Rust UTF-8 panic -> 续写 bundle 加载失败。文思活跃连续续写读伏笔账本，每次必炸。
- **主修复**：title 截取从字节语义改字符语义（`chars().count() > 30` + `chars().take(30).collect()`）。
- **同类预防**：`post_process.rs` 两处 `&draft_content[..8000/6000]` + `intent.rs` `&content[..min(200)]` 改 `floor_char_boundary`（保留字节预算，切点回退最近字符边界）。
- **回归测试**：`service_ledger_title_multibyte_no_panic` 用报错原文验证不 panic。
- **验证**：`cargo test --lib` 1325 passed / 2 ignored（+1）；`cargo +nightly fmt` ✅。纯 Rust 修复。

### v0.38.0 - 代理工作室实时显示修复与三 Agent 完善（2026-08-12）

- **问题**：幕后代理工作室（AgencyStudio）未打开时创世/续写事件丢失、打开后空白等待——事件监听挂在条件挂载的页面上，随卸载销毁。
- **实时显示修复**：事件监听提升到常驻 `App.tsx` 顶层 + 新增全局 `agencyActivityStore`（cap 200，单例无 persist），页面未开不再丢实时动态，打开即见；跨故事切换时 activeRunId 按 storyId 校正。
- **三 Agent 事件信号补齐**：概念/资产/首章/资产补齐/装配 start/done 全路径配对（含 legacy 与快速路径单点覆盖）；修复 legacy 概念完成信号角色标注（LeadWriter→Producer）；后台质检黑板写入实时推 `agency-board-changed`。
- **前端打磨**：幕前动词映射补全；幕后时间线去重改业务键，同源重复事件不再显示两次。
- **续写熔断不丢稿**：行为已由 65d90b5（v0.30.30）实现（≥600 字符降级放行/<600 丢稿），本档补齐流程级测试。
- **验证**：`cargo test --lib` 1306 passed / 2 ignored（+5）；`npx vitest run` 421 passed / 3 skipped（+17）。

### v0.37.0 - 资产回流：后台资产 agent 对已生成正文生效（2026-08-11）

- **问题**：IngestPipeline 从正文提取的角色/关系只写 kg 记忆层，续写 writer 只读生产资产表，两不相通；提取 prompt 字段名与 schema 错配、新登场角色被丢弃、Agency 续写路径不跑提取。
- **提取 prompt 写作级升级**（`resources/prompts/memory/memory_content_analysis.md`）：角色画像（情感内核/触发/创伤/需求）、双向情感关系、世界观增量（规则/历史/文化）、场景大纲、故事增量（核心冲突/转折点），与 schema 严格对齐。
- **新增资产桥**（`src-tauri/src/memory/asset_bridge.rs`）：提取结果 upsert 进生产资产表（characters / character_relationships / world_buildings / scenes.outline_content / story_outlines），新角色自动注册；源感知合并——只精炼机器来源（ingest/agency/auto_placeholder），手工编辑（user_created/manual）永不覆盖。
- **Agency 续写接入**：每章正文落库后后台自动跑提取（`spawn_asset_ingest`，含 KG 持久化）；orchestrator/TriShot 路径经 `run_ingest` 自动生效。并发安全：per-story 进程内锁 + `BACKGROUND_LLM_SEMAPHORE` 后台串行化；失败不致命，绝不影响正文落库。
- **验证**：`cargo test --lib` 1301 passed / 2 ignored（+14）；`npx vitest run` 404 passed / 3 skipped（无前端逻辑变更）。

### v0.30.46-48 - 创世持久化链路审计修复 + issue #13/#14/#15 批量修复（2026-07-31）

- **v0.30.46 创世正文未即时保存与资产缺失**：前端两条创世路径补 `setTimeout(flushSceneSave, 0)` 补偿保存；场景装配 create+update 合成单事务 + 空正文校验；`generate_chapter_outline` 写黑板身份 Producer→LeadWriter（修 `scenes.outline_content` 恒 None）；创世成功臂回读空正文即报错；空串 content 归一 None 防 COALESCE 覆盖；`create_scene` 吞错上抛；materialize 新增 foreshadowing 落库 + item_type 别名归一化 + characters upsert。
- **v0.30.47 角色谱静默失败 + llm_calls 空表（issue #13/#14）**：角色谱/文风/首场景三路径改 `extract_and_sanitize_json` 健壮解析 + 去 unwrap + warn 日志；`llm/service.rs` `prompt[..200]` 字节切 UTF-8 panic（llm_calls 永不落库根因）改 `chars().take(200)`；向导三卡片防重入；拆书页 4 处 toast 改 `extractMessage`。
- **v0.30.48 向导策略加载误报 + 快速创作空输入（issue #15）**：策略推荐加载中误显「策略加载失败」改转圈动画；快速创作简介为空先确认"仅根据标题自由发挥"。
- **验证**：`cargo test --lib` 1098 passed / 2 ignored；`npx vitest run` 352 passed / 3 skipped；tsc/fmt 全绿。

### v0.30.45 - 修复文思活跃模式续写提示词泄露（LLM 思维链泄露到正文）（2026-07-31）

用户报告文思活跃模式续写返回 LLM 思维链推理而非小说正文（提示词泄露）。根因四层叠加：①`llm/openai.rs` 的 `resolve_content` 在 `content` 为空时错误回退到 `reasoning_content`（CoT），把思维链当正文返回；②`max_tokens: 2048` 对推理模型过小，CoT 耗尽全部 token 预算导致 `content` 恒为空、整段被 CoT 占据；③`sanitize_novel_output` 仅清洗 markdown/元评论，无法识别裸 CoT 思维链；④writer 提示词从未显式禁止推理输出。修复：①移除 `resolve_content` 的 `reasoning_content` 回退（`content` 为空即返回空，不再用 CoT 兜底）；②`max_tokens` 2048 -> 4096，给推理模型留足正文预算；③新增 `detect_and_strip_bare_cot`（检测 ≥3 条 CoT 信号行触发剥离），接入 `sanitize_novel_output` 后处理；④writer 提示词新增反推理指令（禁止输出思考过程/推理链）。验证：`cargo test --lib` 1091 passed / 2 ignored（+4）；`npx vitest run` 352 passed / 3 skipped；`cargo clippy --lib` 539（零新增）；tsc/fmt/architecture_guard/format:check 全绿。

### v0.30.44 - 修复文思活跃模式续写报"生成过程异常结束，未收到有效内容"（2026-07-29）

用户报告"开启了文思活跃模式后，出现了报错的诊断信息"。诊断数据显示 LLM 成功返回 2460 字符，但前端 `generatedText` 仅剩 3 字符，打字机动画被中断，最终弹出"生成过程异常结束，未收到有效内容"。根因：`smartExecuteInFlightRef.current = false` 在 smartExecute resolve 后、内容处理前被提前清除--后台活动同步回调（100ms 防抖）在内容处理期间把 `isGenerating` 置 false，触发安全网 effect 误报；`handleRequestGeneration` 活跃模式分支错误地走了打字机幽灵文本（3 字符/帧）而非直接 `appendAiContent`。修复：移除 `handleRequestGeneration` 和 `handleSmartGeneration` 中 smartExecute resolve 后的提前清除，改为在各内容交付退出路径统一清除两标志；活跃模式分支在打字机之前直接 `appendAiContent` 绕过打字机。纯前端修复，无 Rust 变更。验证：`npx vitest run` 352 passed / 3 skipped（+2）；tsc/fmt/clippy（538 零新增）/architecture_guard/format:check 全绿。

### v0.30.43 - 修复续写内容丢失根因：flushSceneSave 读取滞后的 latestContentRef + onChapterUpdated 覆写未保存内容（2026-07-30）

v0.30.33/v0.30.34 的关闭前 flush + 序列化持久化仍未能完全解决续写内容丢失。根因：①`flushSceneSave` 读取 `latestContentRef`（RichTextEditor 200ms HTML 防抖可能滞后）而非编辑器实际 HTML，关闭/切章时最后 200ms 输入丢失；②`onChapterUpdated` 用 DB 旧内容覆写编辑器但不更新 `latestContentRef`，用户未保存输入不可逆丢失。修复：`flushSceneSave` 改读 `editorRef.getHTML()`；`onChapterUpdated` 新增守卫跳过覆写 + 同步 `latestContentRef`。纯前端修复，无 Rust 变更。验证：`cargo test --lib` 1087 passed；`npx vitest run` 350 passed / 3 skipped（+1）。

### v0.30.42 - 修复世界观生成失败（LLM 返回 markdown 代码块包裹的 JSON + 未转义引号 + 静默失败 + prompt 字段名不匹配）（2026-07-30）

issue #14 用户报告"世界观生成失败，请重试"，但日志显示 LLM API 调用成功返回内容，失败发生在下游 JSON 解析且完全无错误日志。根因：模型将 JSON 包裹在 ` ```json ... ``` ` 代码块中、或在字符串值内直接换行/使用裸双引号，`serde_json::from_str` 静默失败；`novel_creation.rs` 严格解析全量响应直接失败；prompt 要求"concepts 数组"但代码读 `world_buildings`，即使解析成功也找不到数组。三层修复：`parse_lenient` 复用 `extract_and_sanitize_json`（剥离围栏/修复裸换行/括号深度匹配）；`novel_creation.rs` 提取 `parse_world_options_response` 纯函数先剥离围栏再解析 + 失败时 `log::warn!` 记录片段；两份 prompt 修正字段名 + 新增格式约束。

### v0.30.41 - 修复续写内容被假阳性去重静默丢弃（模型回显指令 + 短文本假阳性 + 内容丢失）（2026-07-30）

- 用户诊断报告显示续写生成时 LLM（deepseek-v4）成功返回 2511 字符，但前端仅显示 6 字符（"续写\n黑暗。"），随后报"生成过程异常结束，未收到有效内容"。
- **根因链**：①模型在生成内容开头回显用户指令"续写"（非正文）；②打字机动画首帧仅 3 字符（"续写\n"），归一化后 2 字符"续写"几乎必然出现在 9656 字已有正文中；③`isTextDuplicate` 假阳性返回 true，`setGeneratedText` 跳过赋值并 `markAccepted` 存入 2 字符指纹；④生成内容被静默丢弃。
- **Fix 1·`isTextDuplicate` 最小长度守卫（`textCleanup.ts`）**：归一化后 < 30 字符的生成文本直接返回 false，不进行去重检查。
- **Fix 2·`stripInstructionEcho` 指令回显剥离（`textCleanup.ts` + `FrontstageApp.tsx`）**：新增 `stripInstructionEcho(generated, userInput)` 剥离模型回显的用户指令前缀。在 `handleRequestGeneration` 和 `handleSmartGeneration` 的 `sanitizeContinuationOutput` 后调用。
- 验证：`npx vitest run` 349 passed / 3 skipped（+13）；`tsc`/`format:check`/`architecture_guard` 全绿。纯前端修复，无 Rust 变更（cargo 基线 1082 不变）。

### v0.30.40 - 修复代理工作室不显示活动记录数据（activeRunId 仅从事件捕获 + 无 list_runs 命令）（2026-07-29）

- 用户报告"前端后台的代理工作室，没有显示代理活动的记录数据"。
- **根因（`AgencyStudio.tsx`）**：`activeRunId` 仅从实时事件捕获（三个 `listen`），IPC 查询 `enabled: !!activeRunId`--页面后开时无事件到达，`activeRunId` 恒 null，永远显示"暂无活动"。无 `list_runs` 命令发现已有 run，activity 事件 fire-and-forget 不持久化。
- **后端·`agency_list_runs` 命令（`agency/repository.rs` + `agency/commands.rs` + `handlers.rs`）**：`list_runs_for_story(story_id, limit=20)` 按 `created_at DESC` 列出 story 的全部 run。
- **前端·activeRunId 水合（`AgencyStudio.tsx`）**：新增 `listRuns` 查询 + `useEffect` 取最新 run 水合 `activeRunId`，不依赖实时事件。
- **前端·历史时间线重建（`AgencyStudio.tsx`）**：时间线三源合并（live 事件 + board items 历史重建 + run 生命周期），去重排序。无需新表/迁移。
- **前端·Run 选择器（`AgencyStudio.tsx`）**：新增 `<select>` 下拉框切换浏览历史 run。
- 验证：`cargo test --lib` 1082 passed（+1）；`npx vitest run` 339/3 skipped（+3）；`cargo clippy --lib` 538（baseline 540，-2 修复既有）；`tsc`/`fmt`/`architecture_guard`/`format:check` 全绿。

### v0.30.39 - 修复续写不按故事大纲推进剧情（TimeSliced 路径缺失 build_progression_anchor）（2026-07-29）

- 用户报告"续写和故事大纲仍然缺乏强关联"、"没有按照故事大纲来写剧情和推进剧情"。
- **根因（`agents/orchestrator.rs`）**：v0.30.31 引入的 `build_progression_anchor`（注入故事大纲硬约束 + 已推进进度指针 + 世界观规则 + 显式调和指令）**只在 TriShot 路径（`execute_trishot`）调用，从未移植到 TimeSliced 路径（`execute_time_sliced`）**。而 TimeSliced 是默认续写路径（`generation_mode = "auto"` 路由续写到 TimeSliced）。TimeSliced writer 有完整大纲但无进度指针，无法判断当前在故事大纲哪个节点 -> 偏离大纲、原地踏步、仅复述设定。
- **Fix（`agents/orchestrator.rs` `execute_time_sliced`）**：在 prompt 模板渲染后、`ending_anchor` 注入前插入 `build_progression_anchor(&bundle, pool.inner(), &task.context.story.story_id, chapter_number, &user_instruction)` 调用，与 TriShot 路径完全对齐。`story_id` 在 `spawn_blocking` 闭包中被 move，改用 `&task.context.story.story_id`。
- 验证：`cargo test --lib` 1081 passed；`cargo check`/`tsc`/`vitest`（336/3 skipped）/`fmt`/`clippy`（539 零新增）/`architecture_guard`/`format:check` 全绿。

### v0.30.38 - 修复续写输出被编辑器元评论污染（is_prose_request 被 serde 默认 false）（2026-07-30）

- 用户报告"第三次续写时出的错"--续写产出正文后紧接 AI 文学编辑元评论（"好的，作为一名专业的文学编辑，我将根据您提供的问题列表和总体评分…"）。续写误路由 bug 第 6 次复发。
- **根因（三层叠加）**：①分类提示词"继续写"示例省略 `is_prose`，LLM 若遵循示例返回合法 JSON 但缺该字段，serde `#[serde(default)]` 填 `is_prose_request=false`；②serde 默认值（false）与 LLM 失败兜底值（true）相反，且 partial-but-valid JSON 走 `parse_classification_json` 成功解析、`is_fallback=false` 被缓存，后续相同输入持续返回毒化 false；③`sanitize_plan_for_prose_request` 门控仅检查 `is_prose_request`，false 时跳过全部净化 -> SING 多步计划 `[writer, inspector, builtin.style_enhancer]` 未拦截 -> `final_content` = style_enhancer 元评论覆盖 writer 正文。
- **Fix 1·后置不变量（`intent.rs`）**：`parse_classification_json` 成功反序列化后若 `is_continuation || is_new_novel` 但 `is_prose_request=false`，强制设 `true`。
- **Fix 2·提示词示例补全（`intent.rs`）**："继续写"示例补 `is_prose=true`。
- **Fix 3·sanitize 门控扩展（`planner/mod.rs`）**：门控从 `is_prose_request` 扩展为 `is_prose_request || is_continuation`（纵深防御）。
- 验证：`cargo test --lib` 1081 passed（+4 回归）；`cargo check`/`tsc`/`vitest`（336/3 skipped）/`fmt`/`clippy`（540->539）/`architecture_guard`/`format:check` 全绿。

### v0.30.37 - 修复创作生成失败时 toast 显示 "[object Object]"（issue #12）（2026-07-29）

- 用户反馈 issue #12：创作/生成失败时错误提示显示 `[object Object]`。根因与 issue #11（v0.30.31 修复"获取模型列表"）同源：后端 `AppError` 自定义 `Serialize` 产出普通 JSON 对象 `{ code, message, severity, data? }`，Tauri v2.4 作为普通对象（非 JS `Error` 实例）投递到前端 catch 块；前端用 `String(err)` 或 `err instanceof Error ? err.message : String(err)` 转字符串，对普通对象产出 `[object Object]`，可读 `message` 被丢弃。v0.30.31 的 `extractMessage` helper 只覆盖"获取模型列表"，创作/生成错误路径未迁移。
- **主修复·统一改用 `extractMessage`（10 个前端文件，36 处）**：`FrontstageApp.tsx`（5 处：smart_execute 主/次 catch + 修稿/审稿/定稿）/ `SceneEditor.tsx`（2 处：生成大纲/草稿）/ `Stories.tsx`（4 处：快速创作/向导创作/风格保存/风格生成）/ `RichTextEditor.tsx`（2 处：文思生成/排版）/ `WenSiPanel.tsx`（2 处：自动续写/修改）/ `usePipeline.ts`（6 处）/ `CharacterStatePanel.tsx`（1 处）/ `Skills.tsx`（7 处）/ `PromptsPanel.tsx`（5 处）/ `useUpdater.ts`（2 处）。`extractMessage` 依次尝试结构化 AppError 对象取 `.message` -> `Error.message` 内嵌 JSON 解析 -> 普通 Error `.message` -> 字符串 -> 带 `.message` 对象 -> 兜底 `'Unknown error'`。不动 `main.tsx`/`ErrorBoundary.tsx`（已优先取 `.message`）。
- **回归测试（`src/utils/__tests__/errorHandler.test.ts`，+8）**：AppError 普通对象提取 `message`（断言不等于 `[object Object]`）/ 带 `data` / `parseStructuredError` 识别 / `Error.message` 内嵌 JSON / 普通 Error / 字符串 / 带 `.message` 对象 / 兜底文案。
- 验证：`npx tsc --noEmit` ✅；`npx vitest run` 336 passed / 3 skipped（+8）；`npm run format:check` ✅；`architecture_guard` ✅。纯前端，cargo 基线 1077 不变。

### v0.30.36 - 修复首次创世指令不保存到输入历史（按↑调取不到）（2026-07-29）

- 用户报告输入框历史输入内容没保存、按↑调取不到。根因：首次创世（无已有故事）时 `currentStory=null`，`handleInputSubmit` 的 `if (sid) saveInputHistory(...)` 跳过保存，创世指令从未持久化；isBootstrap 分支 `setCurrentStory(null)` 清空历史，创世成功后新故事历史为空。v0.30.23 修复意图分类后创世指令正确走 isBootstrap 路径，暴露了此前被续写误分类掩盖的缺陷。
- **主修复（`FrontstageApp.tsx`）**：`handleSmartGeneration` 的 `story_created` 处理块在 `setCurrentStory(新故事)` 后同步 `saveInputHistory(新故事ID, [创世指令, ...])`，useEffect 随后 `loadInputHistory` 即可读到。关键时序：写入在 useEffect 触发前同步执行（同一同步块无 await）。
- 验证：tsc ✅；vitest 328 passed / 3 skipped（+2）；format:check ✅；architecture_guard ✅。纯前端，cargo 基线 1077 不变。

### v0.30.35 - editor 质检后台异步化：首章立即显示 + 后台质检 + toast 反馈（2026-07-29）

- 用户报告创世顶满 600s 超时无产出。根因：editor 质检（`review_and_assemble` 中的 `evaluate_gate`）在 Scene 装配落库**之前**同步执行，被 `tokio::time::timeout(600s)` 包裹；producer（深度资产 ~30-60s）+ writer（tool_loop ~4-5min）花约9分钟后 editor 只剩约1分钟，其 `editor_verdict_prose_fallback` 用固定 300s timeout 发起 LLM 调用，34s 后被硬 600s 砍掉，既未完成质检也无法走 `salvage_failed_gate` 保产出，整 run 超时无首章返回。
- **后端·装配与质检分离（`coordinator.rs`）**：①新增 `assemble_only`（pub(crate)）从 `review_and_assemble` 提取纯装配部分（`cleanup_prose_for_persist` 抗重复三件套 + `SceneRepository::create/update` 落库），不含 editor 质检与修订。②新增 `spawn_editor_qc`--测试环境 `app_handle=None` 时 no-op；生产环境 `tokio::spawn` 后台任务，用 `Some(Instant::now() + 300s)` 独立 deadline（不受 smart_execute 600s 限制）调 `evaluate_gate_impl`，结果三态分支：`Passed` -> `{passed:true,salvaged:false}`；`RevisionRequired` -> `{passed:false,issues}`；`Failed` -> 先 `salvage_failed_gate`（草稿≥600字保产出）-> `{passed:true,salvaged:true}` 或 `{passed:false,issues}`；`Err` -> 降级放行 `{passed:true,salvaged:true}`。emit `genesis-qc-result` 事件 + `emit_activity(EditorAuditor,"后台审查")`。③`genesis_fastpath` / `run_genesis_legacy_inner` Phase C 改为 `assemble_only` + `spawn_editor_qc`，返回 `revised:false, verdict:EditorVerdict::pending()`。④删除无用 `review_and_assemble`；`EditorVerdict` 新增 `pending()`；新增 `EVENT_GENESIS_QC_RESULT` 常量。
- **前端·后台质检结果 toast（`FrontstageApp.tsx`）**：`setupEventListeners` 新增 `genesis-qc-result` 监听，三态：质检通过 -> `toast.success`；降级放行（审计超时/失败但首章已保留）-> `toast.warning`；不合格 -> `toast.warning('质检不合格，建议重新创世。问题：…')`。后台 editor 不影响 `isGenerating`，用户可继续写作；不自动重新创世，由用户手动决定。
- **producer 深度资产保持前台**：`producer_depth_assets` 已是单次 `complete_json` 调用（约30-60s）非瓶颈，且保障首章不脱节（v0.30.29 修复点）。移 editor 后台后用户在 writer 完成即可见首章（约5-6min vs 此前10min 超时）。
- 验证：`cargo test --lib` 1077 passed（+2；移除 3 个不适用的 genesis 同步质检测试，`test_editor_verdict_prose_fallback` 改为直接测 `evaluate_gate`）；clippy 539 零新增；tsc/vitest(326/3 skipped)/fmt/architecture_guard/format:check 全绿。

### v0.30.32 - 增强性指令纳入世界观/故事大纲/场景大纲/上下文强关联（2026-07-28）

- 承接 v0.30.31 让资产彼此强关联后，补齐增强性指令（logline 后缀）未纳入强关联的缺口：增强后缀生成时不看世界观，进入管线后又与资产各居一隅、互不交叉引用。
- **P0-A 增强生成纳入世界观**：`build_logline_context_sync` 新增拉 `world_buildings`（concept+rules前3+history 截断1000）为 `world_setting` 字段；`agency_logline_suffix_contextual.md` 新增 `## 世界观设定` 段 + "后缀须与世界观规则一致"输出要求。
- **P0-B TriShot 指令纳入 `build_progression_anchor` + 显式调和**：签名加 `user_instruction`，指令非空时作为首个段注入；收尾改为显式调和（资产=硬约束，指令=创作方向，在硬约束内落实指令核心意图，冲突时调整指令以符合约束但保留核心意图，不得因约束丢指令也不得因指令违反约束）。
- **P1-C 创世指令-资产调和**：`writer_first_chapter`/`writer_prose_fallback` 写作要求增"故事前提是创作方向；创作资产是硬约束，须在硬约束内落实前提核心意图"；fallback 补回"资产区为准"系统提示。
- **P1-D TimeSliced 指令-资产调和**：`orchestrator_timesliced_writer.md` + fallback 字符串加"写作指令须与上下文协调一致；冲突时在遵循硬约束前提下落实指令核心意图"。
- 验证：`cargo test --lib` 1078 passed（+1）；clippy 540 零新增；tsc/vitest(322/3 skipped)/fmt/architecture_guard/format:check 全绿。

### v0.30.31 - 续写链路修复：世界观/故事大纲/场景大纲注入与剧情推进方向（2026-07-28）

- 幕前续写实际走 Legacy TriShot，但故事大纲/场景大纲/世界观三者均不到达 writer（TriShot `final_prompt = Call1 LLM 合成`，manifest 不含 story_outline）。新增 `build_progression_anchor` 确定性注入【剧情推进方向】段（故事大纲 + 场景大纲 + 已推进进度 + 世界观 + 推进约束），无论 Call1 合成质量如何都到达 Call3 writer。
- WriteTimeBundle 新增 `world_setting` 字段（读 world_buildings 表 concept/rules/history/cultures），manifest 增加 story_outline/world_setting 清单项，scene_outline 纳入 outline_content。
- 进度指针：用现有 `scenes.outline_content` 回读最近 3 章作为"已推进到哪"，无 DB 迁移、无 schema 变更。
- `scene_outline.md` 修"按序号定位节点"伪前提为"按已推进进度定位"；Legacy（`creation_commands.rs`/`service.rs`）与 Agency（`generate_chapter_outline`）双路径注入 world + progress。
- Agency `build_continue_writer_context` 世界观全字段 + 前文保底（修复阈值倒挂 >8000->>12000）+ 进度指针；`ensure_world_building` concept 存全文 + rules 解析落库；editor 质量门预注入参照资产。
- 验证：`cargo test --lib` 1077 passed（+2）；clippy 540 零新增；tsc/vitest/fmt/architecture_guard/format:check 全绿。

### v0.30.26 - 统一 Logline 增强提示为内联幽灵文本 + 修复分时预检缺少角色（2026-07-27）

- 将 v0.30.24 的独立 `.frontstage-logline-hint` 建议条改为输入框内跟在已输入内容后的幽灵后缀
- 新增 `agency_logline_suffix` prompt，后端只返回应追加的后缀
- 按 `→` 追加后缀，Enter 提交“原输入 + 增强后缀”组合文本
- 简化 `FrontstageApp`：移除 `originalInputForLoglineRef` 与 `intentClassificationInput` 透传
- 修复分时预检缺少角色：意图分类兜底按输入文本判断创世意图；`QuickPreflightChecker` 自动创建占位主角；前端接受 logline 提示后用原输入做意图分类
- 验证：`cargo test -p storymoss` 1060 passed；`npx vitest run` 310 passed / 3 skipped

### v0.30.23 - 意图分类 Bug 修复（LLM 分类去偏 + 失败兜底上下文化）（2026-07-23）

- 修复"写一部现代间谍的长篇小说"被误分类为续写导致 `VALIDATION_FAILED`
- 提示词去偏：移除 `已有故事=true` 上下文注入 + 新增正例 + 移除保守措辞
- 上下文感知兜底：LLM 失败时无故事返回创世，有故事返回续写
- 不缓存失败结果：仅 LLM 成功解析写入缓存
- 前端兜底上下文化：LLM 失败兜底用 `stories.length === 0` 替代硬编码 `false`
- 设计原则：LLM 是意图判断的唯一权威，不回到硬编码关键词匹配

### v0.30.22 - PROBLEM 七元素框架集成（Logline 生成 + 故事大纲增强）（2026-07-22）

- 新增 Erik Bork PROBLEM 七元素（Punishing/Relatable/Original/Believable/Life-Altering/Entertaining/Meaningful）作为后端创作资产
- 新增提示词文件 `agency_problem_logline.md` / `agency_problem_outline.md`
- DB V114 迁移新增 `stories.logline` 列；Story 模型与 StoryRepository 同步改动
- `generate_logline`：简单 premise（< 100 字符）触发 logline 生成并替换原 premise
- `ensure_story_outline`：从注册表加载 PROBLEM outline 提示词 + 注入 logline 上下文
- `producer_depth_assets` outline 字段注入 PROBLEM 指导；`build_continue_writer_context` 以【故事Logline】注入
- `cargo test --lib` 974 passed（+3 logline 测试）；clippy baseline 550 无新增告警

### v0.30.21 - 续写资产层级生成：世界观 -> 故事大纲 -> 章节大纲 -> 正文（2026-07-22）

- 续写 `ensure_assets` 扩展：角色检查后追加 world_buildings / story_outlines 检查，缺失时调 `ensure_world_building` / `ensure_story_outline` 单次 Producer LLM 调用生成并落库（不抢主创 LLM）
- `build_continue_writer_context` 注入故事大纲；`generate_chapter_outline` 在 writer tool_loop 前生成章节大纲（服从故事大纲），strict writer task 含故事大纲 + 本章大纲 + 写作要求
- `handle_gate` 存 `scenes.outline_content`，形成"世界观 -> 故事大纲 -> 章节大纲 -> 正文"层级约束链
- `cargo test --lib` 971 passed

### v0.30.20 - Agency 续写效率优化与质量门硬化（2026-07-22）

- 续写 run_deadline（`run_continue`/`run_continue_batch` 调 `setup_run_deadline`）
- 续写 writer 散文回退（`writer_prose_fallback` 参数化 `chapter_key`；`write_chapter` 熔断回退）
- 续写 writer 上下文预注入（`build_continue_writer_context` 读 DB 角色/世界/场景）
- Editor 质量门 deadline（`evaluate_gate_impl` 加 deadline，v0.30.19 fallback 使安全）
- Editor 草稿预注入（task 注入 `draft.content`，省 1 轮 board_read）
- 连接超时调优（`llm_connect_timeout_secs` 60s -> 15s）
- `cargo test --lib` 967 passed（+2）

### v0.30.19 - 修复质量门编辑审计 Agent 熔断（2026-07-23）

- **问题**：Agency 创世/续写质量门 editor_auditor 的 ReAct tool_loop 在本地模型（Qwen 3.6）不遵从 JSON action 格式时连续解析失败/达最大轮数（6 轮）熔断，原实现直接返回 `GateOutcome::Failed` 导致整 run 失败。
- **Fix（两层兜底，`coordinator.rs`）**：①salvage--熔断时仍先 `parse_lenient` 尝试从末轮输出提取裁决 JSON；②散文回退--新增 `editor_verdict_prose_fallback`，单次 `llm.complete()` 直接请求裁决 JSON（不经 tool_loop/工具），与 `writer_prose_fallback`（v0.30.3）同理。回退失败才降级 Failed。
- **验证**：`cargo test --lib` 965 passed（+1 正向回归）；fmt / clippy / architecture_guard 全绿。

### v0.30.18 - 修复幕前意图分类 null 崩溃（2026-07-23）

- **根因**：`handleSmartGeneration` 调 `classifyIntent` 后直接读 `classification.is_new_novel`；`classifyIntent` resolve 为 null 时不抛异常，catch 无法拦截，`null.is_new_novel` 崩溃。E2E mock 对未注册命令返回 null（`classify_intent` 未 mock）触发，v0.30.16 两次 CI 均 hit，连带 6 个 E2E 失败。
- **Fix（FrontstageApp.tsx）**：catch 后新增 post-catch null 兜底（续写语义，与 catch 一致）+ 不缓存 null。
- **macOS 构建失败（v0.30.16 tag）**：`Info.plist Io(code 5)` runner 瞬时 I/O flake，已 `gh run rerun --failed` 重建。
- 验证：tsc ✅；vitest 307 passed / 3 skipped；format:check ✅。纯前端，cargo 964 不变。

### v0.30.17 - 幕前顶部创世状态显示三 Agent 动作/进度（2026-07-23）

- **背景**：用户反馈幕前顶部创世流程状态提示信息不足，看不出「主创在干嘛、做完了什么工作」。底部 LLM 连接状态未改动。
- **新增 `useAgencyAgentActivity` hook**：幕前订阅后端已有的 `agency-agent-activity` 事件（此前仅幕后 AgencyStudio 消费），按 主创/管理/编辑审计 顺序聚合各角色最新活动，产出文案（进行中「主创正在写第一章」、已完成「管理已完成深度资产」）；run 结束自动清空。
- **接线 FrontstageHeader**：顶部状态栏在 orchestratorStatus 之后渲染各 Agent 进度（进行中琥珀 saving、已完成绿色 saved），无活动时不占位。
- **附带**：AGENTS.md 强制构建规则 #2 改为「本地构建仅在用户明确要求时执行」（用户级永久指令）。
- 验证：tsc ✅；vitest 307 passed / 3 skipped（+2）；format:check ✅。纯前端，cargo 基线 964 不变。

### v0.30.16 - 故事资产手动编辑（补齐编辑缺口）（2026-07-22）

- **背景**：审计后台发现 故事大纲/故事摘要 只读展示（update hook 零调用），伏笔无内容编辑+删除，角色关系无编辑。角色/世界构建/场景已有完整编辑。
- **Gap 1 故事大纲编辑**：Stories.tsx 只读 `<p>` 改为 查看/编辑 切换，调 useUpdateStoryOutline。
- **Gap 2 故事摘要编辑**：KnowledgeGraph.tsx 抽取 SummaryCard 组件，查看/编辑 切换，调 useUpdateStorySummary。
- **Gap 3 伏笔内容编辑+删除**：后端 ForeshadowingTracker 新增 update/delete 方法 + 命令 + 注册；前端 useUpdate/DeleteForeshadowing hook + Foreshadowing.tsx 编辑表单/删除按钮。
- **Gap 4 角色关系编辑**：前端 useUpdateCharacterRelationship hook（后端已存在）+ RelationshipCard 编辑表单。
- **验证**：cargo test --lib 964 passed；vitest 305 passed；tsc/fmt/clippy(零新增,baseline 550)/arch_guard 全绿。

### v0.30.15 - 场景围绕故事大纲生成（创作原则加固）（2026-07-22）

- **根因 A**：`generate_scene_outline` 复用故事级 `outline_planner.md` 提示词且不注入 story_outlines.content，模型幻觉新角色"金敏秀"（不在角色卡），场景大纲与故事大纲冲突。
- **根因 B**：续写走 TimeSliced/TriShot，prompt 从不包含故事大纲（只在 Full/Fast 路径计算），导致内容偏离大纲。
- **Fix A（场景大纲生成锚定故事大纲）**：新增场景级提示词 `scene_outline.md`（强制复用已登场角色、禁止发明新角色、围绕故事大纲对应节点展开）；`generate_scene_outline` 加载 story_outlines.content + 场景序号注入 task.parameters；`build_outline_prompt` 分流（场景模式用 scene_outline，workflow 故事级仍用 outline_planner）。
- **Fix B（writer 锚定故事大纲）**：WriteTimeBundle 新增 story_outline 字段，load_sync 加载 story_outlines.content，to_prompt 在红线之后插入权威段【故事大纲（本场景必须围绕此大纲展开，禁止偏离）】；冲突时以故事大纲为准并使用已登场角色。一处覆盖 TimeSliced+TriShot。
- **验证**：`cargo test --lib` 964 passed（+4）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。

### v0.30.14 - 续写返回风格增强模板修复（多步 plan 尾部非 writer 覆盖正文）（2026-07-22）

- **根因（结构性）**：`execute_plan`（executor.rs:685-687）用**最后产出 `content` 的步骤**作为 `final_content` 返回用户。force-correction（防线 2）只修正首步，无法拦截多步 plan **尾部**的 `style_enhancer`/`inspector`--尾部非 writer 的模板/报告覆盖 writer 正文。用户报告"增强第二章"得到 `[inspector, style_enhancer]` 多步 plan，style_enhancer 收到 inspector 报告后抱怨"这是一份质量检查报告而非章节原文"。这是该误路由 bug 第 5 次复发（v0.30.10/11/12/13 各堵一条路径，多步尾部漏网）。
- **Fix（防线 3，`planner/mod.rs` + `planner/executor.rs`）**：新增 `PlanGenerator::sanitize_plan_for_prose_request`，在咽喉点 `execute_with_context`（force-correction 之后）对所有 `is_prose_request` plan 统一净化：①移除 `builtin.style_enhancer`/`text_formatter`/`character_voice`/`emotion_pacing` 等绝不产出正文的技能；②续写塌缩单 writer；③其余 prose 请求弹出尾部非 writer，保证末步 writer（保留 `[inspector, writer]` Rule 9 流）；④空则补 writer。非 prose（Audit）不净化。
- **验证**：`cargo test --lib` 960 passed（+12 sanitize 回归）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。

### v0.30.13 - 续写返回风格增强模板修复（SING 路径绕过 force-correction）（2026-07-22）

- **根因**：planner force-correction（防线 2）只在 `PlanGenerator::generate_plan` 内施加，而 `PlanExecutor::execute_with_context` 的 SING（IntentionGraphPlanner）路径直接返回 plan、完全绕过 `generate_plan`。当 SING 把续写路由到 `builtin.style_enhancer`（Skill 资产）作为首步时，force-correction 从不执行，style_enhancer 收到空 content 返回"请提供需要增强的原始文本"模板。v0.30.11 禁用模板重放消除了模板路径，但 SING 路径的绕过漏洞仍在。
- **Fix（结构修复，`planner/mod.rs` + `planner/executor.rs`）**：提取 `PlanGenerator::force_correct_first_step_to_writer` 为 `pub(crate)` 方法，在 `generate_plan` 与 **plan 执行咽喉点** `execute_with_context`（所有 plan 来源 SING/PlanGenerator/fallback 必经，`execute_plan` 之前）统一施加。SING 路径产生的 `builtin.style_enhancer`/`inspector`/`outline_planner` 等首步经咽喉点修正为 `writer`。幂等：已为 writer 的首步不受影响，两处重复调用安全。
- **验证**：`cargo test --lib` 948 passed（+4 咽喉点回归）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550 -> 549）。

### v0.30.12 - 续写返回审查报告修复（force-correction 漏拦 inspector）（2026-07-22）

- **根因**：planner force-correction（planner/mod.rs 防线2）的"强制改 writer"capability 列表漏掉 `inspector`；planner 提示词 Rule 9/21 也允许 inspector 用于有内容的请求。本地模型(Gemma)把续写误路由到 inspector，输入"继续写当前这部小说"后得到 inspector 质检员的审查报告而非续写正文。
- **Fix A（planner/mod.rs）**：提取纯函数 `PlanGenerator::should_force_correct_to_writer`（可单测），将 inspector 纳入 swap-to-writer 列表，按 LLM 分类分流：续写/创世/无分类/审查+prose 强制 writer；纯 Audit(非prose)/Rewrite 保留 inspector。
- **Fix B（提示词·mod.rs）**：Rule 9 澄清续写≠refine、Rule 21 加入 inspector 禁用。
- ✅ **验证**：`cargo test --lib` 944 passed（+8）；`npx vitest run` 305 passed；tsc / fmt / clippy / format 全绿。

### v0.30.11 - LLM 意图分类器替换朴素子串匹配（6 处高危路由点修复）（2026-07-20）

- **背景**：全代码库用朴素子串匹配（`user_input.contains(pattern)`）做意图路由，误路由频发（如"这部小说"误匹配"继续写当前这部小说"、否定句"不要新建"仍判为新建等）。
- **新增 IntentParser::classify_writing_intent**：单次 LLM 调用产出全部路由决策（is_new_novel / is_continuation / task_type / is_prose_request / input_clarity / detected_genre / confidence）；8s 超时 + 保守回退（is_new_novel=false=续写）+ 会话 LRU 缓存。
- **修复 6 处高危路由点**：is_novel_creation_intent 子串误判 / find_template 被 disabled 误禁 / from_instruction_and_context 优先级 bug / force-correction 扩展 / extract_genre 否定句漏判 / intention_graph builder。
- **前端**：新增 classifyIntent API，删除 isNovelCreationIntent / isContinuationIntent；修复字段名别名 bug（提示词 is_prose 与结构体 is_prose_request 不一致）。
- ✅ **验证**：`cargo test --lib` 936 passed；`npx vitest run` 305 passed；tsc / fmt / clippy / architecture_guard 全绿。

### v0.30.10 - 续写返回风格增强模板修复（模板匹配误路由 + content 空兜底）（2026-07-20）

- **根因**：`PlanTemplateLibrary::find_match` 用朴素 substring 匹配，之前记录的 style_enhancer 计划的触发词（如"这部小说"）会匹配"继续写当前这部小说"，导致续写请求跳过 planner LLM 和所有安全规则，直接重放 style_enhancer 计划。style_enhancer 收到空 content 后返回"在您提供文本后，我将从以下几个方面进行增强"模板而非续写正文。
- **Fix A（executor.rs 主修复）**：`execute_with_context` 在 `find_template` 前检测续写意图词，命中则跳过模板匹配，强制走 planner LLM 路径。
- **Fix B（mod.rs 防线 2 扩展）**：force-correction 扩展到 `style_mimic` / `plot_analyzer` / `builtin.style_enhancer` 等，prose/续写关键词触发时强制改为 `writer`。
- **Fix C（executor.rs content 兜底）**：新增 `inject_content_fallback`，为 `style_mimic` / `plot_analyzer` / `builtin.*` 在 content 为空时按 depends_on -> step_outputs -> current_content_preview 注入文本。
- **Fix D（mod.rs Rule 21 强化）**：Rule 21 新增"继续"/"续写"关键词，禁止 `style_mimic` / `builtin.style_enhancer` 用于 prose 请求。
- ✅ **验证**：`cargo test --lib` 929 passed（+5）；fmt / clippy 无新增告警。

### v0.30.9 - 续写返回 Inspector 审查模板修复（draft 空内容兜底注入）（2026-07-20）

- **根因**：legacy planner 的 LLM 生成的 ExecutionPlan 中 inspector 步骤常遗漏 `"draft": "{{step_N}}"` 参数。`execute_inspector` 仅从 `params["draft"]` 读取待检查正文，缺失时 `task.input` 为空串，`build_inspector_prompt` 渲染出"【待检查内容】部分为空"的模板文本，Inspector 直接将该模板作为"审查结果"返回，用户看到审查模板而非续写正文。
- **Fix A（主修复·executor.rs）**：`resolved_params` 块新增 inspector draft 兜底注入--当 `capability_id == "inspector"` 且 `draft` 为空时，按 `depends_on` 顺序查找 writer 步骤的 `step_outputs["content"]`，找不到则扫描全部 `step_outputs`，自动注入非空 content 作为 `draft`。提取为可测静态方法 `inject_inspector_draft_fallback`。
- **Fix B（提示词·mod.rs）**：planner 提示词 Rule 9 强化--明确要求 inspector 必须使用 `"draft": "{{step_id}}"` 传参；JSON 示例增加 inspector 步骤示范 `"draft": "{{step_1}}"` + `depends_on: ["step_1"]`。
- ✅ **验证**：`cargo test --lib` 924 passed（+5：inspector draft 兜底注入 5 场景）；fmt / clippy 无新增告警。

### v0.30.8 - 全面修复 nullable 列读取（Invalid column type Null 系列）（2026-07-20）

- **根因**：`world_buildings.cultures`（index 5）/ `rules`（index 3）在基础 schema 为 nullable TEXT，旧数据该列为 NULL，repository 用 `row.get(N)?` 读非空 `String` 即报 `Invalid column type Null`。与 v0.30.6 `dynamic_traits` NULL 同类。
- **全面排查**：系统性审查全部 27 个 repository 文件，发现并修复所有 nullable 列被当作非空 `String` 读取的问题（共 8 个文件、31 处）：`world_building_repository` / `scene_repository`（× 4 方法）/ `scene_version_repository`（× 2 方法）/ `studio_config_repository` / `writing_style_repository` / `knowledge_graph_repository`（attributes × 4 / evidence × 2）/ `user_preference_repository`（6 列 × 2 方法）。全部改为 `Option<String>` + 兜底。
- **迁移**：V112 回填 `world_buildings.cultures/rules`；V113 全面回填 scenes/scene_versions/studio_configs/writing_styles/kg_entities/kg_relations/user_preferences 所有 nullable JSON/TEXT 列。
- ✅ **验证**：`cargo test --lib` 919 passed（+2）；fmt / architecture_guard 全绿。

### v0.30.7 - 计划执行失败修复（LLM 在 depends_on 写入上下文名）（2026-07-20）

- **根因**：LLM 生成的 ExecutionPlan 在 `depends_on` 中混入上下文名（如 `"Story Context"`、`"writer"`）而非 plan 内 step_id。`topological_sort`（swarm.rs）已正确跳过非 step_id 依赖，但 `PlanExecutor::execute` 的依赖校验未对齐--遇到非 step_id 依赖直接判 `not found`，导致 step_1 被跳过 -> step_2 依赖 step_1 也 not found -> step_3 链式失败，整 plan 崩溃。
- **Fix（executor.rs）**：依赖校验前收集 `plan_step_ids` 集合，对不在集合中的依赖（非 step_id）跳过校验并 `log::warn`，与 `topological_sort` 行为一致。
- **Fix（mod.rs）**：Rule 3 强化--明确 `depends_on` MUST ONLY contain step_id values of OTHER steps in this same plan，NEVER put context names / capability names / free text。
- ✅ **验证**：`cargo test --lib` 917 passed（+2：topological_sort 非 step_id 依赖跳过 + 混合依赖排序）；fmt / tsc / architecture_guard 全绿。

### v0.30.4 - 幕前输入历史持久化（按故事隔离）（2026-07-20）

- 幕前底部输入框已输入内容现长久保留，关闭窗口/重启后不丢失，与编码工具一致。每条提交按故事 ID 隔离存入 `localStorage`（`frontstage:inputHistory:<storyId>`，最近 20 条），切换故事自动加载该故事的历史。
- 保留既有 ghost-hint 交互（↑/↓ 切换 LLM 建议 <-> 历史记录，-> 确认填充），持久化对导航无侵入。localStorage 不可用时静默降级为内存态。
- ✅ **验证**：`npx vitest run` 297 passed（+2：持久化写入 + 重载召回）；tsc / prettier 通过。纯前端，无 Rust 变更。

### v0.30.1 — 创世提速（Genesis Fastpath）（2026-07-19）

- 创世从 12-18 次串行 LLM 调用压缩为三阶段 4 次：Phase A 概念包单调用 → Phase B 主创首章 ∥ 管理深度资产（多模型 `tokio::join!` 并行）→ Phase C 编辑质量门/修订/装配（与 legacy 共用），典型远程模型首章 ≤3 分钟。
- 主创模型优先：多模型时 Tool 档自动分配排除 active/creative 模型（TTFB 排序与健康回退两分支均生效，无候选时回退允许 active 不饿死 Tool 档），管理/编辑不再与主创同模型；单模型时主创先出首章，资产与审查随后串行。
- 单调用解析失败自动回退原串行多轮流程（概念包结果复用）；取消信号直接传播收敛为 cancelled，不误入 legacy 回退、不产生 fallback 遥测。
- smart_execute 超时回退统一为 600s（原配置加载失败时回退 180s）。
- ✅ **验证**：`cargo test --lib` 883 passed；`npx vitest run` 295 passed；`npm run type-check` 通过；architecture_guard PASSED；`cargo +nightly fmt --check` 通过。

### v0.30.0 — Agency P5：持续学习 + 代理可视化（2026-07-19）

- 持续学习双轨：观察层（`observations.jsonl`，10MB 轮转，label 过滤防自观察）→ 后台 analyzer（Background 档模型，`agency_analyze_learning`）→ instinct（trigger/action/confidence 文件层）。
- 置信度引擎：按证据初始化 + 采纳 +0.05 / 纠正 −0.1 / 周衰减 −0.02 / 低置信度 prune。
- 晋升管线：置信度 ≥0.8 且跨 story 复现 → 学习中心确认 → 物化为 `skill.yaml` 技能（重启自动 reload）。
- 学习中心页（`AgencyLearning`）：模式列表 + 置信度 + 晋升提案 + 观察流 + 手动分析；代理工作室页（`AgencyStudio`）：三角色实时状态卡 + 黑板视图（事件驱动刷新）+ 活动时间线。
- eval 场景纳入 CI 专用门禁 step；检查点对比 UI；story 级 token 聚合；rule grader 追读力口径对齐生产实现。
- 修复：rusqlite 启用 `unlock_notify` feature，根治 shared-cache 内存库跨连接 SQLITE_LOCKED 导致的两个测试 flake。
- ✅ **验证**：`cargo test --lib` 875 passed（连跑 3 次全绿）；`npx vitest run` 295 passed；`npm run type-check` / `npm run build` 通过；architecture_guard PASSED。

### v0.29.0 — Agency P4：验证循环（2026-07-19）

- 四级 grader：code（字数/自重复/合同禁则）→ rule（合同兑现/追读力/规则复检）→ model（rubric 化编辑裁决 1-5 须引证据，旧格式回退兼容）→ human（用户修改率后置信号，字符二元组 Jaccard，不进 gate）。
- Gate v2 统一加权评分（code 0.2 / rule 0.3 / model 0.5，阈值 0.75）取代二元判定。
- V110 里程碑检查点：`agency_checkpoints` 指标快照 + `agency_compare_checkpoints` 现在 vs 当时对比。
- eval harness：JSON 场景 + pass@k/pass^k + baseline 回归门（确定性模式随 `cargo test` 纳入 CI）。
- 评估仪表盘：`agency_eval_overview` 五段聚合 IPC + 侧栏诊断组「创作评估」页（通过率/加权分 SVG 趋势/判定历史/角色 token 用量，零图表依赖）。
- migration runner 按最高版本选目（修复陈旧副本遮蔽）；resume 改 spawn 模式。
- ✅ **验证**：`cargo test --lib` 855 passed；`npx vitest run` 293 passed；`npm run type-check` / `npm run build` 通过；architecture_guard PASSED；landing `npx vitest run` 19 passed。

### v0.28.0 — Agency P3：代币优化 + 记忆持久性（2026-07-17）

- 角色×任务模型路由：主创 Creative / 管理 Tool / 编辑 Background（经 ModelRole 体系，用户可按角色指派模型）。
- 全局 agency LLM 并发闸门（跨 run 上限 3）+ request_id RAII 注册。
- 上下文注入 token 预算（tiktoken 计数截断）+ 黑板三档目录（catalog/summary/full）+ ToolLoop 会话窗口。
- `agency_sessions` 会话快照（机械提取 + Background 档五段摘要双层）；跨会话恢复 `agency_resume_run`（黑板复制 + stale-replay 防护 + `.storymoss` sessions/ 归档）。
- 同 story 并发 run 原子护栏（V109 部分唯一索引）；创作角色落库去重；质量门判定轮次可追溯；清理 T8 遗留创世专属死代码。
- ✅ **验证**：`cargo test --lib` 830 passed；`npx vitest run` 292 passed；architecture_guard PASSED；landing `npx vitest run` 19 passed；src-frontend / landing 构建成功。

### v0.27.0 — Agency 多代理创作框架（创世 2.0）P1+P2（2026-07-17）

- 新增 `src-tauri/src/agency/` 模块：黑板协作 + ReAct 工具循环 + 三角色（主创/管理/编辑审计）。
- 质量门：编辑裁决 + 规则复检 + 至多 1 轮修订，未过门不装配。
- 并行稳态循环：编辑审第 N 章与主创写第 N+1 章并发；按角色并发预算与 run 级 token 预算。
- request_id 定点取消（不再全局取消）；续写循环 `agency_continue_chapter` / `agency_continue_batch`。
- 创作资产自动落库（characters / world_buildings / story_outlines）。
- `smart_execute` 创世路径切换到 agency；旧 GenesisPipeline 移除（TriShot 续写保留）。
- ✅ **验证**：`cargo test --lib` 834 passed；`npx vitest run` 292 passed；architecture_guard PASSED。

### v0.26.59 — StoryForge → StoryMoss 品牌收尾，官网落地页上线（2026-07-11）

- 完成仓库 tracked 文件 StoryForge → StoryMoss 全局替换；GitHub Release 标题更新为 StoryMoss。
- `landing/` 官网站点部署至 `https://ai.91z.net`，重写 Hero / ValueProp 产品介绍，加入 Logo，新增平台感知下载按钮。
- ✅ **验证**：landing `npx vitest run` 19 passed。

### v0.26.58 — 修复 OpenAI/Deepseek 因 top_p=0 健康检测失败（2026-07-09）

- 根因：OpenAI 兼容 API（Deepseek）要求 `top_p ∈ (0, 1.0]`，配置中 `top_p: 0.0` 导致健康探测/生成直接报错。
- 修复：`OpenAiAdapter` 序列化前过滤 `top_p`，非法值自动省略。
- ✅ **验证**：`cargo test --lib` 770 passed；新增 `llm::openai` 单元测试。

### v0.26.57 — 自动划分章节、本地导出保存与提示词目录（2026-07-09）

- 后台设置新增「划分章节方式」：`word_count` 按字数（上限留空默认 3000 字）、`plot` 按情节；场景保存空闲 30s 后仅对最新章自动切分。
- 导出结果走系统原生保存对话框，文本格式直接写 UTF-8，pdf/epub 复制后端临时文件；取消时不关闭导出弹窗。
- 提示词注册表新增「打开目录」按钮，直接打开 bundled prompts 资源目录；编辑器改为原生 textarea，避免 Monaco CDN 被 CSP 拦截。
- ✅ **验证**：`cargo test --lib` 769 passed；`npx vitest run` 292 passed；tsc / fmt / format:check 全绿。

### v0.26.56 — 网关契约测试串行化（2026-07-09）

- mock app_data_dir 写 config 契约加锁，消除并行污染。

### v0.26.55 — 幕后模型列表开启/关闭（2026-07-09）

- 模型卡片「开启/关闭」开关；仅轮询已启用模型；复用 v0.26.54 fail-closed。
- `is_promotable_user_model` 要求仍在网关注册表。

### v0.26.54 — 修复创作模型被粘性降级绕过（2026-07-09）

- 显式创作角色不受连续失败 demotion 拦截；粘性 Unhealthy 在 resolve 清一次再探。
- `set_active_model` / `save_settings` 调用 `clear_model_demotion`；`generate()` 再提升用 `is_promotable`。
- ✅ **验证**：gateway/health/commands 契约 6 passed；architecture_guard。

### v0.26.53 — 故事名取消单击回幕后（2026-07-09）

- 故事名仅双击改名；设置按钮为回幕后入口（禅模式保留）。
- ✅ **验证**：Header 单击不调 backstage；设置按钮可回幕后。

### v0.26.52 — 修复模型新增与默认创作模型即时生效（2026-07-09）

- 幕前 `gateway-status` 随 `model_config` 失效；状态栏含 Unknown。
- 创作角色允许 Unknown 置顶；同步 `active_llm_profile`。
- ✅ **验证**：Rust 4；useSyncStore 5；tsc/fmt/architecture_guard。

### v0.26.51 — 幕前故事名与章节名内联改名（2026-07-09）

- 故事：草苔/未命名展示 + 粘贴自动建故事；双击改名。
- 章节：`.chapter-header` + 顶栏状态统一双击改名；`update_scene` 持久化 title。
- ✅ **验证**：相关 vitest 30；tsc / format / architecture_guard。

### v0.26.50 — 修复打字触发后台运行与深度思考假超时（2026-07-09）

- AutoIngest 30s 防抖 + 后台信号量；contract-auto 静默；活动不同步拉高 isGenerating；超时看门狗弹诊断。
- ✅ **验证**：scene_service 6；contract gate 2。

### v0.26.49 — 修复续写与正文脱节（末句硬锚点）（2026-07-09）

- Call3/TimeSliced prompt 最末尾注入末 2 句硬锚点，覆盖开场大纲。
- ✅ **验证**：ending_anchor 3 passed。

### v0.26.48 — 修复自动更新：GitHub Releases + latest.json（2026-07-09）

- `createUpdaterArtifacts` + AppImage；CI 上传签名包；tag 后校验 `latest.json`。
- ✅ **验证**：updater 2 passed。

### v0.26.47 — CI 热修复：Rust fmt（2026-07-09）

- 修复 v0.26.46 rust-check 失败；无逻辑变更。

### v0.26.46 — 创世方法论全链路、题材 match-or-create 与拆书持久化（2026-07-09）

- Background 模板恢复方法论占位符；Genesis 分步注入 + step 推进；HDWB ID 统一。
- EnsureGenreProfile match-or-create；拆书 StoryArc/作者/伏笔 + 分块止血。
- ✅ **验证**：genesis/methodology/prompt 契约 20+ passed。

### v0.26.45 — Genesis 人物卡强制落地（姓名 + 欲望/阻力）（2026-07-09）

- ProtagonistCard 双重注入 + 三信号探针 + 软重试；零新 LLM。
- ✅ **验证**：narrative 61；protagonist_card 6。

### v0.26.44 — Genesis 首章质量：开篇骨架与提示词加厚（2026-07-09）

- quick_phase 四步；OpeningSkeletonStep（≤10s fail-open）；概念加厚；strategy 中文；四元组；占位角色去硬编码；纪律单源。
- ✅ **验证**：`narrative::genesis` 12 passed；extract_story_meta 2 passed。

### v0.26.43 — 修复底部状态栏 emoji 显示为方框（2026-07-09）

- 阶段文案去 emoji；StatusIcon 渲染；状态解析修复。
- ✅ **验证**：StatusIcon/BottomBar 相关 18+；vitest 全绿。

### v0.26.42 — 修复续写 Tab 提示可见但无幽灵文本（2026-07-09）

- 新续写清零 hideGhostUntil / postAcceptHideUntil；接受中不误解除。
- ✅ **验证**：RichTextEditor.duplicate 6 passed（+1）。

### v0.26.41 — 记忆统一读模型与 Finalize scene_id 根治（2026-07-09）

- Finalize 按 scene_id 直写；story_memory_facts VIEW + kg_entity_id；表不 DROP。
- ✅ **验证**：cargo 701；finalize 3；facade 7；vitest 261。

### v0.26.40 — 幕后资产闭环 P0–P3（2026-07-09）

- 侧栏影响徽章；SceneEditor 管线轨；KG→Bundle；MCP→设置扩展；MemoryFacade；prompt 覆盖率。
- ✅ **验证**：memory::facade 5；相关 vitest 15+。

### v0.26.39 — 幕后信息架构全面重排（2026-07-09）

- 侧栏五组分类 + 中文命名；数据洞察三 Tab；设置七 Tab；拆书设置就近；账号死链修复。
- ✅ **验证**：vitest 249；tsc/format 通过。

### v0.26.38 — 提示词面板修复与组合智能化（2026-07-09）

- 面板：textarea 替代 Monaco CDN；原生打开目录；导出覆盖/完整包。
- 运行时：Call 1 `methodology`/`contextual_injectors` 回灌 Call 3；场景组合预览。
- ✅ **验证**：cargo test 690；vitest 244；tsc/fmt/architecture_guard 通过。

### v0.26.37 — 修复幕前「保存中」常亮与字数不更新（2026-07-09）

- 幕前 `update_scene` 参数改为 `{ scene_id, updates }`；AI 追加后刷新字数并自动保存。
- ✅ **验证**：vitest 242；tsc/format 通过。

### v0.26.36 — 后台配置变更即时生效（超时/字体/主题热同步）（2026-07-09）

- `save_settings` 热重载 LLM + 广播 `app_settings`；幕前/幕后 Query 即时失效。
- `llm_first_chunk_timeout_secs` 接入适配器；TriShot 预算与 writer prompt 读真实配置。
- 字体/色调主题经 Tauri 事件跨窗口即时同步。
- ✅ **验证**：cargo test 685；vitest 240；fmt/tsc/architecture_guard 通过。

### v0.26.35 — 全面落地幕后工作室审计残留 R1–R11（2026-07-09）

对照 `docs/AUDIT_BACKSTAGE_STUDIO_v0.26.34.md` 残留项一次性关闭：

- **R1**：`list_stories` → `StoryListItem.scene_count`；Dashboard「场景」用真实场景数。
- **R2**：CreationPathGuide 快速创作 → `runCreationWorkflow`；导航统一 `appStore.currentView`。
- **R3**：后端 `apply_wizard_to_story`（去重 + KG）；前端单 IPC。
- **R4**：幕后监听 `genesis-warnings` + GenesisPanel 刷新。
- **R5/R6**：Pipeline/SceneEditor 场景序号语义标注。
- **R7–R11**：文风 Tab、UsageStats 启发式、伏笔 Kanban、角色→场景跳转、拆书转故事导航。
- ✅ **验证**：见 CHANGELOG / AGENTS 本版本门禁结果。

### v0.26.34 — 修复提示词导入参数并新增「打开本地目录」功能（2026-07-09）

- **修复批量导入静默失败**：`PromptsPanel.handleImportAll` 参数键由 `promptId` 修正为 `prompt_id`，与后端命令字段命名对齐。
- **新增「打开目录」功能**：后端新增 `get_prompts_directory` 命令；前端标题栏新增按钮，使用系统文件管理器打开当前 prompts 资源目录。
- **新增「刷新」按钮**：重新加载提示词列表与目录路径。
- **改善错误展示**：加载失败时页面显示具体错误信息。
- **导出/导入按钮归位**：移至页面标题栏，避免与重置操作混淆。
- ✅ **验证**：`cargo test --lib` 685 passed；`npx vitest run` 237 passed / 3 skipped；`cargo +nightly fmt -- --check`、`npx tsc --noEmit`、`architecture_guard.py`、`npm run format:check`、`npm run build` 均通过。

### v0.26.33 — 补齐阶段 2/3/4 具体缺口：KG/角色关系删除、前端解耦（2026-07-08）

- **知识图谱实体归档与关系删除 UI**（Stage 4）：后端新增 `archive_entity` / `delete_relation` 命令；实体详情面板与关系列表新增删除/归档按钮。
- **角色关系删除 UI**（Stage 2）：`useDeleteCharacterRelationship` hook + `Characters.tsx` 关系卡片删除按钮。
- **前端 `frontstage ↔ components` 解耦**（Stage 3）：新增 `hooks/contracts/useEditorConfig.ts`；`FrontstageApp` / `RichTextEditor` 不再直接 import `EditorSettings.tsx`；循环依赖数为 0。
- ✅ **验证**：`cargo test --lib` 684 passed；`cargo +nightly fmt -- --check` 通过；`cargo clippy --lib` 通过；`npx vitest run` 234 passed / 3 skipped；`npx tsc --noEmit` / `architecture_guard.py` / `npm run format:check` 通过。

### v0.26.32 — 完成阶段一剩余项：L1 创作入口、仪表盘统计卡、memory/ingest 测试（2026-07-08）

- **L1 创作入口 UX 统一**：`CreationPathGuide` 卡片可点击；Dashboard “AI 创建故事”主按钮进入幕前 Genesis 流程。
- **仪表盘统计卡修正**：“章节”改为“场景”，新增“字数”统计卡，数据源对齐 `useStories`。
- **`memory/ingest` 测试补齐**：新增 5 条 happy/error 路径测试，不依赖 LLM。
- **新增文档**：`docs/plans/2026-07-08-storymoss-phase1-execution-plan.md` 记录与综合优化计划的对照及执行方案。
- ✅ **验证**：`cargo test --lib` 682 passed；`cargo +nightly fmt -- --check` 通过；`cargo clippy --lib` 通过；`npx vitest run` 222 passed / 3 skipped；`npx tsc --noEmit` / `architecture_guard.py` / `npm run format:check` 通过。

### v0.26.31 — 修复幕前状态栏体验、策略解析鲁棒性与新数据库 schema（2026-07-08）

- **幕前顶部状态栏字数统计滞后**：章节加载后 `wordCount` 始终为 0，直到首次自动保存成功才更新；切章时 diff 基准也未重置。
  - `selectChapter` 加载正文后即时计算并设置当前章节字数。
  - `handleContentChange` 中字数变化时同步更新 `wordCount`。
  - 新增回归测试验证章节加载后立即显示非零字数。
- **顶部状态栏字体大小不可点击**：字号显示无点击响应。
  - `FrontstageHeader` 新增 `onOpenFontSettings` 回调，字号显示可点击。
  - 扩展 `show_backstage` 命令支持 `view` / `panel` 参数，点击后打开幕后通用设置并滚动到编辑器设置卡片。
- **底部状态栏后台任务图标 tofu**：emoji 图标在部分系统字体下显示为缺字符号。
  - 8 个活动类别图标全部替换为 `lucide-react` SVG 图标。
  - 新增回归测试验证图标渲染为 SVG。
- **策略选择 JSON 解析失败**：LLM 输出仍可能使用 `reasoning` 或缺失 `rationale`。
  - `SelectedStrategy.rationale` 增加 `#[serde(default, alias = "reasoning")]`。
  - 新增回归测试覆盖 `reasoning` 别名与缺失默认值。
- **新数据库 schema 列缺失**：v0.26.30 兜底修复未覆盖新库建表。
  - `create_tables` 中 `characters` / `scenes` / `world_buildings` / `kg_entities` 新增 `source` / `is_auto_generated` 列。
- ✅ **验证**：`cargo test --lib` 677 passed；`cargo +nightly fmt -- --check` 通过；`cargo clippy --lib` 通过；`npx vitest run` 213 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.30 — 热修复旧数据库缺失 source/is_auto_generated 列（2026-07-08）

- **问题**：部分旧数据库在 v0.26.28 迁移框架切换后，`characters` / `scenes` / `world_buildings` / `kg_entities` 表缺失 `source` / `is_auto_generated` 列，Genesis 与资产查询报 `no such column: source`。
- **修复**：
  - 新增 Rust migration `V103__ensure_source_columns`，幂等补回缺失列。
  - `init_db` 新增 `ensure_source_columns` 启动兜底修复。
  - 新增回归测试覆盖 `schema_migrations=102` 但列缺失场景。
- ✅ **验证**：`cargo test --lib` 674 passed；`cargo +nightly fmt -- --check` 通过；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.29 — 热修复策略选择 JSON schema 不匹配（2026-07-08）

- **问题**：v0.26.28 将 prompts 外部化后，`strategy_selector.md` 模板字段与 `selector.rs` 的 `SelectedStrategy` schema 不一致，Genesis「选择创作策略」步骤报 `VALIDATION_FAILED: missing field rationale`。
- **修复**：
  - 重写 `resources/prompts/strategy/strategy_selector.md`，对齐 `rationale`/`genre_profile_id`/`methodology_id`/`style_dna_ids`/`skill_ids`/`workflow_id`/`parameters` 字段。
  - `selector.rs` 新增 `LegacyStrategyResponse` 兜底解析，兼容旧格式（`selected_strategy`/`reasoning`/`asset_combination`）。
  - 新增 `test_parse_strategy_response_legacy_schema` 单元测试。
- ✅ **验证**：`cargo test --lib` 673 passed；`cargo +nightly fmt -- --check` 通过；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.28 — Phase 4 架构债务与工程体验（2026-07-07）

- **知识图谱手动 CRUD UI**：Graph 页图例面板新增「新建实体」按钮；实体详情面板新增「添加关系」按钮，支持从当前故事已有实体中按名称搜索并建立关系。
- **世界构建 AI 生成**：`WorldBuilding` 页新增「AI 生成」按钮与 `AiWorldBuildingModal`，基于当前故事调用 `generateWorldBuildingOptions` 一键生成世界观并回写。
- **角色 AI 扩展**：`Characters` 页新增「AI 扩展」按钮与 modal，基于当前世界观调用 `generateCharacterProfiles`，选择角色组后批量 `createCharacter`。
- **叙事分析图表**：`NarrativeAnalysis` 页新增 SVG `ReadingPowerChart` 折线/面积图，替代原有条形图展示追读力趋势。
- **策略选择移入 Quick Phase**：`genesis.rs` 中 `StrategySelectionStep` 从 `background_steps()` 前移至 `quick_phase_steps()`，位于 `ConceptGenerationStep` 之后、`FirstChapterGenerationStep` 之前；同步更新所有步骤的 `step_number`/`total_steps`/`progress_percent` 与前后端测试契约。
- **外部化 prompts**：`prompts/registry.rs` 中 95 个内置提示词迁移至 `resources/prompts/{category}/{id}.md`，运行时从 Tauri 资源目录加载；保留用户覆盖能力。
- **迁移脚本拆分**：`db/connection.rs` 中 2,650 行 inline `run_migrations` 拆分为 `src/db/migrations/V028__*.rs` … `V099__*.rs` 共 70 个编号 Rust 迁移文件；`MigrationRunner` 扩展 `RustMigration` trait，统一排序、过滤、执行 SQL 与 Rust 迁移。

#### 下一 milestone 已识别项

- 无

- ✅ **验证**：`cargo test --lib` 672 passed；`cargo +nightly fmt -- --check` 通过；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.27 — L4 诊断互链、文档与依赖解耦（2026-07-07）

- **诊断页互链**：GenesisPanel ↔ TracingPanel 双向跳转；Genesis 失败运行 → Logs 深链并预填 `session_id`。
- **用量统计 operation 分组**：全部 / bootstrap / smart_execute / 其他 标签（启发式分组）。
- **伏笔看板 UX**：`setup_scene_id` 场景下拉；可编辑 `target_start_scene` / `target_end_scene`。
- **循环依赖解耦**：前端 `components ↔ stores ↔ hooks ↔ frontstage` 解耦；Tauri `creative_engine ↔ llm`、`model_gateway ↔ router` 解耦。
- **文档补齐**：`USER_GUIDE.md` 补 L4 诊断页、修正过度承诺；元文档同步。
- ✅ **验证**：`cargo test --lib` 672 passed；`cargo +nightly fmt -- --check` 通过；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.26 — L2 资产补齐与领域层止血（2026-07-07）

- **角色编辑与关系 CRUD**：新增 `CharacterEditModal` / `CharacterRelationshipForm`；角色资料与关系 Tab 均可编辑/添加。
- **L2 创世溯源徽章**：世界观、角色、场景、KG 实体显示「创世」徽章；后端写入时标记 `source` / `is_auto_generated`。
- **Story System 合同播种状态卡**：Contracts Tab 显示 `MASTER_SETTING` + `CHAPTER_1` 合同状态；失败 run 显示错误摘要。
- **Scenes 续写跳转幕前**：`ExecutionPanel` 主行动打开幕前窗口。
- **StorySystem.tsx 拆分**：8 个独立标签组件，主文件 125 行。
- **Repository 层 trait 化与拆分**：`db/repositories.rs` 拆分为模块文件；`creative_engine/context_builder.rs` 依赖 trait。
- ✅ **验证**：`cargo test --lib` 672 passed；`npx vitest run` 210 passed；`architecture_guard.py` 通过。

### v0.26.25 — Backstage Genesis 可观测性与测试基线（2026-07-07）

- **GenesisPanel 动态步骤**：对齐后端 Quick(2) + Background(6)，展示非致命 `errors[]`，支持 story/幕前跳转。
- **L1 创作路径引导**：Dashboard / Stories 新增 `CreationPathGuide`，消除三路径误判。
- **Wizard 重复建故事修复**：已有故事走 update 资产路径，ID 不变。
- **仪表盘统计卡可点击**：跳转 stories / characters / scenes。
- **测试基线**：`genesisSteps.ts` 18 单测；`model_gateway/executor`、`db/repositories`、`memory/ingest` 首批特征测试。
- ✅ **验证**：`cargo test --lib` 677 passed；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.24 — 修复续写重复、截断与跨内容复述（5 项根因）（2026-07-07）

对照 `creative_workflow.log` 2026-07-07 08:44–09:05 续写会话：

- **散布式句子块重复**：`trimInterspersedRepeatedBlocks`（Rust + TS golden 双跑）。
- **跨内容重叠复述**：`stripExistingOverlap`（尾部 3000 字比对，≥25 归一化字剥离）。
- **截断末句污染**：`trimDanglingTail`（极短末句裁剪）。
- **续写 8% 重试闸门**：TriShot anti-repeat 重试（对齐 Genesis）。
- **前端管线**：`sanitizeContinuationOutput` 全路径接入。
- ✅ **验证**：`cargo test --lib` 666 passed；`npx vitest run` 192 passed。

### v0.26.23 — 修复续写卡死与幽灵文本混乱（4 项根因）（2026-07-07）

对照 `creative_workflow.log` 2026-07-07 续写会话时间线，定位并修复 4 个根因：

- **Bug B（卡死主因）**：`auto_contract` 4 个 LLM 调用加入 `is_silent_background` 列表，后台补齐合同不再阻塞 `isAnyBackendActive`（原 6 分钟阻塞）。
- **Bug D（混乱主因）**：`handleSmartGeneration` 入口加重入守卫，存在未接受幽灵时先丢弃并提示。
- **Bug A**：`RichTextEditor` 新增 `bodyForceHideGhost` state 镜像 `force-hide-ghost` 类，消除 10s 渲染延迟。
- **Bug C**：续写 call3 超时上限从 120s 降至 60s，慢模型 fail-fast 回退到快模型。
- ✅ **验证**：`cargo test --lib` 655 passed；`npx vitest run` 183 passed；fmt/tsc 通过。

### v0.26.21 — 修复 Windows MSI 构建（迁移文件名重命名）（2026-07-07）

- 🎯 **背景**：v0.26.17 起将 `src/db/migrations/` 打包为 Tauri resource，但 24 个迁移文件名含中文/全角逗号/破折号且最长 102 字符，导致 WiX `light.exe` 从文件名生成 `File/@Id` 标识符时失败。v0.26.14/v0.26.16（resources 引入前）Windows MSI 曾成功，根因确凿。v0.26.20 尝试的 `wix.language: zh-CN` 无效（问题在标识符生成而非代码页）。
- 🎯 **修复**：将 24 个迁移文件重命名为 ASCII 短名（保留 `V###` 前缀与排序）。`schema_migrations` 按 version 跟踪，已应用迁移不受影响；`parse_filename` 仅解析 `V###` 前缀，无逻辑变更。
- ✅ **验证**：`cargo test --lib migrations` 8 passed；本地 `cargo tauri build`（macOS）通过；CI Windows MSI 待验证。

### v0.26.20 — 修复 v0.26.19 CI 格式检查失败与 Windows 打包（2026-07-06）

- 🎯 **修复**：v0.26.19 的 `ParallelWorldOutlineCharacterStep` doc 注释行超过 `max_width=100`，运行 `cargo +nightly fmt` 自动换行。仅注释格式变更。
- 🎯 **macOS 公证**：随 Apple Developer 协议续签已恢复成功。

### v0.26.19 — Genesis 创世流程全面审计与测试加固（2026-07-06）

- 🎯 **背景**：对照项目文档对「智能创作流程-创世」进行全面审计，分 Phase 1–4 执行修复、加固与测试补齐。
- 🎯 **Phase 1（P0 竞态与契约）**：
  - **Gap B**：`isFirstChapterReady` 路径在 `finalContent` 为空时不锁 `delivered`，避免编辑器永久空白。
  - **P0-2 角色世界观上下文**：`ParallelWorldOutlineCharacterStep` 中 character 提示词读取 `bundle.world_building` 恒为空（闭包捕获竞态），改为先 await world 拿真实 `world_concept` 再构造 character；提取 `world_concept_for_character_prompt` 纯函数 + 单测。
  - **P0-3 ChapterSwitch delivered 时序**：`selectChapter` 懒加载失败时不标记 `delivered`（`markDeliveredOnLoad` 仅在 `setContent` 成功后标记）。
- 🎯 **Phase 2（P1 架构对齐）**：
  - **后台错误可观测性**：`GenesisContext.errors` 共享错误集合 → `genesis_runs.steps_json` + `genesis-warnings` 事件 → 前端 toast 区分 warning/error。
  - **mutex 中毒锁加固**：`pipeline.rs` cancel flags 与 `model_gateway/executor.rs` registry 锁改用 `unwrap_or_else(|e| e.into_inner())` 恢复中毒锁 + 单测。
  - **策略移入 quick phase**：经评估暂缓，记录为债务。
  - **文档/类型对齐**：`window/mod.rs` 与 `FrontstageEvent.ts` 注释重写，明确创世第一章 `ChapterSwitch` 不携正文。
- 🎯 **Phase 3（测试加固）**：
  - 8% 重试闸门 + ChapterSwitch payload 提取纯函数 + 边界/契约测试。
  - 前端 Gap C 专用测试 + 状态机端点契约测试。
  - **跨层共享 trim golden fixture**：`tests/fixtures/trim_golden.json`，Rust + TS 双跑锁定跨层一致性。
- 🎯 **Phase 4（代码整洁）**：重命名 `*_future` → `*_gen`；去重 `AppConfig::load`；`appendAiContent` skip 路径不 `markAccepted`；Gap C 重复入站也跳过 setContent；评估不合并 `isGenesisSettingUpRef`。
- ✅ **验证**：`cargo test --lib` 655 passed（+10）；`npx vitest run` 183 passed（+17）；`npx tsc --noEmit` 零错误；fmt 通过。

### v0.26.18 — Genesis 第一章重复：竞态路径加固（2026-07-06）

- 🎯 **背景**：用户报告 v0.26.16 后新写小说第一章仍有内容重复。代码审查发现三个残留竞态缺口。
- 🎯 **修复**：
  - **Gap A**：ChapterSwitch `auto_accept=true` 但 content 为空时 `skipContent=true`（不从 DB 加载），不标记 `delivered`（让 smart_execute 投递）。
  - **Gap B**：`isFirstChapterReady` 路径仅在已 append 或编辑器已有内容时标记 `delivered`，避免空内容误锁。
  - **Gap C**：`selectChapter` 咽喉点新增 `delivered` + 编辑器已有内容守卫。
- ✅ **验证**：`npx vitest run` 167 passed（+1 Gap A 回归测试）；`npx tsc --noEmit` 零错误。

### v0.26.17 — Issue #4 启动加固：打包 SQL 迁移与 init_db 诊断增强（2026-07-06）

- 🎯 **背景**：v0.26.16 已修复 `init_db` 失败时的二级 panic（GatewayExecutor `state::<DbPool>()`），但 Windows 用户仍可能因 `init_db` 本身失败或 Release 缺 SQL 迁移而进入降级模式。
- 🎯 **修复**：
  - 打包 `src/db/migrations/` 到 `$RESOURCE/db/migrations/`。
  - `setup` 解析 bundled migrations 并传入 `init_db`。
  - `init_db` 启动前 `create_dir_all`；失败日志含 DB 路径与 migrations 目录。
  - 新增 `init_db_succeeds_on_fresh_directory` 回归测试。
- ✅ **验证**：`cargo test --lib init_db` 2 passed；`cargo check` 通过。

### v0.26.16 — 根治 Genesis 第一章重复、Issue #4 启动稳定性与代码格式修复（2026-07-06）

- 🎯 **症状**：v0.26.14 后 Genesis 第一章重复问题在部分模型/路径上仍偶发；部分 Windows 用户在应用数据目录不可写时遇到启动闪退/ panic。
- 🎯 **根因 1（重复）**：LLM 可能生成自身首尾重复的正文；前端 Genesis 自动接受流程使用布尔守卫，多处赋值导致状态机混乱，多路径并发下内容被叠加。
- 🎯 **根因 2（启动 panic）**：`init_db` 失败后 `setup` 仍构造 `GatewayExecutor`，其通过 `state::<DbPool>()` 读取未 manage 的 pool 导致启动 panic。
- 🎯 **修复**：
  - 生成侧验证闸门：自重复比例 ≥8% 时 anti-repeat 重试；prompt 新增「结构纪律」段。
  - 前端单写者状态机：`idle → generating → delivered`，阻塞外部投递与幽灵恢复。
  - `GatewayExecutor::new` 显式传 pool，`setup` 仅在 pool 可用时初始化网关。
  - 全局代码格式化，修复 CI `fmt`/`prettier` 检查失败。
- ✅ **验证**：`cargo test --lib` 637 passed / 0 failed / 2 ignored；`npx vitest run` 166 passed / 3 skipped；`npx tsc --noEmit` 零错误；`cargo +nightly fmt -- --check` 通过；`npm run format:check` 零差异；`python3 scripts/architecture_guard.py` 通过。

### v0.26.14 — 修复 Genesis 第一章模型输出自重复与降低幕前诊断日志压力（2026-07-05）

- 🎯 **症状**：v0.26.13 日志显示 `append_ai_done` 只触发一次、`append_text_check.occurrences=1`，但用户仍看到第一章「开头段落与结尾段落相同」的内容重复。
- 🎯 **根因**：前端没有追加两次；LLM 生成的正文自身存在首尾段落重复（模型级循环/自重复）。
- 🎯 **修复**：新增 `trimSelfRepetition` 工具，段落级检测「后半段 == 前半段」或「末段 == 首段」，字符级使用 KMP 最长 border 检测长尾重复；在 `appendAiContent` 入口及 `smart_execute.finalContent` 写入编辑器/幽灵文本前统一清理。同时降低 `RichTextEditor` 渲染诊断日志频率（前 20 帧 + 幽灵状态变化 + 200ms IPC 节流），缓解长时间写作后页面卡顿/崩溃。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 151 passed / 3 skipped；`npx playwright test` 36 passed / 5 skipped；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过。

### v0.26.13 — 修复 Genesis 第一章渲染层视觉重复（幽灵容器残留）（2026-07-05）

- 🎯 **症状**：v0.26.12 后日志显示 `append_ai_done` 只触发一次、`hasDuplicate: false`，但用户仍看到第一章「前一段分行、后一段挤成一大段」的虚假重复。
- 🎯 **根因**：数据层只写一次；`RichTextEditor` 的 `shouldShowGhostTree` 条件为 `!!(generatedText || isGenerating)`，当 `generatedText` 为空但 `isGenerating=true` 时会渲染空幽灵容器。该容器若残留旧内容或 React 复用 DOM 节点异常，就会导致「正文 + 幽灵文本」同框。
- 🎯 **修复**：`shouldShowGhostTree` 改为 `!!generatedText`；`FrontstageApp` Genesis 自动接受路径先 `setIsGenerating(false)` 再清空 `generatedText` / 追加正文；增强 `frontstage:rich_editor_diag` 诊断字段；E2E 回归测试新增 `ghost-paragraph` 隐藏断言。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 148 passed / 3 skipped；`npx playwright test --project=chromium` 35 passed / 5 skipped；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过。

### v0.26.12 — 修复角色列表为空/未加载时的幕前崩溃与订阅状态空值（2026-07-05）

- 🎯 **症状**：打开已有故事或新写小说后，幕前界面偶尔白屏崩溃，ErrorBoundary 显示 `Cannot read properties of null (reading 'length')`；订阅状态接口返回异常时日志出现 TypeError。
- 🎯 **根因**：`RichTextEditor`「角色名点击」effect 在初始化时直接访问 `characters.length`，而 `useCharacters` 的 `data` 可能为 `null`（React Query 默认值仅对 `undefined` 生效）；`useSubscription` 未对 `getSubscriptionStatus()` 返回 `null` 做空值防护。
- 🎯 **修复**：`RichTextEditor` 角色点击 effect 增加 `!characters || characters.length === 0` 守卫；`useSubscription` 使用 optional chaining 读取 `status?.tier`、`status?.status` 并回退默认值；新增 Playwright E2E 回归测试 `e2e/genesis-duplicate.spec.ts` 覆盖「已有故事 + 新写末世小说」完整流程。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 148 passed / 3 skipped；`npx playwright test --project=chromium` 35 passed / 5 skipped；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过。

### v0.26.11 — 修复 Genesis 第一章 store-editor 失步与崩溃隐患（2026-07-05）

- 🎯 **症状**：v0.26.10 后日志显示单次 `append_ai_done`，但用户仍看到第一章内容重复；写完后过一会儿页面可能崩溃。
- 🎯 **根因**：追加后 store 依赖 200ms onChange debounce 回写，当 `latestContentRef` 与编辑器 HTML 指纹相同时 `handleContentChange` 提前返回，store 长期为空，导致后续外部同步/章节切换出现视觉重复或状态漂移；`RichTextEditor.appendText` 空文档 setContent 未更新 `lastExternalContentRef`，content prop 到达后外部同步 effect 可能再次 setContent；开发模式下可能加载陈旧 `dist`。
- 🎯 **修复**：`appendAiContent` 追加后立即用 `editorRef.getHTML()` 同步 store 与 `latestContentRef`；`RichTextEditor.appendText` 空文档分支标记外部同步并更新 `lastExternalContentRef`；`RichTextEditorRef` 新增 `getHTML()`；确认 `tauri.conf.json` `devUrl` 指向 dev server。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 147 passed / 3 skipped；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过。

### v0.26.9 — 根治 Genesis 第一章重复（DOM 竞态与追加去重）（2026-07-04）

- 🎯 **症状**：v0.26.8 后用户反馈第一章内容仍会重复显示，日志显示重复检测在 `appendAiContent` 与 `setGeneratedText` 路径上失效。
- 🎯 **根因**：所有重复检测与前缀去重都依赖 `editorRef.current.getText()`，而 TipTap DOM/Text 状态滞后于 React `content` prop。在 ChapterSwitch / pipeline-complete 刚加载正文后、`onChange` debounce 触发前，`getText()` 返回空/旧文本，导致已有正文被当成新内容追加或恢复为幽灵文本。
- 🎯 **修复**：`isTextAlreadyInEditor`、`handleRequestGeneration`、`handleSmartGeneration`、`appendAiContent` 统一改用 `latestContentRef.current` 作为内容基准；`appendAiContent` 追加后立即同步 `latestContentRef`；`RichTextEditor` 幽灵文本直接包含检测剥离 HTML 标签。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 146 passed / 3 skipped；`npx tsc --noEmit` 零错误。

### v0.26.8 — 彻底修复 Genesis 第一章重复（竞态路径覆盖）（2026-07-04）

- 🎯 **症状**：v0.26.7 后，在 pipeline-complete 先加载 DB 正文、smart_execute 后返回 final_content 的竞态下，新小说第一章仍会重复显示。
- 🎯 **根因**：`genesisAutoAcceptedRef` 仅在 ChapterSwitch 自动加载正文时设置，无法覆盖 pipeline-complete 先完成的路径；后续 smart_execute 把 `final_content` 恢复为幽灵文本，与编辑器正文叠加。
- 🎯 **修复**：新增 `isTextDuplicate` 归一化去重工具；提取 `isTextAlreadyInEditor` helper；`pipeline-complete` 加载正文后标记 Genesis 已自动接受；`handleRequestGeneration` / `handleSmartGeneration` 设置 `generatedText` 前检测编辑器是否已包含该内容，已包含则跳过。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 138 passed / 3 skipped；`npx tsc --noEmit` 零错误。

### v0.26.7 — 修复 React #185 无限循环与 Genesis 第一章重复（2026-07-04）

- 🎯 **症状**：新写小说后过一会儿页面崩溃（React #185 Maximum update depth exceeded）；新小说第一章内容重复显示。
- 🎯 **根因**：`pipeline-complete` effect 依赖未 memo 的 `selectChapter`，每次渲染重复触发；Genesis 异步装配期间 `loadStories` 自动选择新 story 并把 DB 正文加载进编辑器，与 `generatedText` 幽灵文本叠加。
- 🎯 **修复**：关键回调全部 `useCallback`/ref 稳定化；`pipeline-complete` effect 增加单次处理守卫并改用 ref 读状态；新增 `isGenesisSettingUpRef` 禁止装配期间自动选择 story。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 138 passed / 3 skipped；`npx tsc --noEmit` 零错误。

### v0.23.49 — 推理模型思考链导致 JSON 提取出空对象修复（2026-06-26）

- 🎯 **症状**：用推理模型（如 MN-Oblivion-26B-UNCENSORED）创世时报 `missing field 'title' at line 1 column 2`，LLM 实际成功返回 5191 字符，失败在 JSON 提取阶段。
- 🎯 **根因**：推理模型在正文前输出 `önh...` / `<thinking>...</thinking>` 思考链，思考链里含花括号（如 "用 {} 格式表示"），`extract_first_json_object` 把第一个 `{}` 当成 JSON 对象提取出空对象，serde 找不到必填 `title`。
- 🎯 **修复**：新增 `strip_reasoning_blocks` 剥离配对思考链块；`extract_first_json_object` 跳过空对象 `{}` 继续向后扫描。
- ✅ **验证**：`cargo test --lib` 571 passed / 0 failed / 2 ignored

### v0.23.48 — JSON 提取用括号匹配修复 trailing characters 解析失败（2026-06-25）

- 🎯 **根因**：LLM 返回故事概念 JSON 后附带额外说明文本（含 `}`），`extract_and_sanitize_json` 用 `rfind('}')` 找 JSON 结尾会误提取过多内容 → serde_json "trailing characters" 错误。
- 🎯 **修复**：新增 `extract_first_json_object` 用括号匹配（跟踪 `{`/`}` 深度 + 跳过字符串字面量）精确提取第一个完整 JSON 对象。
- ✅ **验证**：`cargo test --lib` 568 passed / 0 failed / 2 ignored

### v0.23.47 — 调用模型前实时连接探测（5s），跳过失效死模型（2026-06-25）

- 🎯 **根因**：模型列表里可能存在已失效但健康状态仍为 Healthy 的死模型（本地 llama.cpp/MLX 服务已停止但缓存未更新），直接调用浪费 30-300s 直到 LLM 超时。
- 🎯 **修复**：`GatewayExecutor::generate` 候选循环中，每个候选模型在实际 LLM 调用前先执行 5s 超时实时探测；探测失败/超时则标记 `HealthStatus::Unhealthy`，跳到下一候选。

### v0.23.46 — AI 状态提示使用模型名称（2026-06-25）

- 🎯 `generation-status` 和 `llm-generating-progress` 心跳事件状态文案追加模型名称（格式：`准备上下文... · gemma4-e2b (OpenAI) (15s)`）。

### v0.23.45 — IngestPipeline LLM 调用静默化，根治正文后活动卡死与页面崩溃（2026-06-25）

- 🎯 **根因（日志确认）**：创世正文返回后，IngestPipeline 并发发起多个"记忆-内容分析"LLM 调用，`context_label` 未匹配 `is_silent_background` 静默列表，进度事件覆盖前端主活动状态（"准备上下文"卡住）。本地模型无法处理并发请求返回 `INTERNAL_ERROR`，大量错误事件涌入导致前端页面崩溃空白。
- 🎯 **修复**：将 IngestPipeline 的三个 `context_label` 加入 `is_silent_background` 静默列表。
- ✅ **验证**：`cargo check` 零错误

### v0.23.44 — AI 状态提示使用模型名称（2026-06-25）

- 🎯 `generation-status` 和 `llm-generating-progress` 心跳事件状态文案追加模型名称（格式：`准备上下文... · gemma4-e2b (OpenAI) (15s)`）。

### v0.23.43 — 前端诊断日志 + log_frontend_event 命令（2026-06-25）

- 🎯 新增 `log_frontend_event` Tauri 命令，前端关键路径可写入 `creative_workflow.log`。

### v0.23.42 — 根治创世卡在"最终输出"：BGP-4 自死锁修复（2026-06-25）

- 🎯 **根因（日志确认）**：`execute_trishot` 在 Call 3 成功返回后用 `spawn_blocking().await` 同步等待 BGP-4 `should_trigger` DB 查询，与 BGP-1/BGP-3 后台任务竞争 `std::sync::Mutex` 导致自死锁，`execute_trichot` 永不返回。
- 🎯 **修复**：BGP-4 改为 `tokio::spawn`（fire-and-forget）。
- ✅ **验证**：`cargo test --lib` **563 passed / 0 failed / 2 ignored**

### v0.23.40 — 参照现有诊断机制添加 WorkflowLogger 日志点（2026-06-25）

- 🎯 Bug A/B 诊断日志点接入 WorkflowLogger（`genesis.chapter_switch.sent`、`trishot.call3.done`、`trishot.bgp4` 等）。

### v0.23.37 — Genesis 活动清理（2026-06-25）

- 🎯 Genesis 成功路径补发 `smart-execute-progress` completed/error；`smart-execute-progress` 处理器把 timeout/error 映射为 failed。

### v0.23.36 — 创世正文清洗 + 后台作业不阻塞输入（2026-06-25）

- 🎯 **创世正文质量优化**：TriShot Call 3 的 `final_prompt` 追加 `NOVEL_OUTPUT_DISCIPLINE` 输出纪律段（禁元评论/markdown/小节标题/幕结束批注）+ 新增 `sanitize_novel_output` 后处理兜底（逐行去 markdown 符号→截断尾部元评论→剥离前导过渡语→去整行小节标题/批注）。7 个单元测试覆盖各场景。
- 🎯 **后台作业不阻塞输入**：Genesis 后台阶段 `pipeline-progress` 事件打 `metadata: {background: true}` 标记，前端 `useBackendActivityListener` 检测到后跳过注册 running activity，不禁用输入框。状态文案仍由 `novel-bootstrap-progress` 监听器独立更新。
- ✅ **验证**：`cargo test --lib` **563 passed / 0 failed / 2 ignored**（新增 7 个 sanitize 测试，零回归）；`npx tsc --noEmit` 零错误

### v0.23.35 — 采摘 Step1 JSON 解析容错（2026-06-23）

- 🩹 **Ingest Step1 `missing field entity_type`**：`memory/ingest.rs` 6 个反序列化结构体补 `#[serde(default)]`，LLM 返回 JSON 缺失字段时不再解析失败。

### v0.23.34 — 修复 select_candidates 中 std::sync::Mutex 自死锁（根因彻底查明）（2026-06-23）

- 🎯 **v0.23.31-33 全链路 15 个诊断标记精确定位**：自死锁发生在 `select_candidates` 内部
- 🔧 **自死锁根因**：第125行 `let health = health_registry.lock().ok()` 获取 MutexGuard，变量存活到函数末尾。第188行 `is_model_available` 再次 `lock()` 同一 `std::sync::Mutex`（不可重入）→ 线程永远等待自己释放 → 600s 超时
- 🔧 **修复**：`health` 锁移入嵌套块作用域，块结束时 MutexGuard 自动释放。后续 `is_model_available` 可安全重新锁定
- 🔧 **Call 1 为何不受影响**：Call 1 走 `select_fastest_profile`，不调 `select_candidates`
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.33 — 全链路 15 个精确定位诊断标记（2026-06-23）

- 🔍 从 `trishot.call3.start` 到 `llm.generate.start` 共 15 个 workflow_log 标记

### v0.23.30 — Genesis 全线阻塞点修复：genesis_default + select_candidates spawn_blocking + Chapter 保存 spawn_blocking（2026-06-23）

- 🏛️ **`GenerationMode::genesis_default()`**：显式化 Genesis 模式选择。Genesis 始终走 TriShot（需资产选择 + 快速出章），用户模式设置影响日常续写/改写
- 🔧 **`select_candidates` spawn_blocking**：`GatewayExecutor::generate` 中 `CapabilityStore::load_all()` 用 `spawn_blocking` 预加载，修复 Call 3 卡死
- 🔧 **Chapter 保存 spawn_blocking**：`FirstChapterGenerationStep` 中所有 `ChapterRepository` 操作移入 `spawn_blocking`
- 🔧 **Genesis 跳过 Call 2 精修器**：第 1 章 + 无已有内容时直接进 Call 3
- 🖥️ **前端显示 "[创世]"**：Genesis 期间底部栏显示创世状态而非"三击模式"
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.28 — select_candidates spawn_blocking + v0.23.29 — Chapter 保存 spawn_blocking（2026-06-23）

- 🔧 **Call 3 不再卡在 gateway 路由**：`select_candidates` 中 `CapabilityStore::load_all()` 用 `spawn_blocking` 包裹
- 🔧 **第一章内容写入不再卡在 DB**：ChapterRepository 操作移入 `spawn_blocking`
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.24 — setContent 内容比较 + v0.23.23 — RichTextEditor isExternalSyncRef（2026-06-23）

- 🔧 **从根源杜绝伪"保存中"**：`setContent` 内容未变化时不标记未保存
- 🔧 **从入口杜绝伪"保存中"**：编辑器外部 `setContent` 跳过 `onChange`
- ✅ **验证**：`npx tsc --noEmit` ✅ / `npx vitest run` ✅ 126 passed

### v0.23.22 — 诊断增强 + v0.23.25 — 信号竖条（2026-06-23）

- 🔍 `select_candidates` 慢查询标记（>100ms 输出工作流日志）
- 📊 模型状态指示器改为信号竖条组（3px 宽，4-16px 高，得分低→高排列）
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.19 — 根治 600s 超时：record_llm_call DB 写入不再阻塞 tokio worker（2026-06-22）

- 🎯 **根治概念 LLM 秒回但 pipeline 阻塞 600s**：v0.23.18 行级工作流日志定位卡点——概念生成 LLM 1.1s 完成，但 `record_llm_call` 同步 DB INSERT 卡住 600s 永不返回
- 🔧 **Fix 1 生产连接池加 `connection_timeout(5s)`**：`init_db` 的 `Pool::builder()` 补 `.connection_timeout(Duration::from_secs(5))`，防止 `pool.get()` 无限阻塞
- 🔧 **Fix 2 `record_llm_call` 改为 fire-and-forget `spawn_blocking`**：DB 写入提交到阻塞线程池立即返回，永不阻塞生成主流程
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.18 — 行级诊断：execute_generation Ok 分支 12+ 标记（2026-06-22）

- 🔍 **行级工作流日志**：`execute_generation` Ok 分支每步插入标记（`record_call.start` → `try_state` → `db_write` → `db_done` → `emit_completed.start` → `generate.return_ok`）
- 🧪 **5 个独立模块测试**：心跳 abort 不阻塞、阻塞 emit 由 5s 超时保护、TASK_START_TIMES Mutex 无死锁、pool.get 超时、record_llm_call 非阻塞
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.17 — 心跳阻塞 + 连接池超时双保险（2026-06-22）

- 🔧 `heartbeat_handle.await` 用 `tokio::time::timeout(5s)` 包裹；测试连接池补 `connection_timeout(10s)`
- 🔍 `record_llm_call` 内部添加 `try_state` / `db_write` / `db_done` 诊断标记
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.16 — Genesis 快速阶段卡死修复 + E2E 集成测试（2026-06-22）

- 🔧 `story_repo.create()` 改用 `tokio::task::spawn_blocking` 异步化，防止 DB 锁/连接池满阻塞 tokio worker
- 🧪 新增 `scripts/test_trishot_e2e.py` 端到端集成测试：Gemma4-e2b 真实 LLM **73.2s 完成，1852 中文字**
- ✅ **验证**：`cargo test --lib` **551 passed / 0 failed / 2 ignored**

### v0.23.15 — TriShot 管线 4 处缺陷修复（2026-06-22）

- 🔧 P0 预检失败时调 `AutoContractBuilder::auto_fill` 补齐角色；P1 消息改名 `novel_bootstrap_first_chapter_ready`；P2 Call 1/2 预算守卫用 `total_start`、Call 3 超时 30-120s + 空内容检查
- ✅ **验证**：`cargo test --lib` **551 passed / 0 failed / 2 ignored**

### v0.23.14 — 干净健康的模型池 + 统一身份 + 实时健康报告（2026-06-22）

- 🔧 模型池净化 L1-L4：启动归零清空 `llm_calls`、级联清理死模型、拒绝 disabled 设为活跃、健康报告数据源切换为实时探测快照
- 🔧 Genesis 两阶段：`quick_phase_steps()`（概念+第一章 TriShot）+ `background_steps()`（策略+世界观/大纲/角色）
- ✅ **验证**：`cargo test --lib` **551 passed / 0 failed / 2 ignored**

### v0.23.13 — 强制所有生成路径使用活跃模型（2026-06-22）

- 🎯 **彻底修复“当前模型是 A，实际调用 B”**：`LlmService::select_profile_for_request`、`GatewayExecutor::select_candidates`、`GatewayExecutor::select_fastest_profile` 全部优先返回/置顶用户当前设置的活跃模型
- 🧭 **Genesis 故事概念、TriShot Call 1、普通路由生成统一走活跃模型**：只要活跃模型健康（Healthy/Degraded），不再被 TTFB 阈值或三维打分绕开
- 🩹 **新增模型即时可用**：`create_model` 完成后立即刷新网关注册表并执行健康探测，探测通过即刻进入可用模型池
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.12 — 彻底修复长超时：活跃模型优先 + 智能创作流程日志（2026-06-22）

- 🎯 **修复长超时根因**：模型网关现在强制把用户当前设置的活跃模型放到候选链首位，避免连接到历史/错误模型导致挂起
- 🧭 **`select_fastest_profile` 活跃模型兜底**：即使活跃模型没有算力档案，也优先使用它
- 📝 **新增 `WorkflowLogger`**：记录 TriShot Call 1/Call 3、LLM 调用起止、模型网关候选链与选择原因、错误等详细步骤
- 📋 **诊断卡片增强**：新增 `工作流日志路径` 与 `智能创作流程最近日志`，可直接查看后端执行轨迹
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.11 — 诊断提示词过滤探测/静默调用（2026-06-22）

- 🛡️ **过滤探测/静默调用**：`LlmService::execute_generation` 只在非静默调用时更新诊断提示词
- 🐛 **修复诊断提示词被 probe 覆盖**：避免 `model_gateway_probe` 的 `Respond with exactly the word OK.` 覆盖用户真正关心的生成提示词
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.10 — 模型网关优先使用当前活跃模型（2026-06-22）

- 🎯 **修复 AI 连到旧模型的问题**：`select_fastest_profile` 现在优先使用当前设置的活跃模型（只要健康且 TTFB 不比最快模型差太多）
- 🔗 **`select_candidates` 兜底活跃模型**：候选链中若不存在活跃模型，自动注入，保证用户设置的模型始终有机会被选中
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.9 — 运行时创作资产能力清单 + TriShot 路由增强（2026-06-22）

- 📚 **运行时创作资产能力清单**：应用启动后自动生成并刷新全部系统资产（methodology、genre_profile、style_dna、skill、beat_card、story_engine、pressure_relationship、workflow 等）的紧凑目录
- 🎯 **TriShot Call 1 可见全局资产**：`PromptSynthesizer` 的 prompt 中新增【系统可用创作资产目录】，让最快模型在选资产时知道可调用的系统级资产
- 🔀 **Call 3 资产透传**：TriShot Call 3 通过 `generate_for_task_with_tags` 把 Call 1 选中的资产 ID/标签透传给 `ModelGateway`
- 🧭 **ModelGateway 识别更多资产标签**：`methodology`、`beat_card`、`story_engine`、`pressure_relationship`、`style_dna`、`skill` 等标签会触发 `HeavyCreation`，优先使用创作能力强的模型
- 🐛 **修复 TriShot request_id 错误**：不再把 `gen_response.model` 当作 `request_id`
- 🛡️ **Call 1 预算守卫**：剩余时间不够完成 Call 1 + Call 3 时直接回退本地 `bundle_prompt`，避免前端长时间无响应
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.8 — AI 进度指示精细化 + 提示词诊断可靠性提升（2026-06-22）

- 🎯 **LLM 进度阶段具体化**：每个 LLM 调用都会显示连接模型 ID/提供商、组合提示词规模、等待模型回应、模型回应 token 数、解析结果，不再只显示“构思故事”
- 📊 **`LlmGeneratingProgress` 字段扩展**：新增 `model_id`、`provider`、`prompt_chars`、`prompt_tokens`、`response_tokens`
- 🛡️ **提示词诊断兜底机制**：新增 `diagnostics::DiagnosticStore` Tauri State 与 `get_last_llm_prompt` 命令，解决大提示词事件可能丢失的问题
- 🩹 **修复诊断卡片“未捕获提示词”**：即使 `llm-prompt-sent` 事件未送达，诊断时也会主动通过命令读取完整提示词
- ✅ **验证**：`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.7 — 诊断信息增强 + 超时文案去硬编码（2026-06-22）

- 🩹 **修复诊断版本号硬编码**：`__STORYFORGE_VERSION__` 改为从 `package.json` 动态读取，不再显示 `0.16.0`
- 🩹 **修复超时文案硬编码**：`handleRequestGeneration` / `handleSmartGeneration` 现在从 `settings` 读取 `frontend_timeout_secs` / `smart_execute_total_timeout_secs`，错误提示与诊断卡片均显示实际配置值
- 📋 **诊断卡片新增 AI 生成模式**：显示 `settings.generation_mode`（`auto` / `time_sliced` / `fast` / `full` / `tri_shot`）
- 🤖 **诊断卡片新增当前模型信息**：模型 ID / 名称 / 提供商 / 端点
- 📝 **诊断卡片新增最后发给模型的提示词全文**：后端通过 `llm-prompt-sent` 事件广播，前端实时捕获并展示（上限 12000 字符）
- ✅ **验证**：`cargo check` 零错误；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.6 — 修复 macOS 启动崩溃：VectorStore State 初始化顺序（2026-06-22）

- 🐛 **修复启动 panic**：解决 `state() called before manage() for Arc<dyn VectorStore>` 导致的 macOS 启动崩溃
- 🔧 **根因**：`init_task_system_and_automation` 在 `app.manage(vector_store)` 之前通过 `app_handle.state()` 获取向量存储
- 🔧 **方案**：将 `LanceVectorStore` 创建与 `app.manage(vector_store)` 提前到依赖组件之前，异步 `init()` 保留原地
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 通过；`python3 scripts/architecture_guard.py` 通过

### v0.23.5 — CI 格式化修复（2026-06-21）

- 🎨 **Rust nightly fmt**：修复 import 顺序、函数参数折行、单行化等格式化差异
- 🎨 **前端 Prettier**：修复 `GeneralSettings.tsx` 类型断言单行化差异
- 📋 **无业务逻辑变更**：仅代码风格修复，使 GitHub Actions `rust-check` / `frontend-check` 通过

### v0.23.4 — 智能层闭环落地（2026-06-21）

- 🧠 **LLM JSON mode 原生支持**：新增 `llm::adapter::ResponseFormat::JsonObject`，OpenAI/Ollama 适配器分别附加 `response_format` / `format`，模型网关可透传
- ✍️ **Review/Refine Pipeline 结构化输出**：调用 JSON mode 并解析 `{ refined_content, change_summary, refinement_notes }`
- 💰 **MemoryPack 预算语义强类型化**：`MemoryBudget::for_task_type` 接收 `MemoryTaskType { Write, Plan, Review }`
- 📚 **拆书存储统一**：删除 `reference_characters` / `reference_scenes`，人物/场景数据全部汇入 `narrative_*` 表；迁移 `V100__拆书存储统一_删除_reference_表.sql`
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.3 — 测试基线修复 + 工程化（2026-06-21）

- 🐛 **MigrationRunner 交错执行**：`run_with_legacy` 按版本将 SQL 文件 migration 与 inline Rust migration 交错执行，避免高版本 SQL 文件跳过低版本 inline migrations
- 🗂️ **SING migration 版本上调**：`V095__意图图_SING_数据层.sql` → `V099__...`，确保其跑在所有 inline migrations 之后
- 🗂️ **`narrative_*` 表补 status 列**：`narrative_characters` / `narrative_scenes` / `narrative_world_buildings` 加入 `status TEXT NOT NULL DEFAULT 'active'`，新增 inline Migration 98 为已存在表补列
- 🔄 **ElementSource/ElementStatus round-trip 修复**：`domain/narrative_elements.rs` 新增 `as_str()` / `from_str()`（snake_case 英文）；`db/repositories_narrative.rs` 存储与解析统一使用英文键
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**（新增 3 个测试，零回归）；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.2 — 事件总线与状态同步治理（2026-06-21）

- 📡 **后端 `SyncEvent::ChapterCommitted`**：携带 `projection_status`，`SceneCommitService::apply_commit` 在 projections 完成后统一发射
- 🖥️ **前端 `content/isSaved` 迁移到 `frontstageStore`**：移除本地 `useState`，保留 `isSaved` + editor focus 双重保护
- 🧹 **清理遗留事件/hack**：删除所有 `backstage-data-refreshed` 废弃注释；`useWebViewRedrawFix` 改为 `FIXME` 标记
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 487 passed / 48 failed（零新回归）；`npx tsc --noEmit` 零错误；`npx vitest run` 126 passed / 3 skipped

### v0.23.1 — 架构债务清偿：全局单例治理 + 模块依赖解耦（2026-06-21）

- 🗑️ **全局单例清零**：彻底移除 14 个全局 `static`/缓存，全部改为 Tauri State 注入或每次调用重新加载
- 🏗️ **domain 领域层扩展**：新增 `agent_context` / `agent_types` / `foreshadowing` / `search` / `write_time_bundle` / `asset_snapshot` / `continuity` / `adaptive` / `prompt_synthesis` / `agent_service` / `creative_engine` 等共享类型与端口
- 🔗 **模块循环依赖斩断**：`memory → agents`、`narrative → memory`、`narrative → creative_engine` 数据类型下沉到 `domain`；`agents ↔ creative_engine` 行为依赖通过 `CreativeEnginePort` / `AgentServicePort` 双向反转
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed / 48 failed（零新回归）；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.0 — TriShot 三击生成管线：关键路径压缩至最多 3 次 LLM（2026-06-21）

- 🎯 **TriShot 三击管线**：新增 `GenerationMode::TriShot`（三击），Call 1 最快模型选资产+合成提示词 → Call 2(可选) 精修 → Call 3 Writer 生成。质检/改写/入库/洞察全部下沉后台静默执行
- ⚡ **快速模型选取**：`GatewayExecutor::select_fastest_profile()` 按算力档案 TTFB 升序选最快可用模型，`LlmService::generate_with_fastest()` 捷径
- 🧩 **prompt_synthesis 模块**：`AssetManifest` 把 ~17 段资产打包为紧凑清单（4000 字符预算）+ `PromptSynthesizer` JSON 结构输出 + `PromptRefiner` 可选精修（预算守卫跳过）
- 🏎️ **PlanExecutor 快速路径**：TriShot 跳过 SING/PlanGenerator，`PlanStep::long_running` 跳过 90s 步超时
- 🤖 **BGP-2 智能改写**：`auto_rewrite_executor.rs` 按严重度分流——HIGH 自动改写+可撤销，LOW 仅建议
- 📡 **SyncEvent 扩展**：`ContentAutoRevised`（toast 通知）+ `RevisionSuggested`（审阅面板）
- 🖥️ **前端配置**：设置页面新增「三击模式」下拉选项
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed（新增 TriShot 19 测试全部通过，零回归）；`npx tsc --noEmit` 零错误

### v0.22.4 — 「异星球末世生存」智能创作流程优化 + 后台资产审计（2026-06-21）

- 🧩 **GenreResolver 题材解析**：精确/别名/子串/同义词/复合题材解析，解决自然语言题材词断链
- 🗺️ **意图图资产发现增强**：`AssetNode` tags + `discover_assets` 复合题材补充发现
- 🌉 **模型网关资产感知调度**：`asset_tags`/`discovered_asset_ids` 全链路透传，任务类别按标签校准
- ✍️ **TimeSliced 复合题材补强**：`secondary_genre_profile_strategy` 注入次要题材画像
- 📋 **后台资产全面审计**：新增 `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md`，梳理全部 22 类创作资产、智能创作流程注入点、12 项断链/断环问题与 10 条修复建议
- 🗺️ **项目流程图技术文档**：新增 `docs/PROJECT_PROCESS_FLOWCHARTS_v0.22.4.md`，覆盖创世、拆书、智能创作主路径、79+ 提示词、43 个网文题材模板、40+ 创意资产、Story System 全子系统流程图
- 🏗️ **架构审计报告**：新增 `docs/BROOKS_LINT_ARCHITECTURE_AUDIT_v0.22.4.md`，模块依赖图 + 6 大 decay risks 诊断，Health Score 18/100
- ✅ **验证**：新增 targeted tests 39 passed；`cargo check` 零错误；`npx tsc` 零错误

### v0.22.3 — 钥匙串彻底移除 + 模型健康报告自动刷新（2026-06-21）

- 🔐 **钥匙串彻底移除**：删除 keyring crate、secure_storage 模块、store_api_keys_securely 配置项
- 🧹 **移除 ~260 行钥匙串读写逻辑**：load/save 中全部钥匙串访问已清除
- 📊 **模型健康报告自动刷新**：前端每 30 秒自动刷新，后端改为 async
- ⚡ **冗余 load 消除**：execute_writer 2→1 次、FirstChapterGenerationStep 3→1 次
- ✅ **零回归**：cargo check 零错误，425 passed，tsc 零错误

- GenreProfile 推荐资产种子：7 个题材写入推荐风格/方法论/技能
- 策略选择硬约束：体裁画像有推荐时跳过 LLM 直接使用
- 算力档案默认值修正：capability_score 未测试时默认 0.0

### v0.22.1 — 5 条建设性意见（2026-06-21）

- StrategySelector 题材推荐映射：7 种题材→风格推荐
- StyleDNA 句长偏差检测：>30% 偏差记录建议
- Inspector 方法论动态 prompt：5 种方法论全覆盖
- GenreProfile 推荐字段：4 新列 + Migration 96
- 算力档案质量分权重：HeavyCreation→quality80%

### v0.22.0 — 提示词与后台资产深度结合（2026-06-21）

- Phase A：TimeSliced 路径全资产注入（StyleDNA六维+方法论+题材画像+策略）
- Phase B：Inspector 全资产注入（体裁画像+角色状态+冲突+四元组）
- Phase C：意图感知调度接线（agent_type→intent 自动推导）
- Phase D：算力档案消费闭环（TTFB/TPS 参与候选排序）
- Phase E：资产→生成参数规则映射（asset_params.rs）

### v0.21.0 — 提示词全量可配置化（2026-06-21）

- 79 个提示词全部前端可编辑（21 个分类）
- 假接入修复：15 个 key 改为 resolve_prompt（含 DB 覆盖）
- 旁路接线：40+ 个硬编码提示词全部接入 registry
- 前端 Monaco 编辑器 + 批量导入导出

---

## 🔧 编译状态

| 检查项                                    | 状态                                                |
| ----------------------------------------- | --------------------------------------------------- |
| `cargo check`                             | ✅ 零错误                                           |
| `cargo test --lib`                        | ✅ 974 passed / 0 failed / 2 ignored                |
| `cargo test --lib intention_graph`        | ✅ 21/21                                            |
| `cargo test --lib adaptive::asset_params` | ✅ 3/3                                              |
| `cargo test --lib genre_resolver`         | ✅ 5/5                                              |
| `cargo test --lib selector`               | ✅ 6/6                                              |
| `cargo test --lib write_time_bundle`      | ✅ 13/13                                            |
| `cargo test --lib dispatcher`             | ✅ 5/5                                              |
| 真实模型测试（Gemma4-e2b）                | ✅ 6/6                                              |
| `npx tsc --noEmit`                        | ✅ 零错误                                           |
| `npx vitest run`                          | ✅ 305 passed                           |
| `cargo +nightly fmt -- --check`           | ✅ 零差异                                           |
| `npm run format:check`                    | ✅ 零差异                                           |
| `python3 scripts/architecture_guard.py`   | ✅ 通过                                             |
| 后台资产审计                              | ✅ 完成，见 `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md` |
| 已知测试失败                              | ✅ 无（V092 基线问题已在 v0.23.3 清零）             |

---

## 📊 提示词覆盖统计

| 类别                         | 数量   | 状态            |
| ---------------------------- | ------ | --------------- |
| Writer/Inspector/Commentator | 5      | ✅ 全部可覆盖   |
| Planner/Analyzer             | 4      | ✅ 全部可覆盖   |
| Pipeline（审稿/修稿/后处理） | 4      | ✅ v0.22.0 新增 |
| Audit（质量审计）            | 1      | ✅ v0.22.0 新增 |
| Intent（意图解析）           | 1      | ✅ v0.22.0 新增 |
| Deconstruction（拆书）       | 5      | ✅ v0.22.0 新增 |
| Creation（创世流程）         | 14     | ✅ v0.22.0 新增 |
| Strategy（策略选择）         | 1      | ✅ v0.22.0 新增 |
| Methodology（方法论）        | 19     | ✅ 全部可覆盖   |
| Skill（技能）                | 5      | ✅ 全部可覆盖   |
| Memory/Knowledge/Probe       | 7      | ✅ 全部可覆盖   |
| Narrative（叙事）            | 2      | ✅ 全部可覆盖   |
| World/Character（世界/角色） | 6      | ✅ 全部可覆盖   |
| System/Other                 | 5      | ✅ 全部可覆盖   |
| **总计**                     | **79** | ✅              |
