# 🧪 StoryMoss 自动化测试环境 (v0.60.0)

本机已配置 Playwright 无头浏览器自动化测试环境，专为 AI 助手设计。

## 测试统计

### v0.60.0 变更说明

- 新增 Rust 15 项：`memory::continuity` 8 项（知识流落库/揭示状态单向推进/物品 upsert 与状态/泄密与归属纯函数/探针接线）+ `cascade_rewriter::impact_report` 6 项（帖主测试③下游影响/忽略与 stale 迁移/无处不在实体过滤/冲突解析容错/KG 实体名解析）+ `continue_assets` 连续性块渲染 1 项。
- 新增前端 5 项：`CascadeCenter` 页面（批次渲染与失效标记/去查看定位/忽略/触发改写/空态）。
- 全量基线：`cargo test --lib` 1643 passed / 3 ignored；`npx vitest run` 590 passed / 3 skipped。

### v0.59.4 变更说明

- 无行为变更（发布纪律与网站链路修复）。新增 CI 作业 `docs-guard`（tag 推送时校验 8 份必需文档）。
- 基线不变：`cargo test --lib` 1628 passed / 3 ignored；`npx vitest run` 585 passed / 3 skipped；landing 24 tests。

### v0.59.3 变更说明

- 新增：故事大纲来源保护 3 项（materialize 不覆盖手写 / 仍更新机器来源 / ingest 不追加手写）+ 仓库层来源标记 2 项。
- 删除：死模块 `memory/hybrid_search.rs`（无测试）。
- 全量基线：`cargo test --lib` 1628 passed / 3 ignored；`npx vitest run` 585 passed / 3 skipped。

### v0.59.2 变更说明

- 新增：`isEmptyEditorHtml` 空文档判定 3 项（前端）；`extract_and_sanitize_json` 尾随逗号换行 2 项（Rust）。
- 删除：4 个孤儿 hook 的 27 项测试（无生产消费者）。
- 门禁：`e2e-check` 提升为阻塞；Playwright 39 passed / 5 skipped（连续两轮）。
- 全量基线：`cargo test --lib` 1626 passed / 3 ignored；`npx vitest run` 585 passed / 3 skipped。

### v0.59.1 变更说明

- 无测试变更（整仓格式化修复，恢复 CI 格式门）。基线不变：`cargo test --lib` 1624 passed / 3 ignored；vitest 609 passed / 3 skipped。

### v0.59.0 变更说明

- 新增：迁移 checksum/集合水位线、索引与级联、后台取消传播、质检闭环、提示词资产、导出与 server 加固的回归。
- 新增：provider 故障注入测试（reasoning_content 空正文 / 围栏 JSON / 截断与空响应）。
- 新增：golden 续写 harness（`#[ignore]`，需真机模型；`STORYMOSS_GOLDEN_MODEL` + `STORYMOSS_GOLDEN_OUT` 产出指标 JSON）。
- E2E：新增 `e2e/frontstage-agency-append.spec.ts`（幕前续写落库 / 不重复 / 生成中重复提交）。
- 全量基线：`cargo test --lib` 1624 passed / 3 ignored（+41）；`npx vitest run` 609 passed / 3 skipped（+2）；Playwright 39 passed / 5 skipped。

### v0.58.0 变更说明

- 节拍卡改变项、续写原地踏步合同、编辑审计 impact/fix、复述近文探针、V131 story_format、幕后短剧选择器。
- 测试调整：Rust +11；vitest +1。
- 全量基线：`cargo test --lib` 1583 passed / 2 ignored（+11）；`npx vitest run` 607 passed / 3 skipped（+1）。

### v0.56.2 变更说明

- 悬挂闭合引号合并认全角缩进；空行/存量 HTML 孤段并回；后台审查顶栏 fail-open。
- 测试调整：Rust +1；vitest +4。
- 全量基线：`cargo test --lib` 1572 passed / 2 ignored（+1）；`npx vitest run` 606 passed / 3 skipped（+4）。

### v0.56.1 变更说明

- 拆人探针认独立称呼；死人目光探针。
- 测试调整：Rust +2。
- 全量基线：`cargo test --lib` 1571 passed / 2 ignored（+2）；`npx vitest run` 602 passed / 3 skipped（基线，本版无前端逻辑变更）。

### v0.56.0 变更说明

- 续写导演锁、拆身/亲缘探针、关系 upsert、`ensure_relationships`。
- 测试调整：Rust +20。
- 全量基线：`cargo test --lib` 1569 passed / 2 ignored（+20）；`npx vitest run` 602 passed / 3 skipped（基线，本版无前端逻辑变更）。

### v0.55.0 变更说明

- catalog 名+一行、`asset_read`、续写冻结、短操作合同。
- 测试调整：Rust +7。
- 全量基线：`cargo test --lib` 1549 passed / 2 ignored（+7）；`npx vitest run` 602 passed / 3 skipped。

### v0.54.0 变更说明

- Agency ToolLoop 原生 tools：`ToolSpec`/`ToolCall`、三适配器、`resolve_loop_action`、续写不带 tools。
- 测试调整：Rust +16。
- 全量基线：`cargo test --lib` 1542 passed / 2 ignored（+16）；`npx vitest run` 602 passed / 3 skipped。

### v0.53.6 变更说明

- 大婚礼成死亡检测、死人退出阵容、下一拍跳过已写行刺、探针拦重演。
- 测试调整：Rust +7。
- 全量基线：`cargo test --lib` 1526 passed / 2 ignored（+7）；`npx vitest run` 602 passed / 3 skipped。

### v0.53.5 变更说明

- 确认前不写库；确认稿按手改落库；幕前确认/取消/重写。
- 测试调整：Rust +2；vitest +5。
- 全量基线：`cargo test --lib` 1519 passed / 2 ignored（+2）；`npx vitest run` 602 passed / 3 skipped（+5）。

### v0.53.4 变更说明

- 「写后续的故事大纲，同时生成后续的场景大纲」形状检测 + 后置纠正不进 Append。
- 测试调整：Rust +2。
- 全量基线：`cargo test --lib` 1517 passed / 2 ignored（+2）。

### v0.53.3 变更说明

- 落库摘要含大纲正文；后续指令靶解析；提示词从章末往下写；幕前弹窗契约。
- 测试调整：Rust +3。
- 全量基线：`cargo test --lib` 1515 passed / 2 ignored（+3）；`npx vitest run` 597 passed / 3 skipped。

### v0.53.2 变更说明

- 真机键值散文（双靶英文冒号、场景大纲中文冒号）、仅场景 salvage、「重新生成」形状。
- 测试调整：Rust +7。
- 全量基线：`cargo test --lib` 1512 passed / 2 ignored（+7）。

### v0.53.1 变更说明

- 对象 `story_outline`、中文键、顶层 core_conflict、散文 salvage、短垃圾仍不写库。
- 测试调整：Rust +6。
- 全量基线：`cargo test --lib` 1505 passed / 2 ignored（+6）。

### v0.53.0 变更说明

- 按正文重写资产：靶解析、分类纠正、四靶 persist、mock LLM execute、幕前不进纸面。
- 测试调整：Rust +19；vitest +1。
- 全量基线：`cargo test --lib` 1499 passed / 2 ignored（+19）；`npx vitest run` 597 passed / 3 skipped（+1）。

### v0.52.0 变更说明

- 拍级编译：近文 1500、任务点名准入、L1/L2 卡、大纲封顶、准入轨迹。
- 测试调整：Rust +7。
- 全量基线：`cargo test --lib` 1480 passed / 2 ignored（+7）。

### v0.51.6 变更说明

- 网关 `apply_active_model_front`：工具档/后台档不被创作模型盖链头。
- 测试调整：Rust +3。
- 全量基线：`cargo test --lib` 1473 passed / 2 ignored（+3）。

### v0.51.5 变更说明

- 进行中续写不弹中断卡；`active_run` 弹窗渲染空。
- 测试调整：弹窗契约改为渲染空；FrontstageApp +1。全量 `npx vitest run` 596 passed / 3 skipped。

### v0.51.4 变更说明

- 进行中创作任务弹窗不再去设置；`isActiveCreativeRunConflict` 契约。
- 测试调整：vitest +3；Rust 既有 `test_map_active_run_conflict` 增 `field=active_run`。
- 全量基线：`cargo test --lib` 1470 passed / 2 ignored；`npx vitest run` 596 passed / 3 skipped（+3）。

### v0.51.3 变更说明

- 大纲去规划、过短不在剩余不足时重试、取消停候选链。
- 测试调整：Rust +4。
- 全量基线：`cargo test --lib` 1470 passed / 2 ignored（+4）。

### v0.51.2 变更说明

- 续写切断节拍卡/约束规划泄露：`detect_and_strip_bare_cot` 强信号全文切断。
- 测试调整：Rust CoT / sanitize +3；continue_beat system 断言同步。
- 全量基线：`cargo test --lib` 1466 passed / 2 ignored（+3）。

### v0.51.1 变更说明

- 幕前取消键 / 发射键卸掉系统原生按钮外观；契约锁 `appearance-none`。
- 测试调整：vitest +1；Rust 未重跑。
- 全量基线：`cargo test --lib` 1463 passed / 2 ignored（基线，本版无 Rust 逻辑变更）；`npx vitest run` 593 passed / 3 skipped（+1）。

### v0.51.0 变更说明

- 手写/粘贴正文观察：30s 同窗 Observe/Ingest/Skip；主创不改正文；观察 run 不撞 V109；编辑静默。
- 测试调整：Rust +9（1463）；vitest +1（592）。
- 全量基线：`cargo test --lib` 1463 passed / 2 ignored（+9）；`npx vitest run` 592 passed / 3 skipped（+1）。

### v0.50.2 变更说明

- 自动分章派生标题跟随章号；Append 不用分章前旧全文覆盖截断章；V130 修存量标题。
- 测试调整：Rust +4（1454）；vitest +1（591）。
- 全量基线：`cargo test --lib` 1454 passed / 2 ignored（+4）；`npx vitest run` 591 passed / 3 skipped（+1）。

