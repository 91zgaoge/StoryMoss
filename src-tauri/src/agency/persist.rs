//! 续写落库模式：Append（当前章）与 NextChapter（新章）的纯数据 + Append 写库。

pub use crate::creative_engine::expansion::BeatCounters;
use crate::{db::DbPool, error::AppError};

#[derive(Debug, Clone)]
pub enum PersistMode {
    Append { scene_id: String },
    NextChapter { chapter_number: i32 },
}

#[derive(Debug, Clone)]
pub struct AppendPersistOutcome {
    pub scene_id: String,
    pub chapter_number: i32,
    pub full_content: String,
}

/// 续写落库模式解析。幕前恒传 `explicit_next_chapter=false`（同章 Append）；
/// `true` 仅契约完整（幕后 `agency_continue_chapter` 自己算
/// MAX+1，不走此函数填真实章号）。
pub fn resolve_persist_mode(
    is_continuation: bool,
    scene_id: Option<String>,
    explicit_next_chapter: bool,
) -> Result<PersistMode, AppError> {
    if !is_continuation {
        return Err(AppError::from("resolve_persist_mode 仅用于续写"));
    }
    if explicit_next_chapter {
        return Ok(PersistMode::NextChapter { chapter_number: 0 });
    }
    let sid = scene_id
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::validation_failed("请先打开一个章节", Some("no_scene")))?;
    Ok(PersistMode::Append { scene_id: sid })
}

/// 续写 Append 的 scene 主键。幕前自动分章后分页场景列表常不含新章，
/// `selectChapter` 会把 `chapter.id` 回落成 sceneId 传来。与
/// `update_scene` heal 同口径：id 已是 scene → 原样；id 是 chapter 且该章
/// 已有关联 scene → 用该 scene；否则 `no_scene`。禁止猜「最新有内容场景」。
pub fn resolve_append_scene_id(pool: &DbPool, id: &str) -> Result<String, AppError> {
    let repo = crate::db::repositories::SceneRepository::new(pool.clone());
    if repo.get_by_id(id).map_err(AppError::from)?.is_some() {
        return Ok(id.to_string());
    }
    let linked = repo.get_by_chapter(id).map_err(AppError::from)?;
    linked
        .into_iter()
        .next()
        .map(|s| s.id)
        .ok_or_else(|| AppError::validation_failed("请先打开一个章节", Some("no_scene")))
}

/// 幕前续写进 Agency Append：必须是续写意图，且没有划词选区（选区走改写）。
/// `asset_refresh` 分类不得为 continuation，本函数对其恒为 false。
pub fn should_agency_append_continue(is_continuation: bool, selected_text: Option<&str>) -> bool {
    is_continuation && selected_text.map(str::trim).unwrap_or("").is_empty()
}

