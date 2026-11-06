use std::collections::HashSet;

use chrono::Local;
use rusqlite::{params, Connection};

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        155
    }

    fn description(&self) -> &'static str {
        "inherit payoff windows for foreshadowing fragments from checklist plan threads"
    }

    /// v0.84.1：本机 127 条未回收伏笔里，126 条是 ingest 自动提取的碎片，
    /// 自己没有任何「埋/收」计划文本；计划只写在一条「伏笔清单」blob 里。
    /// V154 只能回填 blob 本身，碎片仍无结构化窗口。
    ///
    /// 这里把清单 blob 按 `F1/F2/...` 行拆成计划线程：线程标签的短关键词
    /// （2–4 字前缀 + 3 字窗口）命中碎片正文时，碎片继承该线程的章号回收
    /// 窗口（target_start/target_end）。只有明确的章号计划可继承；卷号计划
    /// 无法映射到场景序号，仍留给文本提示与作者确认。
    ///
    /// 保守匹配：只认标签里的计划短语，不做全文模糊匹配；未命中的碎片保持
    /// 无窗口（自动提取线索，不代表作者计划）。幂等：只处理 target_end_scene
    /// 仍为 NULL 的 setup 行。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;
        let stories: Vec<String> = {
            let mut stmt = tx.prepare("SELECT id FROM stories")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let mut changed = 0usize;
        for story_id in &stories {
            let names = character_names(&tx, story_id);
            let rows: Vec<(String, String, Option<i32>)> = {
                let mut stmt = tx.prepare(
                    "SELECT id, content, target_end_scene FROM foreshadowing_tracker \
                     WHERE story_id = ?1 AND status = 'setup' \
                       AND content IS NOT NULL AND TRIM(content) <> ''",
                )?;
                let mapped = stmt.query_map([story_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
                let mut out = Vec::new();
                for row in mapped {
                    out.push(row?);
                }
                out
            };

            let mut threads = Vec::new();
            for (_, content, _) in &rows {
                if has_plan_lines(content) {
                    threads.extend(plan_threads(content, &names));
                }
            }
            if threads.is_empty() {
                continue;
            }

            for (id, content, target_end) in &rows {
                if target_end.is_some() || has_plan_lines(content) {
                    continue;
                }
                let normalized = normalize(content, &names);
                let Some(chapter) = best_thread_chapter(&normalized, &threads) else {
                    continue;
                };
                tx.execute(
                    "UPDATE foreshadowing_tracker \
                     SET target_end_scene = ?2, \
                         target_start_scene = COALESCE(target_start_scene, ?2) \
                     WHERE id = ?1 AND target_end_scene IS NULL",
                    params![id, chapter],
                )?;
                changed += 1;
            }
        }

        tx.commit()?;
        if changed > 0 {
            log::info!(
                "[V155] inherited payoff windows for {changed} foreshadowing fragments from checklist threads"
            );
        }
        Ok(())
    }
}

struct PlanThread {
    keywords: Vec<String>,
    payoff_chapter: i32,
}

fn character_names(conn: &Connection, story_id: &str) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT name FROM characters WHERE story_id = ?1") {
        if let Ok(rows) = stmt.query_map([story_id], |r| r.get::<_, String>(0)) {
            for row in rows.flatten() {
                let name = row.trim().to_string();
                if !name.is_empty() {
                    names.push(name);
                }
            }
        }
    }
    // 自动提取的碎片常写简称（「陈小满」→「小满」），去掉 3 字姓名的后两字
    // 以减少人名关键词造成的跨线程误匹配。
    let mut variants: Vec<String> = names.clone();
    for name in &names {
        let chars: Vec<char> = name.chars().collect();
        if chars.len() >= 3 {
            variants.push(chars[chars.len() - 2..].iter().collect());
        }
    }
    variants.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
    variants
}

fn has_plan_lines(content: &str) -> bool {
    content.lines().any(|line| {
        let line = line.trim();
        line.starts_with('F')
            && line
                .chars()
                .nth(1)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            && (line.contains('埋') || line.contains('收'))
    })
}

fn plan_threads(content: &str, names: &[String]) -> Vec<PlanThread> {
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        let Some(label) = plan_label(line) else {
            continue;
        };
        let Some(payoff_chapter) = payoff_chapter(line) else {
            continue;
        };
        // 计划行「埋」到「收」之间是这条线程的埋设描述；标签 + 埋设描述一起
        // 做关键词，能覆盖碎片改写措辞（抄诗/笔迹/沈字/练习本等）。
        let pre = line.find('收').map(|pos| &line[..pos]).unwrap_or(line);
        let keywords = thread_keywords(&label, pre, names);
        if keywords.is_empty() {
            continue;
        }
        out.push(PlanThread {
            keywords,
            payoff_chapter,
        });
    }
    out
}