### v0.50.1 变更说明

- 自动分章后续写：分页不含新章时补拉 `get_chapter_scenes`；`resolve_append_scene_id` 把 chapter id 解析成关联 scene。
- 测试调整：Rust +1（1450）；vitest 计数不变（590，改既有分章契约）。
- 全量基线：`cargo test --lib` 1450 passed / 2 ignored（+1）；`npx vitest run` 590 passed / 3 skipped。

### v0.50.0 变更说明

- 续写三角色闭环：活动 start/done 落库、资产栏投影、当前场大纲、审查进下一拍。
- 测试调整：Rust +13（1449）；vitest +2（590）。
- 全量基线：`cargo test --lib` 1449 passed / 2 ignored（+13）；`npx vitest run` 590 passed / 3 skipped（+2）。

### v0.49.1 变更说明

- 卸掉幕前划词浮条：删除 `AiSelectionActions` 及其 14 项组件测试；宿主测试改为断言浮条永不出现。
- 测试调整：vitest −17（588）；Rust 未重跑。
- 全量基线：`cargo test --lib` 1436 passed / 2 ignored（基线，本版无 Rust 逻辑变更）；`npx vitest run` 588 passed / 3 skipped（−17）。

### v0.49.0 变更说明

- 正文为大纲真相源：姓名门闩、场景结构归纳、熔断不挡续写、未接地书纲不注入、场外开篇探针。
- 测试调整：Rust +18（1436）。
- 全量基线：`cargo test --lib` 1436 passed / 2 ignored（+18）；`npx vitest run` 605 passed / 3 skipped。

### v0.48.1 变更说明

- 划词浮条：短选不出条、拖选结束才出、idle 无输入框、Esc 收起。
- 测试调整：vitest +4（605）；Rust 未重跑。
- 全量基线：`cargo test --lib` 1418 passed / 2 ignored（基线，本版无 Rust 逻辑变更）；`npx vitest run` 605 passed / 3 skipped（+4）。

### v0.48.0 变更说明

- 续写按镜头在场、节点落地当前席、旧快照不覆盖、连续续写先写入幽灵。
- 测试调整：Rust +5（1418）；vitest 计数不变（601，幽灵提交无新用例）。
- 全量基线：`cargo test --lib` 1418 passed / 2 ignored（+5）；`npx vitest run` 601 passed / 3 skipped。

### v0.47.0 变更说明

- 续写质量闭合 P0–P3：债务旗标、节点不回绕、别名/场次、BeatState 探针、八拍 mock 契约。
- 测试调整：Rust +22（1413）；vitest 未重跑。
- 全量基线：`cargo test --lib` 1413 passed / 2 ignored（+22）；`npx vitest run` 601 passed / 3 skipped（基线，本版无前端逻辑变更）。

### v0.46.0 变更说明

- 传统色主题：12 套纸帘印、幕前/幕后分选、旧四套迁移、gold=锚色、`--ai-accent-tint` 跟随当前窗。
- 测试调整：vitest +11（601）；Rust 未重跑。
- 全量基线：`cargo test --lib` 1391 passed / 2 ignored（基线，本版无 Rust 逻辑变更）；`npx vitest run` 601 passed / 3 skipped（+11）。

### v0.45.1 变更说明

- 续写前文：开篇+近文双窗、剥 HTML、在场只看近文、预算保章末。
- 测试调整：Rust +6（1391）；vitest 未重跑。
- 全量基线：`cargo test --lib` 1391 passed / 2 ignored（+6）；`npx vitest run` 590 passed / 3 skipped（基线，本版无前端逻辑变更）。

### v0.45.0 变更说明

- 提示词运行时组装：`assemble()` 金标 + 创世/续写/ToolLoop 接线；场景预览 Agency；三工具用法；内置模板 leftover CI。
- 测试调整：Rust +18（1385）；vitest 计数不变（590，默认场景断言改 `agency_continue`）。
- 全量基线：`cargo test --lib` 1385 passed / 2 ignored（+18）；`npx vitest run` 590 passed / 3 skipped。

### v0.44.1 变更说明

- 幕前输入框去掉 WKWebView 原生描边；契约锁 textarea class，不只查外壳。
- 测试调整：vitest +1（590）；Rust 未重跑。
- 全量基线：`cargo test --lib` 1367 passed / 2 ignored（基线，本版无 Rust 逻辑变更）；`npx vitest run` 590 passed / 3 skipped（+1）。

### v0.44.0 变更说明

- 墨纸补齐：输入无框、Medium 分文件、纸 chroma、选区 22%、顶栏 press、warm 内芯、Panel 高光、弹簧 500ms、侧栏去金框。
- 测试调整：vitest +11（589）；Rust 未重跑。
- 全量基线：`cargo test --lib` 1367 passed / 2 ignored（基线，本版无 Rust 变更）；`npx vitest run` 589 passed / 3 skipped（+11）。

### v0.43.0 变更说明

- 墨纸视觉进化：`AiPromptBar` flush、底栏无 pulse、本地文楷、EmptyHint、press 契约测试。
- 测试调整：vitest +22（578）；Rust 未重跑。
- 全量基线：`cargo test --lib` 1367 passed / 2 ignored（基线，本版无 Rust 变更）；`npx vitest run` 578 passed / 3 skipped（+22）。

### v0.42.0 变更说明

- 续写按拍选取资产：`continue_assets` 纯函数 + `write_beat_once`/`write_chapter`/`build_writer_context_from_db` 接线；章节大纲不再全表角色。
- 测试调整：Rust +13（1367）；vitest 未重跑。
- 全量基线：`cargo test --lib` 1367 passed / 2 ignored（+13）；`npx vitest run` 556 passed / 3 skipped（基线，本版无前端逻辑变更）。

### v0.41.2 变更说明

- 续写超时：网关 `candidate_fits_prompt` 跳过超窗候选；`write_beat_once` 散文失败不进 tool_loop；wrong-key 契约改直打 `write_chapter`。
- 测试调整：Rust +4（1354）；vitest 未重跑。
- 全量基线：`cargo test --lib` 1354 passed / 2 ignored（+4）；`npx vitest run` 556 passed / 3 skipped（基线，本版无前端逻辑变更）；`npx tsc --noEmit` ✅。

### v0.41.1 变更说明

- 续写上线核验：sanitize + 8% 重试、改写永不 TimeSliced/TriShot、划词不 Append、finalize 测试跳过 LLM、文思活跃 `scene_id`、Append 集成（同章不建行 + 立即释放 run）。
- 测试调整：Rust +5（1350）；vitest 仍 556 / 3 skipped。
- 全量基线：`cargo test --lib` 1350 passed / 2 ignored（+5）；`npx vitest run` 556 passed / 3 skipped；`npx tsc --noEmit` ✅。

### v0.41.0 变更说明

- Agency 唯一续写路径：`PersistMode::Append/NextChapter`、SceneBeatCard、Bundle 情感字段、按拍债务、写回出场/冲突/地点、切断 TimeSliced/TriShot 续写路由。
- 测试调整：Rust +17 契约测试（Append 落库门槛、smart_execute 续写路由、execute_writer 拒续写/创世等）；vitest 不变。
- 全量基线：`cargo test --lib` 1345 passed / 2 ignored（+17）；`npx vitest run` 556 passed / 3 skipped（不变）；`npx tsc --noEmit` ✅。

### v0.40.0 变更说明

- AI 原生组件库 P3（数据展示六件套：AiSearchList/AiCodeBlock/AiDiffTable/AiFilterTable/AiRecordsTable/AiInsightCards）+ P4（收尾：残留清理 + 视觉修正 + 浅色页令牌化）。
- 测试调整：vitest P3 +41（564）→ P4 −8（556，死件自带测试随删除移除）；Rust 无改动（1328 不变）。
- 全量基线：`cargo test --lib` 1328 passed / 2 ignored；`npx vitest run` 556 passed / 3 skipped（+33 vs v0.39.0）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` 全绿。

### v0.39.0 变更说明

- AI 原生组件库 P1（生成体验五件套）+ P2（代理与任务五件套）共 10 组件入库 `components/ui/ai/` 并接入落点；保存 UNIQUE 修复（scene 自愈补建重定向既有关联 scene + 序号避让）。
- 测试调整：vitest P1 +32（487）→ P2 +36（523）；Rust +2（`repositories_tests.rs` scene 自愈重定向/序号避让回归）。
- 全量基线：`cargo test --lib` 1328 passed / 2 ignored（+2）；`npx vitest run` 523 passed / 3 skipped（+68）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` 全绿。

### v0.38.2 变更说明

- 代理工作室实时动态持久化 + 前端轮询：新增 `agency_activity_log` 表（V129 迁移），`emit_activity` / `emit_progress` fire-and-forget 写 DB；新增 `agency_list_activities` 命令；前端 3s 轮询 + DB/live 合并去重。
- 测试调整：Rust +1（`test_log_and_list_activities`：log_activity x3 + log_progress x2 -> list_activities 返回 5 条，验证字段 + id ASC 顺序 + run_id 过滤）。
- 全量基线：`cargo test --lib` 1326 passed / 2 ignored（+1）；`npx vitest run` 455 passed / 3 skipped（无前端测试变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` 全绿。

### v0.38.0 变更说明

- 代理工作室实时显示修复与三 Agent 完善：agency 事件监听提升到常驻 `App.tsx` 顶层 + 新增全局 `agencyActivityStore`（cap 200）；三 Agent start/done 全路径配对；legacy 概念完成信号角色标注修复；后台质检实时推 `agency-board-changed`；幕前动词映射补全；幕后时间线业务键去重；续写熔断不丢稿流程级测试补齐（行为已由 65d90b5/v0.30.30 实现）。
- 测试调整：Rust +5（coordinator 事件信号配对 + 续写熔断流程级测试）；vitest +17（agencyActivityStore 单测 + AgencyStudio 订阅 store 重构 + 动词映射/时间线去重）。
- 全量基线：`cargo test --lib` 1306 passed / 2 ignored（+5）；`npx vitest run` 421 passed / 3 skipped（+17）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` 全绿。

