# 提示词工程修复计划

> 基于 `docs/AUDIT_提示词工程.md` 的审计发现，制定本修复计划。
> 版本基线：v0.23.64
> 制定日期：2026-06-27

---

## 一、修复原则

本计划严格遵循以下三条原则（来自项目方约束）：

### 原则 1：结合模型网关的探测与调度功能
所有修复**不新建旁路**，而是让提示词内容搭乘网关已有的候选链 + 5s 预探测 + 3D 打分 + 活跃模型置顶 + 连续失败降级机制。网关已具备的能力（探测死模型、按 `asset_tags` 把创作任务归类为 `HeavyCreation`、按 TTFB 选最快模型）直接复用，不重复造轮子。

### 原则 2：不任意增加模型调用频次
- 关键路径（Call 1 → Call 3）的 LLM 调用次数**不增加**。
- 任何"提示词编写/合成"复用**已有的 Call 1**（最快模型，`generate_with_fastest`，`synthesizer.rs:52`）在单次调用内产出最完整的提示词。
- 资产正文回灌采用**确定性代码查询**（0 次额外 LLM 调用），而非新增 LLM 调用。
- 质检/改写仅在已有调用上做条件触发，不默认追加。

### 原则 3：用最快的工具模型一次性生成最完整的提示词
- Call 1 路由合成器已是 `generate_with_fastest`（`select_fastest_profile` + 5s 预探测 + fallback）。修复增强其**输入完整度**与**输出完整度**，使其单次调用即产出包含选中资产完整指导的合成提示词，而非多次调用拼装。
- 确定性补全步骤（资产正文回灌）发生在 Call 1 之后、Call 3 之前，不引入新 LLM 调用。

---

## 二、P0 修复（严重，立即实施）

### P0-1：让 `writer_system` 在所有创作路径作为 system_prompt 生效

**问题**：旗舰提示词 `writer_system`（7 条写作品质准则）仅在 `build_writer_prompt`（Full 路径，`service.rs:1823`）被 `resolve_prompt` 读取；默认续写（TimeSliced，`orchestrator.rs:860`）和创世（TriShot Call 3，`orchestrator.rs:1424`）走 `generate_for_task*`，其 system_prompt 只取自 `profile.system_prompt_override`（`service.rs:1232-1243`），**完全不读 registry 的 `writer_system`**。

**根因**：`generate_for_task*` 系列签名（`service.rs:556/577/616/640`）无 system_prompt 参数；`GatewayRequest`（`types.rs:83-134`）无 system_prompt 字段；底层 `GenerateRequest.system_prompt`（`adapter.rs:131`，OpenAI/Anthropic 已消费）无法从上层接到 registry 渲染结果。

**修复方案**：打通一条机械透传链，让请求级 system_prompt（= registry `writer_system` 渲染产物）能流到 `execute_generation`。

**改动清单（自底向上）**：

1. **`model_gateway/types.rs:83` `GatewayRequest`** + 字段：
   ```rust
   /// 请求级 system_prompt（来自 PromptRegistry 渲染），优先级低于
   /// LlmProfile.system_prompt_override，高于适配器默认。
   #[serde(skip_serializing_if = "Option::is_none", default)]
   pub system_prompt: Option<String>,
   ```

2. **`llm/service.rs:467` `generate_for_request_with_request_id`** + 参数 `system_prompt: Option<String>`，在构造 `GatewayRequest`（`:498-521`）时填入。

3. **`model_gateway/executor.rs:597` `GatewayExecutor::generate`**：把 `request.system_prompt` 透传给候选循环内的 `generate_with_profile_and_request_id_with_format`（`:779`）。

4. **`llm/service.rs:1883` `generate_with_profile_and_request_id_with_format`** + 参数 `system_prompt: Option<String>`，透传给 `execute_generation`。

