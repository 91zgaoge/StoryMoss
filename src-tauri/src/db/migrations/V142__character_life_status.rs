//! V142：角色生死状态持久化。
//!
//! 真机事故：第 2 章写成死亡的「明成公主」在第 10 章被续写成活人——生死此前只由
//! 章末 1500 字窗口临时推断，没有任何持久化。本迁移加两列并回填存量：
//! - `characters.life_status`（'alive' | 'dead'）
//! - `characters.death_chapter`
//!
//! 回填完全确定性（0 LLM）：按章序扫描 `scenes.content`，用
//! `continue_assets::name_is_dead_in_text` 判定（含「未气绝/假死/诈死」否定句
//! 豁免）；正文扫描漏掉的再并入 KG 抽取已判 `status=Dead` 的角色。命中即把
//! `character_states.physical_state` 打上「已死（第 N
//! 章）」标记，角色卡随即可见。
//!
//! 幂等：已 `dead` 的角色不再处理，重跑无写入。

use rusqlite::Connection;

use crate::db::{character_life, migrations::RustMigration};

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        142
    }

    fn description(&self) -> &'static str {
        "persist character life status (dead once written, never resurrected)"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(characters)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if !cols.iter().any(|c| c == "life_status") {
            conn.execute(
                "ALTER TABLE characters ADD COLUMN life_status TEXT NOT NULL DEFAULT 'alive'",
                [],
            )?;
        }
        if !cols.iter().any(|c| c == "death_chapter") {
            conn.execute(
                "ALTER TABLE characters ADD COLUMN death_chapter INTEGER",
                [],
            )?;
        }

        let stories: Vec<String> = {
            let mut stmt = conn.prepare("SELECT id FROM stories")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for story_id in &stories {
            let marked = character_life::backfill_story(conn, story_id)?;
            if !marked.is_empty() {
                log::warn!(
                    "[V142] 故事 {} 回填已死角色 {} 个：{:?}",
                    story_id,
                    marked.len(),
                    marked
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        character_life::{dead_marker_map_on, DEAD_TAG},
        connection::create_test_pool,
        repositories::CharacterRepository,
        CreateCharacterRequest, CreateStoryRequest,
    };

    fn seed_story(pool: &crate::db::DbPool) -> String {
        crate::db::StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "V142 回填".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id
    }

    fn seed_character(pool: &crate::db::DbPool, story_id: &str, name: &str) {
        CharacterRepository::new(pool.clone())
            .create(CreateCharacterRequest {
                story_id: story_id.to_string(),
                name: name.to_string(),
                ..Default::default()
            })
            .unwrap();
    }

    /// 真机形态：第 2 章正文把公主写死，第 9 章起又让她活过来。
    fn seed_real_machine_scenes(pool: &crate::db::DbPool, story_id: &str) {
        let scene_repo = crate::db::SceneRepository::new(pool.clone());
        let ch2 = scene_repo.create(story_id, 2, None).unwrap();
        scene_repo
            .update(
                &ch2.id,
                &crate::db::repositories::SceneUpdate {
                    content: Some(
                        "苏会山的右拳本能般汇聚全身功力，以雷霆万钧之势一拳击中公主，\
                         将其打得横飞而出，七窍喷血，抽搐几下，登时气绝。\
                         明成公主的尸体躺在原处，短刃落在她手边。"
                            .into(),
                    ),
                    ..Default::default()
                },
            )
            .unwrap();
        let ch9 = scene_repo.create(story_id, 9, None).unwrap();
        scene_repo
            .update(
                &ch9.id,
                &crate::db::repositories::SceneUpdate {
                    content: Some("明成公主往前走了一步，抓住苏亦铁的手腕。".into()),
                    ..Default::default()
                },
            )
            .unwrap();
    }

    #[test]
    fn v142_backfills_dead_character_with_death_chapter() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_character(&pool, &story_id, "明成公主");
        seed_character(&pool, &story_id, "苏亦铁");
        seed_real_machine_scenes(&pool, &story_id);

        // create_test_pool 已跑过 V142（空库无回填）；再跑一次模拟存量库升级
        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();

        let dead = dead_marker_map_on(&pool.get().unwrap(), &story_id).unwrap();
        assert_eq!(
            dead.get("明成公主"),
            Some(&Some(2)),
            "死亡章取先命中的第 2 章"
        );
        assert!(!dead.contains_key("苏亦铁"), "活人不得被标记");
    }

    #[test]
    fn v142_marks_physical_state_for_character_card() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let char_id = {
            let c = CharacterRepository::new(pool.clone())
                .create(CreateCharacterRequest {
                    story_id: story_id.clone(),
                    name: "明成公主".into(),
                    ..Default::default()
                })
                .unwrap();
            c.id
        };
        seed_real_machine_scenes(&pool, &story_id);

        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();

        let state = CharacterRepository::new(pool.clone())
            .get_character_state(&char_id)
            .unwrap()
            .expect("应写入状态行");
        let phys = state.physical_state.unwrap_or_default();
        assert!(phys.contains(DEAD_TAG), "phys={phys}");
        assert!(phys.contains("第2章"), "phys={phys}");
    }

    #[test]
    fn v142_is_idempotent() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_character(&pool, &story_id, "明成公主");
        seed_real_machine_scenes(&pool, &story_id);

        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();
        let first = dead_marker_map_on(&pool.get().unwrap(), &story_id).unwrap();
        // 重跑：列表不变、章次不漂移
        Migration.apply(&mut conn).unwrap();
        let second = dead_marker_map_on(&pool.get().unwrap(), &story_id).unwrap();
        assert_eq!(first, second);
        assert_eq!(second.len(), 1);
    }

    #[test]
    fn v142_keeps_distinct_people_alive() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_character(&pool, &story_id, "苏亦铁");
        seed_real_machine_scenes(&pool, &story_id);

        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();
        assert!(
            dead_marker_map_on(&pool.get().unwrap(), &story_id)
                .unwrap()
                .is_empty(),
            "正文只写死了公主，旁人不得连坐"
        );
    }
}
