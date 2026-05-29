//! Style Analysis - 风格分析管线步骤
//!
//! W3-B6: 每 5 章触发时，计算当前 StyleDNA 六维向量，保存并演化。
//!
//! 触发条件：故事章节数为 5 的倍数时，取最近 5 章的最新草稿拼接后分析。

use crate::{
    creative_engine::style::{
        dna::StyleDNA,
        evolution::{StyleDnaDelta, StyleEvolutionEngine},
        metrics::StyleMetrics,
    },
    db::{repositories::StyleSnapshotRepository, ChapterRepository, DbPool, DraftRepository},
};

/// 风格分析结果
#[derive(Debug, Clone)]
pub struct StyleAnalysisResult {
    pub snapshot_id: String,
    pub metrics: StyleMetrics,
    pub previous_delta: Option<StyleDnaDelta>,
    pub chapter_range: (i32, i32),
}

/// 检查是否应触发风格分析
///
/// 当故事拥有 >= 5 个章节，且最近 5 章尚未被分析时返回 true。
pub fn should_trigger_style_analysis(story_id: &str, pool: &DbPool) -> Result<bool, String> {
    let chapter_repo = ChapterRepository::new(pool.clone());
    let chapters = chapter_repo
        .get_by_story(story_id)
        .map_err(|e| format!("获取章节失败: {}", e))?;

    if chapters.len() < 5 {
        return Ok(false);
    }

    let max_chapter = chapters.iter().map(|c| c.chapter_number).max().unwrap_or(0);

    // 只在 5 的倍数章节触发（如第 5、10、15 章完成后）
    if max_chapter % 5 != 0 {
        return Ok(false);
    }

    let snapshot_repo = StyleSnapshotRepository::new(pool.clone());
    let latest = snapshot_repo
        .get_latest_by_story(story_id)
        .map_err(|e| format!("获取快照失败: {}", e))?;

    // 如果最新快照的 chapter_number >= max_chapter，说明已分析过
    if let Some(snap) = latest {
        if snap.chapter_number.unwrap_or(0) >= max_chapter {
            return Ok(false);
        }
    }

    Ok(true)
}

/// 执行风格分析
///
/// 1. 获取最近 5 章的最新草稿内容
/// 2. 拼接文本并计算六维向量
/// 3. 保存 style_snapshot
/// 4. 如有历史快照，计算与上一次的 delta
pub fn analyze_style_for_story(
    story_id: &str,
    pool: &DbPool,
) -> Result<StyleAnalysisResult, String> {
    let chapter_repo = ChapterRepository::new(pool.clone());
    let draft_repo = DraftRepository::new(pool.clone());

    let chapters = chapter_repo
        .get_by_story(story_id)
        .map_err(|e| format!("获取章节失败: {}", e))?;

    if chapters.is_empty() {
        return Err("故事无章节".to_string());
    }

    // 取最近 5 章
    let mut recent_chapters = chapters;
    recent_chapters.sort_by_key(|c| c.chapter_number);
    let recent = recent_chapters
        .into_iter()
        .rev()
        .take(5)
        .collect::<Vec<_>>();

    let min_ch = recent.iter().map(|c| c.chapter_number).min().unwrap_or(0);
    let max_ch = recent.iter().map(|c| c.chapter_number).max().unwrap_or(0);

    // 收集每章最新草稿的内容
    let mut combined_text = String::new();
    for chapter in &recent {
        if let Some(draft) = draft_repo
            .get_latest_by_chapter(story_id, chapter.chapter_number)
            .map_err(|e| format!("获取草稿失败: {}", e))?
        {
            if !draft.content.is_empty() {
                combined_text.push_str(&draft.content);
                combined_text.push('\n');
            }
        } else if let Some(ref content) = chapter.content {
            // 回退到 chapter 原始内容
            if !content.is_empty() {
                combined_text.push_str(content);
                combined_text.push('\n');
            }
        }
    }

    if combined_text.len() < 100 {
        return Err("最近 5 章内容不足 100 字，无法分析".to_string());
    }

    // 计算六维向量
    let metrics = StyleMetrics::from_text(&combined_text);

    // 保存 snapshot
    let snapshot_repo = StyleSnapshotRepository::new(pool.clone());
    let snapshot = snapshot_repo
        .create(story_id, Some(max_ch), None, &metrics)
        .map_err(|e| format!("保存风格快照失败: {}", e))?;

    // 计算与上一次 snapshot 的 delta
    let previous_delta = snapshot_repo
        .get_latest_by_story(story_id)
        .ok()
        .flatten()
        .and_then(|prev| {
            if prev.id == snapshot.id {
                // 这是刚插入的，取倒数第二个
                // get_latest_by_story 返回最新的（就是我们刚插入的），所以跳过
                None
            } else {
                let prev_metrics = StyleMetrics {
                    sentence_length: prev.sentence_length as f32,
                    dialogue_ratio: prev.dialogue_ratio as f32,
                    metaphor_density: prev.metaphor_density as f32,
                    inner_monologue_ratio: prev.inner_monologue_ratio as f32,
                    emotion_density: prev.emotion_density as f32,
                    rhythm_score: prev.rhythm_score as f32,
                };
                // 简单 delta：直接相减
                Some(StyleDnaDelta {
                    sentence_length_delta: (metrics.sentence_length - prev_metrics.sentence_length)
                        as i32,
                    dialogue_ratio_delta: metrics.dialogue_ratio - prev_metrics.dialogue_ratio,
                    metaphor_density_delta: metrics.metaphor_density
                        - prev_metrics.metaphor_density,
                    interior_monologue_delta: metrics.inner_monologue_ratio
                        - prev_metrics.inner_monologue_ratio,
                    emotion_density_delta: metrics.emotion_density - prev_metrics.emotion_density,
                    rhythm_score_delta: metrics.rhythm_score - prev_metrics.rhythm_score,
                    ..Default::default()
                })
            }
        });

    Ok(StyleAnalysisResult {
        snapshot_id: snapshot.id,
        metrics,
        previous_delta,
        chapter_range: (min_ch, max_ch),
    })
}

