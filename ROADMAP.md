# StoryMoss (草苔) 开发路线图

> 最后更新: 2026-10-07（v0.64.3：下引号孤行根除——句读切分收尾符归属 + 段首并回规则 + V139 存量迁移）

## ✅ v0.27.x–v0.58.x 已实施完成

### ✨ v0.64.3 - 下引号孤行根除 ✅ (2026-10-07)

- [x] **句读切分**：句末标点后紧跟的收尾符归属本句（段首孤引号的源头）。
- [x] **段首并回**：`mergeLeadingClosingPunctParagraphs`（前端）+ `TextUtils::merge_leading_closing_punct_paragraphs`（Rust），方向性收尾符限定、实体形态覆盖。
- [x] **路径统一**：`autoFormatText` 三条路径 + `textToParagraphsHtml` 走「孤段 + 段首」合并。
- [x] **V139 存量迁移**：已入库章节下次启动自动修好（幂等）。
- [x] **真机验收**：库内 2 处段首孤引号 → 修复后 0 处。
- **未关闭**：需新版本启动触发迁移；真机端到端未复跑。

### ✨ v0.64.2 - 续写人物关系错乱修复 ✅ (2026-10-07)

- [x] **配偶推导扩散**：`kin.contains("配偶")` 整串匹配 → 逐段匹配（同段含对方姓名 + 配偶信号），去掉 `（配偶向）` 内部标记泄漏。
- [x] **落库覆盖过宽**：`persist_inferred_relationships` 改为只修脏（叔/姑/侄）或填空，既有父子 / 兄妹 / 同僚不被改写。
- [x] **关系不变量**：`sanitize_relations` / `sanitize_bundle_relations` 覆盖两条注入路径；被拦下行入质量债 `continue_relations`。
- [x] **守卫缺口入账**：探针遗留缺口入质量债 `continue_probe`（此前只打日志）。
- [x] **本机数据修复**：32 行误写夫妻 → 回填 6 / 删除 26；幻影人物「景亲王江顾然」清除。
- **未关闭**：运行中的 0.64.1 需升级到本版才会生效；第二章正文错乱台词待作者决定是否重生成；同名重复角色行未合并；真机端到端未复跑。
### ✨ v0.64.1 - 发布链路恢复 ✅ (2026-10-07)

- [x] **0.64.0 上线确认**：latest.json=0.64.0，四个安装包全 200（含补齐的 `.deb`）；macOS 公证随协议签署解除。
- [x] **landing 兜底版本** 0.58.0 → 0.64.0（最近一次确认在线版本）。
- [x] **分章用例加固**（waitFor 显式 5s）＋ v0.60.0–v0.63.0 补推 GitHub。
- **未关闭**：真机端到端复跑（三把尺子 + 长篇 10+ 章）。

### ✨ v0.64.0 - 运行维护页 + 声明式投影路由表 ✅ (2026-10-06)

- [x] **P3-F 声明式投影路由表**：产物 × writer 纯数据表；注册顺序/状态键派生；路由摘要入日志；6 项契约测试。
- [x] **UI 缺口补齐**：幕后「运行维护」页四 Tab（质量债 / 待确认 / 文风偏好 / 成本）+ 文风偏好管理命令。
- [x] **CI 修复**：`clippy::redundant_comparisons` 阻塞 v0.63.0 发布；`cargo clippy` 纳入每版验证。
- **未关闭**：真机端到端复跑；网站发布待本版 tag 跑通后核对 latest.json。

### ✨ v0.63.0 - 工程纪律（P0–P3 收官）✅ (2026-10-06)

- [x] **P3-A** 质量债台账（V138）：降级/未解决问题入账 + 建议回收窗口 + 结清/忽略命令。
- [x] **P3-C** 检查点（每 10 章快照）+ 时间旅行查询 `query_story_as_of`。
- [x] **P3-D** 终局指南针（确定性派生 + 续写注入）。
- [x] **P3-B** 待确认队列（V138，规则类新增物）+ 确认/拒绝命令。
- [x] **P3-E** 三把尺子组合契约测试。
- [ ] **P3-F** 声明式投影路由表：未实施（架构级重构，留待后续）。
- [ ] 前端 UI（质量债/待确认/成本/文风偏好四处的界面）：后端命令已就绪，待接。
- **未关闭**：真机三把尺子端到端复跑；网站发布待 Apple 公证解阻。

### ✨ v0.62.0 - 文本质量与成本 ✅ (2026-10-06)

- [x] **P2-A** `prose_lint` 确定性两档规则 + 接入 commit/editor_qc。
- [x] **P2-B** V137 `style_preferences` + 防抖提炼 + 续写注入【作者文风偏好】。
- [x] **P2-C** 伏笔 evidence/strength/subtlety/链 + 单章上限 5 + 注入话术窗口分档。
- [x] **P2-D** `llm::cost` 聚合 + 阈值提示 + 零增量计费盲区哨兵 + 命令。
- **进行中**：P3 工程纪律（质量债 / 新增物审批 / 检查点与时间旅行 / Compass / 三测试套件）。

### ✨ v0.61.0 - 分层记忆金字塔 ✅ (2026-10-06)

- [x] **P1-A 语义摘要**：commit 摘要 LLM 化（100-150 字），解析拒绝 JSON/过短/过长，失败回退截断。
- [x] **P1-B 段摘要 + 全书纲要**（V136）：每 10 章一条段摘要，段数 ≥3 生成全书纲要，commit 后台补齐。
- [x] **P1-C 自适应窗口**：10 / 5 / 3 章窗口按书长切换；长篇续写注入【故事纲要】。
- [x] 契约测试 10 项（窗口/段数学/幂等 upsert/纲要块/摘要解析与回退）；全量 Rust 1653。
- **进行中**：P2 文本质量与成本（prose_lint / 风格逆向学习 / 伏笔增强 / 预算哨兵）→ P3 工程纪律与三测试套件。

### ✨ v0.60.0 - 三把尺子：知识边界 / 物品归属 / 改稿级联影响报告 ✅ (2026-10-06)

- [x] **P0-1 知识边界**（V135 `story_timeline_events` + `character_knowledge_log`）：世界真相/读者认知/揭示状态双栏；ingest `knowledge_updates` 修 secrets 冻结断链；续写注入信息差禁令（计划内豁免）+ 泄密探针 + editor_qc 疑点清单。
- [x] **P0-2 物品归属**（V135 `item_holdings`）：持有者账本 + 续写【在场物品】+ 归属矛盾探针（当场转手 / 遗失物再现豁免规则）。
- [x] **P0-3 级联影响报告**（V135 `cascade_impacts`）：re-ingest 后自动影响分析 + LLM 冲突扫描 + `CascadeImpactDetected` 事件 + 4 命令 + 幕后「级联中心」；只报告不改写后文。
- [x] 契约测试 15 项（含帖主测试③「改第 3 章 → 第 7 章冲突」）；全量 Rust 1643 / 前端 590。
- **进行中**：P1 记忆质量（语义摘要 + 分层金字塔 + 自适应窗口）→ P2 文本质量与成本 → P3 工程纪律与三测试套件。

### ✨ v0.59.4 - 发布纪律门禁与网站链路修复 ✅ (2026-10-06)

- [x] `docs-guard`：tag 推送机械校验 8 份必需文档（README/CHANGELOG/AGENTS/PROJECT_STATUS/ROADMAP/ARCHITECTURE/TESTING/USER_GUIDE）。
- [x] 补齐 ARCHITECTURE.md v0.59.1–v0.59.3；landing 兜底回退 0.58.0。
- **未关闭（需人工解阻）**：签署 Apple 开发者协议 → 重跑 macOS 构建 → upload-to-website，线上才会出现 0.59.x 与 `.deb`。

### ✨ v0.59.3 - 手写大纲不再被机器改写 ✅ (2026-10-06)

- [x] V134 source 列 + 三条写入路径分流（materialize 不覆盖 / ingest 不追加 / 用户保存打标）；机器来源仍可精炼。
- [x] 删死模块 hybrid_search.rs 与 capability 死权限 http:default。
- **未关闭**：真机续写；withGlobalTauri + CSP；FTP 明文；Agency↔agents 环依赖与 coordinator 拆分；llm_calls 保留；src-server CI。

### ✨ v0.59.2 - 修静默清空、清死代码、归档旧文档 ✅ (2026-10-06)

- [x] 载入期空文档保护（含 `isEmptyEditorHtml` 单测与 E2E 复现）；JSON 尾随逗号换行形态修复。
- [x] E2E 提升为阻塞门；删除零引用扩展/孤儿 hook；根目录陈旧 .md 归档 33 份；landing SRI。
- **未关闭**：真机续写；src-server CI 覆盖；withGlobalTauri/CSP；FTP 明文；story_outlines 覆盖语义；Agency↔agents 环依赖。

### ✨ v0.59.1 - 构建修复：对齐新版 nightly rustfmt ✅ (2026-10-06)

- [x] 整仓按新版 nightly 的注释折行规则格式化（106 文件，无逻辑改动），恢复 CI 格式门。
- **未关闭**：浮动 nightly 仍会漂移（CI 红时按 CHANGELOG 指令重跑 fmt）。

### ✨ v0.59.0 - 验收证据链、数据层治理与续写质检闭环 ✅ (2026-10-06)

- [x] CI：`cargo test --lib` 恢复阻塞；新增网关故障注入测试 + golden 续写 harness（`#[ignore]`）；新增幕前续写 E2E spec。
- [x] 迁移治理：checksum（V132）+ 集合水位线；迁移目录剔除 target 构建产物；V133 六个索引；故事删除级联；活动日志 30 天剪枝。
- [x] 续写质检闭环：QC 事件带 mode/chapter；幕前「按审查意见修订本章」；后台 ingest/QC 纳入预算与取消传播。
- [x] 安全/发布：server JWT/DEV_UPGRADE 缺省加固；导出 ZIP 默认剔除 API key；发布白名单补 .deb。
- [x] 工程：AGENTS.md 瘦身、FrontstageApp 保存链 hook、全面检视报告落盘。
- **未关闭**：真机续写未复跑；载入期空保存竞态（E2E 门禁暂留非阻塞）；src-server CI 覆盖；golden 基线需真机。

### ✨ v0.58.0 - 戏剧工艺 + 短剧格式 ✅ (2026-08-29)

- [x] `SceneBeatCard.change_delta`；`CONTINUE_BEAT_SYSTEM` 原地踏步范例；编辑审计 impact/fix。
- [x] 复述近文且未兑现改变项才探针 gap；旁白不误杀。
- [x] V131 `story_format` 默认 novel；`DRAMA_BEAT_SYSTEM`；幕后长篇/短剧选择。
- **未关闭**：真机创世/续写/短剧；不得宣称续写质量已修复。不分镜。

### ✨ v0.56.2 - 下引号不再单独成段；编辑审计顶栏不再报失败 ✅ (2026-08-28)

- [x] 悬挂闭合引号合并跳过全角缩进；空行/存量 HTML 并回上一句。
- [x] 后台审查 fail-open 不占顶栏失败文案。
- **未关闭**：真机须再打开该章或再续写；不得宣称续写质量已修复。

### ✨ v0.56.1 - 拆人探针认抱衣角；死人不得再用眼睛锁定 ✅ (2026-08-28)

- [x] 独立出现的两个称呼即拆人；死人目光/锁定缺口。
- **未关闭**：真机须从「飞身扑上」再跑；不得宣称续写质量已修复。

### ✨ v0.56.0 - 续写导演锁 / 管理 Agent 必写人物关系 ✅ (2026-08-27)

- [x] 拍前 `continue_director` 一人一号 + 近文亲缘；主创仍 `complete()` 零工具。
- [x] 探针拦拆身 / 亲缘写反；冻结含锁。
- [x] 管理 Agent 写角色后 upsert `character_relationships`；空表 `ensure_relationships`。
- **未关闭**：真机须从「飞身扑上」再跑；不得宣称续写质量已修复。角色表脏行仍不自动删除。

### ✨ v0.55.0 - 资产渐进展开 / 续写冻结 / 短操作合同 ✅ (2026-08-25)

- [x] Producer/Editor catalog 名+一行；`asset_read` 按名取全卡。续写零工具。
- [x] `write_beat_once` 冻结节拍卡/阵容，返回后解冻。
- [x] `CONTINUE_BEAT_SYSTEM` 短合同 + 三条对错范例。
- **未关闭**：真机创世/续写；不得宣称 JSON 熔断或续写质量已修复。

### ✨ v0.54.0 - Agency ToolLoop 原生 function calling ✅ (2026-08-25)

- [x] ToolLoop 发角色白名单 JSON Schema；原生 `tool_calls` 优先，文本 JSON 回退。
- [x] 续写 `write_beat_once` / `assemble_continue_beat` 不带 tools。
- **未关闭**：真机创世/资产路径；不得宣称 JSON 熔断或续写质量已修复。第二至四期已在 v0.55.0 落地。

### ✨ v0.53.6 - 已写完的行刺/死亡不再被续写重演 ✅ (2026-08-25)

- [x] 死人退出行动阵容；已完成高潮不再当下一拍；探针拦重演。
- **未关闭**：须用「飞身扑上」同一开头再跑真机；不得宣称续写质量已修复。

### ✨ v0.53.5 - 大纲重写确认框（确认 / 取消 / 重写） ✅ (2026-08-25)

- [x] 含大纲的按正文重写不立即落库，幕前可编辑。
- [x] 确认才写入；取消废弃；重写按原指令再生成。
- **未关闭**：原句真机；不得宣称续写质量已修复。

### ✨ v0.53.4 - 「写后续的故事大纲」不进续写正文 ✅ (2026-08-24)

- [x] 写/生成宾语是大纲时 `looks_like_asset_refresh_shape` 为真，覆盖续写兜底。
- [x] 「写后续」「按照故事大纲写后续」「重新生成下一章」仍不是本作业。
- **未关闭**：原句真机；不得宣称续写质量已修复。

### ✨ v0.53.3 - 按正文重写大纲幕前弹出落库预览 ✅ (2026-08-24)

- [x] 落库摘要带回【故事大纲】【场景大纲】；幕前弹「已按正文重写设定」。
- [x] 「后续 / 接下来 / 下一」从章末往下写，禁止复述开场。
- **未关闭**：原句真机弹窗；不得宣称续写质量已修复。

### ✨ v0.53.2 - 键值散文与场景大纲按正文重写 ✅ (2026-08-23)

- [x] 无花括号 `story_outline:` / 中文冒号 `scene_outline：` 解析落库。
- **未关闭**：原句真机；不得宣称续写质量已修复。

### ✨ v0.53.1 - 按正文重写不再丢对象 JSON ✅ (2026-08-21)

- [x] `story_outline` 对象走 `normalize_outline`；中文键；散文 salvage。
- **未关闭**：同一句真机；不得宣称续写质量已修复。

### ✨ v0.53.0 - 按正文重写生产资产 ✅ (2026-08-21)

- [x] 幕前「按正文重写」不改正文，只改点名的生产资产。
- [x] 分类后置纠正禁止当续写；`result_kind=asset_refresh`。
- [x] 接地过滤 + `user_created` 只填空；场景大纲保留手写前缀。
- **未关闭**：真机同一开头；不得宣称续写质量已修复。

### ✨ v0.52.0 - 续写拍级分层与准入轨迹 ✅ (2026-08-21)

- [x] `PRIOR_CAST_CHAR_CAP` 500→1500。
- [x] 大纲/配额/伏笔点名准入；L2/L1 分层卡。
- [x] `story_outlines` 回流保留核心冲突+最近 5 转折。
- [x] `continue_assets:` 准入轨迹日志。
- **未关闭**：真机续写；P3 ingest freshness；P4 ContextPrioritizer；不得宣称质量已修复。

### ✨ v0.51.6 - 工具档/后台档不再被创作模型挤到同一台机 ✅ (2026-08-20)

- [x] `generate()` 不再把当前/创作模型盖回工具档、后台档链头。
- [x] 设置页写明主创/管理/编辑审计与三档对应。
- **未关闭**：真机须用同一三档再点续写确认候选第一名；不得宣称续写质量已修复。

### ✨ v0.51.5 - 进行中的续写不再弹前台中断卡 ✅ (2026-08-20)