/// 每次成功 Append/NextChapter 后 +1。失败不阻断落库。
pub fn increment_append_beat(pool: &DbPool, story_id: &str) -> Result<(), AppError> {
    let conn = pool
        .get()
        .map_err(|e| AppError::from(format!("pool: {e}")))?;
    let mut beats = crate::creative_engine::expansion::read_beat_counters(&conn, story_id);
    beats.append_beats = beats.append_beats.saturating_add(1);
    crate::creative_engine::expansion::write_beat_counters(&conn, story_id, beats)
        .map_err(AppError::from)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RefreshFlags {
    pub conflict: bool,
    pub cast: bool,
    pub location: bool,
    pub foreshadow: bool,
}

pub(crate) fn beat_refresh_flags(
    increment: &str,
    matched_names: &[String],
    prev_present: &[String],
    prev_location: Option<&str>,
    new_location: Option<&str>,
    conflict_parties: &[String],
    foreshadow_needles: &[String],
) -> RefreshFlags {
    let mut names: Vec<String> = matched_names.to_vec();
    names.sort();
    let mut prev: Vec<String> = prev_present.to_vec();
    prev.sort();
    let conflict_named = conflict_parties.len() >= 2
        && conflict_parties
            .iter()
            .all(|p| matched_names.iter().any(|n| n == p));
    let conflict_verb = crate::agency::continue_assets::has_conflict_verb(increment);
    RefreshFlags {
        conflict: conflict_named || conflict_verb,
        cast: names != prev,
        location: match (
            new_location.map(str::trim).filter(|s| !s.is_empty()),
            prev_location.map(str::trim).filter(|s| !s.is_empty()),
        ) {
            (Some(n), Some(p)) => n != p,
            (Some(_), None) => true,
            _ => false,
        },
        foreshadow: foreshadow_needles
            .iter()
            .any(|n| !n.is_empty() && increment.contains(n.as_str())),
    }
}

fn touch_refresh_beats(pool: &DbPool, story_id: &str, flags: RefreshFlags) {
    if !flags.conflict && !flags.cast && !flags.location && !flags.foreshadow {
        return;
    }
    let Ok(conn) = pool.get() else {
        return;
    };
    let mut beats = crate::creative_engine::expansion::read_beat_counters(&conn, story_id);
    if flags.conflict {
        beats.last_conflict_beat = beats.append_beats;
    }
    if flags.cast {
        beats.last_cast_refresh_beat = beats.append_beats;
    }
    if flags.location {
        beats.last_location_beat = beats.append_beats;
    }
    if flags.foreshadow {
        beats.last_foreshadow_beat = beats.append_beats;
    }
    if let Err(e) = crate::creative_engine::expansion::write_beat_counters(&conn, story_id, beats) {
        log::warn!("touch_refresh_beats 失败: {e}");
    }
}

pub(crate) fn merge_progress_line(existing: Option<&str>, node: &str) -> String {
    let node = node.trim();
    let line = format!("进度：{node}");
    if node.is_empty() {
        return existing.unwrap_or("").to_string();
    }
    match existing.map(str::trim).filter(|s| !s.is_empty()) {
        None => line,
        Some(e) if e.contains(&line) => e.to_string(),
        Some(e) => {
            let merged = format!("{e}\n{line}");
            const CAP: usize = 2000;
            if merged.chars().count() <= CAP {
                merged
            } else {
                let mut kept = String::new();
                let mut progress: Vec<&str> = Vec::new();
                for raw in e.lines() {
                    if raw.trim_start().starts_with("进度：") {
                        progress.push(raw);
                    } else if kept.is_empty() {
                        kept.push_str(raw);
                    } else {
                        kept.push('\n');
                        kept.push_str(raw);
                    }
                }
                progress.push(&line);
                while progress.len() > 1 {
                    let candidate = if kept.is_empty() {
                        progress[1..].join("\n")
                    } else {
                        format!("{kept}\n{}", progress[1..].join("\n"))
                    };
                    if candidate.chars().count() <= CAP {
                        return candidate;
                    }
                    progress.remove(0);
                }
                if kept.is_empty() {
                    line
                } else {
                    format!("{kept}\n{line}")
                }
            }
        }
    }
}

fn card_conflicts(
    pool: &DbPool,
    story_id: &str,
    card: &crate::agency::beat_card::SceneBeatCard,
) -> Vec<crate::db::CharacterConflict> {
    if card.conflict_move.parties.len() < 2 {
        return vec![];
    }
    let chars = crate::db::repositories::CharacterRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default();
    let id_of = |name: &str| {
        chars
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.id.clone())
            .unwrap_or_else(|| name.to_string())
    };
    let a = &card.conflict_move.parties[0];
    let b = &card.conflict_move.parties[1];
    vec![crate::db::CharacterConflict {
        character_a_id: id_of(a),
        character_b_id: id_of(b),
        conflict_nature: card.conflict_move.action.chars().take(80).collect(),
        stakes: card.next_outline_node.chars().take(80).collect(),
    }]
}

fn match_names_for_increment(
    pool: &DbPool,
    story_id: &str,
    card: &crate::agency::beat_card::SceneBeatCard,
    increment: &str,
) -> Vec<String> {
    let mut names: Vec<String> = crate::db::repositories::CharacterRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.name)
        .filter(|n| !n.is_empty())
        .collect();
    for m in &card.cast {
        if !m.name.is_empty() && !names.iter().any(|n| n == &m.name) {
            names.push(m.name.clone());
        }
    }
    crate::agency::continue_assets::match_character_names(&names, increment)
}

fn known_locations(
    pool: &DbPool,
    story_id: &str,
    card: &crate::agency::beat_card::SceneBeatCard,
) -> Vec<String> {
    let mut known: Vec<String> = crate::db::repositories::SceneRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| s.setting_location.filter(|l| !l.trim().is_empty()))
        .collect();
    if let Some(ref loc) = card.setting_location {
        if !loc.trim().is_empty() && !known.iter().any(|k| k == loc) {
            known.push(loc.clone());
        }
    }
    known
}

