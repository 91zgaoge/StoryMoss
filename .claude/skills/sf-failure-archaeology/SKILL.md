---
name: sf-failure-archaeology
description: StoryMoss 重大调查、死路、被拒修复、revert 的编年史（symptom→root cause→evidence→status），防止重打已定胜负的仗。何时加载：要修一个反复出现的 bug、要判断某修复方向是否已被试过、看到“第一章重复/卡死/死锁/超时/JSON 空/启动崩溃”等历史问题复现、或被问“这问题以前怎么修的”时。
---

# StoryMoss 失败考古

> 目的：每个已定胜负的战役写清 symptom→root cause→evidence→status，让后来者不再重打。**改一个“似曾相识”的 bug 前先查这里。**

## 战役 1：Genesis 新小说第一章内容重复（v0.26.7–.23，9 轮）

| 版本 | 尝试 | 结果 |
| --- | --- | --- |
| v0.26.7 | React #185 回调稳定化 + ChapterSwitch 路径守卫 | 未根治 |
| v0.26.8 | `isTextDuplicate` + `isTextAlreadyInEditor`，覆盖 pipeline-complete/ChapterSwitch/smart_execute 竞态 | 未根治 |
| v0.26.9 | 重复检测基准从 `editorRef.getText()` 改 `latestContentRef.current`（DOM 滞后） | 未根治 |
| v0.26.10 | `latestContentRef` + `editorRef.getText()` 双重基准 + `appendText` 最终防线 | 未根治 |
| v0.26.11 | 追加后立即 `editorRef.getHTML()` 同步 store 与 `latestContentRef`；`appendText` 空文档分支标记外部同步 | 未根治 |
| v0.26.12 | 修 `characters null` 白屏；`useSubscription` 空值防护；E2E | 相关但非根因 |
| v0.26.13 | 渲染层：`shouldShowGhostTree` 改 `!!generatedText`（空幽灵容器残留）+ 自动接受先 `setIsGenerating(false)` | 视觉重复缓解 |
| v0.26.14 | **日志证实数据层只追加一次，重复来自 LLM 正文自身首尾回环**；新增 `trimSelfRepetition`（段落级 + KMP border） | 治标：清 LLM 自重复 |
| v0.26.16 | **结构性根治**：生成侧 8% 闸门 + anti-repeat 指令；前端单写者状态机 `idle→generating→delivered` | **胜** |
| v0.26.18 | **真正的独立根因被发现**：`src-frontend/src/utils/format.ts` 的 `splitChineseSentences` 全局正则 `lastIndex` 在 `exec` 返回 null 时被重置为 0，循环外读取导致 `text.slice(0)`（整段）被当末句再 push → `autoFormatText` 段落分组后内容翻倍（日志：0+1446 → 2933 ≈ 2×1446）。修复：循环内捕获 `lastIndex`，3 个契约单测。Gap A/B/C 单写者加固作为正交防御保留 | **胜（format 层）** |
| v0.26.23 | `auto_contract` 4 label 静默化（解除续写 6 分钟阻塞，根因非重复但同症状域） | 收尾 |

- **symptom**：新写小说第一章正文出现首尾重复 / 整段叠加。
- **root cause（最终，多层）**：(R1) 前端多路径并发写（DB 正文 + 幽灵 + smart_execute final_content）无单写者；(R2) LLM 输出自重复未被生成侧拦截；(R3) `format.ts` `splitChineseSentences` 正则 `lastIndex` 重置导致 `autoFormatText` 内容翻倍（v0.26.18 才发现，前 11 轮所有前端守卫都挡不住这条）。
- **evidence**：`creative_workflow.log` 显示 `append_ai_done` 触发一次、`append_text_check.occurrences=1`；v0.26.18 日志数学矛盾 `0+1446→2933≈2×1446` 证实 format 层翻倍。
- **status**：**已胜**。死路：散布布尔守卫（v0.26.7–.14 共 6+ 轮）必复发——禁止回退。`*_future` 已重命名为 `*_gen`（澄清顺序 await 非 `tokio::join!` 并行）。

## 战役 2：600s 超时（v0.23.17–.19）

- **symptom**：生成请求挂到 600s 超时。
- **root cause**：`record_llm_call` DB 写入阻塞 tokio worker；`pool.get()` 无 `.connection_timeout` 无限阻塞。
- **evidence**：行级诊断 12+ 标记定位到 `db_write`/`emit_completed` 阻塞。
- **status**：**已胜**。`record_llm_call` 改 `spawn_blocking` fire-and-forget；连接池 `.connection_timeout(5s)`。死路：在热路径同步等 DB。

## 战役 3：Mutex 自死锁（v0.23.34 / .42）

