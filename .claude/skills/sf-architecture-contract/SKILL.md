---
name: sf-architecture-contract
description: StoryForge 承重设计决策与 WHY、必须成立的不变量、已知弱点。何时加载：要改架构、要动数据真相源、要改生成链路、要改 IPC/SyncEvent、要动数据库层、要评估“能不能这样改”、或被问“为什么这样设计/哪些不能动”时。改任何符号前先用 gitnexus impact。
---

# StoryForge 架构契约

> 改任何函数/类/方法前，先跑 `gitnexus_impact({target, direction:"upstream"})` 评估爆炸半径；HIGH/CRITICAL 必须先告知用户。这是 `CLAUDE.md` 强制项。

## 承重决策与 WHY

### 1. 剧院式双界面（frontstage / backstage）
- **决策**：两个 Tauri 窗口、两个 HTML 入口。`frontstage.html`（沉浸写作，暖色 `#f5f4ed`）+ `index.html`（工作室，深色 Cinema）。`tauri.conf.json` 定义两 window，backstage 默认隐藏。
- **WHY**：写作需要心流沉浸，管理需要专业密度；混在一起两边都做不好。
- **不变量**：幕前不承担 CRUD 管理；幕后不承担正文编辑。联动走 Tauri 事件（`backstage-update`/`backstage-shown`），逐步替代 DOM CustomEvent。

### 2. 场景优先（Scene-first）—— 唯一叙事真相源
- **决策**：`scenes.content` 是唯一叙事真相源，`chapters.content` 是只读聚合投影（v0.23.74）。
- **WHY**：章节是时间/长度驱动，场景是戏剧冲突驱动；AI 理解“戏剧目标 + 冲突”优于理解纯文本。
- **不变量**：禁止再向 `chapters.content` 直接写入；场景内容变更经 `SceneCommitDebouncer`（30s）触发 `SceneCommitService::auto_commit` → 5 个 Projection Writer。
- **弱点**：场景↔章节双向映射与 cache 对称失效曾是 v5.6.0 一整轮修复的对象；改这里要跑 `db/cascade_tests.rs`。

### 3. 分时介入架构（Time-Sliced，v0.13.0）
- **决策**：三条时间线解耦。写作时刻（热路径 <15s，`WriteTimeBundle` 最小约束直连 LLM 立即返回）→ 审计时刻（温路径 30-90s 后台 `AuditExecutor` 7 维 Inspector，inline 标注回流）→ 洞察时刻（冷路径分钟级 `InsightExecutor` 每 5 段深度报告）。
- **WHY**：解开“质量与速度不可兼得”——慢的根源不是资产量，而是同步链路堆叠的 Inspector/Rewrite。Phase 0 A/B 盲测：最小约束 vs 全量资产质量差仅 7.9%（< 30% 阈值）。
- **不变量**：`GenerationMode::Fast`/`TimeSliced`/`Full`/`TriShot` 四值并存；`TimeSliced` 默认。热路径禁止同步跑 Inspector/Rewrite/Preflight 补合同。
- **弱点**：策略选择仍在后台阶段，首章 `build_strategy_notes` 拿不到 `selected_strategy`（见 `sf-genesis-campaign`）。

### 4. 合同驱动故事系统（Story System v6.0.0）
- **决策**：四级合同 `MASTER_SETTING → Volume → Chapter → Review` + `SCENE_COMMIT`（state/entity/events/projection_status JSON）+ 5 Projection Writer + `ContractTree`/`RuntimeContract` 动态合并。
- **WHY**：防幻觉三定律——合同即法律、设定即物理、发明需识别。
- **不变量**：所有生成内容受合同约束；新实体必须被识别并记录。

### 5. 单写者状态机（Genesis 第一章，v0.26.16）
- **决策**：前端 `FrontstageApp` 用 `idle → generating → delivered` 三态替换 `genesisAutoAcceptedRef` 布尔。`generating` 态阻塞外部内容投递（`onChapterUpdated`/`loadStories` 自动选择）；`delivered` 态阻塞幽灵文本恢复。
- **WHY**：散布布尔守卫 9 次复发；单写者状态机是结构性根治。
- **不变量**：禁止回退到布尔守卫模式；`selectChapter` 咽喉点必须 `delivered` + 编辑器已有内容守卫；`appendAiContent` skip 路径不 `markAccepted`。

