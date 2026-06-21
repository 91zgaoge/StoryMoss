use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 合同类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractType {
    MasterSetting,
    Volume,
    Chapter,
    Review,
}

impl std::fmt::Display for ContractType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ContractType::MasterSetting => "MASTER_SETTING",
            ContractType::Volume => "VOLUME",
            ContractType::Chapter => "CHAPTER",
            ContractType::Review => "REVIEW",
        };
        write!(f, "{}", s)
    }
}

/// MASTER_SETTING 合同结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterSettingContract {
    #[serde(rename = "schema_version")]
    pub schema_version: String,
    #[serde(rename = "contract_type")]
    pub contract_type: String,
    #[serde(rename = "generator_version")]
    pub generator_version: String,
    pub genre: String,
    #[serde(rename = "core_tone")]
    pub core_tone: String,
    #[serde(rename = "pacing_strategy")]
    pub pacing_strategy: String,
    #[serde(rename = "anti_patterns")]
    pub anti_patterns: Vec<String>,
    #[serde(rename = "world_rules")]
    pub world_rules: Vec<String>,
}

/// 章节合同结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterContract {
    #[serde(rename = "schema_version")]
    pub schema_version: String,
    #[serde(rename = "contract_type")]
    pub contract_type: String,
    #[serde(rename = "generator_version")]
    pub generator_version: String,
    #[serde(rename = "chapter_number")]
    pub chapter_number: i32,
    #[serde(rename = "chapter_directive")]
    pub chapter_directive: ChapterDirective,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterDirective {
    pub goal: String,
    #[serde(rename = "must_cover_nodes")]
    pub must_cover_nodes: Vec<String>,
    #[serde(rename = "forbidden_zones")]
    pub forbidden_zones: Vec<String>,
    #[serde(rename = "time_anchor")]
    pub time_anchor: Option<String>,
    #[serde(rename = "chapter_span")]
    pub chapter_span: Option<String>,
}

/// 运行时合同（写前加载）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeContract {
    pub master_setting: MasterSettingContract,
    pub chapter_contract: Option<ChapterContract>,
}

impl RuntimeContract {
    /// 将合同转换为 prompt 模板变量表。
    /// 配合 PromptRegistry 中的 writer_contract_constraints /
    /// inspector_contract_compliance / write_time_bundle_contract /
    /// review_contract_criteria / refine_contract_criteria 使用。
    pub fn to_constraint_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        let master = &self.master_setting;

        vars.insert("core_tone".to_string(), master.core_tone.clone());
        vars.insert(
            "pacing_strategy".to_string(),
            master.pacing_strategy.clone(),
        );
        vars.insert(
            "world_rules".to_string(),
            if master.world_rules.is_empty() {
                "无".to_string()
            } else {
                master
                    .world_rules
                    .iter()
                    .enumerate()
                    .map(|(i, r)| format!("{}. {}", i + 1, r))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
        );

        if let Some(ref ch) = self.chapter_contract {
            vars.insert(
                "chapter_goal".to_string(),
                ch.chapter_directive.goal.clone(),
            );
            vars.insert(
                "must_cover_nodes".to_string(),
                if ch.chapter_directive.must_cover_nodes.is_empty() {
                    "无".to_string()
                } else {
                    ch.chapter_directive
                        .must_cover_nodes
                        .iter()
                        .enumerate()
                        .map(|(i, n)| format!("{}. {}", i + 1, n))
                        .collect::<Vec<_>>()
                        .join("\n")
                },
            );
            vars.insert(
                "forbidden_zones".to_string(),
                if ch.chapter_directive.forbidden_zones.is_empty() {
                    "无".to_string()
                } else {
                    ch.chapter_directive
                        .forbidden_zones
                        .iter()
                        .enumerate()
                        .map(|(i, n)| format!("{}. {}", i + 1, n))
                        .collect::<Vec<_>>()
                        .join("\n")
                },
            );
            return vars;
        }

        vars.insert("chapter_goal".to_string(), "（未指定）".to_string());
        vars.insert("must_cover_nodes".to_string(), "无".to_string());
        vars.insert("forbidden_zones".to_string(), "无".to_string());
        vars
    }
}