5. **`llm/service.rs:1164` `execute_generation`** + 参数 `system_prompt: Option<String>`。在 `:1232-1243` 改为**三级优先级解析**（与 `adapter.rs:128-129` / `settings.rs:546-547` 注释一致）：
   ```rust
   let system_prompt = profile.system_prompt_override
       .filter(|s| !s.trim().is_empty())
       .or_else(|| {
           // 2) AppConfig.writer_system_prompt_override（全局配置）
           load_app_config_writer_system_override()  // 新增辅助：读 AppConfig
       })
       .or(system_prompt);  // 3) 请求级（registry writer_system 渲染产物）
   ```
   优先级：**每模型 > AppConfig 全局 > registry writer_system**（最具体的胜出，registry 作为基线兜底）。

6. **`generate_for_task*` 系列**（`service.rs:556/577/616/640`）+ 参数 `system_prompt: Option<String>`，向下透传到 `generate_for_request_with_request_id`。

7. **调用方注入**：
   - **TimeSliced**（`orchestrator.rs:860`）：调用 `generate_for_task` 前，从 registry 解析 `writer_system` 并渲染（复用 `build_writer_prompt` 中 `:1823-1824` 的渲染逻辑，提取为可复用函数 `render_writer_system_prompt`），作为 system_prompt 传入。
   - **TriShot Call 3**（`orchestrator.rs:1424`）：同上，传 `writer_system` 渲染产物。
   - **Call 1 合成器**（`synthesizer.rs:52`，JSON 选资产任务）：**不传** `writer_system`（那是创作准则，非合成任务所需）。

8. **Ollama 适配器缺口**（`ollama.rs:86-105`）：`OllamaRequest` 无 system 字段 → Call 3 若走 Ollama 模型，system_prompt 被丢弃。修复：`/api/chat` 原生支持 `system` 字段；若当前用 `/api/generate`，将 system_prompt 前置拼接到 prompt 头部（带分隔标记），保证 Ollama 路径也能拿到写作准则。

**网关结合点**：system_prompt 搭乘 `GatewayExecutor::generate` 候选链——5s 预探测（`executor.rs:702-770`）确保选中模型存活后才发起带 system_prompt 的真实调用；活跃模型置顶（`+1000`，`executor.rs:393-437`）保证用户当前模型优先承载写作准则。

**验证**：
- 单测：`execute_generation` 三级优先级解析（profile set → profile；profile None + AppConfig set → AppConfig；皆 None → registry writer_system）。
- 集成：TimeSliced 续写时，用诊断命令 `get_last_llm_prompt` 确认 system_prompt 非空且含"展示而非讲述"等准则。
- 回归：`cargo test --lib` 全绿；用户在后台编辑 `writer_system` 覆盖后，续写立即生效。

---

### P0-2：补全 system_prompt 优先级链中的 AppConfig 环节

**问题**：`adapter.rs:128-129` 与 `settings.rs:546-547` 注释声称三级优先级，但 `service.rs:1232-1243` 只实现了第一级（`profile.system_prompt_override`）。`AppConfig.writer_system_prompt_override` 在网关路径从未被读取。

**修复方案**：在 P0-1 第 5 步的三级解析中，新增 `load_app_config_writer_system_override()` 辅助函数，从 `AppConfig` 读取 `writer_system_prompt_override`。该字段已存在于 `AppConfig`（`config/commands.rs:267`）和前端设置（`config/settings.rs:318`），只是网关路径没消费。

**改动**：
- 新增 `fn load_app_config_writer_system_override(&self) -> Option<String>`（`llm/service.rs`），读 `AppConfig::load`，返回非空覆盖。
- `execute_generation` 三级解析中插入此环节。

**验证**：前端设置 `writer_system_prompt_override` 后，TimeSliced/TriShot 续写生效（此前无效）。未设置时回退 registry `writer_system`。

---

### P0-3：接通 Call 1 选中资产的正文回灌（0 额外调用）

**问题**：Call 1（`PromptSynthesizer::synthesize`）让最快模型从 `AssetManifest` 选资产并返回 `selected_asset_ids`，但这些 ID 仅被转成 `asset_tags` 用于模型路由（`orchestrator.rs:1395`），**没有任何代码回查资产 payload 把桥段卡/剧情引擎/高压关系的 `function`/`when_to_use`/`remix_hint`/`avoid` 注入 Call 3 prompt**。内容注入唯一通道是 `infer_narrative_quartet`（`quartet_inference.rs`），且仅在输入判定为"模糊"时触发——清晰输入或未命中推荐时，选中资产的指导永远不到达 Writer。

