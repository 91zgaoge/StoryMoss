# TriShot 三击生成管线 — 设计文档

> 版本：v0.23.0 设计稿
> 日期：2026-06-21
> 状态：已批准，实施中

## 一、背景与动机

### 问题

当前 StoryForge 智能创作主流程存在两个核心痛点：

1. **资产注入是「笨拼接」**：`WriteTimeBundle::to_prompt()`（`creative_engine/write_time_bundle.rs:638-813`）把 ~17 个创作资产段落 `sections.join("\n\n")` 无差别堆砌，不区分与当前指令的相关性，不解决段落间冲突，不精炼。所有资产一股脑塞进 Writer，prompt 膨胀 → 本地慢模型生成慢。

2. **Full 模式是「多连接黑洞」**：`execute_full`（`agents/orchestrator.rs:926-1326`）最多 1 Writer + 2×(1 Inspector + 1 Rewrite) = **5 次串行 LLM**，本地 Qwen 累计 250-335s **必然超时**（v0.14.3 changelog 已确认）。

### 用户思路

> 每次收到用户生成内容的指令后，只最多连接 3 次 LLM 就生成内容：
> - 第 1 次：连接最快的工具 LLM 调度，识别意图后从所有创作资产中快速选择并合成综合提示词
> - 第 2 次（可选）：把合成提示词提交给大语言模型调试完善
> - 第 3 次：连接大语言模型根据最终提示词生成内容
> - 其他任务作为后台 agent 静默执行

### 可行性结论：可行 ✅

后台基础设施（`tokio::spawn` + `silent_background` 白名单 + `TaskExecutor` trait + `SyncEvent` 回流 + 前端 `mainGenerationCompletedRef` 守卫）已完整支撑「先返回内容、后台静默干活」。

## 二、关键约束

| 约束 | 现状 | 对 TriShot 的影响 |
|---|---|---|
| `smart_execute` 总超时 | 180s（`commands/orchestrator.rs:49`） | 2-3 次串行 LLM 必须在此内 |
| `PlanExecutor` 单步超时 | 90s（`planner/executor.rs:413`） | TriShot 必须绕过/放宽此路径 |
| 单次 LLM 超时 | 120s 生成 + 60s 首字节（`llm/adapter.rs:41`） | 每次 LLM 需独立紧超时 |
| 前端超时 | 200s（`FrontstageApp.tsx:2041`） | 200s > 180s 后端，安全 |
| 「最快模型」API | 不存在（仅有 `TaskClass::LightTool` 60% 速度权重） | 需新增最快模型选取能力 |

### 延迟预算

| 路径 | Call1(快) | Call2(可选) | Call3(Writer) | 合计 | 180s 内? |
|---|---|---|---|---|---|
| 简单续写·远程 | 5-15s | — | 20-50s | 25-65s | ✅ |
| 简单续写·本地 | 15-40s | — | 50-110s | 65-150s | ✅ |
| 复杂指令·远程 | 5-15s | 10-25s | 20-50s | 35-90s | ✅ |
| 复杂指令·本地 | 15-40s | 25-50s | 50-110s | 90-200s | ⚠️ 预算守卫跳过 Call2 |

## 三、目标架构

### 命名

新增 `GenerationMode::TriShot`（三击），与 `Fast`/`TimeSliced`/`Full` 并存，`AppConfig.generation_mode = "tri_shot"` 启用。新模式并存，配置可切换，零破坏性。

### 关键路径（用户可见，最多 3 次 LLM）

```
smart_execute (180s 伞)
  └─ smart_execute_inner
       ├─ [bootstrap?] → GenesisPipeline（不变，提前返回）
       └─ [TriShot 快速路径] ← 新增分支，绕过 PlanGenerator
            ├─ Phase 0: 加载上下文（DB, spawn_blocking）────────── 0 LLM
            ├─ Phase 1 / Call 1: 路由合成器（最快模型）───────── 1 LLM [fast]
            │    识别意图 + 选相关资产 + 合成连贯提示词 + 判定 needs_refinement
            ├─ Phase 2 / Call 2: 精修器（可选，仅 needs_refinement）─ 0~1 LLM
            ├─ Phase 3 / Call 3: Writer 生成 ───────────────── 1 LLM [writer]
            └─ Phase 4: 后台 agent（静默）
                 BGP-1 质检 / BGP-2 自动改写 / BGP-3 入库 / BGP-4 洞察
```

