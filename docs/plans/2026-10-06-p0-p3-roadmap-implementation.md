# P0–P3 路线图实施计划（改稿级联 / 知识边界 / 物品归属 / 记忆质量 / 文本质量 / 工程纪律）

> 日期：2026-10-06
> 依据：`docs/audits/2026-10-06-ai-novel-landscape-comparison.md`（外部五项目对比与引进路线图）
> 编排：按优先级分四个阶段实施，每阶段一个发布版本（v0.60.0 → v0.63.0），每阶段收口时跑全量验证并按项目规则更新 8 份必需文档、统一版本号、打新 tag 推送。
> 原则：**只借设计思想不抄 GPL 代码**（MuMuAINovel / webnovel-writer 均为 GPL-3.0）；**列出冲突由作者拍板，不自动改写后文**；增量最小、可降级、可测试。

---

## 阶段总览

| 阶段 | 版本 | 主题 | 对应路线图项 | 帖主尺子 |
|---|---|---|---|---|
| **P0** | v0.60.0 | 三把尺子：知识边界 / 物品归属 / 级联影响报告 | P0-1、P0-2、P0-3 | ①②③ |
| **P1** | v0.61.0 | 记忆质量：语义摘要 + 分层金字塔 + 自适应窗口 | P0-4 | 防吃书根基 |
| **P2** | v0.62.0 | 文本质量与成本：prose_lint / 风格逆向学习 / 伏笔增强 / 预算哨兵 | P1-5 ~ P1-8 | 质量确定性 |
| **P3** | v0.63.0 | 工程纪律：质量债 / 三级审批 / 检查点与时间旅行 / Compass / 三测试套件 | P2 全部 | 长期可持续 |

---

# 阶段 P0（v0.60.0）：三把尺子

## 目标

1. **知识边界**：角色知道什么/不知道什么成为可写入、可更新、可校验的数据；续写时注入；泄密可检测。
2. **物品归属**：关键物品的持有者成为状态（玉佩测试）；写作时在场物品可见；归属矛盾可检测。
3. **级联影响报告**：改旧章后自动产出「受影响后续章节 + 疑虑冲突」清单；三条动作（去查看/忽略/触发改写）+ 重跑分析；**不自动改写后文**。

## T1 数据层（迁移 V135）

新增表（纯 additive，SQL 迁移，`IF NOT EXISTS` 幂等）：

```sql
-- 1) 世界真相 / 读者已知 / 揭示状态（三层信息分离的事件载体）
CREATE TABLE IF NOT EXISTS story_timeline_events (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    chapter_number INTEGER,
    scene_id TEXT,
    sequence_number INTEGER,
    objective_fact TEXT NOT NULL,          -- 世界真相（作者侧）
    reader_knowledge TEXT,                 -- 读者此刻认知（可空）
    reveal_status TEXT NOT NULL DEFAULT 'hidden',  -- hidden | partial | revealed
    reveal_chapter INTEGER,                -- 实际揭示章（hidden 时为空）
    participants TEXT NOT NULL DEFAULT '[]',       -- JSON 角色名数组
    source TEXT NOT NULL DEFAULT 'ingest',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_story_timeline_events_story ON story_timeline_events(story_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_story_timeline_events_reveal ON story_timeline_events(story_id, reveal_status);

-- 2) 角色知情变更日志（审计与时间旅行查询的基础）
CREATE TABLE IF NOT EXISTS character_knowledge_log (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    character_id TEXT NOT NULL,
    fact TEXT NOT NULL,
    change_type TEXT NOT NULL DEFAULT 'learned',   -- learned | forgot
    source_scene_id TEXT,
    chapter_number INTEGER,
    evidence TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_character_knowledge_log_char ON character_knowledge_log(story_id, character_id, chapter_number);

-- 3) 物品归属账本（只登记跨章影响行动边界的资源）
CREATE TABLE IF NOT EXISTS item_holdings (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    item_name TEXT NOT NULL,
    item_entity_id TEXT,
    holder_name TEXT,
    holder_character_id TEXT,
    status TEXT NOT NULL DEFAULT 'held',   -- held | lost | destroyed | transferred | unknown
    acquired_chapter INTEGER,
    evidence TEXT,
    source_scene_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_holdings_story_item ON item_holdings(story_id, item_name);

-- 4) 级联影响报告（一个 batch = 一次改稿分析；每行 = 一个受影响目标场景）
CREATE TABLE IF NOT EXISTS cascade_impacts (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    batch_id TEXT NOT NULL,
    source_scene_id TEXT NOT NULL,
    source_chapter_number INTEGER,
    target_scene_id TEXT NOT NULL,
    target_chapter_number INTEGER,
    impact_score REAL NOT NULL DEFAULT 0,
    impact_kind TEXT NOT NULL DEFAULT 'mention',  -- mention | conflict
    severity TEXT NOT NULL DEFAULT 'info',        -- info | warning | critical
    entity_ids TEXT NOT NULL DEFAULT '[]',
    detail TEXT,
    evidence TEXT,
    decision TEXT NOT NULL DEFAULT 'open',        -- open | ignored | rewrite_requested | resolved
    stale_flag INTEGER NOT NULL DEFAULT 0,        -- 目标章分析可能已失效
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(batch_id, target_scene_id, impact_kind),
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_cascade_impacts_story ON cascade_impacts(story_id, decision, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_cascade_impacts_target ON cascade_impacts(target_scene_id);
```

