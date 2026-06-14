# 文思资产分级与三模式创作系统设计文档

> 创建日期: 2026-06-14
> 状态: 设计已确认，待实施
> 问题: AI 创作中资产深度介入导致生成过慢，质量与速度不可兼得

---

## 设计决策摘要

用户的核心痛点：强化了专业资产介入就无法迅速得到正文内容，放松则无法达到应有质量。

**根本原因**：所有模式共用同一条同步执行路径，全量加载资产、全量执行 Inspector、全量做 Rewrite，标准模式用户被迫承载专业模式的开销。

**解决方案**：重构文思系统为三级模式（无/标准/Pro），建立四级资产体系（P0/P1/P2/P3），"/" 命令作为专业资产的边界标志，AgentOrchestrator 拆为 Standard/Pro 两条独立路径。

---

## 模块 1：资产分级体系

### P0 — 防错级（基础元素，标准/Pro 必加载）

确保 AI 生成时不偏离基础设定。

- **世界观核心设定**：location / atmosphere / time
- **角色核心设定**：姓名 + 关系 + 基本属性
- **大纲**：场景大纲 + 章节大纲
- **故事线**：主线方向约束
- **网文模板 GenreProfile**：43 个体裁模板的基础约束（核心基调/节奏策略/反模式清单）

### P1 — 专业级（专业质量核心引擎，Pro 必加载）

决定作品能否达到专业水准。

- **方法论引擎**：雪花法 / 场景节拍表 / 英雄之旅 / 人物深度 / 高密度世界构建
- **风格DNA**：六维模型（词汇/句法/修辞/视角/情感/对白）+ StyleBlend + StyleFingerprint
- **三层记忆编排器**：Working + Episodic + Semantic MemoryPack
- **叙事快照**：CanonicalState（世界事实 / pending payoffs / 角色状态 / 冲突 / 时间线）

### P2 — 审计级（质量审查与评分，Pro 必加载）

决定专业质量的审查上限。

- **合同体系**：MASTER_SETTING + VOLUME + CHAPTER ContractTree + RuntimeContract
- **伏笔追踪**：ForeshadowingTracker（开放伏笔 + 逾期预警）
- **连续性检查**：ContinuityEngine（角色状态 / 时间线一致）
- **追读力评估**：Hook / Coolpoint / Micropayoff / Debt 四维评分
- **Anti-AI 五维审查**：词汇 / 语法 / 叙事 / 情感 / 对话
- **Inspector 系统**：轻量版（P0 资产）+ 深度版（全资产）

### P3 — 深度洞察级（Pro 全量）

需要跨章节深度关联的场景。

- **语义检索**：kb_search（BM25 + Vector RRF）
- **知识图谱深度查询**：KG 实体 + 关系深度遍历

---

## 模块 2：文思三模式行为定义

### 模式映射

| 模式 | 资产层级 | 触发方式 | 调用场景 | Preflight | Inspector | Rewrite | Skills |
|------|---------|---------|---------|-----------|-----------|---------|--------|
| **无** | 按需触发 | 用户在编辑器输入 `/` | 形态 A：人类写作时手动触发专业辅助 | 按需 | 按需 | 按需 | 按需 |
| **标准** | P0 | `auto_write` / 普通生成按钮 | 形态 B：AI 自动续写生成 | QuickCheck | Light（不阻塞） | 跳过 | 不加载 |
| **Pro** | P0+P1+P2+P3 | 用户在编辑器输入 `/` | 形态 C：用户明确要求专业水准 | FullCheck | Deep（阻塞） | 最多 2 轮 | 加载 |

### 关键行为差异

- **标准模式**的 Inspector 和 Rewrite 全部跳过——P0 资产不足以做深度审计，触发 Rewrite 意义不大
- **Pro 模式**的 Preflight 走全检，Inspector 用深度版（6 维度），Rewrite 最多 2 轮，Post-processing skills 加载 emotion_pacing + style_enhancer
- **无模式**本身不是独立的执行路径——它是"等待"状态，用户输入 `/` 后即刻切换到 Pro 模式执行

---

## 模块 3："/" 指令路由重构

### 当前状态

- `/` 输入框支持 `Enter` 提交、`Esc` 取消
- 提交后分三路：`自动续写`→WenSiPanel、`审校`→WenSiPanel、其他→`smart_execute`(后端 LLM IntentParser)
- 路由依赖后端 LLM 意图识别（一次 LLM 调用）

