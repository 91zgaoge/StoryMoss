---
name: sf-debugging-playbook
description: StoryMoss 失败模式的症状→分诊表、卡时间的陷阱与区分实验。何时加载：报告了 bug、续写卡死、Genesis 第一章重复、生成无输出、启动崩溃、CI 失败、JSON 解析空对象、超时、白屏、或被问“为什么 X 失败/卡住”时。先建反馈回路再下结论（见 diagnose 技能 Phase 1）。
---

# StoryMoss 调试手册

**铁律**：先建反馈回路再下结论。本项目大多数“卡死/重复/无输出”都是竞态或同步阻塞，肉眼盯代码无效。参考 `diagnose` 技能 Phase 1 构造回路；本项目特有回路见下文。

> **修复必须走门禁**：下表给出的是*历史已验证的修复方向*（用于快速分诊），不是允许跳过门禁。任何新修复在合并前必须走 `sf-change-control`（变更门禁）+ `sf-validation-and-qa`（证据标准，含回归测试）+ 推送门（docs 同步 + CI 必查）。

## 项目特有的反馈回路构造法

1. **`creative_workflow.log` 时间线对照法**（本项目最有效）：读 `<app_data_dir>/logs/creative_workflow.log`，把用户感知的“卡住时刻”对齐到日志的阶段标记（`genesis.first_chapter.generated`/`smart_execute.start`/`trishot.call3.done`/`trishot.bgp4.spawn/done`/`llm.record_call.spawn`）。卡点 = 时间线断点。v0.26.23 续写卡死就是用 `creative_workflow.log` 时间线定位到 `auto_contract` 阻塞 6 分钟。注：候选模型实时探测（5s）走标准 `log::debug!`（`[Gateway] 候选 [N] 实时探测通过/失败/超时`），不进 `creative_workflow.log`——查探测去标准日志 sink。
2. **前端诊断日志**：`[DEBUG-dup]`/`[DEBUG-act]` console.warn；`frontstage:rich_editor_diag`（前 20 帧 + 幽灵状态变化 + 200ms IPC 节流）。
3. **诊断卡片**：幕前/幕后设置页显示当前模型、最后提示词全文、最后调用模型。
4. **纯函数提取 + 单测**：竞态路径提取纯函数（`select_first_chapter_content`/`world_concept_for_character_prompt` 模式）写契约测试。
5. **跨层 golden 双跑**：算法 bug 用 `tests/fixtures/trim_golden.json` Rust+TS 双跑锁定。
6. **E2E**：仅前端行为；`genesis-duplicate.spec.ts` 验证自动接受后幽灵段落隐藏。

## 症状 → 分诊表

| 症状 | 最可能根因 | 区分实验 / 首查 | 优先级 |
| --- | --- | --- | --- |
| 续写卡死，正文已返回但界面不动 | 后台 LLM 调用未静默化，进度事件覆盖主活动 | 查 `is_silent_background` 白名单（`llm/service.rs`）是否含该 `context_label`；读日志看是否后台 label 触发前端 activity | P0 |
| 续写结果新旧竞争 / 幽灵文本混乱 | `handleSmartGeneration` 重入，旧幽灵未丢弃 | 入口重入守卫：存在未接受幽灵时先丢弃并提示（v0.26.23 Bug D） | P0 |
| 幽灵文本 10s 后才消失 | `force-hide-ghost` 类移除未触发重渲染 | `RichTextEditor` 用 `bodyForceHideGhost` state 镜像类（v0.26.23 Bug A） | P1 |
| 新小说第一章内容重复 | 多路径并发叠加：DB 正文 + 幽灵 + smart_execute final_content 竞态；或 LLM 输出本身首尾回环；或 `format.ts` 正则翻倍 | 先看日志确认是数据层重复、LLM 自重复、还是精确 2× 翻倍（`splitChineseSentences` `lastIndex` 重置，v0.26.18）；v0.26.14 日志证实数据层只追加一次，重复来自 LLM 正文自身 | P0 |
| 第一章字数精确翻倍（≈2×） | `src-frontend/src/utils/format.ts` `splitChineseSentences` 全局正则 `lastIndex` 在 exec 返回 null 时重置为 0，循环外读取导致整段被当末句再 push | 看日志 `afterLen ≈ 2×beforeLen`；修复在循环内捕获 `lastIndex`（v0.26.18） | P0 |
| 第一章 LLM 输出首尾段落相同 | 模型自重复 | `trimSelfRepetition`/`trim_self_repetition`（KMP 最长 border，≥30 字且 ≥8% 裁尾）；生成侧 8% 闸门重试 | P0 |
| 生成无输出 / 空 `{}` JSON | 推理模型思考链里花括号被 `find('{')` 误判 | `strip_reasoning_blocks` + `extract_first_json_object` 跳过空对象（v0.23.49） | P1 |
| JSON 解析 trailing characters | `rfind('}')` 在 JSON 后含 `}` 文本时误提取 | 括号匹配精确提取（v0.23.48） | P1 |
| 600s 超时 | `record_llm_call` DB 写入阻塞 tokio worker / `pool.get()` 无限阻塞 | 连接池 `.connection_timeout(5s)`；`record_llm_call` 改 `spawn_blocking` fire-and-forget（v0.23.19） | P0 |
| select_candidates / Call3 必死锁 | `std::sync::Mutex` 不可重入，持锁期间再 lock | health 锁移入嵌套块作用域（v0.23.34）；中毒锁 `unwrap_or_else(\|e\| e.into_inner())` | P0 |
| BGP-4 卡住 / 自死锁 | `spawn_blocking().await` 等 DB 与 BGP-1/3 竞争同 Mutex | 改 `tokio::spawn` fire-and-forget（v0.23.42） | P0 |

