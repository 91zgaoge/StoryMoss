//! 小说创建 Agent
//!
//! 负责引导式生成小说核心要素：世界观、角色谱、文字风格
#![allow(unused_imports)]

use serde::{Deserialize, Serialize};
use serde_json;

use crate::{
    db::models::*,
    llm::{LlmAdapter, LlmService},
};

/// 小说创建 Agent
pub struct NovelCreationAgent {
    llm_service: LlmService,
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

/// 世界观选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBuildingOption {
    pub id: String,
    pub concept: String,
    pub rules: Vec<WorldRule>,
    pub history: String,
    pub cultures: Vec<Culture>,
}

/// 角色谱选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterProfileOption {
    pub id: String,
    pub name: String,
    pub personality: String,
    pub background: String,
    pub goals: String,
    pub voice_style: String,
}

/// 文字风格选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingStyleOption {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tone: String,
    pub pacing: String,
    pub vocabulary_level: String,
    pub sentence_structure: String,
    pub sample_text: String,
}

impl NovelCreationAgent {
    pub fn new(llm_service: LlmService) -> Self {
        Self { llm_service }
    }

    /// 第一步：根据用户输入生成世界观选项
    pub async fn generate_world_building_options(
        &self,
        user_input: &str,
        options: &GenerationOptions,
    ) -> Result<Vec<WorldBuildingOption>, Box<dyn std::error::Error>> {
        let prompt = format!(
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
        {{"id": "r1", "name": "规则名称", "description": "规则描述", "rule_type": "Magic", "importance": 8}}
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
        );

        let response = self.llm_service.generate(prompt, None, None).await?;
        let parsed: serde_json::Value = serde_json::from_str(&response.content)?;

        let options: Vec<WorldBuildingOption> = parsed["world_buildings"]
            .as_array()
            .ok_or("Invalid response format")?
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        Ok(options)
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

        let prompt = format!(
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
        "voice_style": "语言风格（20-30字）"
      }}
    ]
  ]
}}

注意：
- 姓名应符合世界观文化背景，且具有辨识度
- 禁止使用林、陈、王、李、张、刘等最常见单字姓；禁止单字名
- 同一组角色姓氏不得重复
- 角色应有明确外貌、性别、年龄描述，避免千人一面
- 每组角色之间应有内在联系和冲突
- 确保JSON格式正确"#,
            options.count, world_info
        );

        let response = self.llm_service.generate(prompt, None, None).await?;
        let parsed: serde_json::Value = serde_json::from_str(&response.content)?;

        let sets: Vec<Vec<CharacterProfileOption>> = parsed["character_sets"]
            .as_array()
            .ok_or("Invalid response format")?
            .iter()
            .map(|arr| {
                arr.as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|v| serde_json::from_value(v.clone()).unwrap())
                    .collect()
            })
            .collect();

        Ok(sets)
    }

    /// 第三步：生成文字风格选项
    pub async fn generate_writing_styles(
        &self,
        genre: &str,
        world_building: &WorldBuildingOption,
        options: &GenerationOptions,
    ) -> Result<Vec<WritingStyleOption>, Box<dyn std::error::Error>> {
        let prompt = format!(
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
        );

        let response = self.llm_service.generate(prompt, None, None).await?;
        let parsed: serde_json::Value = serde_json::from_str(&response.content)?;

        let options: Vec<WritingStyleOption> = parsed["writing_styles"]
            .as_array()
            .ok_or("Invalid response format")?
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        Ok(options)
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

        let prompt = format!(
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
        );

        let response = self.llm_service.generate(prompt, None, None).await?;
        let parsed: serde_json::Value = serde_json::from_str(&response.content)?;

        let scene: SceneProposal = serde_json::from_value(parsed["scene"].clone())?;
        Ok(scene)
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