## T2 知识边界（路线图 P0-1）

- **抽取**：ingest 分析 schema 新增 `knowledge_updates[]`（`{character, fact, evidence}`）与 `timeline_events[]`（`{objective_fact, reader_knowledge, reveal_status, chapter_number, participants, evidence}`）——prompt 资产 `memory_content_analysis` 与 ingest 内置 prompt 同步更新；Rust DTO 全部 `#[serde(default)]`。
- **落库**：`persist_knowledge_updates`（角色 → `character_states.secrets_known` 追加、`secrets_unknown` 按内容去重移除 → 写 `character_knowledge_log`）；`persist_timeline_events`（同 story + objective_fact 去重 upsert）。
- **注入**：续写资产渲染区分「稳定设定」与「当下认知」；`【本拍角色】` 卡补「该角色尚不知道：…」行（数据源 `secrets_unknown`）。Agency 断链修复：`creative_engine/adapter.rs` 转 `CharacterStateSnapshot` 时保留 secrets。
- **泄密探针**：新模块 `agency/knowledge_boundary.rs` 纯函数 `detect_leaks(text, present_characters, secrets_map, hidden_events)` —— 判定规则：若增量正文包含某在场角色 `secrets_unknown` 条目的**高区分度片段**（≥8 连续中文字符）或未揭示事件 `objective_fact` 的高区分度片段 → 返回 leak 描述。接入 `probe_increment_ex` 作为新 gap（严格阈值控制误报）。
- **质检**：editor_qc 预注入「知识边界块」（在场角色已知/未知 + 未揭示真相），写进 `agency_editor_auditor_system` prompt 变量，要求核查「提前泄密」。

## T3 物品归属（路线图 P0-2）

- **抽取**：ingest schema 新增 `item_holdings[]`（`{item, holder, action: acquire|transfer|lose|destroy, evidence}`）；prompt 明示「只登记跨章影响行动边界的资源」。
- **落库**：`persist_item_holdings` 按 (story, item) upsert；holder 名解析到 `characters.id`（缺失则留空后续补）；`action` 映射 status。
- **注入**：`BeatState` 增 `items: Vec<String>`（在场角色持有的关键物品），节拍卡 `render_full` 增【在场物品】行。
- **探针**：物品名出现于增量正文、且其持有者不在场、且邻近出现使用动词（拿出/掏出/递给/交给/握着/收起/丢失）→ gap「物品[X]出现在正文但持有者[Y]不在场」。

## T4 级联影响报告（路线图 P0-3）

**触发点**：`SceneIngestor::spawn_ingest_now` 完成 entity_mentions 重建后（`scene_service.rs:245`），spawn 后台 `cascade_impact::analyze_after_scene_ingest`。

