//! Intent Parser - 意图解析引擎
//!
//! 将创作者的自然语言输入解析为结构化意图，
//! 驱动 workflow::scheduler 调用正确的 Agent 执行创作任务。

use std::{collections::HashMap, sync::Mutex, time::Duration};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use uuid::Uuid;

use crate::{
    agents::{
        service::{AgentService, AgentTask, AgentType},
        AgentContext, AgentResult,
    },
    creative_engine::asset_capability_manifest::AssetTaskType,
    llm::{GenerateResponse, LlmService},
    router::TaskType,
};

/// 意图类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    TextGenerate,
    TextRewrite,
    PlotSuggest,
    CharacterCheck,
    WorldConsistency,
    StyleShift,
    MemoryIngest,
    VisualGenerate,
    SceneReorder,
    OutlineExpand,
    Unknown,
}

/// 执行模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Serial,
    Parallel,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Serial
    }
}

/// 反馈类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    DirectApply,
    SuggestionCard,
    DiffPreview,
    SystemNotice,
    VisualHighlight,
}

impl Default for FeedbackType {
    fn default() -> Self {
        FeedbackType::SuggestionCard
    }
}

/// 意图目标
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntentTarget {
    pub target_type: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
}

/// 结构化意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    #[serde(rename = "intent_type")]
    pub intent_type: IntentType,
    #[serde(default)]
    pub target: IntentTarget,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub required_agents: Vec<String>,
    #[serde(default)]
    pub execution_mode: ExecutionMode,
    #[serde(default)]
    pub feedback_type: FeedbackType,
    /// 原始用户输入（补充字段，不由LLM生成）
    #[serde(skip)]
    pub raw_input: String,
}

impl Intent {
    pub fn unknown(raw_input: impl Into<String>) -> Self {
        Self {
            intent_type: IntentType::Unknown,
            target: IntentTarget::default(),
            constraints: vec![],
            required_agents: vec![],
            execution_mode: ExecutionMode::default(),
            feedback_type: FeedbackType::default(),
            raw_input: raw_input.into(),
        }
    }
}

/// v0.30.11: 写作意图路由分类结果。
///
/// 一次轻量 LLM 调用产出的全部路由决策，替代散落各处的朴素子串匹配
/// （`is_novel_creation_intent` / `from_instruction_and_context` /
/// force-correction 的 `prose_keywords` / `detect_input_clarity` 等）。由
/// [`IntentParser::classify_writing_intent`] 产出，经 `PlanContext` 贯穿管线。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WritingIntentClassification {
    /// 是否创建新小说（genesis vs planner 主分叉）。
    /// 误判代价不对称：误判续写为创世会启动 Agency
    /// 全流程并新建故事、覆盖工作（灾难）， 故 LLM 失败兜底为
    /// false（continuation，planner 仍能产出首章）。
    #[serde(default)]
    pub is_new_novel: bool,
    /// 是否续写已有内容（模板跳过 / 状态文案）。
    #[serde(default)]
    pub is_continuation: bool,
    /// 资产清单过滤的任务类型（驱动 `AssetTaskType`）。
    #[serde(default)]
    pub task_type: AssetTaskType,
    /// 是否散文生成请求（planner force-correction 用）。
    /// v0.30.11: alias "is_prose" 对齐 prompt 指示 LLM 返回的字段名。
    #[serde(default, alias = "is_prose")]
    pub is_prose_request: bool,
    /// 输入清晰度（四元组补全用）。
    #[serde(default)]
    pub input_clarity: InputClarity,
    /// 从输入识别的题材（武侠/玄幻/都市等），无法识别为 None。
    #[serde(default)]
    pub detected_genre: Option<String>,
    /// LLM 自评置信度 0.0-1.0；<0.5 时调用方可选用兜底。
    #[serde(default)]
    pub confidence: f32,
}

impl WritingIntentClassification {
    /// 保守兜底：LLM 失败/超时时使用。默认续写（安全降级）。
    ///
    /// - `is_new_novel=false`：避免误启动 Agency 创世覆盖已有工作；
    /// - `is_prose_request=true`：force-correction 仍保 force-to-writer
    ///   安全默认；
    /// - `task_type=Continuation`：注入最贴合正文生成的资产集；
    /// - `input_clarity=Vague`：触发四元组补全（多注入比少注入安全）。
    #[deprecated(note = "v0.30.23: 改用 conservative_fallback_with_context（无故事时创世）")]
    pub fn conservative_fallback() -> Self {
        Self {
            is_new_novel: false,
            is_continuation: true,
            task_type: AssetTaskType::Continuation,
            is_prose_request: true,
            input_clarity: InputClarity::Vague,
            detected_genre: None,
            confidence: 0.0,
        }
    }

