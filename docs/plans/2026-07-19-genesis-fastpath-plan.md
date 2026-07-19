# 创世提速：快速路径（Fast Genesis）计划

> 状态：已批准（用户明确要求：首章 ≤3 分钟、主创模型优先、其它角色不抢主创模型）

## 目标

1. 首章正文产出 ≤3 分钟（典型远程模型 15-30s/调用 → 60-120s 完成）。
2. 主创（LeadWriter）模型优先：**多模型（≥2 个可用生成模型）时，Producer/Editor 不得与主创使用同一模型**；**单模型时主创先跑**（先出首章，资产与审查随后）。
3. 不破坏：Gate v2（含 spec 5.5）、资产落库、smart_execute 前端兼容契约、既有测试基线（877 rust / 295 vitest）。

## 根因（已核实）

- 创世为 6 阶段串行，共 **12~18 次 LLM 调用**：concept 1 + producer ToolLoop 7~8 轮（≥5 次 board_write + 读 + final）+ writer 3~4 轮 + editor 1~2 + 可能修订 2~4。慢速模型下必超 `smart_execute_total_timeout_secs`（默认 600s，配置加载失败回退 180s）。
- 网关 `pick_fastest_for_role`（Tool 档，executor.rs:165-191）**不排除 active/creative** → producer 在多模型配置下也可能抢到主创模型。

## 方案

**三调用快速路径**（LLM 调用从 12-18 降至 4，关键路径 ~3）：

```
Phase A（1 调用，Tool 档）      concept pack：{title, genre, logline, characters[]}
Phase B（并行）                  writer 单调用首章（Creative 档）∥ producer 深度单调用（world/outline/foreshadowing）
Phase C（1 调用，Background 档） editor 质量门（复用 evaluate_gate + 资产上下文）
```

- **单模型（generative_models ≤1）串行**：A → **writer 先跑** → producer → gate。用户在最短等待后看到正文，资产/审查在后。
- **回退**：任一单调用 parse 失败 → 回退现有串行 ToolLoop 路径（run_genesis_inner 原六阶段），行为不劣于现状。
- **网关互斥**：`pick_fastest_for_role` 在 ≥2 个可用生成模型时排除 active/creative（与 `pick_idle_for_background` 既有避让对齐）。
- **超时回退统一**：smart_execute 配置加载失败回退 180 → 600（与 serde 默认一致）+ 陈旧注释修正。

## 任务

- **T1 网关模型互斥 + 超时统一**：`model_gateway/executor.rs` 的 `pick_fastest_for_role` 排除逻辑（active/creative 仅在 ≥2 可用时排除，单模型回退允许）；`commands/orchestrator.rs` 回退 180→600；测试（多模型排除/单模型回退）。
- **T2 Genesis 快速路径**：coordinator.rs 新增 `concept_pack`/`producer_depth_assets`/`writer_first_chapter` 三个单调用辅助（BudgetedLlm 计量保留）+ 并行编排（tokio::join!）+ 单模型串行编排 + parse 回退 legacy + 模型数检测（`AppConfig::load` + `UnifiedModelRegistry::from_app_config`，生产有 app_handle；测试可注入）+ Board 写入（coordinator 以各角色身份直写，zone owner 语义保持）+ materialize 落库。
- **T3 双路径测试 + 文档 + v0.30.1**：mock 验证并行路径调用序与单模型路径 writer 先跑；ARCHITECTURE/CHANGELOG/README 更新；版本 0.30.0 → 0.30.1（四处 + lockfile）。

## 验收

- mock 双路径单测全绿；全量 `cargo test --lib`（877+）与 vitest（295）全绿。
- 真机：以「写一部现代都市小说」创世，远程模型首章 ≤3 分钟；单模型配置下 writer 先于 producer/editor 获得模型。
