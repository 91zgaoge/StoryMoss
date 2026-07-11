---
name: sf-change-control
description: StoryMoss 变更门禁与不可破坏项。何时加载：要提交/推送代码、要 bump 版本、要改架构边界、要动数据库迁移、要改 CI、要发布 tag、要合并 feature 分支、或被问“能不能直接改/直接推”时。也用于判断一个改动是否需要用户授权（R10）。
---

# StoryMoss 变更控制

本项目已有一套带血泪史的强制规则。**先读这一条再动手**：违反任何一条都会触发 CI 失败、用户返工、或已修复的 bug 复活。

## 不可破坏项（每条都附历史事故）

| 规则 | 要求 | 违反的代价（已发生） |
| --- | --- | --- |
| 版本号四源统一 | `git tag`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src-frontend/package.json` 版本一致 | 升级器/CI 版本探测错位；release 产物版本与 tag 不符 |
| 每次推送必更新文档 | 同步更新 `README.md`、`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md` | 用户依据陈旧文档诊断，反复返工 |
| 推送后必查 CI | 推送后立即 `gh run list --limit 3`，监控到全绿（见 `sf-validation-and-qa`） | v0.26.19–.21 多版本积压的 macOS 公证过期、Windows WiX 中文文件名问题，因“推完就走”拖了多版才被发现 |
| tag 不覆盖 | `git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z`，禁止 force push 已有 tag | 升级器 `latest.json` 指向错乱 |
| 迁移文件名 ASCII 短名 | 新迁移文件名只允许 `V###__ascii_short.sql`，禁止中文/全角标点/破折号，长度尽量短 | v0.26.21：WiX `light.exe` 标识符生成失败导致 Windows MSI 整个构建挂掉 |
| 后台 LLM 调用必须静默化 | 任何后台（非用户等待路径）LLM 调用的 `context_label` 必须加入 `is_silent_background` 白名单（`src-tauri/src/llm/service.rs`） | v0.23.45 / v0.26.23：后台 Ingest/auto_contract 进度事件覆盖前端主活动，导致“正文已返回但界面卡死” |
| 不要把后台 DB 写入同步阻塞 tokio worker | `record_llm_call` 等高频 DB 写入走 `spawn_blocking` fire-and-forget | v0.23.19：600s 超时，`pool.get()` 无限阻塞 |
| `std::sync::Mutex` 不可重入 | 同一线程持锁期间禁止再次 lock 同一锁；中毒锁用 `unwrap_or_else(\|e\| e.into_inner())` 恢复 | v0.23.34 select_candidates 自死锁；v0.23.42 BGP-4 自死锁 |
| 不动 `rustfmt.toml` 跨平台行为 | 不要开启 `format_strings`（macOS/Windows nightly 行为不一致） | CI `cargo +nightly fmt -- --check` 在不同平台结果不同 |
| 架构边界 | `python3 scripts/architecture_guard.py` 必须通过；`db` 禁止依赖 `narrative/agents/memory/creative_engine/story_system/pipeline`；`domain` 禁止依赖任何业务模块；禁止重新引入 `FORBIDDEN_GLOBALS`（`VECTOR_STORE/DB_POOL/LLM_SERVICE/APP_CONFIG/SKILL_MANAGER/...`） | v0.23.1 单例清零与循环依赖斩断的成果回潮 |

## 变更分类与门禁

> **提交门 vs 推送门**：下表是*提交*门禁（能否 commit）。**推送**一律触发推送门：四源版本号统一 + docs of record 同步（AGENTS.md 强制规则 4）+ 推送后必查 CI（`sf-validation-and-qa` / `.cursor/rules/post-push-ci-check.mdc`），与变更类别无关。即便是"可逆/本地"类，一旦 push 就走完整推送门。

| 类别 | 例子 | 提交门 |
| --- | --- | --- |
| **可逆 / 本地** | 单文件 bug 修复、新增测试、注释 | 本地 `cargo test --lib` + `npx tsc --noEmit` + `cargo +nightly fmt -- --check` + `npm run format:check` 全绿即可提交 |
| **跨层** | 改 IPC 命令、SyncEvent variant、DTO | 必须跑 `scripts/verify-ipc-manifest.py`；前端 `assertUnreachable` 兜底；ts-rs 重新生成 |
| **跨层共享算法** | 改 `trim_self_repetition`（Rust）或 `trimSelfRepetition`（TS） | 必须双跑 `tests/fixtures/trim_golden.json`（Rust `trim_self_repetition_matches_shared_golden_fixture` + TS `textCleanup.golden.test.ts`） |
| **架构边界** | 改 `db` ↔ `narrative` 依赖、重新引入全局单例 | `architecture_guard.py` 必须通过；HIGH 风险需用户授权（R10） |
| **数据库迁移** | 新增 `.sql` 文件 | 文件名 ASCII 短名 `V###__*.sql`；幂等（`duplicate column`/`already exists` 跳过）；本地 `cargo test --lib` 跑迁移测试 |
| **用户契约 / 不可逆** | 改 quick phase 30-60s 返回首章的承诺、改 tag 策略、改升级器端点 | R10：必须用户授权；先在 ROADMAP 记录为债务并量化 |

## 提交信息格式

```
<type>: <subject>

type: feat / fix / docs / style / refactor / test / chore
```

历史风格见 `git log --oneline -30`：`fix: v0.26.23 修复...`、`feat: v0.26.19 ...`。

## 何时 NOT 用本技能

- 纯探索 / 读代码 → 用 `sf-architecture-contract` 或 `sf-reference`。
- 调试具体 bug → 用 `sf-debugging-playbook`。
- 改完后要跑哪些测试 → 用 `sf-validation-and-qa`。
- 发布流程的命令解剖 → 用 `sf-run-and-operate`。

## 出处与维护

- 强制规则来源：`CLAUDE.md`（META charter + Zero-Pause）、`AGENTS.md`（强制构建规则）、`.cursor/rules/post-push-ci-check.mdc`。
- 重验证命令：
  - `git log --oneline -5`（看最近提交风格与版本号）
  - `grep -rE '^version' src-tauri/Cargo.toml src-frontend/package.json`（四源版本是否一致；另查 `tauri.conf.json` 的 `"version"`）
  - `python3 scripts/architecture_guard.py`（边界是否仍通过）
  - `gh run list --limit 3`（最近 CI 状态）
- 易漂移项：版本号（每次发布必查）、CI workflow（`.github/workflows/build.yml`）、`is_silent_background` 白名单（`src-tauri/src/llm/service.rs`）。
- 最后核对：2026-07-07，v0.26.23。
