---
name: sf-validation-and-qa
description: StoryMoss 的证据标准、测试命令、门槛、golden inventory、如何加测试。何时加载：改完代码要跑验证、要加回归测试、CI 检查失败、要确认“算不算通过”、要跑跨层 golden fixture、或被问“怎么测/跑哪些/门槛是多少”时。
---

# StoryMoss 验证与 QA

## 证据标准（什么算“通过”）

本机基线（v0.26.23，`AGENTS.md`/`TESTING.md`）：

| 套件 | 命令 | 基线 | CI 行为 |
| --- | --- | --- | --- |
| Rust 单元 | `cd src-tauri && cargo test --lib` | 655 passed / 0 failed / 2 ignored | `continue-on-error: true`（历史 V092 基线备注）；Win 跳过 |
| Rust 编译 | `cargo check` | 零错误 | 阻塞 |
| Rust 格式 | `cargo +nightly fmt -- --check` | 零差异 | 阻塞 |
| Clippy | `cargo clippy` | 471 个历史警告未清零（非 `-D warnings`） | 不阻塞 |
| 前端类型 | `cd src-frontend && npx tsc --noEmit` | 零错误 | 阻塞 |
| 前端格式 | `npm run format:check` | 零差异 | 阻塞 |
| 前端单元 | `npm run test:run`（vitest） | 183 passed / 3 skipped | 阻塞 |
| 前端构建 | `npm run build` | 通过 | 阻塞 |
| E2E | `npx playwright test` / `npm test` | 41 (36+5) | `continue-on-error: true`（dev-server 缺后端，IPC 挂起） |
| 架构边界 | `python3 scripts/architecture_guard.py` | 通过 | （本地强制，CI 未单独 job） |
| IPC 一致性 | `python3 scripts/verify-ipc-manifest.py` | 零 ERROR | 前端 `npm run lint` 内含 |

**本地全绿门槛建议**（提交前最少跑）：
```bash
cd src-tauri && cargo test --lib && cargo check && cargo +nightly fmt -- --check
cd src-frontend && npx tsc --noEmit && npm run format:check && npm run test:run
python3 scripts/architecture_guard.py
```

## Golden / 契约 inventory（认证资产）

| 资产 | 位置 | 锁定什么 |
| --- | --- | --- |
| 跨层 trim golden | `tests/fixtures/trim_golden.json`（7 用例） | Rust `trim_self_repetition` 与 TS `trimSelfRepetition` 跨层一致；双跑 `trim_self_repetition_matches_shared_golden_fixture` + `textCleanup.golden.test.ts` |
| Genesis payload 契约 | `narrative/genesis.rs` 测试 | `select_first_chapter_content`/`build_first_chapter_chapter_switch`/`background_steps` 6 步固定顺序 |
| 8% 重试闸门 | `narrative/genesis.rs` 测试 | `compute_trim_ratio`/`should_retry_self_repetition` 边界 |
| IPC 一致性 | `scripts/verify-ipc-manifest.py` | `generate_handler![]` ↔ 前端 `loggedInvoke` |
| 架构边界 | `scripts/architecture_guard.py` | `db`/`domain` 依赖方向 + `FORBIDDEN_GLOBALS` |
| ts-rs 生成 | `src-frontend/src/generated/SyncEvent.ts` | Rust enum ↔ TS 穷尽匹配 |

## 测试文件分布（快速定位）

- Rust：`src-tauri/src/db/{repositories_tests,cascade_tests}.rs`、`canonical_state/tests.rs`、`task_system/{tests,integration_tests}.rs`、`prompts/registry.rs`、`creative_engine/anti_ai/`、`utils/{validation_tests,style_align,text,file}.rs`、`pipeline/{executor,refine,review}.rs`、`story_system/scene_service.rs`、`narrative/elements.rs`、`config/settings_tests.rs`。
- 前端：`src-frontend/src/frontstage/{hooks,components}/*.test.{ts,tsx}`、`utils/*.test.ts`、`hooks/*.test.ts`、`services/*.test.ts`、`textCleanup.golden.test.ts`。
- E2E：`e2e/*.spec.ts`（`storymoss`/`frontstage-editing`/`backstage-pages`/`navigation`/`context-menu`/`genesis-duplicate`/`performance/tiptap-benchmark`）。

## 如何加测试（按场景）

| 场景 | 放哪 | 注意 |
| --- | --- | --- |
| Rust 纯函数 | 同模块 `#[cfg(test)] mod tests` | 优先提取纯函数（`world_concept_for_character_prompt` 模式）便于测 |
| 跨层共享算法 | 加 `tests/fixtures/*.json` 用例，Rust + TS 双跑 | 修改一边必须同步另一边并跑双测 |
| 前端状态机/竞态 | `src-frontend/src/frontstage/**/*.test.tsx` | 用 `waitFor` 轮询替代固定 `setTimeout`（降低 brittleness） |
| IPC 契约 | 新增命令后跑 `verify-ipc-manifest.py` | 前端 default 分支加 `assertUnreachable` |
| E2E 行为 | `e2e/*.spec.ts` | 仅前端可测；需后端的路径目前不可靠（IPC 挂起） |
| 数据库迁移 | `db/repositories_tests.rs` + 迁移幂等 | 文件名 ASCII 短名 |

## 何时 NOT 用本技能

- CI 失败根因 → `sf-debugging-playbook`。
- 变更门禁/版本号 → `sf-change-control`。
- 诊断工具用法 → `sf-diagnostics-and-tooling`。

## 出处与维护

- 重验证命令：
  - `cd src-tauri && cargo test --lib 2>&1 | tail -5`（看 passed/failed 数）
  - `cd src-frontend && npm run test:run 2>&1 | tail -5`
  - `ls tests/fixtures/`（golden 是否还在）
  - `python3 scripts/architecture_guard.py; echo $?`
- 易漂移项：测试计数（每版变）、Clippy 警告数、E2E 用例数、CI workflow 的 `continue-on-error`。
- 最后核对：2026-07-07，v0.26.23（655 Rust / 183 前端 / 41 E2E）。