- [x] `active_run` 冲突不渲染中断卡，二次点击静默。
- [x] 底栏「Agency 续写中」是唯一状态面。
- **未关闭**：真机须再点续写确认不再弹卡。

### ✨ v0.51.4 - 「已有创作任务」不再打发去设置 ✅ (2026-08-20)

- [x] 进行中冲突弹窗改为等待/取消，不再去设置。
- [x] 二次点击不清理真正那次生成状态。
- **未关闭**：真机须再点续写确认弹窗。

### ✨ v0.51.3 - 续写规划污染不再把 600 秒耗尽 ✅ (2026-08-20)

- [x] 大纲注入前切断「故事大纲归纳」类规划。
- [x] 主创过短且剩余 <90s 不重试。
- [x] 取消后不再试下一个候选。
- [x] 契约 +4；`cargo test --lib` 1470 passed / 2 ignored。
- **未关闭**：真机须再点续写；库内旧规划不改表，靠读取切断。

### ✨ v0.51.2 - 续写切断节拍卡/约束规划泄露 ✅ (2026-08-19)

- [x] 节拍卡/状态网/约束清单行话全文切断；文首清空触发重试，文中切断保留前缀。
- [x] 续写 system 禁止输出任务分析。
- [x] 契约：Qwen dump 清空、正文后剥离、sanitize 空串；`cargo test --lib` 1466 passed / 2 ignored（+3）。
- **未关闭**：真机须用同一开头再点续写；不得宣称续写质量已修复。

### ✨ v0.51.1 - 幕前取消键去掉系统原生凸起 ✅ (2026-08-17)

- [x] 取消键 / 发射键卸掉 macOS Aqua 原生按钮外观。
- [x] 取消键透明底，hover 与顶栏设置键同脚印（陶土 18% tint）。
- [x] flush 输入条 CSS 双杀 UA `<button>`。

### ✨ v0.51.0 - 手写/粘贴正文触发三角色观察 ✅ (2026-08-17)

- [x] 与自动分章同一 30s 空闲窗口启动观察 run。
- [x] 管理回流、主创编译大纲/节拍卡（不改正文）、编辑审查只写审查区。
- [x] ≥200 字增长才重跑；创世/续写让路；observing 不挡住点续写。
- [x] 验证：`cargo test --lib` 1463 passed / 2 ignored（+9）；vitest 592 / 3 skipped（+1）。
- **未关闭**：真机须粘贴 ≥200 字停手 30s；整章替换未多 200 字不重跑；分章新章等下次保存；auto_commit 与观察管理可能各烧一轮 LLM。

### ✨ v0.50.2 - 自动分章后章节名跟随章号 ✅ (2026-08-17)

- [x] 重排后「第N章 / 第一章」跟随新章号；自定义标题不动。
- [x] 幕前按 `chapter_number` 显示派生名。
- [x] Append：DB 为客户端前缀且多出 ≥200 字时用 DB 做底稿。
- [x] V130 修存量派生标题。
- [x] 验证：`cargo test --lib` 1454 passed / 2 ignored（+4）；vitest 591 / 3 skipped（+1）。
- **未关闭**：已重复章节正文不自动删。

### ✨ v0.50.1 - 自动分章后续写不再误报「请先打开一个章节」 ✅ (2026-08-17)

- [x] 分章自动切换补拉 `get_chapter_scenes`，`sceneId` 不再回落 `chapter.id`。
- [x] `resolve_append_scene_id`：chapter id → 该章关联 scene（与 `update_scene` heal 同口径）。
- [x] 验证：`cargo test --lib` 1450 passed / 2 ignored（+1）；vitest 590 / 3 skipped。
- **未关闭**：真机须用同一开头再点续写，不得宣称唱反调已修复。

### ✨ v0.50.0 - 续写三角色闭环 ✅ (2026-08-16)

- [x] 后台回流/质检/补齐 start+done 落库（含失败/超时/未获得锁）。
- [x] 本拍资产投影到当前 run 资产栏，卡片可点进角色/故事/世界。
- [x] Append 写入 `【当前场大纲】`；下一拍优先读该块；ingest 不覆盖。
- [x] 未解决 revise 最多 2 条进入下一拍节拍卡。
- [x] 验证：`cargo test --lib` 1449 passed / 2 ignored（+13）；vitest 590 / 3 skipped（+2）。
- **未关闭**：真机须用同一开头再点续写，不得宣称唱反调已修复。

### ✨ v0.49.1 - 卸掉幕前划词润色浮条 ✅ (2026-08-16)

- [x] 幕前划选不再弹出润色/扩写/指令条；删除 `AiSelectionActions`。
- [x] 契约：划选长句不渲染 `ai-selection-actions`。
- [x] 验证：`npx vitest run` 588 passed / 3 skipped（−17）。

### ✨ v0.49.0 - 续写大纲以正文为真相源 ✅ (2026-08-16)

- [x] 有正文时禁止按书名发明角色/大纲；落库姓名门闩。
- [x] 空方法论落库 `scene_structure`；从章节+场景结构归纳大纲。
- [x] 管理熔断 salvage + 后台续跑；未接地书纲不注入；场外开篇探针。
- [x] 验证：`cargo test --lib` 1436 passed / 2 ignored（+18）。
- **未关闭**：真机须用同一开头再点续写，不得宣称唱反调已修复。角色表脏行不清理。

### ✨ v0.48.1 - 划词浮条不再挡住手工写作 ✅ (2026-08-16)

- [x] 选区 ≥4 字且鼠标松开后才弹出润色/扩写条；idle 无输入框；Esc 收起。
- [x] 验证：`npx vitest run` 605 passed / 3 skipped（+4）。

### ✨ v0.48.0 - 续写按镜头在场、禁止旧快照覆盖、未确认幽灵先写入 ✅ (2026-08-16)

- [x] 节拍卡在场改末 500 字镜头；去掉角色表「补位上场」。
- [x] `compile_next_node` 跳过不点名本拍在场者的书大纲句。
- [x] Append 底稿取 DB 与客户端更长者；连续续写先写入未确认幽灵。
- [x] 末句禁止复述已完成动作；`NewScene` 不罚丢掉已在场者。
- [x] 验证：`cargo test --lib` 1418 passed / 2 ignored（+5）；vitest 601 / 3 skipped。
- **未关闭**：真机 8 次须在 0.48.0 上重跑。v0.47.0 真机已失败，不得宣称五症状已修复。角色表脏行不清理。

### ✨ v0.47.0 - 续写质量闭合（债务/节点/阵容/状态网） ✅ (2026-08-15)

- [x] P0 编译器修洞：债务正文兑现、节点不回绕、冲突看阵容、事实写回、续写回退带节拍卡。
- [x] P1 身份与场次：别名、入场门闩、地点 shift、conflicts/goals 进筛选器。
- [x] P2 拍级状态网 + 探针一次重试 + location 规则回写。
- [x] P3 CI 八拍 mock 契约。
- [x] 验证：`cargo test --lib` 1413 passed / 2 ignored（+22）。
- **未关闭**：真机 8 次幕前续写未跑，不得宣称三症状已修复。

### ✨ v0.46.0 - 传统色主题（纸帘印 + 幕前幕后分选） ✅ (2026-08-15)

- [x] 十二套写作向传统色；幕前/幕后分选；旧四套迁移；印=锚色。
- [x] 验证：`npx vitest run` 601 passed / 3 skipped（+11）。
- **未关闭**：全界面截图回归；v0.42.0 §8 真机探针。

### ✨ v0.45.1 - 续写前文改为开篇+近文双窗 ✅ (2026-08-15)

- [x] 短章全文；长章开篇 600 + 近文 1800；剥 HTML；在场只看近文；预算保章末。
- [x] 验证：`cargo test --lib` 1391 passed / 2 ignored（+6）。
- **未关闭**：不叠更早几章全文；v0.42.0 §8 真机探针。

### ✨ v0.45.0 - 提示词运行时组装 ✅ (2026-08-14)

- [x] `prompts/assembly.rs` 哑拼接器 + 创世/续写/ToolLoop 工厂；P0 非空路径金标。
- [x] 场景预览改报 Agency 续写/创世；`prompts` ↛ `agency`；三工具用法行；内置模板 leftover CI。
- [x] 验证：`cargo test --lib` 1385 passed / 2 ignored（+18）；vitest 590 / 3 skipped。
- **未关闭**：空资产/空末句 trim 金标；ToolLoop head 双构造；v0.42.0 §8 真机探针；P3 producer/concept_pack。

### ✨ v0.44.1 - 幕前输入框去掉系统原生描边 ✅ (2026-08-14)

- [x] `AiPromptBar` textarea `appearance-none border-0 shadow-none`；flush CSS 双杀 UA 边。
- [x] 验证：`npx vitest run` 590 passed / 3 skipped（+1）。
- **未关闭**：全界面截图回归；健康探测钮仍 `scale(1.1)`。

### ✨ v0.44.0 - 墨纸 / 机械视觉定向进化补齐 ✅ (2026-08-14)

- [x] P0 幕前输入无框：底栏无顶边/毛玻璃/独立卡片壳。
- [x] P1 Medium 分文件、纸 chroma hue 95、选区 22%、顶栏 press/淡彩。
- [x] P2 暖金内芯同色相、Panel 高光、弹簧 500ms、侧栏去金框。
- [x] 验证：`npx vitest run` 589 passed / 3 skipped（+11）。
- **未关闭**：全界面截图回归；健康探测钮仍 `scale(1.1)`。

### ✨ v0.43.0 - 墨纸 / 机械视觉定向进化 ✅ (2026-08-14)

- [x] 幕前输入条 `AiPromptBar variant="flush"`：一层纸面、陶土淡彩发射、取消去 pulse。
- [x] 霞鹜文楷本地 woff2；幕后色板/阴影收软；press；EmptyHint。
- [x] 验证：`npx vitest run` 578 passed / 3 skipped（+22）。

### ✨ v0.42.0 - 续写按拍选取创作资产 ✅ (2026-08-14)

- [x] `agency/continue_assets.rs`：准入合并、名单、大纲去重、世界观截取、关系过滤、6000 字预算。
- [x] `write_beat_once` / `write_chapter` 先 BeatCard 再筛选渲染；`to_prompt()` 不动。
- [x] 验证：`cargo test --lib` 1367 passed / 2 ignored（+13）。
- **未关闭**：规格 §8 真机对照诊断故事再续一拍（需 LLM）。

### ✨ v0.41.2 - 续写 600s 超时：跳过超窗候选、散文失败不再进 tool_loop ✅ (2026-08-14)

- [x] `GatewayExecutor::generate`：窗口装不下则跳过候选（`candidate_fits_prompt`）。
- [x] `write_beat_once`：散文回退失败不再进入 `write_chapter` tool_loop。
- [x] 验证：`cargo test --lib` 1354 passed / 2 ignored（+4）。

### ✨ v0.41.1 - Agency 续写上线核验加固 ✅ (2026-08-13)

- [x] `write_beat_once`：`sanitize_novel_output` + 8% 自重复重试。
- [x] 改写 `resolve_rewrite_generation_mode`：永不选 TimeSliced/TriShot。
- [x] 划词选区不走 Agency Append；文思活跃测试断言 `scene_id`。
- [x] 测试环境跳过 finalize LLM 摘要，连续 Append 不再抽干 mock。
- [x] 验证：`cargo test --lib` 1350 passed / 2 ignored（+5）。

### ✨ v0.41.0 - Agency 唯一续写路径 + 幕前同章追加 ✅ (2026-08-13)

- [x] 创世/幕前/幕后续写只走 Agency 三角色；幕前续写/文思活跃 `PersistMode::Append` 同章追加；幕后「续写一章」仍 NextChapter
- [x] SceneBeatCard 0 LLM 编译本拍硬任务 + Bundle 情感四元组/关系 + 落库写回出场/冲突/地点 + 按拍债务
- [x] 切断 TimeSliced/TriShot 续写路由；`generation_mode` 仅管改写；划词改写路径不动
- [x] 验证：`cargo test --lib` 1345 passed / 2 ignored（+17）；`npx vitest run` 556 passed / 3 skipped
- **已知债务（本版登记）**：`ContextPrioritizer` 分级排序未接到 Agency 热路径（本版以 BeatCard 双锚点为 Critical，重评估条件：长篇续写仍出现 Lost-in-the-Middle）；`characters_present` 旧数据 id 与名字混杂

### ✨ v0.40.0 - AI 原生组件库 P3（数据展示六件套）+ P4（项目收尾） ✅ (2026-08-13)

- [x] P3 数据展示六件套：AiSearchList / AiCodeBlock / AiDiffTable / AiFilterTable / AiRecordsTable / AiInsightCards 入库 `components/ui/ai/` 并替换幕后落点（PromptsPanel/UsageStats/AgencyEval/Logs/TracingPanel/Mcp/Skills/IntentionGraphDiagnostics）；AiChat 经勘察关闭（ChatComposer 为 AiPromptBar 严格子集）
- [x] P4 清理：P1-P3 替换残留 TS 13 处 + 死 CSS 约 40 类；历史死件 8 件（AiSuggestionBubble/AiHintOverlay/HelpPanel/ZenModeExit/useLlmStream/useStudioConfig/hetiAddon/Toggle）
- [x] P4 修正：新增 `--ai-on-accent` 令牌（契约 17 变量）替换 text-white ×4；/N 透明度失效 13 处 color-mix 修复；Tasks 裸 pre → AiCodeBlock；AiDiffTable testid per-row key
- [x] P4 浅色页令牌化：AgencyEval / AgencyStudio / AgencyLearning 三页令牌化（AgencyLearning 裸表 → AiRecordsTable），gray 映射固化为约定
- [x] 验证：`npx vitest run` 556 passed / 3 skipped（P3 564 − 死件自带测试 8）；`cargo test --lib` 1328 passed / 2 ignored（无 Rust 改动）

### ✨ v0.39.0 - AI 原生组件库 P1+P2（共 10 组件）+ 保存 UNIQUE 修复 ✅ (2026-08-12)

- [x] P1 生成体验五件套：AiLoading / AiThinking / AiStreamingText / AiPromptBar / AiApprovalCard 入库 `components/ui/ai/` 并逐点接入幕后/幕前落点；`--ai-*` 语义令牌桥（16 变量）双窗口各自定义，tailwind 注册 ai 色组 + 9 个动画工具
- [x] P2 代理与任务五件套：AiContextCards / AiToolChips / AiRecommendationCard / AiTaskRows / AiSelectionActions 入库并接入落点；frontstage.css 补 `--shadow-float`
- [x] 保存修复：scene 自愈补建重定向既有关联 scene（不再补建重复行）+ 序号被占取 MAX+1 避让，根治幕前保存 UNIQUE 失败
- [x] 验证：`cargo test --lib` 1328 passed / 2 ignored（+2）；`npx vitest run` 523 passed / 3 skipped（+68）

### ✨ v0.38.2 - 代理工作室实时动态持久化 + 前端轮询 ✅ (2026-08-12)

- [x] DB 持久化：新增 `agency_activity_log` 表（V129 迁移），`emit_activity` / `emit_progress` 在 `app.emit()` 后 fire-and-forget 写 DB（`spawn_blocking`，不阻塞创世，失败仅 warn）
- [x] 后端命令：新增 `agency_list_activities`（`run_id` -> 按 `id ASC` 返回活动日志，limit 200）
- [x] 前端轮询：`AgencyStudio.tsx` 新增 3s 轮询 `useQuery`，DB 活动事件为主源，live store 事件补充轮询间隔内新事件（按业务键去重）
- [x] live 事件监听保留：`App.tsx` 事件监听 + `agencyActivityStore` 不变，提供轮询间隔内的即时更新（双保险）
- [x] 验证：`cargo test --lib` 1326 passed / 2 ignored（+1）；`npx vitest run` 455 passed / 3 skipped（无前端测试变更）

### ✨ v0.38.1 - 修复续写伏笔账本多字节中文切片 panic（文思活跃模式） ✅ (2026-08-12)

- [x] 主修复：`foreshadowing_service.rs` title 截取从字节语义（`&content[..30]`）改字符语义（`chars().take(30)`），中文 content 不再 panic
- [x] 同类预防：`post_process.rs` 两处 `&draft_content[..8000/6000]` + `intent.rs` `&content[..min(200)]` 改 `floor_char_boundary`
- [x] 回归测试：`service_ledger_title_multibyte_no_panic` 用报错原文验证 `get_ledger` 不 panic

