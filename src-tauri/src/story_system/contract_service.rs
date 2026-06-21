use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    db::{DbPool, SceneCommitRepository, StoryContractRepository},
    domain::contracts::*,
};

/// Story System 引擎
pub struct StorySystemEngine {
    pool: DbPool,
}

impl StorySystemEngine {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 创建 MASTER_SETTING 合同
    pub fn create_master_setting(
        &self,
        story_id: &str,
        genre: &str,
        core_tone: &str,
        pacing_strategy: &str,
        anti_patterns: &[String],
        world_rules: &[String],
    ) -> Result<crate::db::StoryContract, String> {
        let contract = MasterSettingContract {
            schema_version: "story-system/v1".to_string(),
            contract_type: "MASTER_SETTING".to_string(),
            generator_version: "v6.0.0".to_string(),
            genre: genre.to_string(),
            core_tone: core_tone.to_string(),
            pacing_strategy: pacing_strategy.to_string(),
            anti_patterns: anti_patterns.to_vec(),
            world_rules: world_rules.to_vec(),
        };

        let json =
            serde_json::to_string(&contract).map_err(|e| format!("序列化合同失败: {}", e))?;

        let repo = StoryContractRepository::new(self.pool.clone());
        repo.create(story_id, "MASTER_SETTING", &json)
            .map_err(|e| format!("创建合同失败: {}", e))
    }

    /// 创建章节合同
    pub fn create_chapter_contract(
        &self,
        story_id: &str,
        chapter_number: i32,
        goal: &str,
        must_cover_nodes: &[String],
        forbidden_zones: &[String],
        time_anchor: Option<&str>,
        chapter_span: Option<&str>,
    ) -> Result<crate::db::StoryContract, String> {
        let contract = ChapterContract {
            schema_version: "story-system/v1".to_string(),
            contract_type: "CHAPTER".to_string(),
            generator_version: "v6.0.0".to_string(),
            chapter_number,
            chapter_directive: ChapterDirective {
                goal: goal.to_string(),
                must_cover_nodes: must_cover_nodes.to_vec(),
                forbidden_zones: forbidden_zones.to_vec(),
                time_anchor: time_anchor.map(|s| s.to_string()),
                chapter_span: chapter_span.map(|s| s.to_string()),
            },
        };

        let json =
            serde_json::to_string(&contract).map_err(|e| format!("序列化合同失败: {}", e))?;

        let repo = StoryContractRepository::new(self.pool.clone());
        repo.create(story_id, "CHAPTER", &json)
            .map_err(|e| format!("创建合同失败: {}", e))
    }

    /// 获取故事的合同树
    pub fn get_contract_tree(&self, story_id: &str) -> Result<ContractTree, String> {
        let repo = StoryContractRepository::new(self.pool.clone());
        let contracts = repo
            .get_by_story(story_id)
            .map_err(|e| format!("查询合同失败: {}", e))?;

        let mut tree = ContractTree {
            master_setting: None,
            volumes: HashMap::new(),
            chapters: HashMap::new(),
            reviews: HashMap::new(),
        };

        for contract in contracts {
            match contract.contract_type.as_str() {
                "MASTER_SETTING" => {
                    tree.master_setting = Some(contract);
                }
                "VOLUME" => {
                    tree.volumes.insert(contract.id.clone(), contract);
                }
                "CHAPTER" => {
                    tree.chapters.insert(contract.id.clone(), contract);
                }
                "REVIEW" => {
                    tree.reviews.insert(contract.id.clone(), contract);
                }
                _ => {}
            }
        }

        Ok(tree)
    }

    /// 获取指定章节的运行时合同
    pub fn get_runtime_contract(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<RuntimeContract, String> {
        let tree = self.get_contract_tree(story_id)?;

        let master_db = tree
            .master_setting
            .ok_or_else(|| "缺少 MASTER_SETTING 合同".to_string())?;

        let master_setting: MasterSettingContract = serde_json::from_str(&master_db.contract_json)
            .map_err(|e| format!("解析 MASTER_SETTING 合同失败: {}", e))?;

        // 查找章节合同
        let chapter_db = tree.chapters.values().find(|c| {
            if let Ok(cc) = serde_json::from_str::<ChapterContract>(&c.contract_json) {
                cc.chapter_number == chapter_number
            } else {
                false
            }
        });

        let chapter_contract = chapter_db
            .map(|c| serde_json::from_str::<ChapterContract>(&c.contract_json))
            .transpose()
            .map_err(|e| format!("解析 CHAPTER 合同失败: {}", e))?;

        Ok(RuntimeContract {
            master_setting,
            chapter_contract,
        })
    }

    /// 检查指定章节的投影一致性健康状态
    pub fn check_projection_health(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<ProjectionHealthReport, String> {
        let repo = SceneCommitRepository::new(self.pool.clone());

        // 查询该 story 的所有 commits，找到匹配 chapter_number 的最新一条
        let commits = repo
            .get_by_story(story_id)
            .map_err(|e| format!("查询 commit 失败: {}", e))?;

        let commit = commits
            .into_iter()
            .find(|c| c.chapter_number == chapter_number)
            .ok_or_else(|| format!("章节 {} 无提交记录", chapter_number))?;

        let projection_status_json = commit.projection_status_json
            .unwrap_or_else(|| r#"{"state":"unknown","index":"unknown","summary":"unknown","memory":"unknown","vector":"unknown"}"#.to_string());

        let status: serde_json::Value = serde_json::from_str(&projection_status_json)
            .unwrap_or_else(|_| {
                serde_json::json!({
                    "state": "unknown",
                    "index": "unknown",
                    "summary": "unknown",
                    "memory": "unknown",
                    "vector": "unknown",
                })
            });

        let writer_names = ["state", "index", "summary", "memory", "vector"];
        let mut writers = Vec::new();
        let mut overall_healthy = true;

        for name in &writer_names {
            let status_str = status
                .get(*name)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let is_ok = status_str == "success"
                || status_str == "skipped"
                || status_str == "skipped: no_store";
            if !is_ok {
                overall_healthy = false;
            }
            writers.push(WriterHealth {
                name: name.to_string(),
                status: status_str.to_string(),
            });
        }

        Ok(ProjectionHealthReport {
            story_id: story_id.to_string(),
            chapter_number,
            commit_id: commit.id,
            overall_healthy,
            writers,
        })
    }
}

/// 合同树
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTree {
    pub master_setting: Option<crate::db::StoryContract>,
    pub volumes: HashMap<String, crate::db::StoryContract>,
    pub chapters: HashMap<String, crate::db::StoryContract>,
    pub reviews: HashMap<String, crate::db::StoryContract>,
}

/// 单个 projection writer 的健康状态
#[derive(Debug, Clone, Serialize)]
pub struct WriterHealth {
    pub name: String,
    pub status: String, // "success" | "skipped" | "error: ..." | "pending" | "unknown"
}

/// 投影一致性健康报告
#[derive(Debug, Clone, Serialize)]
pub struct ProjectionHealthReport {
    pub story_id: String,
    pub chapter_number: i32,
    pub commit_id: String,
    pub overall_healthy: bool,
    pub writers: Vec<WriterHealth>,
}