    /// v0.30.23: 上下文感知兜底。LLM 失败/超时时使用。
    ///
    /// - `has_existing_story=false`：无故事不可能续写，返回创世
    ///   （`is_new_novel=true, task_type=Genesis`）。这是 DB 状态推断
    ///   而非关键词匹配--LLM 没给出结果时，"无故事不可能续写"是 合理逻辑推断。
    /// - `has_existing_story=true`：有故事时偏续写（与原
    ///   [`conservative_fallback`] 同语义），避免误启动 Agency 覆盖工作。
    pub fn conservative_fallback_with_context(has_existing_story: bool) -> Self {
        if !has_existing_story {
            Self {
                is_new_novel: true,
                is_continuation: false,
                task_type: AssetTaskType::Genesis,
                is_prose_request: true,
                input_clarity: InputClarity::Vague,
                detected_genre: None,
                confidence: 0.0,
            }
        } else {
            #[allow(deprecated)]
            Self::conservative_fallback()
        }
    }
}

/// 会话级分类缓存：按 user_input 哈希（v0.30.23: 提示词不再注入上下文，
/// 故缓存键仅按输入文本）。 重复输入（如"继续写"）二次命中即时返回，
/// 避免每次生成都付 LLM 往返。仅缓存 LLM 成功结果，不缓存兜底。
static CLASSIFICATION_CACHE: Lazy<Mutex<HashMap<String, WritingIntentClassification>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn classification_cache_get(key: &str) -> Option<WritingIntentClassification> {
    CLASSIFICATION_CACHE.lock().ok()?.get(key).cloned()
}

fn classification_cache_put(key: String, val: WritingIntentClassification) {
    if let Ok(mut cache) = CLASSIFICATION_CACHE.lock() {
        // 简单容量控制：超 64 条时清掉旧条目保留最近 32 条，防内存膨胀。
        if cache.len() > 64 {
            let keep: Vec<_> = cache.drain().take(32).collect();
            cache.extend(keep);
        }
        cache.insert(key, val);
    }
}

/// 意图解析器
pub struct IntentParser {
    llm_service: LlmService,
}