/// 驱动 StyleDNA 演化（可选）
///
/// 如果存在故事当前的 StyleDNA，将分析结果与前一次的 delta 作为 feedback
/// 输入 StyleEvolutionEngine，输出演化后的 DNA。
pub fn evolve_style_from_analysis(
    base: &StyleDNA,
    result: &StyleAnalysisResult,
    anti_ai_review: Option<&crate::anti_ai::AntiAiReview>,
    pipeline_review: Option<&crate::pipeline::types::ReviewResult>,
) -> StyleDNA {
    let engine = StyleEvolutionEngine::new();

    // 基础：如果有 review feedback，先用 engine 综合
    let mut delta = engine.evolve_from_reviews(base, anti_ai_review, pipeline_review);

    // 叠加历史趋势 delta（如果有）
    if let Some(ref trend) = result.previous_delta {
        delta.sentence_length_delta += trend.sentence_length_delta;
        delta.dialogue_ratio_delta += trend.dialogue_ratio_delta;
        delta.metaphor_density_delta += trend.metaphor_density_delta;
        delta.interior_monologue_delta += trend.interior_monologue_delta;
        delta.emotion_density_delta += trend.emotion_density_delta;
        delta.rhythm_score_delta += trend.rhythm_score_delta;
        delta.reasons.push(format!(
            "历史趋势（第{}-{}章）: 句长变化 {}, 对话变化 {:.2}, 比喻变化 {:.2}",
            result.chapter_range.0,
            result.chapter_range.1,
            trend.sentence_length_delta,
            trend.dialogue_ratio_delta,
            trend.metaphor_density_delta,
        ));
    }

    delta.apply(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_metrics_from_text() {
        let text = "他像山一样稳。她的眼睛如星星般闪烁。「你好。」他说。他心想，这不对。";
        let metrics = StyleMetrics::from_text(text);
        assert!(metrics.sentence_length > 0.0);
        assert!(metrics.dialogue_ratio >= 0.0);
        assert!(metrics.metaphor_density >= 0.0);
        assert!(metrics.rhythm_score >= 0.0 && metrics.rhythm_score <= 1.0);
    }
}
