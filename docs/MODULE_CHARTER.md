# StoryMoss 模块职责清单

> 本文档定义 `src-tauri/src` 各顶层模块的职责边界与依赖规则，用于防止架构腐化回潮。  
> 配套工具：`scripts/architecture_guard.py`（CI 中运行）。

---

## 总体分层原则

```
UI (src-frontend)
  ↓ invoke / event
Commands (src-tauri/src/commands)
  ↓
Orchestration (agents / planner / intention_graph / task_system)
  ↓ 依赖 domain 类型 + 业务服务
Creative Domain (creative_engine / narrative / story_system / memory / audit / reading_power)
  ↓ 依赖 domain 类型
Domain (src-tauri/src/domain)
  ↓ 仅依赖 std/serde 等基础库
Infrastructure (db / llm / vector / embeddings / model_gateway / config / router)
```

**核心规则**：
1. `domain` 只包含纯数据结构，禁止依赖任何业务模块。
2. `db` 禁止依赖 `narrative`、`agents`、`memory`、`creative_engine`、`story_system`、`pipeline`。
3. `memory` 禁止依赖 `agents`（目标状态）。
4. `creative_engine` 禁止依赖 `agents`（目标状态）。
5. 业务模块之间的具体行为依赖应通过 trait/接口抽象，而非直接互相引用。
6. `prompts` 禁止导入 `agency`（组装器在 prompts，上下文仍由 agency 编译；见 `scripts/architecture_guard.py` 的 `PROHIBITED`）。

---

## 模块职责表

| 模块 | 职责一句话 | 允许依赖 | 禁止依赖 | 状态 |
|------|-----------|----------|----------|------|
| `domain` | 跨模块共享的纯数据结构（DTO / Value Objects） | `std`, `serde`, `chrono`, `uuid`, `regex` 等基础库 | 任何 `crate::` 业务模块 | ✅ 已落地 |
| `db` | 数据持久化、连接池、迁移、仓库实现 | `db::models`, `db::traits`, `domain::*` | `narrative`, `agents`, `memory`, `creative_engine`, `story_system`, `pipeline` | ✅ 已落地 |
| `narrative` | 叙事结构分析、Genesis 流水线、LitSeg | `domain`, `db`, `llm`, `memory`, `router`, `story_system`, `strategy` | `creative_engine`, `agents` | 🟡 仍有 `audit.rs->creative_engine`, `search.rs->memory` 待清理 |
| `creative_engine` | 创作资产（风格、方法论、桥段卡、TimeSliced）与上下文构建 | `domain`, `db`, `llm`, `memory`, `router`, `story_system`, `task_system` | `agents` | 🟡 `context_builder.rs`, `workflow/engine.rs` 仍引用 agents 类型 |
| `agents` | 创作编排、prompt 组装、Writer/Inspector 闭环 | `domain`, `db`, `llm`, `memory`, `prompts`, `router`, `story_system`, `task_system` | `creative_engine`（应通过 domain 类型或 trait） | 🟡 service.rs 仍引用 creative_engine::methodology/style |
| `memory` | 知识图谱、向量索引、短期记忆、MemoryPack 编排 | `domain`, `db`, `llm`, `vector`, `embeddings`, `router` | `agents` | 🟡 `mod.rs` 仍有少量 agents 引用 |
| `agency` | 多代理创作框架（创世 2.0）：黑板 + 三角色 + 续写/写作主路径 + 质量门 + 投影/回灌/回归门 | `domain`, `db`, `llm`, `prompts`, `router`, `story_system`, `memory`, `creative_engine`, `vector`（经 ports trait） | `prompts`（反向；prompts 禁导入 agency） | ✅ 已落地 |
| `story_system` | 合同树、SceneCommit、追读力、投影写入 | `domain`, `db`, `llm`, `memory`, `vector`, `automation`, `state_sync` | `agents`, `creative_engine`（核心域应保持独立） | ✅ 已落地 |
| `reading_power` | 追读力评估与债务管理 | `domain`, `db` | `agents`, `creative_engine` | ✅ 已落地 |
| `audit` | 五维质量审计、OpeningClarityGate | `domain`, `db`, `creative_engine`, `llm` | `agents` | ✅ 已落地 |
| `strategy` | 题材解析、策略选择、四元组推断 | `domain`, `db`, `llm`, `skills`, `workflow` | `agents` | ✅ 已落地 |
| `pipeline` | 审稿/修稿/后处理流水线 | `domain`, `db`, `llm`, `creative_engine`, `story_system`, `task_system` | `agents` | ✅ 已落地 |
| `planner` | 执行计划生成、意图图调度回退 | `domain`, `db`, `llm`, `capabilities`, `router` | 直接依赖具体实现 | ✅ 已落地 |
| `intention_graph` | SING 意图图、ReAct、资产发现 | `domain`, `db`, `llm`, `capabilities` | `planner`（已消除循环） | ✅ 已落地 |
| `llm` | LLM 适配器抽象与统一调用 | `config`, `db`, `domain`, `events`, `memory`, `router` | `agents`, `creative_engine` | ✅ 已落地 |
| `commands` | Tauri IPC 命令薄层 | 可按需依赖各业务模块 | 应仅做参数校验与转发 | ✅ 基本合规 |
| `config` | 应用配置加载与默认值 | `db`, `error` | 业务模块 | ✅ 已落地 |
| `router` | 模型路由与任务分类 | `config`, `db`, `llm` | 业务模块 | ✅ 已落地 |
| `vector` | 向量存储抽象（LanceDB） | `db`, `error`, `embeddings` | 业务模块 | ✅ 已落地 |
| `embeddings` | Embedding 生成 Provider | `error`, `config` | 业务模块 | ✅ 已落地 |
| `book_deconstruction` | 拆书解析、分析、向量 embedding | `db`, `llm`, `narrative`, `vector`, `embeddings`, `task_system` | `agents`, `creative_engine`（拆书应为独立分析域） | ✅ 已落地 |

