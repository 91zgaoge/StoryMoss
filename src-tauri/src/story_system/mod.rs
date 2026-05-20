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

use crate::db::{DbPool, StoryContractRepository, SceneCommitRepository};
use crate::vector::lancedb_store::{LanceVectorStore, VectorRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

pub mod contract_builder;
pub mod preflight;
pub mod projection_writers;

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

        let json = serde_json::to_string(&contract)
            .map_err(|e| format!("序列化合同失败: {}", e))?;

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

        let json = serde_json::to_string(&contract)
            .map_err(|e| format!("序列化合同失败: {}", e))?;

        let repo = StoryContractRepository::new(self.pool.clone());
        repo.create(story_id, "CHAPTER", &json)
            .map_err(|e| format!("创建合同失败: {}", e))
    }

    /// 获取故事的合同树
    pub fn get_contract_tree(&self, story_id: &str) -> Result<ContractTree, String> {
        let repo = StoryContractRepository::new(self.pool.clone());
        let contracts = repo.get_by_story(story_id)
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

        let master = tree.master_setting
            .ok_or_else(|| "缺少 MASTER_SETTING 合同".to_string())?;

        // 查找章节合同
        let chapter_contract = tree.chapters.values()
            .find(|c| {
                if let Ok(cc) = serde_json::from_str::<ChapterContract>(&c.contract_json) {
                    cc.chapter_number == chapter_number
                } else {
                    false
                }
            })
            .cloned();

        Ok(RuntimeContract {
            master_setting: master,
            chapter_contract,
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

/// 运行时合同（写前加载）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeContract {
    pub master_setting: crate::db::StoryContract,
    pub chapter_contract: Option<crate::db::StoryContract>,
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

/// SCENE_COMMIT 服务
pub struct SceneCommitService {
    pool: DbPool,
}

impl SceneCommitService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 初始化 commit（写作前）
    pub fn init_commit(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        chapter_number: i32,
    ) -> Result<crate::db::SceneCommit, String> {
        let repo = SceneCommitRepository::new(self.pool.clone());
        repo.create(story_id, scene_id, chapter_id, chapter_number, "pending")
            .map_err(|e| format!("初始化 commit 失败: {}", e))
    }

    /// 自动 commit（update_chapter 30s debounce 后触发）
    ///
    /// 简化入口：自动 init_commit + apply_commit，使用空占位符填充尚未实现的
    /// review/fulfillment 字段（W3-B4/B5 落地后替换为真实数据）。
    pub async fn auto_commit(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        chapter_number: i32,
        content: Option<&str>,
        app_handle: Option<tauri::AppHandle>,
        vector_store: Option<&LanceVectorStore>,
    ) -> Result<(), String> {
        let commit = self.init_commit(story_id, scene_id, chapter_id, chapter_number)?;
        let summary = content.unwrap_or("").chars().take(1000).collect::<String>();

        self.apply_commit(
            &commit.id,
            "{}",
            "{}",
            "{}",
            "{}",
            "{}",
            "{}",
            &summary,
            "",
            content,
            app_handle,
            vector_store,
        ).await
    }

    /// 提交 accepted commit（异步，含投影写入）
    ///
    /// W2-B6: 已吸收 auto_ingest 功能。若提供 `chapter_content`，
    /// 自动执行知识图谱提取和完整内容向量索引。
    pub async fn apply_commit(
        &self,
        commit_id: &str,
        outline_snapshot_json: &str,
        review_result_json: &str,
        fulfillment_result_json: &str,
        accepted_events_json: &str,
        state_deltas_json: &str,
        entity_deltas_json: &str,
        summary_text: &str,
        dominant_strand: &str,
        chapter_content: Option<&str>,
        app_handle: Option<tauri::AppHandle>,
        vector_store: Option<&LanceVectorStore>,
    ) -> Result<(), String> {
        let repo = SceneCommitRepository::new(self.pool.clone());

        // 先更新 commit 状态
        repo.update_commit(
            commit_id,
            "accepted",
            Some(outline_snapshot_json),
            Some(review_result_json),
            Some(fulfillment_result_json),
            Some(accepted_events_json),
            Some(state_deltas_json),
            Some(entity_deltas_json),
            Some(summary_text),
            Some(dominant_strand),
            None,
        ).map_err(|e| format!("更新 commit 失败: {}", e))?;

        // 获取 commit 所属 story_id 和 chapter_number
        let commit = repo.get_by_id(commit_id)
            .map_err(|e| format!("查询 commit 失败: {}", e))?
            .ok_or_else(|| "Commit 不存在".to_string())?;

        let story_id = commit.story_id.clone();
        let chapter_number = commit.chapter_number;

        // 构建 commit JSON 供 projection writers 使用
        let commit_json = serde_json::json!({
            "state_deltas_json": state_deltas_json,
            "entity_deltas_json": entity_deltas_json,
            "accepted_events_json": accepted_events_json,
            "summary_text": summary_text,
        }).to_string();

        // W2-B7: 执行同步 projection writers（带性能测量）
        let writers = projection_writers::get_projection_writers(self.pool.clone());
        let mut projection_status = serde_json::json!({
            "state": "pending",
            "index": "pending",
            "summary": "pending",
            "memory": "pending",
            "vector": "pending",
            "kg": "pending",
        });

        let sync_start = Instant::now();
        for writer in writers {
            let name = writer.name();
            let w_start = Instant::now();
            match writer.apply(&story_id, chapter_number, &commit_json) {
                Ok(true) => {
                    projection_status[name] = serde_json::json!("success");
                }
                Ok(false) => {
                    projection_status[name] = serde_json::json!("skipped");
                }
                Err(e) => {
                    projection_status[name] = serde_json::json!(format!("error: {}", e));
                }
            }
            log::info!("[ProjectionWriter] {} sync completed in {}ms", name, w_start.elapsed().as_millis());
        }
        let sync_elapsed = sync_start.elapsed().as_millis();
        log::info!("[ProjectionWriter] All sync writers completed in {}ms", sync_elapsed);

        // W2-B7: 向量投影 + 知识图谱提取并行执行（两者都是异步且独立）
        let async_start = Instant::now();
        let vector_future = async {
            if let Some(store) = vector_store {
                let text = chapter_content.unwrap_or(summary_text);
                let vector_text = if text.is_empty() {
                    format!("第{}章", chapter_number)
                } else {
                    format!("第{}章: {}", chapter_number, text.chars().take(500).collect::<String>())
                };
                match crate::embeddings::embedding::embed_text_async(vector_text.clone()).await {
                    Ok(embedding) => {
                        let record = VectorRecord {
                            id: format!("{}_ch{}", story_id, chapter_number),
                            story_id: story_id.clone(),
                            chapter_id: String::new(),
                            chapter_number,
                            text: vector_text,
                            record_type: "chapter".to_string(),
                            embedding,
                        };
                        match store.add_record(record).await {
                            Ok(_) => "success".to_string(),
                            Err(e) => format!("error: {}", e),
                        }
                    }
                    Err(e) => format!("embedding_error: {}", e),
                }
            } else {
                "skipped: no_store".to_string()
            }
        };

        let kg_future = async {
            if let (Some(content), Some(app)) = (chapter_content, app_handle) {
                if content.len() >= 20 {
                    match self.run_kg_ingest(&story_id, chapter_number, content, &app).await {
                        Ok(true) => "success".to_string(),
                        Ok(false) => "skipped".to_string(),
                        Err(e) => format!("error: {}", e),
                    }
                } else {
                    "skipped: content_too_short".to_string()
                }
            } else {
                "skipped: no_content_or_app".to_string()
            }
        };

        let (vector_status, kg_status) = futures::future::join(vector_future, kg_future).await;
        projection_status["vector"] = serde_json::json!(vector_status);
        projection_status["kg"] = serde_json::json!(kg_status);

        let async_elapsed = async_start.elapsed().as_millis();
        log::info!("[ProjectionWriter] Async projections (vector + kg) completed in {}ms", async_elapsed);

        let total_elapsed = sync_start.elapsed().as_millis();
        log::info!("[ProjectionWriter] Total apply_commit completed in {}ms", total_elapsed);
        if total_elapsed > 2000 {
            log::warn!("[ProjectionWriter] Total time {}ms exceeds 2s threshold. Consider further parallelizing sync writers.", total_elapsed);
        }

        // 更新投影状态
        repo.update_projection_status(commit_id, &projection_status.to_string())
            .map_err(|e| format!("更新投影状态失败: {}", e))?;

        Ok(())
    }

    /// 运行知识图谱提取（原 auto_ingest 的 IngestPipeline 逻辑）
    async fn run_kg_ingest(
        &self,
        story_id: &str,
        chapter_number: i32,
        content: &str,
        app_handle: &tauri::AppHandle,
    ) -> Result<bool, String> {
        let llm_service = crate::llm::LlmService::new(app_handle.clone());
        let pipeline = crate::memory::ingest::IngestPipeline::new(llm_service)
            .with_pool(self.pool.clone())
            .with_app_handle(app_handle.clone());
        let ingest_content = crate::memory::ingest::IngestContent {
            text: content.to_string(),
            source: format!("chapter:{}:{}", story_id, chapter_number),
            story_id: story_id.to_string(),
            scene_id: None,
        };

        match pipeline.ingest(&ingest_content).await {
            Ok(result) => {
                let kg_repo = crate::db::repositories_v3::KnowledgeGraphRepository::new(self.pool.clone());
                let entity_count = result.entities.len();
                let relation_count = result.relations.len();
                match kg_repo.save_entities_batch(&result.entities) {
                    Ok(saved) => log::info!("[SceneCommitService] Saved {}/{} entities for story {}", saved, entity_count, story_id),
                    Err(e) => log::warn!("[SceneCommitService] Failed to save entities: {}", e),
                }
                match kg_repo.save_relations_batch(&result.relations) {
                    Ok(saved) => log::info!("[SceneCommitService] Saved {}/{} relations for story {}", saved, relation_count, story_id),
                    Err(e) => log::warn!("[SceneCommitService] Failed to save relations: {}", e),
                }
                Ok(true)
            }
            Err(e) => {
                log::warn!("[SceneCommitService] IngestPipeline failed for story {}: {}", story_id, e);
                Err(e.to_string())
            }
        }
    }
}

// ==================== 投影健康检查 ====================

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

impl StorySystemEngine {
    /// 检查指定章节的投影一致性健康状态
    pub fn check_projection_health(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<ProjectionHealthReport, String> {
        let repo = SceneCommitRepository::new(self.pool.clone());

        // 查询该 story 的所有 commits，找到匹配 chapter_number 的最新一条
        let commits = repo.get_by_story(story_id)
            .map_err(|e| format!("查询 commit 失败: {}", e))?;

        let commit = commits.into_iter()
            .find(|c| c.chapter_number == chapter_number)
            .ok_or_else(|| format!("章节 {} 无提交记录", chapter_number))?;

        let projection_status_json = commit.projection_status_json
            .unwrap_or_else(|| r#"{"state":"unknown","index":"unknown","summary":"unknown","memory":"unknown","vector":"unknown"}"#.to_string());

        let status: serde_json::Value = serde_json::from_str(&projection_status_json)
            .unwrap_or_else(|_| serde_json::json!({
                "state": "unknown",
                "index": "unknown",
                "summary": "unknown",
                "memory": "unknown",
                "vector": "unknown",
            }));

        let writer_names = ["state", "index", "summary", "memory", "vector"];
        let mut writers = Vec::new();
        let mut overall_healthy = true;

        for name in &writer_names {
            let status_str = status.get(*name)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let is_ok = status_str == "success" || status_str == "skipped" || status_str == "skipped: no_store";
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