impl IntentParser {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            llm_service: LlmService::new(app_handle),
        }
    }

    /// 解析用户输入为结构化意图
    pub async fn parse(&self, user_input: &str) -> Result<Intent, String> {
        let prompt = Self::build_intent_prompt(user_input);

        match self
            .llm_service
            .generate_for_task(
                TaskType::Analysis,
                prompt,
                Some(512),
                Some(0.1),
                Some("intent_parse"),
            )
            .await
        {
            Ok(GenerateResponse { content, .. }) => Self::parse_intent_json(&content, user_input),
            Err(e) => {
                log::error!("[IntentParser] LLM generation failed: {}", e);
                Ok(Intent::unknown(user_input))
            }
        }
    }

    fn build_intent_prompt(user_input: &str) -> String {
        format!(
            r#"你是一个专业的创作助手意图解析器。请将用户的输入解析为固定的 JSON 格式。

可识别的意图类型 (intent_type):
- text_generate: 文本续写、扩展内容、从头开始创作新内容。用户使用"写"、"创作"、"生成"、"续"、"扩"、"补"、"开篇"、"开头"等词时，必须识别为 text_generate
- text_rewrite: 改写、润色已有文本
- plot_suggest: 情节建议、反转设计、剧情推进
- character_check: 角色一致性检查、角色动机分析
- world_consistency: 世界设定一致性检查
- style_shift: 文风切换、文风模仿
- memory_ingest: 知识摄取、更新记忆
- visual_generate: 生成图像、概念图
- scene_reorder: 场景结构调整、排序
- outline_expand: 大纲扩展（仅在用户明确要求扩展大纲时使用，不要与 text_generate 混淆）
- unknown: 无法识别或闲聊

执行模式 (execution_mode):
- serial: 串行执行（默认）
- parallel: 并行执行

反馈类型 (feedback_type):
- direct_apply: 直接修改（适用于续写、创作）
- suggestion_card: 建议卡片（适用于情节建议）
- diff_preview: Diff预览（适用于改写）
- system_notice: 系统通知（适用于异步任务）
- visual_highlight: 可视化高亮（适用于检查结果）

可用 Agent (required_agents):
- writer: 写作助手，用于 text_generate/text_rewrite
- style_mimic: 风格模仿师
- plot_analyzer: 情节分析师
- outline_planner: 大纲规划师
- character_agent: 角色分析 Agent
- world_building_agent: 世界观 Agent
- memory_agent: 记忆 Agent
- inspector: 质检员

关键规则:
1. 必须且只能返回合法的 JSON，不要包含 markdown 代码块标记。
2. 用户说"写一篇..."、"创作一个..."、"生成..."等明确请求生成文字内容时，intent_type 必须是 text_generate，required_agents 必须包含 writer，feedback_type 必须是 direct_apply。
3. 用户说"帮我想个..."、"给点建议"等请求思路时，intent_type 是 plot_suggest，feedback_type 是 suggestion_card。
4. target 字段用于指明操作对象，如场景、角色等。target_type 可选值: scene, character, story, paragraph。
5. constraints 是用户对结果的具体约束条件列表。
6. 如果用户只是打招呼或闲聊，返回 intent_type: unknown。

JSON Schema:
{{
  "intent_type": "string",
  "target": {{
    "target_type": "string | null",
    "id": "string | null",
    "name": "string | null"
  }},
  "constraints": ["string"],
  "required_agents": ["string"],
  "execution_mode": "serial | parallel",
  "feedback_type": "direct_apply | suggestion_card | diff_preview | system_notice | visual_highlight"
}}

用户输入: "{}"

请直接输出 JSON:"#,
            user_input
        )
    }

    fn parse_intent_json(content: &str, user_input: &str) -> Result<Intent, String> {
        // 尝试清理可能存在的 markdown 代码块
        let json_str = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<Intent>(json_str) {
            Ok(mut intent) => {
                intent.raw_input = user_input.to_string();
                Ok(intent)
            }
            Err(e) => {
                log::warn!(
                    "[IntentParser] Failed to parse JSON: {}. Raw content: {}",
                    e,
                    content
                );
                Ok(Intent::unknown(user_input))
            }
        }
    }

    /// v0.30.11: 轻量写作意图路由分类--一次 LLM 调用产出全部路由决策。
    ///
    /// 替代散落各处的朴素子串匹配（`is_novel_creation_intent` 关键词列表、
    /// `from_instruction_and_context` 单字 contains、force-correction 的
    /// `prose_keywords`、`detect_input_clarity` 信号计数）。AI 应用应让 LLM
    /// 做语义理解，而非用 `str::contains` 推断意图。
    ///
    /// 设计要点：
    /// - **最快模型层 + max_tokens=256 + temp=0**：远程模型 ~1s；
    /// - **8s 超时**：本地慢模型最坏 +8s，超时降级为
    ///   [`conservative_fallback_with_context`]；
    /// - **会话缓存**：相同输入二次命中即时返回（v0.30.23: 缓存键 仅按
    ///   user_input，提示词不再使用上下文故结果不随上下文变化）；
    /// - **一次调用多决策**：is_new_novel / task_type / is_prose /
    ///   input_clarity / detected_genre 全在一次调用产出，避免多次 LLM；
    /// - **v0.30.23 去偏**：提示词不再注入 has_existing_story /
    ///   has_current_content（此前"已有故事=true"使 LLM 倾向续写）。
    ///   参数保留仅供兜底使用。
    /// - **v0.30.23 不缓存失败**：仅 LLM 成功解析的结果写入缓存，
    ///   兜底结果不缓存（临时超时/网络问题不应让错误分类持续存在）。
    pub async fn classify_writing_intent(
        &self,
        user_input: &str,
        has_existing_story: bool,
        has_current_content: bool,
    ) -> WritingIntentClassification {
        // v0.30.23: 缓存键仅按 user_input（提示词不再使用上下文，
        // 同输入的 LLM 结果不随上下文变化）。
        let cache_key = user_input.trim().to_string();
        if let Some(cached) = classification_cache_get(&cache_key) {
            log::debug!("[IntentParser] classify_writing_intent cache hit");
            return cached;
        }

        let prompt =
            Self::build_classification_prompt(user_input, has_existing_story, has_current_content);
        let labelled = self.llm_service.generate_for_task(
            TaskType::Analysis,
            prompt,
            Some(256),
            Some(0.0),
            Some("intent_classify"),
        );
        // v0.30.23: is_fallback 标记--仅成功 LLM 结果写入缓存。
        let (classification, is_fallback) =
            match tokio::time::timeout(Duration::from_secs(8), labelled).await {
                Ok(Ok(GenerateResponse { content, .. })) => {
                    match Self::parse_classification_json(&content) {
                        Some(c) => (c, false),
                        None => {
                            log::warn!(
                            "[IntentParser] classify_writing_intent JSON 解析失败，兜底。raw: {}",
                            &content[..content.len().min(200)]
                        );
                            (
                                WritingIntentClassification::conservative_fallback_with_context(
                                    has_existing_story,
                                ),
                                true,
                            )
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::warn!(
                        "[IntentParser] classify_writing_intent LLM 失败，兜底: {}",
                        e
                    );
                    (
                        WritingIntentClassification::conservative_fallback_with_context(
                            has_existing_story,
                        ),
                        true,
                    )
                }
                Err(_) => {
                    log::warn!("[IntentParser] classify_writing_intent 8s 超时，兜底");
                    (
                        WritingIntentClassification::conservative_fallback_with_context(
                            has_existing_story,
                        ),
                        true,
                    )
                }
            };
        // v0.30.23: 仅缓存成功的 LLM 分类，不缓存兜底结果。
        if !is_fallback {
            classification_cache_put(cache_key, classification.clone());
        }
        classification
    }

    fn build_classification_prompt(
        user_input: &str,
        _has_existing_story: bool,
        _has_current_content: bool,
    ) -> String {
        // v0.30.23: 提示词去偏--不再注入"已有故事/已有正文"上下文。
        // LLM 应基于用户输入本身的表达判定意图，而非被 DB 状态偏差
        // （此前"已有故事=true"使 LLM 倾向续写，导致"写一部X小说"被误分类）。
        // has_existing_story/has_current_content 参数保留（兜底用），但不进入提示词。
        format!(
            r#"判定用户创作意图，仅输出 JSON。

用户输入：{input}

判定规则：
- is_new_novel: 用户想从头创作一部新小说。"写一部/写一本/创作一部/新开一部"等创世表达均为 true。续写/改写/分析/闲聊均为 false。
  注意：判断依据是用户输入本身的表达，与是否已有故事无关。即使已有故事，用户仍可创建新小说。
- is_continuation: 用户想接着已有内容往下写（续写/继续/接着写）。
- task_type: continuation（续写正文）/ rewrite（改写润色已有文本）/ genesis（创世/新小说/新场景）/ audit（检查/质检/分析）
- is_prose: 用户想生成小说正文（续写/创作首章），而非大纲/风格/分析。prose 请求必须用 writer。
- input_clarity: vague（仅题材或笼统）/ with_seed（含部分角色或冲突）/ with_full_concept（角色+冲突+目标齐全）
- detected_genre: 识别的题材（武侠/玄幻/都市/科幻/重生/穿越/历史/军事/言情/间谍等），无法识别为 null。
- confidence: 0.0-1.0。

示例：
- "写一部科幻小说" -> is_new_novel=true, task_type=genesis, is_prose=true
- "继续写" -> is_new_novel=false, is_continuation=true, task_type=continuation
- "把这段改得更生动" -> is_new_novel=false, task_type=rewrite, is_prose=false

仅输出 JSON：
{{"is_new_novel":bool,"is_continuation":bool,"task_type":"continuation|rewrite|genesis|audit","is_prose":bool,"input_clarity":"vague|with_seed|with_full_concept","detected_genre":string|null,"confidence":0.0}}"#,
            input = user_input,
        )
    }

    /// 宽容解析分类 JSON：剥 markdown 代码块 + 截取首 '{' 到末 '}'。
    /// 本地模型常在 JSON 前后缀散文，需容忍。不依赖 agency::parse_lenient
    /// （agency 不允许被反向依赖）。
    fn parse_classification_json(content: &str) -> Option<WritingIntentClassification> {
        let trimmed = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        let start = trimmed.find('{')?;
        let end = trimmed.rfind('}')?;
        if end <= start {
            return None;
        }
        let slice = &trimmed[start..=end];
        match serde_json::from_str::<WritingIntentClassification>(slice) {
            Ok(mut c) => {
                // 合理性校验：confidence 钳到 [0,1]；空字符串题材归 None。
                if !(0.0..=1.0).contains(&c.confidence) {
                    c.confidence = c.confidence.clamp(0.0, 1.0);
                }
                if c.detected_genre
                    .as_deref()
                    .map(str::is_empty)
                    .unwrap_or(false)
                {
                    c.detected_genre = None;
                }
                Some(c)
            }
            Err(e) => {
                log::debug!(
                    "[IntentParser] classification JSON parse failed: {}. slice: {}",
                    e,
                    &slice[..slice.len().min(200)]
                );
                None
            }
        }
    }
}

/// Agent 执行步骤结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStepResult {
    pub agent_name: String,
    pub success: bool,
    pub result: Option<AgentResult>,
    pub error: Option<String>,
}

