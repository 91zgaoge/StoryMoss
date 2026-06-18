//! 资产目录构建器
//!
//! 把各类创作资产统一转换为 SelectableAsset，供 StrategySelector 与
//! CapabilityRegistry 使用。

use super::models::{AssetKind, SelectableAsset};
use crate::{
    creative_engine::{
        beat_cards::{builtin_beat_cards, BeatCard},
        methodology::{MethodologyEngine, MethodologyType},
        pressure_relationships::{builtin_pressure_relationships, PressureRelationship},
        story_engines::{builtin_story_engines, StoryEngine},
        style::{classic_styles::get_builtin_styles, dna::StyleDNA},
    },
    db::GenreProfile,
    skills::{Skill, SkillCategory},
    workflow::Workflow,
};

/// 把创作方法论转换为可选择资产
pub fn methodology_assets() -> Vec<SelectableAsset> {
    MethodologyEngine::list_available()
        .into_iter()
        .map(|mt| {
            let id = format!("methodology.{}", methodology_id(mt));
            SelectableAsset {
                id: id.clone(),
                kind: AssetKind::Methodology,
                name: mt.name().to_string(),
                description: mt.description().to_string(),
                when_to_use: methodology_when_to_use(mt),
                input_description: Some(
                    "故事概念、目标字数、当前创作阶段（世界观/大纲/场景/正文）".to_string(),
                ),
                output_description: Some("该方法论的 system prompt 扩展与步骤指引".to_string()),
                payload: serde_json::json!({
                    "methodology_type": mt,
                    "id": methodology_id(mt),
                }),
                metadata: Default::default(),
            }
        })
        .collect()
}

fn methodology_id(mt: MethodologyType) -> &'static str {
    match mt {
        MethodologyType::Snowflake => "snowflake",
        MethodologyType::SceneStructure => "scene_structure",
        MethodologyType::HeroJourney => "hero_journey",
        MethodologyType::CharacterDepth => "character_depth",
        MethodologyType::HighDensityWorldBuilding => "high_density_world_building",
    }
}

fn methodology_when_to_use(mt: MethodologyType) -> String {
    match mt {
        MethodologyType::Snowflake => {
            "当你需要从一句核心概念开始，层层扩展成完整大纲和正文时使用。适合目标清晰、喜欢自顶向下规划的作者。"
                .to_string()
        }
        MethodologyType::SceneStructure => {
            "当你需要把每个场景写成目标-冲突-灾难-反应-困境-决定的完整节拍时使用。适合注重场景张力与节奏的网文。"
                .to_string()
        }
        MethodologyType::HeroJourney => {
            "当你要写一个清晰的主角成长弧线、按约瑟夫·坎贝尔十二阶段推进故事时使用。适合史诗感强、主角蜕变明显的题材。"
                .to_string()
        }
        MethodologyType::CharacterDepth => {
            "当你希望以角色内心冲突、动机、秘密、弧光为核心驱动力时使用。适合人物关系复杂、心理描写重的故事。"
                .to_string()
        }
        MethodologyType::HighDensityWorldBuilding => {
            "当你需要构建一个元素不多但高度自洽、事件会回流的活世界观时使用。尤其适合末世、科幻、奇幻等需要强设定支撑的题材。"
                .to_string()
        }
    }
}