fn write_character_locations(pool: &DbPool, story_id: &str, names: &[String], location: &str) {
    let loc = location.trim();
    if loc.is_empty() || names.is_empty() {
        return;
    }
    let repo = crate::db::repositories::CharacterRepository::new(pool.clone());
    let chars = repo.get_by_story(story_id).unwrap_or_default();
    let state = crate::db::models::CharacterState {
        location: Some(loc.to_string()),
        power_level: None,
        physical_state: None,
        mental_state: None,
        key_items: None,
        recent_events: None,
        updated_at_chapter: None,
        cs_json: None,
        state_transitions_json: None,
        arc_type: None,
    };
    for name in names {
        if let Some(ch) = chars.iter().find(|c| c.name == *name) {
            if let Err(e) = repo.update_character_state(&ch.id, &state) {
                log::warn!("update_character_state location 失败 name={name}: {e}");
            }
        }
    }
}

fn scene_fields_from_facts(
    pool: &DbPool,
    story_id: &str,
    card: &crate::agency::beat_card::SceneBeatCard,
    increment: &str,
    existing_outline: Option<&str>,
    prev_present: &[String],
    prev_location: Option<&str>,
) -> (crate::db::repositories::SceneUpdate, RefreshFlags) {
    let matched = match_names_for_increment(pool, story_id, card, increment);
    let known = known_locations(pool, story_id, card);
    let shift =
        crate::agency::continue_assets::detect_location_shift(&known, prev_location, increment);
    let flags = beat_refresh_flags(
        increment,
        &matched,
        prev_present,
        prev_location,
        shift.as_deref(),
        &card.conflict_move.parties,
        &[],
    );
    let conflicts = if flags.conflict {
        let c = card_conflicts(pool, story_id, card);
        if c.is_empty() {
            None
        } else {
            Some(c)
        }
    } else {
        None
    };
    let update = crate::db::repositories::SceneUpdate {
        characters_present: if matched.is_empty() {
            None
        } else {
            Some(matched.clone())
        },
        character_conflicts: conflicts,
        setting_location: shift.clone(),
        // v0.64.9：卡块（【当前场大纲】在场/冲突/情感/下一拍）**合并**进既有
        // outline_content，而不是覆盖——此前 `_existing_outline` 被忽略，新建章
        // 时刚生成的章纲（handle_gate 从黑板读出的 outline-第N章）当场被卡块
        // 冲掉，章节从此失去方向，只能照抄上一拍。
        outline_content: Some(crate::agency::observe::merge_current_scene_outline(
            existing_outline,
            &card.render_scene_outline(),
        )),
        ..Default::default()
    };
    (update, flags)
}

/// NextChapter 装配用：从 BeatCard + 新章正文填出场/冲突/地点/进度。
pub fn scene_update_from_card(
    pool: &DbPool,
    story_id: &str,
    card: &crate::agency::beat_card::SceneBeatCard,
    content: String,
    existing_outline: Option<&str>,
) -> crate::db::repositories::SceneUpdate {
    let (mut u, _) =
        scene_fields_from_facts(pool, story_id, card, &content, existing_outline, &[], None);
    u.content = Some(content);
    u
}

/// NextChapter 落库后：+1 拍并按旗标刷新债务；写回角色地点。
pub fn apply_card_beat_refresh(
    pool: &DbPool,
    story_id: &str,
    card: &crate::agency::beat_card::SceneBeatCard,
    increment: &str,
    prev_present: &[String],
    prev_location: Option<&str>,
    matched_or_present: &[String],
    location: Option<&str>,
) {
    let (_, flags) = scene_fields_from_facts(
        pool,
        story_id,
        card,
        increment,
        None,
        prev_present,
        prev_location,
    );
    if let Err(e) = increment_append_beat(pool, story_id) {
        log::warn!("increment_append_beat 失败: {e}");
    }
    touch_refresh_beats(pool, story_id, flags);
    if let Some(loc) = location.filter(|s| !s.trim().is_empty()) {
        write_character_locations(pool, story_id, matched_or_present, loc);
    }
}

/// 将 current_content + increment 写入已有 scene。禁止 create 新行。
pub fn persist_append(
    pool: &DbPool,
    mode: &PersistMode,
    current_content: &str,
    increment: &str,
) -> Result<AppendPersistOutcome, AppError> {
    persist_append_inner(pool, mode, current_content, increment, None)
}

