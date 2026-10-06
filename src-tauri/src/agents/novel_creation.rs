//! 小说创建 Agent
//!
//! 负责引导式生成小说核心要素：世界观、角色谱、文字风格
#![allow(unused_imports)]

use serde::{Deserialize, Serialize};
use serde_json;

pub use crate::domain::novel_creation::{
    CharacterProfileOption, WorldBuildingOption, WritingStyleOption,
};
use crate::{db::DbPool, llm::LlmService, router::TaskType};

/// 小说创建 Agent
pub struct NovelCreationAgent {
    llm_service: LlmService,
    pool: DbPool,
}

/// 生成选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationOptions {
    /// 生成数量（默认3）
    pub count: usize,
    /// 创意程度 (0.0-1.0)
    pub creativity: f32,
    /// 详细程度
    pub detail_level: DetailLevel,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            count: 3,
            creativity: 0.8,
            detail_level: DetailLevel::Normal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetailLevel {
    Brief,
    Normal,
    Detailed,
}

impl NovelCreationAgent {
    pub fn new(llm_service: LlmService, pool: DbPool) -> Self {
        Self { llm_service, pool }
    }

    /// 第一步：根据用户输入生成世界观选项
    pub async fn generate_world_building_options(
        &self,
        user_input: &str,
        options: &GenerationOptions,
    ) -> Result<Vec<WorldBuildingOption>, Box<dyn std::error::Error>> {
        // v0.21.0: 优先从 PromptRegistry 读取
        let mut prompt = if let Some(tpl) =
            crate::prompts::registry::resolve_prompt(&self.pool, "novel_creation_world_options")
                .ok()
                .or_else(|| {
                    crate::prompts::registry::resolve_prompt_default("novel_creation_world_options")
                }) {
            let mut vars = std::collections::HashMap::new();
            vars.insert("count".to_string(), options.count.to_string());
            vars.insert("input".to_string(), user_input.to_string());
            crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
        } else {
            format!(
                r#"作为一位资深世界观设计师，请基于以下用户输入，创建{}个独特的世界观概念。

用户输入：{}

要求：
1. 每个世界观应该有独特的核心概念
2. 包含基本的世界规则（3-5条）
3. 有历史背景概述
4. 包含2-3个主要文化设定

请以JSON格式输出，格式如下：
{{
  "world_buildings": [
    {{
      "id": "wb_1",
      "concept": "世界观核心概念（20-50字）",
      "rules": [
        {{"name": "规则名称", "description": "规则描述", "rule_type": "Magic", "importance": 8}}
      ],
      "history": "历史背景（100-200字）",
      "cultures": [
        {{"name": "文化名称", "description": "文化描述", "customs": ["习俗1", "习俗2"], "values": ["价值观1", "价值观2"]}}
      ]
    }}
  ]
}}

注意：
- 世界观类型可以是：玄幻、科幻、都市、历史、武侠、悬疑等
- 规则类型包括：Magic（魔法）、Technology（科技）、Social（社会）、Physical（物理）
- importance 范围 1-10
- 确保JSON格式正确"#,
                options.count, user_input
            )
        };

        // v0.31 资产融合：注入体裁画像/方法论/四元组（有则注入、无则跳过）
        let asset_ctx = Self::build_creation_asset_context(&self.pool, user_input);
        if !asset_ctx.is_empty() {
            prompt.push_str(&asset_ctx);
        }

        let response = self
            .llm_service
            .generate_for_task(
                TaskType::WorldBuilding,
                prompt,
                None,
                None,
                Some("世界观选项"),
            )
            .await?;
        match Self::parse_world_options_response(&response.content) {
            Ok(options) => Ok(options),
            Err(e) => {
                let snippet: String = response.content.chars().take(200).collect();
                log::warn!(
                    "novel_creation: 世界观选项解析失败 err={} raw_len={} snippet={:?}",
                    e,
                    response.content.len(),
                    snippet
                );
                Err(e.into())
            }
        }
    }