### v0.37.0 变更说明

- 资产回流：提取 prompt 写作级升级（`memory_content_analysis.md`）；新增资产桥 `memory/asset_bridge.rs`（memory 层 → 生产资产表源感知 upsert，手工编辑永不覆盖）；Agency 续写落库后后台 `spawn_asset_ingest`；per-story 进程内锁 + `BACKGROUND_LLM_SEMAPHORE` 后台串行化；失败不致命。
- 测试调整：Rust +14（资产桥 upsert / 源感知合并 / 新角色注册 / 并发锁 / Agency 接入回归）。无前端逻辑变更。
- 全量基线：`cargo test --lib` 1301 passed / 2 ignored（+14）；`npx vitest run` 404 passed / 3 skipped（未动）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` 全绿。

### v0.30.46-48 变更说明

- 创世持久化链路审计修复（v0.30.46）：创世后补偿保存 / 场景装配原子化 + 空正文防护 / foreshadowing 落库 / item_type 别名归一化 / characters upsert。
- 角色谱静默失败 + llm_calls 空表（v0.30.47，issue #13/#14）：角色谱/文风/首场景健壮 JSON 解析；`prompt[..200]` 字节切片 panic 修复；向导防重入；拆书 4 处 toast 改 `extractMessage`。
- 向导策略加载误报 + 快速创作空输入确认（v0.30.48，issue #15）。
- 测试调整：`novel_creation.rs` +5 回归测试（markdown 围栏解析 / 缺字段不 panic / 缺 key 报错）；`materialize.rs` +3 测试（伏笔落库 / 别名 / upsert）；角色去重语义测试更新。
- 全量基线：`cargo test --lib` 1098 passed / 2 ignored；`npx vitest run` 352 passed / 3 skipped；`npx tsc --noEmit` ✅；`cargo +nightly fmt` 全绿。

### v0.30.45 变更说明

- 修复文思活跃模式续写提示词泄露（LLM 思维链泄露到正文）：①`llm/openai.rs` `resolve_content` 移除 `reasoning_content` 回退（`content` 为空即返回空，不再用 CoT 兜底）；②`max_tokens` 2048 -> 4096；③新增 `detect_and_strip_bare_cot`（≥3 条 CoT 信号行触发剥离）接入 `sanitize_novel_output` 后处理；④writer 提示词新增反推理指令（禁止输出思考过程/推理链）。
- 测试调整：新增 `detect_and_strip_bare_cot` 单元测试（+4：纯正文不剥离 / 纯 CoT 全量剥离 / 混合内容剥离 CoT 保留正文 / 不足 3 信号行不剥离）。无前端逻辑变更。
- 全量基线：`cargo test --lib` 1091 passed / 2 ignored（+4）；`npx vitest run` 352 passed / 3 skipped（无前端逻辑变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（539，零新增）/ prettier / architecture_guard 全绿。

### v0.30.44 变更说明

- 修复文思活跃模式续写报"生成过程异常结束，未收到有效内容"：移除 `handleRequestGeneration` 和 `handleSmartGeneration` 中 smartExecute resolve 后的提前 `smartExecuteInFlightRef.current = false`，改为在各内容交付退出路径统一清除 `smartExecuteInFlightRef` + `smartExecuteNeedDiagnosticRef`；活跃模式分支在打字机之前直接 `appendAiContent` 绕过打字机。纯前端修复，无 Rust 变更。
- 测试调整：新增 `FrontstageApp.wensi-active.test.tsx`（+2 测试：①活跃模式续写内容直接追加到编辑器正文不走打字机幽灵文本；②`smartExecuteNeedDiagnosticRef` 被清除不触发误报诊断）。RichTextEditor mock 修复：`getHTML()` 此前返回 stale `props.content`（appendText 后未更新），改为用 mutable ref 跟踪编辑器内部 HTML。
- 全量基线：`cargo test --lib` 1087 passed / 2 ignored（无 Rust 变更）；`npx vitest run` 352 passed / 3 skipped（+2）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（538，零新增）/ prettier / architecture_guard 全绿。

### v0.30.43 变更说明

- 修复续写内容丢失根因：`flushSceneSave` 改为直接读 `editorRef.getHTML()`（编辑器实际 HTML）而非滞后的 `latestContentRef`（200ms HTML 防抖窗口）；`onChapterUpdated` 新增守卫保护未保存内容 + 同步 `latestContentRef`。纯前端修复，无 Rust 变更。
- 测试调整：`FrontstageApp.restart-content.test.tsx` +1（close-flush 保存编辑器实际内容而非滞后 latestContentRef 回归测试）；3 个测试文件的 RichTextEditor mock 补 `getHTML` 方法。
- 全量基线：`cargo test --lib` 1087 passed / 2 ignored（无 Rust 变更）；`npx vitest run` 350 passed / 3 skipped（+1）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（538，零新增）/ prettier / architecture_guard 全绿。

### v0.30.42 变更说明

- 修复世界观生成失败（LLM 返回 markdown 代码块包裹的 JSON + 未转义引号 + 静默失败 + prompt 字段名不匹配）：①`agency/coordinator.rs::parse_lenient` 复用 `crate::narrative::extract_and_sanitize_json`（剥离围栏/修复裸换行/括号深度匹配），失败回退旧首尾花括号截取；②`agents/novel_creation.rs` 提取 `parse_world_options_response` 纯函数先剥离围栏再解析 + 失败时 `log::warn!` 记录片段；③两份 prompt 修正字段名（`concepts` -> `world_buildings`）+ 格式约束。
- 测试调整：`agency/tests.rs` +2（parse_lenient 剥离围栏 + 尾部杂散 `}` / 修复字符串内裸换行）；`novel_creation.rs` +3（干净 JSON / markdown 围栏包裹 / 缺 world_buildings 键报错守卫）。
- 全量基线：`cargo test --lib` 1087 passed / 2 ignored（+5）；`npx vitest run` 349 passed / 3 skipped（无前端逻辑变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（538，零新增）/ prettier / architecture_guard 全绿。

- 修复续写内容被假阳性去重静默丢弃（模型回显指令 + 短文本假阳性 + 内容丢失）：`isTextDuplicate` 新增最小长度守卫（归一化后 < 30 字符直接返回 false）；新增 `stripInstructionEcho` 剥离模型回显的用户指令前缀，在 `handleRequestGeneration` 和 `handleSmartGeneration` 的 `sanitizeContinuationOutput` 后调用。纯前端修复，无 Rust 变更。
- 测试调整：`isTextDuplicate.test.ts` +2（短文本假阳性守卫 + 长文本真阳性）；`textCleanup.test.ts` +7（`stripInstructionEcho` 7 场景：正常剥离/冒号分隔/不匹配不剥/短输入不剥/空输入不剥/剩余过短保留/长指令剥离）；更新 2 既有测试（前缀检测改用 ≥40 字符 + `isTextDuplicate` 用 ≥30 字符）。
- 全量基线：`cargo test --lib` 1082 passed / 2 ignored（无 Rust 变更）；`npx vitest run` 349 passed / 3 skipped（+13）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（538，零新增）/ prettier / architecture_guard 全绿。

- 修复代理工作室不显示活动记录数据（`activeRunId` 仅从事件捕获 + 无 `list_runs` 命令）：后端新增 `agency_list_runs` 命令 + `list_runs_for_story` repository 方法；前端 `AgencyStudio.tsx` 水合 `activeRunId` + 历史时间线重建 + run 选择器。
- 测试调整：`agency/repository.rs` +1 回归（`test_list_runs_for_story`：多 run 排序 / story_id 过滤 / limit / 空结果）；`AgencyStudio.test.tsx` +3（水合 activeRunId 显示黑板 / run 选择器多 option / 历史时间线从 board items 重建）；原有"渲染三角色状态卡与黑板空态"测试更新 mock（加 `listRuns`）。
- 全量基线：`cargo test --lib` 1082 passed / 2 ignored（+1）；`npx vitest run` 339 passed / 3 skipped（+3）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（538，baseline 540，-2 修复既有 `needless_question_mark`）/ prettier / architecture_guard 全绿。

### v0.30.39 变更说明

- 修复续写不按故事大纲推进剧情（TimeSliced 路径缺失 `build_progression_anchor`）：v0.30.31 引入的 `build_progression_anchor` 只在 TriShot 路径调用，从未移植到 TimeSliced 路径（默认续写路径）。修复：`execute_time_sliced` 的 prompt 模板后、`ending_anchor` 前插入 `build_progression_anchor` 调用，与 TriShot 对齐。
- 测试调整：无新增测试（`build_progression_anchor` 函数本身已由 v0.30.31 测试覆盖：`test_build_progression_anchor_directive_only_no_assets` + `test_build_progression_anchor_full_sections`）。本次为调用点接线修复，函数行为不变。
- 全量基线：`cargo test --lib` 1081 passed / 2 ignored（无新增）；`npx vitest run` 336 passed / 3 skipped（无前端变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（539，baseline 540 零新增）/ prettier / architecture_guard 全绿。

### v0.30.38 变更说明

- 修复续写输出被编辑器元评论污染（is_prose_request 被 serde 默认 false 导致 sanitize 跳过）：三层修复--①`intent.rs` `parse_classification_json` 后置不变量（续写/创世缺 `is_prose` 时强制设 `true`）；②`intent.rs` `build_classification_prompt` "继续写"示例补 `is_prose=true`；③`planner/mod.rs` `sanitize_plan_for_prose_request` 门控扩展为 `is_prose_request || is_continuation`。
- 测试调整：`intent.rs` +3 回归（续写缺 is_prose 后置纠正 / 创世缺 is_prose 后置纠正 / 改写缺 is_prose 保持 false）；`planner/mod.rs` +1 回归（is_continuation=true + is_prose_request=false 仍触发净化塌缩）。更新 `test_parse_classification_json_lenient_prose_affix`（is_continuation 改 false 避免与后置不变量冲突）。
- 全量基线：`cargo test --lib` 1081 passed / 2 ignored（+4）；`npx vitest run` 336 passed / 3 skipped（无前端变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（539，baseline 540 零新增）/ prettier / architecture_guard 全绿。

### v0.30.37 变更说明

- 修复创作生成失败时 toast 显示 "[object Object]"（issue #12）：10 个前端文件共 36 处 catch 块的 `String(err)` / `instanceof Error ? .message : String(err)` / `?.message || String(err)` 统一替换为 `extractMessage(err)`（`src/utils/errorHandler.ts`）。覆盖 `FrontstageApp`（smart_execute 主/次 + 修稿/审稿/定稿）/ `SceneEditor`（生成大纲/草稿）/ `Stories`（快速创作/向导创作/风格保存/风格生成）/ `RichTextEditor`（文思生成/排版）/ `WenSiPanel`（自动续写/修改）/ `usePipeline`（6 处）/ `CharacterStatePanel` / `Skills`（7 处）/ `PromptsPanel`（5 处）/ `useUpdater`。不动 `main.tsx`/`ErrorBoundary.tsx`（已优先取 `.message`）。
- 测试调整：新增 `src/utils/__tests__/errorHandler.test.ts`（+8：AppError 普通对象提取 `message` 断言不等于 `[object Object]` / 带 `data` / `parseStructuredError` 识别 / `Error.message` 内嵌 JSON / 普通 Error / 字符串 / 带 `.message` 对象 / 兜底文案）。
- 全量基线：`cargo test --lib` 1077 passed / 2 ignored（纯前端无 Rust 变更）；`npx vitest run` 336 passed / 3 skipped（+8）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（539，纯前端无 Rust 变更 baseline 不变）/ prettier / architecture_guard 全绿。

### v0.30.36 变更说明

- 修复首次创世指令不保存到输入历史：`FrontstageApp.tsx` `handleSmartGeneration` 的 `story_created` 处理块在 `setCurrentStory(新故事)` 后同步 `saveInputHistory(新故事ID, [创世指令, ...])`，useEffect 随后加载即可读到。纯前端修复。
- 测试调整：新增 `FrontstageApp.input-history-genesis.test.tsx`（+2：创世指令持久化到新故事 localStorage + 切换到新故事后按↑召回创世指令）。
- 全量基线：`cargo test --lib` 1077 passed / 2 ignored（纯前端无 Rust 变更）；`npx vitest run` 328 passed / 3 skipped（+2）；`npx tsc --noEmit` ✅；prettier / architecture_guard 全绿。

### v0.30.35 变更说明

- editor 质检后台异步化：`agency/coordinator.rs` 新增 `assemble_only`（pub(crate)，纯装配落库）+ `spawn_editor_qc`（后台 `tokio::spawn`，独立 300s deadline，`app_handle=None` 时 no-op）；`genesis_fastpath` / `run_genesis_legacy_inner` Phase C 改为 `assemble_only` + `spawn_editor_qc`，返回 `verdict:EditorVerdict::pending()`；删除 `review_and_assemble`；`EditorVerdict` 加 `pending()`；新增 `EVENT_GENESIS_QC_RESULT`。前端 `FrontstageApp.tsx` 新增 `genesis-qc-result` 监听 + 三态 toast。
- 测试调整：新增 `test_editor_verdict_pending_defaults` + `test_assemble_only_persists_scene_without_qc`（Rust，+2）；移除 3 个已不适用的 genesis 同步质检测试（`test_genesis_revision_path` / `test_genesis_aborts_when_editor_aborted` / `test_gate_fails_after_verdict_parse_retry`）；`test_editor_verdict_prose_fallback` 由 genesis-based 改为直接测 `evaluate_gate` 保留 prose-fallback 覆盖；`test_fastpath_single_model_producer_first` 调用次数 `>= 4` -> `>= 3`（editor 后台化）；新增前端 `FrontstageApp.genesis-qc.test.tsx`（+4：事件注册 + passed/salvaged/failed 三态 toast）。
- 全量基线：`cargo test --lib` 1077 passed / 2 ignored（+2 净增）；`npx vitest run` 326 passed / 3 skipped（+4）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（539，baseline 540 零新增）/ prettier / architecture_guard 全绿。

### v0.30.34 变更说明

- 序列化场景持久化：`FrontstageApp.tsx` 新增 `saveChainRef` + `persistSceneContent` Promise 链序列化所有 `update_scene`；`flushSceneSave` / `handleContentChange` saveFn / 保护性保存统一走此函数；`handlePipelineRefine` `setContent` + `onReviseResult` `insertText` 补 `latestContentRef` 同步 + `flushSceneSave`；`lib.rs` close 超时 3s -> 6s。
- 全量基线：`cargo test --lib` 1078 passed / 2 ignored（无新增后端测试）；`npx vitest run` 322 passed / 3 skipped；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（baseline 540 零新增）/ prettier / architecture_guard 全绿。

### v0.30.33 变更说明

- 修复关闭应用时续写内容丢失：`lib.rs` `graceful_shutdown` 加 `AtomicBool` 幂等守卫 + 新增 `graceful_quit` 命令 + `CloseRequested` 改 `api.prevent_close()` + emit `frontstage-flush-requested` + 3s 超时兜底；`FrontstageApp.tsx` 新增 `flushSceneSave` 共享落库函数 + close-flush 监听 effect + `appendAiContent` 改立即落库 + `selectChapter` 切换前 flush。
- 全量基线：`cargo test --lib` 1078 passed / 2 ignored（无新增后端测试，无逻辑变更影响现有测试）；`npx vitest run` 322 passed / 3 skipped（无前端逻辑变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（baseline 540 零新增）/ prettier / architecture_guard 全绿。

- 增强性指令纳入世界观/故事大纲/场景大纲/上下文强关联：`commands/orchestrator.rs` `build_logline_context_sync` 新增 `world_setting`（拉 world_buildings concept+rules前3+history）；`agents/orchestrator.rs` `build_progression_anchor` 加 `user_instruction` 参数 + 显式调和指令（资产=硬约束/指令=创作方向）；`agency/coordinator.rs` 创世 `writer_first_chapter`/`writer_prose_fallback` 调和；2 份 prompt 资产（`agency_logline_suffix_contextual.md`/`orchestrator_timesliced_writer.md`）。
- 后端测试变更：`test_build_progression_anchor_injects_all_sections` 扩展断言指令段 + 调和指令（本次创作指令/在硬约束内落实指令核心意图/保留指令核心意图）；`test_build_progression_anchor_empty_returns_empty` 改传空指令；新增 `test_build_progression_anchor_directive_only_no_assets`（仅有指令无资产边界：断言"推进剧情向前发展"且不含"硬约束"调和措辞）。
- 全量基线：`cargo test --lib` 1078 passed / 2 ignored（+1）；`npx vitest run` 322 passed / 3 skipped（无前端逻辑变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（baseline 540 零新增）/ prettier / architecture_guard 全绿。

### v0.30.31 变更说明

- 续写链路修复（世界观/故事大纲/场景大纲注入与剧情推进方向）：`orchestrator.rs` `build_progression_anchor` + `write_time_bundle.rs`/`manifest.rs`/`creation_commands.rs`/`service.rs` 注入层 + `coordinator.rs` Agency 注入函数 + 4 份 prompt 资产。
- 后端测试变更：新增 `test_build_progression_anchor_injects_all_sections`（预置 story + 2 场景 outline_content + bundle 含 story_outline/world_setting/scene_outline -> 输出含剧情推进方向/故事大纲/本章场景大纲/已推进进度/世界观核心规则/不得原地踏步）与 `test_build_progression_anchor_empty_returns_empty`（空 bundle 无场景 -> 空串）；`test_ensure_world_building_generates_when_missing` 断言更新（concept 现存全文含历史背景，history 不再单独冗余存储，断言改为 concept 含"星环崩塌"）；`test_batch_parallel_two_chapters` 因 `generate_chapter_outline` 恢复"无故事大纲时短路"保持并发时序通过。
- 全量基线：`cargo test --lib` 1077 passed / 2 ignored（+2）；`npx vitest run` 322 passed / 3 skipped（无前端逻辑变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（baseline 540 零新增）/ prettier / architecture_guard 全绿。

### v0.30.30 变更说明

- Agency 创作链路结构性优化（抗重复闭环 + 质量门宽松度 + 熔断不丢稿）：`agency/coordinator.rs` D1/D2/E1/E2/E3 五点修复。
- 后端测试变更：新增 4 项单测--`test_model_grader_scoreless_pass_below_threshold`（E1：scoreless pass -> 0.7 < 0.75 阈值，scored 4.5 -> 0.9）、`test_salvage_failed_gate`（E2：长稿 ≥600 -> Some(pass)，短稿 -> None，边界 600 字符 -> Some）、`test_cleanup_prose_for_persist_trims_self_repetition`（D1：双段落自重复 -> 清理后仅 1 段）、`test_continue_writer_maxturns_board_recovery`（E3：writer 10 次 board_write 后 MaxTurns 熔断 -> 从黑板取回草稿 run 完成）；现有 scoreless pass verdict 统一加 `"score":4.5`（E1 使 model_score 0.9 > 旧 0.85 保证原过门测试不退化），`test_verdict_legacy_format_fallback` 断言 0.85 -> 0.7。
- 全量基线：`cargo test --lib` 1069 passed / 2 ignored（+4）；`npx vitest run` 322 passed / 3 skipped（无前端变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（baseline 540 零新增）/ prettier / architecture_guard 全绿。

### v0.30.29 变更说明

- 内容质量根因修复（强模型结构化大纲不再被丢弃 + 大纲/世界观约束到生成链路）：`agency/coordinator.rs` 五点修复。
- 后端测试变更：新增 `depth_assets_outline_tests` 模块 5 项（`normalize_outline` 对象/字符串/空/部分/未知 fallback）；`test_build_continue_writer_context` 扩展 MASTER_SETTING 红线注入断言（红线在 ctx 头部、先于角色段）；`fastpath_script` 等 8 项编排测试适配 P1 串行 producer-first 调用序（mock 队列 concept -> depth -> writer -> editor，取消测试 `fire_on` 2 -> 3）。
- 全量基线：`cargo test --lib` 1065 passed / 2 ignored；`npx vitest run` 322 passed / 3 skipped（无前端变更）；`npx tsc --noEmit` ✅；`cargo +nightly fmt` / clippy（baseline 540 零新增）/ prettier / architecture_guard 全绿。

### v0.30.28 变更说明

- UI 双模式设计系统重塑（幕前墨纸 / 幕后机械）+ 落地页下载从 `latest.json` 自动同步 + 幕前交互打磨。
- 前端测试变更：移除 `useGhostChrome` hook 及其 6 项单元测试（ghost-chrome 静止蒙版下线）；`FrontstageBottomBar` logline 内联幽灵测试全绿。
- 全量基线：`cargo test -p storymoss` 1060 passed（无 Rust 变更）；`npx vitest run` 322 passed / 3 skipped；`npx tsc --noEmit` ✅；fmt / clippy（baseline 549 零新增）/ prettier / architecture_guard 全绿。

### v0.30.26 变更说明

- 统一 Logline 增强提示为内联幽灵文本 + 修复分时预检缺少角色：
  - 前端 `FrontstageBottomBar.tsx` 改为内联幽灵后缀渲染，移除旧的 `.frontstage-logline-hint` 建议条；`FrontstageApp.tsx` 按 `→` 追加后缀逻辑简化，移除 `originalInputForLoglineRef` / `intentClassificationInput` 透传。
  - 后端 `commands/orchestrator.rs` 的 `generate_logline_hint` 改用新增 `agency_logline_suffix` prompt，返回追加后缀。
  - 后端 `intent.rs` 兜底路径增加按输入文本判断创世意图；`story_system/preflight.rs` 的 `QuickPreflightChecker` 在角色表为空时自动创建占位主角。
- 新增 prompt 资产：`resources/prompts/agency/agency_logline_suffix.md`。
- 更新 vitest 测试：`FrontstageBottomBar.test.tsx` 的 logline 相关断言改为验证内联幽灵文本。
- 全量基线：`cargo test -p storymoss` 1060 passed；`npx vitest run` 310 passed / 3 skipped；`npx tsc --noEmit` ✅；fmt / clippy / prettier / architecture_guard 全绿。

### v0.30.25 变更说明

- 续写 600s 超时修复（三层根因）：`openai.rs` 新增 `reasoning_content` 字段 + `resolve_content` 纯函数 fallback（DeepSeek 推理模型空 content）；`auto_contract.rs` 4 个 `build_*` 调用各包 30s timeout；`FrontstageApp.tsx` 续写时后台 fire-and-forget auto_contract。
- 新增 5 个 Rust 测试：`resolve_content` fallback 场景（空 content + reasoning_content / content 非空不 fallback / 空 reasoning_content / Message 反序列化 reasoning_content / OpenAiDelta 反序列化）。
- 全量基线：`cargo test --lib` 987 passed（+5）；`npx vitest run` 311 passed / 3 skipped；fmt / clippy（baseline 549）/ tsc / prettier / architecture_guard 全绿。

### v0.30.24 变更说明

- Logline 幽灵提示（用户输入简单创世指令时实时生成增强版 logline）：`commands/orchestrator.rs` 新增 `generate_logline_hint` 命令 + 纯函数 `should_skip_logline_generation` / `is_valid_logline`；`FrontstageApp.tsx` 新增 logline state + 1.5s 防抖 + `->` / `Esc` 键盘处理；`FrontstageBottomBar.tsx` 新增建议条渲染 + CSS。
- 新增 4 个 Rust 测试：`test_should_skip_logline_generation_empty_input` / `test_should_skip_logline_generation_long_input` / `test_should_skip_logline_generation_normal_input` / `test_is_valid_logline`。
- 新增 4 个 vitest 测试：logline 渲染 / loading 提示 / 点击接受 / 空输入不渲染。
- 全量基线：`cargo test --lib` 982 passed（+4）；`npx vitest run` 311 passed / 3 skipped。

### v0.30.23 变更说明

- 意图分类 Bug 修复（LLM 分类去偏 + 失败兜底上下文化）：`intent.rs` `build_classification_prompt` 移除 `已有故事={story}` 上下文注入（偏差来源）+ 移除保守措辞 + 新增 3 正例；新增 `conservative_fallback_with_context(has_existing_story)`（LLM 失败时无故事->创世/有故事->续写）；仅缓存 LLM 成功结果不缓存兜底；缓存键简化为仅 `user_input`。`FrontstageApp.tsx` 两处 LLM 失败兜底上下文化（`stories.length === 0`）。
- 新增 4 个回归测试：`test_conservative_fallback_no_story_is_genesis`（无故事兜底->创世）/ `test_conservative_fallback_with_story_is_continuation`（有故事兜底->续写）/ `test_classification_prompt_no_context_bias`（提示词无 DB 状态偏差）/ `test_classification_prompt_has_positive_examples`（提示词含正例）。
- 全量基线：`cargo test --lib` 978 passed（+4）；`npx vitest run` 307 passed / 3 skipped。

### v0.30.22 变更说明

- PROBLEM 七元素框架集成（Logline 生成 + 故事大纲增强）：引入 Erik Bork 的 PROBLEM 七元素作为后端创作资产。`coordinator.rs` 新增 `generate_logline`（简单 premise < 100 字触发 PROBLEM logline 生成）；`run_genesis_inner` 在 concept_pack 前注入 logline Producer LLM 调用并落库（V114 迁移 `ALTER TABLE stories ADD COLUMN logline TEXT`，Story 模型加 `logline: Option<String>`，`StoryRepository::update_logline`）；`ensure_story_outline` 改用注册表 PROBLEM outline 提示词，`build_continue_writer_context` 注入【故事Logline】，`producer_depth_assets` 增强 PROBLEM 指导；提示词资产 `resources/prompts/agency/agency_problem_logline.md` / `agency_problem_outline.md` 经 WalkDir 自动注册。
- 新增 3 个 logline 回归测试：`test_generate_logline_from_simple_premise`（简单 premise 触发 PROBLEM logline 生成）/ `test_generate_logline_skipped_for_long_premise`（长 premise ≥ 100 字跳过生成）/ `test_logline_stored_after_genesis`（创世后 logline 落库 `stories.logline`）。
- 全量基线：`cargo test --lib` 974 passed（+3）。

### v0.30.21 变更说明

- 续写资产层级生成：`ensure_assets` 扩展检查 world_buildings / story_outlines，缺失时调 `ensure_world_building` / `ensure_story_outline` 单次 Producer LLM 调用生成并落库。`build_continue_writer_context` 注入故事大纲。`generate_chapter_outline` 在 writer tool_loop 前生成章节大纲。`handle_gate` 存 `scenes.outline_content`。新增 `test_ensure_world_building_generates_when_missing` + `test_ensure_story_outline_generates_when_missing` + `test_generate_chapter_outline` + `test_generate_chapter_outline_skips_without_story_outline`。更新现有续写测试预置 world_buildings + story_outlines + 章节大纲 mock 响应。`cargo test --lib` 971 passed。

### v0.30.20 变更说明

- Agency 续写效率优化：续写 `run_continue`/`run_continue_batch` 加 `setup_run_deadline`（tool_loop 超时保护）；`write_chapter` 加散文回退（`writer_prose_fallback` 参数化 `chapter_key`）+ 上下文预注入（`build_continue_writer_context` 读 DB 角色/世界/场景）；Editor 质量门加 deadline + 草稿预注入；`llm_connect_timeout_secs` 默认 60s -> 15s。新增 `test_continue_writer_prose_fallback`（续写 writer 熔断 -> 散文回退 -> 章节成功）+ `test_build_continue_writer_context`（DB 资产预注入上下文非空）。`cargo test --lib` 967 passed。

### v0.30.19 变更说明

- 修复质量门编辑审计 Agent 熔断（本地模型 JSON 不遵从）：`evaluate_gate_impl` 中 editor tool_loop 连续解析失败/达最大轮数熔断时原直接 Failed。Fix：salvage（熔断时 `parse_lenient` 提取末轮裁决）+ `editor_verdict_prose_fallback`（单次 `llm.complete()` 直接请求裁决 JSON，与 `writer_prose_fallback` 同理）。新增 `test_editor_verdict_prose_fallback` 正向回归（editor 熔断 -> 散文回退产出 pass 裁决 -> run completed）；2 个现有熔断测试更新为显式验证回退也失败时 run 仍 failed。`cargo test --lib` 965 passed。

### v0.30.18 变更说明

- 修复幕前意图分类 null 崩溃：`handleSmartGeneration` 调 `classifyIntent` 后 `classification.is_new_novel` 读 null 崩溃（v0.30.16 CI E2E 根因）。E2E mock（`e2e/mock-tauri.ts`）对未注册命令返回 null 触发。Fix：post-catch null 兜底 + 不缓存 null。无新增单测（null 兜底由现有 genesis-duplicate 14 项 vitest 覆盖回归）。
- macOS 构建失败（v0.30.16 tag）：`Info.plist Io(code 5)` 为 GitHub runner 瞬时 I/O flake，已 `gh run rerun --failed` 重建；E2E 为 `continue-on-error` 非门禁。
- 全量基线：`npx vitest run` 307 passed / 3 skipped；`npx tsc --noEmit` ✅；`npm run format:check` ✅。纯前端无 Rust 变更，cargo 基线 964 passed 不变。

### v0.30.17 变更说明

- 幕前顶部创世状态显示三 Agent 动作/进度：新增 `useAgencyAgentActivity` hook 订阅 `agency-agent-activity`/`agency-run-progress` 事件；FrontstageHeader 顶部状态栏渲染 主创/管理/编辑审计 进度（进行中琥珀 saving、已完成绿色 saved），run 结束清空。
- 测试：FrontstageHeader.test.tsx 新增 `@tauri-apps/api/event` 回调捕获 mock + 2 用例（三 Agent 进度渲染 / run 结束清空）。
- 全量基线：`npx vitest run` 307 passed / 3 skipped（+2）；`npx tsc --noEmit` ✅；`npm run format:check` ✅。纯前端无 Rust 变更，cargo 基线 964 passed 不变。

### v0.30.16 变更说明

- 故事资产手动编辑：故事大纲（Stories.tsx 查看/编辑 UI，调 useUpdateStoryOutline）、故事摘要（KnowledgeGraph.tsx SummaryCard 编辑，调 useUpdateStorySummary）、伏笔内容编辑+删除（后端 ForeshadowingTracker update/delete 方法 + 命令 + 注册；前端 useUpdate/DeleteForeshadowing hook + Foreshadowing.tsx 编辑表单/删除）、角色关系编辑（useUpdateCharacterRelationship hook + RelationshipCard 编辑表单）。
- 测试：更新 Characters.test.tsx mock 补 useUpdateCharacterRelationship 导出。
- 全量基线：`cargo test --lib` 964 passed；`npx vitest run` 305 passed / 3 skipped；tsc / fmt / clippy（零新增，baseline 550）/ architecture_guard 全绿。

### v0.30.15 变更说明

- 场景围绕故事大纲生成（创作原则加固）：根因 A 场景大纲生成 `generate_scene_outline` 复用故事级 outline_planner 提示词且不注入 story_outlines.content 致幻觉新角色"金敏秀"；根因 B writer（TimeSliced/TriShot）prompt 从不包含故事大纲致续写偏离。Fix A：新增场景级提示词 `scene_outline.md`（强制复用已登场角色、禁止发明新角色、围绕故事大纲节点）+ `generate_scene_outline` 注入故事大纲 + `build_outline_prompt` 分流；Fix B：`WriteTimeBundle` 加 `story_outline` 字段 + `to_prompt` 红线后插入权威段【故事大纲（必须围绕展开）】，一处覆盖两条 writer 路径，冲突时以故事大纲为准。
- 新增 4 个回归测试：WriteTimeBundle `to_prompt` story_outline 渲染 / 红线-故事大纲顺序不变量 / story_outline 缺失不渲染；registry `scene_outline` 提示词注册（含"禁止发明新角色"指令）。
- 全量基线：`cargo test --lib` 964 passed（+4）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。

### v0.30.14 变更说明

- 续写返回风格增强模板修复（多步 plan 尾部非 writer 覆盖正文，第 5 次复发）：`execute_plan`（executor.rs:685-687）用最后产出 `content` 的步骤作为 `final_content`，force-correction（防线 2）只修正首步无法拦截尾部 style_enhancer/inspector，其模板/报告覆盖 writer 正文；新增防线 3 `PlanGenerator::sanitize_plan_for_prose_request` 在咽喉点（force-correction 之后）对所有 `is_prose_request` plan 净化（移除 builtin.style_enhancer/text_formatter/character_voice/emotion_pacing；续写塌缩单 writer；其余弹出尾部非 writer 保证末步 writer；空则补 writer），保留 [inspector, writer] Rule 9 流，非 prose（Audit）不净化。
- 新增 12 个 sanitize 回归测试（`sanitize_plan_for_prose_request`：inspector+style_enhancer 多步、style_enhancer 单步、writer+style_enhancer、inspector+writer Rewrite 保留、续写多步塌缩、续写单 writer 不变、Audit 不净化、outline+writer 保留、outline 单步补 writer、空 plan、无分类不净化、净化 writer 步带 instruction/current_content/story_id）。
- 全量基线：`cargo test --lib` 960 passed（+12）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。

### v0.30.13 变更说明

- 续写返回风格增强模板修复（SING 路径绕过 force-correction）：SING（IntentionGraphPlanner）路径直接返回 plan、绕过 `PlanGenerator::generate_plan` 内的防线 2，续写被 SING 路由到 `builtin.style_enhancer` 返回"请提供需要增强的原始文本"模板；提取 `PlanGenerator::force_correct_first_step_to_writer`（`pub(crate)`，封装 swap + understanding/purpose 标注）在 plan 执行咽喉点 `execute_with_context`（`execute_plan` 之前，所有 plan 来源 SING/PlanGenerator/fallback 必经）统一施加，幂等。
- 新增 4 个咽喉点 force-correction 回归测试（`force_correct_first_step_to_writer`：SING style_enhancer 修正、writer 幂等不改动、空 plan 不 panic、inspector 续写修正）。
- 全量基线：`cargo test --lib` 948 passed（+4）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550 -> 549）。

### v0.30.12 变更说明

- planner force-correction 覆盖 inspector（修复续写误路由到 inspector 返回质检报告）：提取纯函数 `PlanGenerator::should_force_correct_to_writer`（可单测），将 inspector 纳入 swap-to-writer 列表，按 LLM 分类分流（续写/创世/无分类/审查+prose 强制 writer；纯 Audit(非prose)/Rewrite 保留 inspector）；提示词 Rule 9 澄清续写≠refine、Rule 21 加入 inspector 禁用。
- 新增 8 个 force-correction 回归测试（`should_force_correct_to_writer`：inspector 续写/审查/改写/创世/无分类分流）。
- 全量基线：`cargo test --lib` 944 passed（+8）；`npx vitest run` 305 passed；tsc / fmt / clippy / format 全绿。

### v0.30.11 变更说明

- 意图识别 LLM 化重构：用 `IntentParser::classify_writing_intent`（`src-tauri/src/intent.rs`）单次 LLM 调用产出 `WritingIntentClassification`，替换 6 处高风险朴素子串匹配点（`is_novel_creation_intent`、`find_template` 禁用、`from_instruction_and_context` 优先级 bug 修复 + hint 参数、force-correction 读 `is_prose_request`、`extract_genre` 否定+排序、intention_graph builder LLM 加固）；新增 `classify_intent` Tauri 命令与前端 `classifyIntent` IPC。
- 新增测试：`parse_classification_json` 解析单测覆盖合法/容错路径；`is_prose` 别名回归测试锁定 force-correction 路径读取 `is_prose_request`。
- 全量基线：`cargo test --lib` 936 passed（+7）；`npx vitest run` 305 passed（+8）；tsc / fmt / clippy / architecture_guard 全绿。

### v0.30.10 变更说明

- 续写返回风格增强模板修复（模板匹配误路由 + content 空兜底注入）：新增 5 个 Rust 单测（`inject_content_fallback` 5 场景：depends_on 注入 / content 已存在跳过 / 无 outputs 时用 current_content / 无 content 返回 false / 优先 outputs 胜过 current_content）；executor.rs 续写意图跳过模板匹配；mod.rs force-correction 扩展到 style_mimic/plot_analyzer/builtin；Rule 21 强化。
- 全量基线：`cargo test --lib` 929 passed（+5）；fmt / clippy 无新增告警。无前端变更。

### v0.30.9 变更说明

- 续写返回 Inspector 审查模板修复（inspector draft 空内容兜底注入）：新增 5 个 Rust 单测（`inject_inspector_draft_fallback` 5 场景：depends_on 注入 / draft 已存在跳过 / dep 不存在扫描兜底 / 无 content 返回 false / 空 content 跳过）；planner 提示词增加 inspector `draft={{step_N}}` JSON 示例与 Rule 9 强化。
- 全量基线：`cargo test --lib` 924 passed（+5）；fmt / clippy 无新增告警。无前端变更。

### v0.30.8 变更说明

- 全面修复 nullable 列读取（`Invalid column type Null` 系列）：新增 2 个 Rust 单测（world_buildings cultures/rules NULL 兜底 + 合法 JSON 解析）；系统性修复 8 个 repository 文件 31 处 nullable 列读取（scene/version/studio/writing_style/kg/user_pref），全部改为 `Option<String>` + 兜底；V112/V113 迁移回填所有 nullable JSON/TEXT 列。
- 全量基线：`cargo test --lib` 919 passed（+2）；fmt / architecture_guard 全绿。无前端变更。

### v0.30.7 变更说明

- 计划执行失败修复（LLM 在 depends_on 写入上下文名）：新增 2 个 Rust 单测（`topological_sort` 非 step_id 依赖跳过 + 混合依赖排序），验证 executor 依赖校验与 `topological_sort` 行为一致。
- 全量基线：`cargo test --lib` 917 passed（+2）；fmt / tsc / architecture_guard 全绿。无前端变更。

### v0.30.4 变更说明

- 幕前输入历史持久化（按故事隔离）：新增 2 个 vitest（持久化写入 + 重载后 ↑ 召回），覆盖 localStorage 读写与故事隔离。
- 全量基线：`npx vitest run` 297 passed（+2）；`npx tsc --noEmit` 通过；`prettier --check` 通过。无 Rust 变更。

### v0.30.0 变更说明

- Agency 多代理创作框架（创世 2.0）P1–P5 全部完成，测试体系包括：`src-tauri/src/agency/` 26+ 单元测试（coordinator / gate / graders / board / budget / session / learning / eval_harness 等）；eval harness JSON 场景随 `cargo test` 运行；CI 另设 `cargo test --lib agency::eval_harness` 专用门禁 step。
- 全量基线：`cargo test --lib` 877 passed；`npx vitest run` 295 passed。

### v0.26.59 变更说明

- 新增官网落地页组件测试（landing）：`DownloadButton` 平台检测与链接断言 8 tests；全量 landing 19 passed。
- 无 Rust/前端应用逻辑变更，全量基线保持 `cargo test --lib` 770 passed、`npx vitest run` 292 passed。

### v0.26.57 变更说明

- 新增 `chapter_splitter` 单元测试 7 passed（mode_parse、resolve_max_chars、word_count/plot 切分边界）。
- 新增 `export::assemble` 单元测试 8 passed（scenes 为真相源、孤儿场景、标题回退）。
- 新增 `prompts::registry` 测试：目录解析、场景组合预览。
- 前端新增 `useExport.test.ts` 4 passed（取消保存、文本/二进制处理、空内容拒绝）。
- 前端 `PromptsPanel.test.tsx` 5 passed（加载、展开编辑器、导入参数、打开目录、组合预览）。
- 全量基线：`cargo test --lib` 769 passed；`npx vitest run` 292 passed。

### v0.26.56 变更说明

- executor 写 AppConfig 契约测试加 `mock_app_config_lock`，并行 `--test-threads=8` 稳定。

### v0.26.55 变更说明

- 新增 `ModelCard.enabled.test.tsx`（开启/关闭开关契约）。
- Rust：`apply_disable_side_effects` / `disabled_model_excluded_from_gateway_registry` / `test_probe_model_rejects_missing_or_disabled` / `test_disabled_model_not_selected_after_registry_reload`。

### v0.26.54 变更说明

- `clear_demotion` / `demoted_degraded_creative_still_promoted` / `auto_clears_sticky_unhealthy_creative` / `user_sets_creative_x_overrides_demoted_y` / `sync_creative_to_active_llm` 契约通过。

### v0.26.53 变更说明

- `FrontstageHeader`：单击故事名不得调用 `onOpenBackstage`；设置按钮可回幕后；双击仍进编辑。

### v0.26.52 变更说明

- `include_in_gateway_status` / `is_promotable_user_model` / `sync_creative_to_active_llm` 契约 4 passed。
- `useSyncStore.bug.spec`：`model_config`/`app_settings` 失效含 `gateway-status` 5 passed。

### v0.26.51 变更说明

- `displayStoryTitle` / `displayChapterTitle` 展示契约；`FrontstageHeader` / `EditableChapterTitle` 双击改名交互 30 passed。

### v0.26.50 变更说明

- `story_system::scene_service`：AutoIngest 防抖窗口契约 6 passed（含 debounce=commit 同窗）。
- `useBackendActivityListener.contract`：contract-auto-progress 不得注册 running 2 passed。

### v0.26.49 变更说明

- `agents::orchestrator`：`last_n_sentences` / `build_ending_anchor` / 纪律后置序契约 3 passed。

### v0.26.48 变更说明

- `updater::tests`：下载进度累加 + 404 错误文案契约 2 passed。
- CI：`verify-updater-manifest` 在 tag 发布后校验 `latest.json`。

### v0.26.47 变更说明

- 无测试变更；`cargo +nightly fmt -- --check` 恢复通过。

### v0.26.46 变更说明

- `background_generate_templates_declare_strategy_section`：5 个 externalized 模板契约。
- `normalize_methodology_id` / `final_methodology_step_after_genesis` / Genesis strategy notes 注入契约。
- 拆书 chunker 与 StoryArc 持久化相关单测（见 commit 5a5c9b1）。

### v0.26.45 变更说明

- `narrative::protagonist_card`：merge/render/probe/soft_retry 契约 6 passed。
- Genesis first_scene 增加 `protagonist_card` 变量；Call3 尾注注入。

### v0.26.44 变更说明

- Genesis `quick_phase_steps` 契约更新为四步（含「铺设开篇骨架」）；新增 `parse_opening_skeleton` / `opening_skeleton_from_concept` 契约测试。
- `extract_story_meta_fallback` 覆盖加厚字段（protagonist_name / core_conflict / world_one_liner）。

### v0.26.43 变更说明

- StatusIcon / FrontstageBottomBar：emoji→Lucide + 状态解析修复（+4）。
- vitest 全绿。

### v0.26.42 变更说明

- RichTextEditor：接受后 30s 内新续写须显示幽灵段落（+1）。
- `RichTextEditor.duplicate.test.tsx` 6 passed。

### v0.26.41 变更说明

- finalize scene_id 直写单测 3；MemoryFacade unified facts 7；V104–V106 迁移。
- `cargo test --lib` 701 passed。

### v0.26.40 变更说明

- Sidebar impact badges / 诊断默认折叠；SceneEditor pipeline rail；PromptCoverageBar；MemoryFacade KG top-5。
- `cargo test --lib memory::facade` 5；相关 vitest 15+。

### v0.26.39 变更说明

- Sidebar 五组 IA + Insights 三 Tab vitest；`writing-stats` 重定向契约。
- vitest 249 passed（+5）。

### v0.26.38 变更说明

- PromptsPanel：展开编辑器 / 打开目录 / 组合预览 vitest；framework_guidance + preview_prompt_composition Rust 单测。
- `cargo test --lib` 690 passed（+5）；vitest 244 passed（+2）。

### v0.26.37 变更说明

- `updateSceneIpc` 契约测试；幕前自动保存参数形状锁定。

### v0.26.36 变更说明

- 配置热同步：`app_settings` sync、editor/theme Tauri 事件；vitest +3（useEditorConfig / useSyncStore）。

### v0.26.35 变更说明

- Dashboard `scene_count` 单测数据对齐；幕后导航统一 store；`apply_wizard_to_story` 为新增 IPC（跨层）。
- 前端 R7–R11 为 UI/导航改动，以 `tsc` + 既有 vitest（含 Dashboard）门禁为主。


| 套件                                | 数量     | 状态                           |
| ----------------------------------- | -------- | ------------------------------ |
| `cargo test --lib`                  | 690      | ✅ 0 failed / 2 ignored        |
| `cargo test --lib prompt_synthesis` | 19       | ✅（TriShot 三击管线全部通过） |
| `cargo test --lib narrative::genesis` | 12     | ✅（创世四步/骨架解析/重试闸门/payload 契约） |
| `cargo test --lib narrative::protagonist_card` | 6 | ✅（人物卡 merge/render/probe） |
| `npx tsc --noEmit`                  | 前端类型 | ✅                             |
| `cargo check`                       | —        | ✅ 零错误                      |
| `npm run format:check`              | 代码风格 | ✅ 零差异                      |

| 类型           | 数量      | 状态                                         |
| -------------- | --------- | -------------------------------------------- |
| Rust 单元测试  | 685       | ✅ 全部通过 (`cargo test --lib`)             |
| 前端单元测试   | 242       | ✅ 全部通过 (`vitest run`)                   |
| 前端构建测试   | —         | ✅ `npm run build` 通过                      |
| Tauri 构建测试 | —         | ✅ `cargo tauri build` 通过                  |
| Playwright E2E | 41 (36+5) | ✅ 行为驱动测试（CI 中 `continue-on-error`），其中 `genesis-duplicate.spec.ts` 验证自动接受后幽灵段落隐藏 |

### v0.26.24 新增测试

- **散布式句子块重复**：Rust `test_trim_self_repetition_interspersed_*` + TS `trimInterspersed*` 用例；golden fixture 新增 `interspersed_repeated_block` / `interspersed_short_sentence_unchanged`。
- **跨内容重叠剥离**：Rust `test_strip_existing_overlap_*`（6 条）；TS `stripExistingOverlap` / `sanitizeContinuationOutput` 用例。
- **截断末句裁剪**：Rust/TS `trimDanglingTail` 用例。

### v0.26.28 Phase 4 新增测试

- **策略选择移入 Quick Phase**：`genesis.rs` `quick_phase_steps` 为「概念 → 策略选择 → 铺设开篇骨架 → 撰写开篇」四步（v0.26.44）、`background_steps` 为 5 步的单元契约；步骤 `step_number`/`total_steps`/`progress_percent` 一致性覆盖。
- **Prompts 外部化**：`prompts/registry.rs` 运行时加载 `resources/prompts/**/*.md` 的集成测试；95 个提示词全部可解析且公开 API 保持不变的回归测试。
- **迁移脚本拆分**：`MigrationRunner` + `RustMigration` trait 对 70 个编号 Rust 迁移与 SQL 迁移统一排序、过滤、执行的测试；`schema_migrations` 版本语义保持不变的兼容性测试。
- **知识图谱手动 CRUD UI**：新建实体、添加关系交互的 Playwright 覆盖。
- **世界构建 AI 生成 / 角色 AI 扩展 / 叙事分析图表**：对应组件渲染、API 调用、数据回写的单元与集成测试。

### v0.26.27 Phase 3 新增测试

- **L4 诊断互链**：GenesisPanel → TracingPanel / Logs 跳转与预过滤行为覆盖；TracingPanel detail → GenesisPanel 回跳选择对应 run 覆盖。
- **UsageStats operation 分组**：按 `operation` 字段拆分的四标签页渲染与聚合逻辑测试。
- **Foreshadowing UX**：`setup_scene_id` 下拉绑定 `useScenes`、高级区 `target_start_scene` / `target_end_scene` 编辑交互测试。
- **前端循环依赖守卫**：`npx madge --circular src/main.tsx` 验证循环数为 0；新增 `types/editor.ts`、`stores/contracts/*` 的导入方向单测。
- **Tauri 循环依赖守卫**：`creative_engine ↔ llm` 已无互相 import；`model_gateway ↔ router` 仍存少量直接 import，静态检查标记后续迁移目标；`ports/` / `domain/` 共享 trait 的单元测试。

### v0.26.26 Phase 2 新增测试

- **角色编辑与关系 CRUD**：`CharacterEditModal` 与 `CharacterRelationshipForm` 的创建 / 更新 / 删除路径测试。
- **L2 创世溯源徽章**：`is_auto_generated` / `source` 字段在角色、场景、世界观、知识图谱等页面的显示规则测试。
- **Story System 合同播种状态**：MASTER_SETTING + CHAPTER_1 合同状态卡渲染；失败运行警告摘要测试。
- **Scenes 续写跳转幕前**：`ExecutionPanel` 主行动打开 frontstage 的交互测试。
- **Repository trait 注入**：`creative_engine` 通过 `db/traits.rs` 调用 repository 的契约测试；`db/repositories/*.rs` 拆分后 re-export 一致性测试。

### v0.26.25 Phase 1 新增测试

- **GenesisPanel 步骤模型**：`src-frontend/src/utils/__tests__/genesisSteps.test.ts` 验证 Quick（3 步）+ Background（5 步）顺序、`steps_json.errors` 展示、story / 幕前跳转。
- **仪表盘统计卡**：点击跳转对应页面与口径一致性测试。
- **Stories Wizard 重复建故事**：已有故事 update 路径不重复创建的故事级测试。
- **后端特征测试**：
  - `model_gateway/executor.rs`：happy path + 模型降级 / 超时错误路径。
  - `db/repositories.rs`：创建 / 更新 / 删除 round-trip 与级联清理。
  - `memory/ingest.rs`：实体关系提取成功与字段缺失降级路径。

### v0.26.19 新增测试

- **Rust Genesis 契约测试**：`compute_trim_ratio`/`should_retry_self_repetition`/`select_first_chapter_content`/`build_first_chapter_chapter_switch` 纯函数边界与 payload 契约；`background_steps` 6 步固定顺序；`world_concept_for_character_prompt`；mutex 中毒恢复；`GenesisStepError` 严重度分级与累计；`genesis_runs` 状态流转。
- **跨层共享 trim golden fixture**：`tests/fixtures/trim_golden.json`（7 条用例），Rust `trim_self_repetition_matches_shared_golden_fixture` 与 TS `textCleanup.golden.test.ts` 双跑同一 fixture，锁定跨层一致性。
- **前端 Gap B/C + 状态机**：Gap B（空 finalContent 不锁 delivered）、P0-3（懒加载失败不锁 delivered）、Gap C（delivered + 编辑器有内容 → 跳过 setContent）、p4-4（重复入站也跳过）、状态机端点契约。

### 测试文件分布

**前端单元测试** (`src-frontend/src/**/*.test.{ts,tsx}`):

- `frontstage/hooks/`：useFrontstageWensi ×6、useFrontstagePanels ×8、useFrontstageEditor ×7、useFrontstageGeneration ×6
- `frontstage/components/`：HelpPanel ×3、ZenModeExit ×2、FrontstageHeader ×11、FrontstageSidebar ×3、FrontstageBottomBar ×3、FrontstageApp ×5
- `utils/`：cn ×5、format ×14、numberFormat ×19、settings ×4
- `hooks/`：useSettings ×4
- `services/`：settings ×4
- 其他：smoke ×1、useSyncStore.bug ×1、LlmProfileForm.bug ×1

**Rust 单元测试** (`src-tauri/src/**/*.rs` 内 `#[cfg(test)]`):

