# StoryMoss 综合优化计划执行状态

> **关联计划**：`docs/plans/2026-07-07-storymoss-comprehensive-optimization-plan.md`  
> **当前版本**：v0.26.33  
> **日期**：2026-07-08

---

## 一、总体进度

| 阶段 | 目标版本 | 完成度 | 状态 |
|------|----------|--------|------|
| 阶段一：可观测性与测试基线 | v0.26.25 | 100% | ✅ 已完成（v0.26.32 补齐） |
| 阶段二：L2 资产补齐与领域层止血 | v0.26.26 | ~90% | 角色关系删除 UI 已补；`creative_engine` 全模块 trait 注入仍待推进 |
| 阶段三：L4 诊断互链、文档与依赖解耦 | v0.26.27 | ~85% | 前端 `frontstage ↔ components` 解耦已完成；Tauri `model_gateway/llm/router` 真正解耦仍待推进 |
| 阶段四：架构债务与工程体验 | v0.26.28 | ~80% | KG 删除 UI 已补；prompts 彻底外部化、迁移脚本拆分仍待推进 |

---

## 二、已交付版本

### v0.26.31 — 修复用户反馈 6 项问题

| # | 问题 | 修复文件 |
|---|------|----------|
| 1 | 顶部状态栏字数统计滞后 / 始终显示“保存中” | `FrontstageApp.tsx` |
| 2 | 顶部状态栏 `12px` 字体大小点击无响应 | `FrontstageHeader.tsx`, `FrontstageApp.tsx`, `Settings.tsx`, `GeneralSettings.tsx`, `sync.rs` |
| 3 | 底部状态栏后台任务图标显示为缺字符号 | `FrontstageBottomBar.tsx` |
| 4 | 顶部状态栏整体精细化 | `FrontstageHeader.tsx` |
| 5 | 小说初始化策略 JSON 解析失败：`missing field rationale` | `strategy.rs`, `selector.rs` |
| 6 | 获取角色报错：`no such column: source` | `connection.rs` |

- CI：`https://github.com/91zgaoge/StoryMoss/actions/runs/28929492984` ✅

### v0.26.32 — 完成阶段一剩余项

- **L1 创作入口 UX 统一**：`CreationPathGuide` 卡片可点击；Dashboard “AI 创建故事”进入幕前 Genesis。
- **仪表盘统计卡修正**：“章节”→“场景”，新增“字数”统计卡。
- **`memory/ingest` 测试补齐**：5 条 happy/error 路径测试。
- CI：`https://github.com/91zgaoge/StoryMoss/actions/runs/28935204061` ✅

### v0.26.33 — 补齐阶段 2/3/4 具体缺口

- **Stage 4**：KG 实体归档 + 关系删除 UI（后端命令 + 前端）。
- **Stage 2**：角色关系删除 UI。
- **Stage 3**：前端 `frontstage ↔ components` 解耦，`hooks/contracts/useEditorConfig.ts`。
- CI：监控中（run `28940520198`）

---

## 三、剩余缺口与建议

### 缺口 1：`creative_engine` 全模块 repository trait 注入（Stage 2）

**现状**：仅 `context_builder.rs` 用 `Box<dyn Trait>` 包装了 repository；`adaptive/feedback.rs`、`generator.rs`、`miner.rs`、`personalizer.rs`、`cascade_rewriter/commands.rs`、`impact_analyzer.rs`、`rewrite_engine.rs`、`continuity.rs`、`workflow/engine.rs`、`write_time_bundle.rs`、`asset_snapshot.rs` 等仍直接 `use crate::db::repositories::*` 并实例化具体 repository。

**建议**：
1. 将 `db/traits.rs` 扩展为覆盖所有高频 repository 操作。
2. 逐步替换上述文件中的具体 repository 实例化为 `Box<dyn Trait>` 或函数参数注入。
3. 每改一个模块运行 `cargo check` 与 `cargo test --lib`。