> 术语：**BGP** = Background Pipeline（后台管线阶段，BGP-1/2/3/4 为创世后台的 4 个阶段，如 BGP-2 自动改写、BGP-3 后台 Ingest、BGP-4 后台审计+洞察）。
| macOS 启动 panic `state() called before manage()` | VectorStore State 注入顺序 | `LanceVectorStore` 创建与 `app.manage` 提前（v0.23.6） | P0 |
| 启动 panic / Windows 闪退（Issue #4） | `init_db` 不可写目录失败；`GatewayExecutor::new` 读未 manage 的 pool | `setup` 显式传 pool，仅 pool 可用时初始化（v0.26.16）；打包迁移资源（v0.26.17） | P0 |
| 幕前白屏 | `useCharacters` 返回 null 时访问 `.length` | null 防护（v0.26.12）；`ErrorBoundary` 增强诊断 | P1 |
| React #185 无限循环 | `FrontstageApp` pipeline-complete effect 回调不稳定化 | 回调稳定化 + Zustand selector 同步 isGenerating（v0.26.2–.7） | P0 |
| Windows MSI 构建挂掉 | 迁移文件名含中文/全角/破折号/过长 | 重命名 ASCII 短名（v0.26.21） | P0 |
| macOS 公证失败 | 协议过期 / 证书密码尾随空格 | 续签；Secret 必须 28 字符无尾随 | P0（外部） |
| CI `cargo +nightly fmt -- --check` 跨平台不一致 | `format_strings` macOS/Win nightly 不同 | 不要启用 `format_strings` | P1 |
| 本地模型并发崩溃页面空白 | IngestPipeline 并发 LLM 未静默 | 三个 label 加入 `is_silent_background`（v0.23.45） | P0 |

## 卡时间的陷阱（每个都真实踩过）

- **盯代码找竞态**：第一章重复 saga 从 v0.26.7 到 v0.26.16 反复 9 次，每次都“看起来修了”。根因是单一基准（`editorRef.getText()`）滞后 DOM + 多写者并发。教训：竞态必须有单写者状态机 + 双重基准，散布布尔守卫必复发。
- **不读日志就猜**：v0.26.14 用户感知“第一章重复”，不读日志会以为是前端追加了两次；日志证实数据层只追加一次，重复来自 LLM 正文自身首尾回环。
- **把后台当同步**：后台 LLM/DB 调用同步化是本项目卡死的最常见根因（600s、BGP-4、Ingest、auto_contract）。
- **改 fmt 配置**：动 `rustfmt.toml` 的 `format_strings` 会引入跨平台 CI 差异。

## 何时 NOT 用本技能

- 通用调试纪律（建回路→复现→假设→仪器化→修+回归）→ `diagnose` 技能。
- 已修复战役的完整编年史 → `sf-failure-archaeology`。
- 架构不变量 → `sf-architecture-contract`。
- 诊断工具具体用法 → `sf-diagnostics-and-tooling`。

## 出处与维护

- 重验证命令：
  - `rg -n 'is_silent_background|BACKGROUND_LLM_SEMAPHORE|connection_timeout' src-tauri/src/llm`
  - `rg -n 'spawn_blocking|tokio::spawn' src-tauri/src/llm src-tauri/src/task_system | head`
  - `ls tests/fixtures/`（golden 在仓库根 tests/，非 src-tauri/）
  - `git log --oneline --all | rg 'fix|revert|重复|卡死|死锁|panic' | head -30`
- 易漂移项：白名单 label、超时常量、State 注入顺序、rustfmt 配置。
- 最后核对：2026-07-07，v0.26.23。