### 重构方向

`/` 是**专业命令的标志**，所有 `/` 指令必须走 Pro tier（P0+P1+P2+P3）。路由策略改为**前端轻量关键词匹配**，不消耗额外 LLM 调用。

### 路由表

| 前端关键词 | 路由目标 | 执行模式 |
|-----------|---------|---------|
| 续写 / 写 / 生成 / 继续 | WriterAgent(tier=PRO) | Pro |
| 润色 / 精修 / 修改 / 优化 | Pipeline(Refine→Review→Finalize) | Pro |
| 分析 / 审读 / 评价 / 追读力 | Analyzer(tier=PRO) | Pro |
| 角色状态 / 角色更新 | CharacterStateService | Pro |
| 其他自然语言 | WriterAgent(tier=PRO, 自由指令) | Pro |

### 实现位置

前端 `RichTextEditor.tsx` 中的 `handleSlashSubmit` 改为先做本地关键词匹配，再将匹配结果 + 原始文本发送到后端。后端不再需要 IntentParser 做路由判断，直接根据前端传来的 `command_type` 执行。

---

## 模块 4：Preflight 延迟校验重构

### 当前问题

- `PreflightChecker` 执行 4 项检查（合同 / 角色 / 大纲 / 场景），全部阻塞
- 每次生成都要跑完整预检
- 合同检查涉及 DB 查询 `story_contracts` 表

### 重构方案

**QuickPreflightChecker**（标准模式）：
- 仅检查：角色非空
- 失败返回 `PreflightError::NoCharacters`

**FullPreflightChecker**（Pro 模式）：
- 现有 4 项检查全部执行：
  1. MASTER_SETTING contract 存在
  2. CHAPTER contract 存在
  3. Characters 非空
  4. Scene 有 outline
- 失败触发 AutoContractBuilder 自动补齐（5 次 LLM 调用）

---

## 模块 5：Inspector 分层重构

### LightInspector（标准模式）

- 3 维度：角色是否存在（名称匹配）/ 场景设定是否一致 / 逻辑基本一致性
- **不阻塞输出、不触发 Rewrite**——仅做信息收集
- 结果写入 scene metadata，不中断生成流程

### DeepInspector（Pro 模式）

- 6 维度：连续性 / 逻辑 / 角色 / 伏笔 / 节奏 / 风格
- **阻塞输出并触发 Rewrite**——Pro 模式的质量门禁
- 保留现有 skip_rewrite_threshold（0.90），评分高时跳过 Rewrite
- 最多 2 轮 Rewrite 循环

---

## 模块 6：AgentOrchestrator 重构

### 新增 tier 枚举

```rust
enum GenerationTier {
    Standard,  // P0 资产，LightInspector，无 Rewrite
    Pro,       // P0+P1+P2+P3，DeepInspector，最多 2 轮 Rewrite
}
```

### Standard 路径

1. QuickPreflightChecker::check()
2. ContextOptimizer: tier=0 只构建 P0 上下文
3. WriterAgent(tier=Standard) → LLM 调用
4. StyleChecker (轻量)
5. LightInspector (信息收集，不阻塞)
6. 保存

### Pro 路径

1. FullPreflightChecker::check()
2. ContextOptimizer: tier=3 构建 P0+P1+P2+P3 上下文
3. WriterAgent(tier=Pro) → LLM 调用
4. DeepInspector → 评分 < threshold → Rewrite (最多 2 轮)
5. StyleChecker (全维度)
6. apply_writing_skills
7. Memory write (P1 三层记忆 + P3 KG)

### Fast 模式保留

用于 Ghost Text（幽灵文本）等实时辅助场景，不走完整流程，不改变现有行为。

---

## 模块 7：AssetLoader 新增模块

### 职责

按 tier 动态加载和注入资产到 writer prompt，替代当前 `build_writer_prompt` 中的硬编码注入逻辑。

### 接口

```rust
struct AssetLoader;

impl AssetLoader {
    /// 根据 tier 加载资产，返回 prompt 注入字符串
    fn load(tier: AssetTier, story_id: &str) -> Result<PromptInjection>;
}

enum AssetTier {
    L0, // P0 防错级
    L1, // P1 专业级
    L2, // P2 审计级
    L3, // P3 深度洞察级
}
```