### 6. 生成侧 8% 自重复闸门（v0.26.16）
- **决策**：`genesis.rs` 检测 LLM 输出自重复比例，≥8% 用更强 anti-repeat 指令重试一次；prompt 模板含「结构纪律」段禁止首尾回环与整章重复。
- **不变量**：`trim_self_repetition`（Rust）与 `trimSelfRepetition`（TS）必须跨层一致，由 `tests/fixtures/trim_golden.json` 双跑锁定。改一边必须同步另一边。

### 7. Context Rot 显式防御（v0.25.0）
- **决策**：`ContextPrioritizer` 按 Critical/High/Normal/Background 排序系统提示词，Critical 在开头与结尾双重锚定。
- **不变量**：合同红线、在世作者保护、反 AI 陈词滥调属 Critical，不得降级。

### 8. 四级错误分类与恢复（v0.25.0）
- **决策**：`ErrorSeverity` Fatal/Retry/Degraded/UserAction + 指数退避重试 + 降级回退 + `AgentInterruptionModal`。
- **不变量**：Fatal/UserAction 必须前端中断 UI；Retry 走 `retry_with_backoff`；Degraded 走 `with_degraded_fallback`。

### 9. 分层架构（v0.23.6 单例清零）
- **决策**：Command（薄层参数校验 + EmitSync）→ DTO（`db/dto.rs`）→ Domain Service（`story_system/*_service.rs`）→ Repository（`db/repositories*.rs`）→ SQLite/LanceDB/FS。14 个 `static`/缓存改为 Tauri State 注入；模块循环依赖斩断。
- **不变量**：`architecture_guard.py` 强制——`db` 禁止依赖 `narrative/agents/memory/creative_engine/story_system/pipeline`；`domain` 禁止依赖任何业务模块；`FORBIDDEN_GLOBALS`（`VECTOR_STORE/DB_POOL/LLM_SERVICE/APP_CONFIG/SKILL_MANAGER/...`）不得重新引入。
- **弱点**：`KNOWN_VIOLATIONS` 当前为空（Phase 2.4 已清零）；新增模块依赖要更新 `architecture_guard.py`。

### 10. 类型安全基座（v6.0.0）
- **决策**：`SyncEvent`/`FrontstageEvent`/`BackstageEvent` `#[derive(TS)]` → `src-frontend/src/generated/`；前端 `assertUnreachable`；`verify-ipc-manifest.py` 校验 `generate_handler![]` ↔ `loggedInvoke`。
- **不变量**：新增 enum variant 必须前端 default 分支 `assertUnreachable`，否则编译失败。

## 已知弱点（明说，不藏）

- 策略选择已前移至 quick phase（v0.26.28）；方法论在 background 的注入与步进推进见 `docs/plans/2026-07-09-methodology-in-genesis-remediation-design.md` / 审计 `docs/audits/2026-07-09-methodology-in-genesis-audit.md`。
- LanceDB 持久化 blocked（Arrow 依赖与工具链冲突），现用 SQLite 向量兜底。
- Clippy 471 个历史警告未清零（非 `-D warnings`）；前端 200+ lint 错误未恢复 lint job。
- E2E 跑在 dev server 无后端，IPC 挂起，仅 `continue-on-error`。
- `format_strings` 跨平台 nightly 不一致，已禁用。

## 何时 NOT 用本技能

- 具体失败模式分诊 → `sf-debugging-playbook`。
- 已修复战役编年史 → `sf-failure-archaeology`。
- 领域理论（TipTap/Tauri/LanceDB 基础）→ `sf-reference`。
- 要改配置开关 → `sf-config-and-flags`。

## 出处与维护

- 重验证命令：
  - `python3 scripts/architecture_guard.py; echo $?`
  - `rg -n 'scenes.content|chapters.content' src-tauri/src | head`
  - `rg -n 'idle|generating|delivered' src-frontend/src/frontstage/FrontstageApp.tsx | head`
  - `rg -n 'FORBIDDEN_GLOBALS' scripts/architecture_guard.py`
  - `rg -n '#\[derive\(TS\)\]' src-tauri/src/state_sync/events.rs`
- 易漂移项：模块边界、State 注入顺序、SyncEvent variant、8% 阈值、`FORBIDDEN_GLOBALS` 清单。
- 最后核对：2026-07-07，v0.26.23。
