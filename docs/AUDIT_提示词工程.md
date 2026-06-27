# StoryForge 提示词工程审计分析报告

> **审计日期**：2026-06-27
> **审计版本**：v0.23.64
> **审计范围**：智能创作全链路的提示词应用——注册表、注入路径、资产消费、质量与协同
> **审计方法**：静态代码追踪 + 路径覆盖分析（注册表 79 条提示词、3 条 Writer 路径、12 类后台资产）

---

## 一、总体判断

项目拥有一套**设计精良、覆盖面广**的提示词体系：79 条注册提示词横跨 21 个分类，配合 31 张桥段卡、21 种剧情引擎、13 种高压关系、雪花法 10 步、11 维质量审计等高质量后台资产。这些资产本身质量很高，体现了深厚的中文网文创作功底。

**但提示词体系的"最后一公里"存在系统性断裂**：高价值提示词和资产在默认创作路径中要么被旁路，要么仅通过脆弱的旁路通道注入，导致"好资产没用在刀刃上"。核心矛盾是——**注册表完整、覆盖端点齐全，但默认续写路径（TimeSliced）和创世路径（TriShot）恰好绕过了注入最充分的 Full 路径**。

---

## 二、三条 Writer 路径的提示词覆盖差异（核心架构事实）

整个系统存在三条并行的 Writer 路径，它们对提示词资产的消费程度天差地别：

| 维度 | TimeSliced（默认续写） | TriShot（创世/部分续写） | Full（build_writer_prompt） |
|------|----------------------|----------------------|---------------------------|
| `writer_system` 7 条准则 | ❌ 不注入 | ❌ 不注入 | ✅ 注入 |
| `orchestrator_timesliced_writer` | ✅（3 行极简模板） | ❌ | ❌ |
| Call 1 合成 prompt | ❌ | ✅（LLM 合成） | ❌ |
| `current_content` 前文 | ✅（build_continuation_context） | ✅（v0.23.64 补） | ✅ |
| 叙事四元组（含桥段卡/引擎/高压关系） | ⚠️ 仅 quartet 命中时 | ⚠️ 仅 quartet 命中时 | ⚠️ 仅 quartet 命中时 |
| StyleDNA 六维 | ✅（bundle） | ✅（bundle） | ✅ |
| 方法论 | ✅（bundle，无 Pro 限） | ✅（bundle，无 Pro 限） | ✅（Pro 守卫） |
| 题材画像 | ✅（bundle） | ✅（bundle） | ✅ |
| LivingAuthorGuard 在世作者保护 | ❌ | ❌ | ✅ |
| AdaptiveGenerator 个性化 | ❌ | ❌ | ✅（Pro 守卫） |
| Inspector 质检 | ❌（单次 LLM，跳过） | ❌（单次 LLM，跳过） | ✅（内联） |
| asset→param 映射（temperature 等） | ❌（硬编码 0.75/2048） | ❌ | ✅ |

**结论**：注入最充分、资产消费最完整的 Full 路径，恰恰是**默认续写不走**的路径。用户日常续写用的是 TimeSliced——它是三条路径中提示词资产最薄的。

---

## 三、P0 严重问题

### P0-1：旗舰 `writer_system` 提示词在默认路径被完全旁路

`writer_system` 是注册表中的**一号提示词**，定义了"什么是好的小说写作"的 7 条核心准则（中文写作、角色声音一致性、展示而非讲述、对话推动情节、场景结尾留钩子、遵循世界观、连贯性）+ 风格控制 + 输出要求。这是整个系统**最重要的写作品质定义提示词**。

**证据**：`writer_system` 仅在 `agents/service.rs:1823`（`build_writer_prompt`，Full 路径）被 `resolve_prompt` 读取。TimeSliced 路径走 `generate_for_task`（`orchestrator.rs:860`），TriShot Call 3 走 `generate_for_task_with_tags_and_timeout`（`orchestrator.rs:1424`），二者最终都进入 `generate_for_request_with_request_id`，此处 `system_prompt` 仅取自 `profile.system_prompt_override`（`service.rs:1232-1243`）——**完全未读取 PromptRegistry 的 `writer_system`**。

**影响**：默认续写场景下，模型收到的 system_prompt 要么是用户手动设置的单模型覆盖，要么是 `None`（适配器默认）。那 7 条定义"好小说"的写作准则完全缺席。模型仅凭 `orchestrator_timesliced_writer` 这个 3 行极简模板（"你是一名专业的小说作者。请根据以下设定写一段正文…"）+ bundle 约束工作——**约束了"写什么"，却没约束"怎么写才算好"**。

