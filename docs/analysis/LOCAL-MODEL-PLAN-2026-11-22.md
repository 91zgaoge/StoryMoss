# 本机大模型运行方案分析（M5 Max 128 GB / StoryMoss）

> 日期：2026-11-22 08:30–09:10 CST
> 对象：本机（Apple M5 Max 40 核 GPU / 128 GB / macOS 26.6.2）+ StoryMoss v0.99.10 的三档模型路由
> 触发问题：日志报告 **推理模型空 content 2,377 次**、**Qwren127 GPU 容量不足**、**127.0.0.1:11234 不可达**
> 边界：本文的“本机事实”全部来自实机读取（配置、launchd、日志、服务探针）；“生态事实”来自公开仓库/模型卡/榜单（§8 列链接）。未实测的部分标注为推断。

---

## 0. 结论摘要

**三个症状都不是“模型不够强”，而是配置与内存超卖。**

| 症状 | 直接原因（证据） | 先做什么 |
|---|---|---|
| 空 content 2,377 次 | 云端 `deepseek-flash` = **DeepSeek-V4.1-Flash**，是带 effort 档位的**推理模型**；StoryMoss 给它 `max_tokens=16384`，CoT 把预算吃满，`content` 为空（日志签名 `[OpenAI] content 为空但 reasoning_content 有 N 字符`）。应用不回退 CoT 是**正确**行为 | 提高 `max_tokens`、对 JSON/抽取任务设 `reasoning_effort=low`、长文任务用 `high` |
| Qwren127 GPU OOM | 本地服务加载的是 **iQ-MLX-4.7bpw** 变体（resident **107.3 GB**）+ `--ctx-size 262144` + `--kv-quant 8` + prefix cache，在 128 GB 上超卖。实测**连 20 token 的请求都返回 400**：“requires ~657MB but only ~111MB is available” | 换成**同盘已存在**的 `mixed-4-8bit` 包（其 README 自述“128 GB Mac 约 75 GB resident”），并把 `--ctx-size` 降到 32768–65536 |
| 11234 不可达 | `11234` 是 **serial-proxy**（单并发 FIFO 闸门）转发到 `11235` 的 mlx-serve；两个 launchd 进程各自 KeepAlive，**没有依赖与健康联动**。OOM 400、排队、5 秒 keepalive 超时都会被上层当成“端点不可达” | 健康探针放宽到 30 s + 失败重拉；考虑去掉单并发闸门；OOM 时返回可重试语义而不是让调用方冷却整条链 |

**一句话方案**：云端 V4.1-Flash（1M ctx）做创作/长上下文；本地 Flash-Next 的 **75 GB 变体**做离线长文；本地 27B/35B-A3B 做 JSON、门禁、巡检等高频小任务。**今天就改配置，不必先下载任何东西。**

---

## 1. 本机现状（实机读取）

### 1.1 硬件与磁盘

| 项 | 值 |
|---|---|
| 芯片 / GPU | Apple M5 Max，40 核 GPU（Metal 4） |
| 内存 | 128 GB 统一内存 |
| 系统 | macOS 26.6.2（25G83） |
| 磁盘 | 1.8 TB，Data 卷 **仅剩 60 GB**（97% 已用） |

> 磁盘是硬约束：任何“再下一个 68–93 GB 模型”的方案都必须先腾空间（§4.4 有一笔“删重复目录即得 93 GB”的现成空间）。

### 1.2 本地推理服务（launchd）

```
~/Library/LaunchAgents/com.mlx-serve.qwen38.plist      → mlx-serve --serve 127.0.0.1:11235
~/Library/LaunchAgents/com.mlx-serve.serial-proxy.plist → python3.11 serial_proxy.py 127.0.0.1:11234 → 11235
```

mlx-serve 关键参数（plist 原文）：

```
--model  ~/.mlx-serve/models/Qwen3.8-Flash-Next-MLX-Serve-iQ-MLX-4.7bpw
--ctx-size 262144      --kv-quant 8      --prefill-chunk 8192
--kv-attn-mode fused   --timeout 0
环境：MLX_SERVE_MOE_BATCHED_DECODE=0
```

服务自报（`GET /v1/models`）：