- `db/repositories_tests.rs`：18 例
- `db/cascade_tests.rs`：6 例
- `db/repositories_narrative.rs`：3 例（source/status round-trip、repository 读写 round-trip）
- `canonical_state/tests.rs`：8 例
- `task_system/tests.rs`：15 例
- `task_system/integration_tests.rs`：5 例
- `prompts/registry.rs`：提示词注册表测试（内置 prompt 解析、覆盖读取、分类枚举）
- `creative_engine/anti_ai/`：AntiAiRewriter 4 例、OpeningClarityGate 5 例、LivingAuthorGuard 6 例
- `utils/validation_tests.rs`：16 例
- `utils/style_align.rs`：3 例
- `utils/text.rs`：12 例（新增 `trim_self_repetition` 自重复清理测试）
- `utils/file.rs`：3 例
- `pipeline/executor.rs`：9 例
- `pipeline/refine.rs`：3 例
- `pipeline/review.rs`：3 例
- `story_system/scene_service.rs`：5 例
- `narrative/elements.rs`：8 例
- `creative_engine/style/dna.rs`：4 例
- `book_deconstruction/parser.rs`：3 例
- `config/settings_tests.rs`：13 例

**E2E 测试** (`e2e/*.spec.ts`):

- `storymoss.spec.ts`：12 例（数据持久化、页面加载、响应式）
- `frontstage-editing.spec.ts`：7 例（编辑器、自动保存、模式切换）
- `backstage-pages.spec.ts`：8 例（各页面加载与导航）
- `navigation.spec.ts`：4 例（URL 路由）
- `context-menu.spec.ts`：2 例（右键菜单）
- `example.spec.ts`：1 例（冒烟测试）
- `performance/tiptap-benchmark.spec.ts`：2 例（性能基准，默认跳过）

