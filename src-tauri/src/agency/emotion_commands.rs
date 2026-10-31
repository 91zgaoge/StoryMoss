//! 情感回路的 IPC 命令：契约、快照、张力/债务、读者面板。

use rusqlite::OptionalExtension;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    db::DbPool,
    emotion_circuit::{
        aggregate_reviews, build_reader_panel_prompt, parse_reader_review, CharacterEmotionState,
        EmotionCircuitRepository, EmotionContract, EmotionDebt, EmotionProbeLog, EmotionTension,
        ReaderAggregate, ReaderResonanceReview, SceneEmotionBeat, READER_PERSONAS,
    },
    error::AppError,
};

#[derive(Debug, Serialize)]
pub struct EmotionCircuitSnapshot {
    pub contract: Option<EmotionContract>,
    pub states: Vec<CharacterEmotionState>,
    pub tensions: Vec<EmotionTension>,
    pub debts: Vec<EmotionDebt>,
    pub beats: Vec<SceneEmotionBeat>,
    pub probes: Vec<EmotionProbeLog>,
    pub reader_reviews: Vec<ReaderResonanceReview>,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_get_story_emotion_contract(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<EmotionContract>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        EmotionCircuitRepository::new(pool)
            .get_contract(&story_id)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("emotion contract join error: {e}")))?
}

/// P3：按题材/读者承诺建议一份情感契约（不落库，作者确认后保存）。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_suggest_emotion_contract(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<EmotionContract, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<EmotionContract, AppError> {
        let conn = pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let row: Option<(Option<String>, String)> = conn
            .query_row(
                "SELECT s.genre, COALESCE(gp.reader_promise, '') FROM stories s \
                 LEFT JOIN genre_profiles gp ON gp.id = s.genre_profile_id WHERE s.id = ?1",
                [&story_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(AppError::from)?;
        let Some((genre, promise)) = row else {
            return Err(AppError::validation_failed("故事不存在", None::<String>));
        };
        Ok(crate::emotion_circuit::suggest_contract(
            &story_id,
            genre.as_deref().unwrap_or(""),
            &promise,
        ))
    })
    .await
    .map_err(|e| AppError::from(format!("emotion suggest join error: {e}")))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_update_story_emotion_contract(
    contract: EmotionContract,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    if contract.story_id.trim().is_empty() {
        return Err(AppError::validation_failed(
            "story_id 不能为空",
            None::<String>,
        ));
    }
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        EmotionCircuitRepository::new(pool)
            .upsert_contract(&contract)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("emotion contract save join error: {e}")))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_get_emotion_circuit_snapshot(
    story_id: String,
    scene_id: Option<String>,
    chapter_number: Option<i32>,
    pool: State<'_, DbPool>,
) -> Result<EmotionCircuitSnapshot, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<EmotionCircuitSnapshot, AppError> {
        let repo = EmotionCircuitRepository::new(pool.clone());
        let contract = repo.get_contract(&story_id).map_err(AppError::from)?;
        let states = repo.latest_states(&story_id).unwrap_or_default();
        let tensions = repo.list_tensions(&story_id).unwrap_or_default();
        let debts = repo.list_debts(&story_id).unwrap_or_default();
        let beats = match (&scene_id, chapter_number) {
            (Some(sid), _) => repo.list_scene_beats(&story_id, sid).unwrap_or_default(),
            (None, Some(ch)) => repo
                .latest_beat_for_chapter(&story_id, ch)
                .ok()
                .flatten()
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        let probes = repo
            .list_probe_logs(&story_id, scene_id.as_deref())
            .unwrap_or_default();
        let reader_reviews = repo
            .list_reader_reviews(&story_id, chapter_number)
            .unwrap_or_default();
        Ok(EmotionCircuitSnapshot {
            contract,
            states,
            tensions,
            debts,
            beats,
            probes,
            reader_reviews,
        })
    })
    .await
    .map_err(|e| AppError::from(format!("emotion snapshot join error: {e}")))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_resolve_emotion_debt(
    story_id: String,
    debt_id: String,
    status: String,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let status = status.trim().to_string();
    if !matches!(status.as_str(), "paid" | "deferred" | "ignored" | "open") {
        return Err(AppError::validation_failed(
            "status 只能是 paid/deferred/ignored/open",
            None::<String>,
        ));
    }
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        EmotionCircuitRepository::new(pool)
            .resolve_debt(&story_id, &debt_id, &status)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("emotion debt join error: {e}")))?
}