---

## 关键子模块登记（v0.60.0–v0.75.0 新增）

> 顶层模块内部的子模块只登记「跨模块被依赖 / 承载关键不变量」者，其余以源码文档为准。

| 子模块 | 职责一句话 | 引入版本 |
|---|---|---|
| `story_system::volume_plan` | 卷/弧中层计划：VOLUME 合同存取（同卷号原地更新）、卷纲状态机（no_plan/in_volume/past_end）、章纲注入块与指南针接卷 | v0.70.0 |
| `story_system::human_voice` | 人类文笔基线：确定性规则（8 条）+ 手法池（含稳定 id 与作者裁决加权选择）+ 语料校准探针 | v0.65.0 / v0.69.0 / v0.72.0 |
| `story_system::recompute` | 物料失效与指纹感知重算（章摘要 / 段摘要 / 快照；`chapter_outline` 单列不入 `ALL_KINDS`） | v0.64.11 / v0.67.0 |
| `agency::retrieval` | 续写按需检索门：节拍卡 → 查询 → 向量检索 → 只读注入块（失败静默降级） | v0.66.0 |
| `agency::outline_replan` | 纲层失效与局部重规划：候选判定 / 确有下游才标记 / 预览不落库 / 只落库作者勾选条目 | v0.67.0 |
| `agency::editor_agreement` | 判官一致率：gate 裁决落库（V146）+ 引用幸存判定 + 每章采纳率与修改率对照 | v0.68.0 |
| `agency::technique_feedback` | 手法回灌：每拍手法落库（V147）+ 反馈按时间窗归因 + 统计/重置 | v0.69.0 |
| `agency::serial_harness` | 长篇稳定性回归门（`#[cfg(test)]`）：四拍串联跨状态约束，随 `cargo test` 进 CI；守卫 `tests/serial_harness_guard.rs` | v0.71.0 |
| `emotion_circuit` | 情感回路领域层：12 维情绪向量 / OCC 评估 / 8 类载体 / 压力场选拍 / 调制器 / 确定性探针 / 读者面板 / 题材契约建议 / 白盒端口；V150 七张表 | v0.75.0 |
| `agency::emotion_commands` | 情感回路 IPC：契约读取/保存/按题材建议、快照、债务结算、读者面板 | v0.75.0 |

---

## 已知架构债务（待后续清理）

| 源模块 | 目标模块 | 位置 | 清理方向 |
|--------|----------|------|----------|
| `creative_engine` | `agents` | `context_builder.rs` | 将 `AgentContext` 等数据类型迁入 `domain` |
| `creative_engine` | `agents` | `workflow/engine.rs` | 引入 `AgentExecutor` trait，由 `agents` 实现 |
| `agents` | `creative_engine` | `service.rs` | 将 `MethodologyConfig` / `StyleDNA` 等纯数据类型迁入 `domain` |
| `memory` | `agents` | `mod.rs` | 将 `AgentContext` 等类型迁入 `domain` 后移除 |
| `narrative` | `creative_engine` | `audit.rs` | 审计应依赖 `audit` 模块或抽象，而非直接引用 creative_engine |
| `narrative` | `memory` | `search.rs` | 将共享的搜索 DTO 迁入 `domain` 或 `vector` |

---

## CI 集成

`.github/workflows/architecture-guard.yml` 会在每次 PR 时运行 `scripts/architecture_guard.py`。

本地手动检查：

```bash
python3 scripts/architecture_guard.py
```

---

## 更新规则

1. 新增模块时，必须在本表登记职责与依赖规则。
2. 放宽禁止依赖列表需要经过代码审查，并更新本文件。
3. 修复已知架构债务后，将对应条目从 `KNOWN_VIOLATIONS` 移入 `PROHIBITED`，并更新本文件。