- **symptom**：`select_candidates`（Call3）必死锁；BGP-4 卡住。
- **root cause**：`std::sync::Mutex` 不可重入——`health_registry.lock()` 持锁期间 `is_model_available` 再 lock；BGP-4 `spawn_blocking().await` 与 BGP-1/3 竞争同 Mutex。
- **evidence**：15 个诊断标记精确定位自死锁位置。
- **status**：**已胜**。health 锁移入嵌套块作用域；BGP-4 改 `tokio::spawn`。死路：持 `std::sync::Mutex` 期间再 lock 同锁。

## 战役 4：推理模型 JSON 空 `{}`（v0.23.48–.49）

- **symptom**：推理模型生成 `missing field 'title'` / 空 `{}`。
- **root cause**：思考链 `önh...`/`<thinking>` 里的花括号被 `find('{')` 误判为 JSON 对象，提取出空 `{}`。
- **evidence**：`strip_reasoning_blocks` 剥离思考链后正常。
- **status**：**已胜**。`strip_reasoning_blocks` + `extract_first_json_object`（括号匹配）跳过空对象继续扫描。死路：`find('{')`/`rfind('}')` 简单查找。

## 战役 5：Issue #4 启动 panic / Windows 闪退（v0.26.16–.17）

- **symptom**：应用启动 panic；Windows 闪退。
- **root cause**：`init_db` 在不可写 app data 目录失败；`GatewayExecutor::new` 通过 `state::<DbPool>()` 读未 manage 的 pool 在启动时 panic。
- **evidence**：不可写应用目录回归测试复现。
- **status**：**已胜**。`GatewayExecutor::new` 显式接收 `pool`，`setup` 仅在 pool 可用时初始化网关；打包 `src/db/migrations/` 为 Tauri resource；`init_db` 启动前 `create_dir_all`。

## 战役 6：Windows MSI 构建（v0.26.21）

- **symptom**：WiX `light.exe` 标识符生成失败。
- **root cause**：24 个迁移文件名含中文/全角逗号/破折号且最长 102 字符。
- **status**：**已胜**。重命名为 ASCII 短名（保留 `V###` 前缀与排序）。`schema_migrations` 按 version 跟踪，已应用迁移不受影响。死路：`wix.language: zh-CN`（v0.26.20 尝试无效，问题在标识符生成而非代码页；该配置项仍在 `tauri.conf.json` 但已无效，可清理）。

## 战役 7：续写卡死与幽灵文本混乱（v0.26.23，4 根因）

- **symptom**：续写卡死 6 分钟；新旧续写结果竞争；幽灵 10s 渲染延迟。
- **root cause（4 项）**：
  - Bug B（卡死主因）：`auto_contract` 4 label 未静默化，后台补合同阻塞 `isAnyBackendActive` 6 分钟。
  - Bug D（混乱主因）：`handleSmartGeneration` 重入，旧幽灵未丢弃。
  - Bug A：`force-hide-ghost` 类移除未触发重渲染。
  - Bug C：续写 call3 超时 120s，慢模型不 fail-fast。
- **status**：**已胜**。4 label 加入 `is_silent_background`；重入守卫；`bodyForceHideGhost` state 镜像；call3 超时 120s→60s 慢模型回退快模型。

## 战役 8：macOS 启动崩溃（v0.23.6）

- **symptom**：`state() called before manage() for Arc<dyn VectorStore>`。
- **root cause**：Tauri State 注入顺序。
- **status**：**已胜**。`LanceVectorStore` 创建与 `app.manage` 提前到依赖组件之前。

## Reverts（死路标记，避免重走）

| commit | 回滚了什么 | 教训 |
| --- | --- | --- |
| `a9114d2`（v0.23.39） | 回滚激进前端守卫（`selectChapter` 同章跳过、移除 `story_created` selectChapter、`isFirstChapterReady` 时 `setGeneratedText('')`） | **过度守卫打破正常加载路径 → 白屏回归**。保留诊断日志 + 后端 completed/error 修复。这是前端守卫的安全边界——v0.26.7–.14 的散布布尔守卫终将撞到这条 |
| `b444832` | 整个重构提交 | 366 个编译错误——过大的重构落地即碎 |
| `b775c2a` | `Cargo.lock` 回退到 v0.14.4 | `cargo update`（v0.15.0 期间）破坏 SQLite 迁移行为（V25 `scenes.execute_stage` 未应用）→ 28 个 `canonical_state` 测试失败 |

## 历史教训总集

更多条目见 `docs/archive/LESSONS_LEARNED.md` 与 `docs/archive/AGENTS_HISTORY.md`（完整历史版本记录）。

## 何时 NOT 用本技能

- 当前 bug 分诊 → `sf-debugging-playbook`。
- 架构不变量 → `sf-architecture-contract`。
- 要推进的活问题 campaign → `sf-genesis-campaign`。

## 出处与维护

- 重验证命令：
  - `git log --oneline --all | rg 'fix|revert|重复|卡死|死锁|panic|超时' | head -40`
  - `ls docs/archive/`
  - `git tag --sort=-creatordate | head -20`（版本→战役对照）
- 易漂移项：版本号映射、新增战役需追加。
- 最后核对：2026-07-07，v0.26.23。
