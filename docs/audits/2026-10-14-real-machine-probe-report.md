# 真机功能探针报告（v0.66.0–v0.72.0 验收，2026-10-14）

> 日期：2026-10-14（当晚补测见 §五：修正 §二 发现 a/b 的归因，并给出换模结论）
> 缘起：Sherpa 借鉴清单八项落地后，各版文档的「未关闭」均写着「真机端到端未复跑」。本报告是**第一次在真机上把新功能跑通**的记录。
> 方法：真机库副本 + 真机模型。新增 `src-tauri/src/agency/real_machine_probes.rs`（`#[ignore]`、CI 不执行）：
> 复制真机库与向量库到临时目录 → 读真机 AppConfig（`~/Library/Application Support/com.storymoss.app`）构造真实适配器
> （**三档路由**：写作 = `creative_model_id` / 规划 = `tool_model_id` / 编辑 = `background_model_id`，与生产 `llm_for_run(role)` 同构；
> 可经 `STORYMOSS_PROBE_TOOL_MODEL` / `STORYMOSS_PROBE_EDITOR_MODEL` 试穿候选）→ 逐项断言。
> 运行：`cd src-tauri && cargo test --lib real_machine_probe -- --ignored --nocapture`。
> **安全**：真机库/向量库一律副本操作，原库只读；真机文件零写入。

## 〇、环境事实

- 本机安装的是 **v0.72.1** 正式包（`/Applications/StoryMoss.app`，10-14 16:32 安装），当日 16:33 运行过一次 → 新表（`editor_gate_records` / `beat_technique_log`）已随迁移建好。
- 真机库《帝国的烟火》：**3 章**（5382 / 3650 / 1456 字；第 1 章有 1000 字摘要=截断回退、第 2 章 177 字 LLM 摘要、第 3 章空摘要并已入质量债）。故事已从早期的 10+ 章回滚重写。
- 模型：写作档 `deepseek-flash`（deepseek.com，远程）；编辑档 `Qwen3.5-27B-Uncensored-Q4_K_M`（`10.62.239.13:17098`，服务器实报 `qwen3.6-35b-a3b-vision`，512k 上下文 MoE）；**生产档 `Gemma-4-31B-it-Q6_K`（`10.62.239.13:17092`）**；未指派 `Qwen3.8-27B-Abliterated-8bit`（`127.0.0.1:11500`）。

## 一、逐项结果

| 项 | 结论 | 证据（真机运行输出） |
|---|---|---|
| **v0.66.0 检索接线** | ✅ **通过** | 以第 1 章的高区分度片段（「一条用红毡铺就的长道…」）为指令续写第 3 章，writer prompt 注入【前文相关原文（检索）】块并**同时召回第 1 章（红毡长道/「忠烈公明」匾）与第 2 章原文**；前序章索引补建 2 条（缺则由正文前 500 字补）；增量 1510 字 / 100.2s / 共 2 次调用（含一次探针缺口重试） |
| **v0.69.0 手法回灌** | ✅ 通过 | 本拍手法落库 `["useless-detail"]`；`technique_stats` 8 条可读；落库走 detach 写（探针轮询等待） |
| **v0.70.0 卷纲 + 注入** | ✅ 契约通过（模型产出见发现 a） | 真机模型在真实故事上产出完整卷纲（卷目标/卷末高潮/5 条须兑现伏笔/线索预算 2，�n两轮各不同、均合理）；保存后 `generate_chapter_outline` 的 **prompt 含【本卷计划（第 1 卷：第 1–60 章）】与卷目标片段**（硬断言通过）；章纲**产出**为空（模型侧，见发现 a） |
| **v0.67.0 纲层重规划** | ✅ 契约通过（产出同见发现 a） | 候选识别第 3 章（旧纲 434 字）；preview 返回 1 条；**预览不落库**（副本库章纲候选集合不变）硬断言通过；新纲未产出（模型侧） |
| **v0.68.0 判官裁决落库** | ⚠️ 链路可达，真机模型不产出裁决 | `evaluate_gate` 实跑：编辑档（Qwen3.5-27B）**达到最大轮数**熔断；换写作档（deepseek-flash）则**不遵循 JSON action** 熔断 → 无 findings 可落，一致率数字不可得。链路本身由单测覆盖（`editor_gate_records` 落库 + 引用幸存判定） |
| **v0.72.0 人类语料校准（对真机正文）** | ✅ 通过 | 既有探针 `machine_probe`：机器腔样本命中 5 条、人类对照 0 误报；真机 3 章各命中 1 条（第 3 章 `mood-particle-absent`、第 1/2 章 `flat-rhythm`）——与语料校准后的人类文本表现一致（"恰好 3 句连串"不再报，此二章命中为更长连串或情绪直陈缺失） |
| **v0.64.7 生死状态（顺带复跑）** | ✅ 通过（探针已加数据形态守卫） | 真机死亡名单：`{苏会山: 第2章, 明成公主: 第2章}`；第 10 章不存在（故事已重写）→ 复活场景无法复现，探针按形态跳过后半段 |

