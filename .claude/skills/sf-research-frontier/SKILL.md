---
name: sf-research-frontier
description: StoryForge 可推进的开放前沿——Context Rot 防御深化（长篇连载下记忆衰减量化与对抗）。何时加载：要规划研究、要评估长上下文记忆方向、要量化 Lost in the Middle、要扩展 ContextPrioritizer、或被问“下一步研究做什么/本项目的前沿在哪”时。
---

# 研究前沿：Context Rot 防御深化

> 用户确认的本项目“超越当前 SOTA”方向。当前 `ContextPrioritizer`（v0.25.0）只做按优先级排序 + Critical 双重锚定；前沿是把“记忆衰减”从启发式变成可量化、可对抗的机制。

## 为何当前 SOTA 失败

- 长上下文模型普遍存在 "Lost in the Middle"：中间段落的约束记忆衰减，越长越严重。
- 现有缓解（位置编码外推、注意力缩放、RAG）都是模型侧通用手段，**不针对“小说连载数十万字后哪些约束必须不丢”**这一具体场景。
- 本项目 `ContextPrioritizer` 把系统提示词分 Critical/High/Normal/Background 并双重锚定，但**尚无量化**：衰减到什么程度？哪些类先丢？锚定到底救回多少？——目前是启发式，不是测量过的机制。

## 本项目特定资产（别人没有的杠杆）

1. **结构化的记忆分层**：Working/Episodic/Semantic + 五级优先级 + 艾宾浩斯 `R(t)`——已有衰减数学，可复用做“上下文内”衰减模型。
2. **合同系统**：Critical 约束（合同红线、在世作者保护、反 AI 陈词滥调）明确可枚举——可做“哪些约束丢了”的精确探针。
3. **`creative_workflow.log` + 诊断卡片**：可捕获每次生成的完整提示词与模型输出——天然实验台。
4. **长篇真实创作会话**：真实连载数据（不是合成 benchmark）。

## 本 repo 内的前三步（具体）

### 步骤 1：衰减探针——测量“哪些约束先丢”
- 在 `ContextPrioritizer` 注入的 Critical 约束里埋可探针的“事实性问题”（如“主角当前是否在 X 地”），在生成输出后用一次轻量校验 LLM（或规则）检测是否被违反。
- 落点：`src-tauri/src/creative_engine/context_prioritizer.rs` + 新增 `context_rot_probe` 模块（**实验 flag：新增配置轴走 `sf-config-and-flags` 清单 + `sf-change-control` 门禁，实验期默认关闭**）。
- 产出：每次生成的“约束违反率”随上下文长度变化的曲线。

### 步骤 2：锚定有效性量化——双重锚定到底救回多少
- A/B：同一长上下文，A 组用现 `ContextPrioritizer` 双重锚定，B 组只排序不锚定。
- 指标：Critical 约束违反率差异。
- 落点：复用步骤 1 的探针；记录到 `docs/plans/` 一份对照报告。

### 步骤 3：自适应锚定——根据衰减曲线动态补强
- 基于步骤 1/2 的曲线，对“先丢的类别”在尾部追加更强锚定（或周期性 mid-context 重锚）。
- 落点：`ContextPrioritizer` 加 `adaptive_reinforce` 路径；实验 flag 控制。

## 可证伪的“你有结果当…”里程碑

- **M1（测量）**：你有一条“约束违反率 vs 上下文长度”曲线，且能复现（N≥3 模型 × N≥5 长度档）。
- **M2（机制）**：你提出一个机制（如“Critical 类在 ≥K token 后违反率超阈值”），它预测的“先丢类别”与实测一致，且对抗该类后违反率下降 ≥30%。
- **M3（采纳）**：自适应锚定在真实连载会话中使 Critical 违反率下降 ≥30%，且不显著增加 token 成本（< 20%），通过 `sf-change-control` 晋升为默认。

未达 M1 前所有结论标 candidate；未达 M3 前不写入 `ARCHITECTURE.md` 为既成事实。

## 何时 NOT 用本技能

- 把直觉变结果的纪律 → `sf-research-methodology`。
- 已实施功能 → `sf-architecture-contract` §7 ContextPrioritizer。
- 活问题战役 → `sf-genesis-campaign`。

## 出处与维护

- 重验证命令：
  - `rg -n 'ContextPrioritizer|ContextChunk|Critical|High|Normal|Background' src-tauri/src | head`
  - `rg -n 'Context Rot|Lost in the Middle|context_prioritizer' ARCHITECTURE.md`
- 易漂移项：ContextPrioritizer 实现位置、优先级枚举。
- 最后核对：2026-07-07，v0.26.23（前沿，candidate）。
