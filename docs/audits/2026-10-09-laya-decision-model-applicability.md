# Laya 决策模型在 StoryMoss 中的可用性分析

> 日期：2026-10-09
> 缘起：用户提供 https://huggingface.co/convaiinnovations/laya，问「这个模型在本项目中有无可用之处」。
> 方法：laya 侧全部结论取自**模型卡原文逐行阅读**（`convaiinnovations/laya` README 384 行、`laya-multilingual` 卡、ONNX/GGUF 产物清单）+ GitHub API + PyPI 元数据；StoryMoss 侧由两个并行代理走查代码库，本文引用的每一处 StoryMoss 结论均带 `file:line` 抽查复核。
> 核验声明：本机到 `huggingface.co` 直连超时（DNS 解析到 31.13.71.19，非 HF 地址），模型卡经 `hf-mirror.com` 镜像获取；镜像文本与 GitHub 仓库 README、PyPI 描述三方互证一致，非单一来源转述。

---

## 〇、结论（先读这段）

**当前不接入。laya 与 StoryMoss 约九成的 AI 面（生成正文/章纲/资产）零交集，剩下可对口的判定面里，唯一有说服力的候选位（意图分类）需要自建标注与训练流水线才能达到可用精度，投入产出比不足以支撑今天动手；建议把它登记为 ROADMAP「自研小模型部署」项下的候选技术，并先做一个零依赖的前置实验来证伪。**

三条支撑判断：

1. **前提需要纠正**：laya **不是生成式 LLM**，是 322–421M 参数的非自回归「类型化决策」分类模型，**不产出任何文本**。它无法写、改、审、判任何一段正文——而这正是 StoryMoss 的 AI 价值所在（`LlmAdapter::generate` 的 `prompt → content` 契约与它从根本上对不上）。
2. **它自认零样本近乎随机**：在自家 headline 基准 typed-decisions 上，未微调的 checkpoint 只有 0.362 准确率，**低于多数类基线 0.461**；宣传的 0.766 属于在基准训练集上微调出来的产物。官方原话：「Laya is a fast base to specialise, not a zero-shot decision engine.」对 StoryMoss 而言，采用 laya 不等于「接入一个模型」，而等于**开启一条本地分类器训练流水线**（采标注 → 训练 → 校准 → 导出部署），是一项持续能力建设。
3. **与项目两条既有方针存在张力**：一是「LLM 是意图判断的唯一权威，不回到硬编码关键词匹配」（`PROJECT_STATUS.md:648`，v0.30.23 明确写下的设计原则）；二是 v0.65.0 确立的质检纪律「每一条 finding 必须能引用原文、确定性可复现」。神经分类器既不是关键词匹配（不是回退），也是黑箱（不可引用）。任何引入都需要先重新论证这两条。

**但它不是「无关」**：laya 代表的「System 1 便宜本地判定 / System 2 昂贵远端推理」分层，正是 StoryMoss `model_gateway`（TaskClass → 候选链 → 成本哨兵）已在局部实现的模式。laya 是这一模式的现成封装。若有一天**离线可用**或**隐私强制本地**成为硬需求，它的定位会从「可有可无」直接变成「正解候选」。

---

## 一、laya 是什么：一次必要的前提纠正

### 1.1 它不是 LLM

网络上有将其误传为「推理大模型」的说法（名称也与一篇图像分类论文 LAYA: Layer-wise Attention Aggregation 撞名）。以模型卡原文为准：

> **Multilingual, non-autoregressive System 1 decision model.** Give it a **state** (text, email, ticket, or JSON) and **typed questions**; it returns typed answers with mathematically calibrated probabilities in a single forward pass (~33 ms) across 100+ languages. … **It never generates text**, so there is nothing to parse and nothing to hallucinate.