/// 把体裁画像转换为可选择资产
pub fn genre_profile_assets(profiles: &[GenreProfile]) -> Vec<SelectableAsset> {
    profiles
        .iter()
        .map(|profile| {
            let id = format!("genre_profile.{}", profile.id);
            let aliases: Vec<String> = profile
                .aliases_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            let typical_structure: Vec<serde_json::Value> = profile
                .typical_structure_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            SelectableAsset {
                id: id.clone(),
                kind: AssetKind::GenreProfile,
                name: profile.genre_name.clone(),
                description: profile
                    .core_tone
                    .clone()
                    .unwrap_or_else(|| format!("{} 体裁模板", profile.genre_name)),
                when_to_use: format!(
                    "当用户要创作 {} 题材（别名：{}）时使用。参考节奏策略、反套路清单与典型结构来指导世界观、大纲和正文。",
                    profile.genre_name,
                    aliases.join(", ")
                ),
                input_description: Some("故事概念、目标字数、主角设定".to_string()),
                output_description: Some("体裁专家策略（core_tone / pacing / anti_patterns / reference_tables / typical_structure）".to_string()),
                payload: serde_json::json!({
                    "genre_name": profile.genre_name,
                    "canonical_name": profile.canonical_name,
                    "aliases": aliases,
                    "core_tone": profile.core_tone,
                    "pacing_strategy": profile.pacing_strategy,
                    "anti_patterns": profile.anti_patterns_json.as_deref().and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()).unwrap_or_default(),
                    "reference_tables": profile.reference_tables_json,
                    "typical_structure": typical_structure,
                    "reader_promise": profile.reader_promise,
                }),
                metadata: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "is_builtin".to_string(),
                        serde_json::Value::Bool(profile.is_builtin),
                    );
                    m
                },
            }
        })
        .collect()
}

/// 把 Style DNA 转换为可选择资产
pub fn style_dna_assets() -> Vec<SelectableAsset> {
    get_builtin_styles()
        .into_iter()
        .map(style_dna_to_asset)
        .collect()
}

fn style_dna_to_asset(dna: StyleDNA) -> SelectableAsset {
    let id = dna
        .meta
        .name
        .to_lowercase()
        .replace(' ', "_")
        .replace("\u{3000}", "_")
        .replace("\u{ff0c}", "_");
    let id = format!("style_dna.{}", id);
    let genre_association = dna.meta.genre_association.clone().unwrap_or_default();

    SelectableAsset {
        id: id.clone(),
        kind: AssetKind::StyleDna,
        name: dna.meta.name.clone(),
        description: dna.meta.description.clone(),
        when_to_use: format!(
            "当用户希望正文呈现 {} 风格（关联题材：{}）时使用。注意控制句长、修辞密度、对话比例与情感外露程度。",
            dna.meta.name,
            if genre_association.is_empty() {
                "通用".to_string()
            } else {
                genre_association
            }
        ),
        input_description: Some("故事正文或待生成段落".to_string()),
        output_description: Some("符合该 Style DNA 量化指标的文本".to_string()),
        payload: serde_json::to_value(&dna).unwrap_or_default(),
        metadata: {
            let mut m = std::collections::HashMap::new();
            if let Some(author) = &dna.meta.author {
                m.insert("author".to_string(), serde_json::Value::String(author.clone()));
            }
            m
        },
    }
}

/// 把 Workflow 转换为可选择资产
#[allow(dead_code)]
pub fn workflow_assets(workflows: &[Workflow]) -> Vec<SelectableAsset> {
    workflows
        .iter()
        .map(|wf| SelectableAsset {
            id: format!("workflow.{}", wf.id),
            kind: AssetKind::Workflow,
            name: wf.name.clone(),
            description: wf.description.clone(),
            when_to_use: format!(
                "当创作流程需要严格遵循 '{}' 定义的可视化 DAG 节点与条件边时使用。",
                wf.name
            ),
            input_description: Some("故事上下文与用户指令".to_string()),
            output_description: Some("按 workflow 节点执行后的结果".to_string()),
            payload: serde_json::to_value(wf).unwrap_or_default(),
            metadata: Default::default(),
        })
        .collect()
}

/// 把 Skill 转换为可选择资产
pub fn skill_assets(skills: &[Skill]) -> Vec<SelectableAsset> {
    skills
        .iter()
        .map(|skill| {
            let category = match skill.manifest.category {
                SkillCategory::Writing => "writing",
                SkillCategory::Analysis => "analysis",
                SkillCategory::Character => "character",
                SkillCategory::WorldBuilding => "world_building",
                SkillCategory::Style => "style",
                SkillCategory::Plot => "plot",
                SkillCategory::Export => "export",
                SkillCategory::Integration => "integration",
                SkillCategory::Custom => "custom",
            };
            SelectableAsset {
                id: skill.manifest.id.clone(),
                kind: AssetKind::Skill,
                name: skill.manifest.name.clone(),
                description: skill.manifest.description.clone(),
                when_to_use: skill
                    .manifest
                    .config
                    .get("when_to_use")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| format!("{} 类型的技能", category)),
                input_description: Some(
                    skill
                        .manifest
                        .parameters
                        .iter()
                        .map(|p| p.name.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                output_description: None,
                payload: serde_json::json!({
                    "category": category,
                    "entry_point": skill.manifest.entry_point,
                }),
                metadata: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "enabled".to_string(),
                        serde_json::Value::Bool(skill.is_enabled),
                    );
                    m
                },
            }
        })
        .collect()
}