**流程**：
1. **确定性影响分析**（零 LLM）：本场景实体 → 其余场景 mention 聚合打分（复用 `ImpactAnalyzer` 思路，修正 story_id 过滤 bug）→ 仅保留**后续章节**（target.chapter > source.chapter）→ 写入 `cascade_impacts`（kind=mention, stale_flag=1）。
2. **LLM 冲突扫描**（受后台闸门约束，可降级）：取分数最高的至多 5 个目标章，输入「改动章正文节选 + 各下游章摘要（scene_commits.summary_text）」→ JSON 输出 `{conflicts:[{target_chapter, severity, description, source_evidence, target_evidence, suggestion}]}` → 命中者 upsert 为 kind=conflict 行（severity 提升）。解析用 `narrative::extract_and_sanitize_json`，失败仅告警不阻断。
3. **事件**：新增 `SyncEvent::CascadeImpactDetected { story_id, batch_id, count, critical_count }` + TS 导出 + `useSyncStore` case（`assertUnreachable` 要求前端同步）。
4. **命令**：`list_cascade_impacts(story_id, decision?)`、`ignore_cascade_impact(id)`、`reanalyze_scene(scene_id)`（触发 `spawn_ingest_now` 并在完成后清除该场景 stale 标记）、`trigger_cascade_rewrite_for_impact(impact_id)`（由 impact 的 entity 构造 `EntityChangeEvent`，`after_json` 携带「上游第 N 章正文已修改」说明与节选，修复 `entity_name` TODO 从 KG 解析）。
5. **前端**：新页面「级联中心」（诊断组导航项）：按 batch 分组、严重度徽章、证据对照，行内四动作（去查看=幕后 Scenes 定位 / 忽略 / 重跑分析 / 触发改写→跳任务）；`useSyncStore` 事件刷新 `['cascade_impacts']`。

## T5 测试与验收（P0）

| 契约测试 | 内容 |
|---|---|
| `test_persist_knowledge_updates_moves_secret_from_unknown_to_known` | 信息流落库：已知追加、未知移除、日志留痕 |
| `test_knowledge_boundary_detects_unknown_secret_leak` / `..._ignores_known_fact` | 泄密检测纯函数正反例 |
| `test_persist_item_holdings_upserts_by_item` | 物品归属 upsert 与 status 迁移 |
| `test_probe_flags_item_use_without_holder_present` | 玉佩探针正反例 |
| `test_edit_early_chapter_creates_downstream_impacts` | **帖主测试③**：改第 3 章 → 第 7 章（共享实体）产生 impact + stale |
| `test_ignore_cascade_impact_marks_decision` | 忽略动作状态迁移 |
| `test_cascade_conflict_scan_parses_downstream_conflicts` | LLM 输出解析（fake） |

## 阶段 P0 验收标准

- 帖主三测试全部可在测试套件中回归（器物/知识/级联）。
- 改旧章后「级联中心」自动出现受影响清单与冲突；三条动作可用；无任何自动改写后文。
- 全量验证绿：`cargo test --lib` / `npx tsc --noEmit` / `npx vitest run` / `cargo +nightly fmt --check` / `prettier` / `architecture_guard.py`。

---

# 阶段 P1（v0.61.0）：记忆质量

- **P1-A 语义摘要**：`scene_commits.summary_text` 从「前 1000 字截断」升级为 LLM 100–150 字语义摘要（失败回退截断保底，label `bg-summary` 入静默后台名单）；测试锁解析与回退。
- **P1-B 分层金字塔**：新增段级摘要（每 10 章/卷末滚动，「段摘要」表或复用 `story_summaries` 的 scope 字段），章 → 段 → 全书三级。
- **P1-C 自适应窗口**：续写上下文按章节数自适应（≤15 章近 10 章摘要；16–50 近 5；>50 近 3 + 段摘要），替换写死的「近 3 章」。
- 验收：长书（>50 章）续写上下文包含确定性纲要而非仅检索召回；摘要相关契约测试。

# 阶段 P2（v0.62.0）：文本质量与成本

- **P2-A prose_lint**：纯 Rust 确定性检测器（oh-story 思想）：blocking 档（否定排比/章尾总结预告/工程词泄漏——扫【必须改变】【本拍】等注入术语）＋ advisory 档（跨章逐字重复句、章尾短句率、开篇时间词率）；接入 `run_mini_review` 之后与 editor_qc 前；规则经本地语料校准。
- **P2-B 作者风格逆向学习**：幕前手改 AI 文本时后台提取风格增量 → `style_preferences` 表 → `style_dna_summary` 槽注入；设置页可视化管理。
- **P2-C 伏笔增强**：计划/实际回收分离、strength/subtlety、伏笔链、每章自动新建上限、四层注入话术（近期待回收明确「请勿本章回收」）、8–25 字证据锚定；统一 `PayoffLedger::detect_overdue` 与真源阈值。
- **P2-D 预算哨兵**：per-story 成本聚合 + `book_usd` 预算 + warn 比例 + 零增量计费盲区检测。
- 验收：prose_lint 契约测试（正反例）；风格偏好落库与注入测试；伏笔超期/话术测试；预算熔断测试。

