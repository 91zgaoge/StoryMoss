# StoryForge Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格和工具配置

## 🧠 永久记忆：自动化测试与产品文档

本项目已配置 **Playwright + Chromium** 无头浏览器自动化测试环境，以及可复用的 **product-docs** Skill，专为 AI 助手设计。

### 快速启动测试

```bash
# 运行完整 E2E 测试
npm test

# 使用 CDP 检查并截图所有关键页面
node scripts/cdp-inspect.js

# 仅截图幕前界面
npm run screenshot:front

# 仅截图幕后界面
npm run screenshot:back
```

### CDP 截图脚本

文件位置：`scripts/cdp-inspect.js`

使用 Playwright + `--remote-debugging-port=9223` 启动 Chromium，通过 CDP 导航每个视图并截图：

```bash
cd src-frontend && npm run dev    # 保持前端 dev server 运行
cd .. && node scripts/cdp-inspect.js
```

输出目录：`docs/product-screenshots/`，包含 `.png` 截图和同名的 `.json` 元素清单。

### 测试助手 API

文件位置：`e2e/test-helper.ts`

```typescript
import { runTest } from './e2e/test-helper';

runTest(async (helper) => {
  // 导航
  await helper.navigate('http://localhost:5173');

  // 截图
  await helper.screenshot('homepage');

  // 交互
  await helper.click('button');
  await helper.type('input[name="title"]', '测试标题');
  await helper.press('Enter');

  // 等待
  await helper.waitFor('.success-message');
  await helper.sleep(1000);

  // 执行 JS
  const title = await helper.eval<string>('document.title');
});
```

### 已配置的测试环境

| 组件 | 版本 | 路径 |
|------|------|------|
| Playwright | 1.59.1 | `e2e/` |
| Chromium | 系统安装 | `~/Library/Caches/ms-playwright/` |

### 测试文件位置

- E2E 测试：`e2e/*.spec.ts`
- 测试截图：`e2e/screenshots/`
- 产品截图：`docs/product-screenshots/`
- 测试报告：`playwright-report/`
- 配置：`playwright.config.ts`

---

## 📋 项目背景

**StoryForge (草苔)** - AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryForge`（永久记忆，AI 助手默认以此为工作目录）
- **版本**: v0.26.6
- **GitHub**: https://github.com/91zgaoge/StoryForge
- **技术栈**: Tauri 2.4 + Rust 1.95.0（通过 `rust-toolchain.toml` 固定） + React 18 + TypeScript 5.8 + Vite 6 + SQLite + LanceDB
- **构建锁定**: `Cargo.lock` 已纳入版本控制，确保 CI 与本地依赖解析一致

### 双界面架构

| 界面 | 用途 | URL |
|------|------|-----|
| 幕前 (Frontstage) | 沉浸式写作 | `/frontstage.html` |
| 幕后 (Backstage) | 工作室管理 | `/index.html` |

### Agent Skills（项目级）

| Skill | 用途 | 触发场景 |
|-------|------|----------|
| `brainstorming` | 创意探索、需求分析 | 新建功能或修改行为前 |
| `design` | UI/UX 设计 | 任何视觉界面改动 |
| `product-docs` | 生成/更新面向用户的产品说明文档 | 需要截图、写用户指南、沉淀可复用文档流程时 |
| `systematic-debugging` | 调试 bug、测试失败 | 遇到意外行为时 |
| `react-components` | Stitch 设计转 React 组件 | UI 实现 |

`product-docs` Skill 路径：`.agents/skills/product-docs/SKILL.md`。典型流程：启动 dev server → CDP 截图所有视图 → 提取 DOM 与交互元素 → 撰写 `docs/USER_GUIDE.md` → 沉淀截图到 `docs/product-screenshots/`。

---

## 🎨 编码风格

### Rust 后端

- 使用 `snake_case` 命名
- 错误处理使用 `Result<T, E>`
- 异步函数使用 `async/await`
- 数据库使用 `rusqlite` + `r2d2` 连接池

### TypeScript 前端

- 使用 `camelCase` 命名
- 组件使用函数式组件 + Hooks
- 状态管理使用 Zustand
- API 调用使用 TanStack Query

### 提交信息格式

```
<type>: <subject>

<body>

type:
  feat: 新功能
  fix: 修复
  docs: 文档
  style: 格式
  refactor: 重构
  test: 测试
  chore: 构建

### 🏷️ 版本标签规则（永久）

- **每次推送必须使用新 tag**（如 v0.23.66 → v0.23.73），禁止 force push 覆盖已有 tag
- 原因：① 新 tag 可靠触发 CI 构建 ② 版本可追溯 ③ 回滚安全
- 做法：`git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z`
```

---

## 🔧 开发命令

```bash
# 启动前端开发服务器（默认 http://127.0.0.1:5173/）
cd src-frontend && npm run dev

# 启动 Tauri 桌面应用
cd src-tauri && cargo tauri dev

# 构建生产版本
cd src-tauri && cargo tauri build

# Rust 测试
cd src-tauri && cargo test

# 前端类型检查
cd src-frontend && npx tsc --noEmit

# 运行 E2E 测试
npm test

