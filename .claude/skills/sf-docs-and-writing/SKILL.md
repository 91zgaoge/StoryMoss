---
name: sf-docs-and-writing
description: StoryMoss 文档维护规范、docs of record 清单、版本号同步、提交信息与 house style。何时加载：要发布或推送、要写/更新文档、要 bump 版本、要写 commit message、或被问“文档怎么写/要更新哪些/版本号在哪”时。
---

# StoryMoss 文档与写作

## Docs of record（每次推送必须同步，AGENTS.md 强制）

| 文档 | 用途 | 更新时机 |
| --- | --- | --- |
| `README.md` | 面向用户的总览 + 安装 + 截图 | 用户可见行为变化、新功能、新版本动态 |
| `CHANGELOG.md` | 版本变更日志 | 每次版本 bump |
| `AGENTS.md` | AI 助手项目指南（背景/编码风格/命令/强制规则/最近完成） | 每次推送（最近完成功能段） |
| `PROJECT_STATUS.md` | 项目状态 | 每次推送 |
| `ROADMAP.md` | 路线图 + 已知债务 | 每次推送；新增债务/暂缓决策必须登记 |
| `ARCHITECTURE.md` | 系统架构（含分时介入、合同驱动、记忆系统等） | 架构变化 |
| `TESTING.md` | 测试环境与统计 | 测试数量/分布变化 |
| `docs/USER_GUIDE.md` | 完整用户指南 | 用户操作路径变化（如 auto-accept 真实路径） |

## 版本号四源统一（强制）

> 规则的家在 `sf-change-control`（含违反代价与历史事故）。本技能只记 docs 视角：写文档时引用的版本号必须与四源一致——`src-tauri/Cargo.toml` 的 `version`、`src-tauri/tauri.conf.json` 的 `"version"`、`src-frontend/package.json` 的 `"version"`、`git tag vX.Y.Z`。`Cargo.lock` 也需同步（曾出现需 `chore: sync Cargo.lock`）。

## 提交信息格式

```
<type>: <subject>

type: feat / fix / docs / style / refactor / test / chore
```

历史风格（`git log --oneline -30`）：
- `fix: v0.26.23 修复 v0.26.22 CI prettier 格式检查失败`
- `feat: v0.26.19 Genesis 创世流程全面审计与测试加固`
- `style: v0.26.20 修复 v0.26.19 CI cargo +nightly fmt -- --check 失败`
- `chore: sync Cargo.lock version to v0.26.17`

约定：subject 常带版本号；多版本连发修 CI 格式时用 `style:`/`fix:`。

## House style

- **语言**：项目主语言中文；文档与代码注释中文为主，技术名词保留英文。
- **Rust**：`snake_case`，`Result<T,E>`，`async/await`，`rusqlite`+`r2d2`。
- **TypeScript**：`camelCase`，函数组件 + Hooks，Zustand，TanStack Query。
- **CHANGELOG/AGENTS 历史段**：用「根因→修复→验证」三段式，附测试计数变化（如 `cargo test --lib 655 passed (+10)`）。
- **新增债务**：在 `ROADMAP.md` 的「已知债务」段登记，含暂缓原因 + 重评估条件（参考「策略选择移入 quick_phase」条目）。

## 何时 NOT 用本技能

- 发布命令解剖 → `sf-run-and-operate`。
- 门禁规则（tag 不覆盖、CI 必查）→ `sf-change-control`。
- 文档里要写的架构内容 → `sf-architecture-contract`。

## 出处与维护

- 重验证命令：
  - `grep -rE '^version' src-tauri/Cargo.toml src-frontend/package.json` + `grep '"version"' src-tauri/tauri.conf.json` + `git tag --sort=-creatordate | head -3`
  - `git log --oneline -10`（提交风格）
  - `ls docs/ docs/archive/`（文档清单）
- 易漂移项：版本号、最近完成功能段（AGENTS.md）、ROADMAP 已知债务。
- 最后核对：2026-07-07，v0.26.23。
