#![allow(dead_code)]
//! 终局指南针（P3-D，v0.63.0）。
//!
//! 对应外部项目（ainovel-cli Compass）的轻量做法：长篇需要一份「不随章节膨胀
//! 的方向锚」——终局方向、活跃长线、规模估计。本模块**确定性派生**它
//! （不额外消耗 LLM）：
//! - 终局方向 ← 故事大纲中的「核心冲突」行（创世/资产回流写入）；
//! - 活跃长线 ← 未回收伏笔（importance 降序前 5）；
//! - 规模 ← 已提交章数与段摘要进度。
//!
//! 派生视图的好处是永远与真相同步、无需「卷末更新」的纪律；代价是无法承载
//! 作者独有的终局构想——需要时可在后续版本升级为可编辑资产。

use rusqlite::params;

use crate::db::DbPool;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoryCompass {
    pub ending_direction: Option<String>,
    pub open_threads: Vec<String>,
    pub scale: Option<String>,
}

/// 从故事大纲文本里提取「核心冲突」一行（asset_bridge 写入的块格式）。
fn extract_core_conflict(outline: &str) -> Option<String> {
    let lines: Vec<&str> = outline.lines().map(str::trim).collect();
    for (index, line) in lines.iter().enumerate() {
        let Some(rest) = line
            .strip_prefix("【核心冲突】")
            .or_else(|| line.strip_prefix("核心冲突："))
            .or_else(|| line.strip_prefix("核心冲突:"))
        else {
            continue;
        };
        let value = rest.trim();
        if !value.is_empty() {
            return Some(value.chars().take(120).collect());
        }
        // 标记单独成行：取下一非空行
        if let Some(next) = lines[index + 1..].iter().find(|l| !l.is_empty()) {
            if !next.starts_with('【') {
                return Some(next.chars().take(120).collect());
            }
        }
    }
    None
}

/// 构建指南针（纯确定性派生）。
pub fn build_compass(pool: &DbPool, story_id: &str) -> StoryCompass {
    let mut compass = StoryCompass::default();
    let Ok(conn) = pool.get() else {
        return compass;
    };

    if let Ok(content) = conn.query_row(
        "SELECT content FROM story_outlines WHERE story_id = ?1",
        params![story_id],
        |row| row.get::<_, String>(0),
    ) {
        compass.ending_direction = extract_core_conflict(&content);
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT content FROM foreshadowing_tracker \
         WHERE story_id = ?1 AND status = 'setup' ORDER BY importance DESC, created_at ASC LIMIT 5",
    ) {
        if let Ok(rows) = stmt.query_map(params![story_id], |row| row.get::<_, String>(0)) {
            compass.open_threads = rows.flatten().collect();
        }
    }

    let max_chapter: Option<i32> = conn
        .query_row(
            "SELECT MAX(chapter_number) FROM scene_commits WHERE story_id = ?1",
            params![story_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    let segments =
        crate::story_system::segment_summary::load_segment_summaries(pool, story_id).len();
    if let Some(chapter) = max_chapter.filter(|c| *c > 0) {
        compass.scale = Some(format!("已提交 {chapter} 章 / {segments} 段摘要"));
    }

    compass
}

/// 渲染续写注入块；方向与长线都为空时不产生空块。
pub fn render_compass_block(pool: &DbPool, story_id: &str) -> Option<String> {
    let compass = build_compass(pool, story_id);
    let mut lines: Vec<String> = Vec::new();
    if let Some(direction) = &compass.ending_direction {
        lines.push(format!("终局方向：{direction}"));
    }
    if !compass.open_threads.is_empty() {
        lines.push(format!("活跃长线：{}", compass.open_threads.join("；")));
    }
    if let Some(scale) = &compass.scale {
        lines.push(format!("进度：{scale}"));
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "【终局指南针（长线方向锚，勿在支线中丢失）】\n{}",
        lines.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_story(pool: &DbPool) -> String {
        let story_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '指南针', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        story_id
    }

    #[test]
    fn extract_core_conflict_reads_block_format() {
        // 真实写入格式：标记与值同行（asset_bridge）＋兼容「标记单独成行」
        let outline = "【核心冲突】林家与镇北王府的旧案真相\n【转折点】玉佩易主";
        let standalone = "【核心冲突】\n林家与镇北王府的旧案真相\n【转折点】玉佩易主";
        assert_eq!(
            extract_core_conflict(outline).as_deref(),
            Some("林家与镇北王府的旧案真相")
        );
        assert_eq!(
            extract_core_conflict(standalone).as_deref(),
            Some("林家与镇北王府的旧案真相")
        );
        assert!(extract_core_conflict("没有冲突块").is_none());
    }

    #[test]
    fn compass_derives_direction_threads_and_scale() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        {
            let conn = pool.get().unwrap();
            let now = chrono::Local::now().to_rfc3339();
            conn.execute(
                "INSERT INTO story_outlines (id, story_id, content, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    &story_id,
                    "【核心冲突】仇家上门\n【转折点】玉佩易主",
                    &now
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO foreshadowing_tracker (id, story_id, content, status, created_at, importance) \
                 VALUES (?1, ?2, '玉佩的真正主人', 'setup', ?3, 9)",
                params![uuid::Uuid::new_v4().to_string(), &story_id, &now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO scene_commits (id, story_id, chapter_number, status, created_at) \
                 VALUES (?1, ?2, 7, 'accepted', ?3)",
                params![uuid::Uuid::new_v4().to_string(), &story_id, &now],
            )
            .unwrap();
        }

        let compass = build_compass(&pool, &story_id);
        assert_eq!(compass.ending_direction.as_deref(), Some("仇家上门"));
        assert_eq!(compass.open_threads, vec!["玉佩的真正主人"]);
        assert!(compass.scale.unwrap().contains("7 章"));

        let block = render_compass_block(&pool, &story_id).expect("应有块");
        assert!(block.contains("终局方向"));
        assert!(block.contains("活跃长线"));
    }

    #[test]
    fn empty_story_yields_none() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        assert!(render_compass_block(&pool, &story_id).is_none());
    }
}