/// 把桥段卡转换为可选择资产（v0.17.0 中文叙事增强）
pub fn beat_card_assets() -> Vec<SelectableAsset> {
    builtin_beat_cards()
        .into_iter()
        .map(beat_card_to_asset)
        .collect()
}

fn beat_card_to_asset(card: BeatCard) -> SelectableAsset {
    SelectableAsset {
        id: card.id.clone(),
        kind: AssetKind::BeatCard,
        name: card.name.clone(),
        description: card.function.clone(),
        when_to_use: format!("{} | 重构提示：{}", card.when_to_use, card.remix_hint),
        input_description: Some("故事题材、主角概念、当前章节定位".to_string()),
        output_description: Some("用于大纲与正文的叙事骨架，可单独或组合使用".to_string()),
        payload: serde_json::to_value(&card).unwrap_or_default(),
        metadata: {
            let mut m = std::collections::HashMap::new();
            m.insert(
                "category".to_string(),
                serde_json::Value::String(card.category.label().to_string()),
            );
            m.insert(
                "avoid".to_string(),
                serde_json::Value::String(card.avoid.clone()),
            );
            m
        },
    }
}

/// 把剧情引擎转换为可选择资产（v0.17.0 中文叙事增强）
pub fn story_engine_assets() -> Vec<SelectableAsset> {
    builtin_story_engines()
        .into_iter()
        .map(story_engine_to_asset)
        .collect()
}

fn story_engine_to_asset(engine: StoryEngine) -> SelectableAsset {
    SelectableAsset {
        id: engine.id.clone(),
        kind: AssetKind::StoryEngine,
        name: engine.name.clone(),
        description: engine.payoff.clone(),
        when_to_use: format!("最佳收束：{} | 反例：{}", engine.best_payoff, engine.avoid),
        input_description: Some("故事题材、主情绪与已选高压关系".to_string()),
        output_description: Some("叙事动力：可与其他 1-3 个引擎正交组合".to_string()),
        payload: serde_json::to_value(&engine).unwrap_or_default(),
        metadata: {
            let mut m = std::collections::HashMap::new();
            if !engine.pairs_well_with.is_empty() {
                m.insert(
                    "pairs_well_with".to_string(),
                    serde_json::to_value(&engine.pairs_well_with).unwrap_or_default(),
                );
            }
            m
        },
    }
}

/// 把高压关系转换为可选择资产（v0.17.0 中文叙事增强）
pub fn pressure_relationship_assets() -> Vec<SelectableAsset> {
    builtin_pressure_relationships()
        .into_iter()
        .map(pressure_relationship_to_asset)
        .collect()
}

fn pressure_relationship_to_asset(rel: PressureRelationship) -> SelectableAsset {
    SelectableAsset {
        id: rel.id.clone(),
        kind: AssetKind::PressureRelationship,
        name: rel.name.clone(),
        description: rel.pressure_source.clone(),
        when_to_use: format!("适合搭配的引擎/桥段：{}", rel.works_with.join(", ")),
        input_description: Some("主角与对手的关系定位".to_string()),
        output_description: Some("结构化高压关系，自带冲突放大机制".to_string()),
        payload: serde_json::to_value(&rel).unwrap_or_default(),
        metadata: Default::default(),
    }
}

/// 从仓库加载 genre profiles 并构建资产列表
pub fn load_assets_with_genre_profiles(
    repo: &crate::db::GenreProfileRepository,
) -> Result<Vec<SelectableAsset>, crate::error::AppError> {
    let profiles = repo.get_all().map_err(crate::error::AppError::from)?;
    let mut assets = Vec::new();
    assets.extend(methodology_assets());
    assets.extend(genre_profile_assets(&profiles));
    assets.extend(style_dna_assets());
    // v0.17.0 中文叙事增强资产
    assets.extend(beat_card_assets());
    assets.extend(story_engine_assets());
    assets.extend(pressure_relationship_assets());
    Ok(assets)
}