### ✨ v0.38.0 - 代理工作室实时显示修复与三 Agent 完善 ✅ (2026-08-12)

- [x] 实时显示修复：agency 事件监听提升到常驻 `App.tsx` 顶层 + 新增全局 `agencyActivityStore`（cap 200，单例无 persist），页面未开不再丢实时动态，打开即见；跨故事切换时 activeRunId 按 storyId 校正
- [x] 三 Agent（主创/管理/编辑审计）事件信号补齐：概念/资产/首章/资产补齐/装配 start/done 全路径配对（含 legacy 与快速路径单点覆盖）；修复 legacy 概念完成信号角色标注（LeadWriter→Producer）；后台质检黑板写入实时推 `agency-board-changed`
- [x] 前端打磨：幕前动词映射补全（概念/装配/资产补齐/资产回流/第N章草稿/审查第N章等）；幕后时间线去重改业务键，同源重复事件不再显示两次
- [x] 续写熔断不丢稿：行为已由 65d90b5（v0.30.30）实现（≥600 字符降级放行/<600 丢稿），本档补齐流程级测试
- [x] 验证：`cargo test --lib` 1306 passed / 2 ignored（+5）；`npx vitest run` 421 passed / 3 skipped（+17）

### ✨ v0.37.0 - 资产回流：后台资产 agent 对已生成正文生效 ✅ (2026-08-11)

- [x] 提取 prompt 写作级升级（`memory_content_analysis.md`）：角色情感画像 / 双向情感关系 / 世界观增量 / 场景大纲 / 故事增量，字段与 schema 严格对齐
- [x] 新增资产桥 `memory/asset_bridge.rs`：提取结果 upsert 进生产资产表（characters / character_relationships / world_buildings / scenes.outline_content / story_outlines），新角色自动注册；源感知合并——只精炼机器来源，手工编辑永不覆盖
- [x] Agency 续写路径接入：每章正文落库后后台自动跑提取（`spawn_asset_ingest`，含 KG 持久化）；orchestrator/TriShot 路径经 `run_ingest` 自动生效
- [x] 并发安全：per-story 进程内锁 + `BACKGROUND_LLM_SEMAPHORE` 后台串行化；失败不致命，绝不影响正文落库
- [x] 验证：`cargo test --lib` 1301 passed / 2 ignored（+14）；`npx vitest run` 404 passed / 3 skipped（未动）

### ✨ v0.30.46-48 - 创世持久化链路审计修复 + issue #13/#14/#15 批量修复 ✅ (2026-07-31)

- [x] v0.30.46 创世正文未即时保存与资产缺失：前端创世后补偿 flushSceneSave；场景装配 create+update 单事务 + 空正文校验；outline 写黑板身份 Producer→LeadWriter；创世成功臂回读校验；空串 content 防覆盖；materialize 新增 foreshadowing 落库 + item_type 别名归一化 + characters upsert
- [x] v0.30.47 角色谱静默失败（issue #14）：角色谱/文风/首场景改 `extract_and_sanitize_json` 健壮解析 + 去 unwrap + warn 日志；`llm/service.rs` `prompt[..200]` 字节切 UTF-8 panic（llm_calls 永不落库根因）改 `chars().take(200)`；向导三卡片 `isGenerating` 防重入；拆书页 4 处 toast 改 `extractMessage`（issue #13）
- [x] v0.30.48 向导策略加载误报失败 + 快速创作空输入无确认（issue #15）
- [x] 验证：`cargo test --lib` 1098 passed / 2 ignored；`npx vitest run` 352 passed / 3 skipped；tsc/fmt 全绿

### ✨ v0.30.45 - 修复文思活跃模式续写提示词泄露（LLM 思维链泄露到正文）✅ (2026-07-31)

- [x] 根因（四层叠加）：①`llm/openai.rs` `resolve_content`（v0.30.25 引入，当时为修复 DeepSeek 推理模型空 content 静默失败而加的 `reasoning_content` fallback）在 `content` 为空时错误回退到 `reasoning_content`（CoT），把思维链当正文返回；②`max_tokens: 2048` 对推理模型过小，CoT 耗尽全部 token 预算导致 `content` 恒为空、整段被 CoT 占据；③`sanitize_novel_output`（v0.23.36 引入）仅清洗 markdown/元评论/小节标题，无法识别裸 CoT 思维链；④writer 提示词从未显式禁止推理输出
- [x] Fix 1（`llm/openai.rs` `resolve_content`）：移除 `reasoning_content` 回退，`content` 为空即返回空，不再用 CoT 兜底（v0.30.25 的 fallback 本为避免"空 content 静默失败"，但推理模型空 content 多因 CoT 耗尽预算，fallback 反而把 CoT 当正文，弊大于利）
- [x] Fix 2：`max_tokens` 2048 -> 4096，给推理模型留足正文预算
- [x] Fix 3：新增 `detect_and_strip_bare_cot`（检测 ≥3 条 CoT 信号行触发剥离），接入 `sanitize_novel_output` 后处理
- [x] Fix 4：writer 提示词新增反推理指令（禁止输出思考过程/推理链）
- [x] 验证：`cargo test --lib` 1091 passed / 2 ignored（+4）；`npx vitest run` 352 passed / 3 skipped；`cargo clippy --lib` 539（零新增）；tsc ✅；fmt ✅；architecture_guard ✅；format:check ✅

### ✨ v0.30.44 - 修复文思活跃模式续写报"生成过程异常结束，未收到有效内容" ✅ (2026-07-29)

- [x] 根因：`smartExecuteInFlightRef.current = false` 在 smartExecute resolve 后、内容处理前被提前清除--后台活动同步回调（100ms 防抖）在内容处理期间把 `isGenerating` 置 false，触发安全网 effect（`!isGenerating && smartExecuteNeedDiagnosticRef.current`）误报"生成过程异常结束"；`handleRequestGeneration` 活跃模式分支错误地走了打字机幽灵文本（3 字符/帧）而非直接 `appendAiContent` 追加到编辑器正文
- [x] Fix 1（`FrontstageApp.tsx` `handleRequestGeneration`）：移除 smartExecute resolve 后的 `smartExecuteInFlightRef.current = false`；改为在各退出路径（打字机完成 / displayText 空 bail / background bootstrap / genesis 首章 / aborted / active mode 追加后）统一清除
- [x] Fix 2（`FrontstageApp.tsx` `handleSmartGeneration`）：移除 smartExecute resolve 后的 `smartExecuteInFlightRef.current = false`；在各内容交付路径（aborted / isAlreadyPresent / isBootstrapCompleted&&delivered / active mode append / isFirstChapterReady / ghost text）统一清除 `smartExecuteInFlightRef` + `smartExecuteNeedDiagnosticRef`；`finally` 块兜底清除 flight 标志防泄漏
- [x] Fix 3（`FrontstageApp.tsx` `handleRequestGeneration`）：活跃模式分支在打字机之前直接 `appendAiContent(displayText, 'auto')` + 清除两标志，绕过打字机（与 `handleSmartGeneration` 活跃模式行为一致）
- [x] 回归测试（`FrontstageApp.wensi-active.test.tsx`）：+2 测试（活跃模式直追 + 不触发误报诊断）；RichTextEditor mock 修复 `getHTML()` 返回 stale props.content 问题
- [x] 验证：`npx vitest run` 352 passed / 3 skipped（+2）；`tsc`/`fmt`/`clippy`（538 零新增）/`architecture_guard`/`format:check` 全绿。纯前端修复，无 Rust 变更

### ✨ v0.30.43 - 修复续写内容丢失根因：flushSceneSave 读取滞后的 latestContentRef + onChapterUpdated 覆写未保存内容 ✅ (2026-07-30)

- [x] 根因：`flushSceneSave` 读取 `latestContentRef`（RichTextEditor 200ms HTML 防抖可能滞后 200ms）而非编辑器实际 HTML，关闭/切章时最后 200ms 输入丢失；`onChapterUpdated` 用 DB 旧内容覆写编辑器但不更新 `latestContentRef`，用户未保存输入不可逆丢失
- [x] Fix 1（`FrontstageApp.tsx` `flushSceneSave`）：改为直接读 `editorRef.current?.getHTML()`（编辑器实际 HTML），回退 `latestContentRef.current`，读后回写 `latestContentRef` 保持一致。覆盖关闭前 flush / 章节切换 / AI 追加 / 修稿全部 flush 路径
- [x] Fix 2（`FrontstageApp.tsx` `onChapterUpdated`）：`setContent` 前新增守卫--`latestContentRef` 非空且与 DB 内容不同时跳过（用户有未落库输入）；`setContent` 后补 `latestContentRef.current = formatted` 同步
- [x] Fix 3（`FrontstageApp.tsx`）：无章节时 `setContent('')` 后补 `latestContentRef.current = ''`
- [x] 验证：`cargo test --lib` 1087 passed（无 Rust 变更）；`npx vitest run` 350 passed / 3 skipped（+1）；`tsc`/`fmt`/`clippy`（538 零新增）/`architecture_guard`/`format:check` 全绿

### ✨ v0.30.42 - 修复世界观生成失败（LLM 返回 markdown 代码块包裹的 JSON + 未转义引号 + 静默失败 + prompt 字段名不匹配）✅ (2026-07-30)

- [x] 根因（issue #14）：用户报告"世界观生成失败，请重试"，但日志显示 LLM API 调用成功返回内容（7636 字符），失败发生在下游 JSON 解析且完全无错误日志。模型将 JSON 包裹在 ` ```json ... ``` ` 代码块中、或在字符串值内直接换行/使用裸双引号，`serde_json::from_str` 静默失败；`novel_creation.rs` 严格解析全量响应（含围栏）直接失败；prompt 要求"concepts 数组"但代码读 `world_buildings`
- [x] Fix 1（`agency/coordinator.rs` `parse_lenient`）：复用 `crate::narrative::extract_and_sanitize_json`（剥离 markdown 围栏 / 推理链、括号深度匹配跳过尾部杂散 `}`、修复字符串内未转义换行、移除 BOM/注释/尾随逗号），失败回退旧首尾花括号截取。覆盖 agency 全部 JSON 解析路径
- [x] Fix 2（`agents/novel_creation.rs`）：提取 `parse_world_options_response` 纯函数先剥离围栏再解析；失败时 `log::warn!` 记录错误 + 200 字片段（此前完全静默）；元素反序列化 `unwrap` 改 `map_err` 不再 panic
- [x] Fix 3（prompt）：`novel_creation_world_options.md` "concepts" -> `world_buildings`（与代码一致）+ 补全 schema；两份 prompt 新增格式约束（禁 markdown 围栏、引号转义、JSON 外无文字）
- [x] 验证：cargo test --lib 1087 passed / 2 ignored（+5）；tsc ✅；vitest 349 passed / 3 skipped；fmt / clippy（538 零新增）/ architecture_guard / format:check 全绿

### ✨ v0.30.41 - 修复续写内容被假阳性去重静默丢弃（模型回显指令 + 短文本假阳性 + 内容丢失）✅ (2026-07-30)

- [x] 根因：续写生成时 LLM（deepseek-v4）成功返回 2511 字符，但前端仅显示 6 字符（"续写\n黑暗。"）并报"生成过程异常结束，未收到有效内容"。根因链：①模型在生成内容开头回显用户指令"续写"（非正文）；②打字机动画首帧仅 3 字符（"续写\n"），归一化后 2 字符"续写"几乎必然出现在 9656 字已有正文中；③`isTextDuplicate` 假阳性返回 true，`setGeneratedText` 跳过赋值并 `markAccepted` 存入 2 字符指纹；④生成内容被静默丢弃
- [x] Fix 1（`textCleanup.ts` `isTextDuplicate`）：归一化后 < 30 字符的生成文本直接返回 false，不进行去重检查
- [x] Fix 2（`textCleanup.ts` + `FrontstageApp.tsx` `stripInstructionEcho`）：新增 `stripInstructionEcho(generated, userInput)` 剥离模型回显的用户指令前缀，在 `handleRequestGeneration` 和 `handleSmartGeneration` 的 `sanitizeContinuationOutput` 后调用
- [x] 验证：vitest 349 passed / 3 skipped（+13）；tsc ✅；format:check ✅；architecture_guard ✅。纯前端修复，无 Rust 变更（cargo 基线 1082 不变）

### ✨ v0.30.40 - 修复代理工作室不显示活动记录数据（activeRunId 仅从事件捕获 + 无 list_runs 命令）✅ (2026-07-29)

- [x] 根因：`AgencyStudio.tsx` 的 `activeRunId` 仅从实时事件捕获，页面后开时恒 null -> IPC 查询不触发 -> 永远"暂无活动"；无 `list_runs` 命令发现已有 run；activity 事件 fire-and-forget 不持久化
- [x] 后端：新增 `agency_list_runs` 命令（`list_runs_for_story` 按 `created_at DESC`，limit=20）
- [x] 前端：`useQuery` + `useEffect` 水合 `activeRunId`（取最新 run，不依赖实时事件）
- [x] 前端：历史时间线三源合并（live 事件 + board items 重建 + run 生命周期）
- [x] 前端：run 选择器 `<select>` 下拉框切换浏览历史 run
- [x] 验证：cargo test 1082 passed（+1）；vitest 339/3 skipped（+3）；clippy 538（baseline 540，-2 修复既有）；tsc ✅；fmt ✅；architecture_guard ✅；format:check ✅

### ✨ v0.30.39 - 修复续写不按故事大纲推进剧情（TimeSliced 路径缺失 build_progression_anchor）✅ (2026-07-29)

- [x] 根因：v0.30.31 引入的 `build_progression_anchor`（注入故事大纲硬约束 + 已推进进度指针 + 世界观规则 + 显式调和指令）只在 TriShot 路径调用，从未移植到 TimeSliced 路径，而 TimeSliced 是默认续写路径（`generation_mode = "auto"` 路由续写到 TimeSliced）-> writer 有完整大纲但无进度指针，无法判断当前在故事大纲哪个节点 -> 偏离大纲、原地踏步、仅复述设定
- [x] Fix（`agents/orchestrator.rs` `execute_time_sliced`）：在 prompt 模板渲染后、`ending_anchor` 注入前插入 `build_progression_anchor` 调用，与 TriShot 路径完全对齐
- [x] 验证：cargo test 1081 passed；tsc ✅；vitest 336/3 skipped；fmt ✅；clippy 539（零新增）；architecture_guard ✅；format:check ✅

### ✨ v0.30.38 - 修复续写输出被编辑器元评论污染（is_prose_request 被 serde 默认 false）✅ (2026-07-30)

- [x] 根因：分类提示词"继续写"示例省略 `is_prose`，LLM 若遵循示例返回合法 JSON 但缺该字段，serde `#[serde(default)]` 填 `is_prose_request=false`；serde 默认值（false）与 LLM 失败兜底值（true）相反，partial-but-valid JSON 被缓存后持续返回毒化 false；`sanitize_plan_for_prose_request` 门控仅检查 `is_prose_request`，false 时跳过全部净化 -> SING 多步计划 `[writer, inspector, builtin.style_enhancer]` 未拦截 -> style_enhancer 元评论覆盖 writer 正文
- [x] Fix 1（`intent.rs` `parse_classification_json`）：后置不变量--续写/创世缺 `is_prose` 时强制设 `true`
- [x] Fix 2（`intent.rs` `build_classification_prompt`）："继续写"示例补 `is_prose=true`
- [x] Fix 3（`planner/mod.rs` `sanitize_plan_for_prose_request`）：门控扩展为 `is_prose_request || is_continuation`（纵深防御）
- [x] 验证：cargo test 1081 passed（+4）；tsc ✅；vitest 336/3 skipped；fmt ✅；clippy 539（零新增）；architecture_guard ✅；format:check ✅

### ✨ v0.30.37 - 修复创作生成失败时 toast 显示 "[object Object]"（issue #12）✅ (2026-07-29)

