---
name: sf-config-and-flags
description: StoryMoss 全部配置轴：选项、默认值、生产 vs 实验、守卫、如何新增。何时加载：要改生成模式、要改并发/超时、要加 PromptRegistry 覆盖、要调体裁模板、要改 silent_background 白名单、要动 rustfmt、或被问“在哪配/默认是什么/这个开关干啥”时。
---

# StoryMoss 配置与开关

## 生成模式（GenerationMode）

`src-tauri/src/agents/orchestrator.rs` 等。四值并存：

| 模式 | 路径 | 用途 | 默认 |
| --- | --- | --- | --- |
| `Fast` | Ghost Text 实时补全 | 单句续写 | 幕前幽灵文本 |
| `TimeSliced` | 三时间线（写/审/洞察） | 普通生成 / auto_write / auto_revise | **默认** |
| `Full` | 同步审计 + Rewrite 闭环 | 向导 / Genesis / Planner / Workflow | 创世等高质量路径 |
| `TriShot` | Call1→Call2→Call3 + 预算守卫 | 快速首章 / PlanExecutor 快路径 | 可在设置页选 |

切换：前端「三击模式」配置项；TimeSliced 写作策略从 `AppConfig` 读取用户配置。

## 并发 / 超时 / 守卫

| 开关 | 位置 | 默认 | 说明 |
| --- | --- | --- | --- |
| `BACKGROUND_LLM_SEMAPHORE` | `agents/orchestrator.rs:40`（定义；`llm/service.rs` 引用） | 1（串行） | 后台 LLM 调用全局串行化；`ParallelWorldOutlineCharacterStep` 已由 `tokio::join!` 3 路改串行 + 此信号量全覆盖 |
| `is_silent_background` 白名单 | `src-tauri/src/llm/service.rs` | 见文件 | 后台 LLM 调用 label 加入后不触发前端主活动；**新增后台 LLM 调用必须登记** |
| 候选超时 | `config/settings_tests.rs` 默认 | 远程 `candidate_timeout_seconds=180` / 本地 `candidate_timeout_local_seconds=120` | 默认值；用户可在设置改 |
| 续写 call3 超时 | `agents/orchestrator.rs`（v0.26.22 Bug C 注释，约 1599 行） | 60s（原 120s） | 慢模型 fail-fast 回退快模型 |
| 死模型退避 | 网关 | 30→60→120→…→3600s | 指数退避 |
| keepalive 刷新 | 网关 | 10s | `is_health_fresh()` 跳过内联 5s 探测 |
| `SceneCommitDebouncer` | `story_system/scene_service.rs` | 30s | 场景内容变更后防抖 auto_commit |
| 自动保存 debounce | 前端编辑器 | 2s / 200ms（onChange） | store 回写 |
| LLM 输出自重复闸门 | `narrative/genesis.rs` | ≥8% 重试一次 | anti-repeat 指令；prompt 模板含「结构纪律」段 |

## 提示词注册表（PromptRegistry，v0.19.0）

`src-tauri/src/prompts/registry.rs`：**21 个 `PromptCategory`**（枚举实测）。内置 prompt 数随版本增长，**以 `registry.rs` 源码为准**（ROADMAP v0.19.0 记 79，`rg -c 'name:' src-tauri/src/prompts/registry.rs` 实测约 61——以源码为准，文档数字可能滞后）。`resolve_prompt()` 运行时优先读 DB 覆盖。前端「设置→提示词」可编辑/批量重置/导入导出。新增 prompt：注册到 `registry.rs`，消费方（Writer/Inspector/Commentator/Planner/Analyzer/Probe/Memory/Knowledge/Skill/Methodology）通过 `resolve_prompt_content` 取。

## 体裁模板（GenreProfile）

启动时优先读 `<app_data_dir>/templates/genres.json`，缺失回退内置 **43 体裁**（`templates/genres.json` `count:43`；`strategy/models.rs` "43 个网文模板"）。模板五要素：核心基调、节奏策略、反模式清单、参考数据表、典型结构。`GenreResolver` 解析复合题材（v0.22.4 异星球末世生存）。新增体裁：编辑 `genres.json` 或 Migration 96+ 的种子数据。

## 类型安全基座

- `SyncEvent`/`FrontstageEvent`/`BackstageEvent` 加 `#[derive(TS)]` → 自动生成到 `src-frontend/src/generated/`。
- 前端 `assertUnreachable(x: never)` 在 default 分支兜底，新增 variant 时编译失败。
- `scripts/verify-ipc-manifest.py` 解析 `generate_handler![]` 与前端 `loggedInvoke`，未注册命令报 ERROR。

## 格式化配置（不要乱动）

`rustfmt.toml`（仓库根目录）：`max_width=100`、`imports_granularity=Crate`、`group_imports=StdExternalCrate`、`wrap_comments=true`。**`format_strings` 已注释禁用**（跨平台 nightly 不一致）。前端 `prettier` 默认配置。CI 在 `src-tauri/` 下跑 `cargo +nightly fmt -- --check`（rustfmt 会向上查找根目录 `rustfmt.toml`），前端跑 `npm run format:check`。

## 如何新增一个配置轴（清单）

1. 在 `AppConfig`（`src-tauri/src/config/`）加字段 + 默认值 + serde 兼容（`#[serde(default)]`）。
2. 加配置命令（`commands/`）+ DTO。
3. 前端 `services/api/settings.ts` 加 IPC 调用 + 设置页 UI。
4. 写 `config/settings_tests.rs` 测试（默认值/round-trip/向后兼容）。
5. 若影响生成行为，更新 `ARCHITECTURE.md` 与 `docs/USER_GUIDE.md`。
6. 重验证：`cargo test --lib config`、`npx tsc --noEmit`。
7. **走 `sf-change-control`**（改默认值/影响生成行为属变更，可能需用户授权 R10）+ **`sf-validation-and-qa`**（证据标准）+ 推送门（docs 同步 + CI 必查）。

## 何时 NOT 用本技能

- 配置改完要跑什么 → `sf-validation-and-qa`。
- 配置项背后的架构意图 → `sf-architecture-contract`。
- 后台静默化的根因 → `sf-debugging-playbook`（卡死陷阱）。

## 出处与维护

- 重验证命令：
  - `rg -n 'is_silent_background|BACKGROUND_LLM_SEMAPHORE' src-tauri/src/llm`
  - `rg -n 'GenerationMode::' src-tauri/src | head -20`
  - `cat rustfmt.toml`（仓库根，不是 src-tauri/）
  - `rg -n 'resolve_prompt' src-tauri/src/prompts/registry.rs | head`
- 易漂移项：超时常量、白名单 label、体裁数量、prompt 数量、rustfmt 项。
- 最后核对：2026-07-07，v0.26.23。