### P0-2：声称的三级 system_prompt 优先级链只实现了第一级

`adapter.rs:128-129` 和 `settings.rs:546-547` 的注释都声称：
> 优先级：LlmProfile.system_prompt_override > AppConfig.writer_system_prompt_override > PromptRegistry "writer_system" 默认

但 `service.rs:1232-1243` 的实际代码**只实现了第一级**（`profile.system_prompt_override`）。`AppConfig.writer_system_prompt_override` 和 PromptRegistry `writer_system` 在网关路径中**从未被读取**。

**影响**：用户在后台设置页面看到的"Writer 系统提示词"条目（`writer_system`），即使精心编辑保存了覆盖，也**不会作用于 TimeSliced/TriShot 路径**——只对 Full 路径生效。这是"提示词可配置"承诺的实质性缺口：用户以为改了生效，实际默认路径根本不读它。

### P0-3：桥段卡/剧情引擎/高压关系的内容注入完全依赖 quartet 旁路

31 张桥段卡、21 种剧情引擎、13 种高压关系——每张/每种都精心编写了 `function`（可复用功能）、`when_to_use`、`remix_hint`（重构提示）、`avoid`（反例）、`pairs_well_with` 等字段。这些是极具价值的创作指导。

**但它们的内容注入存在断裂**：
- TriShot Call 1（PromptSynthesizer）给 LLM 看完整 manifest 目录，LLM 返回 `selected_asset_ids`——**但这些 ID 只被转成 `asset_tags` 用于模型路由**（`orchestrator.rs:1395`），**没有任何代码回查 asset payload 把卡片/引擎正文重新注入 Writer prompt**。
- 内容注入的唯一通道是 `infer_narrative_quartet`（`quartet_inference.rs`）的启发式推荐——且**仅在输入被判定为"模糊"（Vague/WithSeed）时**才触发推荐。

**证据**：`render_narrative_quartet_section`（`service.rs:3209`）渲染四元组时，桥段卡/引擎/高压关系的正文来自 quartet 推荐结果（`serialize_quartet_for_prompt`），而非来自 Call 1 的 `selected_asset_ids`。

**影响**：如果 Call 1 选中了"公开打脸"桥段卡，但 quartet 启发式恰好没推荐那张，该卡的 `function`/`when_to_use`/`remix_hint`/`avoid` 指导**永远不会到达 Writer**。LLM 精心选择的资产在传递链上丢失了内容。

---

## 四、P1 重要问题

### P1-1：默认续写路径（TimeSliced）完全跳过质检

TimeSliced 设计为"单次 LLM，跳过 Inspector/Rewrite"以优先速度。因此以下高质量质检提示词在默认续写中**完全不执行**：
- `inspector_system`（8 维质检：连续性/人物/世界观/风格/伏笔/逻辑/对话/描写）
- `inspector_contract_compliance`（合同合规检查）
- `audit_quality_inspector`（11 维质量审计）
- `mini_review_system`（轻量审校）

它们仅在 Full 路径（内联）或后台 AuditExecutor（fire-and-forget）中运行。后台审计的结果**不反馈到当前生成**——用户看不到它影响这次续写。这意味着默认续写**没有实时质量门**。

### P1-2：TriShot 路径丢失 Full 路径的多项资产注入

TriShot Call 3 不经过 `build_writer_prompt`，因此 Full 路径中以下已接入的特性在 TriShot 缺席：
- **LivingAuthorGuard**（`sanitize_style_brief`）：在世作者名保护 + 手工艺滑块——TriShot 不触发
- **AdaptiveGenerator / PromptPersonalizer**：个性化写作偏好——TriShot 不注入
- **asset→param 映射**（`assetParams.rs`）：StyleDNA→temperature、methodology→max_tokens、genre→max_tokens——这套精心设计的参数映射在 TriShot 不消费（硬编码 0.75/2048）

### P1-3：anti_ai 改写器是骨架，且 CONTEXT.md 声称的注入未实现

- `anti_ai/rewriter.rs`：`AntiAiRewriter` 仅有 `should_trigger`，无实际改写 LLM 调用，主流程从不调用
- `CONTEXT.md:63` 声称"anti_ai_flag 注入 `build_review_prompt`"——在 `pipeline/review.rs:192` 中**未实现**
- `ai_cliches` 词表（含 v0.17.1 新增 7 词）虽然存在，但没有接入任何生成/审校路径