| 维度 | 事实（模型卡原文） |
|---|---|
| 形态 | 编码器 + 从零训练的决策头；非自回归，单次前向，**无解码、无生成** |
| 主干 | English：ModernBERT-large 全量微调（395M）+ head，共 **421M**；Multilingual：mmBERT-base 22 层 256k 词表，**322M** |
| 接口 | `state`（文本/JSON）+ `questions`（类型化问题）→ `answers`（类型化答案 + 概率） |
| 三种原语 | `choice`（从选项里选一）/ `score`（有序量表期望值）/ `noul`（yes/no 概率） |
| 上下文预算 | English：512（`head_max_len=192`，**state 只剩 ~320 token**）；Multilingual：1024 默认（256 给选项 → **state ~768 token**），encoder 支持到 8192（需 `max_len=8192`） |
| 延迟 | T4 GPU 单问 32.8ms / 10 问批 72.3ms；**CPU（preload）193–464ms**；4,000 token 输入在 Apple GPU ≈1.7s |
| 训练 | **RLCD**（对严格 proper scoring rule 做 REINFORCE/GRPO 式强化），不是 SFT instruct；无指令跟随训练 |
| 许可 | **Apache-2.0**（可商用） |
| 发布 | 2026-09-18 首发，至 2026-10-09 仅三周：GitHub 31,782★ / HF 36,328 下载 / 5,383 likes |

### 1.2 接口形态（原文示例）

```python
questions = {
    "department": {"type": "choice", "instructions": "Which department should handle this?",
                   "criteria": {"billing": "invoices, payments, refunds",
                                "technical": "bugs, outages, system errors"}},
    "urgency":    {"type": "score",  "instructions": "How urgent is this?",
                   "criteria": ["not urgent", "soon", "blocking"]},
    "churn_risk": {"type": "noul",   "instructions": "Does the user threaten to cancel?"},
}
result = router.predict(state, questions)
result["answers"]["department"]["choice"]    # 'billing'
result["answers"]["churn_risk"]["noul"]      # 0.8727  ← P(true)，非布尔值
```

两个对集成有实际意义的性质：

- **选项空间在请求时定义**（「The answer space is defined at request time, so new schemas need no retraining」）——意味着换一套标签不需要重训；但**准确率仍需微调**，见 1.3 第一条。
- **没有 system prompt、没有 prompt 工程**：`instructions` / `criteria` 都是结构化数据字段，不存在「提示词被正文污染」的通道。这对 StoryMoss 这种被 CoT 泄漏、提示词污染反复教育过的项目，是有吸引力的属性。

### 1.3 官方「Honest Limits」逐条（这一节几乎每条都直接决定适用性）

| 官方自陈的限制 | 对 StoryMoss 的含义 |
|---|---|
| **零样本近乎随机**：typed-decisions 上 base 0.362 / 多语 0.342，随机 0.318，多数类 0.461。「Laya is a fast base to specialise, not a zero-shot decision engine.」 | 必须微调才可用 → 需要自采标注数据与训练流程。**这是本文结论的核心依据** |
| **高基数选项退化**：选项共用 `head_max_len` 预算，77 选项只分到 3–4 token/项（Banking77 仅 0.425） | 标签集必须小（≤20 项）或做两级 coarse-to-fine |
| **`score` 是最弱原语**（SST-5 0.372） | 「给这段文字打个分」类需求不要指望它 |
| **`noul` 会跟着选项标签走**而不是跟着输入（#156）：对明确正面输入返回自信的「否」 | 二分类要改为中性 key 的 `choice` 规避 |
| **`action.act_probability` 无可用信号**（#185）：几乎恒为 1.0，AUROC 0.30；官方建议改用 `confidence`（同批数据 AUROC 0.77） | 「要不要升级给人确认」这类判定**不能**用它的 act head；只能用 confidence 做级联门控 |
| **自带过度自信**：按 (题型, 选项数) 重拟合 temperature 后 ECE 0.466→0.081（多语 0.314→0.106），「Do this on your own data before trusting the probabilities」 | 想用它的概率做置信度门控，**校准这一步必须自己做**，官方数值不能直接信 |
| **多语卡：低资源语言弱**（Swahili 0.210 / Tamil 0.250 / Amharic 0.110）；English root 遇非拉丁文字直接崩（Khmer 0.000 准确率却带 0.952 置信度） | 「带置信度所以能安全门控」在跨语种时不成立，要靠 Router 的脚本检测兜底（root 仅英文） |
| 上下文：英文 512 很小；8192 时模型卡自陈「约 4,000 token 以内 20 题对 16–18 题，再长 8–17 题，波动大，请在自己数据上验证」 | **章节级判定（数千字正文）在结构上出局**，除非分块+聚合，而聚合层会把语义优势吃掉大半 |