**风险**：改动面较广，建议分 2–3 个小 PR/提交，避免一次性大面积编译错误。

---

### 缺口 2：Tauri `model_gateway ↔ llm ↔ router` 真正解耦（Stage 3）

**现状**：`ports/llm.rs` 已定义 `LlmService` trait，`domain/creative_engine.rs` 已定义 `CreativeEnginePort` trait，但业务代码仍大量直接 import 具体类型：
- `model_gateway/executor.rs` 直接 import `crate::router::UnifiedModelRouter`
- `model_gateway/dispatcher.rs` import `crate::router::{Complexity, RoutingRequest, TaskType}`
- `llm/service.rs` import `crate::model_gateway::executor::GatewayExecutor`
- `creative_engine/style/mod.rs`、`workflow/engine.rs`、`prompt_synthesis/refiner.rs` 等直接 import `llm::service::LlmService` / `router::TaskType`

**建议**：
1. 在 `ports/llm.rs` 补充 `TaskType`、`RoutingRequest` 等通用类型的端口定义，或将其下沉到 `domain/`。
2. 让 `model_gateway` 依赖 `ports/llm` 的 trait，而非 `llm::service` 的具体实现。
3. 让 `creative_engine` 通过 `dyn LlmService` 或 `Arc<dyn LlmService>` 接收 LLM 服务，而非直接 import。
4. 这通常需要一次中型重构，建议单独立项并写详细设计文档。

**风险**：这是架构层面的改动，可能影响运行时依赖注入和启动顺序，需要充分测试。

---

### 缺口 3：prompts 彻底外部化（Stage 4）

**现状**：`resources/prompts/**/*.md` 已有 96 个文件，运行时通过 `prompts/registry.rs` 加载，但源码中仍有大量 inline fallback prompts：
- `src-tauri/src/narrative/prompts.rs`：约 835 行，14 个内嵌提示词
- `src-tauri/src/creative_engine/methodology/character_depth.rs`：完整内嵌提示词
- `src-tauri/src/creative_engine/methodology/hero_journey.rs`：完整内嵌提示词

**建议**：
1. 将上述 inline prompt 迁移到 `resources/prompts/{category}/`。
2. 删除 fallback 硬编码，让代码只从 registry 加载。
3. 如果某些 prompt 只在特定上下文使用，可保留最小化 fallback，但需明确标注为 deprecated。

**收益**：实现“2,900 行函数消失；prompt 编辑无需重新编译”的目标。

---

### 缺口 4：迁移脚本拆分（Stage 4）

**现状**：`src-tauri/src/db/migrations/` 已有 96 个迁移文件和 `MigrationRunner`，但 `src-tauri/src/db/connection.rs` 仍保留约 1,307 行的巨型 `create_tables` 函数，内含内联 `CREATE TABLE` 和历史版本迁移逻辑。

**建议**：
1. 将 `create_tables` 中的 schema 拆分为 `migrations/V001__baseline_schema.sql`（或 Rust migration）。
2. 将历史版本迁移逻辑（Migration 17/28/29/38/39/40 等）拆分为独立编号迁移文件。
3. 逐步删除 `connection.rs` 中的内联块，让 `MigrationRunner` 成为 schema 演进的唯一入口。

**风险**：这涉及数据库初始化路径，需要确保新安装和旧升级都正常。建议在拆分后做完整迁移测试。

---

## 四、下一步建议

1. **立即**：等待 v0.26.33 CI 全部 green 后发布。
2. **短期（1–2 天）**：完成剩余 4 个缺口的详细设计文档，评估每个缺口的测试策略。
3. **中期（1–2 周）**：按以下顺序执行：
   - prompts 彻底外部化（收益明显，blast radius 可控）
   - `creative_engine` repository trait 注入（与 Stage 2 目标一致）
   - Tauri 模块解耦（需要设计评审）
   - 迁移脚本拆分（风险最高，放最后）
4. **长期**：在 v0.26.34–v0.26.36 中分阶段交付上述架构债务清理。