## 二、发现（按处置优先级）

> **⚠️ 2026-10-14 当晚补测修正**：本节两条发现（a/b）的「坏在哪个模型」结论已被 §五 的换模矩阵推翻——
> 旧探针的档位路由有缺陷（只把 `TaskType::Editing` 认作编辑档，而 EditorAuditor 的角色规格任务类型是
> `Proofreading`；章纲/规划作为 Producer 任务在生产走 Tool 档）。修正后的实测：**编辑档 Qwen3.5-27B 能出完整裁决并落库
> （b 不成立，无需换）**；**章纲路径的真正问题在 Tool 档 Gemma-4-31B——端点已死**（a 的处置对象是 Tool 档，不是写作档）。
> 以下原文保留作审计记录，处置以 §五 为准。

### a. `deepseek-flash` 是推理模型：长输出会 content 为空 ⚠️ 影响章纲类任务（处置已修正，见 §五）

应用日志原文：`[OpenAI] content 为空但 reasoning_content 有 17 字符（思维链/CoT）。不回退到 reasoning_content——它是模型的推理过程而非正文。`
适配器的拒绝回退是**正确**的；但在该模型上，章纲生成（max_tokens 2048）实际拿不到正文（真机两轮均 0 字），卷纲（2048，JSON）与续写（8192）可用——**输出预算被思维链吃掉**（补测：空响应可复现，但「预算被思维链吃掉」的机制证据不足——应用日志里 reasoning_content 仅 16–22 字符；改写为「结构类任务上间歇性空响应/非 JSON」更准确）。

- 影响：`generate_chapter_outline`（章纲）与 `outline_replan`（重规划预览）在写作档模型为推理模型时不可用；续写/卷纲可用。
- 建议（用户决策）：章纲/规划类任务换**非推理**模型；或为该档提高 `max_tokens`（应用侧可加「推理模型 content 为空时调大预算重试一次」的韧性，属后续候选改进）。

### b. 真机质量门拿不到编辑裁决 ⚠️ 审查质量打折（**已推翻：探针路由缺陷，非模型问题**，见 §五）

编辑档（Background=Qwen3.5-27B）达到最大轮数熔断；写作档（deepseek-flash）不遵循 JSON action 熔断。两条路都走不到可解析裁决 → 质量门 Failed → 既有 salvage 保产出（≥600 字放行），但**编辑审查形同虚设**，且「判官一致率」永远无数据。

- 建议：把「编辑审计」角色指向**遵循 JSON 协议**的模型（设置 → 模型 → 角色指定），然后做一次续写并在「创作评估」观察是否出现一致率数字。这也印证了 v0.68.0 文档里那条「评审模型档位由作者按数据自行决定」的提醒。

### c. 仪表盘「创作路径」面板在窄屏（≤1115pt）文字竖排 ⚠️ 布局缺陷

本机主屏可见宽 1115pt（即用户日常窗口上限）；仪表盘右侧面板的三张卡被挤到每个约 85pt 宽，正文逐字竖排。同一组卡片在「故事库」页渲染正常 → 是**仪表盘面板的响应式断点问题**，非截图环境问题。

- 建议：下次 UI 迭代时给该面板加断点（窄屏改为纵向堆叠或隐藏次要卡）。

### d. 知识图谱画布在真机不渲染（图谱 tab 空白）⚠️ 待复现

真机「知识图谱 → 图谱」tab：头部计数正常（11 实体 · 160 关系），但画布全黑（连 ReactFlow 的背景点阵都没有 → 整棵子树未渲染，疑似渲染异常被边界吞掉或容器高度为 0）。「记忆健康」「已归档」「知识蒸馏」tab 正常。前端 `KnowledgeGraphView` 单测存在且绿（jsdom），真机 WKWebView 差异待查。

- 建议：开发模式打开 devtools 复现（`cargo tauri dev`），确认是容器尺寸还是渲染异常；本报告仅记录现象。

## 三、副产品