```json
{"id":"Qwen3.8-Flash-Next-MLX-Serve-iQ-MLX-4.7bpw",
 "context_length":262144,"max_model_len":262144,
 "bytes_resident":107303406192,"state":"ready",
 "capabilities":["chat","tool_use","streaming","vision","reasoning","json_schema"]}
```

### 1.3 实测：服务“活着”，但已经没有内存

```
POST /v1/chat/completions  {"max_tokens":64,"messages":[...20 tokens...]}
→ 400 {"message":"Prompt (20 tokens) requires ~657MB GPU memory but only ~111MB
        is available and the hot prefix cache holds ~224MB more ... Reduce prompt
        size, lower --ctx-size, or use a smaller model."}
```

**这条错误解释了另外两个症状**：服务在跑、端口在听，但任何真实请求都失败；StoryMoss 侧的失败日志会被归类为“端点不可达/容量不足”。

### 1.4 盘上的模型（两份，几乎一样大）

| 目录 | 权重 + n-gram | 目录占用 | 自述/实测内存 |
|---|---|---|---|
| `Qwen3.8-Flash-Next-MLX-Serve-iQ-MLX-4.7bpw`（**当前加载**） | 70.1 GB safetensors + 23 GB `ngram_table.bin` | 93 GB | **107.3 GB resident**（实测） |
| `Qwen3.8-Flash-Next-MLX-Serve-mixed-4-8bit`（**未加载**） | 70.1 GB + 23 GB | 93 GB | README：“Runs on a 128 GB Mac with about **75 GB resident**” |

> 两份目录内容几乎重复。**保留 mixed 变体、删除 iQ 变体即可立即腾出 93 GB**。

### 1.5 StoryMoss 三档模型配置（`app_config` 实读）

| 档位 | 显示名 | 实际 model | 端点 | max_tokens | 声明 ctx | 备注 |
|---|---|---|---|---|---|---|
| 创作 + 后台 | `deepseek-v4` | `deepseek-flash` | `https://api.deepseek.com` | 16384 | 131072 | 推理模型；API 实际 1,048,576 ctx / 393,216 输出 |
| 工具 | `Qwren127` | `Qwen3.8-Flash-Next-MLX-Serve-iQ-MLX-4.7bpw` | `http://127.0.0.1:11234` | 40960 | 131072 | 服务端 ctx 262144；本地 107 GB |
| （占位） | `Ds10` | `deepseek-v4.1-flash` | `http://10.62.239.13:17098` | 16384 | 131072 | 局域网 2×4090 机（首字慢，勿删） |

DeepSeek 账户实况：可用模型 `deepseek-flash`（=V4.1-Flash，1M ctx，393k 输出，effort low/high/max，文本+图像）、`deepseek-v4-pro`；余额 **¥298.73**。

### 1.6 日志报告（2026-11-22 巡检）

| 签名 | 次数 | 判读 |
|---|---|---|
| `content 为空但 reasoning_content 有 N 字符` | **2,377** | 云端推理模型 CoT-only（max_tokens 被吃满） |
| `跳过 N 个因连续空正文处于冷却期的候选` | 335 | 冷却策略在兜底，但把可用候选也拖下水 |
| `所有候选模型都因连续空正文进入冷却 → 回退旧路径` | 78 | 直接导致旧路径返回空 content |
| `候选实测 GPU 容量上限装不下当前提示` | 76 | 容量路由生效（Qwren127） |
| `[GatewayScheduler] keepalive 超时 (5s)` | 85 | 单并发闸门 + 大模型首字慢 |
| `DeepSeek-V4.1-Flash` 单次 prompt 12.6k–20.1k tokens | 1,200+ | 提示词规模本身在 12–26k |

---

## 2. 根因分析

### 2.1 空 content：推理模型 + 预算错配（不是模型坏了）

- `deepseek-flash` 是 V4.1-Flash，官方 API 暴露 `effort`（low/high/max，默认 **high**）与 393k 输出上限；CoT 与正文共享 `max_tokens`。
- StoryMoss 三档里 creative/background 都指向它，`max_tokens=16384`。长 prompt（12–26k）+ 默认 high effort → 思考链把 16k 用光，正文 0 字。
- 应用在 `llm/openai.rs` 明确**不回退** reasoning（正确，防止把思维链当小说），于是记为一次“空正文失败”，连续两次进 10 分钟冷却，最后整条候选链冷却。
- 报告里 `23,877/23,921/23,914` 这类 completion 计数正好是“CoT 吃满额度”的指纹。