**总计：2 次（简单）或 3 次（复杂）LLM。**

### 后台 agent 产物（分严重度策略）

| 后台 agent | 触发 | 产物 | 前端表现 |
|---|---|---|---|
| BGP-1 质检 | 每次 TriShot 生成后 | 11 维审计批注 | 写入 DB |
| BGP-2 自动改写 | HIGH 严重度（逻辑/连续性/设定，priority≥3） | 自动改写 + 替换正文 + 修订历史 | `ContentAutoRevised` → toast「已修正 N 处，可撤销」 |
| BGP-2 建议 | LOW 严重度（风格/节奏/余韵） | 修订建议 + 差异 | `RevisionSuggested` → 审阅面板 |
| BGP-3 入库 | 每次 TriShot 生成后 | 实体/关系/事件 + KG + 向量 | 写入 DB（静默） |
| BGP-4 洞察 | 每 N 段（默认 5） | 趋势摘要 | 写入 story_summaries |

## 四、分阶段实施

### Phase 0 — 基础设施（无行为变更）

- `GenerationMode::TriShot` 枚举 + dispatch
- `AppConfig.generation_mode` 支持 `"tri_shot"`
- `silent_background` 白名单 +4 标签
- 最快模型选取：`select_fastest_profile()` + `generate_with_fastest()`
- `SyncEvent` 新增 `ContentAutoRevised` / `RevisionSuggested`

### Phase 1 — 资产清单 + Call 1 路由合成器

- 新模块 `creative_engine/prompt_synthesis/`（`manifest.rs` / `synthesizer.rs`）
- `AssetManifest::build()` 把 WriteTimeBundle + 资产目录打包成紧凑清单（4000 字符预算）
- `PromptSynthesizer::synthesize()` 调最快模型选资产+合成提示词，失败回退 `bundle.to_prompt()`
- 注册表新增 `trishot_synthesizer` / `trishot_refiner` 模板

### Phase 2 — Call 2 精修器（可选）

- `PromptRefiner::refine()` 调试完善提示词
- 预算守卫：剩余预算 < Writer 估算 + 20s 时跳过 Call2

### Phase 3 — `execute_trishot` 编排 + 快速路径

- `AgentOrchestrator::execute_trishot()` 串起完整管线
- `smart_execute_inner` TriShot 快速路径分支（跳过 PlanGenerator）
- `PlanStep::long_running` 跳过 90s 步超时，受 180s 伞保护

### Phase 4 — 后台 agent 体系

- BGP-1 质检（复用 AuditExecutor）
- BGP-2 自动改写器（新 `auto_rewrite_executor.rs`，分严重度）
- BGP-3 入库（复用 IngestPipeline，补 smart_execute 缺口）
- BGP-4 洞察（复用 InsightExecutor）
- 前端监听 `content-auto-revised` / `revision-suggested`

### Phase 5 — 真实模型全流程验证 + 文档

- 6 场景真实模型测试
- `#[ignore]` 集成测试
- 文档更新（CHANGELOG/README/AGENTS/PROJECT_STATUS/ROADMAP/ARCHITECTURE/TESTING/USER_GUIDE）

## 五、回退策略

- **配置级回退**：`generation_mode = "auto"` 恢复现状
- **运行级回退**：Call 1 失败 → `synthesized_prompt = bundle.to_prompt()` → 等价 TimeSliced
- **后台级回退**：任一后台 agent 失败仅 `log::warn`，不影响已返回内容

## 六、风险与缓解

| 风险 | 缓解 |
|---|---|
| 本地慢模型 3 次超 180s | 预算守卫跳过 Call2；每 Call 独立紧超时；配置可关 |
| Call 1 快模型太弱 | confidence<阈值回退本地拼接；真实模型 A/B |
| 后台改写改掉满意内容 | 仅 HIGH 自动改；修订历史可撤销；阈值可配置 |
| 丢失多步计划能力 | 仅写作类指令走 TriShot；结构性指令回退 PlanExecutor |
| 后台并发干扰主流程 | 全 `silent_background` 标签；`mainGenerationCompletedRef` 守卫 |