- **官网三张截图换成真机实拍**（`landing/public/screenshots/`）：幕前写作（真实第 3 章正文）、幕后故事库、知识图谱页的记忆健康视图。**旧图实为错误态截图**——2026-07 用无后端的 headless Chromium 截的，红条「无法连接到本地服务」+ 空状态 + 改名前的 StoryForge 品牌，已上线数月。第三张因发现 d 改用同页记忆健康视图（真实数据：11 实体 / 160 关系 / 系统状态良好），文案同步。
- 探针模块入库（3 项，全 `#[ignore]`）；`life_status` 真机探针加数据形态守卫（故事回滚后不再误红）。

## 四、未关闭

- 发现 a/b 的处置见 §五：**Tool 档换 Qwen3.5-27B（待用户在设置页执行）**、编辑档不换、写作档不换；本报告**不声称续写质量或审查质量在真机已达标**。
- 发现 c/d 未修（本轮范围外），c 有明确修法、d 需开发模式复现。
- 编排：本轮探针未启后台质检（`for_test` 无 app_handle 时 `spawn_editor_qc` 显式跳过）；真机上的「续写 → 后台质检 → toast」链路由用户手动续写观察。
- §五 的换模结论已用探针复跑（矩阵 / 生产函数 / 全链路）验收，但**设置页换档后的应用内复验**（重规划章纲预览 + 创作评估一致率）仍待用户执行后确认。


## 五、换模决策（2026-10-14 晚补测；发现 a/b 的处置）

### 5.1 生产路由事实（代码核实）

| 路径 | 角色标签 | 模型档 | 现值 |
|---|---|---|---|
| 章节纲 `generate_chapter_outline`（`TaskType::Analysis`）、卷纲 `generate_volume_plan`（`WorldBuilding`）、重规划预览 | `agency_producer` | **Tool** | **Gemma-4-31B-it-Q6_K** |
| 质量门裁决（`Proofreading` 的 ReAct tool_loop + 散文回退单发 JSON） | `agency_editor` | **Background** | Qwen3.5-27B-Uncensored-Q4_K_M |
| 续写正文（`CreativeWriting`） | `agency_writer` | **Creative** | model-0a2ffdd2（deepseek-flash） |

端点现状（本机实测）：`10.62.239.13:17092`（Gemma）**端口已关**；`127.0.0.1:11500`（Qwen3.8-27B-Abliterated，未指派）**服务未启动**；`10.62.239.13:17098`（Qwen3.5-27B，服务器实际报 `qwen3.6-35b-a3b-vision`，**512k 上下文**）与 `api.deepseek.com` 存活。应用日志还显示：10-07/10-08 的 37 次 `agency_producer` 请求首位候选都是 Gemma，其中 **28 次以错误收尾**（Gemma `[INTERNAL_ERROR]` / 后继候选也失败）、9 次靠候选链回退到其它模型才完成；今天 16:33 应用启动时 Gemma 再次报错；应用自身模型基准里 Gemma 21 次尝试全部未完成。

### 5.2 换模矩阵（`real_machine_probe_role_model_viability`，四档直连实测）

| 模型档案 | 连通（短任务） | 章纲式产出（2048 token） | 编辑裁决 JSON（生产散文回退函数） |
|---|---|---|---|
| **Gemma-4-31B-it-Q6_K**（现 Tool） | ❌ 端点拒连 | ❌ | ❌ |
| **Qwen3.5-27B-Uncensored-Q4_K_M**（现 Background） | ✅ 2 字 / 2.4s | ✅ **375 字 / 4.4s** | ✅ 解析出裁决 / 6.4s（8 维打分） |
| **deepseek-flash**（现 Creative） | ✅ | ✅ 403 字 / 3.5s（短提示） | ✅ 单发 11.8s |
| **Qwen3.8-27B-Abliterated-8bit** | ❌ 服务未启动（本机） | ❌ | ❌ |

### 5.3 生产函数复测（`real_machine_probe_volume_plan_and_outline_injection`，同一 60s 连接超时口径）

| Tool 档设为 | 卷纲（JSON） | 章纲（真实 `scene_outline` 提示词 + 本卷计划注入） |
|---|---|---|
| Gemma-4-31B（**当前配置**） | ❌ 端点拒连，跳过 | ❌（同） |
| **Qwen3.5-27B-Uncensored-Q4_K_M** | ✅ 完整卷纲（目标/高潮/伏笔/预算） | ✅ **1708 字 / 21s**（含【本卷计划】注入断言通过） |
| deepseek-flash | ❌ 2/2「未返回 JSON」 | ❌ 空响应（0 字；另一次卷纲成功后章纲 0 字——间歇性） |

### 5.4 全链路验收（拟换配置下复跑探针 1）