- [x] 根因：后端 `AppError` 序列化为普通对象 `{ code, message, severity }`，Tauri v2.4 作为普通对象（非 `Error` 实例）投递到前端 catch 块；`String(err)` / `instanceof Error ? .message : String(err)` 对普通对象产出 `[object Object]`，可读 `message` 被丢弃。v0.30.31（issue #11）的 `extractMessage` 只覆盖"获取模型列表"，创作/生成路径未迁移
- [x] 修复（10 个前端文件，36 处）：`FrontstageApp`/`SceneEditor`/`Stories`/`RichTextEditor`/`WenSiPanel`/`usePipeline`/`CharacterStatePanel`/`Skills`/`PromptsPanel`/`useUpdater` 所有创作/生成错误路径统一改用 `extractMessage(err)`
- [x] 新增 `src/utils/__tests__/errorHandler.test.ts`（+8）：AppError 普通对象 / 内嵌 JSON / 普通 Error / 字符串 / 兜底文案
- [x] 验证：tsc ✅；vitest 336 passed / 3 skipped（+8）；format:check ✅；architecture_guard ✅。纯前端，cargo 基线 1077 不变

### ✨ v0.30.36 - 修复首次创世指令不保存到输入历史（按↑调取不到）✅ (2026-07-29)

- [x] 根因：首次创世（无已有故事）时 `currentStory=null`，`handleInputSubmit` 的 `if (sid) saveInputHistory(...)` 跳过保存；isBootstrap 分支 `setCurrentStory(null)` 清空历史，新故事历史始终为空
- [x] 修复（`FrontstageApp.tsx`）：`handleSmartGeneration` 的 `story_created` 处理块在 `setCurrentStory(新故事)` 后同步 `saveInputHistory(新故事ID, [创世指令, ...])`，useEffect 随后加载即可读到
- [x] v0.30.23 修复意图分类后创世指令正确走 isBootstrap 路径，暴露了此前被续写误分类掩盖的缺陷
- [x] 验证：tsc ✅；vitest 328 passed / 3 skipped（+2）；format:check ✅；architecture_guard ✅。纯前端，cargo 基线 1077 不变

### ✨ v0.30.35 - editor 质检后台异步化：首章立即显示 + 后台质检 + toast 反馈 ✅ (2026-07-29)

- [x] 根因：editor 质检（`review_and_assemble` 的 `evaluate_gate`）在 Scene 装配前同步执行，被 600s 硬超时包裹；producer+writer 花约9分钟后 editor 仅剩约1分钟，其 300s LLM 调用被硬 600s 砍掉，整 run 超时无首章
- [x] 新增 `assemble_only`（pub(crate)）：从 `review_and_assemble` 提取纯装配（抗重复三件套 + Scene 落库），不含 editor 质检
- [x] 新增 `spawn_editor_qc`：后台 `tokio::spawn` 用独立 300s deadline 调 `evaluate_gate_impl`（不受 600s 限制）；三态结果 emit `genesis-qc-result`；`app_handle=None`（测试）时 no-op
- [x] `genesis_fastpath` / `run_genesis_legacy_inner` Phase C 改为 `assemble_only` + `spawn_editor_qc`，返回 `verdict:EditorVerdict::pending()`
- [x] 删除无用 `review_and_assemble`；`EditorVerdict` 加 `pending()`；新增 `EVENT_GENESIS_QC_RESULT`
- [x] 前端 `genesis-qc-result` 监听 + 三态 toast（通过/降级放行/不合格建议重新创世）；后台不影响 `isGenerating`
- [x] producer 深度资产保持前台（单次 `complete_json` ~30-60s，非瓶颈，保障首章不脱节）
- [x] 验证：`cargo test --lib` 1077 passed（+2）；tsc / vitest（326/3 skipped）/ fmt / clippy（539 零新增）/ architecture_guard / prettier 全绿

### ✨ v0.30.34 - 序列化场景持久化 + 修稿 bypass 修复 + 关闭超时提升 ✅ (2026-07-29)

- [x] 序列化 `persistSceneContent`：Promise 链确保所有 `update_scene` 串行提交，消除并发覆写竞态（文思活跃连续续写时较早的小内容覆写较晚的大内容）
- [x] `handleContentChange` saveFn / 保护性保存统一走 `persistSceneContent`
- [x] 关闭超时 3s -> 6s（超过 SQLite `busy_timeout` 5s）
- [x] `handlePipelineRefine` `setContent` bypass：补 `latestContentRef` 同步 + `flushSceneSave`
- [x] `onReviseResult` `insertText` bypass：补 `latestContentRef` 同步 + `flushSceneSave`

### ✨ v0.30.33 - 修复关闭应用时续写内容丢失 ✅ (2026-07-28)

- [x] 关闭前 flush 协调：后端 `CloseRequested` -> `api.prevent_close()` + emit `frontstage-flush-requested` + 3s 超时兜底；前端监听 -> 立即 `update_scene` 落库 -> `invoke('graceful_quit')` 优雅关闭（WAL checkpoint）；`graceful_shutdown` 加 `AtomicBool` 幂等守卫
- [x] AI 追加立即落库：`appendAiContent` 的 `scheduleAutoSave(..., 2000)` 替换为 `void flushSceneSave()`，消除文思活跃连续续写防抖永不出火的丢失窗口
- [x] 章节切换前 flush：`selectChapter` 的 `cancelAutoSave()` 替换为 `void flushSceneSaveRef.current()`，切换前落库当前场景
- [x] 提取 `flushSceneSave` 共享落库逻辑

### ✨ v0.30.32 - 增强性指令纳入世界观/故事大纲/场景大纲/上下文强关联 ✅ (2026-07-28)

- [x] P0-A：增强生成纳入世界观--`build_logline_context_sync` 拉 `world_buildings`（concept+rules前3+history）为 `world_setting`；`agency_logline_suffix_contextual.md` 新增世界观段 + 输出要求"后缀须与世界观规则一致"
- [x] P0-B：TriShot `build_progression_anchor` 加 `user_instruction` 参数，指令作为首个段注入 + 显式调和（资产=硬约束，指令=创作方向，在硬约束内落实指令核心意图，冲突时调整指令以符合约束但保留核心意图）
- [x] P1-C：创世 `writer_first_chapter`/`writer_prose_fallback` 指令-资产调和
- [x] P1-D：TimeSliced `orchestrator_timesliced_writer.md` + fallback 指令-资产调和
- [x] 验证：`cargo test --lib` 1078 passed（+1）；tsc / vitest（322/3 skipped）/ fmt / clippy（baseline 540 零新增）/ architecture_guard / prettier 全绿

### ✨ v0.30.31 - 续写链路修复：世界观/故事大纲/场景大纲注入与剧情推进方向 ✅ (2026-07-28)

- [x] P0-A：Legacy TriShot 确定性注入--`build_progression_anchor` 在 `final_prompt = synthesized_prompt` 后注入【剧情推进方向】段（故事大纲+场景大纲+已推进进度+世界观+推进约束）；WriteTimeBundle 新增 `world_setting` 字段读 world_buildings 表；manifest 增加 story_outline/world_setting 清单项
- [x] P0-B：writer_system/timesliced_writer/trishot_synthesizer prompt 加"剧情必须推进到下一节点，不得原地踏步"
- [x] P0-C：scene_outline.md 修"按序号定位节点"伪前提为"按已推进进度定位"；Legacy（`creation_commands.rs`/`service.rs`）与 Agency（`generate_chapter_outline`）双路径注入 world + progress
- [x] P1-A：Agency `build_continue_writer_context` 世界观全字段 + 前文保底（阈值倒挂修复 >8000->>12000）+ 进度指针；`write_chapter` 三分支加推进约束
- [x] P1-C：`ensure_world_building` concept 存全文 + rules 解析落库；history 不再冗余存储
- [x] P1-D：`evaluate_gate_impl` editor 预注入参照资产（世界观红线+世界观设定+故事大纲）
- [x] 验证：`cargo test --lib` 1077 passed（+2）；tsc / vitest（322/3 skipped）/ fmt / clippy（baseline 540 零新增）/ architecture_guard / prettier 全绿

### ✨ v0.30.30 - Agency 创作链路结构性优化：抗重复闭环 + 质量门宽松度 + 熔断不丢稿 ✅ (2026-07-28)

- [x] D1：抗重复提示词补齐（lead_writer/editor_auditor 系统提示词 + 内联 writer prompts）+ 创世装配接入清理三件套（抽取 `cleanup_prose_for_persist` 共享 helper，创世 `review_and_assemble` 与续写 `handle_gate` 共用）
- [x] D2：失效 prompt_id 核查（占位 ID by-design 回退 `default_role_prompt`，仅加注释）
- [x] E1：质量门 scoreless pass 兜底 0.85 -> 0.7（editor 不给数值分时不再单凭 model 项过门）
- [x] E2：editor 连累整 run 修复（`salvage_failed_gate` helper：substantive 草稿 ≥600 字符降级放行保产出，4 个 Failed arm 不再直接丢稿）
- [x] E3：writer 熔断丢稿修复（MaxTurns/Deadline 先 `latest_draft`/`latest_draft_by_key` 取黑板已产出草稿，取不到才散文回退）
- [x] 验证：`cargo test --lib` 1069 passed（+4）；tsc / vitest（322/3 skipped）/ fmt / clippy（baseline 540 零新增）/ architecture_guard / prettier 全绿

### ✨ v0.30.26 - 统一 Logline 增强提示为内联幽灵文本 + 修复分时预检缺少角色 ✅ (2026-07-27)

- [x] 将独立 `.frontstage-logline-hint` 建议条改为输入框内跟在已输入内容后的幽灵后缀
- [x] 新增 `resources/prompts/agency/agency_logline_suffix.md` prompt 资产
- [x] 按 `→` 追加后缀，Enter 提交“原输入 + 增强后缀”组合文本
- [x] 简化 `FrontstageApp`：移除 `originalInputForLoglineRef` 与 `intentClassificationInput` 透传
- [x] 修复分时预检缺少角色：意图分类兜底按输入文本判断创世意图；`QuickPreflightChecker` 自动创建占位主角；前端用原输入做意图分类
- [x] 验证：`cargo test -p storymoss` 1060 passed；`npx vitest run` 310 passed / 3 skipped

### ✨ v0.30.23 - 意图分类 Bug 修复 ✅ (2026-07-23)

- [x] 提示词去偏：移除 `已有故事=true` 上下文注入（偏差来源）+ 移除 `仅当` 保守措辞 + 新增 3 个正例（"写一部科幻小说" -> is_new_novel=true）。
- [x] 上下文感知兜底：新增 `conservative_fallback_with_context(has_existing_story)`--LLM 失败时无故事返回创世，有故事返回续写。原 `conservative_fallback()` 标记 `#[deprecated]`。
- [x] 不缓存失败：仅 LLM 成功解析的结果写入缓存，兜底结果不缓存。缓存键简化为仅 `user_input`。
- [x] 前端兜底上下文化：catch 块和 null 防御用 `stories.length === 0` 替代硬编码 `is_new_novel: false`。
- [x] 设计原则：LLM 是意图判断的唯一权威；不回到硬编码关键词匹配；不用 `|| !has_existing_story` 覆盖 LLM 结果。
- [x] 验证：`cargo test --lib` 978 passed（+4）；`npx vitest run` 307 passed；clippy baseline 550 无新增告警。

### ✨ v0.30.22 - PROBLEM 七元素框架集成 ✅ (2026-07-22)

- [x] 新增 Erik Bork PROBLEM 七元素（Punishing/Relatable/Original/Believable/Life-Altering/Entertaining/Meaningful）作为后端创作资产；新增提示词 `agency_problem_logline.md` / `agency_problem_outline.md`。
- [x] DB V114 迁移新增 `stories.logline` 列，Story 模型与 StoryRepository 同步。
- [x] `generate_logline`：简单 premise（< 100 字符）触发 logline 生成并替换原 premise。
- [x] `ensure_story_outline`：从注册表加载 PROBLEM outline 提示词并注入 logline 上下文；`producer_depth_assets` outline 字段注入 PROBLEM 指导；`build_continue_writer_context` 以【故事Logline】注入。
- [x] 验证：`cargo test --lib` 974 passed（+3 logline 测试）；clippy baseline 550 无新增告警。

### ✨ v0.30.21 - 续写资产层级生成 ✅ (2026-07-22)

- [x] 续写 `ensure_assets` 扩展：角色检查后追加 world_buildings / story_outlines 检查，缺失时调 `ensure_world_building` / `ensure_story_outline` 单次 Producer LLM 调用生成并落库（不抢主创 LLM）。`build_continue_writer_context` 注入故事大纲。`generate_chapter_outline` 在 writer tool_loop 前生成章节大纲（服从故事大纲），strict writer task 含故事大纲 + 本章大纲 + 写作要求。`handle_gate` 存 `scenes.outline_content`。形成"世界观 -> 故事大纲 -> 章节大纲 -> 正文"层级约束链。`cargo test --lib` 971 passed。

### 🐛 v0.30.20 - 修复质量门编辑审计 Agent 熔断 ✅ (2026-07-23)

- [x] 修复 Agency 质量门 editor_auditor tool_loop 在本地模型不遵从 JSON action 时连续解析失败/达最大轮数熔断 -> 原直接 Failed 导致整 run 失败。Fix：①salvage（熔断时仍 `parse_lenient` 提取末轮裁决 JSON）；②散文回退（`editor_verdict_prose_fallback` 单次 `llm.complete()` 直接请求裁决 JSON，与 `writer_prose_fallback` 同理）。`cargo test --lib` 965 passed。

### 🐛 v0.30.18 - 修复幕前意图分类 null 崩溃 ✅ (2026-07-23)

- [x] 修复 `handleSmartGeneration` 中 `classifyIntent` resolve 为 null 时不抛异常导致 `null.is_new_novel` 崩溃（v0.30.16 CI E2E PAGEERROR 根因，连带 6 个 E2E 失败）。E2E mock 对未注册命令返回 null 触发。Fix：catch 后新增 post-catch null 兜底（续写语义）+ 不缓存 null。附带：v0.30.16 tag macOS 构建 Info.plist Io(code 5) 为 runner 瞬时 I/O flake，已 rerun 重建。

### ✨ v0.30.17 - 幕前顶部创世状态显示三 Agent 动作/进度 ✅ (2026-07-23)

- [x] 幕前顶部创世流程状态改进：新增 `useAgencyAgentActivity` hook 订阅后端 `agency-agent-activity` 事件（此前仅幕后 AgencyStudio 消费），FrontstageHeader 顶部状态栏渲染 主创/管理/编辑审计 三 Agent 的动作与进度（进行中「主创正在写第一章」、已完成「管理已完成深度资产」，进行中琥珀/已完成绿色），run 结束自动清空。底部 LLM 连接状态未改。附带：AGENTS.md 强制构建规则 #2 改为「本地构建仅在用户明确要求时执行」。

### ✨ v0.30.16 - 故事资产手动编辑（补齐编辑缺口）✅ (2026-07-22)

- [x] 后台故事资产手动编辑：故事大纲（Stories.tsx 查看/编辑 UI）、故事摘要（KnowledgeGraph.tsx SummaryCard 编辑）、伏笔内容编辑+删除（后端 update/delete 方法+命令+注册 + hook + UI）、角色关系编辑（hook + RelationshipCard 编辑表单）。角色/世界构建/场景已有完整编辑。

### 🐛 v0.30.15 - 场景围绕故事大纲生成（创作原则加固）✅ (2026-07-22)

- [x] 创作原则加固：有故事大纲时场景必须围绕大纲展开。根因 A：场景大纲生成 `generate_scene_outline` 复用故事级 outline_planner 提示词且不注入 story_outlines.content，幻觉新角色"金敏秀"；根因 B：writer（TimeSliced/TriShot）prompt 从不包含故事大纲。Fix A：新增场景级提示词 scene_outline.md（强制复用已登场角色、禁止发明新角色、围绕故事大纲节点）+ generate_scene_outline 注入故事大纲 + build_outline_prompt 分流；Fix B：WriteTimeBundle 加 story_outline 字段 + to_prompt 红线后插入权威段，一处覆盖两条 writer 路径。

### 🐛 v0.30.14 - 续写返回风格增强模板修复（多步 plan 尾部非 writer 覆盖正文）✅ (2026-07-22)