    /// 解析世界观选项响应为纯函数（issue #14）：先用 narrative 健壮提取器剥离
    /// markdown 围栏 / 修复字符串内未转义换行，再反序列化。模型常将 JSON 包裹
    /// 在 ` ```json ... ``` ` 中，旧实现直接 `serde_json::from_str` 全量内容会
    /// 静默失败。提取为独立函数便于单测（无需 mock LlmService）。
    fn parse_world_options_response(content: &str) -> Result<Vec<WorldBuildingOption>, String> {
        let sanitized = crate::narrative::extract_and_sanitize_json(content)
            .unwrap_or_else(|_| content.to_string());
        let parsed: serde_json::Value = serde_json::from_str(&sanitized)
            .map_err(|e| format!("世界观选项 JSON 解析失败: {}", e))?;
        parsed["world_buildings"]
            .as_array()
            .ok_or_else(|| "Invalid response format: 缺少 world_buildings 数组".to_string())?
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| format!("世界观项反序列化失败: {}", e))
            })
            .collect()
    }

    /// 解析角色谱选项响应为纯函数（issue #14 角色谱静默失败）：与世界观选项
    /// 同款健壮提取（剥 markdown 围栏/修未转义换行），逐项 map_err 而非 unwrap
    /// （旧实现 unwrap 会在 tokio task 内 panic，fire-and-forget
    /// 下无任何日志）。
    fn parse_character_roster_response(
        content: &str,
    ) -> Result<Vec<Vec<CharacterProfileOption>>, String> {
        let sanitized = crate::narrative::extract_and_sanitize_json(content)
            .unwrap_or_else(|_| content.to_string());
        let parsed: serde_json::Value = serde_json::from_str(&sanitized)
            .map_err(|e| format!("角色谱选项 JSON 解析失败: {}", e))?;
        parsed["character_sets"]
            .as_array()
            .ok_or_else(|| "Invalid response format: 缺少 character_sets 数组".to_string())?
            .iter()
            .map(|arr| {
                let set = arr.as_array().ok_or_else(|| {
                    "Invalid response format: character_sets 元素非数组".to_string()
                })?;
                set.iter()
                    .map(|v| {
                        serde_json::from_value(v.clone())
                            .map_err(|e| format!("角色项反序列化失败: {}", e))
                    })
                    .collect::<Result<Vec<CharacterProfileOption>, String>>()
            })
            .collect()
    }

    /// 第二步：根据世界观生成角色谱选项
    pub async fn generate_character_profiles(
        &self,
        world_building: &WorldBuildingOption,
        options: &GenerationOptions,
    ) -> Result<Vec<Vec<CharacterProfileOption>>, Box<dyn std::error::Error>> {
        let world_info = format!(
            "世界观概念：{}\n历史背景：{}\n文化设定：{}",
            world_building.concept,
            world_building.history,
            world_building
                .cultures
                .iter()
                .map(|c| format!("{} - {}", c.name, c.description))
                .collect::<Vec<_>>()
                .join("\n")
        );

        // v0.21.0: 优先从 PromptRegistry 读取
        let mut prompt = if let Some(tpl) =
            crate::prompts::registry::resolve_prompt(&self.pool, "novel_creation_character_roster")
                .ok()
                .or_else(|| {
                    crate::prompts::registry::resolve_prompt_default(
                        "novel_creation_character_roster",
                    )
                }) {
            let mut vars = std::collections::HashMap::new();
            vars.insert("count".to_string(), options.count.to_string());
            vars.insert("input".to_string(), world_info.to_string());
            crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
        } else {
            format!(
                r#"作为一位角色设计专家，请基于以下世界观，创建{}组不同的角色配置。

{}

要求：
1. 每组包含3-5个核心角色
2. 角色应该代表不同的立场和功能（主角、反派、导师、盟友等）
3. 角色性格应该鲜明，有冲突和互补
4. 考虑世界观对角色塑造的影响

请以JSON格式输出，格式如下：
{{
  "character_sets": [
    [
      {{
        "id": "char_1_1",
        "name": "角色姓名",
        "personality": "性格特点（30-50字）",
        "background": "背景故事（50-100字）",
        "goals": "目标动机（30-50字）",
        "voice_style": "语言风格（20-30字）",
        "emotional_core": "情感内核：主导情感倾向（20-40字）",
        "emotional_trigger": "情感触发：引爆情绪的场景或行为（20-40字）",
        "emotional_wound": "情感创伤：塑造情感模式的过往伤口（20-40字）",
        "emotional_need": "情感需求：深层渴望的情感满足（20-40字）"
      }}
    ]
  ]
}}

注意：
- 每个角色必须含全部10个字段，emotional_* 四字段不得为空
- 姓名应符合世界观文化背景，且具有辨识度
- 禁止使用林、陈、王、李、张、刘等最常见单字姓；禁止单字名
- 同一组角色姓氏不得重复
- 角色应有明确外貌、性别、年龄描述，避免千人一面
- 每组角色之间应有内在联系和冲突
- 确保JSON格式正确"#,
                options.count, world_info
            )
        };

        // v0.31 资产融合：注入体裁画像/方法论/四元组（有则注入、无则跳过）
        let asset_ctx = Self::build_creation_asset_context(&self.pool, &world_building.concept);
        if !asset_ctx.is_empty() {
            prompt.push_str(&asset_ctx);
        }

        let response = self
            .llm_service
            .generate_for_task(
                TaskType::WorldBuilding,
                prompt,
                None,
                None,
                Some("角色谱选项"),
            )
            .await?;
        match Self::parse_character_roster_response(&response.content) {
            Ok(sets) => Ok(sets),
            Err(e) => {
                let snippet: String = response.content.chars().take(200).collect();
                log::warn!(
                    "novel_creation: 角色谱选项解析失败 err={} raw_len={} snippet={:?}",
                    e,
                    response.content.len(),
                    snippet
                );
                Err(e.into())
            }
        }
    }

    /// 解析文字风格选项响应为纯函数（同 parse_world_options_response 模式）。
    fn parse_writing_styles_response(content: &str) -> Result<Vec<WritingStyleOption>, String> {
        let sanitized = crate::narrative::extract_and_sanitize_json(content)
            .unwrap_or_else(|_| content.to_string());
        let parsed: serde_json::Value = serde_json::from_str(&sanitized)
            .map_err(|e| format!("文字风格选项 JSON 解析失败: {}", e))?;
        parsed["writing_styles"]
            .as_array()
            .ok_or_else(|| "Invalid response format: 缺少 writing_styles 数组".to_string())?
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).map_err(|e| format!("风格项反序列化失败: {}", e))
            })
            .collect()
    }

    /// 解析首场景响应为纯函数（同 parse_world_options_response 模式）。
    fn parse_first_scene_response(content: &str) -> Result<SceneProposal, String> {
        let sanitized = crate::narrative::extract_and_sanitize_json(content)
            .unwrap_or_else(|_| content.to_string());
        let parsed: serde_json::Value =
            serde_json::from_str(&sanitized).map_err(|e| format!("首场景 JSON 解析失败: {}", e))?;
        serde_json::from_value(parsed["scene"].clone())
            .map_err(|e| format!("首场景反序列化失败: {}", e))
    }

    /// v0.31 资产融合：为向导 prompt 组装创作资产上下文——体裁画像内容
    /// （core_tone/反模式/典型结构）+ 推荐方法论 system_prompt_extension +
    /// 四元组推荐。画像解析失败返回空串（调用方跳过注入，记 debug）。
    fn build_creation_asset_context(pool: &DbPool, genre_text: &str) -> String {
        let repo = crate::db::GenreProfileRepository::new(pool.clone());
        let resolver = crate::strategy::GenreResolver::new();
        let profile = resolver
            .resolve_from_text(genre_text, &repo)
            .ok()
            .and_then(|matches| matches.first().map(|m| m.profile_id.clone()))
            .and_then(|id| repo.get_by_id(&id).ok().flatten());
        let profile = match profile {
            Some(p) => p,
            None => {
                log::debug!(
                    "[novel_creation] 未能从输入解析体裁画像，跳过资产注入: {}",
                    genre_text
                );
                return String::new();
            }
        };

        let mut sections = Self::render_genre_profile_section(&profile);

        // 推荐方法论的 system_prompt_extension
        if let Some(ref mid) = profile.recommended_methodology_id {
            let normalized = crate::domain::methodology::normalize_methodology_id(mid);
            if let Ok(mtype) = serde_json::from_value::<crate::domain::methodology::MethodologyType>(
                serde_json::Value::String(normalized.to_string()),
            ) {
                let config = crate::domain::methodology::MethodologyConfig {
                    methodology_type: mtype,
                    is_active: true,
                    current_step: None,
                    custom_params: serde_json::json!({}),
                };
                let ext =
                    crate::creative_engine::methodology::MethodologyEngine::build_prompt_extension(
                        &config,
                        Some(pool),
                    );
                if !ext.trim().is_empty() {
                    // final-review F5：头部显示归一化后的方法论 ID
                    sections.push_str(&format!("\n【推荐方法论：{}】\n{}\n", normalized, ext));
                }
            }
        }

        // 四元组推荐（纯启发式，不调 LLM）
        let mut strategy = crate::domain::strategy::SelectedStrategy::default();
        crate::strategy::infer_narrative_quartet(
            &mut strategy,
            Some(&profile.canonical_name),
            profile.reader_promise.as_deref(),
            crate::intent::InputClarity::Vague,
        );
        if let Ok(quartet) =
            crate::strategy::quartet_inference::serialize_quartet_for_prompt(&strategy)
        {
            if !quartet.is_null() {
                sections.push_str(&format!("\n【中文叙事四元组推荐】\n{}\n", quartet));
            }
        }
        sections
    }

    /// 渲染体裁画像段落（纯函数，便于单测）。三个内容字段全空返回空串。
    fn render_genre_profile_section(profile: &crate::db::GenreProfile) -> String {
        let mut body = String::new();
        if let Some(ref tone) = profile.core_tone {
            body.push_str(&format!("核心基调：{}\n", tone));
        }
        if let Some(ref anti) = profile.anti_patterns_json {
            body.push_str(&format!("反模式（必须避免）：{}\n", anti));
        }
        if let Some(ref structure) = profile.typical_structure_json {
            body.push_str(&format!("典型结构：{}\n", structure));
        }
        if body.is_empty() {
            String::new()
        } else {
            format!("\n【体裁画像：{}】\n{}", profile.genre_name, body)
        }
    }

    /// 第三步：生成文字风格选项
    pub async fn generate_writing_styles(
        &self,
        genre: &str,
        world_building: &WorldBuildingOption,
        options: &GenerationOptions,
    ) -> Result<Vec<WritingStyleOption>, Box<dyn std::error::Error>> {
        // v0.21.0: 优先从 PromptRegistry 读取
        let mut prompt = if let Some(tpl) =
            crate::prompts::registry::resolve_prompt(&self.pool, "novel_creation_writing_style")
                .ok()
                .or_else(|| {
                    crate::prompts::registry::resolve_prompt_default("novel_creation_writing_style")
                }) {
            let mut vars = std::collections::HashMap::new();
            vars.insert("count".to_string(), options.count.to_string());
            vars.insert(
                "input".to_string(),
                format!("{} {}", genre, world_building.concept),
            );
            crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
        } else {
            format!(
                r#"作为一位资深文学编辑，请基于以下小说类型和世界观，创建{}种不同的文字风格。

小说类型：{}
世界观概念：{}

要求：
1. 每种风格应该有独特的名称和描述
2. 明确语调和节奏特点
3. 提供词汇水平和句式结构说明
4. 每种风格配一段示例文本（100-150字）

请以JSON格式输出，格式如下：
{{
  "writing_styles": [
    {{
      "id": "ws_1",
      "name": "风格名称",
      "description": "风格描述（30-50字）",
      "tone": "语调特点",
      "pacing": "节奏特点",
      "vocabulary_level": "词汇水平",
      "sentence_structure": "句式结构",
      "sample_text": "示例文本（100-150字）"
    }}
  ]
}}

注意：
- 风格应该适合所选小说类型
- 示例文本应该能体现该风格特点
- 确保JSON格式正确"#,
                options.count, genre, world_building.concept
            )
        };

        // v0.31 资产融合：注入体裁画像/方法论/四元组（有则注入、无则跳过）
        let asset_ctx = Self::build_creation_asset_context(&self.pool, genre);
        if !asset_ctx.is_empty() {
            prompt.push_str(&asset_ctx);
        }

        let response = self
            .llm_service
            .generate_for_task(
                TaskType::CreativeWriting,
                prompt,
                None,
                None,
                Some("文字风格选项"),
            )
            .await?;
        match Self::parse_writing_styles_response(&response.content) {
            Ok(options) => Ok(options),
            Err(e) => {
                let snippet: String = response.content.chars().take(200).collect();
                log::warn!(
                    "novel_creation: 文字风格选项解析失败 err={} raw_len={} snippet={:?}",
                    e,
                    response.content.len(),
                    snippet
                );
                Err(e.into())
            }
        }
    }

    /// 生成首个场景建议
    pub async fn generate_first_scene(
        &self,
        world_building: &WorldBuildingOption,
        characters: &[CharacterProfileOption],
        writing_style: &WritingStyleOption,
    ) -> Result<SceneProposal, Box<dyn std::error::Error>> {
        let char_info = characters
            .iter()
            .map(|c| format!("{}：{}，{}", c.name, c.personality, c.goals))
            .collect::<Vec<_>>()
            .join("\n");

        // v0.21.0: 优先从 PromptRegistry 读取
        let mut prompt = if let Some(tpl) =
            crate::prompts::registry::resolve_prompt(&self.pool, "novel_creation_opening_scene")
                .ok()
                .or_else(|| {
                    crate::prompts::registry::resolve_prompt_default("novel_creation_opening_scene")
                }) {
            let mut vars = std::collections::HashMap::new();
            vars.insert(
                "input".to_string(),
                format!(
                    "{} {} {}",
                    world_building.concept, char_info, writing_style.name
                ),
            );
            crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
        } else {
            format!(
                r#"作为一位场景设计专家，请基于以下设定，设计一个开场场景。

世界观：{}
角色：
{}
文字风格：{}

要求：
1. 场景应该有强烈的戏剧冲突或悬念
2. 展示主要角色的特点和关系
3. 体现世界观的独特元素
4. 符合指定的文字风格

请以JSON格式输出：
{{
  "scene": {{
    "title": "场景标题",
    "dramatic_goal": "戏剧目标",
    "external_pressure": "外部压迫",
    "conflict_type": "冲突类型（ManVsMan/ManVsSelf/ManVsSociety/ManVsNature/ManVsTechnology/ManVsFate/ManVsSupernatural）",
    "setting_location": "地点",
    "setting_time": "时间",
    "setting_atmosphere": "氛围",
    "content": "场景正文（500-800字）"
  }}
}}

注意：
- 场景应该能吸引读者继续阅读
- 确保JSON格式正确"#,
                world_building.concept, char_info, writing_style.name
            )
        };

        // v0.31 资产融合：注入体裁画像/方法论/四元组（有则注入、无则跳过）
        let asset_ctx = Self::build_creation_asset_context(&self.pool, &world_building.concept);
        if !asset_ctx.is_empty() {
            prompt.push_str(&asset_ctx);
        }

        let response = self
            .llm_service
            .generate_for_task(
                TaskType::CreativeWriting,
                prompt,
                None,
                None,
                Some("首场景"),
            )
            .await?;
        match Self::parse_first_scene_response(&response.content) {
            Ok(scene) => Ok(scene),
            Err(e) => {
                let snippet: String = response.content.chars().take(200).collect();
                log::warn!(
                    "novel_creation: 首场景解析失败 err={} raw_len={} snippet={:?}",
                    e,
                    response.content.len(),
                    snippet
                );
                Err(e.into())
            }
        }
    }
}