/// 从 SkillManager 构建完整资产列表（含技能）
pub fn load_all_assets(
    repo: &crate::db::GenreProfileRepository,
    skills: &[crate::skills::Skill],
) -> Result<Vec<SelectableAsset>, crate::error::AppError> {
    let mut assets = load_assets_with_genre_profiles(repo)?;
    assets.extend(skill_assets(skills));
    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_methodology_assets_count() {
        let assets = methodology_assets();
        assert_eq!(assets.len(), 5);
        assert!(assets.iter().any(|a| a.id == "methodology.snowflake"));
        assert!(assets
            .iter()
            .any(|a| a.id == "methodology.high_density_world_building"));
    }

    #[test]
    fn test_genre_profile_assets_mapping() {
        let profile = GenreProfile {
            id: "apocalyptic".to_string(),
            genre_name: "末世流".to_string(),
            canonical_name: "Post-apocalyptic".to_string(),
            aliases_json: Some("[\"post-apocalyptic\", \"apocalyptic\"]".to_string()),
            core_tone: Some("文明崩溃后的世界".to_string()),
            pacing_strategy: Some("快节奏".to_string()),
            anti_patterns_json: Some("[\"物资无限\"]".to_string()),
            reference_tables_json: Some("| 元素 | 比例 |".to_string()),
            typical_structure_json: Some(
                "[{\"title\": \"末日降临\", \"description\": \"...\"}]".to_string(),
            ),
            reader_promise: Some("怕,燃,生存压迫".to_string()),
            is_builtin: true,
            created_at: chrono::Local::now(),
        };

        let assets = genre_profile_assets(&[profile]);
        assert_eq!(assets.len(), 1);
        let asset = &assets[0];
        assert_eq!(asset.id, "genre_profile.apocalyptic");
        assert_eq!(asset.kind, AssetKind::GenreProfile);
        assert!(
            asset
                .payload
                .get("aliases")
                .unwrap()
                .as_array()
                .unwrap()
                .len()
                > 0
        );
        assert!(asset.payload.get("typical_structure").is_some());
    }

    #[test]
    fn test_style_dna_assets_count() {
        let assets = style_dna_assets();
        assert_eq!(assets.len(), 52);
    }

    #[test]
    fn test_skill_assets_respects_category() {
        let skills = vec![crate::skills::Skill {
            manifest: crate::skills::SkillManifest {
                id: "builtin.test".to_string(),
                name: "Test Skill".to_string(),
                version: "1.0.0".to_string(),
                description: "A test skill".to_string(),
                author: "test".to_string(),
                category: crate::skills::SkillCategory::Writing,
                entry_point: "test".to_string(),
                parameters: vec![],
                capabilities: vec![],
                hooks: vec![],
                config: std::collections::HashMap::new(),
            },
            path: std::path::PathBuf::from("builtin"),
            is_enabled: true,
            loaded_at: chrono::Utc::now(),
            runtime: crate::skills::SkillRuntime::Prompt(crate::skills::PromptRuntime {
                system_prompt: "test".to_string(),
                user_prompt_template: "test".to_string(),
            }),
        }];
        let assets = skill_assets(&skills);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].payload["category"], "writing");
    }

    #[test]
    fn test_load_all_assets_integration() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let repo = crate::db::GenreProfileRepository::new(pool);
        let _ = repo.create(
            "测试体裁",
            "Test Genre",
            Some("[\"test\"]"),
            Some("核心基调"),
            Some("节奏策略"),
            Some("[\"反套路\"]"),
            Some("参考表"),
            Some("[{\"title\": \"第一章\", \"description\": \"...\"}]"),
        );

        let assets = load_all_assets(&repo, &[]).unwrap();
        // v0.17.0 新增：beat_cards (>=30) + story_engines (21) + pressure_relationships
        // (13)
        assert!(assets.len() >= 5 + 1 + 52 + 30 + 21 + 13);
        assert!(assets.iter().any(|a| a.name == "测试体裁"));
    }
}