- [x] 修复续写返回风格增强模板（第 5 次复发）：`execute_plan` 用最后产出 content 的步骤作为 final_content，force-correction 只修正首步无法拦截尾部 style_enhancer/inspector；新增防线 3 `sanitize_plan_for_prose_request` 在咽喉点对所有 is_prose_request plan 净化（移除非 prose 技能 / 续写塌缩单 writer / 弹出尾部非 writer 保证末步 writer / 空则补 writer），保留 [inspector, writer] Rule 9 流，非 prose（Audit）不净化。

### 🐛 v0.30.13 - 续写返回风格增强模板修复（SING 路径绕过 force-correction）✅ (2026-07-22)

- [x] 修复续写返回风格增强模板：SING（IntentionGraphPlanner）路径直接返回 plan 绕过 `PlanGenerator::generate_plan` 内的 force-correction，续写被 SING 路由到 `builtin.style_enhancer` 返回空内容模板；提取 `force_correct_first_step_to_writer` 在 plan 执行咽喉点（`execute_with_context`）统一施加，覆盖 SING/PlanGenerator/fallback 所有来源。

### 🐛 v0.30.12 - 续写返回审查报告修复（force-correction 漏拦 inspector）✅ (2026-07-22)

- [x] 修复续写返回审查报告：force-correction 漏拦 inspector（planner 强制改 writer 列表漏 inspector，本地模型 Gemma 把续写误路由到 inspector；提取 `PlanGenerator::should_force_correct_to_writer` 纯函数按 LLM 分类分流，Rule 9/21 澄清续写≠refine 并禁用 inspector）。

### 🧠 v0.30.11 - LLM 意图分类器替换朴素子串匹配 ✅ (2026-07-20)

- [x] **IntentParser::classify_writing_intent**：单次 LLM 调用产出全部路由决策（is_new_novel / is_continuation / task_type / is_prose_request / input_clarity / detected_genre / confidence），8s 超时 + 保守回退（is_new_novel=false=续写）+ 会话 LRU 缓存。
- [x] **修复 6 处高危路由点**：is_novel_creation_intent 子串误判 / find_template 被 disabled 误禁 / from_instruction_and_context 优先级 bug / force-correction 扩展 / extract_genre 否定句漏判 / intention_graph builder。
- [x] **前端**：新增 classifyIntent API，删除 isNovelCreationIntent / isContinuationIntent；修复字段名别名 bug（提示词 is_prose 与结构体 is_prose_request 不一致）。
- [x] 验证：`cargo test --lib` 936 passed；`npx vitest run` 305 passed；tsc / fmt / clippy / architecture_guard 全绿。

### 🐛 v0.30.10 - 续写返回风格增强模板修复 ✅ (2026-07-20)

- [x] **Fix A（executor.rs）**：续写意图词检测跳过模板匹配，强制走 planner LLM 路径。
- [x] **Fix B（mod.rs）**：force-correction 扩展到 style_mimic/plot_analyzer/builtin.style_enhancer，prose 关键词触发强制改 writer。
- [x] **Fix C（executor.rs）**：`inject_content_fallback` 为 style_mimic/plot_analyzer/builtin 在 content 空时注入文本。
- [x] **Fix D（mod.rs）**：Rule 21 新增续写关键词，禁止 style_enhancer 用于 prose 请求。
- [x] 验证：`cargo test --lib` 929 passed（+5）；fmt / clippy 无新增告警。

### 🐛 v0.30.9 - 续写返回 Inspector 审查模板修复 ✅ (2026-07-20)

- [x] **Fix A（executor.rs）**：inspector draft 兜底注入--当 `capability_id == "inspector"` 且 `draft` 为空时，按 `depends_on` 顺序查找 writer 步骤的 `step_outputs["content"]`，找不到则扫描全部 `step_outputs`，自动注入非空 content 作为 `draft`。
- [x] **Fix B（mod.rs）**：planner 提示词 Rule 9 强化--inspector 必须使用 `"draft": "{{step_id}}"` 传参；JSON 示例增加 inspector 步骤示范。
- [x] 验证：`cargo test --lib` 924 passed（+5：inspector draft 兜底注入 5 场景）；fmt / clippy 无新增告警。

### 🤝 v0.30.4 - 幕前输入历史持久化 ✅ (2026-07-20)

- [x] 幕前底部输入框已输入内容按故事隔离持久化到 localStorage（最近 20 条），关闭窗口/重启后不丢失，↑/↓ 浏览历史、-> 确认填充。
- [x] 保留既有 ghost-hint UX（LLM 建议 <-> 历史记录切换），持久化对导航无侵入；localStorage 不可用时静默降级为内存态。
- [x] 验证：vitest 297 passed（+2）；tsc / prettier 通过。

### 🤝 v0.30.0 — Agency P5：持续学习 + 代理可视化 ✅ (2026-07-19)

- [x] 持续学习双轨：观察层（observations.jsonl，10MB 轮转、防自观察）→ 后台 analyzer（Background 档）→ instinct（trigger/action/confidence 文件层）。
- [x] 置信度引擎：按证据初始化 + 采纳 +0.05 / 纠正 −0.1 / 周衰减 −0.02 / prune（promoted 晋升产物豁免衰减与清理）。
- [x] 晋升管线：confidence ≥0.8 且跨 story 复现 → 学习中心确认 → 物化为 skill.yaml 目录技能（重启自动 reload）。
- [x] 学习中心页（模式列表/置信度/晋升提案/观察流/手动分析）+ 代理工作室页（三角色实时状态卡/黑板视图/活动时间线）。
- [x] eval 场景纳入 CI 专用门禁 step；检查点对比 UI；story 级 token 聚合；rule grader 追读力对齐生产口径。

### 🤝 v0.29.0 — Agency P4：验证循环 ✅ (2026-07-19)

- [x] 四级 grader：code（确定性）→ rule（合同/追读力/规则复检）→ model（rubric 化编辑裁决）→ human（修改率后置信号）。
- [x] Gate v2 加权评分（0.2/0.3/0.5，阈值 0.75）取代二元判定。
- [x] V110 里程碑检查点 + 现在 vs 当时对比（IPC）。
- [x] eval harness：JSON 场景 + pass@k/pass^k + baseline 回归门（随 `cargo test` 纳入 CI）。
- [x] 评估仪表盘页（`agency_eval_overview` 聚合 IPC + 侧栏「创作评估」）。
- [x] migration runner 按最高版本选目；resume 改 spawn 模式。

### 🤝 v0.28.0 — Agency P3：代币优化 + 记忆持久性 ✅ (2026-07-17)

- [x] 角色×任务模型路由：主创 Creative / 管理 Tool / 编辑 Background（经 ModelRole 体系，用户可按角色指派模型）。
- [x] 全局 agency LLM 并发闸门（跨 run 上限 3）+ request_id RAII 注册。
- [x] 上下文注入 token 预算（tiktoken 计数截断）+ 黑板三档目录（catalog/summary/full）+ ToolLoop 会话窗口。
- [x] `agency_sessions` 会话快照（机械提取 + Background 档五段摘要双层）。
- [x] 跨会话恢复 `agency_resume_run`（黑板复制 + stale-replay 防护 + `.storymoss` sessions/ 归档）。
- [x] 同 story 并发 run 原子护栏（部分唯一索引）；创作角色落库去重；质量门判定轮次可追溯；清理 T8 遗留创世专属死代码。

### 🤝 v0.27.0 — Agency 多代理创作框架（创世 2.0）P1+P2 ✅ (2026-07-17)

- [x] 新增 `src-tauri/src/agency/` 模块：黑板协作 + ReAct 工具循环 + 三角色（主创/管理/编辑审计）。
- [x] 质量门（编辑裁决 + 规则复检 + 至多 1 轮修订）；并行稳态循环；按角色并发预算与 run 级 token 预算。
- [x] request_id 定点取消；续写循环 `agency_continue_chapter` / `agency_continue_batch`；创作资产自动落库。
- [x] `smart_execute` 创世路径切换到 agency；旧 GenesisPipeline 移除（TriShot 续写保留）。
- [x] 验证：`cargo test --lib` 834 passed；`npx vitest run` 292 passed；architecture_guard PASSED。

## 🚀 下一步方向

Agency 多代理创作框架 P1–P5 已全部交付（v0.27.0 → v0.30.0），v0.41.0 已把续写收口到三角色同章追加。后续重点：

- **真机验收与学习飞轮调优**：三代理创世/续写的真机盲测；**v0.47.0 设计 §8.2 真机已失败**（《帝国的烟火》2026-08-16 晨，executed）；**须在 v0.48.0 上重跑连续 8 次幕前续写**（需 LLM；CI 仅有 mock），未通过前不得宣称人物丢失/错配、情节混乱、前后文断裂已修复；质量门阈值与 grader 权重校准；持续学习双轨（观察 → instinct → 晋升技能）的置信度参数与晋升质量跟踪。
- **ContextPrioritizer 接到 Agency 热路径**：v0.41.0 以 SceneBeatCard 双锚点为 Critical，分级排序仍仅 Full 路径；长篇续写若再出现 Lost-in-the-Middle 时重评估。
- **characters_present 身份归一**：旧数据 id 与名字混杂，写回/债务匹配可能漏人。
- **本地模型连接超时重试**：keepalive 显示健康时跳过 5s 预探测，死模型仍可能 60s×2 空转（v0.41.2 诊断登记）。
- **续写上下文膨胀（注入层已收口，表内未收口）**：Agency 续写已按拍筛选角色卡/名单/大纲去重/单场前文。`story_outlines` 仍无界追加、跨故事脏角色行不删除（v0.56.0 只 upsert 关系，不合并/删除角色行）；注入层忽略不能替代表内收口。
- **代理可视化深化**：代理工作室 / 学习中心 / 创作评估三页的交互深化（黑板实时性、检查点对比体验、学习飞轮透明度）。
- **云同步与协作**：用户账户、云存储、多设备同步与协作写作增强（沿用下方「后续路线图」方向，暂无具体排期）。

## ✅ v0.26.x 已实施完成

### 📝 v0.26.59 — StoryForge → StoryMoss 品牌收尾，官网落地页上线 ✅ (2026-07-11)

- [x] 完成仓库 tracked 文件 StoryForge → StoryMoss 全局替换。
- [x] GitHub Release 标题更新为 StoryMoss；下次 CI 构建产物名将自动为 StoryMoss。
- [x] `landing/` 官网站点部署至 `https://ai.91z.net`。
- [x] 重写 Hero / ValueProp 产品介绍，加入 Logo。
- [x] 新增平台感知下载按钮，按 OS 自动匹配安装包并直接触发下载。
- [x] 验证：landing `npx vitest run` 19 passed。

### 📝 v0.26.58 — 修复 OpenAI/Deepseek 因 top_p=0 健康检测失败 ✅ (2026-07-09)

- [x] 定位根因：配置中 `top_p: 0.0` 被 OpenAI 兼容 API（含 Deepseek）拒绝。
- [x] `OpenAiAdapter` 序列化前过滤 `top_p`，仅保留 `(0, 1.0]`。
- [x] 新增 `llm::openai` 单元测试。
- [x] 验证：`cargo test --lib` 770 passed。

### 📝 v0.26.57 — 自动划分章节 / 本地导出保存 / 提示词目录 ✅ (2026-07-09)

- [x] 后台设置新增「划分章节方式」：`word_count` 按字数（上限默认 3000 字）、`plot` 按情节。
- [x] 场景保存空闲 30s 后仅对最新章自动切分，避免中间章节改写重排。
- [x] 导出走系统原生保存对话框，文本写 UTF-8，pdf/epub 复制后端临时文件。
- [x] 提示词注册表新增「打开目录」按钮；编辑器改为原生 textarea，避免 CSP 拦截。
- [x] 验证：`cargo test --lib` 769 passed；`npx vitest run` 292 passed；tsc / fmt / format:check 全绿。

### 📝 v0.26.56 — 网关契约测试串行化 ✅ (2026-07-09)

- mock app_data_dir 写 config 测试加锁

### 📝 v0.26.55 — 幕后模型列表开启/关闭 ✅ (2026-07-09)

- 列表页「开启/关闭」；禁用不探测/不调用；活跃自动回退（复用 0.26.54）。

### 📝 v0.26.54 — 修复创作模型被粘性降级绕过 ✅ (2026-07-09)

- [x] 显式角色模型不受粘性 demotion 拦截；Unhealthy resolve 清一次再探
- [x] `set_active_model` / `save_settings` → `clear_model_demotion`
- [x] `generate()` 再提升对齐 `is_promotable_user_model`
- [x] 契约测试：demoted creative / sticky unhealthy / creative X overrides Y

### 📝 v0.26.53 — 故事名取消单击回幕后 ✅ (2026-07-09)

- [x] 故事名移除单击→回幕后；双击改名保留
- [x] 设置按钮为回幕后入口（禅模式也保留）
- [x] Header 测试：单击不调 onOpenBackstage

### 📝 v0.26.52 — 修复模型新增与默认创作模型即时生效 ✅ (2026-07-09)

- [x] `model_config`/`app_settings` 刷新失效 `gateway-status`
- [x] `get_gateway_status` 展示 Unknown；用户显式角色 `is_promotable_user_model`
- [x] `set_active_model(creative)` / `save_settings` 同步 `active_llm_profile`
- [x] `delete_model` 补齐 `emit_data_refresh`

### 📝 v0.26.51 — 幕前故事名与章节名内联改名 ✅ (2026-07-09)

- [x] `displayStoryTitle` / `ensureUntitledStory` / Header 双击改故事名
- [x] `displayChapterTitle` / `EditableChapterTitle` / 顶栏+编辑器上方双击改章节名
- [x] 章节 title 优先 `update_scene`（回写 chapter）

### 📝 v0.26.50 — 修复打字触发后台运行与深度思考假超时 ✅ (2026-07-09)

- [x] AutoIngest 30s 防抖 + BACKGROUND_LLM_SEMAPHORE
- [x] AutoContract 不再 emit contract-auto-progress；前端忽略 running
- [x] backendActivity 不得单独 setIsGenerating(true)
- [x] isGenerating 超时看门狗强制弹诊断

### 📝 v0.26.49 — 修复续写与正文脱节（末句硬锚点） ✅ (2026-07-09)

- [x] `last_n_sentences` + `build_ending_anchor`（末 2 句，最高优先级）
- [x] TriShot Call3 / TimeSliced 在输出纪律之后追加硬锚点
- [x] 契约测试：末句提取、锚点内容、纪律后置序

### 📝 v0.26.48 — 修复自动更新（GitHub Releases） ✅ (2026-07-09)

- [x] `createUpdaterArtifacts: true` + AppImage bundle
- [x] CI 上传 `.sig` / `.app.tar.gz` / AppImage；tag 后校验 `latest.json`
- [x] 下载进度累加；清单 404 可操作错误提示
- [ ] 发布后人工确认旧版客户端能检出并安装本版

### 📝 v0.26.47 — CI 热修复：Rust fmt ✅ (2026-07-09)

- [x] `cargo +nightly fmt` 修复 v0.26.46 rust-check

### 📝 v0.26.46 — 创世方法论全链路 + 题材 match-or-create + 拆书 Phase A/D0 ✅ (2026-07-09)

- [x] P0：5 个 background 模板恢复 `strategy_section` / `quartet_section` + 契约测试
- [x] P1：normalize_methodology_id；Selector 预填；WriteTimeBundle 别名
- [x] P2：Genesis 分步 notes + ContractSeeding 后 methodology_step 推进（雪花→4、HDWB→2）
- [x] EnsureGenreProfileStep match-or-create + 概念题材保真
- [x] 拆书：StoryArc→outline、作者、伏笔落库；chunker 12h/并发止血
- [ ] 拆书 Phase B/C（图表可视化等）— 见审计文档

### 📝 v0.26.45 — Genesis 人物卡强制落地 ✅ (2026-07-09)

- [x] `ProtagonistCard` merge/render/probe（真名 + 欲望/阻力）
- [x] first_scene Critical + TriShot Call3 双重注入
- [x] 与 8% 自重复共享一次额外 Call3 软重试；fail-open
- [ ] 发布后盲测「是谁/要什么/阻力」N≥5

### 📝 v0.26.44 — Genesis 首章质量：开篇骨架与提示词加厚 ✅ (2026-07-09)