`creative=deepseek-flash`（不变）+ `tool=Qwen3.5` + `editor=Qwen3.5`：续写 **1660 字 / 64s** ✅；检索块注入（召回第 1/2 章）✅；
每拍手法落库 ✅；**质量门产出完整裁决**（`RevisionRequired`，8 维打分 + 3 条建议）✅；`editor_gate_records` **1 行已落库** ✅；
判官一致率可算（本章 blocking 为 0，命中 0 条属正常）✅。测试 78.78s 全绿。

### 5.5 结论：怎么换

1. **工具模型（Tool）：从 `Gemma-4-31B-it-Q6_K` 换成 `Qwen3.5-27B-Uncensored-Q4_K_M`**（幕后 → 设置 → 模型管理 → 角色指定 → 工具模型；底层 `set_active_model(role='tool')`，自动刷新网关注册表并清除降级）。这是修复「章纲/卷纲/重规划不可用」的一步。
2. **后台任务模型（Background）：保持 `Qwen3.5-27B-Uncensored-Q4_K_M` 不变**——补测证明它能出裁决并落库，发现 b 不需要换模型。
3. **创作模型（Creative）：保持 `deepseek-flash` 不变**——正文质量最好（1660–1746 字/次）；它不适合 Tool/结构任务（空响应、非 JSON），换到 Qwen3.5 后即各就各位。
4. （可选）若要用回 Gemma / Qwen3.8-Abliterated：先把 `10.62.239.13:17092`、`127.0.0.1:11500` 两个服务起起来，再跑 `real_machine_probe_role_model_viability` 复测通过后才指派。

**探针侧修正（test-only）**：`ProbeLlm` 改为三档路由（`CreativeWriting`→创作 / `WorldBuilding|Analysis`→工具 / `Proofreading|Editing`→编辑）+ `STORYMOSS_PROBE_TOOL_MODEL`、`STORYMOSS_PROBE_EDITOR_MODEL` 覆盖；`build_adapter` 连接超时 10s→60s（对齐生产；10s 会把本地 MoE 大提示词的非流式调用误判为连接超时）；gate 记录读取加 10s 轮询（落库走 detach 写，旧探针读到「0 行」是时序假象——本轮实测行已落库）。

### 5.6 换档后复测（2026-10-15，用户已改配置）

用户实际换法与建议有一处不同：**后台任务模型**未保持 Qwen3.5，而是指向了**新的本地 MLX 档** `Qwen3.8-Flash-Next-MLX-Serve-iQ-MLX-4.7bpw`（`127.0.0.1:11234`，MLX 引擎，MoE，**262144 上下文**，capabilities 含 `json_schema`/`tool_use`/`reasoning`；旧的 Gemma 档案已从配置移除、Abliterated 档案被此新档替换）。三档现为：Creative=deepseek-flash / Tool=Qwen3.5-27B / Background=新 MLX。复测（全部默认读真机配置）：

| 测试 | 结果 |
|---|---|
| 换模矩阵（三档直连） | Qwen3.5（Tool）：连通 ✅ / 章纲 403 字 ✅ / 裁决 ✅（5.7s）；**新 MLX（Background）：连通 ✅ / 章纲 384 字 ✅ / 裁决 ✅（9.8s，score 4.5）**；deepseek（Creative）：连通 ✅ / 章纲 554 字 ✅ / **编辑裁决 ❌ 应答 0 字**（单发空响应再次出现，晨间其曾成功——间歇性） |
| 探针 2（生产函数） | 卷纲 ✅ + **章纲 1835 字**（`scene_outline` 提示词 2544 字，含【本卷计划】注入断言）✅ |
| 探针 1（全链路） | 续写 1515 字 / 103s ✅；检索块注入 ✅；手法落库 ✅；**质量门完整裁决（8 维，编辑分 5.0）** ✅；`editor_gate_records` 1 行 ✅；判官一致率链路通（本章无 blocking，命中 0）✅ |
| 全套本地验证 | `cargo test --lib` 1816 passed；fmt / clippy / architecture_guard ✅ |

**观察点（只报不改）**：
- 新编辑档（MLX）本轮给出 8 维全 5.0、model 分 1.00（Qwen3.5 同期给 4.0–4.5）——编辑审美偏宽，需用「判官一致率」随作者修改数据积累后再判断；本轮 gate 仍因 rule/合同项（加权 0.72 < 0.75）拦下，说明加权门未被编辑放水绕过。
- 「合同兑现偏低（0.00）」在近几轮探针中持续出现（换档前后一致）——疑为 runtime contract 未覆盖该章的既有现象，与模型无关，留待单查。
- deepseek-flash 的空响应具有间歇性且与任务形态相关（正文生成稳定；结构/裁决类调用偶发 0 字）——维持「只做写作」的定位。
