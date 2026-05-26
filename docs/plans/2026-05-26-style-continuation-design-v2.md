# 续写功能加固设计方案 v2 — 统一风格指纹引擎

> 状态：待审批
> 核心原则：**不区分场景，统一技术底座**。参考文本无论来源（用户前文/外部经典/任意片段），技术要素完全相同。

---

## 一、问题本质的高度归纳

用户要求的核心能力用一句话概括：

> **给定任意参考文本，在其基础上续写，使续写部分与参考文本在语言风格上保持高度一致。**

无论参考文本是：
- 用户自己小说的前文
- 红楼梦选段
- 金庸章节
- 网文章节
- 科幻/悬疑/言情任意类型

**技术要素完全相同**：参考文本 → 风格指纹提取 → 约束化生成 → 一致性校验

唯一的变量是「参考文本来源」，这只是一个输入获取问题，不是技术架构问题。

---

## 二、现有资产复用分析

### 已存在的轮子（零改动复用）

| 组件 | 复用方式 |
|------|---------|
| **续写入口** | Ctrl+Enter / WenSiPanel / Slash命令 完全不变 |
| **Writer Agent** | 核心生成器，只改 prompt 注入内容 |
| **Orchestrator 闭环** | Writer→Inspector→Writer，只增加风格评分维度 |
| **auto_write 循环** | 长续写的分段生成，每段都注入同一风格指纹 |
| **StyleDNA 框架** | 作为风格指纹的上层分类结构 |
| **Anti-AI Review** | 复用文本统计基础设施（词频、句长、修辞计数） |
| **PlanExecutor** | 执行编排逻辑完全不变 |

### 需要精准增强的 3 个关键点

| 位置 | 现状 | 需要增强为 |
|------|------|-----------|
| `prompts/engine.rs` Writer 系统 prompt | "风格: {{tone}} / 节奏: {{pacing}}" + "保持文风一致" | 注入**量化风格指纹**（句长分布、词汇偏好、N-gram 白名单、锚点片段） |
| `agents/commands.rs` auto_write prompt | "请继续续写，保持故事连贯性和风格一致性" | 复用 Writer 系统 prompt 的风格指纹注入 |
| `prompts/engine.rs` Inspector 质检 | 5 个维度（无风格一致性） | 增加第 6 维度**风格一致性评分**，参与 Orchestrator 反馈闭环 |

---

## 三、核心设计：风格指纹（Style Fingerprint）

### 3.1 什么是风格指纹

从任意参考文本中提取的**可量化、可对比、可注入 prompt** 的风格特征集合。

```rust
/// 风格指纹 — 统一描述任意文本的语言风格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleFingerprint {
    /// 文本来源标识
    pub source: FingerprintSource,
    /// 词汇层指纹
    pub vocabulary: VocabularyFingerprint,
    /// 句法层指纹
    pub syntax: SyntaxFingerprint,
    /// 修辞层指纹
    pub rhetoric: RhetoricFingerprint,
    /// 对话层指纹（如参考文本含对话）
    pub dialogue: DialogueFingerprint,
    /// 锚点片段 — 最具代表性的原文段落（用于少样本注入）
    pub anchor_samples: Vec<String>,
    /// N-gram 白名单 — 高频搭配（用于生成时优先使用）
    pub ngram_whitelist: NgramWhitelist,
}

pub enum FingerprintSource {
    Internal { story_id: String, scene_id: String }, // 用户故事前文
    External { text_preview: String },               // 用户提供的外部文本
}
```

### 3.2 指纹提取流程（统一，不分场景）

```
参考文本（任意来源）
    │
    ├─ 文本预处理（清洗、分句、分词）
    │
    ├─ 统计层（复用 Anti-AI Review 基础设施）
    │   ├─ 句长分布（均值、标准差、短/中/长句占比）
    │   ├─ 词汇频率（TOP50 实词、TOP20 虚词、四字格密度）
    │   ├─ 标点密度（逗号、句号、引号频率）
    │   ├─ 对话标签模式（"道" vs "说" vs "问" 占比）
    │   └─ 修辞密度（比喻、排比、对偶频率）
    │
    ├─ 结构层
    │   ├─ 段落平均长度
    │   ├─ 对话/叙述比例
    │   └─ 环境描写密度
    │
    ├─ 锚点采样（少样本学习用）
    │   └─ 按风格强度排序，取最典型 3-5 段（每段 50-100 字）
    │
    └─ N-gram 提取（白名单用）
        ├─ 高频双字搭配 TOP30
        ├─ 高频四字词 TOP20
        └─ 高频衔接模式 TOP15
```

### 3.3 指纹注入 prompt 格式