- [x] Phase 1：概念提示加厚 + strategy_selector 中文化
- [x] Phase 2：`OpeningSkeletonStep`（≤10s，fail-open + 概念映射降级）
- [x] Phase 3：`infer_narrative_quartet` 接入 Genesis；TriShot 占位用骨架主角
- [x] Phase 4：first_scene 输出纪律单源化；`genesis.opening_skeleton.done` 观测
- [x] USER_GUIDE：创世 30–90s，先骨架后正文
- [ ] Phase 5 A/B 盲测（末世等 5 题材样本）— 发布后对照 `creative_workflow.log`

### 📝 v0.26.43 — 修复底部状态栏 emoji 显示为方框 ✅ (2026-07-09)

- [x] getMajorPhase 纯文案；FrontstageBottomBar 接入 StatusIcon
- [x] 状态解析先剥 emoji 再提取 (Ns)；回归测试

### 📝 v0.26.42 — 修复续写 Tab 提示可见但无幽灵文本 ✅ (2026-07-09)

- [x] 新续写入口 / setGeneratedText 清零 hideGhostUntil
- [x] RichTextEditor 新幽灵到达时清零 postAcceptHideUntilRef（接受中不解除）
- [x] 回归测试：接受后 30s 内新续写须显示幽灵段落

### 📝 v0.26.41 — 记忆统一读模型与 Finalize scene_id 根治 ✅ (2026-07-09)

- [x] V104 drafts.scene_id；run_finalize / SceneEditor / Frontstage 贯穿 scene_id 直写
- [x] V105 story_memory_facts VIEW；V106 memory_items.kg_entity_id；MemoryFacade::list_unified_facts
- [x] get_story_memory_facts IPC + MemoryTab 徽章
- [ ] 物理表 schema 硬合并（明确不做；读模型已统一）
- [ ] 前端孤儿 IPC：`auto_write_cancel` / `auto_revise_cancel` / `get_canonical_state`（已 allowlist）

### 📝 v0.26.40 — 幕后资产闭环 P0–P3 ✅ (2026-07-09)

- [x] P0 侧栏影响徽章 + 合同/KG 文案；诊断组默认折叠
- [x] P1a SceneEditor 收纳 Pipeline 轨（UI 统一；finalize chapter_number 语义另债）
- [x] P1b KG 轻量摘要进 WriteTimeBundle（top-5）
- [x] P1c MCP 降级至设置「扩展」；不进热路径
- [x] P2 MemoryFacade 统一读模型（表不合并）
- [x] quality_gate：**明确永不热路径 LLM**（仅日志 / 未来温路径）
- [x] P3 TracingPanel 资产→prompt 覆盖率
- [x] Schema 合并 kg_entities + memory_items → **读模型统一（v0.26.41）**；物理 DROP 不做
- [x] `run_finalize` chapter_number ↔ scene_id 根治（v0.26.41）
- [ ] 前端孤儿 IPC：`auto_write_cancel` / `auto_revise_cancel` / `get_canonical_state`（已 allowlist）

### 📝 v0.26.39 — 幕后信息架构全面重排 ✅ (2026-07-09)

- [x] 侧栏五组分类 + 中文重命名
- [x] 数据洞察合并（用量/写作/功能使用）
- [x] 设置七 Tab 重组；拆书设置就近；账号死链修复
- [x] SceneEditor vs PipelinePanel UI 统一（v0.26.40）
- [x] KG 记忆与故事合同记忆 **读模型** 统一（v0.26.41）；schema DROP 不做

### 📝 v0.26.38 — 提示词面板与组合智能化 ✅ (2026-07-09)

- [x] 面板 Loading / 打开目录 / 导出修复
- [x] FrameworkSelections methodology + contextual_injectors 回灌 Call 3
- [x] 场景组合预览（preview_prompt_composition）
- [x] quality_gate 策略：永不热路径 LLM（v0.26.40 文档化）
- [ ] 前端孤儿 IPC：`auto_write_cancel` / `auto_revise_cancel` / `get_canonical_state`（已 allowlist）

### 📝 v0.26.37 — 幕前保存与字数 ✅ (2026-07-09)

- [x] 修复 `update_scene` IPC 参数形状（「保存中」常亮）
- [x] `appendAiContent` 后刷新字数并调度自动保存

### 📝 v0.26.36 — 后台配置即时生效 ✅ (2026-07-09)

- [x] `save_settings` → `reload_config` + `app_settings` sync
- [x] `llm_first_chunk_timeout_secs` 接入适配器
- [x] 字体/主题跨窗口 Tauri 事件同步
- [x] TriShot 预算 / writer prompt 读真实配置

### 📝 v0.26.35 — 幕后工作室审计残留 R1–R11 ✅ (2026-07-09)

- [x] R1 Dashboard `scene_count` 真实口径
- [x] R2 CreationPathGuide 快速创作 → `runCreationWorkflow`
- [x] R3 `apply_wizard_to_story` 去重 + KG
- [x] R4 幕后 `genesis-warnings`
- [x] R5/R6 场景序号语义标注
- [x] R7 世界构建文风 Tab
- [x] R8 UsageStats 启发式加强
- [x] R9 伏笔三列 Kanban
- [x] R10 角色→场景跳转
- [x] R11 拆书转故事后导航到场景

### 📝 v0.26.28 Phase 4 — 架构债务与工程体验 ✅ (2026-07-07)

- [x] **外部化 prompts**：`prompts/registry.rs` 中 95 个内置提示词迁移至 `resources/prompts/{category}/{id}.md`，运行时从 Tauri 资源目录加载，保留用户覆盖能力。
- [x] **迁移脚本拆分**：`db/connection.rs` 中 ~2,650 行 inline `run_migrations` 拆分为 `src/db/migrations/V028__*.rs` … `V099__*.rs` 共 70 个编号 Rust 迁移文件；`MigrationRunner` 新增 `RustMigration` trait 统一执行 SQL 与 Rust 迁移。
- [x] **知识图谱手动 CRUD UI**：Graph 页支持新建实体与添加关系。
- [x] **世界构建 AI 生成**：`WorldBuilding` 页新增「AI 生成」按钮，调用 `generateWorldBuildingOptions` 一键生成世界观。
- [x] **角色 AI 扩展**：`Characters` 页新增「AI 扩展」按钮，批量生成并创建角色。
- [x] **叙事分析图表**：`NarrativeAnalysis` 页新增 SVG 折线/面积图展示追读力趋势。
- [x] **策略选择移入 Quick Phase**：`genesis.rs` 中 `StrategySelectionStep` 前移至 `quick_phase_steps()`，`quick_phase_steps` 变为 3 步，`background_steps` 变为 5 步；同步更新步骤编号、进度百分比与测试契约。
- [x] **元文档同步**：`README.md`、`AGENTS.md`、`ARCHITECTURE.md`、`TESTING.md`、`CHANGELOG.md`、`PROJECT_STATUS.md` 版本与内容同步。

### 📝 v0.26.27 Phase 3 — L4 诊断互链、文档与依赖解耦 ✅ (2026-07-07)

- [x] **TracingPanel ↔ GenesisPanel 互链**：Genesis 运行记录可跳转对应生成链路；链路详情可跳转对应 Genesis 运行。
- [x] **Logs 深链**：失败 Genesis 运行一键跳转日志页并预填 `session_id`。
- [x] **UsageStats 按 operation 分组**：全部 / bootstrap / smart_execute / 其他 四标签页。
- [x] **Foreshadowing UX 改进**：`setup_scene_id` 改为场景下拉选择；Ledger 字段在可折叠高级区编辑。
- [x] **前端循环依赖解耦**：`components ↔ stores ↔ hooks ↔ frontstage` 分层清晰化，新增 `types/editor.ts`、`stores/contracts/*`；`appStore.ts` 不再从 `components/EditorSettings.tsx` import；`hooks/contracts/*` 仍待补齐。
- [x] **Tauri 循环依赖解耦**：`creative_engine ↔ llm` 已无互相 import；`model_gateway ↔ router` 仍存少量直接 import，后续继续向 `ports/` / `domain/` 迁移共享 trait。
- [x] **用户文档更新**：`docs/USER_GUIDE.md` 补全 L4 诊断页说明，修正过度承诺，与 v0.26.27 实现一致。
- [x] **元文档同步**：`AGENTS.md`、`ROADMAP.md`、`TESTING.md`、`ARCHITECTURE.md`、`README.md` 版本与内容同步。

### 📝 v0.26.26 Phase 2 — L2 资产补齐与领域层止血 ✅ (2026-07-07)

- [x] **角色页编辑 + 关系 CRUD**：`Characters.tsx` 支持编辑 Genesis 产出角色；新增关系增删改 UI。
- [x] **L2 创世溯源徽章**：Genesis 产出的资产（角色、场景、世界观等）显示「创世」来源标识，手动创建不显示。
- [x] **Story System 合同播种状态卡**：展示 MASTER_SETTING + CHAPTER_1 合同播种状态；失败运行显示警告摘要。
- [x] **Scenes 续写跳转幕前**：`ExecutionPanel` 主行动打开幕前写作界面。
- [x] **拆分 StorySystem.tsx**：标签页拆分为独立组件；原文件 < 200 行，只做 tab 路由。
- [x] **注入 repository traits 到 creative_engine**：`creative_engine/context_builder.rs` 通过 `db/traits.rs` 抽象依赖，领域代码不再直接 `use crate::db::repositories::*`。
- [x] **拆分 db/repositories.rs**：新建 `db/repositories/*.rs`，每个 repo 独立文件，原文件仅做 re-export。

### 📝 v0.26.25 Phase 1 — 可观测性与测试基线 ✅ (2026-07-07)

- [x] **重构 GenesisPanel 步骤模型**：步骤与后端 Quick（3 步）+ Background（5 步）对齐；展示 `steps_json.errors`；支持跳转到 story / 幕前。
- [x] **统一 L1 创作入口 UX**：`Dashboard.tsx`、`Stories.tsx` 与新增 `CreationPathGuide.tsx` 共同引导用户区分三条创作路径。
- [x] **修复 Stories Wizard 重复建故事**：已有故事走 update 路径，避免重复创建。
- [x] **仪表盘统计卡修正**：标签与口径一致；点击卡片可跳转对应页面。
- [x] **高频后端模块首批特征测试**：为 `model_gateway/executor.rs`、`db/repositories.rs`、`memory/ingest.rs` 各补 happy path + 错误路径测试。

### 📝 v0.26.19 Genesis 流程审计与 Phase 2 优化 ✅ (2026-07-06)

**Phase 1 — P0 关键正确性修复**
- [x] `handleSmartGeneration` Gap B 对齐：空 `finalContent` 不锁 `delivered`（与 `handleRequestGeneration` 一致）
- [x] 角色生成世界观上下文修复：`character_future` 不再读取空 `bundle.world_building`，改为 await `world_future` 后用真实 `world_concept` 构造提示词
- [x] `genesis_runs` 表接入：记录创世运行状态机（pending → quick_done → completed/failed）+ story_id + 错误累计
- [x] 新增 `GenesisRunRepository::set_story_id_and_status` / `update_steps_json`

**Phase 2 — P1 架构对齐**
- [x] 后台错误可观测性：`GenesisContext.errors` 共享 `Arc<Mutex<Vec<GenesisStepError>>>`，收集 world update / character relations / scene update / KG relations / contract seeding 的非致命错误，写入 `genesis_runs.steps_json`，发射 `genesis-warnings` 事件供前端 toast
- [x] mutex 中毒锁加固：`PIPELINE_CANCEL_FLAGS` 与 `GatewayExecutor::registry` 改用 `unwrap_or_else(|e| e.into_inner())` 恢复中毒锁，新增 `lock_cancel_flags_recovers_from_poison` 测试
- [x] 文档/注释对齐：`genesis.rs` ChapterSwitch 注释、`window/mod.rs` `auto_accept` 文档、`USER_GUIDE.md` 创世章节更新为 auto-accept 真实路径
- [x] 策略移入 quick_phase 暂缓，记为本节已知债务

**Phase 3 — 测试加固**
- [x] Rust Genesis 契约测试：`compute_trim_ratio`/`should_retry_self_repetition`/`select_first_chapter_content`/`build_first_chapter_chapter_switch` 纯函数边界 + payload 契约；`background_steps` 6 步固定顺序
- [x] 前端 Gap B/C 专用测试 + 状态机端点契约（idle → delivered 可观测效果）
- [x] 跨层共享 trim golden fixture：`tests/fixtures/trim_golden.json`，Rust + TS 双跑锁定 `trim_self_repetition`/`trimSelfRepetition` 跨层一致性
- [x] 降低测试 brittleness：新 Gap C 测试用 `waitFor` 轮询替代固定 `setTimeout`

**Phase 4 — 代码整洁**
- [x] 重命名 `*_future` → `*_gen`（澄清顺序 await，非 tokio::join! 并行）+ 更新 `ParallelWorldOutlineCharacterStep` 注释（标注 world/outline 可并行化延迟债务）
- [x] 去重 `AppConfig::load`（`FirstChapterGenerationStep` 内连续两次合并为单次）
- [x] `appendAiContent` skip 路径不 `markAccepted`（移入实际追加成功的 else 分支）
- [x] `selectChapter` Gap C 重复入站也跳过 setContent（移除 `!isTextAlreadyInEditor` 条件）
- [x] 评估合并 `isGenesisSettingUpRef` → `genesisDeliveryRef`：不合并（覆盖窗口不同，前者覆盖续写 story_created bootstrap，后者仅创世 generating 态）

### 📝 v0.26.18 Genesis 第一章重复竞态加固 ✅ (2026-07-06)

- [x] Gap A：ChapterSwitch auto_accept=true 但 content 为空时 skipContent=true，不标记 delivered
- [x] Gap B：isFirstChapterReady 路径仅在已 append 或编辑器已有内容时标记 delivered
- [x] Gap C：selectChapter 咽喉点新增 delivered + 编辑器已有内容守卫
- [x] 新增 Gap A 回归测试

### 📝 v0.26.17 Issue #4 启动加固：打包 SQL 迁移 ✅ (2026-07-06)

- [x] `tauri.conf.json` 打包 `src/db/migrations/` 到 `$RESOURCE/db/migrations/`
- [x] `setup` 从 Resource 解析 bundled migrations 并传入 `init_db`
- [x] `init_db` 启动前 `create_dir_all`；失败日志含 DB 路径
- [x] 新增 `init_db_succeeds_on_fresh_directory` 回归测试

### 📝 v0.26.16 Genesis 第一章重复根治 + Issue #4 启动稳定性修复 ✅ (2026-07-06)

- [x] 生成侧验证闸门：`genesis.rs` 检测 LLM 自重复比例，≥8% 时 anti-repeat 重试
- [x] Prompt 模板新增「结构纪律」段，禁止首尾回环与整章重复
- [x] 前端单写者状态机：`idle → generating → delivered` 三态替换布尔守卫
- [x] `generating` 态阻塞 `onChapterUpdated` 与 `loadStories` 自动选择
- [x] `delivered` 态阻塞 `setGeneratedText` 幽灵文本恢复
- [x] `textCleanup` 提升为 `src-frontend/src/utils` 共享工具
- [x] Rust `trim_self_repetition` 对齐前端 KMP 最长 border 检测
- [x] Issue #4：`GatewayExecutor::new` 显式接收 `pool`，`setup` 仅在 pool 可用时初始化网关
- [x] 新增不可写应用目录回归测试
- [x] 修复 CI `cargo +nightly fmt -- --check` 与 `npm run format:check` 失败

## ✅ v0.23.x 已实施完成

### 📝 v0.23.74 场景优先架构迁移——Scene 成为唯一叙事真相源 ✅ (2026-06-28)

- [x] Phase 1: 消灭内容双写 — `scenes.content` 为唯一真相源，`chapters.content` 不再直接写入
- [x] Phase 2: 前端编辑器切到 Scene — store `sceneId` 主键，`update_scene` 自动保存
- [x] Phase 3: Commit 触发点迁移 — `SceneCommitDebouncer` 接替 `ChapterCommitDebouncer`
- [x] Phase 4: 创世提示词场景化 — `narrative_first_scene_generate`（14 场景变量），`SceneOutline` 扩展
- [x] 幕前纯正文 — 移除 `SceneDividerNode`，章内容无缝聚合
- [x] `SceneUpdated` 事件新增 `content_changed` 字段

