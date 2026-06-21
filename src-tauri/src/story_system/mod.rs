#![allow(dead_code)]
//! Story System - 合同驱动体系
//!
//! 参考 webnovel-writer 的 Story System Phase 5 设计：
//! - 写前真源：story_contracts 表
//! - 写后真源：scene_commits 表
//! - 投影/read-model：state.json, index.db, summaries
//!
//! 防幻觉三定律：
//! 1. 大纲即法律 — 遵循合同约束
//! 2. 设定即物理 — 不违反已有规则
//! 3. 发明需识别 — 新实体必须入库

pub mod auto_contract;
pub mod chapter_service;
pub mod commit_service;
pub mod contract_builder;
pub mod contract_service;
pub mod fulfillment_checker;
pub mod mini_review;
pub mod preflight;
pub mod projection_writers;
pub mod scene_service;

// Re-export public types used by external code.
#[allow(unused_imports)]
pub use commit_service::SceneCommitService;
#[allow(unused_imports)]
pub use contract_service::{ContractTree, ProjectionHealthReport, StorySystemEngine, WriterHealth};

#[allow(unused_imports)]
pub use crate::domain::contracts::{
    ChapterContract, ChapterDirective, ContractType, MasterSettingContract, RuntimeContract,
};

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_contract_to_constraint_vars_parses_master_and_chapter() {
        let master = MasterSettingContract {
            schema_version: "1".to_string(),
            contract_type: "MASTER_SETTING".to_string(),
            generator_version: "0.22.5".to_string(),
            genre: "玄幻".to_string(),
            core_tone: "黑暗压抑".to_string(),
            pacing_strategy: "慢热铺陈".to_string(),
            anti_patterns: vec!["系统流".to_string(), "无敌流".to_string()],
            world_rules: vec!["修炼者不可飞行".to_string(), "灵气不可再生".to_string()],
        };
        let chapter = ChapterContract {
            schema_version: "1".to_string(),
            contract_type: "CHAPTER".to_string(),
            generator_version: "0.22.5".to_string(),
            chapter_number: 1,
            chapter_directive: ChapterDirective {
                goal: "主角发现灵气枯竭真相".to_string(),
                must_cover_nodes: vec!["主角出场".to_string(), "灵气异常".to_string()],
                forbidden_zones: vec!["提前揭示反派身份".to_string()],
                time_anchor: Some("清晨".to_string()),
                chapter_span: Some("青云镇".to_string()),
            },
        };

        let rc = RuntimeContract {
            master_setting: master,
            chapter_contract: Some(chapter),
        };
        let vars = rc.to_constraint_vars();

        assert_eq!(vars.get("core_tone").unwrap(), "黑暗压抑");
        assert_eq!(vars.get("pacing_strategy").unwrap(), "慢热铺陈");
        assert!(vars.get("world_rules").unwrap().contains("修炼者不可飞行"));
        assert_eq!(vars.get("chapter_goal").unwrap(), "主角发现灵气枯竭真相");
        assert!(vars.get("must_cover_nodes").unwrap().contains("灵气异常"));
        assert!(vars
            .get("forbidden_zones")
            .unwrap()
            .contains("提前揭示反派身份"));
    }

    #[test]
    fn runtime_contract_to_constraint_vars_handles_missing_chapter() {
        let master = MasterSettingContract {
            schema_version: "1".to_string(),
            contract_type: "MASTER_SETTING".to_string(),
            generator_version: "0.22.5".to_string(),
            genre: "都市".to_string(),
            core_tone: "轻松日常".to_string(),
            pacing_strategy: "轻快".to_string(),
            anti_patterns: vec![],
            world_rules: vec![],
        };

        let rc = RuntimeContract {
            master_setting: master,
            chapter_contract: None,
        };
        let vars = rc.to_constraint_vars();

        assert_eq!(vars.get("chapter_goal").unwrap(), "（未指定）");
        assert_eq!(vars.get("must_cover_nodes").unwrap(), "无");
        assert_eq!(vars.get("forbidden_zones").unwrap(), "无");
    }
}
