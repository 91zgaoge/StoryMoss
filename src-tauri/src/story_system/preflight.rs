//! Preflight - 写前校验
//!
//! 检查合同完整性、大纲结构化、blocking issues

use crate::db::{DbPool, StoryContractRepository, SceneRepository, CharacterRepository};

/// 校验结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreflightResult {
    pub ready: bool,
    pub missing_contracts: Vec<String>,
    pub warnings: Vec<String>,
    pub blocking_issues: Vec<String>,
}

/// 写前校验器
pub struct PreflightChecker;

impl PreflightChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check(
        &self,
        pool: &DbPool,
        story_id: &str,
        chapter_number: i32,
    ) -> PreflightResult {
        let mut missing_contracts = Vec::new();
        let mut warnings = Vec::new();
        let mut blocking_issues = Vec::new();

        // 1. 检查 MASTER_SETTING 合同
        let contract_repo = StoryContractRepository::new(pool.clone());
        match contract_repo.get_by_story(story_id) {
            Ok(contracts) => {
                let has_master = contracts.iter().any(|c| c.contract_type == "MASTER_SETTING");
                let has_chapter = contracts.iter().any(|c| {
                    if c.contract_type != "CHAPTER" {
                        return false;
                    }
                    // 从 contract_json 中解析 chapter_number
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&c.contract_json) {
                        json.get("chapter_number")
                            .and_then(|v| v.as_i64())
                            .map(|n| n as i32 == chapter_number)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                });

                if !has_master {
                    missing_contracts.push("MASTER_SETTING".to_string());
                    blocking_issues.push(
                        format!("故事 [{}] 缺少世界观合同 (MASTER_SETTING)，请先创建世界观设定", story_id)
                    );
                }
                if !has_chapter {
                    missing_contracts.push(format!("CHAPTER_{}", chapter_number));
                    blocking_issues.push(
                        format!("第 {} 章缺少章节合同，请先创建章节合同", chapter_number)
                    );
                }
            }
            Err(e) => {
                warnings.push(format!("查询合同时出错: {}", e));
            }
        }

        // 2. 检查角色列表是否非空
        let char_repo = CharacterRepository::new(pool.clone());
        match char_repo.get_by_story(story_id) {
            Ok(characters) => {
                if characters.is_empty() {
                    blocking_issues.push("故事中没有角色，请先创建至少一个角色".to_string());
                } else if characters.len() < 2 {
                    warnings.push("故事中角色较少（<2），建议增加角色以丰富互动".to_string());
                }
            }
            Err(e) => {
                warnings.push(format!("查询角色时出错: {}", e));
            }
        }

        // 3. 检查当前 scene 是否有 outline
        let scene_repo = SceneRepository::new(pool.clone());
        match scene_repo.get_by_story(story_id) {
            Ok(scenes) => {
                let scene = scenes.iter().find(|s| s.sequence_number == chapter_number);
                if let Some(s) = scene {
                    let has_outline = s.outline_content.as_ref().map(|o| !o.trim().is_empty()).unwrap_or(false);
                    if !has_outline {
                        blocking_issues.push(
                            format!("第 {} 章 (scene_id: {}) 缺少大纲，请先编写场景大纲", chapter_number, s.id)
                        );
                    }
                } else {
                    blocking_issues.push(
                        format!("第 {} 章的场景不存在，请先创建场景", chapter_number)
                    );
                }
            }
            Err(e) => {
                warnings.push(format!("查询场景时出错: {}", e));
            }
        }

        let ready = blocking_issues.is_empty();

        PreflightResult {
            ready,
            missing_contracts,
            warnings,
            blocking_issues,
        }
    }
}

impl Default for PreflightChecker {
    fn default() -> Self {
        Self::new()
    }
}