**修复方案**：在 Call 1 返回后、Call 3 发起前，插入**确定性资产正文回灌**（纯代码查询，0 次 LLM 调用），把选中资产的完整指导附加到 `final_prompt`。

**改动清单**：

1. **新增 `fn attach_selected_asset_guidance(selected_ids: &[String]) -> String`**（`creative_engine/prompt_synthesis/synthesizer.rs` 或新模块）：
   - 遍历 `selected_asset_ids`，按 ID 前缀/注册表分类查询对应资产 payload：
     - `beat_card:*` → 查 `BeatCardRegistry`（`beat_cards/registry.rs`），取 `name`/`function`/`when_to_use`/`remix_hint`/`avoid`。
     - `story_engine:*` → 查 `story_engines/mod.rs`，取 `name`/`payoff`/`best_payoff`/`avoid`/`pairs_well_with`。
     - `pressure_relationship:*` → 查 `pressure_relationships/mod.rs`，取 `name`/`pressure_source`/`works_with`。
     - `methodology:*` → 查对应方法论 prompt id（已在 registry）。
   - 渲染为 `【已选创作资产·完整指导】` section，每项含完整字段，避免 LLM 自行省略。
   - 确定性：无 LLM，纯内存查表。

2. **`orchestrator.rs` Call 3 前置**（`:1361` 附近，`final_prompt` 组装后、`NOVEL_OUTPUT_DISCIPLINE` 追加前）：
   ```rust
   let asset_guidance = attach_selected_asset_guidance(&synthesis.selected_asset_ids);
   if !asset_guidance.is_empty() {
       final_prompt.push_str(&asset_guidance);
   }
   ```
   即便 Call 1 的 `synthesized_prompt` 已含部分资产描述，确定性回灌保证硬约束资产（红线/角色）之外选中的创作资产指导**必达** Call 3。

3. **Call 1 合成器 prompt 强化**（`trishot_synthesizer`，`registry.rs:2316`）：在【输出格式】说明中明确——`synthesized_prompt` 应聚焦于"如何写"的整体约束，资产的具体指导由系统自动附加，LLM 无需在 synthesized_prompt 中重复资产正文（避免 token 冗余）。这样 Call 1 单次调用产出最完整的"创作约束"提示词，资产正文由确定性步骤补全。

**网关结合点**：
- `selected_asset_ids` → `asset_tags`（`orchestrator.rs:1395`）→ `adjust_by_asset_tags`（`dispatcher.rs:56-68`）把 `beat_card`/`story_engine`/`pressure_relationship` 标签归类为 `HeavyCreation` → Call 3 走能力权重 0.8 的强模型路由（`executor.rs:344`）。即"选了创作资产"自动触发强模型，资产正文又确保到达 Writer——调度与内容协同。
- 候选链 5s 预探测保证承载 Call 3 的强模型存活。

**验证**：
- 单测：`attach_selected_asset_guidance` 给定 `["beat_card:公开打脸"]` 返回含 `function`/`remix_hint`/`avoid` 的文本。
- 集成：Call 1 选了桥段卡后，`get_last_llm_prompt` 显示 Call 3 prompt 含该卡完整指导。
- 回归：Call 1 fallback（`is_fallback`）时 `selected_asset_ids` 为空，回灌为空字符串，零影响。

---

## 三、P1 修复（重要，近期实施）

### P1-1：为 TimeSliced 增加可选轻量质检门（复用现有调用，不默认追加）

**问题**：TimeSliced 设计为单次 LLM 跳过 Inspector，默认续写无实时质量门。

**修复方案**：复用已有 `mini_review_system`（`registry.rs:1866`，4 维快速评分），仅在生成内容可疑（如长度远低于预期、或 StyleDNA 句长偏差 >50%）时**条件触发**一次轻量审校，不默认追加。