Writer 系统 prompt 中新增「风格指纹」段：

```
【风格指纹 — 基于参考文本的量化分析】
以下数据精确描述了参考文本的语言风格特征，续写时必须严格遵循：

【句法特征】
- 平均句长: 18.2±4.1 字（你的续写必须保持此分布，±30% 以内）
- 短句占比(<10字): 15% | 中句(10-25字): 55% | 长句(>25字): 30%
- 逗号密度: 每百字 8.2 个

【词汇偏好】
- 高频虚词（优先使用）: 原来(12次)、且说(8次)、正是(7次)、不想(6次)、因问(5次)
- 四字格密度: 12.3%（每百字至少 12 个四字结构）
- 时代感: 古典白话（禁用"但是""所以""然后"等现代虚词，改用"只是""故""随后"）

【对话标签模式】
- 主要标签: "道"(80%)、"问道"(10%)、"答道"(10%)
- 禁用: "说""告诉""问道"（除非原文风格如此）

【高频搭配白名单】（生成时优先使用）
- 双字: 一面...一面、连忙、只得、偏又、况又
- 四字: 花柳繁华、温柔富贵、风流袅娜、标致非常
- 衔接: 原来...、不想...、且说...、正是...

【锚点片段示例】（参考以下段落的语感、节奏和用词习惯）
[片段1] 黛玉道："要来一群，岂不热闹？"宝玉道："什么大家..."
[片段2] 贾母听说，便笑道："你今日来得正好..."
[片段3] 王夫人因说："你舅舅今日斋戒去了..."

【叙事约束】（来自故事上下文）
- 世界观: {world_setting}
- 角色: {character_list}
- 续写长度: {target_length} 字

重要：以上风格约束优先于叙事约束。如果情节推进需要使用现代词汇或长句，宁可放慢叙事节奏，也要保持语言风格一致。
```

---

## 四、统一续写流程

### 4.1 短续写（≤2000 字）

```
参考文本获取（内部/外部统一接口）
    │
    ▼
风格指纹提取（单次）
    │
    ▼
Writer Agent 生成（prompt 注入指纹 + 3 候选并行）
    │
    ▼
后处理替换层（虚词对齐）
    │
    ▼
Inspector 质检（含风格一致性评分）
    │
    ▼
风格分 ≥ 0.75 ? 直接输出 : 反馈改写
```

### 4.2 长续写（>2000 字）

```
参考文本获取
    │
    ▼
风格指纹提取（缓存，跨段复用）
    │
    ▼
分段生成（每段 800-1200 字）
    │
    ├─ 段 1: Writer → 后处理 → StyleChecker（与指纹对比）
    ├─ 段 2: Writer → 后处理 → StyleChecker（与指纹 + 段1 对比）
    ├─ 段 3: Writer → 后处理 → StyleChecker（与指纹 + 段1+2 对比）
    │   ...
    │
    ▼
跨段一致性校验（防止段 3 突然变现代白话）
    │
    ▼
Inspector 终检（整体质检 + 风格评分）
```

**关键**：长续写时，每段的参考文本 = 原始指纹 + 已生成的前几段（作为动态上下文），防止风格漂移累积。

---

## 五、Inspector 风格一致性维度

在现有 5 维质检基础上增加第 6 维：

```
6. 风格一致性（0-100 分）

评分细则：
- 句长分布偏离度（25分）
  对比参考文本的句长均值和标准差
  偏离 <10%: 25分 | 偏离 10-30%: 15分 | 偏离 >30%: 5分

- 词汇偏好匹配度（25分）
  标志性词汇出现频率对比
  匹配 >80%: 25分 | 匹配 50-80%: 15分 | 匹配 <50%: 5分

- 虚词使用模式（15分）
  "道"vs"说"、"原来"vs"但是"等关键虚词的偏好一致性

- 四字格密度（15分）
  四字结构占比是否匹配参考文本

- 整体语感（20分）
  Inspector 对"这段文字读起来像不像参考文本"的综合判断

输出格式：
{
  "score": 82,
  "dimension_scores": {
    "sentence_length": 20,
    "vocabulary": 22,
    "function_words": 12,
    "four_char": 13,
    "overall": 15
  },
  "suggestions": [
    "句长偏长：平均 25 字 vs 参考 18 字，建议多用短句",
    "虚词漂移：使用了 3 次'但是'，参考文本用'只是'",
    "四字格不足：当前 8% vs 参考 12%，建议增加"
  ]
}
```

---

## 六、Orchestrator 双轨平衡

