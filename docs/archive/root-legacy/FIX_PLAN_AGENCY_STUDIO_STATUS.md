# 代理工作室（Agency Studio）状态显示故障 — 审计与修复方案

日期：2026-08-02 · 版本：v0.30.48（本机安装版与仓库 HEAD 一致）

## 一、审计范围与结论摘要

对代理工作室功能做了全链路审计：前端页面 → IPC 接口 → Rust 后端 → 数据库 → 本机运行日志。

**结论：后端接口、数据库、事件通道均正常；故障在前端页面自身的数据重建逻辑与错误处理缺失。**

- 后端三个查询命令（`agency_list_runs` / `agency_get_run` / `agency_list_board`）均已注册且工作正常（`src-tauri/src/agency/commands.rs:199-242`，注册于 `src-tauri/src/handlers.rs:232-240`）。
- 三类实时事件（`agency-agent-activity` / `agency-run-progress` / `agency-board-changed`）发射正常；创世后台质检路径（`spawn_editor_qc`，`coordinator.rs:2362-2450`）也会发射 `editor_auditor` 的 start/done 活动事件。
- 数据库 `agency_runs` 表有 11+ 条历史 run，最新为 2026-07-29（failed / review）。
- 字段名前后端完全一致（snake_case），无序列化不匹配。

## 二、本机日志分析

日志位置：`~/Library/Application Support/com.storymoss.app/logs/`

- 当前会话（08-02 07:53 启动至今）：`storymoss.2026-08-01` 全 50 行，**无任何 agency 相关记录、无任何前端 `api:tauri` 报错**——说明本会话 IPC 调用没有失败，页面"不显示"不是后端报错导致。
- 历史 run 失败记录（`agency_runs.error_message`）反映的是运营层问题，非本故障原因：
  - 7-29 run：`process exited`（应用中途退出，重启后兜底置 failed，`lib.rs:744`）；
  - 7-20 ~ 7-23 多条：Agent 熔断（模型未按 JSON action 格式输出 / 达到最大轮数）；
  - 7-30 起 `creative_workflow.log` 集中出现本地模型服务不可达：`http://10.62.239.13:17098` 连接失败 / 60s 超时（Qwen3.5-27B、Gemma-4-31B）。
- 附带发现（非本次修复范围）：每次启动有一条 WARN——`[IntentionGraph] 资产同步失败: FOREIGN KEY constraint failed`（`lib.rs:914`），建议另行立项排查。

## 三、根因（为什么三个代理的状态显示不出来）

页面：`src-frontend/src/pages/AgencyStudio.tsx`，三张角色卡 = 主创(lead_writer) / 管理(producer) / 编辑审计(editor_auditor)。

**根因 1（直接原因）：角色卡"最近动作"只消费页面会话内的实时事件，不做历史重建。**
`lastAction()`（`AgencyStudio.tsx:143-146`）只在 `activities` state 中查找，而 `activities` 仅由页面打开后收到的 `agency-agent-activity` 事件填充（`:85-88`）。页面后开时，虽然 v0.30.40 已修复 run 水合（`runs[0]` → `activeRunId`），时间线也已能从 board items 重建（`:164-175`），**但三张角色卡没有接入这同一个历史数据源**，因此恒显示 "-"。用户看到的就是"三个代理都没有运作状态信息"。

**根因 2（放大因素）：页面无任何 loading / error UI，IPC 失败被静默吞掉。**
三个 `useQuery` 只取 `data`（`:107,122,128`）；`loggedInvoke` 失败只在控制台/后端日志记录（`core.ts:40-43`），react-query 重试耗尽后静默失败。用户无法区分"没有数据"和"出错了"，且三张卡永远停在 "-"，表现为"功能坏了"。

**根因 3（次要）：状态文案未本地化。**
卡片上的 `run 状态` 直接拼接英文原值（如 `review · failed`，`:137-142`），已有的 `runStatusLabel()`（`:60-71`）只用于时间线，没用在卡片上。

