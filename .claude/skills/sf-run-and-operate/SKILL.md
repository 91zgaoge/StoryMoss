---
name: sf-run-and-operate
description: 运行/部署 StoryForge 的命令解剖、运行时数据与产物落点。何时加载：要启动应用、要找日志、要找数据库/配置文件、要发布 tag、要看生成链路日志、或被问“日志在哪/数据在哪/怎么发布”时。
---

# StoryForge 运行与操作

## 启动命令解剖

```bash
# 桌面应用（推荐）：自动起前端 dev server + 开两个 Tauri 窗口
cd src-tauri && cargo tauri dev
#   beforeDevCommand: npm run --prefix src-frontend dev   → Vite @ :5173
#   beforeBuildCommand 不触发（dev 模式）
#   打开 window "frontstage"（frontstage.html，可见）+ "backstage"（index.html，默认隐藏）
```

两个窗口（`src-tauri/tauri.conf.json`）：
- `frontstage`：草苔 - 幕前创作，`frontstage.html`，1400×900，启动可见。
- `backstage`：草苔 - 幕后工作室，`index.html`，1200×800，启动隐藏（幕前点「草苔」按钮显示）。

仅前端（无后端，IPC 会挂起，仅用于 UI 调试）：
```bash
cd src-frontend && npm run dev   # http://localhost:5173/
```

## 运行时数据落点

| 内容 | 位置 | 说明 |
| --- | --- | --- |
| 应用数据根 | `<app_data_dir>/` | Tauri `app_data_dir()`；macOS `~/Library/Application Support/com.storyforge.app/`，Win `%APPDATA%\com.storyforge.app\`，Linux `~/.local/share/com.storyforge.app/` |
| SQLite 数据库 | `<app_data_dir>/` 下 | 由 `init_db` 创建；删掉可重置 |
| 生成链路日志 | `<app_data_dir>/logs/creative_workflow.log` | `WorkflowLogger`（`src-tauri/src/workflow_logger.rs`）；诊断卡片会显示路径与最近日志 |
| 全局配置 | `<app_data_dir>/config.json` 等 | `AppConfig` |
| 工作室配置 | `<app_data_dir>/studios/{story_id}/` | `studio.json`、`llm_config.json` 等 |
| 体裁模板 | `<app_data_dir>/templates/genres.json` | 启动时优先读取，缺失回退内置 43 体裁 |
| 提示词覆盖 | SQLite（`prompt_overrides` 表） | `PromptRegistry::resolve_prompt()` 优先读 DB 覆盖 |
| 构建产物 | `src-tauri/target/release/bundle/{dmg,deb,msi}/` | `cargo tauri build` 产出 |

## 发布流程（tag 驱动）

```bash
# 1. 确认四源版本号一致（见 sf-change-control）
# 2. 提交并更新 docs of record
git add -A && git commit -m "fix: vX.Y.Z ..."
git push origin master

# 3. 打 tag（禁止覆盖已有 tag）
git tag -a vX.Y.Z -m "vX.Y.Z 简述"
git push origin vX.Y.Z

# 4. 推送后立即查 CI（强制）
gh run list --limit 3
# 监控 rust-check / frontend-check / e2e-check / tauri-build 直到全绿
# 任何失败：gh run view <run-id> --log-failed → 修复 → bump → 重推

# 5. 本地出本平台包
cd src-tauri && cargo tauri build
```

CI 在 tag push 时触发 `tauri-build` 的 stable 分支，发布 GitHub Release（含 `.msi`/`.dmg`/`.deb`）并生成 `latest.json` 供应用内升级器拉取（端点：`https://github.com/91zgaoge/StoryForge/releases/latest/download/latest.json`）。

## 生成链路观测

`creative_workflow.log` 记录 TriShot/LLM/ModelGateway 各阶段。诊断卡片（幕前/幕后设置页）显示：AI 生成模式、当前模型 ID/名/Provider/端点、最后调用模型、最后发给 LLM 的提示词全文、工作流日志路径与最近条目。后端 `log_frontend_event` 命令允许前端写入 WorkflowLogger。

关键诊断事件：`genesis.first_chapter.generated`、`genesis.chapter_switch.sent`、`genesis.final_content`、`smart_execute.start`、`trishot.call3.done`、`trishot.bgp4.spawn/done`、`llm.record_call.spawn`（候选实时探测走标准 `[Gateway]` 日志，不进 `creative_workflow.log`）。

## 何时 NOT 用本技能

- 怎么装环境/构建失败 → `sf-build-and-env`。
- 测试命令 → `sf-validation-and-qa`。
- CI 失败根因 → `sf-debugging-playbook`。
- 发布门禁规则 → `sf-change-control`。

## 出处与维护

- 重验证命令：
  - `grep -n 'devUrl\|frontendDist\|productName\|version' src-tauri/tauri.conf.json`
  - `rg -n 'app_data_dir' src-tauri/src | head`（数据根路径来源）
  - `gh run list --limit 3`（CI 状态）
- 易漂移项：`tauri.conf.json` 窗口/版本、`latest.json` 端点、CI release body。
- 最后核对：2026-07-07，v0.26.23。