### 1.4 家族与部署产物（已核验）

| 产物 | 内容 | 说明 |
|---|---|---|
| `laya` (root) | ModernBERT-large，421M，512 ctx | 仅英文 |
| `laya-multilingual` | mmBERT-base，322M，1024（可 8192） | 语言表含 `zh`；是中文场景唯一选择 |
| `laya-typed-decisions` | 在基准训练集上微调的示范 checkpoint | 展示微调增益（0.362→0.766），非通用模型 |
| `pip install laya` | Python ≥3.10；extras：`[serve]` `[mcp]` `[onnx]` `[langchain]` `[fast]` | 官方主路径 |
| `laya-serve` | HTTP 服务，暴露 **Jev 协议 `POST /v1/systemone`**，默认 0.0.0.0 无鉴权 | **不是** OpenAI 兼容接口 |
| `onnx-community/laya-multilingual-ONNX` | `model_fp16.onnx_data` **643.8 MB** + tokenizer.json 34.4 MB；fp32 版 1287.6 MB | 唯一可进程内嵌入（Rust `ort`）的现成产物；**未见官方 int8 量化版** |
| `ggml-org/Laya-GGUF` | BF16 844 MB / Q8_0 449.4 MB | StoryMoss 无 llama.cpp 运行时，仅供备查 |
| 官方微调 notebook | Kaggle 免费 2×T4 跑完整闭环（造数据→训练→温度校准→评估→推 Hub） | 训练门槛确实低，成本在数据不在算力 |

### 1.5 热度 ≠ 成熟度

发布三周 31.8k★/36k 下载是极强的传播信号，但同期 **93 个 open issue**、PyPI 三周 39 个版本、且存在第三方复现研究对其校准叙事提出不同结论。这是一个**高速迭代中的新项目**，任何生产依赖都要按「上游可能变」来设计。本文不把社区评价作为论据，只标注事实。

> **未核验线索（不作为论据）**：检索中发现有第三方对其中文盘面的负面诊断（HF discussions 的中文命令用例、社区中文基准）。本文未逐条亲自复现，故不采信；但结合 1.3 表格（root 仅英文、低资源语言弱、零样本近随机）可以确定：**中文能力无论如何都需要自训验证，不能零样本假设可用。**

---

## 二、StoryMoss 的对接面：先划清哪些地方根本用不上

StoryMoss 当前的 AI 架构（走查结论）：

- 所有生成走 Rust 侧 `LlmService` → `LlmAdapter`（OpenAI 兼容 / Anthropic / Ollama 三协议），`GenerateRequest{prompt, system_prompt, max_tokens, …, response_format}` → `GenerateResponse{content, …}`（`src-tauri/src/llm/adapter.rs:249-286`）。
- **无本地推理运行时**：无 llama.cpp / ONNX / ort / candle 依赖，无 sidecar；本地模型一律走 HTTP（Ollama / vLLM / MLX 等外部服务）。
- 既有 embedding 是**零权重的** hash bag-of-bigrams 384 维（`embeddings/embedding.rs:198-254`）或远程 OpenAI 兼容 embedding。
- `LlmProfile` 已有 `api_base` / `capabilities` / `quality_tier` 等完备元数据，接入「任何 OpenAI 兼容端点」今天零代码改动即可完成。

### 2.1 生成面 vs 判定面

StoryMoss 的 AI 调用可分两类，laya 的适用性完全不同：