/// Append 落库并回写本拍阵容/冲突/地点/进度。
pub fn persist_append_with_card(
    pool: &DbPool,
    scene_id: &str,
    current_content: &str,
    increment: &str,
    card: &crate::agency::beat_card::SceneBeatCard,
) -> Result<AppendPersistOutcome, AppError> {
    persist_append_inner(
        pool,
        &PersistMode::Append {
            scene_id: scene_id.to_string(),
        },
        current_content,
        increment,
        Some(card),
    )
}

fn persist_append_inner(
    pool: &DbPool,
    mode: &PersistMode,
    current_content: &str,
    increment: &str,
    card: Option<&crate::agency::beat_card::SceneBeatCard>,
) -> Result<AppendPersistOutcome, AppError> {
    let PersistMode::Append { scene_id } = mode else {
        return Err(AppError::from("persist_append 只接受 Append"));
    };
    let scene_id = resolve_append_scene_id(pool, scene_id)?;
    let repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let scene = repo
        .get_by_id(&scene_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::validation_failed("请先打开一个章节", Some("no_scene")))?;
    let cleaned_old = current_content.trim();
    let cleaned_inc = increment.trim();
    if cleaned_inc.chars().count() < 200 {
        return Err(AppError::from("续写增量过短，拒绝落库"));
    }
    // 客户端快照可能仍是未接受幽灵之前的正文；DB 若已更长，接到 DB
    // 上，禁止覆盖掉上一拍。
    let db_raw = scene.content.as_deref().unwrap_or("").trim();
    let base = append_base_content(db_raw, cleaned_old);
    let full = join_content(base, cleaned_inc);
    let (mut update, flags) = if let Some(card) = card {
        scene_fields_from_facts(
            pool,
            &scene.story_id,
            card,
            cleaned_inc,
            scene.outline_content.as_deref(),
            &scene.characters_present,
            scene.setting_location.as_deref(),
        )
    } else {
        (
            crate::db::repositories::SceneUpdate::default(),
            RefreshFlags::default(),
        )
    };
    update.content = Some(full.clone());
    repo.update(&scene_id, &update).map_err(AppError::from)?;
    if let Err(e) = increment_append_beat(pool, &scene.story_id) {
        log::warn!("increment_append_beat 失败: {e}");
    }
    if card.is_some() {
        touch_refresh_beats(pool, &scene.story_id, flags);
        let loc = update
            .setting_location
            .as_deref()
            .or(scene.setting_location.as_deref())
            .unwrap_or("");
        let names = update
            .characters_present
            .as_ref()
            .unwrap_or(&scene.characters_present);
        if !loc.is_empty() {
            write_character_locations(pool, &scene.story_id, names, loc);
        }
    }
    Ok(AppendPersistOutcome {
        scene_id: scene.id,
        chapter_number: scene.sequence_number,
        full_content: full,
    })
}

/// 分章截断后客户端仍可能持有旧全文。DB 是其前缀且多出的部分够长，
/// 视为溢出已迁走，用 DB 做底稿。短增量（未保存打字）仍用客户端。
const SPLIT_RESTORE_MIN_EXTRA_CHARS: usize = 200;

pub(crate) fn append_base_content<'a>(db_raw: &'a str, client: &'a str) -> &'a str {
    let db_plain = crate::agency::continue_assets::strip_editor_markup(db_raw);
    let client_plain = crate::agency::continue_assets::strip_editor_markup(client);
    let db_n = db_plain.chars().count();
    let client_n = client_plain.chars().count();
    if db_n > client_n {
        db_raw
    } else if db_n > 0
        && client_n.saturating_sub(db_n) >= SPLIT_RESTORE_MIN_EXTRA_CHARS
        && client_plain.starts_with(&db_plain)
    {
        db_raw
    } else {
        client
    }
}