**修法优先级**：① 给不同任务设 `reasoning_effort`（JSON/抽取=low，创作=high）；② creative 的 `max_tokens` 32k–64k；③ 长上下文/一致性任务交给 1M ctx 的 V4.1-Flash；④ 连续空正文的冷却不要连坐“本来可用的其他候选”。

### 2.2 GPU OOM：权重 + ctx 预留超卖

- 128 GB 统一内存里，macOS 自身与 GPU 驱动要占一部分；107.3 GB 权重剩下不足 20 GB。
- `--ctx-size 262144` 意味着服务要为 262k 预留 KV/缓冲；`--kv-quant 8` 只是压缩 KV 元素，不改变“为 262k 预留”的事实。
- 20 token 请求只剩 **111 MB** 可用，说明内存已被权重与预留吃掉；这不是“再重试一次就好”的抖动。
- 报告里的两个数字（16,781 tokens 需 9,439 MB / 仅 3,387 MB；22,659 tokens 需 9,295 MB / 仅 9,148 MB）说明可用量会随服务重启变化，但**上限始终不够**。

**结论**：换 75 GB 变体 + 降 ctx 是根治；只降 ctx 是止血。

### 2.3 11234 不可达：架构问题，不是端口问题

- `serial_proxy.py` 的职责就是“**同一时刻只放一个 LLM 请求**，其余排队”（文件头注释原文）。StoryMoss 的 10 章训练、并发 3 的摘要、巡检 keepalive 全部排在这一条队列上。
- mlx-serve 与 proxy 各自 `KeepAlive`，但**没有“proxy 等 server ready”的依赖**；server 重启/OOM 时 proxy 仍监听，请求失败即被上层当“不可达”。
- keepalive 5 秒超时在“大模型首字 20–60 秒”面前必然误判。

**结论**：要么去掉单并发闸门（mlx-serve 本身支持 batching，见 §3.2），要么把 keepalive 放宽并做“busy ≠ down”的区分。

---

## 3. 生态扫描（2026-11 现状）

### 3.1 开源模型候选（128 GB 可行性）

| 模型 | 规模/激活 | 原生 ctx | 公开榜单分（BenchLM 2026-11） | 128 GB 上可行性 |
|---|---|---|---|---|
| Kimi K3.1 Pro | 未公开（超大） | — | **70**（开源第一） | ❌ 只能云 |
| DeepSeek V4.1 Flash 1017 | 284B / 13B | **1M** | **70** | ⚠️ 2-bit 约 102 GB、DSpark 112 GB，128 GB 跑 1M ctx 不现实；**云上最优** |
| Qwen3.8 Max | ~1T（云级） | 1M | 65.3 | ❌ |
| GLM-5.3-Flash | 320B / 18B | — | 63.5 | ⚠️ FP8 328 GB；2-bit 约 90–100 GB，18B 激活 → 慢；不推荐 |
| **Qwen3.8-Flash-Next** | **180B（125B 主 + 51B n-gram + 4B MTP）/ 6B 激活** | **262k（可扩 1M）** | **60.4** | ✅ 2-bit 混合 **68 GiB**（“128 GB Mac profile”）；3-bit 在 M5 Max 上实测 128k 仍有 20 tok/s |
| Qwen3.8-27B | dense 27B | 262k | 54.1 | ✅ 4-bit ≈16 GB / 5-bit ≈20 GB；M5 Max 5-bit 128k 实测 **47 tok/s** |
| Qwen3.6-35B-A3B | 35B / 3B | 262k | — | ✅ 4-bit 24 GB；M5 Pro 20c 实测 **101–108 tok/s**，M5 Max 更快 |
| MiniMax H3 | —（视频/图像生成） | — | — | 与 LLM 无关 |