| | 生成面（≈九成调用） | 判定面（少数调用） |
|---|---|---|
| 内容 | 正文续写、章纲、世界观、角色、润色、审查意见、摘要 | 意图分类、任务分级、门控通过与否、lint 命中 |
| 代表代码 | `agency/coordinator.rs`、`prompts/assembly.rs`、`agents/orchestrator.rs` | `intent.rs`、`model_gateway/dispatcher.rs`、`agency/gate.rs`、`story_system/prose_lint.rs` |
| laya 可用性 | **完全不可用**（不生成文本） | 结构上对口，但多数位点已被更合适的东西占据 |

### 2.2 判定点清单（带坐标，供第三节逐条评估）

| # | 判定点 | 位置 | 现状实现 |
|---|---|---|---|
| 1 | 写作意图分类（本期核心候选） | `intent.rs:448-522` | **一次 LLM 调用**（256 tok / temp 0 / **8s 超时** `:473`）+ LRU 缓存 + 关键词兜底 |
| 2 | 任务分级（模型路由） | `model_gateway/dispatcher.rs:34-136` | 关键词表（verb→class `:75-92`）+ token 尺寸升级 `:127-136` |
| 3 | 质检门控（过/退回改） | `agency/gate.rs:10-33`、`coordinator.rs:6222-6531` | 0.2 确定性代码 + 0.3 规则 + 0.5 LLM editor，阈值 0.75，一轮修订 |
| 4 | 文本质检 lint | `story_system/prose_lint.rs:142-279`、`anti_ai/mod.rs:64-146`、`human_voice.rs` | 纯确定性字符串规则/加权启发式，**强制可引用原文** |
| 5 | 世界观条目重要性（是否待作者确认） | `memory/ingest.rs:542-555`、`pending_review.rs:18`（阈值 ≥7） | 由 ingest 的 LLM 顺带产出，**无额外调用成本** |
| 6 | 内容审核 / 敏感词 | 无（仅 `living_author_guard.rs:23-69` 活人作家黑名单 43 条） | 基本空白 |
| 7 | 隐私/本地路由 | `router.rs:275-279`（`LocalOnly`） | 仅用户手动配置，无内容敏感性检测 |
| 8 | 前端写作建议 | `frontstage/ai-perception/suggestionEngine.ts:31-345` | 17 条可读模板 + 条件打分，**面向用户的解释性逻辑** |

---

## 三、逐位可行性评估

### A. 意图分类（`intent.rs::classify_writing_intent`）—— 唯一值得认真讨论的位点

**为什么它最对口**：

- 同为「给定一段用户输入 → 输出一个小标签集」：现有输出是 `is_new_novel / is_continuation / task_type(5类) / is_prose_request / input_clarity(3类) / detected_genre / confidence`（`intent.rs:119-146`），正是 laya `choice` 原语的形状。
- **输入短**：一句中文指令通常 <100 字，远在 state 预算（多语 ~768 token）之内——这是 laya 少数不超预算的场景。
- **痛点真实且被项目自己记录**：v0.30.11 起因「全代码库用朴素子串匹配做意图路由，误路由频发」而引入 LLM 分类器，一次修掉 6 处高危路由点（`PROJECT_STATUS.md:736-739`）；v0.30.38 又因分类 JSON 缺字段被 serde 默认值毒化缓存，一路放到 `style_enhancer` 元评论覆盖正文（`ROADMAP.md:586-588`）。这条链路的每一次故障都直接伤及用户可见产物。
- **延迟可量化**：每次用户指令前串行一次 LLM 调用，最坏 8s（`:473`）后落保守兜底。

**laya 能带来什么**：本地、离线、零 token、~33ms（GPU）/193–464ms（CPU）、**返回概率** → 可以搭「高置信本地直答 + 低置信回退 LLM」的级联，把绝大多数请求从「一次 LLM 往返」降为「一次本地前向」。

**差距与风险（为什么不是现在就做）**：