fn looks_like_html(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('<') && t.contains('>')
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 旧文已是 HTML 时用 `<p>` 包增量，避免把 TipTap 标记压成纯文本；纯文本仍用
/// `\n\n`。
fn join_content(old: &str, increment: &str) -> String {
    if old.is_empty() {
        increment.to_string()
    } else if looks_like_html(old) {
        format!("{old}<p>{}</p>", html_escape(increment))
    } else {
        format!("{old}\n\n{increment}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        connection::create_test_pool,
        repositories::{CreateStoryRequest, SceneRepository, StoryRepository},
    };

    fn long_increment() -> String {
        "续写增量正文。".repeat(30) // 7 * 30 = 210 字，满足 ≥200 落库门槛
    }

    fn seed_story_with_scene(pool: &crate::db::DbPool) -> (String, String) {
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "追加测试".into(),
                description: None,
                genre: Some("玄幻".into()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let scene_repo = SceneRepository::new(pool.clone());
        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let scene = scene_repo
            .create_in_tx(&tx, &story.id, 1, Some("第一章"))
            .unwrap();
        scene_repo
            .update_in_tx(
                &tx,
                &scene.id,
                &crate::db::repositories::SceneUpdate {
                    content: Some("旧文开头。".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        tx.commit().unwrap();
        (story.id, scene.id)
    }

    #[test]
    fn append_does_not_create_new_scene_row() {
        let pool = create_test_pool().unwrap();
        let (story_id, scene_id) = seed_story_with_scene(&pool);
        let out = persist_append(
            &pool,
            &PersistMode::Append {
                scene_id: scene_id.clone(),
            },
            "旧文开头。",
            &long_increment(),
        )
        .unwrap();
        assert_eq!(out.scene_id, scene_id);
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert_eq!(scenes.len(), 1);
        let expected = format!("旧文开头。\n\n{}", long_increment());
        assert_eq!(scenes[0].content.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn append_missing_scene_id_is_err() {
        let pool = create_test_pool().unwrap();
        let err = persist_append(
            &pool,
            &PersistMode::Append {
                scene_id: "no-such".into(),
            },
            "旧",
            "新",
        )
        .unwrap_err();
        assert!(err.to_string().contains("请先打开一个章节") || err.to_string().contains("不存在"));
    }

    /// 幕前自动分章后常把 chapter.id 当成 scene_id 传来。
    /// 契约：id 命中 chapters 且该章已有关联 scene 时，Append 接到该 scene，
    /// 不得报「请先打开一个章节」。
    #[test]
    fn append_chapter_id_resolves_to_linked_scene() {
        let pool = create_test_pool().unwrap();
        let (_story_id, scene_id) = seed_story_with_scene(&pool);
        let scene = SceneRepository::new(pool.clone())
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        let chapter_id = scene
            .chapter_id
            .expect("create_in_tx 必须给 scene 挂 chapter_id");
        assert_ne!(chapter_id, scene_id);
        assert_eq!(
            resolve_append_scene_id(&pool, &chapter_id).unwrap(),
            scene_id
        );
        let out = persist_append(
            &pool,
            &PersistMode::Append {
                scene_id: chapter_id,
            },
            "旧文开头。",
            &long_increment(),
        )
        .unwrap();
        assert_eq!(out.scene_id, scene_id);
    }

    #[test]
    fn rewrite_selection_does_not_route_to_agency_append() {
        assert!(should_agency_append_continue(true, None));
        assert!(should_agency_append_continue(true, Some("  ")));
        assert!(!should_agency_append_continue(true, Some("选中的一段")));
        assert!(!should_agency_append_continue(false, None));
    }

    #[test]
    fn continuation_requires_scene_id_for_append() {
        let err = resolve_persist_mode(true, None, false).unwrap_err();
        assert!(err.to_string().contains("请先打开一个章节"));
        let ok = resolve_persist_mode(true, Some("s1".into()), false).unwrap();
        match ok {
            PersistMode::Append { scene_id } => {
                assert_eq!(scene_id, "s1")
            }
            _ => panic!("expected Append"),
        }
        let next = resolve_persist_mode(true, None, true).unwrap();
        match next {
            PersistMode::NextChapter { chapter_number } => {
                assert_eq!(chapter_number, 0); // 占位；幕后自己算 MAX+1，
                                               // 不走此函数填真实章号
            }
            _ => panic!("expected NextChapter"),
        }
    }

    #[test]
    fn append_html_wraps_increment_in_p() {
        let pool = create_test_pool().unwrap();
        let (story_id, scene_id) = seed_story_with_scene(&pool);
        let old = "<p>旧文开头。</p>";
        let inc = long_increment();
        let out = persist_append(
            &pool,
            &PersistMode::Append {
                scene_id: scene_id.clone(),
            },
            old,
            &inc,
        )
        .unwrap();
        let expected = format!("{old}<p>{inc}</p>");
        assert_eq!(out.full_content, expected);
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert_eq!(scenes.len(), 1);
        assert_eq!(scenes[0].content.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn append_stale_client_snapshot_does_not_wipe_previous_increment() {
        let pool = create_test_pool().unwrap();
        let (story_id, scene_id) = seed_story_with_scene(&pool);
        let first = persist_append(
            &pool,
            &PersistMode::Append {
                scene_id: scene_id.clone(),
            },
            "旧文开头。",
            &long_increment(),
        )
        .unwrap();
        let second_inc = "第二拍增量正文。".repeat(30);
        let second = persist_append(
            &pool,
            &PersistMode::Append {
                scene_id: scene_id.clone(),
            },
            "旧文开头。",
            &second_inc,
        )
        .unwrap();
        assert!(
            second.full_content.contains("续写增量正文。"),
            "上一拍不得被客户端旧快照覆盖 got={}",
            second.full_content
        );
        assert!(second.full_content.contains("第二拍增量正文。"));
        assert!(second.full_content.len() > first.full_content.len());
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert_eq!(
            scenes[0].content.as_deref(),
            Some(second.full_content.as_str())
        );
    }

    #[test]
    fn append_base_prefers_db_when_client_is_pre_split_superset() {
        let keep = "截断后留在旧章的正文。".repeat(20);
        let overflow = "已经迁到新章的溢出正文。".repeat(20);
        let client = format!("{keep}{overflow}");
        assert!(overflow.chars().count() >= 200);
        assert_eq!(append_base_content(&keep, &client), keep.as_str());
    }

    #[test]
    fn append_base_keeps_short_unsaved_client_suffix() {
        let db = "已落库正文。";
        let client = "已落库正文。又打了几个字";
        assert_eq!(append_base_content(db, client), client);
    }

    fn dummy_card() -> crate::agency::beat_card::SceneBeatCard {
        use crate::agency::beat_card::{CastMember, ConflictMove, EmotionBeat, SceneBeatCard};
        SceneBeatCard {
            cast: vec![
                CastMember {
                    name: "阿岩".into(),
                    purpose: "守门".into(),
                },
                CastMember {
                    name: "林雪".into(),
                    purpose: "质问".into(),
                },
            ],
            conflict_move: ConflictMove {
                action: "加压".into(),
                parties: vec!["阿岩".into(), "林雪".into()],
            },
            emotion_beat: EmotionBeat {
                summary: "怒".into(),
            },
            next_outline_node: "夜宴破裂".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("夜宴厅".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: crate::agency::beat_card::ChangeDelta::default(),
        }
    }

    #[test]
    fn append_writeback_sets_characters_present_names() {
        let pool = create_test_pool().unwrap();
        let (_story_id, scene_id) = seed_story_with_scene(&pool);
        let card = dummy_card();
        let inc = format!(
            "阿岩看了林雪一眼，夜宴厅灯火未歇。{}",
            "续写增量正文。".repeat(30)
        );
        persist_append_with_card(&pool, &scene_id, "旧文。", &inc, &card).unwrap();
        let scene = SceneRepository::new(pool.clone())
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        assert!(scene.characters_present.contains(&"阿岩".into()));
        assert!(scene.characters_present.contains(&"林雪".into()));
        assert_eq!(scene.setting_location.as_deref(), Some("夜宴厅"));
        assert!(scene
            .outline_content
            .as_deref()
            .unwrap_or_default()
            .contains("夜宴破裂"));
        assert!(scene
            .outline_content
            .as_deref()
            .unwrap_or_default()
            .contains("【当前场大纲】"));
        assert!(scene
            .outline_content
            .as_deref()
            .unwrap_or_default()
            .contains("下一拍"));
    }

    #[test]
    fn test_run_continue_append_keeps_scene_count() {
        let pool = create_test_pool().unwrap();
        let (story_id, scene_id) = seed_story_with_scene(&pool);
        persist_append(
            &pool,
            &PersistMode::Append {
                scene_id: scene_id.clone(),
            },
            "旧文开头。",
            &long_increment(),
        )
        .unwrap();
        persist_append(
            &pool,
            &PersistMode::Append { scene_id },
            "旧文开头。",
            &long_increment(),
        )
        .unwrap();
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert_eq!(scenes.len(), 1);
    }

    #[test]
    fn next_chapter_create_adds_one_scene_row() {
        use crate::agency::beat_card::{CastMember, ConflictMove, EmotionBeat, SceneBeatCard};
        let pool = create_test_pool().unwrap();
        let (story_id, _scene_id) = seed_story_with_scene(&pool);
        let card = SceneBeatCard {
            cast: vec![CastMember {
                name: "阿岩".into(),
                purpose: "守门".into(),
            }],
            conflict_move: ConflictMove {
                action: "加压".into(),
                parties: vec!["阿岩".into(), "林雪".into()],
            },
            emotion_beat: EmotionBeat {
                summary: "怒".into(),
            },
            next_outline_node: "夜宴破裂".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("夜宴厅".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: crate::agency::beat_card::ChangeDelta::default(),
        };
        let scene_repo = SceneRepository::new(pool.clone());
        let update = scene_update_from_card(&pool, &story_id, &card, long_increment(), None);
        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let scene = scene_repo
            .create_in_tx(&tx, &story_id, 2, Some("第二章"))
            .unwrap();
        scene_repo.update_in_tx(&tx, &scene.id, &update).unwrap();
        tx.commit().unwrap();
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert_eq!(scenes.len(), 2);
        assert!(scenes.iter().any(|s| s.sequence_number == 2));
    }

    #[test]
    fn third_beat_quota_includes_conflict_after_two_append_without_conflict() {
        let pool = create_test_pool().unwrap();
        let (story_id, scene_id) = seed_story_with_scene(&pool);
        crate::db::repositories::CharacterRepository::new(pool.clone())
            .create(crate::db::repositories::CreateCharacterRequest {
                story_id: story_id.clone(),
                name: "阿岩".into(),
                background: None,
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
                source: None,
                is_auto_generated: None,
                emotional_core: None,
                emotional_trigger: None,
                emotional_wound: None,
                emotional_need: None,
            })
            .unwrap();
        for _ in 0..2 {
            persist_append(
                &pool,
                &PersistMode::Append {
                    scene_id: scene_id.clone(),
                },
                "旧文开头。",
                &long_increment(),
            )
            .unwrap();
        }
        let card = crate::agency::beat_card::compile_beat_card(&pool, &story_id, "阿岩站在雨里。")
            .unwrap();
        assert!(card
            .expansion_quota
            .contains(&crate::creative_engine::expansion::debt::QuotaItem::ConflictEscalation));
    }

    #[test]
    fn beat_refresh_flags_conflict_only_when_parties_in_increment_or_verbs() {
        let flags = beat_refresh_flags(
            "两人继续喝茶聊天。",
            &["阿岩".into()],
            &["阿岩".into()],
            Some("雨巷"),
            Some("雨巷"),
            &["阿岩".into(), "林雪".into()],
            &[],
        );
        assert!(!flags.conflict);
        assert!(!flags.cast);
        assert!(!flags.location);
        assert!(!flags.foreshadow);
    }

    #[test]
    fn beat_refresh_flags_conflict_when_both_parties_named() {
        let flags = beat_refresh_flags(
            "阿岩逼视林雪，林雪没有退。",
            &["阿岩".into(), "林雪".into()],
            &["阿岩".into()],
            Some("雨巷"),
            Some("雨巷"),
            &["阿岩".into(), "林雪".into()],
            &[],
        );
        assert!(flags.conflict);
        assert!(flags.cast);
    }

    #[test]
    fn beat_refresh_flags_conflict_verb_without_both_names() {
        let flags = beat_refresh_flags(
            "对峙已经无法再拖。",
            &["阿岩".into()],
            &["阿岩".into()],
            None,
            None,
            &["阿岩".into(), "林雪".into()],
            &[],
        );
        assert!(flags.conflict);
    }

    #[test]
    fn merge_progress_line_appends_distinct_nodes() {
        let once = merge_progress_line(None, "夜宴破裂");
        assert_eq!(once, "进度：夜宴破裂");
        let twice = merge_progress_line(Some(&once), "密诏败露");
        assert!(twice.contains("进度：夜宴破裂"));
        assert!(twice.contains("进度：密诏败露"));
        let dup = merge_progress_line(Some(&twice), "密诏败露");
        assert_eq!(dup.matches("进度：密诏败露").count(), 1);
    }

    #[test]
    fn two_appends_without_conflict_realization_leave_last_conflict_zero() {
        let pool = create_test_pool().unwrap();
        let (story_id, scene_id) = seed_story_with_scene(&pool);
        let card = dummy_card();
        persist_append_with_card(&pool, &scene_id, "旧文。", &long_increment(), &card).unwrap();
        persist_append_with_card(&pool, &scene_id, "旧文。", &long_increment(), &card).unwrap();
        let conn = pool.get().unwrap();
        let beats = crate::creative_engine::expansion::read_beat_counters(&conn, &story_id);
        assert_eq!(beats.append_beats, 2);
        assert_eq!(beats.last_conflict_beat, 0);
        let debt = crate::creative_engine::expansion::ExpansionDebt::from_beats(&beats);
        assert!(debt
            .triggered()
            .contains(&crate::creative_engine::expansion::debt::QuotaItem::ConflictEscalation));
    }

    #[test]
    fn writeback_present_from_increment_names_not_card_plan() {
        let pool = create_test_pool().unwrap();
        let (_story_id, scene_id) = seed_story_with_scene(&pool);
        let mut card = dummy_card();
        card.cast = vec![
            crate::agency::beat_card::CastMember {
                name: "阿岩".into(),
                purpose: "计划".into(),
            },
            crate::agency::beat_card::CastMember {
                name: "林雪".into(),
                purpose: "计划".into(),
            },
            crate::agency::beat_card::CastMember {
                name: "幽灵".into(),
                purpose: "沉寂回归".into(),
            },
        ];
        let inc = format!("阿岩看了林雪一眼。{}", "续写增量正文。".repeat(28));
        persist_append_with_card(&pool, &scene_id, "旧文。", &inc, &card).unwrap();
        let scene = SceneRepository::new(pool.clone())
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        assert!(scene.characters_present.contains(&"阿岩".into()));
        assert!(scene.characters_present.contains(&"林雪".into()));
        assert!(!scene.characters_present.contains(&"幽灵".into()));
    }

    #[test]
    fn writeback_keeps_old_present_when_increment_has_no_names() {
        let pool = create_test_pool().unwrap();
        let (_story_id, scene_id) = seed_story_with_scene(&pool);
        {
            let repo = SceneRepository::new(pool.clone());
            repo.update(
                &scene_id,
                &crate::db::repositories::SceneUpdate {
                    characters_present: Some(vec!["阿岩".into()]),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        persist_append_with_card(&pool, &scene_id, "旧文。", &long_increment(), &dummy_card())
            .unwrap();
        let scene = SceneRepository::new(pool.clone())
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        assert_eq!(scene.characters_present, vec!["阿岩".to_string()]);
    }

    #[test]
    fn writeback_character_location_for_matched_names() {
        let pool = create_test_pool().unwrap();
        let (story_id, scene_id) = seed_story_with_scene(&pool);
        let char_repo = crate::db::repositories::CharacterRepository::new(pool.clone());
        let ch = char_repo
            .create(crate::db::repositories::CreateCharacterRequest {
                story_id: story_id.clone(),
                name: "阿岩".into(),
                background: None,
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
                source: None,
                is_auto_generated: None,
                emotional_core: None,
                emotional_trigger: None,
                emotional_wound: None,
                emotional_need: None,
            })
            .unwrap();
        let mut card = dummy_card();
        card.setting_location = Some("钟楼".into());
        let inc = format!("阿岩潜入钟楼底层。{}", "续写增量正文。".repeat(30));
        persist_append_with_card(&pool, &scene_id, "旧文。", &inc, &card).unwrap();
        let state = char_repo.get_character_state(&ch.id).unwrap().unwrap();
        assert_eq!(state.location.as_deref(), Some("钟楼"));
    }

    /// v0.64.9 真机契约：写入节拍卡时，章纲前缀必须保留（此前被覆盖丢弃，
    /// 真机新建章当场失去方向）。
    #[test]
    fn card_write_keeps_chapter_plan_prefix() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let story_id = crate::db::StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "章纲保留".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id;
        crate::db::repositories::CharacterRepository::new(pool.clone())
            .create(crate::db::CreateCharacterRequest {
                story_id: story_id.clone(),
                name: "阿岩".into(),
                ..Default::default()
            })
            .unwrap();
        let card = crate::agency::beat_card::compile_beat_card(&pool, &story_id, "阿岩站在雨里。")
            .unwrap();
        let plan = "本章：阿岩必须在天亮前交出断灯绳。";
        let update =
            scene_update_from_card(&pool, &story_id, &card, "阿岩站在雨里。".into(), Some(plan));
        let outline = update.outline_content.unwrap_or_default();
        assert!(outline.starts_with(plan), "{outline}");
        assert!(outline.contains(crate::agency::beat_card::CURRENT_SCENE_OUTLINE_MARK));
    }
}