## ✅ 已安装组件

| 组件       | 版本          | 状态      |
| ---------- | ------------- | --------- |
| Bun        | 1.3.6         | ✅        |
| bunwv      | 0.0.5         | ✅ (备用) |
| Playwright | latest        | ✅        |
| Chromium   | 147.0.7727.15 | ✅        |

## 🚀 快速开始

### 1. 运行所有测试

```bash
npm test
# 或
npx playwright test
```

### 2. 截图所有页面

```bash
npm run screenshot
```

### 3. 快速截图幕前界面

```bash
npm run screenshot:front
```

### 4. 快速截图幕后界面

```bash
npm run screenshot:back
```

### 5. 交互式调试

```bash
npm run test:ui
```

## 📸 截图示例

测试环境已成功截图：

### 幕前界面 (Frontstage)

- 温暖纸张色调 (#f5f4ed)
- 简洁写作界面
- AI 续写功能入口

### 幕后界面 (Backstage)

- 深色影院主题
- 仪表盘统计
- 左侧导航菜单

截图保存在 `e2e/screenshots/` 目录。

## 🛠️ 测试脚本

### 使用 test-helper.js

```bash
# 显示帮助
node scripts/test-helper.js help

# 启动开发服务器
node scripts/test-helper.js start

# 运行测试
node scripts/test-helper.js test

# 截图
node scripts/test-helper.js screenshot

# 清理截图
node scripts/test-helper.js clean

# 查看报告
node scripts/test-helper.js report
```

### 使用 BrowserTestHelper 类

```typescript
import { BrowserTestHelper, runTest } from "./e2e/test-helper";

// 方式 1: 使用 runTest 包装器
runTest(async (helper) => {
  await helper.navigate("http://localhost:5173");
  await helper.screenshot("homepage");
  await helper.click("button");
  await helper.type('input[name="title"]', "测试标题");
  await helper.sleep(1000);
});

// 方式 2: 手动控制
const helper = new BrowserTestHelper();
await helper.start("chromium", false); // 启动有界面浏览器
await helper.navigate("http://localhost:5173");
await helper.screenshot("test");
await helper.stop();
```

## 📝 测试命令参考

### 导航

- `helper.navigate(url)` - 导航到 URL
- `helper.getTitle()` - 获取页面标题
- `helper.getUrl()` - 获取当前 URL

### 截图

- `helper.screenshot(name)` - 截图保存
- `helper.sleep(ms)` - 等待指定时间

### 交互

- `helper.click(selector)` - 点击元素
- `helper.clickText(text)` - 点击包含文本的元素
- `helper.type(selector, text)` - 输入文本
- `helper.clear(selector)` - 清除输入框
- `helper.press(key)` - 按下按键
- `helper.scroll(dx, dy)` - 滚动页面

### 等待

- `helper.waitFor(selector)` - 等待元素出现
- `helper.waitForText(text)` - 等待文本出现

### JavaScript

- `helper.eval(script)` - 执行 JS 代码
- `helper.getText(selector)` - 获取元素文本
- `helper.exists(selector)` - 检查元素是否存在

## 🎯 测试场景示例

### 测试版本管理功能

```typescript
test("版本时间线截图", async ({ page }) => {
  await page.goto("/index.html#/scenes");
  await page.waitForTimeout(3000);

  // 查找版本时间线组件
  const versionTimeline = page.locator('[data-testid="version-timeline"]');
  if (await versionTimeline.isVisible()) {
    await versionTimeline.screenshot({
      path: "e2e/screenshots/version-timeline.png",
    });
  }
});
```

### 测试响应式布局

```typescript
test("多分辨率测试", async ({ page }) => {
  const sizes = [
    { width: 1920, height: 1080, name: "desktop" },
    { width: 1366, height: 768, name: "laptop" },
    { width: 768, height: 1024, name: "tablet" },
  ];

  for (const size of sizes) {
    await page.setViewportSize(size);
    await page.goto("/frontstage.html");
    await page.screenshot({
      path: `e2e/screenshots/responsive_${size.name}.png`,
    });
  }
});
```

## 🔧 配置说明

### Playwright 配置 (playwright.config.ts)

```typescript
export default defineConfig({
  testDir: "./e2e",
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  use: {
    baseURL: "http://localhost:5173",
    screenshot: "only-on-failure",
    video: "on-first-retry",
  },
  webServer: {
    command: "cd src-frontend && npm run dev",
    url: "http://localhost:5173",
  },
});
```

## 📊 测试报告

运行测试后查看报告：

```bash
npm run test:report
```

报告位于 `playwright-report/` 目录。

## 🐛 故障排除

### 浏览器未安装

```bash
npx playwright install chromium
```

### 端口被占用

修改 `playwright.config.ts` 中的端口配置。

### 测试超时

增加 `timeout` 配置：

```typescript
timeout: 60000, // 60秒
```

## 📚 参考文档

- [Playwright 官方文档](https://playwright.dev/)
- [bunwv GitHub](https://github.com/NatiCha/bunwv)
- [StoryMoss 架构文档](./ARCHITECTURE.md)

---

_最后更新: 2026-08-29 - v0.58.0 戏剧工艺 / 短剧格式，测试基线 cargo 1583 / vitest 607_