fn plan_label(line: &str) -> Option<String> {
    if !line.starts_with('F') {
        return None;
    }
    let rest = line[1..]
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim();
    let label = rest.split(['—', '–', '：']).next()?.trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

/// 计划线程关键词：标签 + 埋设描述里拆掉角色名与结构助词后的 2–4 字前缀/
/// 后缀与 3 字窗口；通用词（档案/文件/修复/名字等）不参与匹配，避免把
/// 无关碎片挂到线程上。
fn thread_keywords(label: &str, pre: &str, names: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for source in [label, pre] {
        let cleaned = strip_names(source, names);
        for run in chinese_runs(&cleaned) {
            for seg in run.split(['的', '上', '中', '之', '与', '和', '及', '或', '第', '章'])
            {
                let seg = seg.trim();
                let count = seg.chars().count();
                if count < 2 {
                    continue;
                }
                let chars: Vec<char> = seg.chars().collect();
                for len in 2..=count.min(4) {
                    let prefix: String = chars[..len].iter().collect();
                    if !is_generic_keyword(&prefix) && seen.insert(prefix.clone()) {
                        out.push(prefix);
                    }
                    let suffix: String = chars[count - len..].iter().collect();
                    if !is_generic_keyword(&suffix) && seen.insert(suffix.clone()) {
                        out.push(suffix);
                    }
                }
                if count >= 3 {
                    for start in 0..=(count - 3) {
                        let keyword: String = chars[start..start + 3].iter().collect();
                        if !is_generic_keyword(&keyword) && seen.insert(keyword.clone()) {
                            out.push(keyword);
                        }
                    }
                }
            }
        }
    }
    out
}

/// 通用词：出现在太多碎片里，不能用来判断线程归属。
fn is_generic_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "档案"
            | "档案室"
            | "文件"
            | "记录"
            | "事情"
            | "发现"
            | "出现"
            | "可能"
            | "暗示"
            | "真相"
            | "身份"
            | "秘密"
            | "线索"
            | "内容"
            | "后果"
            | "证明"
            | "关联"
            | "关键"
            | "意味"
            | "人物"
            | "名字"
            | "姓名"
            | "故事"
            | "事件"
            | "场景"
            | "时间"
            | "过程"
            | "存在"
            | "情况"
            | "关系"
            | "行动"
            | "结果"
            | "选择"
            | "决定"
            | "危险"
            | "安全"
            | "计划"
            | "目标"
            | "需要"
            | "应该"
            | "已经"
            | "一直"
            | "自己"
            | "他人"
            | "手中"
            | "手上"
            | "背后"
            | "当时"
            | "现在"
            | "未来"
            | "过去"
            | "一种"
            | "一个"
            | "部分"
            | "细节"
            | "信息"
            | "证据"
            | "材料"
            | "签字"
            | "签名"
            | "复印"
            | "复件"
            | "纸张"
            | "修复"
            | "开始"
            | "结束"
            | "之前"
            | "之后"
    )
}

fn normalize(content: &str, names: &[String]) -> String {
    strip_names(content, names)
}

fn strip_names(text: &str, names: &[String]) -> String {
    let mut out = text.to_string();
    for name in names {
        out = out.replace(name, "");
    }
    out
}

fn chinese_runs(text: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if is_chinese(c) {
            current.push(c);
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

fn is_chinese(c: char) -> bool {
    matches!(c, '\u{4e00}'..='\u{9fff}')
}

fn best_thread_chapter(normalized: &str, threads: &[PlanThread]) -> Option<i32> {
    let mut best: Option<(usize, i32)> = None;
    for thread in threads {
        let mut longest = 0usize;
        for keyword in &thread.keywords {
            if normalized.contains(keyword) {
                longest = longest.max(keyword.chars().count());
            }
        }
        if longest < 2 {
            continue;
        }
        if best.map(|(len, _)| longest > len).unwrap_or(true) {
            best = Some((longest, thread.payoff_chapter));
        }
    }
    best.map(|(_, chapter)| chapter)
}

/// 从计划行里取「收/回收：第N章」的章号；跨卷（卷2）不返回数字。
fn payoff_chapter(line: &str) -> Option<i32> {
    let pos = line.find('收')?;
    let tail = &line[pos..];
    // 只认「第N章」；若「第」后面不是章号（如第二卷），交给卷号提示。
    let hash = tail.find('第')?;
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
    parse_chapter_number(&raw).filter(|n| *n >= 1)
}

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

    fn windows(conn: &Connection, id: &str) -> (Option<i32>, Option<i32>) {
        conn.query_row(
            "SELECT target_start_scene, target_end_scene FROM foreshadowing_tracker WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn v155_inherits_windows_from_numeric_plan_threads_only() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '测试', ?1, ?1)",
            [&now],
        )
        .unwrap();

        insert_foreshadowing(
            &conn,
            "plan",
            "s1",
            "【伏笔清单】\n\
             F1 代签笔迹——埋：第1章；回收：第12章\n\
             F2 缺页的复印件——埋：第1章；回收：卷2\n\
             F3 红章「副本已核」——埋：第3章；回收：卷2\n\
             F4 女儿练习本上的诗——埋：第1章；回收：第12章\n\
             F5 沈姓名字——埋：第2章；回收：第11章",
        );
        insert_foreshadowing(&conn, "f1", "s1", "代签笔迹与修复员名录的笔迹特写");
        insert_foreshadowing(&conn, "f2", "s1", "缺页的复印件被裁走");
        insert_foreshadowing(&conn, "f4", "s1", "小满练习本上的诗页被雨泡开");
        insert_foreshadowing(&conn, "f5", "s1", "陈砚对沈姓卷宗的刻意回避");
        insert_foreshadowing(&conn, "none", "s1", "门牌被水泥抹去");

        Migration.apply(&mut conn).unwrap();

        assert_eq!(windows(&conn, "f1"), (Some(12), Some(12)));
        assert_eq!(
            windows(&conn, "f2"),
            (None, None),
            "卷号计划不能伪造章号窗口"
        );
        assert_eq!(windows(&conn, "f4"), (Some(12), Some(12)));
        assert_eq!(windows(&conn, "f5"), (Some(11), Some(11)));
        assert_eq!(windows(&conn, "none"), (None, None));

        // 幂等：重跑不覆盖已有窗口
        conn.execute(
            "UPDATE foreshadowing_tracker SET target_end_scene = 30 WHERE id = 'f4'",
            [],
        )
        .unwrap();
        Migration.apply(&mut conn).unwrap();
        assert_eq!(windows(&conn, "f4"), (Some(12), Some(30)));
    }
}