### 📝 v0.23.66 模型角色分配 × 后台并发根治 ✅ (2026-06-28)

- [x] 模型角色分配：创作/工具/后台三层默认模型 + 网关按角色智能调度 + 前端「模型角色分配」卡片
- [x] 后台并发过载根治：`ParallelWorldOutlineCharacterStep` `tokio::join!` 3 路 → 串行 + `BACKGROUND_LLM_SEMAPHORE` 全覆盖

### 📝 v0.23.63 系统提示词可配置 + 第一章注册表化 + 框架级智能路由 ✅ (2026-06-27)

- [x] Gap 1: 第一章正文指令从硬编码 `format!()` 迁移到 PromptRegistry `narrative_first_chapter_generate`（15 个模板变量）
- [x] Gap 2: `LlmProfile.system_prompt_override` → `GenerateRequest.system_prompt` → OpenAI/Anthropic adapter 去硬编码英文
- [x] Gap 3: 新增 `FrameworkSelections` + `build_prompt_framework_catalog()`，Call 1 最快模型自主选择方法论/质量门/注入器
- [x] 前端 ModelModal 新增「系统提示词覆盖」多行文本框

### 📝 v0.23.60 网关探测异步化 + 调度退避 + 并发限流 ✅ (2026-06-27)

- [x] 后台 keepalive 每 10s 刷新缓存 → `is_health_fresh()` 跳过内联 5s 探测，0ms 延迟
- [x] 死模型指数退避 30→60→120→…→3600s
- [x] `BACKGROUND_LLM_SEMAPHORE(1)` 后台 LLM 串行化
- [x] `execute_trishot` → `orchestrator.generate` → genesis DB 保存全线 `log::warn!` 诊断

### 📝 v0.23.59 全面修复并强化模型网关调度 ✅ (2026-06-27)

- [x] `generate_for_request_with_context_and_pipeline` 路由到网关（单点覆盖概念生成 + 5 后台 pipeline）
- [x] `generate_with_fastest` 加 5s 探测 + 回退网关候选链
- [x] 活跃模型连续失败 ≥2 次降级，3 个强制置顶点跳过
- [x] TimeSliced 写作策略从 AppConfig 读取用户配置

### 📝 v0.23.49 推理模型思考链导致 JSON 提取出空对象修复 ✅ (2026-06-26)

- [x] 新增 `strip_reasoning_blocks` 剥离 `önh...` / `<thinking>...</thinking>` 思考链块
- [x] `extract_first_json_object` 跳过空对象 `{}` 继续向后扫描
- [x] 根因：推理模型思考链里的花括号会被 `find('{')` 误当成 JSON 对象，提取出空 `{}` → serde "missing field 'title'"

### 📝 v0.23.48 JSON 提取用括号匹配修复 trailing characters 解析失败 ✅ (2026-06-25)

- [x] 新增 `extract_first_json_object` 用括号匹配精确提取第一个完整 JSON 对象
- [x] 根因：`rfind('}')` 在 JSON 后附带含 `}` 文本时会误提取过多内容

### 📝 v0.23.47 调用模型前实时连接探测 + JSON 尾部多余文本容错 ✅ (2026-06-25)

- [x] 候选模型在实际 LLM 调用前先执行 5s 超时实时探测，探测失败标记 Unhealthy 跳到下一候选
- [x] 三处 WorkflowLogger 日志点：`pre_call_probe.ok` / `pre_call_probe.fail` / `pre_call_probe.timeout`

### 📝 v0.23.46 AI 状态提示使用模型名称 ✅ (2026-06-25)

- [x] `generation-status` 和 `llm-generating-progress` 心跳事件状态文案追加模型名称

### 📝 v0.23.45 IngestPipeline LLM 调用静默化，根治正文后活动卡死与页面崩溃 ✅ (2026-06-25)

- [x] 将 IngestPipeline 的三个 `context_label`（`"记忆-内容分析"`、`"记忆-生成知识"`、`"记忆-叙事事件提取"`）加入 `is_silent_background` 静默列表
- [x] 根因：创世正文返回后 IngestPipeline 并发发起多个 LLM 调用未静默，进度事件覆盖前端主活动导致卡死，本地模型并发崩溃导致页面空白

### 📝 v0.23.44 AI 状态提示使用模型名称 ✅ (2026-06-25)

- [x] `generation-status` 和 `llm-generating-progress` 心跳事件状态文案追加模型名称

### 📝 v0.23.43 前端诊断日志 + log_frontend_event 命令 ✅ (2026-06-25)

- [x] 新增 `log_frontend_event` Tauri 命令，前端可写入 WorkflowLogger

### 📝 v0.23.42 根治创世卡在"最终输出"：BGP-4 自死锁修复 ✅ (2026-06-25)

- [x] BGP-4 从 `spawn_blocking().await` 改为 `tokio::spawn`（fire-and-forget）
- [x] 根因：BGP-4 同步等待 DB 查询与 BGP-1/BGP-3 竞争 `std::sync::Mutex` 自死锁

### 📝 v0.23.40 参照现有诊断机制添加 WorkflowLogger 日志点 ✅ (2026-06-25)

- [x] Bug A 日志点：`genesis.first_chapter.generated`、`genesis.chapter_switch.sent`、`genesis.final_content`
- [x] Bug B 日志点：`smart_execute.start`、`trishot.call3.done`、`trishot.bgp4.start`/`bgp4.done`
- [x] 前端 `[DEBUG-dup]` / `[DEBUG-act]` console.warn 诊断日志

### 📝 v0.23.37 Genesis 活动清理 + 前端正文重复修复尝试 ✅ (2026-06-25)

- [x] Genesis 成功路径补发 `smart-execute-progress` completed/error 事件
- [x] `smart-execute-progress` 处理器把 timeout/error 映射为 failed

### 📝 v0.23.36 创世正文清洗 + 后台作业不阻塞输入 ✅ (2026-06-25)

- [x] TriShot Call 3 追加 `NOVEL_OUTPUT_DISCIPLINE` 输出纪律段（禁元评论/markdown/小节标题/幕结束批注）
- [x] 新增 `sanitize_novel_output` 后处理兜底（逐行去 markdown→截断尾部元评论→剥离前导过渡语→去整行小节标题/批注）
- [x] 7 个单元测试覆盖各场景（前导剥离/尾部截断/markdown清洗/幕结束/小节标题/纯净正文不误伤/空输入）
- [x] Genesis 后台阶段事件打 `metadata: {background: true}` 标记，前端跳过注册 running activity，输入框不再被禁用

### 🩹 v0.23.35 采摘 Step1 JSON 解析容错 ✅ (2026-06-23)

- [x] `memory/ingest.rs` 6 个反序列化结构体补 `#[serde(default)]`，修复 `missing field entity_type`

### 🏛️ v0.23.34 select_candidates Mutex 自死锁修复 ✅ (2026-06-23)

- [x] 全链路 15 个诊断标记精确定位自死锁位置
- [x] 根因：`health_registry.lock()` MutexGuard 不释放，`is_model_available` 再次 lock → std::sync::Mutex 不可重入 → 自死锁
- [x] 修复：health 锁移入嵌套块作用域，块结束时自动释放
- [x] Call 1 走 select_fastest_profile 不受影响，Call 3 走 select_candidates 此前必死锁
- [x] 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### 🚑 v0.23.19 根治 600s 超时：record_llm_call DB 写入不再阻塞 tokio worker ✅ (2026-06-22)

- [x] 生产连接池 `init_db` 补 `.connection_timeout(5s)`，防止 `pool.get()` 无限阻塞
- [x] `record_llm_call` 改为 fire-and-forget `spawn_blocking`，DB 写入提交到阻塞线程池立即返回
- [x] 工作流日志新增 `llm.record_call.spawn` phase 标记提交点
- [x] 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### 🔬 v0.23.18 行级诊断：execute_generation Ok 分支 12+ 标记 ✅ (2026-06-22)

- [x] `execute_generation` Ok 分支每步前后插入工作流日志标记（`record_call.start` → `try_state` → `db_write` → `db_done` → `emit_completed.start` → `generate.return_ok`）
- [x] 新增 5 个独立模块测试（心跳 abort、阻塞 emit、Mutex 死锁、pool 超时、record 非阻塞）

### 🛡️ v0.23.17 心跳阻塞 + 连接池超时双保险 ✅ (2026-06-22)

- [x] `heartbeat_handle.await` 用 `tokio::time::timeout(5s)` 包裹
- [x] 测试连接池补 `.connection_timeout(10s)`
- [x] `record_llm_call` 内部添加诊断标记

### 🔧 v0.23.16 Genesis 快速阶段卡死修复 + E2E 集成测试 ✅ (2026-06-22)

- [x] `story_repo.create()` 改用 `tokio::task::spawn_blocking` 异步化
- [x] 新增 `scripts/test_trishot_e2e.py` E2E 集成测试（73.2s 完成，1852 中文字）

### 🔧 v0.23.15 TriShot 管线 4 处缺陷修复 ✅ (2026-06-22)

- [x] P0: 预检失败时调 `AutoContractBuilder::auto_fill` 补齐角色后重试
- [x] P1: `novel_bootstrap_background_started` → `novel_bootstrap_first_chapter_ready`
- [x] P2: Call 1/2 预算守卫用 `total_start` 计算已耗时间；Call 3 超时 30-120s + 空内容检查

### 🏗️ v0.23.14 干净健康的模型池 + 两阶段 Genesis ✅ (2026-06-22)

- [x] 启动归零清空 `llm_calls` + 过滤 `HealthRegistry` 残留；删除/更新模型级联清理
- [x] Genesis 拆分为 `quick_phase_steps()`（概念+第一章 TriShot）+ `background_steps()`（世界观/大纲/角色）

### 🔒 v0.23.13 强制所有生成路径使用活跃模型 ✅ (2026-06-22)

- [x] `LlmService::select_profile_for_request` 无条件优先返回 `active_llm_profile`
- [x] `GatewayExecutor::select_candidates` 将健康活跃模型强制置顶为 primary
- [x] `GatewayExecutor::select_fastest_profile` 健康活跃模型无条件优先，不再受 TTFB 阈值限制
- [x] Genesis 故事概念、TriShot Call 1、普通路由生成全部走用户当前设置的活跃模型
- [x] 新增模型保存后即时刷新注册表并执行健康探测

### 🎯 TriShot 三击生成管线 ✅ (v0.23.0)

- [x] GenerationMode::TriShot 三击模式（与 Fast/TimeSliced/Full 并存）
- [x] prompt_synthesis 模块（manifest + synthesizer + refiner）
- [x] GatewayExecutor::select_fastest_profile + generate_with_fastest
- [x] PlanExecutor TriShot 快速路径（跳过计划生成 LLM）
- [x] PlanStep::long_running 跳过 90s 步超时
- [x] execute_trishot 完整管线（Call 1 → Call 2 → Call 3 + 预算守卫）
- [x] BGP-2 auto_rewrite_executor（HIGH 自动改写 / LOW 建议）
- [x] SyncEvent::ContentAutoRevised / RevisionSuggested
- [x] 前端「三击模式」配置选项
- [x] BGP-3 后台 IngestPipeline（补 smart_execute 路径缺口）
- [x] BGP-1/BGP-4 后台审计+洞察链式 spawn
- [x] silent_background 白名单扩展（4 个新标签）

### 🧩 v0.23.4 智能层闭环落地 ✅ (2026-06-21)

- [x] LLM JSON mode 原生支持（`ResponseFormat::JsonObject`）
- [x] OpenAI/Ollama 适配器结构化输出接线
- [x] Review/Refine Pipeline 解析 `refinement_notes`
- [x] `MemoryBudget::for_task_type` 强类型化预算参数
- [x] 拆书存储统一：`reference_characters` / `reference_scenes` 删除，汇入 `narrative_*` 表
- [x] 迁移 `V100__拆书存储统一_删除_reference_表.sql`

### 🎨 v0.23.5 CI 格式化修复 ✅ (2026-06-21)

- [x] Rust nightly `cargo fmt` 格式化差异清零
- [x] 前端 Prettier 格式化差异清零
- [x] GitHub Actions `rust-check` / `frontend-check` 通过

### 🐛 v0.23.6 修复 macOS 启动崩溃 ✅ (2026-06-22)

- [x] 修复 `state() called before manage() for Arc<dyn VectorStore>` 启动 panic
- [x] `LanceVectorStore` 创建与 `app.manage` 提前到依赖组件之前
- [x] 全平台 CI 构建通过，生成 `.dmg` / `.deb` / `.msi`

### 📋 v0.23.7 诊断信息增强 ✅ (2026-06-22)

- [x] 修复诊断卡片版本号硬编码为 `0.16.0`
- [x] 修复前端/后端超时文案硬编码 `200s` / `180s`
- [x] 诊断信息新增 AI 生成模式、当前模型 ID/名称/提供商/端点
- [x] 诊断信息新增最后调用模型与最后发给 LLM 的提示词全文
- [x] 后端 `LlmService` 发射 `llm-prompt-sent` 事件供前端诊断捕获

### 🚀 v0.23.8 AI 进度指示精细化 ✅ (2026-06-22)

- [x] `LlmGeneratingProgress` 新增 `model_id`、`provider`、`prompt_chars`、`prompt_tokens`、`response_tokens`
- [x] 进度文案具体化：连接模型、组合提示词、等待回应、模型回应 token 数、解析结果
- [x] 新增 `diagnostics::DiagnosticStore` 与 `get_last_llm_prompt` 命令
- [x] 解决大提示词事件丢失导致诊断“未捕获”的问题

### 📚 v0.23.9 运行时创作资产能力清单 ✅ (2026-06-22)

- [x] 应用启动时自动生成并刷新全部系统创作资产目录
- [x] `AssetCapabilityManifest` 注入 Tauri State
- [x] TriShot Call 1 prompt 注入【系统可用创作资产目录】
- [x] TriShot Call 3 透传 `selected_asset_ids` / `asset_tags` 给 ModelGateway
- [x] ModelGateway dispatcher 识别 methodology/beat_card/story_engine/pressure_relationship/style_dna/skill 等标签
- [x] 修复 TriShot `request_id` 错误赋值、Call 1 无预算守卫

### 🎯 v0.23.10 模型网关优先使用当前活跃模型 ✅ (2026-06-22)

- [x] `select_fastest_profile` 优先使用当前 `active profile`（健康且 TTFB 不比最快模型差太多）
- [x] `select_candidates` 保证活跃模型始终出现在候选链中

### 🛡️ v0.23.11 诊断提示词过滤探测/静默调用 ✅ (2026-06-22)

- [x] 静默/探测调用不再更新 `DiagnosticStore` 和 `llm-prompt-sent` 事件
- [x] 避免 `model_gateway_probe` 的 `Respond with exactly the word OK.` 覆盖诊断提示词

### 🐛📝 v0.23.12 活跃模型优先 + 智能创作流程日志 ✅ (2026-06-22)

- [x] `GatewayExecutor::generate` 强制把当前活跃模型放到候选链首位
- [x] `select_fastest_profile` 无算力档案时也优先使用活跃模型
- [x] 新增 `WorkflowLogger`，记录 TriShot/LLM/ModelGateway 各阶段到 `logs/creative_workflow.log`
- [x] 诊断卡片显示工作流日志路径与最近日志

## ✅ v0.22.x 已实施完成

### 🧩 「异星球末世生存」复合题材创作流程优化 ✅ (v0.22.4)

- [x] GenreResolver 题材解析服务
- [x] GenreProfile 中文别名扩展
- [x] StrategySelector / build_selected_strategy / story_concept_prompt 接入 GenreResolver
- [x] AssetNode tags 与资产同步标签注入
- [x] IntentionGraphPlanner 复合题材资产补充发现
- [x] GatewayRequest asset_tags / discovered_asset_ids 透传
- [x] TaskClassifier / GatewayExecutor 资产标签感知调度
- [x] WriteTimeBundle secondary_genre_profile_strategy 复合题材续写补强

### 🔐 钥匙串彻底移除 + 模型健康报告自动刷新 ✅ (v0.22.3)