**改动**：
- `execute_time_sliced`（`orchestrator.rs:680`）Writer 返回后，已有 StyleDNA 句长偏差检测（`:884-894`）。扩展：当偏差 >50% 或正文 <400 字时，用 `generate_with_fastest`（最快模型，静默标签）调一次 `mini_review_system`，分数 <0.5 则用 `pipeline_refine` 触发一次 Rewrite。
- 严格条件触发：正常生成（偏差 ≤50% 且长度达标）**0 次额外调用**；仅异常时追加最多 2 次（审校+改写），且走最快模型。
- 用户可通过 `AppConfig` 开关 `timesliced_quality_gate`（默认关闭，保证默认路径 0 追加）。

**网关结合点**：审校/改写走 `generate_with_fastest` → `select_fastest_profile`（TTFB 最优）+ 5s 预探测，避免死模型拖慢异常恢复。

**验证**：默认配置下续写 LLM 调用数不变（仍 1 次）；开启开关且异常时最多 +2 次最快模型调用。

---

### P1-2：TriShot 路径补齐 LivingAuthorGuard 与个性化注入

**问题**：TriShot Call 3 不经 `build_writer_prompt`，导致 `sanitize_style_brief`（在世作者保护，`style/living_author_guard.rs:127`，Full 路径 `service.rs:2352`）和 `PromptPersonalizer`（个性化，`adaptive/personalizer.rs:14`，Full 路径 `service.rs:2156`，Pro 守卫）在 TriShot 缺席。

**修复方案**：将这两个注入逻辑从 `build_writer_prompt` 提取为可复用函数，在 TriShot `final_prompt` 组装时调用。

**改动**：
- 提取 `pub fn sanitize_style_brief(text: &str) -> String`（已是 `pub(crate)`，`living_author_guard.rs:127`）——直接在 `orchestrator.rs` Call 3 `final_prompt` 上调用。
- 提取 `pub async fn build_personalizer_extension(...) -> Option<String>`，在 TriShot 组装 `final_prompt` 时（非 Pro 也注入——消除 Full 路径的 Pro 守卫不一致，让个性化对所有用户生效）。
- asset→param 映射（`adaptive/asset_params.rs`，StyleDNA→temperature 等）：在 TriShot Call 3 调用时，从 bundle 的 StyleDNA 推导 temperature/max_tokens，替代硬编码 `0.75/2048`。

**验证**：TriShot 续写在世作者名被替换 + 手工艺滑块注入；个性化偏好进 prompt；temperature 随 StyleDNA 变化。

---

### P1-3：实现 anti_ai 注入审校（接通 CONTEXT.md 已声称的设计）

**问题**：`CONTEXT.md:63` 声称"anti_ai_flag 注入 `build_review_prompt`"，但 `pipeline/review.rs:192` 未实现；`AntiAiRewriter`（`anti_ai/rewriter.rs:69`）是骨架，无实际改写。

**修复方案**：
1. **审校注入**：在 `build_review_prompt`（`pipeline/review.rs:192`）追加 anti-AI 检查维度——把 `ai_cliches` 词表（`anti_ai/mod.rs:56`，含 v0.17.1 新增 7 词）作为"AI 味检测"维度注入，要求审校标注命中词。
2. **轻量改写闭环**：`AntiAiRewriter::rewrite` 实现 LocalReplace 策略（命中 cliché 词 → 替换为中性表达，纯规则 0 LLM）；ParagraphRewrite/ChapterRewrite 仍留骨架（需 LLM，后续版本）。
3. 接入点：TimeSliced 质量门（P1-1）触发改写时，先跑 AntiAiRewriter LocalReplace，再走 `pipeline_refine`。

**网关结合点**：无新增 LLM（LocalReplace 纯规则）；`pipeline_refine` 复用已有调用。

**验证**：生成文本含"综上所述"等词时，审校标注 + LocalReplace 替换。

---

### P1-4：直接注入 reader_promise 完整读者承诺

