//! 资产落库：把黑板资产区的条目物化到应用资产表（characters/world_buildings/
//! story_outlines）。 character 条目 content 须为 JSON
//! {"name","background","personality","goals"}； world/outline 条目 content
//! 为纯文本。解析失败的条目跳过并 log::warn!。

use rusqlite::params;

use crate::{agency::models::BoardItem, db::DbPool};

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn materialize_assets(pool: &DbPool, story_id: &str, items: &[BoardItem]) -> usize {
    let mut count = 0usize;
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("materialize_assets: pool 获取失败: {}", e);
            return 0;
        }
    };
    for item in items.iter().filter(|i| i.status == "active") {
        match item.item_type.as_str() {
            "character" => {
                let parsed =
                    crate::agency::coordinator::parse_lenient::<serde_json::Value>(&item.content);
                let (name, background, personality, goals) = match parsed.as_ref() {
                    Some(v) => (
                        v.get("name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("background")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("personality")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("goals")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                    ),
                    None => {
                        log::warn!("materialize: 角色条目 {} 非 JSON，跳过", item.key);
                        continue;
                    }
                };
                if name.is_empty() {
                    log::warn!("materialize: 角色条目 {} 缺 name，跳过", item.key);
                    continue;
                }
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                match conn.execute(
                    "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'agency', 1, ?7, ?8)",
                    params![id, story_id, name, background, personality, goals, ts, ts],
                ) {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("materialize: 插入角色失败: {}", e),
                }
            }
            "world" => {
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                let result = conn.execute(
                    "INSERT INTO world_buildings (id, story_id, concept, rules, source, is_auto_generated, created_at, updated_at)
                     VALUES (?1, ?2, ?3, '[]', 'agency', 1, ?4, ?5)
                     ON CONFLICT(story_id) DO UPDATE SET concept = excluded.concept, updated_at = excluded.updated_at",
                    params![id, story_id, item.content, ts, ts],
                );
                match result {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("materialize: 写入世界观失败: {}", e),
                }
            }
            "outline" => {
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                let result = conn.execute(
                    "INSERT INTO story_outlines (id, story_id, content, act_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 3, ?4, ?5)
                     ON CONFLICT(story_id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
                    params![id, story_id, item.content, ts, ts],
                );
                match result {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("materialize: 写入大纲失败: {}", e),
                }
            }
            _ => {} // foreshadowing 等 P2 不落库
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agency::models::*,
        db::{create_test_pool, dto::CreateStoryRequest, repositories::StoryRepository},
    };

    fn story(pool: &crate::db::DbPool, id: &str) {
        let s = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "测试书".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        // create 生成自己的 id；测试统一改用其返回值
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE stories SET id = ?1 WHERE id = ?2",
            rusqlite::params![id, s.id],
        )
        .unwrap();
    }

    fn item(item_type: &str, key: &str, content: &str) -> BoardItem {
        BoardItem::new(
            "r1",
            "s1",
            BoardZone::Asset,
            item_type,
            key,
            content,
            "摘要",
            AgentRole::Producer,
            "active",
        )
    }

    #[test]
    fn test_materialize_character_json() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item(
            "character",
            "主角",
            r#"{"name":"阿苔","background":"拾荒者","personality":"坚韧","goals":"找到星环"}"#,
        )];
        let n = materialize_assets(&pool, "s1", &items);
        assert_eq!(n, 1);
        let conn = pool.get().unwrap();
        let name: String = conn
            .query_row("SELECT name FROM characters WHERE story_id='s1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "阿苔");
        // 重复插入（story_id+name 去重）：跳过不计数，仍一行
        assert_eq!(materialize_assets(&pool, "s1", &items), 0);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM characters WHERE story_id='s1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_materialize_world_upsert_idempotent() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("world", "世界观", "双星废土，磁力风暴")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        // 再次执行不报错（upsert），仍一行
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM world_buildings WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_materialize_skips_non_json_character() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("character", "主角", "自由文本不是 JSON")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 0);
        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM characters WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_materialize_outline() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("outline", "第一卷", "第一卷大纲：起承转合……")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        let conn = pool.get().unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM story_outlines WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(content.contains("起承转合"));
    }
}