- [x] 移除 keyring crate（全平台依赖）
- [x] 移除 secure_storage 模块
- [x] API Key 改为直接存 SQLite
- [x] 模型健康报告每 30 秒自动刷新
- [x] AppConfig.load() 热路径冗余调用消除
- [x] Phase A：TimeSliced 路径全资产注入（StyleDNA六维+方法论+体裁画像+写作策略）
- [x] Phase B：Inspector 全资产注入（体裁画像+角色状态+活跃冲突+四元组+方法论）
- [x] Phase C：意图感知调度接线（agent_type→intent 自动推导，activate classify_by_intention）
- [x] Phase D：算力档案消费闭环（CapabilityProfile TTFB/TPS 参与候选排序）
- [x] Phase E：资产→生成参数规则映射（asset_params.rs）
- [x] Phase F：GenreProfile 推荐资产字段（Migration 96 + 4 新列 + 种子数据 7 题材）

### 提示词全量可配置化 ✅

- [x] 79 个提示词全部纳入 PromptRegistry（21 个分类）
- [x] 前端 Monaco 编辑器 + 批量导入/导出
- [x] 40+ 个原硬编码提示词全部接入 registry
- [x] 15 个假接入 key 修复为真实 DB 覆盖

### SING 意图图集成 ✅

- [x] Migration 95：6 张意图图表
- [x] 意图合成流水线（LLM 增强 + 规则回退）
- [x] PPR 分层发现
- [x] 动态 ReAct 执行
- [x] IntentionGraphPlanner × PlanExecutor 集成
- [x] 前端诊断面板（IntentionGraphDiagnostics）

### v0.20.x 基础设施 ✅

- [x] Phase 1-5: SING 数据层/离线合成/分层发现/PlanGenerator重构/动态ReAct
- [x] Phase 6: 模型网关意图感知集成
- [x] Phase 7: 前端意图图诊断面板
- [x] P0 断环修复: 资产同步/意图分类/执行图持久化/LLM合成/PPR传播
- [x] 真实模型测试（Gemma4-e2b, 6/6）
- [x] Multi-Agent Sessions（6种助手类型）

### Phase 4: AI 智能生成 ✅

**状态**: 完整实现

- [x] NovelCreationAgent
- [x] NovelCreationWizard 组件
- [x] 卡片式选择 UI
- [x] 首个场景自动生成

### Phase 5: 工作室配置系统 ✅

**状态**: 完整实现

- [x] StudioConfig 模型
- [x] StudioManager（导入/导出）
- [x] ZIP 格式支持
- [x] 默认主题配置

### Phase 6: 场景版本系统 ✅ (v3.1.0)

**状态**: 完整实现

- [x] SceneVersionRepository（版本CRUD）
- [x] SceneVersionService（比较、恢复、统计）
- [x] VersionTimeline 组件（垂直时间线）
- [x] DiffViewer 组件（差异对比）
- [x] ConfidenceIndicator 组件（置信度可视化）
- [x] 版本链管理（supersession）

### Phase 7: 混合搜索系统 ✅ (v3.1.0)

**状态**: 完整实现

- [x] BM25 Search（CJK二元组分词）
- [x] Hybrid Search（RRF融合排序）
- [x] Entity Hybrid Search（名称+向量）
- [x] 可配置权重和参数

### Phase 8: 记忆保留系统 ✅ (v3.1.0)

**状态**: 完整实现

- [x] RetentionManager（遗忘曲线计算）
- [x] 五级优先级分类
- [x] 遗忘时间预测
- [x] 保留报告生成
- [x] 上下文窗口优化

### Phase 9: 幕前界面重构与本地模型 ✅ (v3.1.1)

**状态**: 完整实现

- [x] 精简侧边栏（仅保留"幕后"按钮）
- [x] OKLCH 颜色系统重构（去除 AI 感模板色）
- [x] LXGW WenKai 字体替换（去除 Crimson/Inter）
- [x] Blockquote 与微交互重设计（Waza 原则）
- [x] 顶部动态状态栏
- [x] 底部 LLM 对话栏（悬停显示、模型状态灯、去除模式切换图标）
- [x] 流式对话交互（Enter 发送 / Shift+Enter 换行）
- [x] 本地三模型配置（Gemma / Qwen3.5 / bge-m3）
- [x] Tauri Windows 构建与打包（MSI + NSIS）
- [x] GitHub Actions CI 图标修复（macOS / Ubuntu）

---

### Phase 10: 设计-实现对齐修复 ✅ (v5.6.0)

**状态**: 全部完成

- [x] Scene 删除外键清理（chapters.scene_id → NULL）
- [x] Wizard 同步事件（story_created + data_refresh）
- [x] Character relationships 真实查询（character_relationships 表 JOIN）
- [x] Collab 文档 OT 重建（operations apply 重建内容）
- [x] Workflow EdgeCondition 条件求值（8 种运算符）
- [x] Task 心跳超时指数退避重试
- [x] Outline/Foreshadowing/Payoff 修改后同步事件
- [x] Cache 对称失效（sceneUpdated↔chapters、chapterDeleted↔scenes）
- [x] Workflow 节点 300s 超时
- [x] INGEST_COOLDOWN 24h 过期清理
- [x] FrontstageApp 真实 feedback（移除 mock learnings）
- [x] WritingStyle 更新同步事件
- [x] Workflow 并发守卫与重试幂等性
- [x] Pending vector SQLite 持久化
- [x] Task 执行 300s 超时

### Phase 11: 提示词全面可配置化 ✅ (v0.19.0)

**状态**: 全部完成

- [x] 35+ 内置提示词注册表（`prompts/registry.rs`）
- [x] 15 个 `PromptCategory` 分类体系
- [x] 雪花法 10 步提示词注入注册表
- [x] 5 个内置技能提示词映射（`skill_id_to_prompt_id`）
- [x] Memory / Knowledge / MultiAgent 模块接入注册表
- [x] 前端 PromptsPanel 重写（分类 + 搜索 + 批量重置 + 默认值预览）
- [x] GeneralSettings 精简为「提示词注册表」链接卡片
- [x] `reset_all_prompt_overrides` 批量重置 IPC
- [x] 运行时覆盖生效（`resolve_prompt()` 优先查 DB）

---

## 📊 v0.19.0 项目状态

| 模块             | 完成度   | 说明                                                                                                    |
| ---------------- | -------- | ------------------------------------------------------------------------------------------------------- |
| 场景化叙事系统   | 100%     | Scene 模型、StoryTimeline、SceneEditor                                                                  |
| 增强记忆系统     | 100%     | Ingest/Query Pipeline、Knowledge Graph、LanceDB 语义搜索、Pending Vector SQLite 持久化                  |
| AI 智能生成      | 100%     | NovelCreationAgent、Bootstrap 两阶段、创建向导、真实自适应学习反馈                                      |
| 工作室配置       | 100%     | 导入/导出、主题系统                                                                                     |
| 混合搜索         | 100%     | BM25 + Vector RRF融合 + 语义嵌入                                                                        |
| 场景版本         | 100%     | 版本历史、对比、恢复                                                                                    |
| 记忆保留         | 100%     | 遗忘曲线、优先级管理                                                                                    |
| 幕前界面         | 100%     | 精简侧边栏、幽灵文本、`/` 菜单                                                                          |
| 幕前幕后自动关联 | 100%     | Chapter↔Scene 双向映射、state_sync、实时同步、Cache 对称失效完整、writingStyle/storySelected 缓存精确化 |
| 后台自动化       | 100%     | Workflow 持久化、能力进化反馈环、向量索引闭环（Chapter + Scene）、Workflow 幂等性                       |
| 本地模型配置     | 100%     | 三模型集成                                                                                              |
| 提示词可配置化   | 100%     | 35+ 提示词注册表、15 分类、前端完整管理面板、运行时覆盖生效                                             |
| Tauri 构建       | 100%     | MSI + NSIS 安装包                                                                                       |
| 设计-实现对齐    | 100%     | v5.6.4 Tauri IPC rename_all 修复                                                                        |
| **整体 v0.19.0** | **100%** | 核心功能全部完成                                                                                        |

---

## 🚀 编译状态

```bash
$ cd src-frontend && npm run build
    vite v6.4.2 building for production...
    ✓ 2156 modules transformed.
    dist/                     655.75 kB │ gzip: 216.60 kB
```

```bash
$ cd src-tauri && cargo tauri build
    Finished release profile [optimized] target(s) in 8m 04s
       Built application at: target/release/storymoss
    Finished 3 bundles at:
        target/release/bundle/dmg/StoryMoss_0.23.6_aarch64.dmg
        target/release/bundle/deb/storymoss_0.23.6_amd64.deb
        target/release/bundle/msi/StoryMoss_0.23.6_x64_en-US.msi
```

```bash
$ cd src-tauri && cargo test --lib
    running 538 tests
    test result: ok. 538 passed; 0 failed; 2 ignored
```

✅ **编译成功** | ✅ **测试全绿** | ✅ **打包成功**

---

## 🆕 v3.1.1 新增依赖

| 依赖                          | 版本    | 用途             |
| ----------------------------- | ------- | ---------------- |
| @tiptap/react                 | ^3.22.3 | 幕前富文本编辑器 |
| @tiptap/starter-kit           | ^3.22.3 | TipTap 基础扩展  |
| @tiptap/extension-placeholder | ^3.22.3 | 占位符扩展       |

---

## 📋 后续路线图

### v3.2.x 进行中

- [x] LLM 真实 SSE 流式输出
- [x] Anthropic 适配器
- [x] Ollama 适配器
- [x] 实体嵌入持久化修复

#### 向量存储增强

- [x] SQLite 向量存储持久化（已替代 JSON-memory fallback）
- [ ] LanceDB 持久化存储（ blocked：Arrow 依赖与当前工具链冲突）
- [x] 实体向量持久化（`kg_entities.embedding` BLOB 读写修复）
- [x] 实体向量自动更新（属性变更时重新生成嵌入）
- [x] 语义搜索优化
- [ ] 向量索引性能优化

#### 知识图谱可视化

- [x] 实体关系图谱可视化
- [x] 交互式图谱浏览（双击聚焦、搜索筛选、类型过滤）
- [x] 实体详情弹窗
- [x] 关系强度可视化

#### 记忆系统增强

- [x] 自动归档系统（一键归档 + 恢复 + 已归档浏览）
- [x] 创建向导自动 Ingest
- [x] 实体嵌入持久化
- [x] 知识蒸馏
- [x] 记忆压缩

#### 协作功能

- [x] 评论和批注系统
- [x] 修订模式
- [x] 变更追踪

### v3.3.0 (中期计划)

#### 云端同步

- [ ] 用户账户系统
- [ ] 云存储集成
- [ ] 多设备同步

#### 协作写作增强

- [ ] 实时协作场景编辑
- [ ] 评论和批注系统
- [ ] 修订模式

#### 插件市场

- [ ] Skills 分享平台
- [ ] 主题市场
- [ ] Agent 模板市场

#### 导出增强

- [ ] 自定义导出模板
- [ ] 批量导出
- [ ] 自动发布集成

### v4.0.0 (长期计划)

#### 技术架构升级

- [ ] WebAssembly 前端 (Leptos)
- [ ] 自研小模型部署
- [ ] 边缘计算支持

#### 多人实时协作

- [ ] OT 算法完整实现
- [ ] 实时光标同步
- [ ] 冲突解决机制

#### 移动端支持

- [ ] iOS 应用
- [ ] Android 应用
- [ ] 响应式 Web 版本

#### 发布平台集成

- [ ] 起点中文网集成
- [ ] 晋江文学城集成
- [ ] 自出版平台 (Amazon KDP)

---

## 📈 历史版本

### v0.23.13 (2026-06-22)

- [x] 强制 Genesis / TriShot / 普通路由生成统一使用用户设置的活跃模型
- [x] `select_profile_for_request`、`select_candidates`、`select_fastest_profile` 全部优先活跃模型
- [x] 新增模型保存后即时健康探测并刷新网关注册表

### v0.23.12 (2026-06-22)

- [x] 活跃模型强制优先，修复连接错误模型导致的长超时
- [x] 新增 WorkflowLogger 记录 TriShot/LLM/ModelGateway 详细执行步骤

### v0.23.11 (2026-06-22)

- [x] 诊断提示词过滤探测/静默调用，避免被 probe prompt 覆盖

### v0.23.10 (2026-06-22)

- [x] `select_fastest_profile` 优先使用当前活跃模型，避免连到旧模型
- [x] `select_candidates` 候选链兜底活跃模型

### v0.23.9 (2026-06-22)

- [x] 运行时创作资产能力清单：启动时刷新全部系统资产并注入 TriShot/ModelGateway
- [x] TriShot Call 1 可见全局资产，Call 3 透传选中资产给模型网关
- [x] 修复 TriShot request_id 错误与 Call 1 预算守卫

### v0.23.8 (2026-06-22)

- [x] AI 进度指示精细化：连接模型、组合提示词、等待回应、模型回应、解析结果
- [x] 新增 `DiagnosticStore` 与 `get_last_llm_prompt` 命令，提升提示词诊断可靠性

### v0.23.7 (2026-06-22)

- [x] 诊断卡片版本号改为从 `package.json` 动态读取
- [x] 超时文案去硬编码，读取用户实际设置
- [x] 诊断信息新增 AI 生成模式、当前模型、最后 LLM 提示词

### v0.23.6 (2026-06-22)

- [x] 修复 macOS 启动崩溃（VectorStore State 初始化顺序）
- [x] 全平台 CI 构建通过（`.dmg` / `.deb` / `.msi`）

### v0.23.5 (2026-06-21)

- [x] CI 格式化修复（Rust nightly fmt + 前端 Prettier）
- [x] `rust-check` / `frontend-check` 通过

### v0.23.4 (2026-06-21)

- [x] LLM JSON mode 原生支持（OpenAI/Ollama）
- [x] Review/Refine Pipeline 结构化输出
- [x] MemoryPack 预算参数强类型化
- [x] 拆书存储统一，删除 `reference_characters` / `reference_scenes`

### v0.23.3 (2026-06-21)

- [x] MigrationRunner 交错执行修复
- [x] V092 测试基线 48 个失败清零
- [x] `narrative_*` 表 `status` 列补齐

### v0.23.2 (2026-06-21)

- [x] `SyncEvent::ChapterCommitted`
- [x] 前端编辑器状态收敛到 `frontstageStore`

### v0.23.1 (2026-06-21)

- [x] 全局单例清零（14 个）
- [x] 模块循环依赖斩断

### v0.23.0 (2026-06-21)

- [x] TriShot 三击生成管线
- [x] prompt_synthesis 模块
- [x] BGP-2 智能改写
- [x] 前端「三击模式」配置

### v3.1.1 (2026-04-13)

- [x] 幕前界面重构（Waza 设计原则）
- [x] OKLCH 颜色系统 / LXGW WenKai 字体
- [x] 本地三模型配置
- [x] Tauri Windows 构建打包
- [x] GitHub Actions CI 跨平台修复

### v3.1.0 (2025-04-13)

- [x] 混合搜索
- [x] 场景版本管理
- [x] 记忆保留曲线

### v3.0.0 (2025-04-12)

- [x] 场景化叙事架构
- [x] 增强记忆系统
- [x] AI 智能生成
- [x] 工作室配置

### v2.0.x (已完成)

- [x] 双界面架构 (幕前/幕后)
- [x] 技能系统
- [x] MCP 支持
- [x] 状态管理
- [x] 模型路由
- [x] 进化算法
- [x] 导出功能 (PDF/EPUB)

### v1.x (已完成)

- [x] 基础架构
- [x] LLM 集成
- [x] 数据库设计
- [x] 前端界面

---

## 🎯 优先级说明

| 优先级 | 说明               |
| ------ | ------------------ |
| P0     | 核心功能，必须完成 |
| P1     | 重要功能，影响体验 |
| P2     | 增强功能，锦上添花 |
| P3     | 未来规划，长期目标 |

---

## 📚 相关文档

- [V3 架构计划](docs/plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计
- [CHANGELOG](CHANGELOG.md) - 版本变更记录
- [PROJECT_STATUS](PROJECT_STATUS.md) - 详细项目状态