### 注入顺序

1. L0 → P0 资产（世界观 / 角色 / 大纲 / 故事线 / GenreProfile）
2. L1 → P1 资产（方法论 / 风格DNA / MemoryPack / CanonicalState）
3. L2 → P2 资产（ContractTree / 伏笔 / 连续性 / 追读力 / Anti-AI）
4. L3 → P3 资产（语义检索 / KG 查询）

### 缓存策略

- 复用现有 `ContextCache`（RwLock, 50 条, 300s TTL）
- 缓存 key 包含 story_id + tier 组合

---

## 改动文件清单（预估）

### 后端（Rust）

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `src-tauri/src/creative_engine/mod.rs` | 新增 | 新建 `asset_loader` 模块 |
| `src-tauri/src/creative_engine/asset_loader.rs` | 新增 | 四级资产加载器 |
| `src-tauri/src/agents/orchestrator.rs` | 修改 | 新增 `GenerationTier` 枚举，Standard/Pro 两条路径 |
| `src-tauri/src/agents/service.rs` | 修改 | 调整 context build 逻辑，按 tier 选择资产 |
| `src-tauri/src/story_system/preflight.rs` | 修改 | 拆分 QuickPreflightChecker / FullPreflightChecker |
| `src-tauri/src/agents/mod.rs` | 修改 | 新增 Inspector 分层枚举 |
| `src-tauri/src/pipeline/inspector.rs` | 修改 | 拆分为 LightInspector / DeepInspector |
| `src-tauri/src/commands.rs` | 修改 | `/` 路由接收前端传来的 `command_type` |
| `src-tauri/src/agents/commands.rs` | 修改 | `handleSlashSubmit` 增加前端关键词匹配 |
| `src-tauri/src/creative_engine/context_builder.rs` | 修改 | `build_core_sync` 接受 tier 参数，按 tier 选择上下文 |

### 前端（TypeScript / React）

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `src-frontend/src/frontstage/components/RichTextEditor.tsx` | 修改 | `handleSlashSubmit` 改为前端关键词路由 |
| `src-frontend/src/frontstage/components/WenSiPanel.tsx` | 修改 | 新增 tier 选择 UI（可选，Pro 版功能） |
| `src-frontend/src/agents/types.ts` | 修改/新增 | 新增 `GenerationTier` 类型定义 |
| `src-frontend/src/services/api/` | 修改 | 新增 tier 参数传递 |

---

## 实施建议

### Phase 1：基础设施（资产分级 + AssetLoader）
- 建立四级资产体系的数据结构
- 实现 AssetLoader 模块
- 重构 ContextOptimizer 接受 tier 参数

### Phase 2：执行路径（AgentOrchestrator + Inspector + Preflight）
- 新增 GenerationTier 枚举
- 实现 Standard 和 Pro 两条路径
- 拆分 Inspector 为 Light / Deep 版本
- 拆分 Preflight 为 Quick / Full 版本

### Phase 3："/" 指令路由
- 前端关键词匹配逻辑
- 后端接收 command_type 并执行
- 移除后端 LLM IntentParser 的路由依赖

### Phase 4：前端适配
- RichTextEditor slash 输入框改造
- 更新状态管理，传递 tier 参数

---

## 风险评估

| 风险 | 影响 | 缓解 |
|------|------|------|
| AssetLoader 重构引入回归 | 中 | 保留现有 `build_writer_prompt` 作为 fallback，逐步迁移 |
| Standard 模式跳过 Inspector 影响质量 | 低 | Standard 模式本身就是"人类水平"，不需要深度审计 |
| 前端关键词匹配覆盖不全 | 低 | 关键词匹配失败时 fallback 到默认 WriterAgent |
| 性能回归 | 低 | Pro 模式性能预期不变，Standard 模式预期加速 50%+ |

---

## 预期效果

| 模式 | 重构前耗时 | 重构后耗时 | 改善 |
|------|-----------|-----------|------|
| 普通生成（形态 B） | 25-170s | 5-15s | 加速 5-10x |
| Pro 生成（形态 C） | 25-170s | 60-180s | 性能持平（但质量更高） |
| / 指令触发（形态 C） | 无 | 60-180s | 新增专业创作能力 |