/// 意图执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentExecutionResult {
    pub intent_type: IntentType,
    pub feedback_type: FeedbackType,
    pub execution_mode: ExecutionMode,
    pub steps: Vec<AgentStepResult>,
    pub summary: String,
}

/// 输入清晰度（v0.17.1 智能后台预访谈）
///
/// 用于决定 StrategySelector 是否在后台静默补全四元组
/// （主情绪 / 高压关系 / 冲突场 / 剧情引擎 / 桥段卡）。
/// **不弹选项卡打扰用户**——完全后台决定，对前端透明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InputClarity {
    /// 模糊输入：仅含题材或非常笼统的指令（"写一篇修仙打脸"/"我要写小说"/"续写"
    /// ） -> 后端透明补全四元组并注入 Writer prompt
    #[default]
    Vague,
    /// 含部分故事元素：题材 + 主角设定 / 题材 + 钩子 / 题材 + 情境
    /// -> 后端补全 1-2 项缺失维度
    WithSeed,
    /// 含完整故事概念：明确角色、冲突、目标、场景
    /// → 后端不主动追加，尊重用户已有意图
    WithFullConcept,
}

impl InputClarity {
    pub fn as_str(&self) -> &'static str {
        match self {
            InputClarity::Vague => "vague",
            InputClarity::WithSeed => "with_seed",
            InputClarity::WithFullConcept => "with_full_concept",
        }
    }

    /// 是否需要后端透明补全四元组
    pub fn needs_quartet_inference(&self) -> bool {
        matches!(self, InputClarity::Vague | InputClarity::WithSeed)
    }
}