/// 手动触发章节读者面板（四人格，后台/手动命令，不进续写热路径）。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_run_reader_panel(
    story_id: String,
    scene_id: Option<String>,
    chapter_number: Option<i32>,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<ReaderAggregate, AppError> {
    let pool = pool.inner().clone();
    let pool_ctx = pool.clone();
    let sid = story_id.clone();
    let scene_c = scene_id.clone();
    let chapter_c = chapter_number;
    let ctx = tokio::task::spawn_blocking(move || -> Result<ReaderContext, AppError> {
        let scene = if let Some(ref scene_id) = scene_c {
            crate::db::repositories::SceneRepository::new(pool_ctx.clone())
                .get_by_id(scene_id)
                .map_err(AppError::from)?
        } else {
            let scenes = crate::db::repositories::SceneRepository::new(pool_ctx.clone())
                .get_by_story(&sid)
                .unwrap_or_default();
            let scene = chapter_c
                .and_then(|ch| scenes.iter().find(|s| s.sequence_number == ch).cloned())
                .or_else(|| scenes.into_iter().max_by_key(|s| s.sequence_number));
            scene
        };
        let Some(scene) = scene else {
            return Err(AppError::validation_failed(
                "找不到要评审的章节",
                None::<String>,
            ));
        };
        let content = scene.content.clone().unwrap_or_default();
        if content.trim().is_empty() {
            return Err(AppError::validation_failed("章节正文为空", None::<String>));
        }
        let chapter = chapter_c.unwrap_or(scene.sequence_number);
        let repo = EmotionCircuitRepository::new(pool_ctx.clone());
        let beat = repo
            .latest_beat_for_chapter(&sid, chapter)
            .ok()
            .flatten()
            .or_else(|| {
                repo.list_scene_beats(&sid, &scene.id)
                    .ok()
                    .and_then(|mut beats| beats.pop())
            });
        let Some(beat) = beat else {
            return Err(AppError::validation_failed(
                "本章没有情感回路计划，无法评审",
                Some("no_emotion_beat"),
            ));
        };
        let contract = repo.get_contract(&sid).ok().flatten();
        Ok(ReaderContext {
            story_id: sid,
            scene_id: Some(scene.id),
            chapter_number: Some(chapter),
            content,
            beat,
            contract,
        })
    })
    .await
    .map_err(|e| AppError::from(format!("reader panel context join error: {e}")))??;

    let llm = crate::llm::service::LlmService::new(app_handle);
    let beat = ctx.beat.to_emotion_beat();
    let mut reviews = Vec::new();
    for persona in READER_PERSONAS {
        let prompt = build_reader_panel_prompt(persona, &ctx.content, &beat, ctx.contract.as_ref());
        let (_, result) = llm
            .generate_for_task_with_system_prompt(
                crate::router::TaskType::Analysis,
                prompt,
                Some(1024),
                Some(0.3),
                Some("emotion_reader_panel"),
                Some("你是读者共鸣评审。只输出 JSON，不要解释，不要代码块围栏。".to_string()),
                None,
            )
            .await;
        match result {
            Ok(resp) => match parse_reader_review(&resp.content) {
                Some(mut review) => {
                    if review.persona.trim().is_empty() {
                        review.persona = persona.to_string();
                    }
                    reviews.push(review);
                }
                None => log::warn!(
                    "reader panel: {} 返回无法解析的 JSON: {}",
                    persona,
                    resp.content.chars().take(160).collect::<String>()
                ),
            },
            Err(e) => log::warn!("reader panel: {} 调用失败: {e}", persona),
        }
    }
    if reviews.is_empty() {
        return Err(AppError::from("读者面板所有人格调用均失败"));
    }
    let aggregate = aggregate_reviews(&reviews);
    let rows: Vec<ReaderResonanceReview> = reviews
        .iter()
        .map(|r| {
            crate::emotion_circuit::repo::reader_review_to_row(
                &ctx.story_id,
                ctx.scene_id.as_deref(),
                ctx.chapter_number,
                r,
            )
        })
        .collect();
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let repo = EmotionCircuitRepository::new(pool);
        for row in rows {
            if let Err(e) = repo.insert_reader_review(&row) {
                log::warn!("reader panel: 落库失败: {e}");
            }
        }
    })
    .await
    .map_err(|e| AppError::from(format!("reader panel store join error: {e}")))?;
    Ok(aggregate)
}

struct ReaderContext {
    story_id: String,
    scene_id: Option<String>,
    chapter_number: Option<i32>,
    content: String,
    beat: SceneEmotionBeat,
    contract: Option<EmotionContract>,
}