1. **零样本不可用**（1.3 首条）：必须先用 StoryMoss 自己的数据微调。数据从哪来是有利条件——`classify_writing_intent` 的历史结果本身就是**银标签**（LLM 分类结果 + 用户实际行为可反推对错），配合人工抽检数千条即可构成训练集。但这仍是一条需要立项的流水线。
2. **中文必须自证**：root 仅英文，多语 checkpoint 是唯一选择；其语言表含 `zh`，但官方未给任何中文基准，且自陈零样本近随机。中文精度只能在自采数据上验证，不能引用官方数字。
3. **概率要重新校准才能做门控**：官方明说自带过度自信、需要按域重拟合 temperature。级联阈值本身是一项校准工程，不是配置项。
4. **与项目方针的张力**：`PROJECT_STATUS.md:648` 写下「LLM 是意图判断的唯一权威，不回到硬编码关键词匹配」。laya 不是关键词匹配，不违反字面；但它把「唯一权威」改成「分级权威」，属于**设计原则变更**，需要正面论证而不是技术顺手。
5. **它最独特的卖点在这里用不上**：「选项空间请求时定义、换 schema 不重训」——而 StoryMoss 的标签集是**静态的固定 5+3 类**。既然标签固定，一个普通的多头分类器（甚至线性头）就够了，laya 的灵活性溢价收不到。
6. **置信度与延迟的折扣**：桌面端绝大多数用户没有 CUDA；官方 CPU 数字是 **193–464ms**（preload 后），与「33ms」给人的印象差距很大，也未必胜过一次轻量 LLM 调用。Apple Silicon 单问延迟官方**未给数字**，必须自测。

**结论：结构最对口，但当前是一个「有门槛的验证课题」，不是可立即兑现的收益。**验证路径见第五节。

### B. 任务分级 / 模型路由（`dispatcher.rs`）—— 低价值

- 现状是关键词表 + token 尺寸升级，分类到 LightTool / BalancedWork / HeavyCreation 三类。
- **主要信息是静态已知的**：全库 `TaskType::` 字面量出现 288 次（非测试文件 284 次，实测计数），无一处从文本推断；role 表（`agency/roles.rs:20-51`）同样直接硬编码。真正需要「从文本推断」的空间很小。
- **误判后果轻**：路由有候选链、健康权重、回退（`model_gateway/executor.rs`），且 `upgrade_by_size`（`:127-136`）已捕获最强的信号（token 数）。
- 为这个位点引入一个新运行时+训练流水线，收益与成本完全不成比例。**结论：不做。**

### C. 质检门控与文本 lint —— 不推荐，且理由不止一条

1. **预算不够**：state 只有 ~768 token（多语），章节正文数千字。要做得先分块再聚合，聚合层的判定精度会吃掉分类器的优势。
2. **判「revise」但说不出 why**：门控的修订动作**依赖** `blocking_issues` 文本（`agency/gate.rs:36-49`、`EditorVerdict:455-467`）去驱动下一轮改写。分类器只给出一个分数/标签，没有可执行的理由——它能把判定做快，但不能把修订做对。
3. **与项目自己的纪律冲突**：v0.65.0 起，lint 的每条 finding 必须能引用原文、且方向存疑的规则一律不入规则（`AGENTS.md` v0.65.0 段）。神经分类器是黑箱、不可引用、跨版本漂移。用它替换确定性 lint 是**设计倒退**。
4. 「预筛跳过昂贵编辑轮」的旁路虽然省 token，但一个假「通过」就是一次**静默降质**，与项目「不静默降级 / 不静默丢弃」原则（v0.65.3 幽灵续文处理）冲突。

**结论：不做。**（`anti_ai`/`human_voice` 作为 advisory 第二意见的设想也因第 3 条不成立。）

### D. 内容审核 / 安全（第 6 位点，当前空白）—— 概念成立，但这是产品决策

StoryMoss 目前几乎没有审核层（只有活人作家黑名单）。laya 自带 guard/moderation 问题模板，且本地跑、输入不外发。但：官方零样本近随机、模板是英文中心、中文写作场景的政策边界需要自训，**而且要不要做审核是个产品/合规决策**，不是技术可行性问题。**结论：观察项，无落地理由。**