**影响**：反"AI 味"是小说质量的关键维度（cliché 词、"让我们""综上所述"等翻译腔），但系统只有检测骨架（手动触发），没有自动改写闭环，更没有在审校阶段注入 anti-AI 检查项。

### P1-4：reader_promise 未直接注入 Writer

`reader_promise.rs` 为 43 个题材映射了 9 种基础情绪（爽/甜/虐/恨/惊/燃/怕/痛/治愈）+ 衍生爽点，是极具价值的"读者期待"资产。但它**从不直接注入 Writer prompt**，仅作为 `infer_narrative_quartet` 派生 `emotional_payoff` 的源头（取第一段），最终只有一行 `emotional_payoff` 进入四元组。完整的读者承诺体系（多爽点、衍生爽点）被压缩成单行后丢失了大量信息。

---

## 五、P2 质量与协同问题

### P2-1：最常用提示词反而是最薄的

`orchestrator_timesliced_writer` 是**默认续写每次都走**的提示词，但它的默认内容只有 3 行：

```
你是一名专业的小说作者。请根据以下设定写一段正文（800-1500字）。
故事上下文：{{context}}
写作指令：{{instruction}}
要求：1. 只输出小说正文 2. 保持与已有内容的自然衔接 3. 符合角色性格和世界观设定
```

对比 `writer_system`（7 条准则 + 风格控制 + 输出要求）和 `narrative_first_chapter_generate`（含创作策略/四元组/写作策略/输出纪律的完整模板），这个承载了日常续写的提示词过于单薄——它把"怎么写好"的责任完全推给了 bundle 上下文，而 bundle 里恰好没有 `writer_system` 的写作准则。

### P2-2：合同约束四重重复

`writer_contract_constraints`、`write_time_bundle_contract`、`review_contract_criteria`、`refine_contract_criteria` 四个提示词携带**完全相同的 6 个合同变量**（core_tone / pacing_strategy / world_rules / chapter_goal / must_cover_nodes / forbidden_zones），只是措辞框架略有不同。同一份合同在 Writer、bundle、审稿、修稿四个位置重复注入，增加了 token 消耗和维护负担（改一处合同语义要同步四处）。

### P2-3：输出纪律重复

`narrative_first_chapter_generate` 模板末尾自带"输出纪律"段（禁止元评论/markdown/小节标题/批注），同时 TriShot Call 3 又追加 `NOVEL_OUTPUT_DISCIPLINE` 常量（`orchestrator.rs:2936`，内容高度重叠）。创世第一章路径会收到**两段近乎相同的输出纪律**。

### P2-4：写作准则跨提示词重复

"展示而非讲述""对话推动情节""保持连贯性"等准则在 `writer_system`、`methodology_snowflake_step9`、`orchestrator_timesliced_writer`、`narrative_first_chapter_generate` 中反复出现，措辞各异。当多处定义时，用户编辑覆盖时不知道哪处是权威来源。

### P2-5：`orchestrator_timesliced_writer` 缺 `continuation` 变量声明

代码（`orchestrator.rs:832`）向模板注入了 `continuation` 变量（前文回顾），但注册表中 `orchestrator_timesliced_writer` 的 `variables` 字段只声明了 `["context", "instruction"]`，未声明 `continuation`。若用户在前端编辑该提示词，看不到 `{{continuation}}` 变量标签，容易误删导致前文回顾丢失。

---

## 六、提示词资产接入总览

下表汇总 12 类后台创作资产的接入状态：

| 资产 | 目录展示 | 内容注入 Writer | 状态 |
|------|---------|---------------|------|
| StyleDNA / blend / classic_styles | ✅ | ✅（双路径，经 bundle） | 完整接入 |
| 方法论（雪花/英雄之旅/场景结构/人物深度/高密度世界构建） | ✅ | ✅（bundle 无限制；Full 路径 Pro 限） | 完整接入 |
| 题材画像 GenreProfile | ✅ | ✅（双路径） | 完整接入 |
| 桥段卡 beat_cards（31 张） | ✅ | ⚠️ 仅 quartet 旁路 | 内容注入断裂 |
| 剧情引擎 story_engines（21 种） | ✅ | ⚠️ 仅 quartet 旁路 | 内容注入断裂 |
| 高压关系 pressure_relationships（13 种） | ✅ | ⚠️ 仅 quartet 旁路 | 内容注入断裂 |
| 读者承诺 reader_promise（43 题材） | ✅ | ❌ 仅作 payoff 源头 | 未直接注入 |
| 在世作者保护 LivingAuthorGuard | — | ⚠️ 仅 Full 路径 | TriShot 缺失 |
| 个性化 AdaptiveGenerator | ✅ | ⚠️ 仅 Full 路径（Pro） | TriShot 缺失 |
| 反 AI 味 AntiAiRewriter | — | ❌ 骨架未接入 | 未实现 |
| 开篇清晰度门 OpeningClarityGate | — | ❌ 仅事后审计 | 不影响生成 |
| 级联改写 CascadeRewriter | — | ❌ 事后级联任务 | 不参与生成 prompt |

