# StoryForge Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格、工具配置与强制构建规则。

## 项目背景

**StoryForge (草苔)** — AI 辅助小说创作桌面应用

- **项目根目录**: `/Users/yuzaimu/projects/StoryForge`
- **版本**: v0.26.11
- **GitHub**: https://github.com/91zgaoge/StoryForge
- **技术栈**: Tauri 2.4 + Rust 1.95.0 + React 18 + TypeScript 5.8 + Vite 6 + SQLite + LanceDB
- **双界面**: 幕前 `/frontstage.html`（沉浸式写作），幕后 `/index.html`（工作室管理）

## 编码风格

- **Rust**: `snake_case`，`Result<T, E>`，异步 `async/await`，数据库 `rusqlite` + `r2d2`。
- **TypeScript**: `camelCase`，函数组件 + Hooks，Zustand 状态管理，TanStack Query 调用后端。

## 开发命令

```bash
# 前端开发服务器
cd src-frontend && npm run dev

# 启动 Tauri 桌面应用
cd src-tauri && cargo tauri dev

# 构建生产版本
cd src-tauri && cargo tauri build

# 测试与检查
cd src-tauri && cargo test --lib
cd src-frontend && npx tsc --noEmit
npx vitest run
npm test                              # Playwright E2E
node scripts/cdp-inspect.js           # CDP 截图
```

## 强制构建规则（用户级）

1. **每次修改代码后**：先推送到 GitHub，触发 GitHub Actions 全平台构建。
2. **推送后**：在本地执行 `cargo tauri build`，生成本平台安装包（macOS `.dmg` / Windows `.exe`+`.msi` / Linux `.AppImage`+`.deb`）。
3. **版本号统一**：`Git tag`、`Cargo.toml`、`src-tauri/tauri.conf.json`、`src-frontend/package.json` 必须一致。
4. **每次推送必须更新** `README.md` 与以下文档：`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`。
5. **版本标签**：每次推送使用新 tag，禁止 force push 覆盖已有 tag。
   ```bash
   git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z
   ```

## 提交信息格式

```
<type>: <subject>

type:
  feat / fix / docs / style / refactor / test / chore
```

## 重要文档

- [README.md](./README.md)
- [docs/USER_GUIDE.md](./docs/USER_GUIDE.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [TESTING.md](./TESTING.md)
- [CHANGELOG.md](./CHANGELOG.md)
- [ROADMAP.md](./ROADMAP.md)
- [docs/archive/AGENTS_HISTORY.md](./docs/archive/AGENTS_HISTORY.md) — 完整历史版本记录

## 当前编译状态

- `cargo check` ✅ 零错误
- `cargo test --lib` ✅ 632 passed / 0 failed / 2 ignored
- `npx tsc --noEmit` ✅ 零错误
- `npx vitest run` ✅ 147 passed / 3 skipped
- `cargo +nightly fmt -- --check` ✅
- `npm run format:check` ✅
- `python3 scripts/architecture_guard.py` ✅

## 最近完成的功能

### v0.26.11 — 修复 Genesis 第一章 store-editor 失步与崩溃隐患
- 修复 Genesis 自动接受第一章后，store 依赖 200ms onChange debounce 回写导致的 store-editor 失步。
- `appendAiContent` 追加后立即用 `editorRef.getHTML()` 同步 store 与 `latestContentRef`。
- `RichTextEditor.appendText` 空文档分支标记外部同步并更新 `lastExternalContentRef`，防止 content prop 被再次 setContent。
- `RichTextEditorRef` 新增 `getHTML()` 方法。
- 确认 `tauri.conf.json` `devUrl` 指向 dev server，避免开发时加载陈旧 dist 崩溃。

### v0.26.10 — 强化 Genesis 第一章重复防护（双重基准与追加最终防线）
- 修复 v0.26.9 单一 `latestContentRef` 基准与编辑器 DOM 短暂失步时，重复检测仍可能失效的问题。
- `isTextAlreadyInEditor`、`appendAiContent` 采用 `latestContentRef` + `editorRef.getText()` 双重基准。
- `appendAiContent` 增加正文前缀剥离安全网，并在追加后用 DOM 文本校准 `latestContentRef`。
- `RichTextEditor.appendText` 增加最终防线：编辑器尾部已包含待追加内容则直接跳过。

### v0.26.9 — 根治 Genesis 第一章重复（DOM 竞态与追加去重）
- 修复 TipTap DOM 状态滞后于 React `content` prop 时，重复检测依赖 `editorRef.getText()` 导致失效的问题。
- `isTextAlreadyInEditor`、`handleRequestGeneration`、`handleSmartGeneration`、`appendAiContent` 统一改用 `latestContentRef` 作为内容基准。
- `appendAiContent` 追加后立即同步 `latestContentRef`，避免 onChange debounce 窗口期内重复追加。
- `RichTextEditor` 幽灵文本直接包含检测剥离 HTML 标签，覆盖 ContentUpdate/AppendContent 路径。
- 新增 DOM 滞后竞态单元测试。

### v0.26.8 — 彻底修复 Genesis 第一章重复（竞态路径覆盖）
- 修复 `genesisAutoAcceptedRef` 无法覆盖 pipeline-complete 先加载 DB 正文竞态的问题。
- 新增 `isTextDuplicate` 归一化去重工具与 `isTextAlreadyInEditor` helper。
- `handleRequestGeneration` / `handleSmartGeneration` 设置幽灵文本前检测编辑器是否已包含生成内容。
- `pipeline-complete` 加载正文后标记 Genesis 已自动接受。

---

*最后更新: 2026-07-05 - v0.26.11*