**问题**：`reader_promise.rs`（43 题材 × 9 基础情绪 + 衍生爽点）仅作为 `infer_narrative_quartet` 派生 `emotional_payoff` 的源头（取第一段），完整承诺被压缩成单行丢失。

**修复方案**：在 `WriteTimeBundle::to_prompt`（`write_time_bundle.rs:463`）新增 `【读者承诺】` section，从 `genre_profiles.reader_promise` 读取完整文本直接注入（与 `genre_profile_strategy` 同源加载，`load_sync` 已具备数据）。

**改动**：
- `WriteTimeBundle` + 字段 `reader_promise: Option<String>`，`load_sync`（`:230` 附近加载 genre_profile 时）一并填充。
- `to_prompt` 在 `【体裁画像策略】` 后追加 `【读者承诺】` section。

**验证**：续写 prompt 含完整读者承诺（多爽点），而非单行 emotional_payoff。

---

## 四、P2 修复（质量优化，择机实施）

### P2-1：充实 `orchestrator_timesliced_writer` 或明确职责边界

**问题**：默认续写每次走的提示词仅 3 行（`registry.rs:2287`），把"怎么写好"推给 bundle，而 bundle 无 `writer_system` 准则。

**修复方案**：P0-1 修复后，写作准则由 system_prompt（`writer_system`）承载，`orchestrator_timesliced_writer` 职责明确为"上下文组装 + 指令"。更新其 `description` 注明此分工，避免用户误以为该模板应包含写作准则。可选：在模板末尾追加一句"遵循系统提示词中的写作准则"作为锚点。

**改动**：`registry.rs:2284` description 更新 + 默认内容微调。

---

### P2-2：收敛合同约束为单一来源

**问题**：`writer_contract_constraints`/`write_time_bundle_contract`/`review_contract_criteria`/`refine_contract_criteria`（`registry.rs:2696/2763/2794/2833`）携带相同 6 变量，四重重复。

**修复方案**：以 `write_time_bundle_contract` 为单一权威来源（bundle 已注入 Writer）；`review_contract_criteria`/`refine_contract_criteria` 改为引用其变量渲染结果，而非各自重述合同文本。`writer_contract_constraints`（Full 路径用）保留但内容与 bundle 版对齐。

**验证**：改合同语义只需改一处。

---

### P2-3：去重输出纪律

**问题**：`narrative_first_chapter_generate`（`registry.rs:1605`）模板内嵌"输出纪律"段，TriShot Call 3 又追加 `NOVEL_OUTPUT_DISCIPLINE`（`orchestrator.rs:2936`），创世第一章双份注入。

**修复方案**：从 `narrative_first_chapter_generate` 模板移除内嵌输出纪律段（改由 Call 3 统一追加 `NOVEL_OUTPUT_DISCIPLINE`）；或将 `NOVEL_OUTPUT_DISCIPLINE` 提取为 registry 条目供复用。推荐前者。

---

### P2-4：声明遗漏变量 + 跨提示词去重

**问题**：
- `orchestrator_timesliced_writer`（`registry.rs:2287`）的 `variables` 缺 `continuation`（代码 `orchestrator.rs:832` 实际注入）。
- "展示而非讲述""对话推动情节"等准则在 `writer_system`/`methodology_snowflake_step9`/`orchestrator_timesliced_writer`/`narrative_first_chapter_generate` 重复措辞各异。

**修复方案**：
- `orchestrator_timesliced_writer.variables` 补 `"continuation"`。
- P0-1 后 `writer_system` 成为写作准则唯一权威源，其他提示词中的重复准则改为"（遵循系统提示词写作准则）"引用，避免多处定义。

---

## 五、实施顺序与验证矩阵

### 阶段 1：P0-1 + P0-2（system_prompt 透传链）— 最高 ROI
让 `writer_system` 7 条准则和用户后台编辑在所有路径生效。

