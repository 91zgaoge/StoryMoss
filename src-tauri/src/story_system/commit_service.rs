use std::time::Instant;

use crate::{
    db::{ChapterReadingPowerRepository, ChaseDebtRepository, DbPool, SceneCommitRepository},
    domain::contracts::*,
    story_system::{fulfillment_checker, mini_review, projection_writers},
    vector::lancedb_store::{LanceVectorStore, VectorRecord},
};

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
    /// Phase 2.1: 使用真实 review/fulfillment/KG 数据填充 apply_commit 参数，
    /// 不再使用 "{}" 占位符。LLM 不可用时自动回退到启发式评分。
    pub async fn auto_commit(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        chapter_number: i32,
        content: Option<&str>,
        llm_service: Option<&crate::llm::service::LlmService>,
        app_handle: Option<tauri::AppHandle>,
        vector_store: Option<&LanceVectorStore>,
    ) -> Result<(), String> {
        let commit = self.init_commit(story_id, scene_id, chapter_id, chapter_number)?;
        let summary = content.unwrap_or("").chars().take(1000).collect::<String>();

        // 加载运行时合同；失败时使用空合同，保证 commit 不阻塞
        let engine = super::StorySystemEngine::new(self.pool.clone());
        let contract = engine
            .get_runtime_contract(story_id, chapter_number)
            .unwrap_or_else(|e| {
                log::warn!(
                    "[SceneCommitService] 加载运行时合同失败（非阻塞）: story={} chapter={} err={}",
                    story_id,
                    chapter_number,
                    e
                );
                RuntimeContract {
                    master_setting: MasterSettingContract {
                        schema_version: "story-system/v1".to_string(),
                        contract_type: "MASTER_SETTING".to_string(),
                        generator_version: "v0.0.0".to_string(),
                        genre: String::new(),
                        core_tone: String::new(),
                        pacing_strategy: String::new(),
                        anti_patterns: Vec::new(),
                        world_rules: Vec::new(),
                    },
                    chapter_contract: None,
                }
            });

        // 若调用方未提供 LlmService，但存在 AppHandle，则构造一个
        let local_llm_service;
        let llm_ref = if llm_service.is_none() {
            if let Some(ref app) = app_handle {
                local_llm_service = crate::llm::LlmService::new(app.clone());
                Some(&local_llm_service)
            } else {
                None
            }
        } else {
            llm_service
        };

        // Mini review（LLM 失败自动回退启发式）
        let review_result = mini_review::run_mini_review(content.unwrap_or(""), &contract, llm_ref)
            .await
            .unwrap_or_else(|e| {
                log::warn!("[SceneCommitService] mini review 失败（非阻塞）: {}", e);
                mini_review::heuristic_review(content.unwrap_or(""), &contract)
            });

        // 合同履行度检查
        let fulfillment_result =
            fulfillment_checker::evaluate_contract_fulfillment(content.unwrap_or(""), &contract);

        // 知识图谱提取，用于生成 state/entity deltas 与 narrative events
        let (entities, relations, narrative_events) =
            if let (Some(ref app), Some(content_text)) = (app_handle.as_ref(), content) {
                match self
                    .run_kg_ingest(story_id, chapter_number, content_text, app)
                    .await
                {
                    Ok(result) => (result.entities, result.relations, result.narrative_events),
                    Err(e) => {
                        log::warn!(
                            "[SceneCommitService] auto_commit KG 提取失败（非阻塞）: {}",
                            e
                        );
                        (Vec::new(), Vec::new(), Vec::new())
                    }
                }
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };

        let outline_snapshot_json = serde_json::json!({}).to_string();

        let review_result_json = serde_json::to_string(&review_result)
            .map_err(|e| format!("序列化 review 失败: {}", e))?;
        let fulfillment_result_json = serde_json::to_string(&fulfillment_result)
            .map_err(|e| format!("序列化 fulfillment 失败: {}", e))?;

        let accepted_events_json =
            serde_json::to_string(&narrative_events).unwrap_or_else(|_| "[]".to_string());

        let state_deltas: Vec<serde_json::Value> = entities
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "name": e.name,
                    "entity_type": e.entity_type,
                    "attributes": e.attributes,
                })
            })
            .collect();
        let state_deltas_json =
            serde_json::to_string(&state_deltas).unwrap_or_else(|_| "[]".to_string());

        let entity_deltas: Vec<serde_json::Value> = relations
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "source_id": r.source_id,
                    "target_id": r.target_id,
                    "relation_type": r.relation_type,
                    "strength": r.strength,
                })
            })
            .collect();
        let entity_deltas_json =
            serde_json::to_string(&entity_deltas).unwrap_or_else(|_| "[]".to_string());

        self.apply_commit(
            &commit.id,
            &outline_snapshot_json,
            &review_result_json,
            &fulfillment_result_json,
            &accepted_events_json,
            &state_deltas_json,
            &entity_deltas_json,
            &summary,
            "",
            content,
            app_handle,
            vector_store,
        )
        .await
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
        )
        .map_err(|e| format!("更新 commit 失败: {}", e))?;

        // 获取 commit 所属 story_id 和 chapter_number
        let commit = repo
            .get_by_id(commit_id)
            .map_err(|e| format!("查询 commit 失败: {}", e))?
            .ok_or_else(|| "Commit 不存在".to_string())?;

        let story_id = commit.story_id.clone();
        let chapter_number = commit.chapter_number;

        // v0.22.5: Phase C - 提交后追读力评估与债务清算
        // 失败不阻塞提交流程，仅记录 warning
        if let Err(e) = self.evaluate_and_reconcile_reading_power(&story_id, chapter_number) {
            log::warn!(
                "[SceneCommitService] 追读力评估失败（非阻塞）: story={} chapter={} err={}",
                story_id,
                chapter_number,
                e
            );
        }

        // 构建 commit JSON 供 projection writers 使用
        let commit_json = serde_json::json!({
            "state_deltas_json": state_deltas_json,
            "entity_deltas_json": entity_deltas_json,
            "accepted_events_json": accepted_events_json,
            "summary_text": summary_text,
        })
        .to_string();

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
            log::info!(
                "[ProjectionWriter] {} sync completed in {}ms",
                name,
                w_start.elapsed().as_millis()
            );
        }
        let sync_elapsed = sync_start.elapsed().as_millis();
        log::info!(
            "[ProjectionWriter] All sync writers completed in {}ms",
            sync_elapsed
        );

        // W2-B7: 向量投影 + 知识图谱提取并行执行（两者都是异步且独立）
        let async_start = Instant::now();
        let vector_future = async {
            if let Some(store) = vector_store {
                let text = chapter_content.unwrap_or(summary_text);
                let vector_text = if text.is_empty() {
                    format!("第{}章", chapter_number)
                } else {
                    format!(
                        "第{}章: {}",
                        chapter_number,
                        text.chars().take(500).collect::<String>()
                    )
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
                            metadata: None,
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

        // 若 auto_commit 已经提供 entity_deltas_json（非占位符），则跳过重复 KG 提取，
        // 但仍触发 knowledgeGraph 刷新与叙事分析流水线。
        let has_precomputed_deltas =
            !entity_deltas_json.trim().is_empty() && entity_deltas_json.trim() != "{}";

        let kg_future = async {
            if has_precomputed_deltas {
                if let Some(ref app) = app_handle {
                    let _ = crate::state_sync::StateSync::emit_data_refresh(
                        app,
                        Some(&story_id),
                        "knowledgeGraph",
                    );
                }
                return "success (precomputed)".to_string();
            }

            if let (Some(content), Some(app)) = (chapter_content, app_handle) {
                if content.len() >= 20 {
                    match self
                        .run_kg_ingest(&story_id, chapter_number, content, &app)
                        .await
                    {
                        Ok(_) => {
                            // P0 修复: KG 提取成功后发射同步事件，确保幕后知识图谱自动刷新
                            let _ = crate::state_sync::StateSync::emit_data_refresh(
                                &app,
                                Some(&story_id),
                                "knowledgeGraph",
                            );
                            "success".to_string()
                        }
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

        // LitSeg E1: 触发叙事分析流水线（在 kg ingest 完成后执行）
        let narrative_status = if kg_status == "success" || kg_status.starts_with("success (") {
            let pool = self.pool.clone();
            match crate::narrative::litseg_pipeline::run_narrative_analysis(&story_id, pool, None)
                .await
            {
                Ok(()) => "success".to_string(),
                Err(e) => format!("error: {}", e),
            }
        } else {
            "skipped: kg_not_success".to_string()
        };
        projection_status["narrative"] = serde_json::json!(narrative_status);

        let async_elapsed = async_start.elapsed().as_millis();
        log::info!(
            "[ProjectionWriter] Async projections (vector + kg) completed in {}ms",
            async_elapsed
        );

        let total_elapsed = sync_start.elapsed().as_millis();
        log::info!(
            "[ProjectionWriter] Total apply_commit completed in {}ms",
            total_elapsed
        );
        if total_elapsed > 2000 {
            log::warn!(
                "[ProjectionWriter] Total time {}ms exceeds 2s threshold. Consider further \
                 parallelizing sync writers.",
                total_elapsed
            );
        }

        // 更新投影状态
        repo.update_projection_status(commit_id, &projection_status.to_string())
            .map_err(|e| format!("更新投影状态失败: {}", e))?;

        Ok(())
    }

    /// v0.22.5: Phase C - 提交后追读力评估与债务清算。
    /// 评估当前章节追读力并写入 `chapter_reading_power`，
    /// 同时根据钩子强度创建或偿还 `chase_debt`。
    fn evaluate_and_reconcile_reading_power(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<(), String> {
        let evaluator = crate::reading_power::ReadingPowerEvaluator::new(self.pool.clone());
        let evaluation = evaluator.evaluate(story_id, chapter_number)?;

        // 写入 chapter_reading_power
        let rp_repo = ChapterReadingPowerRepository::new(self.pool.clone());
        let coolpoint_patterns_json = if evaluation.coolpoint_patterns.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&evaluation.coolpoint_patterns).unwrap_or_default())
        };
        let micropayoffs_json = if evaluation.micropayoffs.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&evaluation.micropayoffs).unwrap_or_default())
        };
        rp_repo
            .save(
                story_id,
                None,
                chapter_number,
                evaluation.hook_type.as_deref(),
                &evaluation.hook_strength,
                coolpoint_patterns_json.as_deref(),
                micropayoffs_json.as_deref(),
                evaluation.is_transition,
            )
            .map_err(|e| format!("保存追读力评估失败: {}", e))?;

        // 债务利息累计
        let _ = evaluator.accrue_interest(story_id);

        // 简单债务清算：
        // - 钩子弱且非过渡章：新增 weak_hook 债务
        // - 钩子强：偿还最老的一笔 active 债务（若存在）
        let debt_repo = ChaseDebtRepository::new(self.pool.clone());
        match evaluation.hook_strength.as_str() {
            "weak" if !evaluation.is_transition => {
                let due_chapter = chapter_number + 3;
                if let Err(e) =
                    debt_repo.create(story_id, "weak_hook", 1.0, 0.1, chapter_number, due_chapter)
                {
                    log::warn!("[SceneCommitService] 创建 weak_hook 债务失败: {}", e);
                }
            }
            "strong" => {
                if let Ok(debts) = debt_repo.get_active_by_story(story_id) {
                    if let Some(oldest) = debts.iter().min_by_key(|d| d.created_at) {
                        if let Err(e) = debt_repo.mark_paid(oldest.id) {
                            log::warn!("[SceneCommitService] 标记债务已偿还失败: {}", e);
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// 运行知识图谱提取（原 auto_ingest 的 IngestPipeline 逻辑）
    ///
    /// 返回 `IngestResult`，调用方负责从中提取 state/entity deltas 与 narrative events。
    async fn run_kg_ingest(
        &self,
        story_id: &str,
        chapter_number: i32,
        content: &str,
        app_handle: &tauri::AppHandle,
    ) -> Result<crate::memory::ingest::IngestResult, String> {
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
                let kg_repo =
                    crate::db::repositories::KnowledgeGraphRepository::new(self.pool.clone());
                let entity_count = result.entities.len();
                let relation_count = result.relations.len();
                match kg_repo.save_entities_batch(&result.entities) {
                    Ok(saved) => log::info!(
                        "[SceneCommitService] Saved {}/{} entities for story {}",
                        saved,
                        entity_count,
                        story_id
                    ),
                    Err(e) => log::warn!("[SceneCommitService] Failed to save entities: {}", e),
                }
                match kg_repo.save_relations_batch(&result.relations) {
                    Ok(saved) => log::info!(
                        "[SceneCommitService] Saved {}/{} relations for story {}",
                        saved,
                        relation_count,
                        story_id
                    ),
                    Err(e) => log::warn!("[SceneCommitService] Failed to save relations: {}", e),
                }
                Ok(result)
            }
            Err(e) => {
                log::warn!(
                    "[SceneCommitService] IngestPipeline failed for story {}: {}",
                    story_id,
                    e
                );
                Err(e.to_string())
            }
        }
    }
}