> 说明：榜单分数来自 BenchLM 的 open-weight 排行（11-21 数据）；Qwen3.8-Flash-Next 的 60.4 是“开源可下载”里 128 GB 能跑的最高档之一，和云端的 70 分相差约 10 分——这 10 分应该用云端 V4.1-Flash 补，而不是硬塞 284B 到 128 GB。

### 3.2 运行时候选

| 运行时 | 优势 | 代价/限制 | 本机适配 |
|---|---|---|---|
| **mlx-serve**（当前） | 已装、支持 MTP、kv-quant、视觉、262k；本机有两份 pack | serial-proxy 单并发；OOM 直接 400；没有请求级队列上限 | 保留，改参数/换 pack |
| **oMLX** | 有 M5 Max 公开基准、oQ 量化、batch（4× 提速 3.15×）、benchmark 工具 | 需要重新下载 oQ 量化版本 | 推荐作为目标运行时 |
| **mere.run v0.45** | Swift/MLX 原生支持该 180B 模型 2-bit；macOS App + OpenAI 兼容 API | 该 2-bit pack 的 **262k 尚未集成**（QSA 长上下文在推进）；磁盘 82 GB 预算 | 可作备选 |
| LM Studio | 图形化、GGUF/MLX 双路径 | 大 MoE 的 MLX 支持滞后；工具调用有已知问题 | 不推荐为主 |
| llama.cpp + GGUF | 生态最广、DSpark 对 DeepSeek 1.5–1.9× | 180B MoE 在 Mac 上不如 MLX；MTP 在 Metal 是负收益 | 仅作兜底 |

### 3.3 本机可对照的公开基准（M5 Max 40c）

| 模型 / 量化 | ctx | PP tok/s | TG tok/s | 来源 |
|---|---|---|---|---|
| Qwen3.8-Flash-Next oQ3.5e-mtp（3bit） | 128k | 857 | **20.4** | oMLX 社区基准（11-22） |
| 同上 | 64k | 906 | 18.1 | 同上 |
| Swift-1.5-Qwen3.8-27B-Uncensored oQ5e-mtp | 128k | 655 | **47.1** | 同上 |
| 同上 | 64k | 874 | 53.6 | 同上 |
| Qwen3.6-35B-A3B-4bit（M5 **Pro** 20c） | 4k | 2243 | **101.5** | 同上 |
| 参考：Nex-N2-mini 8bit | 195k | 757 | 39.6 | oMLX（Peak mem 51 GB） |

**换算到 StoryMoss 的单章任务**（章纲 1.8k 字 + 上下文 ~20k tokens + 输出 ~3k tokens）：

- Flash-Next 3bit：预填充约 20k/857 ≈ **23 s**，生成 3k/20 ≈ **150 s** → 单章约 3 分钟。
- Qwen3.8-27B 5bit：预填充 ~30 s，生成 3k/47 ≈ **64 s** → 单章约 1.5 分钟，质量低约 6 分。
- 云端 V4.1-Flash：网络+首字 ~2–5 s，3k 输出 ~30–60 s → 单章 <1.5 分钟，质量最高（但按 token 计费）。

---

## 4. 推荐方案

### 4.1 方案 0：今天就改，0 下载（止血，30 分钟）

1. **换 pack**：`com.mlx-serve.qwen38.plist` 的 `--model` 指向 `.../Qwen3.8-Flash-Next-MLX-Serve-mixed-4-8bit`（盘上已有，自述 75 GB resident）。
2. **降 ctx**：`--ctx-size 65536` 起步（验证稳定后再试 131072）；`--prefix-cache-mem` 保持小值。
3. **开 batching**：`MLX_SERVE_MOE_BATCHED_DECODE=1`（当前为 0，等于放弃批处理）。
4. **proxy 语义**：keepalive 超时 5 s → 30 s；把“busy/OOM”与“down”区分开；如可，直接让 StoryMoss 连 11235（保留 11234 仅作兼容）。
5. **云端档**：`deepseek-v4` 的 `max_tokens` 16384 → 32768；JSON/抽取任务加 `reasoning_effort=low`；`max_context_length` 131072 → 1048576。

**判据**：20 token 请求 200；16k prompt 请求 200；并发 2 请求都能返回；报告里 OOM 归零。