/// 场景建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneProposal {
    pub title: String,
    pub dramatic_goal: String,
    pub external_pressure: String,
    pub conflict_type: String,
    pub setting_location: String,
    pub setting_time: String,
    pub setting_atmosphere: String,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_WORLD_JSON: &str = r#"{"world_buildings":[{"id":"wb_1","concept":"双星废土文明","rules":[{"name":"星环律","description":"资源受星环周期约束","rule_type":"physical","importance":9}],"history":"千年前双星相撞","cultures":[{"name":"拾荒者","description":"废土游民","customs":["以物易物"],"values":["生存至上"]}]}]}"#;

    #[test]
    fn test_parse_world_options_clean_json() {
        let opts = NovelCreationAgent::parse_world_options_response(VALID_WORLD_JSON).unwrap();
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].id, "wb_1");
        assert_eq!(opts[0].concept, "双星废土文明");
        assert_eq!(opts[0].rules.len(), 1);
        assert_eq!(opts[0].rules[0].importance, 9);
        assert_eq!(opts[0].cultures[0].customs, vec!["以物易物".to_string()]);
    }

    #[test]
    fn test_parse_world_options_markdown_fenced() {
        // issue #14：模型将 JSON 包裹在 ```json ... ``` 代码块中，旧实现直接
        // serde_json::from_str 全量内容会失败；现先经 extract_and_sanitize_json
        // 剥离围栏再解析。
        let raw = format!(
            "好的，以下是世界观选项：\n```json\n{}\n```\n希望你喜欢。",
            VALID_WORLD_JSON
        );
        let opts = NovelCreationAgent::parse_world_options_response(&raw).unwrap();
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].concept, "双星废土文明");
    }

    #[test]
    fn test_parse_world_options_missing_key_errors() {
        // prompt 误写 "concepts" 而 code 读 "world_buildings" 的回归守卫
        let raw =
            r#"{"concepts":[{"id":"wb_1","concept":"x","rules":[],"history":"","cultures":[]}]}"#;
        let err = NovelCreationAgent::parse_world_options_response(raw).unwrap_err();
        assert!(
            err.contains("world_buildings"),
            "错误信息应指出缺少 world_buildings 数组: {}",
            err
        );
    }

    const VALID_ROSTER_JSON: &str = r#"{"character_sets":[[{"id":"char_1_1","name":"阿苔","personality":"坚韧沉默","background":"拾荒者出身","goals":"找到星环","voice_style":"简短直接"}]]}"#;

    #[test]
    fn test_parse_character_roster_markdown_fenced() {
        // issue #14 角色谱静默失败：模型将 JSON 包裹在 ```json 围栏中，旧实现
        // serde_json::from_str 全量解析失败且无任何日志。
        let raw = format!("以下是角色谱：\n```json\n{}\n```", VALID_ROSTER_JSON);
        let sets = NovelCreationAgent::parse_character_roster_response(&raw).unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].len(), 1);
        assert_eq!(sets[0][0].name, "阿苔");
    }

    #[test]
    fn test_parse_character_roster_bad_item_no_panic() {
        // 旧实现 from_value(...).unwrap() 会在缺字段时 panic（tokio task
        // 内被吞）， 现应返回可诊断错误。
        let raw = r#"{"character_sets":[[{"id":"char_1_1"}]]}"#;
        let err = NovelCreationAgent::parse_character_roster_response(raw).unwrap_err();
        assert!(err.contains("角色项反序列化失败"), "err={}", err);
    }

    #[test]
    fn test_parse_character_roster_missing_key_errors() {
        let raw = r#"{"characters":[]}"#;
        let err = NovelCreationAgent::parse_character_roster_response(raw).unwrap_err();
        assert!(err.contains("character_sets"), "err={}", err);
    }

    #[test]
    fn test_character_profile_option_deserializes_emotional_fields() {
        let json = r#"{"id":"c1","name":"阿岩","personality":"偏执","background":"孤儿",
          "goals":"夺回令牌","voice_style":"冷硬",
          "emotional_core":"表面冷漠内心炽热","emotional_trigger":"被背叛时暴怒",
          "emotional_wound":"童年被师父抛弃","emotional_need":"渴望被认可"}"#;
        let opt: CharacterProfileOption = serde_json::from_str(json).unwrap();
        assert_eq!(opt.emotional_core, "表面冷漠内心炽热");
        assert_eq!(opt.emotional_wound, "童年被师父抛弃");
    }

    #[test]
    fn test_character_profile_option_backward_compat() {
        let json = r#"{"id":"c1","name":"甲","personality":"","background":"","goals":"","voice_style":""}"#;
        let opt: CharacterProfileOption = serde_json::from_str(json).unwrap();
        assert_eq!(opt.emotional_core, "");
    }

    #[test]
    fn test_parse_writing_styles_markdown_fenced() {
        let raw = r#"```json
{"writing_styles":[{"id":"ws_1","name":"冷峻纪实","description":"克制白描","tone":"冷","pacing":"缓","vocabulary_level":"中","sentence_structure":"短句为主","sample_text":"风刮过废土。"}]}
```"#;
        let styles = NovelCreationAgent::parse_writing_styles_response(raw).unwrap();
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].name, "冷峻纪实");
    }

    #[test]
    fn test_parse_first_scene_markdown_fenced() {
        let raw = r#"```json
{"scene":{"title":"星环坠落","dramatic_goal":"求生","external_pressure":"磁力风暴","conflict_type":"ManVsNature","setting_location":"废土","setting_time":"黄昏","setting_atmosphere":"压抑","content":"风刮过……"}}
```"#;
        let scene = NovelCreationAgent::parse_first_scene_response(raw).unwrap();
        assert_eq!(scene.title, "星环坠落");
        assert_eq!(scene.conflict_type, "ManVsNature");
    }

    fn sample_genre_profile() -> crate::db::GenreProfile {
        crate::db::GenreProfile {
            id: "apoc-id".into(),
            genre_name: "末世流".into(),
            canonical_name: "Post-apocalyptic".into(),
            aliases_json: None,
            core_tone: Some("压抑中见温情".into()),
            pacing_strategy: None,
            anti_patterns_json: Some(r#"["圣母主角","无敌开局"]"#.into()),
            reference_tables_json: None,
            typical_structure_json: Some(r#"["崩塌-流浪-聚落-抉择"]"#.into()),
            reader_promise: Some("爽,燃".into()),
            recommended_style_dna_ids: None,
            recommended_methodology_id: Some("hero_journey".into()),
            recommended_skill_ids: None,
            min_quality_tier: None,
            is_builtin: true,
            created_at: chrono::Local::now(),
        }
    }

    #[test]
    fn test_render_genre_profile_section_includes_assets() {
        // 向导 prompt 必须含体裁画像的 core_tone / anti_patterns /
        // typical_structure
        let section = NovelCreationAgent::render_genre_profile_section(&sample_genre_profile());
        assert!(section.contains("末世流"));
        assert!(section.contains("压抑中见温情"), "应含 core_tone");
        assert!(section.contains("圣母主角"), "应含 anti_patterns");
        assert!(
            section.contains("崩塌-流浪-聚落-抉择"),
            "应含 typical_structure"
        );
    }

    #[test]
    fn test_render_genre_profile_section_empty_when_no_assets() {
        // 画像无内容字段时返回空串（调用方跳过注入，不污染 prompt）
        let mut p = sample_genre_profile();
        p.core_tone = None;
        p.anti_patterns_json = None;
        p.typical_structure_json = None;
        assert!(NovelCreationAgent::render_genre_profile_section(&p).is_empty());
    }
}