/// 启发式输入清晰度检测（v0.17.1 智能后台预访谈）
///
/// v0.30.11: smart_execute 入口已先调 LLM
/// 分类器（`classify_writing_intent`）产出 `input_clarity`，
/// 调用方应优先读分类结果；本函数仅作无分类时的字面兜底。 三档判定：
/// - 长度 < 8 字符 或 仅含题材关键词 -> Vague
/// - 含 1-2 个故事元素（角色/动作/冲突词）-> WithSeed
/// - 含 ≥3 个故事元素 -> WithFullConcept
///
/// v0.30.11: 移除单字信号（"他/她/我/杀/比" 等会命中任意中文文本，噪声严重），
/// 仅保留 ≥2 字的多字信号。
#[allow(dead_code)] // 保留为无分类时的字面兜底；smart_execute 入口已用 LLM 分类
pub fn detect_input_clarity(user_input: &str) -> InputClarity {
    let trimmed = user_input.trim();
    if trimmed.is_empty() {
        return InputClarity::Vague;
    }

    // 字数（按 Unicode scalar 近似中文字数）
    let char_count = trimmed.chars().count();
    if char_count < 8 {
        return InputClarity::Vague;
    }

    // 故事元素信号词（出现 ≥3 个 = WithFullConcept；1-2 个 = WithSeed；0 个 =
    // Vague）。v0.30.11: 仅多字 pattern，单字（"他/她/杀/比"…）会命中任意文本。
    const ELEMENT_SIGNALS: &[&str] = &[
        // 角色信号
        "主角", "弟子", "少年", "少女", "青年", "女主", "男主", // 动作/冲突信号
        "复仇", "逆袭", "陷害", "背叛", "误会", "争夺", "羞辱", "证明", "揭露", "对决",
        // 场景信号
        "拍卖", "庭审", "大比", "婚礼", "公开", "禁地", "宗门", "家宴", // 关系信号
        "师父", "师兄", "师姐", "敌人", "恋人", "前夫", "前妻", "继承", // 目标信号
        "为了", "想要", "决定", "立志", "誓言", "目标", "重生", "归来",
    ];
    let mut signal_count = 0;
    for signal in ELEMENT_SIGNALS {
        if trimmed.contains(signal) {
            signal_count += 1;
            if signal_count >= 3 {
                return InputClarity::WithFullConcept;
            }
        }
    }

    if signal_count == 0 {
        InputClarity::Vague
    } else {
        InputClarity::WithSeed
    }
}

/// 意图执行器 - 将解析后的意图调度到具体 Agent 执行
pub struct IntentExecutor {
    agent_service: AgentService,
}