### 4.2 方案 1：目标配置（1–2 小时，可选下载）

- 删除重复的 `iQ-MLX-4.7bpw` 目录 → **+93 GB** 磁盘。
- 下载并校验 **Qwen3.8-Flash-Next 2-bit 混合（68.08 GiB）**（Sawfwair 包，mere.run 或 mlx-serve 兼容版本），resident 预计 **75–85 GB**。
- 若运行时支持 262k：`--ctx-size 262144` 恢复长上下文；否则 131072。
- 运行时二选一：**oMLX**（有 M5 Max 基准、batching、oQ 量化）或继续 mlx-serve（省迁移）。

**判据**：Peak mem < 105 GB；64k/128k 请求 TTFT < 60 s；连续 3 次 10 章训练无 OOM。

### 4.3 方案 2：分层路由（推荐长期形态）

| 任务类型 | 推荐模型 | 理由 |
|---|---|---|
| 长文创作 / 全书一致性 | 云端 **DeepSeek V4.1-Flash**（1M ctx，effort high） | 质量 70 分、1M 上下文、¥298 余额足够 |
| 离线长文草稿 / 隐私 | 本地 **Flash-Next 2-bit**（75–85 GB，262k） | 质量 60.4，离线可用 |
| 章纲 / 门禁 / JSON / 巡检 | 本地 **Qwen3.8-27B 5bit**（47 tok/s）或 **Qwen3.6-35B-A3B**（100+ tok/s） | 快 2–5 倍；非思考模式 + json_schema 更稳 |
| 摘要 / 向量化前的清洗 | 本地 27B 或 35B-A3B | 高频小任务，不占大模型内存 |

**关键点**：不要让 180B 模型承担 JSON 门禁这类高频小任务——它的 prefill 成本（20k 上下文 23 s）是纯浪费。

### 4.4 磁盘与内存的账

| 动作 | 磁盘变化 | 内存变化 |
|---|---|---|
| 删 `iQ-MLX-4.7bpw`（重复） | +93 GB（60 → ~153 GB 可用） | — |
| 下载 2-bit pack（68 GiB，抓取预算 82 GB） | −82 GB（余 ~71 GB） | — |
| 换 mixed pack（75 GB resident） | 0 | −32 GB（可用 ~50 GB） |
| ctx 262144 → 65536 | 0 | 再省 KV/缓冲（估 10–30 GB，取决于实现） |

---

## 5. StoryMoss 字段级改动清单

| 位置 | 现值 | 建议值 | 原因 |
|---|---|---|---|
| `deepseek-v4.max_tokens` | 16384 | **32768**（创作）/ **16384**（JSON+low effort） | CoT 与正文共享额度 |
| `deepseek-v4.max_context_length` | 131072 | **1048576** | API 实际支持 1M |
| `deepseek-v4.reasoning_effort` | 未设（默认 high） | JSON/抽取=**low**，创作=high | 降低 CoT 吃满概率 |
| `Qwren127.model` | iQ-4.7bpw | **mixed-4-8bit**（或 2-bit pack） | 75 GB vs 107 GB |
| `Qwren127.max_tokens` | 40960 | **16384–32768** | 输出上限越大，KV 预留越多 |
| `Qwren127.max_context_length` | 131072 | 与 server `--ctx-size` **一致** | 避免应用按 262k 估算容量 |
| `Qwren127.timeout_seconds` | 300 | **600** | 20k prompt + 3k 输出在本地可能超 300 s |
| 工具档路由 | Qwren127 | 建议改 27B/35B-A3B | 高频小任务用快模型 |
| 网关冷却 | 空正文 2 次冷却 10 分钟 | 仅冷却**该模型**，不连坐候选链 | 报告里 78 次“全部候选冷却” |

---

## 6. 验证方法（每次改动都跑同一组）

1. **冒烟**：20 token 请求返回 200；`/v1/models` 的 `bytes_resident` 符合预期。
2. **中等**：16k token prompt + 512 输出，记录 TTFT 与 tok/s。
3. **并发**：同时 2 个请求，确认不是串行（总时长 ≈ 单请求而非 2×）。
4. **端到端**：StoryMoss 跑 10 章训练，统计：OOM 次数、空 content 次数、每章耗时、总分/承接分。
5. **回归线**：与 11-21 的 `f8119c97`（0.857 / 承接 9.9）对比，确认换模型没有把质量换没。