# 阶段 P3（v0.63.0）：工程纪律

- **P3-A 质量债台账**：QC fail-open 降级时未解决问题入账（含建议回收窗口），任务中心可见、可批量触发修订。
- **P3-B 新增物三级审批**：金手指规则/世界观硬规则类新实体弹窗确认（复用 V134 弹窗 UX）。
- **P3-C 检查点快照 + 时间旅行查询**：每 10 章全量资产快照 + 「某章时点的角色状态/知情/物品归属」查询（基于 append-only 日志与 `source_chapter`）。
- **P3-D Compass 指南针**：ending_direction / open_threads / estimated_scale，创世生成、卷末更新，注入规划上下文。
- **P3-E 三测试探针套件**：玉佩/知识边界/级联冲突三个端到端场景作为持续回归门（在 P0 契约测试基础上扩展为 e2e 组合）。
- **P3-F（远景，可不随版本）**：commit 投影的声明式「事件→writer」路由表（webnovel 思想），提升可测试性与可审计性。
- 验收：各契约测试 + 三测试套件全绿。

---

## 全局约束

1. **许可证**：MuMuAINovel、webnovel-writer（GPL-3.0）只借设计思想，一行代码不引入；oh-story（MIT）/ani-book（Apache-2.0）/ainovel-cli（License 与 README 不一致，参考前需澄清）实现可参考但一律 Rust 重写。
2. **不自动改写后文**：级联只报告与标记，改写必须作者显式触发（与 V134 `user_created` 保护同精神）。
3. **注入术语与用户内容隔离**：prompt 注入的工程术语不得泄漏进正文（P2-A 检测兜底）。
4. **每阶段收口**：全量测试 + 8 份必需文档 + 版本号统一（Cargo.toml / tauri.conf.json / package.json / tag）+ landing 兜底版本处理（线上 0.59.x 仍 404 时维持 0.58.0 不回退）+ 新 tag 推送。
5. **回退安全**：所有新功能失败必须降级不阻断（后台 LLM 失败仅告警；探针新增 gap 需控制误报，必要时降级为建议态）。


---

## 实施状态（2026-10-06 收口）

| 阶段 | 版本 | 状态 | 说明 |
|---|---|---|---|
| P0 三把尺子 | v0.60.0 | ✅ 已发布 | 知识边界（V135 双栏时间线 + 信息流 + 注入 + 探针） / 物品归属（账本 + 探针） / 级联影响报告（确定性 + LLM 扫描 + 级联中心）；契约测试 15 项 |
| P1 记忆质量 | v0.61.0 | ✅ 已发布 | 语义摘要（LLM 100-150 字，回退截断） / 段摘要 + 全书纲要（V136） / 自适应窗口（10/5/3 + 纲要注入） |
| P2 文本质量与成本 | v0.62.0 | ✅ 已发布 | prose_lint 确定性两档 / 文风逆向学习（V137，防抖 120s） / 伏笔增强（证据锚定 + 窗口话术 + 上限） / 成本账本 + 计费盲区哨兵 |
| P3 工程纪律 | v0.63.0 | ✅ 已发布 | 质量债台账（V138）/ 检查点 + 时间旅行查询 / 终局指南针（确定性派生）/ 待确认审批队列 / 三把尺子组合契约；**P3-F 声明式投影路由表未实施**（架构级重构，留待后续版本） |

### 范围说明（诚实记录）

- **前端 UI 缺口**：P2-B（文风偏好管理）、P2-D（成本视图）、P3-A（质量债列表）、P3-B（待确认队列）四处的**后端与命令均已就绪并有契约测试**，但尚未接入界面；建议后续在幕后新增「运行维护」页统一承载（或并入级联中心做多 Tab）。
- **P3-F**（commit 投影的声明式「事件→writer」路由表）属于架构级重构，未随本批次实施。
- **真机验证**：P0–P3 的所有契约测试均在本地套件内（Rust 1685 / 前端 590），但「三把尺子」的真机端到端复跑（真实 LLM 生成 + 长书 10+ 章）仍未完成——**不得宣称续写质量已修复**。
- **网站发布**：0.59.x 起各版本因 Apple 公证协议未签署未上传；landing 兜底维持 0.58.0。
