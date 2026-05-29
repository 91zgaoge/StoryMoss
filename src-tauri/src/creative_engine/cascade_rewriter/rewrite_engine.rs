//! 改写引擎
//!
//! 基于 LLM 的增量改写，包含 Prompt 构建、调用、验证。

use tauri::AppHandle;

use super::{
    impact_analyzer::ImpactAnalyzer,
    models::{
        CascadeTaskPayload, CascadeTaskResult, EntityChangeEvent, RewriteSegment, RewriteStatus,
        SceneImpact, UserDecision,
    },
    repository::EntityMentionRepository,
};
use crate::{
    db::{repositories::SceneRepository, DbPool},
    error::AppError,
    llm::LlmService,
};

pub struct RewriteEngine {
    pool: DbPool,
    app_handle: AppHandle,
}

impl RewriteEngine {
    pub fn new(pool: DbPool, app_handle: AppHandle) -> Self {
        Self { pool, app_handle }
    }

    pub async fn execute(
        &self,
        payload: &CascadeTaskPayload,
    ) -> Result<CascadeTaskResult, AppError> {
        let mut segments = Vec::new();
        let mut warnings = Vec::new();

        let analyzer = ImpactAnalyzer::new(self.pool.clone());
        let mention_repo = EntityMentionRepository::new(self.pool.clone());
        let llm = LlmService::new(self.app_handle.clone());

        for change in &payload.change_events {
            let impacts = analyzer.analyze(change)?;
            for impact in impacts {
                match self
                    .rewrite_scene(&impact, change, &mention_repo, &llm)
                    .await
                {
                    Ok(mut segs) => segments.append(&mut segs),
                    Err(e) => {
                        warnings.push(format!("Scene {} rewrite failed: {}", impact.scene_id, e));
                    }
                }
            }
        }

        let status = if segments.is_empty() && !warnings.is_empty() {
            RewriteStatus::Failed
        } else if !warnings.is_empty() {
            RewriteStatus::NeedsReview
        } else {
            RewriteStatus::Ok
        };

        Ok(CascadeTaskResult {
            status,
            segments,
            warnings,
        })
    }

    async fn rewrite_scene(
        &self,
        impact: &SceneImpact,
        change: &EntityChangeEvent,
        mention_repo: &EntityMentionRepository,
        llm: &LlmService,
    ) -> Result<Vec<RewriteSegment>, AppError> {
        let scene_repo = SceneRepository::new(self.pool.clone());
        let scene = scene_repo
            .get_by_id(&impact.scene_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Scene", &impact.scene_id))?;

        let content = scene.content.unwrap_or_default();
        if content.is_empty() {
            return Ok(vec![]);
        }

        let mentions = mention_repo.get_by_scene(&impact.scene_id)?;
        let relevant_mentions: Vec<_> = mentions
            .into_iter()
            .filter(|m| m.entity_id == change.entity_id)
            .collect();

        if relevant_mentions.is_empty() {
            return Ok(vec![]);
        }

        let paragraphs: Vec<&str> = content.split('\n').collect();
        let mut rewritten = Vec::new();
        let mut processed_indices = std::collections::HashSet::new();

        for mention in relevant_mentions {
            let para_index = find_paragraph_index(&paragraphs, mention.start_pos as usize);
            if processed_indices.contains(&para_index) {
                continue;
            }
            processed_indices.insert(para_index);

            let paragraph_text = paragraphs.get(para_index).unwrap_or(&"").to_string();
            let prev_paragraph = if para_index > 0 {
                paragraphs.get(para_index - 1).unwrap_or(&"").to_string()
            } else {
                String::new()
            };
            let next_paragraph = paragraphs.get(para_index + 1).unwrap_or(&"").to_string();

            let prompt = build_rewrite_prompt(
                &change.entity_name,
                &change.changed_fields,
                &change.before_json,
                &change.after_json,
                &paragraph_text,
                &prev_paragraph,
                &next_paragraph,
            );

            match llm.generate(prompt, Some(2048), Some(0.3)).await {
                Ok(response) => {
                    let rewritten_text = response.content.trim().to_string();

                    // 长度检查
                    let len_ratio =
                        rewritten_text.len() as f64 / paragraph_text.len().max(1) as f64;
                    if len_ratio < 0.5 || len_ratio > 1.5 {
                        log::warn!(
                            "[CascadeRewriter] Length ratio {:.2} for scene {}, paragraph {}",
                            len_ratio,
                            impact.scene_id,
                            para_index
                        );
                    }

                    // 实体保留检查
                    if !rewritten_text.contains(&change.entity_name) {
                        log::warn!(
                            "[CascadeRewriter] Entity '{}' missing in rewritten text for scene {}",
                            change.entity_name,
                            impact.scene_id
                        );
                    }

                    rewritten.push(RewriteSegment {
                        scene_id: impact.scene_id.clone(),
                        paragraph_index: para_index as i32,
                        original_text: paragraph_text,
                        rewritten_text,
                        change_reason: format!(
                            "{}: {:?}",
                            change.entity_name, change.changed_fields
                        ),
                        user_decision: UserDecision::Pending,
                    });
                }
                Err(e) => {
                    return Err(AppError::internal(format!("LLM call failed: {}", e)));
                }
            }
        }

        Ok(rewritten)
    }
}

fn find_paragraph_index(paragraphs: &[&str], start_pos: usize) -> usize {
    let mut current_pos = 0;
    for (i, para) in paragraphs.iter().enumerate() {
        let end_pos = current_pos + para.len() + 1; // +1 for '\n'
        if start_pos < end_pos {
            return i;
        }
        current_pos = end_pos;
    }
    paragraphs.len().saturating_sub(1)
}

fn build_rewrite_prompt(
    entity_name: &str,
    changed_fields: &[String],
    _before_json: &str,
    after_json: &str,
    paragraph_text: &str,
    prev_paragraph: &str,
    next_paragraph: &str,
) -> String {
    let changed_summary = changed_fields.join(", ");
    let after_summary = serde_json::from_str::<serde_json::Value>(after_json)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| after_json.to_string());

    format!(
        r#"你是一位小说编辑。以下段落中引用了角色「{}」，但该角色的设定已发生变更。

【变更内容】
变更字段: {}
变更后设定: {}

【原文段落】
{}

【上下文】
{}
{}

【约束】
- 仅改写与变更设定直接冲突的句子
- 保持原文风格、语气、节奏不变
- 不要增加原文没有的新情节
- 输出完整的改写后段落，不要包含解释"#,
        entity_name,
        changed_summary,
        after_summary,
        paragraph_text,
        if prev_paragraph.is_empty() {
            "（无前文）"
        } else {
            prev_paragraph
        },
        if next_paragraph.is_empty() {
            "（无后文）"
        } else {
            next_paragraph
        },
    )
}