```rust
pub struct WorkflowConfig {
    pub rewrite_threshold: f32,      // 质检阈值（现有）
    pub max_feedback_loops: u32,     // 最大循环（现有）
    pub style_weight: f32,           // 新增：风格权重（0-1）
    pub narrative_weight: f32,       // 新增：叙事权重（0-1）
}

// 平衡逻辑
fn evaluate(workflow_result: &WorkflowResult) -> Action {
    let style_ok = workflow_result.style_score >= 0.75;
    let narrative_ok = workflow_result.narrative_score >= 0.70;
    
    match (style_ok, narrative_ok) {
        (true, true) => Action::Accept,           // 双达标，直接通过
        (true, false) => Action::RewriteNarrative, // 风格好但叙事弱，改写叙事
        (false, true) => Action::RewriteStyle,     // 叙事好但风格弱，改写风格
        (false, false) => Action::RewriteBoth,     // 都不达标，全面改写
    }
}
```

**用户可调节**：Settings 中新增「风格-叙事平衡滑块」
- 0 = 纯风格优先（宁要风格完美的平淡叙事）
- 50 = 平衡（默认）
- 100 = 纯叙事优先（宁要精彩情节的风格漂移）

---

## 七、参考文本来源的统一处理

### 内部来源（默认）

```rust
fn get_reference_text_internal(story_id: &str, scene_id: &str) -> String {
    // 从数据库读取当前场景最新内容
    let scene = scene_repo.get_by_id(scene_id);
    // 取最后 3000-5000 字作为参考文本
    scene.content.chars().rev().take(5000).collect()
}
```

### 外部来源（用户主动更换）

前端在 WenSiPanel 中新增「更换参考文本」按钮：
- 用户粘贴任意文本（最多 5000 字）
- 系统保存为临时风格指纹
- 后续续写使用该指纹，直到用户再次更换或切回「使用前文」

**不需要模式切换、不需要独立入口** — 只是参考文本的获取方式不同。

---

## 八、四优化在统一方案中的位置

| 优化 | 融入位置 | 说明 |
|------|---------|------|
| **少样本锚定** | 风格指纹的 `anchor_samples` | 每段生成时注入 3-5 段锚点 |
| **N-gram 白名单** | 风格指纹的 `ngram_whitelist` | prompt 中列出优先搭配 |
| **后处理替换** | 生成后的 `align_style()` | 轻量虚词替换，不改变语义 |
| **3 候选选优** | Writer 生成阶段 | 短续写 3 候选，长续写每段 2 候选 |

---

## 九、实施计划

### P0 — 核心（风格指纹注入）

| 文件 | 改动 | 工时 |
|------|------|------|
| `creative_engine/style/fingerprint.rs` **新增** | 风格指纹数据结构 + 提取算法 | 3h |
| `prompts/engine.rs` | Writer 系统 prompt 增加指纹注入段 | 1h |
| `agents/commands.rs` | auto_write / writer_agent_execute 中调用指纹提取 | 1h |

### P1 — 闭环（Inspector + Orchestrator）

| 文件 | 改动 | 工时 |
|------|------|------|
| `prompts/engine.rs` | Inspector 系统 prompt 增加风格一致性维度 | 1h |
| `agents/orchestrator.rs` | 新增 style_score 字段 + 双轨平衡逻辑 | 2h |

### P2 — 增强（四优化）

| 文件 | 改动 | 工时 |
|------|------|------|
| `creative_engine/style/fingerprint.rs` | 锚点采样 + N-gram 提取 | 1h |
| `utils/style_align.rs` **新增** | 后处理替换层 | 1h |
| `agents/orchestrator.rs` | 3 候选并行生成 | 1h |

### P3 — 前端（最小改动）

| 文件 | 改动 | 工时 |
|------|------|------|
| `WenSiPanel.tsx` | 增加「更换参考文本」按钮 + 风格-叙事滑块 | 1h |

**总计**：~12 小时，1 个新增 module（`style/fingerprint.rs`），其余全部在现有组件内增强。

---

## 十、验收标准

| 测试项 | 通过标准 |
|--------|---------|
| 红楼梦 500 字续写 | 3 位非专业读者 ≥2 人认为"风格像"，StyleChecker 风格分 ≥ 0.75 |
| 网文 2000 字续写 | 与原文风格分差异 < 0.15（跨段一致性） |
|  Inspector 风格评分 | 能准确指出"句长偏长 +23%""虚词漂移：用了'但是'而非'只是'"等具体问题 |
| 滑块调节 | 调至"风格优先"时，输出句长分布与参考文本偏离 < 10% |
| 外部参考文本 | 粘贴任意 1000 字文本后，续写 500 字，风格分 ≥ 0.70 |

---

**等待审批。**