## 四、修复方案（全部限前端单文件 + 测试）

### 改动 1：角色卡接入历史重建（修根因 1）
文件：`src-frontend/src/pages/AgencyStudio.tsx`

- 从 `board`（黑板条目含 `producer` / `zone` / `key` / `summary` / `created_at`）按角色推导"最近动作"，与现有时间线历史重建（`:164-175`）同一数据源；
- 优先级：实时事件 > 历史重建 > "-"；
- 历史动作文案示例：`创建 草稿：首章 - <summary>`；若无 board 但有 run，则显示 run 级状态（如"运行已失败：review"）。

### 改动 2：补 loading / error 态（修根因 2）
文件：同上

- 三个 `useQuery` 读出 `isLoading` / `isError` / `error`；
- 加载中显示"加载中…"；任一查询失败时在页面顶部显示错误条（含错误信息），角色卡显示"状态获取失败"而非 "-"。

### 改动 3：卡片状态文案本地化（修根因 3）
- `run 状态` 行使用 `runStatusLabel()` 转换（failed→失败 等），phase 保留原值。

### 改动 4：测试
文件：`src-frontend/src/pages/__tests__/AgencyStudio.test.tsx`

- 新增用例：页面后开（无实时事件）时，角色卡能从 board items 重建显示各角色最近动作；
- 新增用例：`listRuns` 失败时页面显示错误提示而非静默 "-"；
- 确保现有 3 个用例（空态、水合、时间线重建）不回归。

### 不改动的部分
- 后端 Rust 代码：审计确认正常，零改动；
- 事件协议、数据库 schema：零改动。

## 五、验证步骤

1. `cd src-frontend && npx vitest run src/pages/__tests__/AgencyStudio.test.tsx` 全绿；
2. `npx tsc --noEmit`（或项目既有 lint/typecheck 脚本）通过；
3. 手动验证（ dev 实例连接本机真实 DB）：打开代理工作室 → 未启动新 run 时三张卡即显示历史最近动作与本地化 run 状态；断开后端模拟 IPC 失败 → 页面显示错误条。

## 六、风险与回滚

- 改动集中在一个页面组件与其测试，不影响其他页面与后端；回滚 = `git checkout` 两个文件。

## 七、实施与验证记录（2026-08-02，已批准并实施）

已实施，改动文件：

- `src-frontend/src/pages/AgencyStudio.tsx`
  - `lastAction()` 增加历史重建回退：实时事件 → 黑板条目（按 `producer` 精确匹配三角色，取 `created_at` 最新）→ 查询失败提示 → "-"（DB 实测 `producer` 仅有 lead_writer/producer/editor_auditor 三个值，精确匹配可全覆盖）；
  - 三个 `useQuery` 改捕获完整结果，新增页面顶部红色错误条（含错误信息，提示 10s 自动重试）；`run 状态` 在出错时显示"状态获取失败"；
  - `runStatusLabel()` 补 `pending→等待` / `running→运行中`，卡片与时间线统一使用本地化文案；
  - run 选择器加载中显示"加载中…"。
- `src-frontend/src/pages/__tests__/AgencyStudio.test.tsx`
  - 新增 2 个用例：①页面后开无实时事件时三角色卡从 board items 重建各自最近动作 + 本地化 run 状态；②`listRuns` 失败时显示错误条（含错误消息）而非静默空态。

验证结果：

- `npx vitest run src/pages/__tests__/AgencyStudio.test.tsx` — 6/6 通过（含 4 个原有用例无回归）；
- `AgencyEval` / `AgencyLearning` 相邻测试 2/2 通过；
- `npx tsc --noEmit` — 通过，零类型错误。

待用户手动确认：重启/重装应用后打开代理工作室，未启动新 run 时三张角色卡应直接显示历史最近动作（如"创建 资产：世界观"）与本地化 run 状态（如"review · 失败"）。