---

## 7. 风险与边界

1. **2-bit 质量下降**：68 GiB 的 2-bit 混合比 4-bit 的 107 GB 少约 39 GB 权重精度，长文质量可能下降；建议用 10 章训练做 A/B，不要只看单章。
2. **262k 在 2-bit pack 的运行时限制**：mere.run 明确写“262,144 在该 runtime 尚未可用（QSA 长上下文仍在集成）”；mlx-serve 是否支持要实测。
3. **磁盘**：Data 卷仅剩 60 GB；不删重复模型就没有下载空间；删除前确认 launchd 没有引用。
4. **单并发闸门**：去掉 serial-proxy 会让并发请求直接打到 mlx-serve；若其 batching 未开，可能反而更慢——两者要一起改。
5. **云侧成本**：V4.1-Flash 便宜但非免费；余额 ¥298.73，按 10 章训练约 30–60 万 token 估算，可跑很多轮，但仍建议“小任务本地、长文云端”。
6. **本文未实测**：2-bit pack 在本机的 TTFT/tok/s、oMLX 迁移成本、mlx-serve 对 262k 的真实支持度——都需要按 §6 验证。

---

## 8. 参考链接

- DeepSeek API 模型与能力（本机 `GET https://api.deepseek.com/models`）：`deepseek-flash` = DeepSeek-V4.1-Flash，1,048,576 ctx / 393,216 输出 / effort low-high-max。
- Qwen3.8-Flash-Next 官方模型卡：https://huggingface.co/Qwen/Qwen3.8-Flash-Next（180B=125B+51B n-gram+4B MTP，6B 激活，262k 原生、可扩 1M）
- Qwen3.8-27B 官方模型卡：https://huggingface.co/Qwen/Qwen3.8-27B（dense 27B、262k、可关思考）
- 2-bit 混合 MLX（68.08 GiB，“128 GB Mac profile”）：https://huggingface.co/Sawfwair/Qwen3.8-Flash-Next-MLX-Mixed-2bit
- iq4 混合 MLX（133 GB，参考对照）：https://huggingface.co/airagrp/Qwen3.8-Flash-Next-iq4-mixed-mlx
- DeepSeek-V4 本地运行与量化体积：https://unsloth.ai/docs/models/deepseek-v4（V4-Flash 284B/13B，2-bit 102 GB、3-bit 103 GB、4-bit 162 GB）
- GLM-5.3-Flash 参数与体积：https://glm5.app/blog/glm-5-3-flash-parameters（320B-A18B，FP8 328 GB，MIT）
- M5 Max 40c 公开基准：https://omlx.ai/benchmarks（Flash-Next 3bit 128k = 857 PP / 20.4 TG；27B 5bit 128k = 655 / 47.1；35B-A3B 在 M5 Pro = 2243 / 101.5）
- 开源模型排行（2026-11）：https://benchlm.ai/best/open-source（Kimi K3.1 Pro 70、DeepSeek V4.1 Flash 70、Qwen3.8-Flash-Next 60.4、Qwen3.8-27B 54.1）
- Apple Silicon 运行时经验汇总（MLX / oMLX / llama.cpp / Ollama 对照）：https://github.com/AtlasOmnia/hermes-megathreads/blob/main/megathreads/mac-mlx-apple-silicon-2026-06.md
- mere.run v0.45（Swift/MLX 运行时，支持该 180B 模型的 2-bit pack）：https://mere.run/

---

## 9. 下一步（按顺序）

1. **改 4 个配置项**（§4.1 的 1–5 条中的 1/2/3/5），重启 launchd，跑 §6 的冒烟与中等测试。
2. 观察 24 小时报告：OOM 应归零，空 content 应显著下降（云端 max_tokens/effort 改动生效）。
3. 若质量不够：删重复模型腾 93 GB → 下 2-bit pack → A/B 10 章训练。
4. 若并发仍是瓶颈：把 11234 的串行闸门换成 oMLX/mlx-serve 原生 batching，保留 11234 作为兼容别名。