### E. 隐私强制本地路由（第 7 位点）—— 唯一「只有本地模型才能做」的正当理由

`RoutingConstraint::LocalOnly` 已存在但只有手动配置入口；没有「这段内容敏感 → 判定与处理必须留在本机」的检测器。这里 laya 的逻辑地位是独特的：**判定输入本身不能出机器，所以判定器必须本地**。但需求侧无任何证据支持，且 laya 无此能力内置（要自训）。**结论：概念成立，需求不存在；记为未来隐私分级路由的技术选项。**

> 顺带说明一个「看似对口实则无收益」的位点：**世界观条目重要性打分**（第 5 位点）。laya 的 `score` 原语（有序量表期望值）与「这条设定重要度 0–4，是否 ≥7 需作者确认」形状完全吻合。但该字段目前由 ingest 的 LLM 顺带产出，**不产生额外调用成本**；且官方自陈 `score` 是最弱原语。用一个新的训练流水线去替换一个免费的顺带产出，收益为零。

### F. 明确不要做的四件事（反模式清单）

1. **不要试图把它接成 `LlmProvider` / `LlmProfile`**。它不是 chat 接口：没有 `/chat/completions`、没有 system prompt、不产出 `content`。塞进 `LlmAdapter` 会在类型层面就撞墙（`GenerateRequest.prompt → GenerateResponse.content`，`llm/adapter.rs:249-286`），`ResponseFormat` 也只有 `json_object` 一种。
2. **不要把 `laya-serve` 当作面向用户的分发方案**。它要求目标机器有 Python 环境；协议是 Jev 的 `/v1/systemone`，**不是** OpenAI 兼容——StoryMoss 现有的 `api_base` 探测链（`{base}/v1/models` → `{base}/models` → `/chat/completions`，`config/commands.rs:1368-1674`）打不到它，等于要为一个可选功能新增一套协议适配 + 用户侧 Python 依赖。
3. **不要用它当「写作质量裁判」**。它判不了文学质量（无对应评测、无生成解释能力、`score` 是最弱原语），更替代不了 LLM editor 的 `blocking_issues`。
4. **不要指望零样本直接上线**。官方自己把这条写进了限制清单第一条。

---

## 四、若要落地：真实成本清单

| 成本项 | 具体量 | 备注 |
|---|---|---|
| **标注数据** | 数千条（意图分类，5+3 类标签） | 有现成起步路径：历史 LLM 分类结果作银标签 + 人工抽检。这是本项目相对多数团队的优势 |
| **训练** | 官方 Kaggle 2×T4 notebook，免费额度可跑完 | 算力不是门槛，**数据与持续维护**是 |
| **校准** | 需按域重拟合 temperature（官方指明必须自己做） | 级联门控阈值的前置 |
| **模型体积** | ONNX fp16 **643.8 MB** + tokenizer **34.4 MB**（fp32 版 1.29 GB）；**未见官方 int8** | 相当于把整个本地模型塞进安装包/自动更新通道；随包分发需单独评估 |
| **运行时** | Rust 需新增 `ort`（ONNX Runtime）依赖与 tokenizer 组件 | 项目当前**零** ML 运行时依赖（连 embedding 都是 hash 向量），这是新增的一整类依赖 |
| **延迟** | GPU 32.8ms；**CPU 193–464ms**；Apple Silicon **官方无数字，需自测** | 「本地更快」在桌面端并不自动成立 |

### 三条部署路径横评

| 路径 | 可行性 | 结论 |
|---|---|---|
| `laya-serve` HTTP（Jev 协议） | 需用户装 Python + 新协议适配 | ✗ 不可作为产品分发路径（内部实验可用） |
| **ONNX + Rust `ort` 进程内** | 唯一可发布路径；现成 fp16 产物可用 | △ 唯一正解，但代价是 678 MB 级体积 + 新运行时依赖 |
| `laya-ts` / WASM（webview 侧） | 决策逻辑在 Rust，搬去前端会割裂架构 | ✗ 不建议 |

