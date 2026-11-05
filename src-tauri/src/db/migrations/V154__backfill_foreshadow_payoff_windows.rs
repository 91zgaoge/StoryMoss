use chrono::Local;
use rusqlite::{params, Connection};

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        154
    }

    fn description(&self) -> &'static str {
        "backfill foreshadow payoff windows from plan text (收：第N章)"
    }

    /// v0.82.6：旧数据/创世清单常把回收计划写在 `content` 文本里
    /// （「埋：第1章；收：第9章」），而 `target_start_scene/target_end_scene`
    /// 为空——伏笔看板不显示计划窗口、续写 prompt 拿不到回收窗口。
    ///
    /// 对 target_end_scene 为空的未回收伏笔，从文本解析「收/回收：第N章」
    /// （阿拉伯/中文数字），取最早的计划章号回填 target_end_scene。
    /// 幂等：只处理 target_end_scene IS NULL 的行，重跑不重复写。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let now = Local::now().to_rfc3339();
        let tx = conn.transaction()?;
        let rows: Vec<(String, String)> = {
            let mut stmt = tx.prepare(
                "SELECT id, content FROM foreshadowing_tracker \
                 WHERE target_end_scene IS NULL AND status = 'setup' \
                   AND content IS NOT NULL AND TRIM(content) <> ''",
            )?;
            let mapped = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            let mut out = Vec::new();
            for row in mapped {
                out.push(row?);
            }
            out
        };

        let mut changed = 0usize;
        for (id, content) in rows {
            let Some(end) = planned_payoff_chapters(&content) else {
                continue;
            };
            tx.execute(
                "UPDATE foreshadowing_tracker SET target_end_scene = ?2 WHERE id = ?1 \
                 AND target_end_scene IS NULL",
                params![id, end],
            )?;
            changed += 1;
        }

        tx.commit()?;
        let _ = now;
        if changed > 0 {
            log::info!(
                "[V154] backfilled payoff windows for {changed} foreshadowings from plan text"
            );
        }
        Ok(())
    }
}

/// 收集文本里的全部回收计划章号（「收：第4章」「回收：第九章」）。
fn planned_payoff_chapters(content: &str) -> Option<i32> {
    let mut out: Vec<i32> = Vec::new();
    for line in content.lines() {
        let Some(pos) = line.find('收') else {
            continue;
        };
        let tail = &line[pos..];
        let Some(hash) = tail.find('第') else {
            continue;
        };
        let after: String = tail[hash..].chars().skip(1).collect();
        let mut raw = String::new();
        for c in after.chars() {
            if c == '章' {
                break;
            }
            if c.is_whitespace() {
                continue;
            }
            raw.push(c);
        }
        if let Some(n) = parse_chapter_number(&raw) {
            if n >= 1 && !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out.into_iter().min()
}

/// 解析「4」「9」「十二」「一百零八」这类章号（0 视为无效）。
fn parse_chapter_number(raw: &str) -> Option<i32> {
    if raw.is_empty() {
        return None;
    }
    if raw.chars().all(|c| c.is_ascii_digit()) {
        return raw.parse::<i32>().ok();
    }
    const DIGITS: &[(char, i32)] = &[
        ('零', 0),
        ('〇', 0),
        ('一', 1),
        ('二', 2),
        ('两', 2),
        ('三', 3),
        ('四', 4),
        ('五', 5),
        ('六', 6),
        ('七', 7),
        ('八', 8),
        ('九', 9),
    ];
    let mut total = 0i32;
    let mut current = 0i32;
    let mut seen = false;
    for c in raw.chars() {
        if let Some((_, v)) = DIGITS.iter().find(|(d, _)| *d == c) {
            current = *v;
            seen = true;
        } else {
            let unit = match c {
                '十' => 10,
                '百' => 100,
                '千' => 1000,
                _ => return None,
            };
            if current == 0 {
                current = 1;
            }
            total += current * unit;
            current = 0;
            seen = true;
        }
    }
    let value = total + current;
    if seen && value > 0 {
        Some(value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;

    fn insert_foreshadowing(conn: &Connection, id: &str, story_id: &str, content: &str) {
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO foreshadowing_tracker \
             (id, story_id, content, status, importance, created_at) \
             VALUES (?1, ?2, ?3, 'setup', 5, ?4)",
            params![id, story_id, content, now],
        )
        .unwrap();
    }

    fn target(conn: &Connection, id: &str) -> Option<i32> {
        conn.query_row(
            "SELECT target_end_scene FROM foreshadowing_tracker WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn v154_backfills_arabic_and_chinese_plan_windows() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '测试', ?1, ?1)",
            [&now],
        )
        .unwrap();

        insert_foreshadowing(&conn, "f1", "s1", "伏笔清单：F1 代签——埋：第1章；收：第9章");
        insert_foreshadowing(
            &conn,
            "f2",
            "s1",
            "伏笔清单：F2 门牌——收：第十二章；收：第20章",
        );
        insert_foreshadowing(&conn, "f3", "s1", "没有计划窗口的线索");

        Migration.apply(&mut conn).unwrap();

        assert_eq!(target(&conn, "f1"), Some(9));
        assert_eq!(target(&conn, "f2"), Some(12), "多条计划取最早");
        assert_eq!(target(&conn, "f3"), None, "无计划文本不写");

        // 幂等：重跑不覆盖已有窗口
        conn.execute(
            "UPDATE foreshadowing_tracker SET target_end_scene = 30 WHERE id = 'f1'",
            [],
        )
        .unwrap();
        Migration.apply(&mut conn).unwrap();
        assert_eq!(target(&conn, "f1"), Some(30));
    }
}