---

## 七、改进建议（按优先级）

### 立即修复（P0）

1. **让 `writer_system` 在网关路径生效**：在 `service.rs:1232` 的 `system_prompt` 解析中补全文档承诺的三级优先级——`profile.system_prompt_override` → `AppConfig.writer_system_prompt_override` → PromptRegistry `writer_system`。这样默认续写和创世都能拿到 7 条写作准则，且用户在后台的编辑真正生效。

2. **把 `writer_system` 作为 TimeSliced/TriShot 的 system_prompt 注入**：当前 `generate_for_task` 不传 system_prompt。应在 TimeSliced 和 TriShot Call 3 调用时，从 registry 解析 `writer_system`（或其精简版）作为 system_prompt 传入，而非只靠极简的 `orchestrator_timesliced_writer` 模板。

3. **接通 Call 1 selected_asset_ids → 内容回灌**：Call 1 返回的 `selected_asset_ids` 应触发 asset payload 回查，把选中卡/引擎/关系的 `function`/`when_to_use`/`remix_hint`/`avoid` 注入 Writer prompt，而非只转成路由标签。

### 近期改进（P1）

4. **为 TimeSliced 增加轻量质检门**：可复用 `mini_review_system`（已有，4 维快速评分）作为可选内联质检，分数低于阈值时触发一次 Rewrite，而非完全跳过质检。

5. **TriShot 路径补齐 LivingAuthorGuard 和个性化**：将 `sanitize_style_brief` 和 `PromptPersonalizer` 的注入从 `build_writer_prompt` 提取为可复用函数，在 TriShot final_prompt 组装时也调用。

6. **实现 anti_ai 注入审校**：在 `build_review_prompt` 中注入 anti-AI cliché 检查项（CONTEXT.md 已声称但未实现）；将 `AntiAiRewriter` 从骨架升级为实际改写（至少做 cliché 词的 local replace）。

7. **直接注入 reader_promise**：将题材的完整读者承诺（多爽点 + 衍生爽点）作为独立 section 注入 Writer，而非压缩成单行 emotional_payoff。

### 质量优化（P2）

8. **充实 `orchestrator_timesliced_writer`**：将其从 3 行扩充为包含核心写作准则的实质模板，或在文档中明确"此模板仅做上下文组装，写作准则由 system_prompt（writer_system）承担"。

9. **合并合同约束**：将四处重复的合同注入收敛为单一来源（如 `write_time_bundle_contract`），其他位置引用而非复制。

10. **声明遗漏变量**：为 `orchestrator_timesliced_writer` 补充 `continuation` 变量声明，避免用户编辑时误删。

11. **去重输出纪律**：`narrative_first_chapter_generate` 内嵌的输出纪律与 `NOVEL_OUTPUT_DISCIPLINE` 常量二选一，避免创世路径双份注入。

---

## 八、结论

StoryForge 的提示词**资产端是富矿**（79 条提示词 + 12 类创作资产，质量普遍很高），但**消费端存在系统性"漏接"**：

- 最关键的 `writer_system`（写作品质定义）在默认路径被旁路，且文档承诺的优先级链未实现
- 高价值资产（桥段卡/引擎/高压关系）的内容注入依赖脆弱的 quartet 旁路，Call 1 的选择结果在传递中丢失内容
- 注入最完整的 Full 路径恰好是默认续写不走的路径

这意味着**用户精心配置的提示词和资产，在最常见的"续写"场景下大量未生效**。修复 P0-1 和 P0-2（让 writer_system 在网关路径生效 + 补全优先级链）是投入产出比最高的改进——它让 7 条写作准则和用户的后台编辑立即在所有路径生效。修复 P0-3 则让 Call 1 的资产选择真正传递到 Writer。