---

## 五、推荐的行动顺序（含决策门）

而不是「接 or 不接」的二选一，这里给一个**可证伪、零新依赖**的推进顺序——先验证命题，再验证模型：

**步骤 0（零成本，先做这个）：不引入 laya，验证「本地分类器能否可靠替代 LLM 意图判定」这个命题本身。**

- 用项目**已有**的 384 维 hash bag-of-bigrams embedding（`embeddings/embedding.rs:198-254`，零权重、零新依赖）+ 一个逻辑回归 / 线性头，在历史 `classify_writing_intent` 日志（银标签）上训练并留出评估。
- 这一步的价值：把「数据是否够用、任务是否可分、置信度是否分离」三个问题一次问清。**如果线性头都做不到，换任何编码器都救不了——说明问题出在数据或任务定义，而不在模型。**
- 成本：天级；零依赖；不改动生产路径。

**步骤 1（仅当步骤 0 通过）：用 laya 的编码器在同一数据集上对比。**

- Kaggle 免费 notebook 跑微调，同一留出集对比；同时实测 **Apple Silicon（MPS/CPU）单问延迟**与内存占用——这是官方数字里缺失、而 StoryMoss 用户最相关的一项。
- 顺带验证中文：训练集本身即中文，等于免费完成「中文需自证」这一条。

**步骤 2（仅当步骤 1 三项全赢）：才谈接入。**

- 接入形态：ONNX + `ort`，进程内，作为**置信度门控的快速路径 + LLM 兜底**，绝不作为唯一权威。

**建议的决策门（通过标准，满足才进入下一步）**：

| 指标 | 建议阈值 |
|---|---|
| 留出集准确率 | ≥ 现有 LLM 分类器的准确率（同集同口径） |
| 高置信区间精度 | ≥ 0.98（该区间可直接直答、不回退 LLM） |
| 高置信覆盖率 | ≥ 60%（否则省不下调用，等于白加） |
| Apple Silicon 单问延迟 | < 100 ms（否则不如等 LLM） |
| 体积 | 与安装包/更新通道体积预算不冲突（需与发布策略一起定） |

**什么情况会让结论反转（重开条件）**：① 离线可用或隐私强制本地成为硬需求（此时「本地」本身是需求，不再是优化项）；② 意图分类的 LLM 往返被实测证明确为可感知瓶颈；③ 已积累数千条标注数据（训练成本被摊薄）；④ Apple Silicon 延迟实测过关且体积可接受。

---

## 六、结论与 ROADMAP 落点

- **对「laya 在本项目中有无可用之处」的直答**：作为**生成模型**——不可用，它不生成文本；作为**本地判定模型**——理念对口、候选位真实存在（意图分类），但当前收益未证明、成本实在、且与项目两条既有设计原则存在张力，**不构成现在动手的理由**。
- **登记方式**：建议并入 `ROADMAP.md` 的 `[ ] 自研小模型部署` 长期待办（**截至 2026-10-14 尚未登记**——该待办当前位于 ROADMAP 长期计划段，行号随版本漂移，检索「自研小模型部署」定位），作为候选技术之一，并在该项下附本文第五节的「步骤 0」作为启动前置。
- **值得保留的认知**：laya 所代表的 System 1 / System 2 分层（本地廉价判定 + 远端昂贵生成）与 StoryMoss `model_gateway`（TaskClass → 候选链 → 成本哨兵 / 预算）是同构的。本项目已经在没有它的情况下实现了这一模式的骨架；laya 的价值不在提出该模式，而在于「带校准概率的本地判定器」这一现成封装——**当且仅当判定必须本地时**，它才不可替代。

---

## 附录

### A. 信息源与核验状态