---

## 10. 复核更正（2026-11-22 09:00–09:40 实机对账）

落地前用日志与数据库逐条核对，有三处需要更正；结论方向不变，但证据链要换。

1. **2,377 次空 content 不是云端 CoT。** 当前日志里的同类 WARN 全部是 10 秒一次、`reasoning_content` 仅 16–22 字符——这是 keepalive 探测打到 Qwren127 的形状，不是 DeepSeek 的 16k CoT。云端探针（`Respond with exactly the word OK.`）在 `max_tokens=4` 时 2 个 token 就返回 `OK`，不会产生空正文。
2. **`llm_calls.completion_tokens` 实际存的是 `usage.total_tokens`。** `record_llm_call` 里 `completion_tokens = response.tokens_used`，而适配器把 `tokens_used` 设为 `usage.total_tokens`；所以 23,877/27,103 是「提示+输出」总量，不是 CoT 指纹。`prompt_tokens` 又是本地 `count_tokens` 估算，两列不可直接相加当账本。
3. **writer 实际申请的是 8,192，不是 16,384。** `agency/roles/writer.rs` 的 `max_output_tokens = 8192`；`LlmProfile.max_tokens=16384` 只是适配器默认值。真机 08:38–08:44 连续 8 次 `agency_writer` 空正文，是 8k 被 CoT 吃满（`finish_reason=length`）后没有任何放大机制。
4. **DeepSeek 的 `reasoning_effort` 需要显式发送且值域受校验。** 传 `banana` 会 400：`expected one of none/minimal/low/medium/high/xhigh/ultra/max`；模型卡自报支持 `low/high/max`，默认 `high`。StoryMoss 原先的 `LlmProfile.reasoning_effort` 字段从未被读取，属于死配置。

据此，StoryMoss v0.99.11 的应用侧改动是：解析 `finish_reason` + `reasoning_tokens`，空正文撞 `length` 时把 `max_tokens` ×4（封顶 32768）重试一次；打通 `reasoning_effort`（JSON/结构化任务默认 low，未声明 reasoning 的端点不发送）；全部候选冷却时恢复候选而不是直接报错；keepalive 与 pre-call 探测从 5s/4 tokens 放宽到 30s/64 tokens，空正文 WARN 60s 节流。

本机运维（已执行，见 §4.1）：`--model` 换 `mixed-4-8bit`、`--ctx-size 65536`、`--os-reserve-gib 8`、`--prefix-cache-mem 4GB`、`MLX_SERVE_MOE_BATCHED_DECODE=1`。实测：15-token 请求 200；12,618-token prompt 预填充 6.9s；并发 2 请求总耗时 3.56s，串行 2 请求 4.45s（batching 生效）；`/v1/models` 报 `free_mib≈23.6 GB`（改前约 8.2 GB）。`bytes_resident` 仍报 107.3 GB，与 free_mib 矛盾，判断为 pack 元数据静态值，不能当作真实占用。

**仍未做**：StoryMoss 运行中的 `app_config` 仍写旧模型名 `iQ-MLX-4.7bpw` + `http://127.0.0.1:11234`。实测 mlx-serve 对请求里的 `model` 字段不做白名单校验（用旧 id 请求 11235/11234 均返回 200，服务已加载的 mixed 包），因此**不改配置也能继续用**；改配置的收益只是绕过 11234 的单并发闸门（直连 11235）并把 `max_context_length` 对齐到 65536、`is_local_model=true`。建议在方便时重启应用并更新，而不是把它当阻塞项。删除重复的 iQ 目录（+93 GB）与换 2-bit pack 仍按 §4.4 有条件执行。

可选的 `app_config` 更新（`app_settings.app_config` → `llm_profiles["model-a62431a7-6107-4870-9eec-dfb32c83cf0c"]`）：`model` → `Qwen3.8-Flash-Next-MLX-Serve-mixed-4-8bit`；`api_base` → `http://127.0.0.1:11235`；`max_context_length` → `65536`；`timeout_seconds` → `600`；`is_local_model` → `true`。应用启动时读取该 JSON，改库后需重启；运行中的训练结束前不要重启应用。

