---
name: sf-diagnostics-and-tooling
description: StoryForge 诊断与度量工具及解读指南（“用仪器量，别用眼睛看”）。何时加载：要定位卡在哪一步、要看生成链路日志、要截图、要跑集成 E2E、要验证 IPC/架构边界、要用 GitNexus 查调用图、或被问“怎么看当前状态/卡在哪/怎么截屏”时。
---

# StoryForge 诊断与工具

> R5：执行是 ground truth，观察是假设。能用仪器量就不要用眼睛看。

## 生成链路日志（首选）

`<app_data_dir>/logs/creative_workflow.log` —— `WorkflowLogger`（`src-tauri/src/workflow_logger.rs`）形式化记录 TriShot/LLM/ModelGateway 各阶段。

关键阶段标记（卡点 = 时间线断点）：`genesis.first_chapter.generated` / `genesis.chapter_switch.sent` / `genesis.final_content` / `smart_execute.start` / `trishot.call3.done` / `trishot.bgp4.spawn` / `trishot.bgp4.done` / `llm.record_call.spawn`。注：候选模型实时探测（5s 超时）走标准 `log::debug!`（`[Gateway] 候选 [N] 实时探测通过/失败/超时`），**不进** `creative_workflow.log`——查探测去标准日志 sink。

> **怎么用这套日志定位卡点**：见 `sf-debugging-playbook` 的「`creative_workflow.log` 时间线对照法」（该方法的家在调试手册，本技能只列工具与标记）。v0.26.23 用此法定位 `auto_contract` 阻塞 6 分钟。

## 诊断卡片（UI）

幕前/幕后设置页诊断卡片显示：AI 生成模式、当前模型 ID/名/Provider/端点、最后调用模型、**最后发给 LLM 的提示词全文**、工作流日志路径与最近条目。后端 `get_last_llm_prompt` 命令 + `DiagnosticStore`（避免大提示词事件丢失）。`log_frontend_event` 命令允许前端写 WorkflowLogger。

## 前端诊断日志

- `[DEBUG-dup]` / `[DEBUG-act]` console.warn（Genesis 第一章 saga 诊断）。
- `frontstage:rich_editor_diag`：前 20 帧渲染 + 幽灵/隐藏锁状态变化 + 200ms IPC 节流（v0.26.14 收紧，防长写作 IPC 过载）。
- `ErrorBoundary` 增强崩溃诊断输出（v0.26.12）。

## 脚本工具

| 脚本 | 用途 |
| --- | --- |
| `scripts/cdp-inspect.js` | CDP 截图页面 |
| `scripts/test-helper.js` | `start/test/screenshot/clean/report`（Playwright 辅助） |
| `scripts/sf_smart_creation_e2e.py` | 智能创作流程 E2E |
| `scripts/test_trishot_e2e.py` | TriShot E2E（v0.23.16：73.2s 完成，1852 中文字） |
| `scripts/architecture_guard.py` | 架构边界 + `FORBIDDEN_GLOBALS` 守护 |
| `scripts/verify-ipc-manifest.py` | `generate_handler![]` ↔ 前端 `loggedInvoke` 一致性 |
| `scripts/migrate-genres-to-json.py` | 体裁模板迁移到 `genres.json` |

截图：`npm run screenshot` / `screenshot:front` / `screenshot:back`；产物 `e2e/screenshots/`。

## GitNexus（代码图谱，CLAUDE.md 强制）

本项目已索引为 StoryForge（14625 symbols / 24467 关系 / 293 执行流）。

- **改任何符号前**：`gitnexus_impact({target, direction:"upstream"})` 看爆炸半径；HIGH/CRITICAL 先告知用户。
- **探索不熟代码**：`gitnexus_query({query:"概念"})` 按执行流分组返回。
- **符号全貌**：`gitnexus_context({name})`。
- **重命名**：用 `gitnexus_rename`（理解调用图），**禁止 find-and-replace**。
- **提交前**：`gitnexus_detect_changes()` 验证只影响预期符号。
- 若提示索引陈旧：`npx gitnexus analyze`。

## 度量脚本（性能分支）

性能回归用计时/剖析，不要靠日志猜：
- TipTap 基准：`e2e/performance/tiptap-benchmark.spec.ts`（默认跳过）。
- 浏览器剖析：CDP `Profiler.start/stop`，profile 落 `log_file`。
- 连接池超时：`.connection_timeout` 已设；DB 阻塞用 `spawn_blocking` 标记 `record_call.spawn` 追踪。

## 度量什么算证据（见 `sf-validation-and-qa`）

- 跨层算法：`tests/fixtures/trim_golden.json` 双跑。
- IPC：`verify-ipc-manifest.py` 零 ERROR。
- 架构：`architecture_guard.py` 退出 0。

## 何时 NOT 用本技能

- 跑哪些测试 + 门槛 → `sf-validation-and-qa`。
- 症状分诊 → `sf-debugging-playbook`。
- CI 失败根因 → `sf-debugging-playbook` + `sf-change-control`（推送后必查）。

## 出处与维护

- 重验证命令：
  - `rg -n 'try_state::<.*WorkflowLogger>' src-tauri/src | head`
  - `ls scripts/`
  - `npx gitnexus status` 或读 `gitnexus://repo/StoryForge/context`
  - `node scripts/test-helper.js help`
- 易漂移项：日志阶段标记名、脚本清单、GitNexus 索引规模。
- 最后核对：2026-07-07，v0.26.23。