| 来源 | 用途 | 状态 |
|---|---|---|
| `hf-mirror.com/convaiinnovations/laya/raw/main/README.md`（384 行） | 本文全部 laya 数字 | ✅ 逐行阅读；与 GitHub README、PyPI 描述三方互证 |
| `api.github.com/repos/NandhaKishorM/laya` | 31,782★ / 2,825 forks / 93 open issues / Apache-2.0 / created 2026-09-18 | ✅ 直接 API 响应 |
| `hf-mirror.com/api/models/convaiinnovations/laya` | 36,328 downloads / 5,383 likes / `pipeline_tag: text-classification` | ✅ 直接 API 响应 |
| `hf-mirror.com/api/models/onnx-community/laya-multilingual-ONNX?blobs=true` | ONNX 产物体积（fp16 643.8 MB / fp32 1287.6 MB / tokenizer 34.4 MB） | ✅ 直接 API 响应 |
| `hf-mirror.com/api/models/ggml-org/Laya-GGUF?blobs=true` | GGUF 体积（BF16 844 MB / Q8_0 449.4 MB） | ✅ 直接 API 响应 |
| `convaiinnovations/laya-multilingual` 模型卡 | 语言表含 `zh`、低资源语言数字 | ✅ 原文 grep |
| StoryMoss 代码库 | 第二节判定点清单、第三节全部坐标 | ✅ 关键坐标已 `sed`/`grep` 抽查复核（见附录 C） |
| huggingface.co 直连 | — | ❌ 本机超时（DNS 指向 31.13.71.19）；已改镜像并做三方互证 |

### B. 未核验 / 存疑清单（明确标注，不作为论据）

1. **第三方中文盘面的负面诊断**（社区中文命令用例、第三方中文基准分数）：来自检索转述，未逐条亲自复现。本文的结论不依赖它们——「中文必须自训验证」由官方限制即可导出。
2. **社区运行时移植**（CoreML / LiteRT / MLX / 第三方推理服务）：未逐一验证；已核验的只有 ONNX 与 GGUF 两个产物。
3. **独立复现研究对校准叙事的异议**：仅记录存在争议，未采用其数值。
4. **`laya` 名称含义**：上游未文档化。
5. **Apple Silicon 实测延迟**：官方只给了 T4 与 CPU 两组数字，MPS 无单问数字，本文按「需自测」处理。

### C. StoryMoss 侧代码坐标索引

| 引用 | 位置 |
|---|---|
| LLM 适配器契约 | `src-tauri/src/llm/adapter.rs:249-307` |
| 三 provider 与 OpenAI 兼容探测 | `src-tauri/src/llm/openai.rs:320-344`、`config/commands.rs:1368-1674` |
| 意图分类器（8s 超时） | `src-tauri/src/intent.rs:448-522`（超时 `:473`，兜底 `:495-506`） |
| 任务分级关键词表 | `src-tauri/src/model_gateway/dispatcher.rs:75-136` |
| role 表（静态声明） | `src-tauri/src/agency/roles.rs:20-51` |
| 质检门控 | `src-tauri/src/agency/gate.rs:10-49`、`agency/coordinator.rs:6222-6531` |
| 确定性 lint / 启发式质检 | `src-tauri/src/story_system/prose_lint.rs:142-279`、`anti_ai/mod.rs:64-146` |
| 活人作家黑名单（43 条） | `src-tauri/src/creative_engine/style/living_author_guard.rs:23-69` |
| 本地 hash embedding（步骤 0 的基座） | `src-tauri/src/embeddings/embedding.rs:198-254` |
| LocalOnly 约束 | `src-tauri/src/router/router.rs:275-279` |
| 待确认阈值 | `src-tauri/src/story_system/pending_review.rs:18`、`memory/ingest.rs:542-555` |
| 前端写作建议引擎 | `src-frontend/src/frontstage/ai-perception/suggestionEngine.ts:31-345` |
| 设计原则原文 | `PROJECT_STATUS.md:648`、`PROJECT_STATUS.md:736-739`、`ROADMAP.md:586-588`；「自研小模型部署」待办见 ROADMAP 长期计划段（按关键词检索） |