impl IntentExecutor {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            agent_service: AgentService::new(app_handle),
        }
    }

    /// 执行意图对应的 Agent 任务
    pub async fn execute(
        &self,
        intent: Intent,
        story_id: String,
    ) -> Result<IntentExecutionResult, String> {
        let agents = Self::map_agents(&intent.required_agents);

        if agents.is_empty() {
            return Ok(IntentExecutionResult {
                intent_type: intent.intent_type.clone(),
                feedback_type: intent.feedback_type.clone(),
                execution_mode: intent.execution_mode.clone(),
                steps: vec![],
                summary: "暂无可执行的相关 Agent，已回退到对话模式。".to_string(),
            });
        }

        let context =
            Self::build_context(&story_id, &intent, self.agent_service.app_handle()).await;
        let steps = match intent.execution_mode {
            ExecutionMode::Serial => self.execute_serial(agents, context, &intent).await,
            ExecutionMode::Parallel => self.execute_parallel(agents, context, &intent).await,
        };

        let summary = Self::build_summary(&intent, &steps);

        Ok(IntentExecutionResult {
            intent_type: intent.intent_type,
            feedback_type: intent.feedback_type,
            execution_mode: intent.execution_mode,
            steps,
            summary,
        })
    }

    /// 将 agent 名称字符串映射到 AgentType
    fn map_agents(agent_names: &[String]) -> Vec<AgentType> {
        agent_names
            .iter()
            .filter_map(|name| match name.as_str() {
                "writer" => Some(AgentType::Writer),
                "style_mimic" => Some(AgentType::StyleMimic),
                "plot_analyzer" => Some(AgentType::PlotAnalyzer),
                "outline_planner" => Some(AgentType::OutlinePlanner),
                "inspector" => Some(AgentType::Inspector),
                // 以下 agent 尚未实现独立类型，暂时映射到最接近的实现
                "character_agent" => Some(AgentType::Inspector),
                "world_building_agent" => Some(AgentType::Inspector),
                "memory_agent" => Some(AgentType::Writer),
                _ => None,
            })
            .collect()
    }

    /// 构建 Agent 执行上下文
    ///
    /// 使用 StoryContextBuilder 从数据库读取真实故事数据，
    /// 替代原有的硬编码默认值。
    async fn build_context(
        story_id: &str,
        _intent: &Intent,
        app_handle: &tauri::AppHandle,
    ) -> AgentContext {
        use tauri::Manager;

        use crate::{creative_engine::StoryContextBuilder, db::DbPool};

        match app_handle.try_state::<DbPool>() {
            Some(pool_state) => {
                let pool = pool_state.inner().clone();
                let builder = StoryContextBuilder::new(pool);
                match builder.build_quick(story_id).await {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        log::warn!(
                            "[IntentExecutor] Failed to build context from DB: {}, falling back \
                             to minimal",
                            e
                        );
                        AgentContext::minimal(story_id.to_string(), String::new())
                    }
                }
            }
            None => {
                log::warn!("[IntentExecutor] DbPool not available, using minimal context");
                AgentContext::minimal(story_id.to_string(), String::new())
            }
        }
    }

    /// 串行执行
    async fn execute_serial(
        &self,
        agents: Vec<AgentType>,
        context: AgentContext,
        intent: &Intent,
    ) -> Vec<AgentStepResult> {
        let mut steps = Vec::new();
        let mut current_input = intent.raw_input.clone();

        for agent in agents {
            let task = AgentTask {
                id: Uuid::new_v4().to_string(),
                agent_type: agent,
                context: context.clone(),
                input: current_input.clone(),
                parameters: Self::build_parameters(intent),
                tier: None,
            };

            match self.agent_service.execute_task(task).await {
                Ok(result) => {
                    current_input = result.content.clone();
                    steps.push(AgentStepResult {
                        agent_name: agent.name().to_string(),
                        success: true,
                        result: Some(result),
                        error: None,
                    });
                }
                Err(e) => {
                    steps.push(AgentStepResult {
                        agent_name: agent.name().to_string(),
                        success: false,
                        result: None,
                        error: Some(e.to_string()),
                    });
                    // 串行模式下遇到错误可选择中断，这里继续记录但停止传递输入
                    break;
                }
            }
        }

        steps
    }

    /// 并行执行
    async fn execute_parallel(
        &self,
        agents: Vec<AgentType>,
        context: AgentContext,
        intent: &Intent,
    ) -> Vec<AgentStepResult> {
        let mut handles = Vec::new();
        let service = self.agent_service.clone();

        for agent in agents {
            let task = AgentTask {
                id: Uuid::new_v4().to_string(),
                agent_type: agent,
                context: context.clone(),
                input: intent.raw_input.clone(),
                parameters: Self::build_parameters(intent),
                tier: None,
            };

            let service_clone = service.clone();
            let handle = tokio::spawn(async move {
                match service_clone.execute_task(task).await {
                    Ok(result) => AgentStepResult {
                        agent_name: agent.name().to_string(),
                        success: true,
                        result: Some(result),
                        error: None,
                    },
                    Err(e) => AgentStepResult {
                        agent_name: agent.name().to_string(),
                        success: false,
                        result: None,
                        error: Some(e.to_string()),
                    },
                }
            });
            handles.push(handle);
        }

        let mut steps = Vec::new();
        for handle in handles {
            if let Ok(step) = handle.await {
                steps.push(step);
            }
        }

        steps
    }

    /// 构建额外参数
    fn build_parameters(intent: &Intent) -> HashMap<String, serde_json::Value> {
        let mut params = HashMap::new();
        if let Some(target_type) = &intent.target.target_type {
            params.insert("target_type".to_string(), serde_json::json!(target_type));
        }
        if let Some(target_id) = &intent.target.id {
            params.insert("target_id".to_string(), serde_json::json!(target_id));
        }
        if let Some(target_name) = &intent.target.name {
            params.insert("target_name".to_string(), serde_json::json!(target_name));
        }
        if !intent.constraints.is_empty() {
            params.insert(
                "constraints".to_string(),
                serde_json::json!(intent.constraints),
            );
        }
        params
    }

    /// 构建执行结果摘要
    /// 优先返回最后一个成功 Agent 的实际生成内容，让用户看到有用的结果
    fn build_summary(intent: &Intent, steps: &[AgentStepResult]) -> String {
        let success_count = steps.iter().filter(|s| s.success).count();
        let total_count = steps.len();

        if total_count == 0 {
            return "未执行任何 Agent 任务。".to_string();
        }

        // 优先返回最后一个成功 Agent 的实际内容
        if let Some(last_success) = steps.iter().rev().find(|s| s.success) {
            if let Some(ref result) = last_success.result {
                if !result.content.is_empty() {
                    return result.content.clone();
                }
            }
        }

        // 回退到状态摘要
        if success_count == total_count {
            format!(
                "{} 意图已完全执行，共调用 {} 个 Agent。",
                Self::intent_display_name(&intent.intent_type),
                total_count
            )
        } else {
            format!(
                "{} 意图部分执行，成功 {}/{}。",
                Self::intent_display_name(&intent.intent_type),
                success_count,
                total_count
            )
        }
    }

    fn intent_display_name(intent_type: &IntentType) -> &'static str {
        match intent_type {
            IntentType::TextGenerate => "续写生成",
            IntentType::TextRewrite => "文本改写",
            IntentType::PlotSuggest => "情节建议",
            IntentType::CharacterCheck => "角色检查",
            IntentType::WorldConsistency => "世界观检查",
            IntentType::StyleShift => "文风切换",
            IntentType::MemoryIngest => "知识摄取",
            IntentType::VisualGenerate => "视觉生成",
            IntentType::SceneReorder => "场景调整",
            IntentType::OutlineExpand => "大纲扩展",
            IntentType::Unknown => "自由对话",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_intent_json() {
        let json = r#"{
            "intent_type": "text_rewrite",
            "target": {"target_type": "scene", "id": "scene_2", "name": null},
            "constraints": ["增强紧张感", "保持 K-7 语气"],
            "required_agents": ["writer", "style_mimic"],
            "execution_mode": "serial",
            "feedback_type": "diff_preview"
        }"#;

        let intent = IntentParser::parse_intent_json(json, "把 Scene 2 改得更紧张").unwrap();
        assert_eq!(intent.intent_type, IntentType::TextRewrite);
        assert_eq!(intent.target.target_type, Some("scene".to_string()));
        assert_eq!(intent.target.id, Some("scene_2".to_string()));
        assert_eq!(intent.constraints.len(), 2);
        assert_eq!(intent.required_agents, vec!["writer", "style_mimic"]);
        assert_eq!(intent.execution_mode, ExecutionMode::Serial);
        assert_eq!(intent.feedback_type, FeedbackType::DiffPreview);
    }

    #[test]
    fn test_parse_intent_json_with_markdown() {
        let json = "```json\n{\"intent_type\": \"plot_suggest\", \"target\": {}, \"constraints\": \
                    [], \"required_agents\": [\"plot_analyzer\"], \"execution_mode\": \"serial\", \
                    \"feedback_type\": \"suggestion_card\"}\n```";

        let intent = IntentParser::parse_intent_json(json, "帮我想个反转").unwrap();
        assert_eq!(intent.intent_type, IntentType::PlotSuggest);
    }

    #[test]
    fn test_parse_intent_json_fallback() {
        let invalid = "这不是 JSON";
        let intent = IntentParser::parse_intent_json(invalid, "你好").unwrap();
        assert_eq!(intent.intent_type, IntentType::Unknown);
    }

    #[test]
    fn test_parse_classification_json_normal() {
        let json = r#"{"is_new_novel":true,"is_continuation":false,"task_type":"genesis","is_prose":true,"input_clarity":"vague","detected_genre":"武侠","confidence":0.9}"#;
        let c = IntentParser::parse_classification_json(json).unwrap();
        assert!(c.is_new_novel);
        assert!(!c.is_continuation);
        assert_eq!(c.task_type, AssetTaskType::Genesis);
        assert!(c.is_prose_request);
        assert_eq!(c.input_clarity, InputClarity::Vague);
        assert_eq!(c.detected_genre.as_deref(), Some("武侠"));
        assert!((c.confidence - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_parse_classification_json_markdown_fence() {
        let json = "```json\n{\"is_new_novel\":false,\"is_continuation\":true,\"task_type\":\"continuation\",\"is_prose\":true,\"input_clarity\":\"with_seed\",\"detected_genre\":null,\"confidence\":0.8}\n```";
        let c = IntentParser::parse_classification_json(json).unwrap();
        assert!(!c.is_new_novel);
        assert!(c.is_continuation);
        assert_eq!(c.task_type, AssetTaskType::Continuation);
        assert_eq!(c.detected_genre, None);
    }

    #[test]
    fn test_parse_classification_json_lenient_prose_affix() {
        // 本地模型常在 JSON 前后加散文，需截取首 { 到末 }
        let raw = "好的，这是判定结果：{\"is_new_novel\":false,\"is_continuation\":true,\"task_type\":\"rewrite\",\"is_prose\":false,\"input_clarity\":\"with_full_concept\",\"detected_genre\":\"都市\",\"confidence\":0.7} 以上判定。";
        let c = IntentParser::parse_classification_json(raw).unwrap();
        assert_eq!(c.task_type, AssetTaskType::Rewrite);
        assert!(!c.is_prose_request);
        assert_eq!(c.input_clarity, InputClarity::WithFullConcept);
        assert_eq!(c.detected_genre.as_deref(), Some("都市"));
    }

    #[test]
    fn test_parse_classification_json_empty_genre_to_none() {
        let json = r#"{"is_new_novel":false,"is_continuation":true,"task_type":"continuation","is_prose":true,"input_clarity":"vague","detected_genre":"","confidence":0.5}"#;
        let c = IntentParser::parse_classification_json(json).unwrap();
        assert_eq!(c.detected_genre, None, "空字符串题材应归一为 None");
    }

    #[test]
    fn test_parse_classification_json_confidence_clamped() {
        let json = r#"{"is_new_novel":false,"is_continuation":true,"task_type":"continuation","is_prose":true,"input_clarity":"vague","detected_genre":null,"confidence":1.5}"#;
        let c = IntentParser::parse_classification_json(json).unwrap();
        assert_eq!(c.confidence, 1.0, "confidence 应钳到 [0,1]");
    }

    #[test]
    fn test_parse_classification_json_invalid_returns_none() {
        assert!(IntentParser::parse_classification_json("这不是 JSON").is_none());
        assert!(IntentParser::parse_classification_json("").is_none());
        // 只有 { 没有 } -> None
        assert!(IntentParser::parse_classification_json("{ broken").is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn test_conservative_fallback_safe_defaults() {
        let c = WritingIntentClassification::conservative_fallback();
        assert!(!c.is_new_novel, "兜底不应启动创世（避免覆盖已有工作）");
        assert!(c.is_continuation);
        assert_eq!(c.task_type, AssetTaskType::Continuation);
        assert!(c.is_prose_request, "兜底保 force-to-writer 安全默认");
        assert_eq!(c.input_clarity, InputClarity::Vague);
        assert_eq!(c.confidence, 0.0);
    }

    #[test]
    fn test_conservative_fallback_no_story_is_genesis() {
        // v0.30.23: LLM 失败 + 无故事 -> 创世（不可能续写不存在的作品）
        let c = WritingIntentClassification::conservative_fallback_with_context(false);
        assert!(c.is_new_novel, "无故事时兜底应为创世");
        assert!(!c.is_continuation);
        assert_eq!(c.task_type, AssetTaskType::Genesis);
        assert!(c.is_prose_request);
        assert_eq!(c.input_clarity, InputClarity::Vague);
        assert_eq!(c.confidence, 0.0);
    }

    #[test]
    fn test_conservative_fallback_with_story_is_continuation() {
        // v0.30.23: LLM 失败 + 有故事 -> 续写（避免误启动 Agency 覆盖工作）
        let c = WritingIntentClassification::conservative_fallback_with_context(true);
        assert!(!c.is_new_novel, "有故事时兜底应偏续写");
        assert!(c.is_continuation);
        assert_eq!(c.task_type, AssetTaskType::Continuation);
        assert!(c.is_prose_request);
    }

    #[test]
    fn test_classification_prompt_no_context_bias() {
        // v0.30.23: 提示词不应注入"已有故事="上下文行（偏差来源）
        let prompt = IntentParser::build_classification_prompt("写一部科幻小说", true, true);
        assert!(
            !prompt.contains("已有故事="),
            "提示词不应注入 DB 状态上下文（偏差来源）"
        );
        assert!(
            !prompt.contains("已有正文="),
            "提示词不应注入 DB 状态上下文（偏差来源）"
        );
    }

    #[test]
    fn test_classification_prompt_has_positive_examples() {
        // v0.30.23: 提示词应含正例，让 LLM 知道"写一部X" = is_new_novel=true
        let prompt = IntentParser::build_classification_prompt("写一部科幻小说", false, false);
        assert!(
            prompt.contains("写一部科幻小说"),
            "提示词应含'写一部科幻小说'正例"
        );
        assert!(
            prompt.contains("is_new_novel=true"),
            "提示词正例应标注 is_new_novel=true"
        );
        assert!(
            prompt.contains("与是否已有故事无关"),
            "提示词应明确判断与已有故事无关"
        );
    }

    #[test]
    fn test_map_agents() {
        let agents = IntentExecutor::map_agents(&vec![
            "writer".to_string(),
            "plot_analyzer".to_string(),
            "unknown_agent".to_string(),
        ]);
        assert_eq!(agents.len(), 2);
        assert!(matches!(agents[0], AgentType::Writer));
        assert!(matches!(agents[1], AgentType::PlotAnalyzer));
    }

    #[test]
    fn test_build_summary() {
        let intent = Intent::unknown("测试");
        let steps = vec![AgentStepResult {
            agent_name: "Writer".to_string(),
            success: true,
            result: None,
            error: None,
        }];
        let summary = IntentExecutor::build_summary(&intent, &steps);
        assert!(summary.contains("自由对话"));
        assert!(summary.contains("1 个 Agent"));
    }
}

#[cfg(test)]
mod input_clarity_tests {
    use super::*;

    #[test]
    fn empty_input_is_vague() {
        assert_eq!(detect_input_clarity(""), InputClarity::Vague);
        assert_eq!(detect_input_clarity("   "), InputClarity::Vague);
    }

    #[test]
    fn short_input_is_vague() {
        assert_eq!(detect_input_clarity("写小说"), InputClarity::Vague);
        assert_eq!(detect_input_clarity("续写"), InputClarity::Vague);
    }

    #[test]
    fn genre_only_is_vague() {
        // 仅含题材关键词，没有故事元素信号词
        assert_eq!(
            detect_input_clarity("写一篇修仙小说，要爽文风格"),
            InputClarity::Vague
        );
    }

    #[test]
    fn with_one_or_two_signals_is_seed() {
        // 含 1 个角色信号
        assert_eq!(
            detect_input_clarity("主角在山中修炼了三年"),
            InputClarity::WithSeed
        );
        // 含 2 个信号（主角 + 复仇），超过 8 字下限
        assert_eq!(
            detect_input_clarity("主角下山誓要找仇人复仇"),
            InputClarity::WithSeed
        );
    }

    #[test]
    fn with_three_plus_signals_is_full_concept() {
        // 主角 + 师父 + 复仇 = 3 个信号
        assert_eq!(
            detect_input_clarity("主角的师父被人陷害，他立志复仇"),
            InputClarity::WithFullConcept
        );
    }

    #[test]
    fn quartet_inference_decision() {
        assert!(InputClarity::Vague.needs_quartet_inference());
        assert!(InputClarity::WithSeed.needs_quartet_inference());
        assert!(!InputClarity::WithFullConcept.needs_quartet_inference());
    }

    /// 审计测试：验证“异星球末世生存”类输入的清晰度判定
    ///
    /// 该输入仅有题材/主题词，缺少主角、动作、冲突等故事元素信号词，
    /// 当前启发式规则会判定为 Vague，从而触发后台四元组自动补全。
    #[test]
    fn alien_postapocalyptic_input_is_vague() {
        let input = "写一部异星球末世生存题材的小说";
        let clarity = detect_input_clarity(input);
        assert_eq!(
            clarity,
            InputClarity::Vague,
            "'{input}' 只有题材词，应被判定为 Vague 以触发四元组补全"
        );
    }
}
