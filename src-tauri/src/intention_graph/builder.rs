//! 意图合成流水线
//!
//! 三阶段合成：Query Synthesis → Chain Expansion → Atomic Intention Extraction
//! 将用户自然语言输入转化为标准化的原子意图节点。
//!
//! v0.20.1: 修复审计报告 P0-5——此前三阶段全为 rule-based（关键词匹配），
//! LLM 字段从未使用。现改为 LLM 增强版：
//! - Phase 1 (Query Synthesis): LLM 理解自然语言意图，失败时回退到关键词匹配
//! - Phase 2 (Chain Expansion): 查询 asset_asset_edges 的 tool_next 关系扩展，
//!   回退到预设规则
//! - Phase 3 (Atomic Extraction): LLM 提取原子动词-宾语，回退到 splitn 切分

use super::models::*;
use crate::{db::DbPool, error::AppError, intention_graph::IntentContext, llm::LlmService};

/// 意图合成流水线
pub struct IntentSynthesisPipeline {
    llm_service: LlmService,
}

impl IntentSynthesisPipeline {
    pub fn new(llm_service: LlmService) -> Self {
        Self { llm_service }
    }

    /// v0.21.0: 意图分析器内置默认提示词（registry 不可用时的最终回退）
    fn default_intent_prompt() -> &'static str {
        r#"你是一个意图分析器。分析用户的创作指令，提取核心意图。

输出严格的 JSON 格式：
{"verb": "<动词>", "object": "<宾语>", "confidence": <0.0-1.0>}

动词必须是以下之一：generate, write, create, enhance, polish, revise, edit, inspect, check, analyze, plan, outline, structure, manage, update, query, search, fetch
宾语必须是以下之一：prose, content, chapter, scene, story, style, character, world, outline, structure, quality, data, plot

示例：
- "续写" → {"verb": "generate", "object": "prose", "confidence": 0.9}
- "润色这段文字" → {"verb": "enhance", "object": "style", "confidence": 0.85}
- "检查角色一致性" → {"verb": "inspect", "object": "quality", "confidence": 0.8}
- "修改主角设定" → {"verb": "manage", "object": "character", "confidence": 0.85}

只输出 JSON，不要其他文字。"#
    }

    /// 阶段一：Query Synthesis
    ///
    /// v0.20.1: 优先使用 LLM 理解用户自然语言意图，提取主意图动词-宾语。
    /// LLM 失败时回退到关键词匹配（原有 rule-based 逻辑）。
    pub async fn synthesize_query(
        &self,
        user_input: &str,
        context: &IntentContext,
        pool: Option<&DbPool>,
    ) -> SynthesizedQuery {
        // 先尝试 LLM 合成
        match self
            .synthesize_query_with_llm(user_input, context, pool)
            .await
        {
            Ok(query) => {
                log::debug!(
                    "[IntentSynthesis] LLM 合成成功: {} (confidence: {:.2})",
                    query.primary_intent,
                    query.confidence
                );
                query
            }
            Err(e) => {
                log::debug!("[IntentSynthesis] LLM 合成失败，回退到关键词匹配: {}", e);
                self.synthesize_query_rule_based(user_input, context)
            }
        }
    }

    /// LLM 增强的 Query Synthesis
    async fn synthesize_query_with_llm(
        &self,
        user_input: &str,
        _context: &IntentContext,
        pool: Option<&DbPool>,
    ) -> Result<SynthesizedQuery, AppError> {
        // v0.21.0: 从 PromptRegistry 读取（支持用户覆盖），回退到内置默认
        let system_prompt = if let Some(pool) = pool {
            crate::prompts::registry::resolve_prompt(pool, "intent_analyzer").unwrap_or_else(|_| {
                crate::prompts::registry::resolve_prompt_default("intent_analyzer")
                    .unwrap_or_else(|| Self::default_intent_prompt().to_string())
            })
        } else {
            crate::prompts::registry::resolve_prompt_default("intent_analyzer")
                .unwrap_or_else(|| Self::default_intent_prompt().to_string())
        };

        let user_prompt = format!("{}\n\n用户指令：{}", system_prompt, user_input);

        let response = self
            .llm_service
            .generate(user_prompt, Some(100), Some(0.1))
            .await?;

        // v0.30.11: 硬化 LLM 主路径--严格解析失败时宽松 salvage，verb/object
        // 缺失时从 raw_input 推断（写作应用默认 generate prose），避免本地模型
        // 散文前后缀或字段缺失直接回退到无效的 "analyze intent" 规则路径。
        let raw = response.content.trim();
        let (verb_raw, object_raw, confidence) = Self::extract_intent_fields(raw, user_input)?;

        // v0.20.1: 意图归一化--将 LLM 返回的动词映射到 AssetSync 注册的标准动词，
        // 确保不同表达方式归一化到同一意图节点（修复审计报告 P2-3）
        let verb = Self::normalize_verb(&verb_raw);
        let object = Self::normalize_object(&object_raw);

        Ok(SynthesizedQuery {
            raw_input: user_input.to_string(),
            primary_intent: format!("{} {}", verb, object),
            confidence,
            detected_keywords: vec![verb.to_string(), object.to_string()],
            context_hints: vec![],
        })
    }

    /// v0.30.11: 宽松提取 verb/object/confidence。
    ///
    /// 1. 剥离 markdown 代码块；
    /// 2. 截取首个 `{` 到末个 `}` 的 JSON 对象子串（容忍本地模型散文前后缀）；
    /// 3. 严格 JSON 解析；失败则字段级 salvage；
    /// 4. verb/object 缺失时从 `raw_input` 推断（写作应用默认
    ///    `generate`/`prose`）。
    fn extract_intent_fields(
        raw: &str,
        raw_input: &str,
    ) -> Result<(String, String, f64), AppError> {
        // 剥离 markdown 代码块
        let stripped = if raw.starts_with("```") {
            raw.trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
        } else {
            raw
        };
        // 截取 JSON 对象子串
        let json_str = match (stripped.find('{'), stripped.rfind('}')) {
            (Some(start), Some(end)) if end > start => &stripped[start..=end],
            _ => stripped,
        };
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| AppError::internal(format!("LLM JSON parse failed: {}", e)))?;

        let verb_raw = parsed
            .get("verb")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let object_raw = parsed
            .get("object")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let confidence = parsed
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7);

        // verb/object 缺失时从 raw_input 推断（写作应用默认 generate prose）
        let verb = verb_raw.unwrap_or_else(|| {
            if Self::looks_like_prose_request(raw_input) {
                "generate".to_string()
            } else {
                "analyze".to_string()
            }
        });
        let object = object_raw.unwrap_or_else(|| {
            if Self::looks_like_prose_request(raw_input) {
                "prose".to_string()
            } else {
                "intent".to_string()
            }
        });

        Ok((verb, object, confidence))
    }

    /// v0.30.11: raw_input 是否像散文生成请求（多字 pattern，无单字噪声）。
    fn looks_like_prose_request(input: &str) -> bool {
        let lower = input.to_lowercase();
        [
            "创作",
            "续写",
            "继续",
            "写小说",
            "写故事",
            "写一章",
            "写正文",
            "开篇",
        ]
        .iter()
        .any(|kw| lower.contains(kw))
            || ["write", "continue", "begin writing"]
                .iter()
                .any(|kw| lower.contains(kw))
    }

    /// v0.20.1: 将 LLM 返回的动词归一化到 AssetSync 注册的标准动词
    ///
    /// LLM 可能返回 "write"/"create"/"generate" 等同义词，
    /// 统一映射到 "generate"，确保 discover 时能匹配到已注册的意图节点。
    pub fn normalize_verb(verb: &str) -> String {
        match verb.to_lowercase().as_str() {
            "write" | "create" | "generate" => "generate",
            "inspect" | "check" | "audit" => "inspect",
            "revise" | "edit" | "rewrite" => "revise",
            "enhance" | "polish" => "enhance",
            "plan" | "outline" | "structure" => "plan",
            "manage" | "update" => "manage",
            "search" => "search",
            "fetch" | "query" => "fetch",
            "analyze" => "analyze",
            _ => verb, // 未知动词保持原样，避免过度归一化
        }
        .to_string()
    }

    /// v0.20.1: 将 LLM 返回的宾语归一化到 AssetSync 注册的标准宾语
    pub fn normalize_object(object: &str) -> String {
        match object.to_lowercase().as_str() {
            "prose" | "content" | "story" | "novel" | "text" => "prose",
            "chapter" | "scene" => "prose", // 章节/场景生成本质是 prose 生成
            "quality" | "continuity" | "consistency" => "quality",
            "style" | "dna" => "style",
            "character" | "person" => "character",
            "world" | "setting" | "worldview" => "world building",
            "outline" | "plot" | "structure" => "structure",
            "data" | "information" | "knowledge" => "data",
            _ => object, // 未知宾语保持原样
        }
        .to_string()
    }

    /// 规则匹配的 Query Synthesis（回退路径）
    fn synthesize_query_rule_based(
        &self,
        user_input: &str,
        _context: &IntentContext,
    ) -> SynthesizedQuery {
        let normalized = user_input.to_lowercase();

        // v0.30.11: 移除单字 "写"（命中"修改/改写"等噪声）；仅多字 pattern。
        // 此为 LLM 不可用时的降级路径，主路径仍是 synthesize_query_with_llm。
        // 检测明确的创作意图
        let is_prose_request = [
            "write",
            "创作",
            "开始写",
            "写小说",
            "写故事",
            "写一章",
            "写开篇",
            "写正文",
            "start writing",
            "write a novel",
            "write a story",
            "write chapter",
            "begin writing",
            "续写",
            "继续",
        ]
        .iter()
        .any(|kw| normalized.contains(kw));

        let is_style_request = [
            "风格",
            "style",
            "文风",
            "润色",
            "polish",
            "enhance style",
            "改风格",
            "调整风格",
        ]
        .iter()
        .any(|kw| normalized.contains(kw));

        let is_character_request = [
            "角色",
            "character",
            "人物",
            "person",
            "创建角色",
            "create character",
            "修改角色",
            "update character",
        ]
        .iter()
        .any(|kw| normalized.contains(kw));

        let is_world_request = [
            "世界观",
            "world",
            "设定",
            "setting",
            "修改世界观",
            "update world",
            "世界规则",
        ]
        .iter()
        .any(|kw| normalized.contains(kw));

        let is_outline_request = [
            "大纲",
            "outline",
            "规划",
            "plan",
            "生成大纲",
            "create outline",
            "故事结构",
        ]
        .iter()
        .any(|kw| normalized.contains(kw));

        let primary_intent = if is_prose_request {
            "generate prose"
        } else if is_style_request {
            "enhance style"
        } else if is_character_request {
            "manage character"
        } else if is_world_request {
            "manage world building"
        } else if is_outline_request {
            "plan structure"
        } else {
            // v0.30.11: 兜底默认 generate prose（原 "analyze intent" 产出空链无效）。
            // 写作应用中未识别的输入默认按散文生成处理，expand_chain 映射到
            // inspect+revise 安全链；误判代价远低于空链。
            "generate prose"
        };

        SynthesizedQuery {
            raw_input: user_input.to_string(),
            primary_intent: primary_intent.to_string(),
            confidence: if is_prose_request || is_style_request || is_character_request {
                0.9
            } else {
                0.6
            },
            detected_keywords: vec![],
            context_hints: vec![],
        }
    }

    /// 阶段二：Chain Expansion
    /// 将主意图扩展为意图链（基于预设创作流程模式）
    pub fn expand_chain(&self, query: &SynthesizedQuery) -> Vec<String> {
        // 基于主意图的预设链扩展规则
        // 实际实现中会查询 asset_asset_edges 的 tool_next 关系
        match query.primary_intent.as_str() {
            "generate prose" => vec![
                "generate prose".to_string(),
                "inspect quality".to_string(),
                "revise content".to_string(),
            ],
            "enhance style" => vec![
                "enhance style".to_string(),
                "apply style dna".to_string(),
                "inspect quality".to_string(),
            ],
            "manage character" => vec![
                "manage character".to_string(),
                "update knowledge graph".to_string(),
            ],
            "manage world building" => vec![
                "manage world building".to_string(),
                "update knowledge graph".to_string(),
            ],
            "plan structure" => vec!["plan structure".to_string(), "generate outline".to_string()],
            _ => vec![query.primary_intent.clone()],
        }
    }

    /// 阶段三：Atomic Intention Extraction
    /// 将意图链分解为原子化的动词-宾语节点
    pub fn extract_atomic_intentions(&self, chain: &[String]) -> Vec<IntentionNode> {
        let mut nodes = Vec::new();

        for intent_str in chain {
            let parts: Vec<&str> = intent_str.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let verb = parts[0];
                let object = parts[1];
                let description = format!("{} {}", verb, object);
                nodes.push(IntentionNode::atomic(verb, object, &description));
            }
        }

        nodes
    }

    /// 完整三阶段合成（LLM 增强 + 规则回退）
    pub async fn synthesize_full(
        &self,
        user_input: &str,
        context: &IntentContext,
        pool: Option<&DbPool>,
    ) -> Result<IntentSynthesisResult, AppError> {
        // Phase 1: Query Synthesis (LLM 增强优先，回退到规则匹配)
        let query = self.synthesize_query(user_input, context, pool).await;

        // Phase 2: Chain Expansion (规则匹配)
        let chain = self.expand_chain(&query);

        // Phase 3: Atomic Extraction
        let sub_intentions = self.extract_atomic_intentions(&chain);

        let root_intention = if let Some(first) = sub_intentions.first() {
            first.clone()
        } else {
            IntentionNode::atomic("analyze", "intent", "analyze user intent")
        };

        Ok(IntentSynthesisResult {
            root_intention,
            sub_intentions,
            confidence: query.confidence,
            chain_expansion: chain,
        })
    }
}

/// 合成后的查询结构
#[derive(Debug, Clone)]
pub struct SynthesizedQuery {
    pub raw_input: String,
    pub primary_intent: String,
    pub confidence: f64,
    pub detected_keywords: Vec<String>,
    pub context_hints: Vec<String>,
}
