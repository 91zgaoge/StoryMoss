# D0 — 拆书分块止血 Implementation Plan

> **For agentic workers:** Execute task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 让拆书真正按「应用分块调度」跑完：短篇不再整本只截头、中长篇不再被 300s 墙钟掐死、并发滑条真正并行。

**Architecture:** 不改上传入口；改 `create_chunks` 策略、`TaskService` 对 `book_deconstruction` 的墙钟语义、`AnalysisPipeline` 角色/场景步的 `buffer_unordered` 并行，并把 `book_deconstruction_concurrency` 注入 `AnalysisContext`。

**Non-goals (D1+):** chunks 表持久化、断点续跑、分层 map-reduce 归纳。

**Tech Stack:** Rust chunker / AnalysisPipeline / TaskService / futures::stream

---

## Success criteria

| ID | 标准 |
|----|------|
| D0-S1 | `book_deconstruction` 墙钟 ≥12h 或禁用；心跳仍 300–600s 无进度则杀 |
| D0-S2 | ≤10 万字且多章 → 多 chunk；单章大块 → 再切；无 `Full` 单块吞全书 |
| D0-S3 | 角色/场景步按 concurrency 并行；设置滑条生效 |
| D0-S4 | 相关单测绿；architecture_guard 绿 |

---

## File map

| File | Change |
|------|--------|
| `task_system/service.rs` | book_deconstruction 不用 heartbeat 当墙钟 |
| `book_deconstruction/service.rs` | heartbeat 600；文档注释 |
| `book_deconstruction/chunker.rs` | 废除 Full 实义；短篇按章/叙事切；大章再切 |
| `narrative/analysis.rs` | concurrency 注入；角色/场景 `buffer_unordered` |
| `book_deconstruction/executor.rs` | 读配置传入 concurrency |

---

## Task D0.1 — 超时语义

- [x] 墙钟与心跳分离：`book_deconstruction` → 12h；其它任务保持原逻辑
- [x] 拆书 `heartbeat_timeout_seconds: 600`
- [x] 契约测试 `wall_clock_tests`

## Task D0.2 — 强制有界分块

- [x] 短篇按章/固定窗口；废除 Full 单块吞全书
- [x] 大章再切；blob 防御再切
- [x] 单测：多章 ≥3 chunks；blob 再切

## Task D0.3 — 真并行 + 配置贯通

- [x] `AnalysisContext::with_concurrency`
- [x] executor 读 `book_deconstruction_concurrency`
- [x] Character/Scene `buffer_unordered(concurrency)`

## Task D0.4 — 验证

- [x] 相关单测绿；fmt / architecture_guard 通过
- [ ] 全量发布门（bump / 文档 / 推送）— 待用户确认