# CDP 截图所有关键页面（需先启动 dev server）
node scripts/cdp-inspect.js
```

---

## 📚 重要文档

- [README.md](./README.md) - 项目概览与使用说明
- [docs/USER_GUIDE.md](./docs/USER_GUIDE.md) - 面向普通用户的完整产品说明（图文）
- [ARCHITECTURE.md](./ARCHITECTURE.md) - 架构设计
- [TESTING.md](./TESTING.md) - 测试文档
- [CHANGELOG.md](./CHANGELOG.md) - 更新日志
- [ROADMAP.md](./ROADMAP.md) - 开发路线

---


### 最近完成的功能

> 完整历史版本记录已归档到 [`docs/archive/AGENTS_HISTORY.md`](./docs/archive/AGENTS_HISTORY.md)。

- **v0.26.6 彻底修复第一章重复与页面崩溃** (2026-07-04) — 修复 `frontstage-update` 事件类型匹配错误（PascalCase → camelCase）、Genesis 自动加载正文后禁止恢复幽灵文本、`loadStoryWordCount` 空安全、`onChapterUpdated` 不必要状态更新、`selectChapter` 懒加载无限递归。验证：`cargo test --lib` 632/0/2，`npx tsc --noEmit` ✓，`npx vitest run` 136/3 skipped。

- **v0.26.0 数据飞轮、Harness 可观测性与子代理协作** (2026-07-04) — 工作空间 `.storyforge/`、反馈偏好对导出、生成链路 `TraceStore`、子代理协作（Continuity/Style/World）。

- **v0.25.0 Context Rot 显式防御 + 四级错误分类与恢复** (2026-07-03) — 上下文分块优先级、四级错误严重度/恢复策略、前端中断模态。

- **v0.23.74 场景优先架构迁移** (2026-06-28) — `scenes.content` 成为唯一真相源，编辑器主键切到 scene，Commit 触发点迁移，创世场景化。

---

### 编译状态

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 632 passed / 0 failed / 2 ignored
- `npx tsc --noEmit` ✅ 零错误
- `npx vitest run` ✅ 136 passed / 3 skipped
- `cargo +nightly fmt -- --check` ✅
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅

---

### 🏗️ 永久构建规则（用户强制要求）

> **每次修改代码后，先推送到 GitHub，触发 GitHub Actions 全平台构建。**
> **推送完成后，在本地执行构建并打包生成本平台安装包（macOS `.dmg` / Windows `.exe`+`.msi` / Linux `.AppImage`+`.deb`）。**
> **每次推送到 GitHub，都必须逐条更新 GitHub 项目的 `README.md` 文件内容。**
> **Git tag、Cargo.toml、`src-tauri/tauri.conf.json`、`src-frontend/package.json` 中的版本号必须保持统一。**

> **README.md 更新检查清单（推送前必做）：** 版本号一致、功能列表更新、截图更新、图标/Logo 最新、安装说明、使用指南、CHANGELOG 链接。

> **代码更新后必做：**
> - 重新构建应用包：`cargo tauri build`
> - 同步更新 `CHANGELOG.md`、`README.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`。

> **🧪 真实模型全流程测试（推送前必做）：**
> - 确认真实模型端点可达
> - 运行 `cargo test --lib -- --ignored --nocapture`
> - 覆盖创作生成、续写、润色、检查、修改、规划 6 类意图
> - 验证意图图发现资产不为空
> - 检查 LLM 输出格式兼容性
> - 验证 PPR 分层发现生效

> **🧠 AI 创作工具交互设计原则：** 智能判断意图并主动调整状态；减少用户操作步骤；避免用弹窗要求用户做本应由 AI 自动完成的事。

> **🌿 「越写越懂」核心理念：** StoryForge 是理解用户意图并智能化调用全套创作工具的 AI 导演式创作系统。

**本地构建：**
```bash
cd src-tauri && cargo tauri build
```

**平台构建现实：**
- macOS 主机 ✅ 可本地构建 `.app`/`.dmg`
- Windows 主机 ✅ 需 Visual Studio 生成工具
- Linux 主机 ⚠️ 需对应工具链
- 跨平台完整构建 → GitHub Actions

---

## 🏛️ Spec-Kit 集成

本项目使用 Spec-Driven Development (SDD)。关键命令：`/skill:speckit-specify` → `/skill:speckit-plan` → `/skill:speckit-tasks` → `/skill:speckit-implement`。项目宪法见 `.specify/memory/constitution.md`。

---

## Agent skills

- **Issue tracker**: GitHub Issues，使用 `gh` CLI。详见 `docs/agents/issue-tracker.md`。
- **Triage labels**: `needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`。详见 `docs/agents/triage-labels.md`。
- **Domain docs**: 多上下文布局，见 `CONTEXT-MAP.md` 与 `docs/agents/domain.md`。

---

*最后更新: 2026-07-04 - v0.26.6*

### 重要参考文档
- [docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md](./docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md) — 后台创作资产清单与断链审计
- [docs/archive/AGENTS_HISTORY.md](./docs/archive/AGENTS_HISTORY.md) — 完整历史版本记录

### 当前编译状态
- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 632 passed / 0 failed / 2 ignored
- `npx tsc --noEmit` ✅ 零错误
- `npx vitest run` ✅ 136 passed / 3 skipped
- `cargo +nightly fmt -- --check` ✅
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **StoryForge** (14625 symbols, 24467 relationships, 293 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/StoryForge/context` | Codebase overview, check index freshness |
| `gitnexus://repo/StoryForge/clusters` | All functional areas |
| `gitnexus://repo/StoryForge/processes` | All execution flows |
| `gitnexus://repo/StoryForge/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