| 步骤 | 文件 | 改动 |
|------|------|------|
| 1 | `model_gateway/types.rs:83` | `GatewayRequest` + `system_prompt` 字段 |
| 2 | `llm/service.rs:467` | `generate_for_request_with_request_id` + 参数 |
| 3 | `model_gateway/executor.rs:597,779` | 透传 `request.system_prompt` |
| 4 | `llm/service.rs:1883` | `generate_with_profile_and_request_id_with_format` + 参数 |
| 5 | `llm/service.rs:1164,1232` | `execute_generation` + 参数 + 三级解析 + AppConfig 辅助 |
| 6 | `llm/service.rs:556/577/616/640` | `generate_for_task*` + 参数透传 |
| 7 | `llm/ollama.rs:86` | Ollama system 字段/prompt 前置 |
| 8 | `agents/service.rs:1823` | 提取 `render_writer_system_prompt` 复用函数 |
| 9 | `agents/orchestrator.rs:860,1424` | TimeSliced/Call3 注入 writer_system |

**验证**：`cargo test --lib` 全绿；诊断命令确认 system_prompt 非空；后台编辑生效。

### 阶段 2：P0-3（资产正文回灌）
让 Call 1 选中的桥段卡/引擎/高压关系指导必达 Call 3。

| 步骤 | 文件 | 改动 |
|------|------|------|
| 1 | `creative_engine/prompt_synthesis/` 新增 `asset_guidance.rs` | `attach_selected_asset_guidance` |
| 2 | `agents/orchestrator.rs:1361` | Call 3 前置回灌 |
| 3 | `prompts/registry.rs:2316` | `trishot_synthesizer` 说明更新 |

**验证**：Call 3 prompt 含选中资产完整字段；fallback 时零影响。

### 阶段 3：P1 修复
P1-1（可选质检门）→ P1-2（TriShot 补齐 Guard/个性化）→ P1-3（anti_ai）→ P1-4（reader_promise）。

### 阶段 4：P2 质量优化
P2-1 ~ P2-4。

### 全程验证基线
- `cargo check` 零错误
- `cargo test --lib` ≥ 578 passed / 0 failed（零回归）
- `npx tsc --noEmit` 零错误
- 真实模型续写：`writer_system` 准则生效 + Call 3 含选中资产指导 + 0 额外调用（对比修复前后 LLM 调用计数）

---

## 六、风险与回退

| 风险 | 缓解 |
|------|------|
| system_prompt 透传链改动面广（9 处） | 机械透传无逻辑分支；逐层加参数默认 `None`，旧调用零影响；先 `cargo check` 再测 |
| Ollama system 处理方式变更 | `/api/chat` 原生支持；`/api/generate` 前置拼接带分隔标记；保留适配器默认回退 |
| 资产正文回灌增加 Call 3 token | 仅回灌 Call 1 **选中**的少量资产；硬约束已在 bundle，回灌聚焦创作资产指导；单卡 ~150 字，可接受 |
| P1-1 质检门误触发拖慢 | 默认关闭（`timesliced_quality_gate=false`）；触发条件严格（偏差>50% 或 <400 字）；走最快模型 |
| Full 路径 Pro 守卫被 P1-2 移除 | 个性化对所有用户生效是正向（消除不一致）；LivingAuthorGuard 本就应全路径生效 |

**回退**：每阶段独立提交，任一阶段 `cargo test` 回归即 revert 该阶段，不影响其他。

---

## 七、预期成效

修复后，**默认续写场景**（用户最常用路径）将发生质变：
1. 模型收到 `writer_system` 7 条写作准则（system_prompt）——"展示而非讲述""对话推动情节""场景留钩子"等真正生效。
2. 用户在后台编辑的提示词覆盖（`writer_system` / `writer_system_prompt_override`）真正作用于所有路径。
3. Call 1 选中的桥段卡/剧情引擎/高压关系的完整创作指导必达 Call 3——31 张桥段卡、21 种引擎、13 种关系的价值不再被"目录展示后丢失"。
4. 关键路径 LLM 调用次数不变（Call 1 + Call 3），网关探测/调度/降级机制全程护航。

**核心收益**：项目"80 多个高质量提示词没有得到很好应用"的根因——**注册完整但消费断裂**——被系统性修复，让精心编写的提示词资产真正决定生成质量。
